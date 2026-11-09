use crate::Item;
use rusqlite::{params, Connection, Result};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Store {
    conn: Connection,
}

/// ~/Library/Application Support/Driftwatch/driftwatch.db
pub fn default_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    Path::new(&home).join("Library/Application Support/Driftwatch/driftwatch.db")
}

impl Store {
    /// Open (and create, with its parent dir) the database at `path`.
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Store::init(Connection::open(path)?)
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

    pub fn save_snapshot(&mut self, items: &[Item]) -> Result<i64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        let tx = self.conn.transaction()?;
        tx.execute("INSERT INTO snapshots (taken_at) VALUES (?1)", [now])?;
        let id = tx.last_insert_rowid();
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
        Ok(id)
    }

    /// Items of the most recent snapshot, or None if there is no snapshot yet.
    pub fn latest_items(&self) -> Result<Option<Vec<Item>>> {
        let id: Option<i64> = self
            .conn
            .query_row("SELECT MAX(id) FROM snapshots", [], |r| r.get(0))?;
        let Some(id) = id else { return Ok(None) };
        let mut stmt = self
            .conn
            .prepare("SELECT json FROM items WHERE snapshot_id = ?1 ORDER BY rowid")?;
        let items = stmt
            .query_map([id], |r| r.get::<_, String>(0))?
            .map(|json| json.map(|j| serde_json::from_str(&j).expect("stored Item json is valid")))
            .collect::<Result<Vec<Item>>>()?;
        Ok(Some(items))
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
        assert!(b > a);
        assert_eq!(
            store.latest_items().unwrap(),
            Some(vec![item("b"), item("c")])
        );
    }
}
