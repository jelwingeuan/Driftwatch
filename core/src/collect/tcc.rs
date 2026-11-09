use super::error_item;
use crate::Item;
use rusqlite::{Connection, OpenFlags};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const CATEGORY: &str = "tcc";

/// Privacy grants from the user and system TCC databases (needs Full Disk Access).
pub fn collect() -> Vec<Item> {
    let mut dbs = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        let p = Path::new(&home).join("Library/Application Support/com.apple.TCC/TCC.db");
        dbs.push((p, "user"));
    }
    let system = PathBuf::from("/Library/Application Support/com.apple.TCC/TCC.db");
    dbs.push((system, "system"));
    dbs.iter().flat_map(|(p, s)| read_db(p, s)).collect()
}

/// Read one TCC.db strictly read-only.
/// Key = "<scope>: <service> <client> (type <client_type>)[ -> <indirect object>]".
/// A missing DB yields nothing; an unreadable one yields a single error Item.
pub fn read_db(path: &Path, scope: &str) -> Vec<Item> {
    // Not `Path::exists`: it hides permission errors, which must surface as an error Item.
    if matches!(std::fs::metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound) {
        return Vec::new();
    }
    query(path, scope).unwrap_or_else(|e| {
        let msg = format!("needs Full Disk Access ({e})");
        vec![error_item(CATEGORY, &path.to_string_lossy(), &msg)]
    })
}

fn query(path: &Path, scope: &str) -> rusqlite::Result<Vec<Item>> {
    let encoded: String = path
        .to_string_lossy()
        .chars()
        .map(|c| match c {
            '%' | '?' | '#' | ' ' => format!("%{:02X}", c as u32),
            c => c.to_string(),
        })
        .collect();
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI;
    let conn = Connection::open_with_flags(format!("file:{encoded}?immutable=1"), flags)?;
    let mut stmt = conn.prepare(
        "SELECT service, client, client_type, auth_value, indirect_object_identifier FROM access",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, Option<String>>(4)?,
        ))
    })?;
    let mut items = Vec::new();
    for row in rows {
        let (service, client, client_type, auth, indirect) = row?;
        let mut fields = BTreeMap::from([
            ("scope".to_string(), scope.to_string()),
            ("service".to_string(), service.clone()),
            ("client".to_string(), client.clone()),
            ("client_type".to_string(), client_type.to_string()),
            ("auth_value".to_string(), auth.to_string()),
        ]);
        let mut key = format!("{scope}: {service} {client} (type {client_type})");
        // "UNUSED" is the column default when there is no target (e.g. Apple Events).
        if let Some(target) = indirect.filter(|t| t != "UNUSED") {
            key.push_str(&format!(" -> {target}"));
            fields.insert("indirect_object".into(), target);
        }
        items.push(Item::new(CATEGORY, &key, fields));
    }
    Ok(items)
}
