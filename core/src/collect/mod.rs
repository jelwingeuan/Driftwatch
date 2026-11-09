use crate::Item;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub mod apps;
pub mod btm;
pub mod certs;
pub mod cron;
pub mod launchd;
pub mod network;
pub mod shell;
pub mod ssh;
pub mod sysext;
pub mod tcc;

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

/// Sorted paths in `dir` (hidden files included). Missing dir → empty. An
/// unreadable dir or entry is pushed to `errors` as an error Item, never dropped.
pub(crate) fn list_dir(category: &str, dir: &Path, errors: &mut Vec<Item>) -> Vec<PathBuf> {
    let key = dir.to_string_lossy();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            errors.push(error_item(category, &key, &e.to_string()));
            return Vec::new();
        }
    };
    let mut paths = Vec::new();
    for (i, entry) in entries.enumerate() {
        match entry {
            Ok(e) => paths.push(e.path()),
            Err(e) => errors.push(error_item(
                category,
                &format!("{key}#entry{i}"),
                &e.to_string(),
            )),
        }
    }
    paths.sort();
    paths
}

/// Like `list_dir`, minus subdirectories (symlinks are followed, so links to files stay).
pub(crate) fn list_files(category: &str, dir: &Path, errors: &mut Vec<Item>) -> Vec<PathBuf> {
    let mut paths = list_dir(category, dir, errors);
    paths.retain(|p| !p.is_dir());
    paths
}
