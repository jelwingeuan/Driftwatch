use super::{error_item, list_dir, output};
use crate::Item;
use plist::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub const CATEGORY: &str = "apps";

/// How long one app's signature check may take (Xcode takes about 15s).
const VERIFY_LIMIT: Duration = Duration::from_secs(60);

/// Signature and version of every app in /Applications and ~/Applications.
pub fn collect() -> Vec<Item> {
    let mut dirs = vec![PathBuf::from("/Applications")];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(Path::new(&home).join("Applications"));
    }
    let mut items = Vec::new();
    let mut apps = Vec::new();
    for dir in dirs {
        let paths = list_dir(CATEGORY, &dir, &mut items);
        apps.extend(
            paths
                .into_iter()
                .filter(|p| p.extension().is_some_and(|x| x == "app")),
        );
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

/// One app: its signature details, whether the signature checks out, and its version.
pub fn app_item(app: &Path) -> Item {
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
        let mut fields = parse_codesign(&text);
        fields.extend(verify(app));
        fields
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

/// `codesign --verify --strict` (no --deep): `signature_valid` "yes"; "no" with a
/// `signature_problem` when the check fails; "unknown" with the reason when the
/// check could not run or took too long, so that never reads as valid.
fn verify(app: &Path) -> BTreeMap<String, String> {
    let mut cmd = Command::new("codesign");
    cmd.args(["--verify", "--strict"]).arg(app);
    let (valid, problem) = match output(&mut cmd, VERIFY_LIMIT) {
        Ok(o) if o.status.success() => ("yes", None),
        // Exit 1 is codesign's "the signature doesn't check out"; anything else is
        // a failure to check.
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            let line = err.lines().next().unwrap_or("").trim();
            let prefix = format!("{}: ", app.display());
            let reason = line.strip_prefix(&prefix).unwrap_or(line).to_string();
            match o.status.code() {
                Some(1) => ("no", Some(reason)),
                _ => (
                    "unknown",
                    Some(format!("codesign failed ({}): {reason}", o.status)),
                ),
            }
        }
        Err(e) => ("unknown", Some(e)),
    };
    let mut fields = BTreeMap::from([("signature_valid".to_string(), valid.to_string())]);
    if let Some(p) = problem {
        fields.insert("signature_problem".into(), p);
    }
    fields
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
