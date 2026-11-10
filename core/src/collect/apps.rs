use super::{error_item, list_dir, output, read_file, TOOL_LIMIT};
use crate::Item;
use plist::Value;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const CATEGORY: &str = "apps";

/// How long one app's signature check may take (Xcode takes about 15s).
const VERIFY_LIMIT: Duration = Duration::from_secs(60);

/// How long an unchanged app's last signature check may be reused (in seconds):
/// every app is fully checked again at least once a day.
pub const REUSE_LIMIT: u64 = 24 * 60 * 60;

/// The fields `verify` sets: the verdict a reused check carries over.
const VERDICT: [&str; 3] = ["signature_valid", "signature_problem", "signature_note"];

/// When an app's signature was last fully checked, and its `check_key` just before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureCheck {
    /// Seconds since 1970 (UTC).
    pub checked_at: u64,
    pub key: String,
}

/// Signature and version of every app in /Applications and ~/Applications.
/// `previous` (the last snapshot) lets unchanged apps skip the slow check.
pub fn collect(previous: &[Item]) -> Vec<Item> {
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
    let before: BTreeMap<&str, &Item> = previous
        .iter()
        .filter(|i| i.category == CATEGORY)
        .map(|i| (i.key.as_str(), i))
        .collect();
    let before = &before;
    // codesign is slow per app, so run the chunks in parallel.
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = apps.len().div_ceil(threads).max(1);
    std::thread::scope(|s| {
        let handles: Vec<_> = apps
            .chunks(chunk)
            .map(|c| {
                s.spawn(move || {
                    c.iter()
                        .map(|p| app_item(p, before.get(&*p.to_string_lossy()).copied()))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in handles {
            items.extend(h.join().expect("app scan thread panicked"));
        }
    });
    items
}

/// One app: its signature details, whether the signature checks out, and its
/// version. The verdict of `previous` (this app in the last snapshot) is reused
/// when `reusable`, else the signature is checked again.
pub fn app_item(app: &Path, previous: Option<&Item>) -> Item {
    let key = app.to_string_lossy();
    let out = match output(
        Command::new("codesign")
            .args(["-dv", "--verbose=4"])
            .arg(app),
        TOOL_LIMIT,
    ) {
        Ok(o) => o,
        Err(e) => return error_item(CATEGORY, &key, &e),
    };
    let text = String::from_utf8_lossy(&out.stderr);
    let mut check = None;
    let mut fields = if out.status.success() {
        let mut fields = parse_codesign(&text);
        // Read before verifying, so a change made during the check gives a new key.
        let now = now();
        let check_key = check_key(app, &text, &fields);
        match previous.filter(|p| reusable(p, check_key.as_deref(), now)) {
            Some(p) => {
                for name in VERDICT {
                    if let Some(v) = p.fields.get(name) {
                        fields.insert(name.to_string(), v.clone());
                    }
                }
                check = p.signature_check.clone();
            }
            None => {
                let verdict = verify(app);
                if matches!(verdict["signature_valid"].as_str(), "yes" | "no") {
                    check = check_key.map(|key| SignatureCheck {
                        checked_at: now,
                        key,
                    });
                }
                fields.extend(verdict);
            }
        }
        fields
    } else if text.contains("not signed at all") {
        BTreeMap::from([("signed".to_string(), "no".to_string())])
    } else {
        let err = text.lines().last().unwrap_or("").trim();
        BTreeMap::from([("error".to_string(), format!("codesign: {err}"))])
    };
    let info = read_file(&app.join("Contents/Info.plist"));
    if let Some(Value::Dictionary(d)) = info
        .ok()
        .and_then(|b| Value::from_reader(std::io::Cursor::new(b)).ok())
    {
        if let Some(v) = d
            .get("CFBundleShortVersionString")
            .and_then(Value::as_string)
        {
            fields.insert("version".into(), v.to_string());
        }
    }
    let mut item = Item::new(CATEGORY, &key, fields);
    item.team_id = item.fields.get("team_id").cloned();
    item.signature_check = check;
    item
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Whether `previous`'s verdict still holds: a "yes" or "no" (never "unknown")
/// from a check less than `REUSE_LIMIT` old, made when the app had this `key`.
fn reusable(previous: &Item, key: Option<&str>, now: u64) -> bool {
    let Some(check) = &previous.signature_check else {
        return false;
    };
    let valid = previous.fields.get("signature_valid").map(String::as_str);
    matches!(valid, Some("yes" | "no"))
        && key == Some(check.key.as_str())
        && now
            .checked_sub(check.checked_at)
            .is_some_and(|age| age < REUSE_LIMIT)
}

/// What must stay the same for a past signature check to still hold, hashed: the
/// signature details from `codesign -dv` and the device, inode, size, modify and
/// change times of the main executable, Info.plist and the resource seal
/// (`touch` can set a modify time back, not a change time). None when codesign
/// names no executable or one of those files can't be read: then always check.
// shortcut: an edit to a nested file (a framework, helper or resource) that
// leaves these three files untouched is only caught at the next full check,
// within REUSE_LIMIT; hash every sealed file if that gap ever matters.
fn check_key(app: &Path, codesign: &str, signature: &BTreeMap<String, String>) -> Option<String> {
    let exe = codesign
        .lines()
        .find_map(|l| l.strip_prefix("Executable="))?;
    let mut key: String = signature
        .iter()
        .map(|(k, v)| format!("{k}={v}\n"))
        .collect();
    for file in [
        PathBuf::from(exe),
        app.join("Contents/Info.plist"),
        app.join("Contents/_CodeSignature/CodeResources"),
    ] {
        let m = std::fs::metadata(&file).ok()?;
        key += &format!(
            "{} {} {} {} {}.{} {}.{}\n",
            file.display(),
            m.dev(),
            m.ino(),
            m.size(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec()
        );
    }
    Some(crate::hex(&Sha256::digest(key)))
}

/// What `codesign --strict` says about extended attributes (Finder info, resource
/// forks) on a bundle's files: metadata macOS ignores when it checks an app it runs.
const DETRITUS: &str = "resource fork, Finder information, or similar detritus not allowed";

/// Whether `codesign --verify --strict`'s stderr names detritus as its one problem.
pub fn only_detritus(stderr: &str) -> bool {
    let mut lines = stderr.lines().map(str::trim).filter(|l| !l.is_empty());
    let first = lines.next().unwrap_or("");
    first.ends_with(&format!(": {DETRITUS}")) && lines.next().is_none()
}

/// `codesign --verify --strict` (no --deep): `signature_valid` "yes"; "no" with a
/// `signature_problem` when the check fails; "unknown" with the reason when the
/// check could not run or took too long, so that never reads as valid. When the
/// only complaint is detritus, "yes" with a `signature_note` if every other
/// strict check passes.
fn verify(app: &Path) -> BTreeMap<String, String> {
    let check = |strict| {
        output(
            Command::new("codesign").args(["--verify", strict]).arg(app),
            VERIFY_LIMIT,
        )
    };
    let mut out = check("--strict");
    let mut note = None;
    let detritus = out.as_ref().is_ok_and(|o| {
        o.status.code() == Some(1) && only_detritus(&String::from_utf8_lossy(&o.stderr))
    });
    if detritus {
        // codesign stops at its first complaint, so detritus can hide a real
        // problem: check again with every strict check but the detritus one
        // (plain --verify would also drop the symlink check).
        out = check("--strict=symlinks");
        if out.as_ref().is_ok_and(|o| o.status.success()) {
            note = Some(format!(
                "Has extra file metadata macOS ignores (strict check: {DETRITUS})"
            ));
        }
    }
    let (valid, problem) = match out {
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
    if let Some(n) = note {
        fields.insert("signature_note".into(), n);
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
