use crate::Item;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub mod btm;
pub mod cron;
pub mod launchd;
pub mod shell;
pub mod sysext;

/// Run a tool and return its stdout, or a one-line error naming what failed.
pub(crate) fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("{cmd}: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let last = err.lines().last().unwrap_or("").trim();
        return Err(format!("{cmd} failed ({}): {last}", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// An Item that records why something could not be read.
pub(crate) fn error_item(category: &str, key: &str, error: &str) -> Item {
    let fields = BTreeMap::from([("error".to_string(), error.to_string())]);
    Item::new(category, key, fields)
}

/// Hash a file's bytes so any edit shows up. `None` when the file does not exist;
/// an error Item when it exists but cannot be read.
pub(crate) fn file_item(category: &str, path: &Path) -> Option<Item> {
    let key = path.to_string_lossy();
    match std::fs::read(path) {
        Ok(bytes) => Some(Item::new(
            category,
            &key,
            BTreeMap::from([
                ("file_sha256".into(), crate::hex(&Sha256::digest(&bytes))),
                ("size".into(), bytes.len().to_string()),
            ]),
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => Some(error_item(category, &key, &e.to_string())),
    }
}

/// Sorted file paths in `dir` (hidden files included). Missing dir → empty;
/// unreadable dir → Err.
pub(crate) fn list_dir(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();
    Ok(paths)
}
