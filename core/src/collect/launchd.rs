use super::read_file;
use crate::Item;
use plist::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const CATEGORY: &str = "launchd";

/// The standard launchd directories (not /System) with their scope labels.
/// The user dir is left out when HOME is unset.
pub fn default_dirs() -> Vec<(PathBuf, &'static str)> {
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push((Path::new(&home).join("Library/LaunchAgents"), "user agent"));
    }
    dirs.push((PathBuf::from("/Library/LaunchAgents"), "global agent"));
    dirs.push((PathBuf::from("/Library/LaunchDaemons"), "global daemon"));
    dirs
}

pub fn collect() -> Vec<Item> {
    collect_dirs(&default_dirs())
}

/// Read every *.plist in the given dirs. Missing dirs are skipped; anything
/// unreadable becomes an Item with an `error` field so it is never silently missed.
pub fn collect_dirs(dirs: &[(PathBuf, &str)]) -> Vec<Item> {
    let mut items = Vec::new();
    for (dir, scope) in dirs {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                items.push(error_item(&dir.to_string_lossy(), scope, &e.to_string()));
                continue;
            }
        };
        let mut paths = Vec::new();
        for (i, entry) in entries.enumerate() {
            match entry {
                Ok(e) => paths.push(e.path()),
                Err(e) => {
                    let key = format!("{}#entry{i}", dir.to_string_lossy());
                    items.push(error_item(&key, scope, &e.to_string()));
                }
            }
        }
        paths.retain(|p| p.extension().is_some_and(|x| x == "plist"));
        paths.sort();
        for path in paths {
            items.push(read_plist(&path, scope));
        }
    }
    items
}

fn read_plist(path: &Path, scope: &str) -> Item {
    let key = path.to_string_lossy();
    let bytes = match read_file(path) {
        Ok(b) => b,
        Err(e) => return error_item(&key, scope, &e.to_string()),
    };
    // The raw-file hash makes any edit show up, including keys not shown below.
    let mut fields = BTreeMap::from([
        ("scope".to_string(), scope.to_string()),
        (
            "file_sha256".to_string(),
            crate::hex(&Sha256::digest(&bytes)),
        ),
    ]);
    let dict = match Value::from_reader(std::io::Cursor::new(&bytes)) {
        Ok(Value::Dictionary(d)) => d,
        Ok(_) => {
            fields.insert("error".into(), "plist root is not a dictionary".into());
            return Item::new(CATEGORY, &key, fields);
        }
        Err(e) => {
            fields.insert("error".into(), e.to_string());
            return Item::new(CATEGORY, &key, fields);
        }
    };
    for name in [
        "Label",
        "Program",
        "ProgramArguments",
        "RunAtLoad",
        "KeepAlive",
        "StartInterval",
        "UserName",
    ] {
        if let Some(val) = dict.get(name) {
            fields.insert(name.to_string(), to_text(val));
        }
    }
    Item::new(CATEGORY, &key, fields)
}

fn error_item(key: &str, scope: &str, error: &str) -> Item {
    let fields = BTreeMap::from([
        ("scope".to_string(), scope.to_string()),
        ("error".to_string(), error.to_string()),
    ]);
    Item::new(CATEGORY, key, fields)
}

fn to_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Real(r) => r.to_string(),
        Value::Array(a) => a.iter().map(to_text).collect::<Vec<_>>().join(" "),
        Value::Dictionary(d) => d
            .iter()
            .map(|(k, v)| format!("{k}={}", to_text(v)))
            .collect::<Vec<_>>()
            .join(", "),
        other => format!("{other:?}"),
    }
}
