use crate::Item;
use plist::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const CATEGORY: &str = "launchd";

/// The standard launchd directories (not /System) with their scope labels.
pub fn default_dirs() -> Vec<(PathBuf, &'static str)> {
    let home = std::env::var("HOME").unwrap_or_default();
    vec![
        (Path::new(&home).join("Library/LaunchAgents"), "user agent"),
        (PathBuf::from("/Library/LaunchAgents"), "global agent"),
        (PathBuf::from("/Library/LaunchDaemons"), "global daemon"),
    ]
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
                items.push(error_item(dir, scope, &e.to_string()));
                continue;
            }
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "plist"))
            .collect();
        paths.sort();
        for path in paths {
            items.push(match Value::from_file(&path) {
                Ok(v) => item_from_plist(&path, scope, &v),
                Err(e) => error_item(&path, scope, &e.to_string()),
            });
        }
    }
    items
}

fn item_from_plist(path: &Path, scope: &str, v: &Value) -> Item {
    let Some(dict) = v.as_dictionary() else {
        return error_item(path, scope, "plist root is not a dictionary");
    };
    let mut fields = BTreeMap::from([("scope".to_string(), scope.to_string())]);
    let program = dict.get("Program").or_else(|| dict.get("ProgramArguments"));
    if let Some(p) = program {
        fields.insert("Program".to_string(), to_text(p));
    }
    for key in [
        "Label",
        "RunAtLoad",
        "KeepAlive",
        "StartInterval",
        "UserName",
    ] {
        if let Some(val) = dict.get(key) {
            fields.insert(key.to_string(), to_text(val));
        }
    }
    Item::new(CATEGORY, &path.to_string_lossy(), fields)
}

fn error_item(path: &Path, scope: &str, error: &str) -> Item {
    let fields = BTreeMap::from([
        ("scope".to_string(), scope.to_string()),
        ("error".to_string(), error.to_string()),
    ]);
    Item::new(CATEGORY, &path.to_string_lossy(), fields)
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
