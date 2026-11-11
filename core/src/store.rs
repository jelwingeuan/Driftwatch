use crate::Item;
use rusqlite::{params, Connection, Result};
use sha2::{Digest, Sha256};
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

    fn init(mut conn: Connection) -> Result<Store> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snapshots (
                 id INTEGER PRIMARY KEY, taken_at INTEGER NOT NULL, hash TEXT);
             CREATE TABLE IF NOT EXISTS items (
                 snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
                 category TEXT NOT NULL, key TEXT NOT NULL, json TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS items_snapshot ON items(snapshot_id);",
        )?;
        // A database from before hashes gets them once, oldest first, from what each
        // snapshot holds now: the history as it stands, edits and all, starts the chain.
        let hashed: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('snapshots') WHERE name = 'hash'",
            [],
            |r| r.get(0),
        )?;
        if !hashed {
            let tx = conn.transaction()?;
            tx.execute_batch("ALTER TABLE snapshots ADD COLUMN hash TEXT")?;
            let ids: Vec<i64> = tx
                .prepare("SELECT id FROM snapshots ORDER BY id")?
                .query_map([], |r| r.get(0))?
                .collect::<Result<_>>()?;
            for id in ids {
                let hash = chain_hash(&tx, id)?;
                tx.execute(
                    "UPDATE snapshots SET hash = ?1 WHERE id = ?2",
                    params![hash, id],
                )?;
            }
            tx.commit()?;
        }
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
        let hash = chain_hash(&tx, id)?;
        tx.execute(
            "UPDATE snapshots SET hash = ?1 WHERE id = ?2",
            params![hash, id],
        )?;
        tx.commit()?;
        Ok(snapshot)
    }

    /// Whether every snapshot's stored hash still matches `chain_hash`: false once a
    /// snapshot or item was edited, added or deleted outside Driftwatch, unless whoever
    /// did it also recomputed the hashes. Deleting the newest snapshots goes unseen.
    pub fn history_intact(&self) -> Result<bool> {
        let rows: Vec<(i64, Option<String>)> = self
            .conn
            .prepare("SELECT id, hash FROM snapshots ORDER BY id")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_>>()?;
        for (id, hash) in rows {
            if hash != Some(chain_hash(&self.conn, id)?) {
                return Ok(false);
            }
        }
        Ok(true)
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

/// Snapshot `id`'s link in the history chain: SHA-256 hex over the stored hash of
/// the snapshot before it ("" for the first), then its id, its taken_at and each
/// of its items' category, key and json as stored, in order. Numbers are 8 bytes
/// big-endian and each text comes after its length in bytes, so no two different
/// histories encode the same.
fn chain_hash(conn: &Connection, id: i64) -> Result<String> {
    let (taken_at, prev): (i64, String) = conn.query_row(
        "SELECT taken_at, COALESCE(
             (SELECT hash FROM snapshots WHERE id < ?1 ORDER BY id DESC LIMIT 1), '')
         FROM snapshots WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    fn text(h: &mut Sha256, s: &str) {
        h.update((s.len() as u64).to_be_bytes());
        h.update(s);
    }
    let mut h = Sha256::new();
    text(&mut h, &prev);
    h.update(id.to_be_bytes());
    h.update(taken_at.to_be_bytes());
    let mut stmt = conn.prepare_cached(
        "SELECT category, key, json FROM items WHERE snapshot_id = ?1 ORDER BY rowid",
    )?;
    let mut rows = stmt.query([id])?;
    while let Some(r) = rows.next()? {
        for i in 0..3 {
            text(&mut h, r.get_ref(i)?.as_str()?);
        }
    }
    Ok(crate::hex(&h.finalize()))
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

    /// Change the database behind Driftwatch's back.
    fn edit(store: &Store, sql: &str) {
        store.conn.execute_batch(sql).unwrap();
    }

    fn three_scans() -> Store {
        let mut store = Store::open_in_memory().unwrap();
        for keys in [&["a"][..], &["a", "b"], &["b", "c"]] {
            let items: Vec<Item> = keys.iter().map(|k| item(k)).collect();
            store.save_snapshot(&items).unwrap();
        }
        store
    }

    #[test]
    fn history_is_intact_until_edited_outside_driftwatch() {
        assert!(Store::open_in_memory().unwrap().history_intact().unwrap());
        assert!(three_scans().history_intact().unwrap());
        for sql in [
            r#"UPDATE items SET json = replace(json, '"b"', '"x"') WHERE snapshot_id = 2"#,
            "UPDATE snapshots SET taken_at = taken_at - 60 WHERE id = 2",
            "UPDATE snapshots SET hash = NULL WHERE id = 3",
            "INSERT INTO items VALUES (2, 'cron', 'job', '{}')",
            // The same bytes, split between fields another way.
            "UPDATE items SET category = 'launch', key = 'd' || key WHERE snapshot_id = 2",
        ] {
            let store = three_scans();
            edit(&store, sql);
            assert!(!store.history_intact().unwrap(), "{sql}");
        }
    }

    #[test]
    fn deleting_a_snapshot_or_an_item_breaks_the_chain() {
        for sql in [
            "DELETE FROM items WHERE snapshot_id = 2 AND key = 'b'",
            "DELETE FROM items WHERE snapshot_id = 2; DELETE FROM snapshots WHERE id = 2",
            "DELETE FROM items WHERE snapshot_id = 1; DELETE FROM snapshots WHERE id = 1",
        ] {
            let store = three_scans();
            edit(&store, sql);
            assert!(!store.history_intact().unwrap(), "{sql}");
        }
    }

    #[test]
    fn snapshots_from_before_hashes_start_the_chain_as_they_are() {
        // A database from before hashes, with a scan that was edited back then.
        let old = three_scans();
        edit(
            &old,
            r#"ALTER TABLE snapshots DROP COLUMN hash;
               UPDATE items SET json = replace(json, '"c"', '"x"') WHERE snapshot_id = 3;"#,
        );
        let mut store = Store::init(old.conn).unwrap();
        assert!(store.history_intact().unwrap());
        store.save_snapshot(&[item("d")]).unwrap();
        assert!(store.history_intact().unwrap());
        edit(
            &store,
            r#"UPDATE items SET json = replace(json, '"a"', '"x"') WHERE snapshot_id = 1"#,
        );
        assert!(!store.history_intact().unwrap());
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
