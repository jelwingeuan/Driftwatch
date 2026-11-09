use super::{error_item, list_dir};
use crate::Item;
use plist::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const CATEGORY: &str = "apps";

/// Signature and version of every app in /Applications and ~/Applications.
pub fn collect() -> Vec<Item> {
    let mut dirs = vec![PathBuf::from("/Applications")];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(Path::new(&home).join("Applications"));
    }
    let mut items = Vec::new();
    let mut apps = Vec::new();
    for dir in dirs {
        match list_dir(&dir) {
            Ok(paths) => apps.extend(
                paths
                    .into_iter()
                    .filter(|p| p.extension().is_some_and(|x| x == "app")),
            ),
            Err(e) => items.push(error_item(CATEGORY, &dir.to_string_lossy(), &e)),
        }
    }
    // codesign is slow per app, so run the chunks in parallel.
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = apps.len().div_ceil(threads).max(1);
    std::thread::scope(|s| {
        let handles: Vec<_> = apps
            .chunks(chunk)
            .map(|c| s.spawn(move || c.iter().map(|p| app_item(p)).collect::<Vec<_>>()))
            .collect();
        for h in handles {
            items.extend(h.join().expect("app scan thread panicked"));
        }
    });
    items
}

fn app_item(app: &Path) -> Item {
    let key = app.to_string_lossy();
    let out = match Command::new("codesign")
        .args(["-dv", "--verbose=4"])
        .arg(app)
        .output()
    {
        Ok(o) => o,
        Err(e) => return error_item(CATEGORY, &key, &format!("codesign: {e}")),
    };
    let text = String::from_utf8_lossy(&out.stderr);
    let mut fields = if out.status.success() {
        parse_codesign(&text)
    } else if text.contains("not signed at all") {
        BTreeMap::from([("signed".to_string(), "no".to_string())])
    } else {
        let err = text.lines().last().unwrap_or("").trim();
        BTreeMap::from([("error".to_string(), format!("codesign: {err}"))])
    };
    if let Ok(Value::Dictionary(d)) = Value::from_file(app.join("Contents/Info.plist")) {
        if let Some(v) = d
            .get("CFBundleShortVersionString")
            .and_then(Value::as_string)
        {
            fields.insert("version".into(), v.to_string());
        }
    }
    let mut item = Item::new(CATEGORY, &key, fields);
    item.team_id = item.fields.get("team_id").cloned();
    item
}

/// Parse `codesign -dv --verbose=4` (its stderr): identifier, team ID,
/// first authority line, CDHash, and "adhoc" when ad hoc signed.
pub fn parse_codesign(text: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let name = match k {
            "Identifier" => "identifier",
            "TeamIdentifier" if v != "not set" => "team_id",
            "Authority" if !fields.contains_key("authority") => "authority",
            "CDHash" => "cdhash",
            "Signature" => "signature",
            _ => continue,
        };
        fields.insert(name.to_string(), v.to_string());
    }
    fields
}
