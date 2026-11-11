use crate::Item;
use rusqlite::{params, Connection, Result};
use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Store {
    conn: Connection,
}

/// A saved scan. `taken_at` is RFC 3339 in UTC, e.g. "2026-11-09T15:45:29Z".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub id: i64,
    pub taken_at: String,
}

const TAKEN_AT: &str = "strftime('%Y-%m-%dT%H:%M:%SZ', taken_at, 'unixepoch')";

/// ~/Library/Application Support/Driftwatch/driftwatch.db
pub fn default_path() -> Result<PathBuf, std::env::VarError> {
    let home = std::env::var("HOME")?;
    Ok(Path::new(&home).join("Library/Application Support/Driftwatch/driftwatch.db"))
}

impl Store {
    /// Open (and create, with its parent dir) the database at `path`. Each time,
    /// only this user is let in: the parent dir is set to 0700, the database and
    /// any -wal and -shm files next to it to 0600.
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
            let _ = fs::set_permissions(dir, Permissions::from_mode(0o700));
        }
        let store = Store::init(Connection::open(path)?)?;
        for suffix in ["", "-wal", "-shm"] {
            let mut file = path.as_os_str().to_owned();
            file.push(suffix);
            let _ = fs::set_permissions(file, Permissions::from_mode(0o600));
        }
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Store> {
        Store::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Store> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snapshots (
                 id INTEGER PRIMARY KEY, taken_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS items (
                 snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
                 category TEXT NOT NULL, key TEXT NOT NULL, json TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS items_snapshot ON items(snapshot_id);",
        )?;
        Ok(Store { conn })
    }

    pub fn save_snapshot(&mut self, items: &[Item]) -> Result<Snapshot> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        let tx = self.conn.transaction()?;
        let snapshot = tx.query_row(
            &format!("INSERT INTO snapshots (taken_at) VALUES (?1) RETURNING id, {TAKEN_AT}"),
            [now],
            |r| {
                Ok(Snapshot {
                    id: r.get(0)?,
                    taken_at: r.get(1)?,
                })
            },
        )?;
        let id = snapshot.id;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO items (snapshot_id, category, key, json) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for item in items {
                let json = serde_json::to_string(item).expect("Item serializes");
                stmt.execute(params![id, item.category, item.key, json])?;
            }
        }
        tx.commit()?;
        Ok(snapshot)
    }

    /// Every snapshot, newest first.
    pub fn snapshots(&self) -> Result<Vec<Snapshot>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT id, {TAKEN_AT} FROM snapshots ORDER BY id DESC"
        ))?;
        let rows = stmt.query_map([], |r| {
            Ok(Snapshot {
                id: r.get(0)?,
                taken_at: r.get(1)?,
            })
        })?;
        rows.collect()
    }

    /// The time `days` days ago, written like `Snapshot::taken_at`.
    pub fn days_ago(&self, days: u32) -> Result<String> {
        self.conn.query_row(
            "SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ?1)",
            [format!("-{days} days")],
            |r| r.get(0),
        )
    }

    /// Items of one snapshot, in the order they were saved.
    pub fn items(&self, snapshot_id: i64) -> Result<Vec<Item>> {
        let mut stmt = self
            .conn
            .prepare("SELECT json FROM items WHERE snapshot_id = ?1 ORDER BY rowid")?;
        let rows = stmt.query_map([snapshot_id], |r| r.get::<_, String>(0))?;
        rows.map(|json| json.map(|j| serde_json::from_str(&j).expect("stored Item json is valid")))
            .collect()
    }

    /// Items of the most recent snapshot, or None if there is no snapshot yet.
    pub fn latest_items(&self) -> Result<Option<Vec<Item>>> {
        let id: Option<i64> = self
            .conn
            .query_row("SELECT MAX(id) FROM snapshots", [], |r| r.get(0))?;
        id.map(|id| self.items(id)).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn item(key: &str) -> Item {
        Item::new(
            "launchd",
            key,
            BTreeMap::from([("Label".to_string(), key.to_string())]),
        )
    }

    #[test]
    fn empty_store_has_no_snapshot() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.latest_items().unwrap(), None);
    }

    #[test]
    fn latest_returns_most_recent_snapshot() {
        let mut store = Store::open_in_memory().unwrap();
        let a = store.save_snapshot(&[item("a")]).unwrap();
        let b = store.save_snapshot(&[item("b"), item("c")]).unwrap();
        assert!(b.id > a.id);
        assert_eq!(
            store.latest_items().unwrap(),
            Some(vec![item("b"), item("c")])
        );
    }

    #[test]
    fn open_lets_only_this_user_in() {
        let dir = std::env::temp_dir().join(format!("driftwatch-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let db = dir.join("driftwatch.db");
        let files = ["", "-wal", "-shm"].map(|s| format!("{}{s}", db.display()));
        let mode = |p: &str| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        let dir = dir.to_str().unwrap();

        drop(Store::open(&db).unwrap());
        assert_eq!((mode(dir), mode(&files[0])), (0o700, 0o600));

        // Loosened since, with -wal and -shm files held open by another connection.
        let other = Connection::open(&db).unwrap();
        other
            .execute_batch("PRAGMA journal_mode = WAL; CREATE TABLE t (x);")
            .unwrap();
        fs::set_permissions(dir, Permissions::from_mode(0o755)).unwrap();
        for f in &files {
            fs::set_permissions(f, Permissions::from_mode(0o644)).unwrap();
        }
        let _store = Store::open(&db).unwrap();
        assert_eq!(mode(dir), 0o700);
        assert_eq!(files.each_ref().map(|f| mode(f)), [0o600; 3]);
        drop(other);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn days_ago_is_written_like_taken_at() {
        let mut store = Store::open_in_memory().unwrap();
        let now = store.save_snapshot(&[]).unwrap().taken_at;
        let day_ago = store.days_ago(1).unwrap();
        assert_eq!(day_ago.len(), now.len());
        assert!(day_ago < now);
        assert!(store.days_ago(0).unwrap() >= now);
    }
}
