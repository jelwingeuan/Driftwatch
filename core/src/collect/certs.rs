use super::{error_item, run};
use crate::Item;
use std::collections::BTreeMap;
use std::process::Command;

pub const CATEGORY: &str = "certs";

/// Certificates in the login and System keychains, plus admin and user trust
/// overrides (a user-added "always trust" is the risky signal).
pub fn collect() -> Vec<Item> {
    let mut items = Vec::new();
    let mut trust = Vec::new();
    for (args, domain) in [(&["-d"][..], "admin"), (&[][..], "user")] {
        let out = Command::new("security")
            .arg("dump-trust-settings")
            .args(args)
            .output();
        match out {
            Ok(o) if o.status.success() => {
                trust.extend(parse_trust(&String::from_utf8_lossy(&o.stdout), domain))
            }
            Ok(o) if String::from_utf8_lossy(&o.stderr).contains("No Trust Settings") => {}
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                items.push(error_item(CATEGORY, &format!("{domain} trust"), &err));
            }
            Err(e) => items.push(error_item(
                CATEGORY,
                &format!("{domain} trust"),
                &e.to_string(),
            )),
        }
    }
    let mut args = vec!["find-certificate", "-a", "-Z"];
    let login = std::env::var("HOME").map(|h| format!("{h}/Library/Keychains/login.keychain-db"));
    if let Ok(login) = &login {
        args.push(login);
    }
    args.push("/Library/Keychains/System.keychain");
    match run("security", &args) {
        Ok(out) => items.extend(parse_certs(&out, &trust)),
        Err(e) => items.push(error_item(CATEGORY, "security find-certificate", &e)),
    }
    items
}

/// `security dump-trust-settings` → (cert name, "<domain>: settings…") per cert.
pub fn parse_trust(text: &str, domain: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Cert ") {
            if let Some((_, name)) = rest.split_once(": ") {
                out.push((name.to_string(), Vec::new()));
            }
        } else if let Some((_, settings)) = out.last_mut() {
            if !t.is_empty() && !t.starts_with("Trust Setting ") {
                settings.push(t.split_whitespace().collect::<Vec<_>>().join(" "));
            }
        }
    }
    out.into_iter()
        .map(|(name, s)| (name, format!("{domain}: {}", s.join("; "))))
        .collect()
}

/// `security find-certificate -a -Z` → one Item per cert, key = SHA-1 hash.
/// Trust entries are attached by name; ones with no matching cert get their own Item.
pub fn parse_certs(text: &str, trust: &[(String, String)]) -> Vec<Item> {
    let mut certs: Vec<BTreeMap<String, String>> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(sha1) = t.strip_prefix("SHA-1 hash: ") {
            certs.push(BTreeMap::from([("sha1".into(), sha1.to_string())]));
        } else if let (Some(c), Some(kc)) = (certs.last_mut(), t.strip_prefix("keychain: ")) {
            c.insert("keychain".into(), kc.trim_matches('"').to_string());
        } else if let (Some(c), Some(l)) = (certs.last_mut(), t.strip_prefix("\"labl\"<blob>=")) {
            c.insert("label".into(), l.trim_matches('"').to_string());
        }
    }
    let mut used = vec![false; trust.len()];
    let mut items = Vec::new();
    for mut c in certs {
        let label = c.get("label").cloned().unwrap_or_default();
        let matched: Vec<&str> = trust
            .iter()
            .enumerate()
            .filter(|(_, (n, _))| *n == label)
            .map(|(i, (_, s))| {
                used[i] = true;
                s.as_str()
            })
            .collect();
        if !matched.is_empty() {
            c.insert("trust".into(), matched.join(" | "));
        }
        let key = c.remove("sha1").unwrap_or_default();
        items.push(Item::new(CATEGORY, &key, c));
    }
    for ((name, s), used) in trust.iter().zip(used) {
        if !used {
            let fields =
                BTreeMap::from([("label".into(), name.clone()), ("trust".into(), s.clone())]);
            items.push(Item::new(CATEGORY, &format!("trust: {name}"), fields));
        }
    }
    items
}
