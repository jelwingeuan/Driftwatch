use crate::Item;
use std::collections::BTreeMap;
use std::process::Command;

pub mod btm;
pub mod launchd;

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
