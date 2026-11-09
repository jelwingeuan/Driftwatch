use super::{error_item, file_item, list_dir};
use crate::Item;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const CATEGORY: &str = "ssh";

/// ~/.ssh (authorized keys, config, public keys, presence of private keys) and sshd config.
pub fn collect() -> Vec<Item> {
    let mut items = match std::env::var("HOME") {
        Ok(home) => collect_ssh_dir(&Path::new(&home).join(".ssh")),
        Err(_) => Vec::new(),
    };
    items.extend(file_item(CATEGORY, Path::new("/etc/ssh/sshd_config")));
    let paths = list_dir(CATEGORY, Path::new("/etc/ssh/sshd_config.d"), &mut items);
    items.extend(paths.iter().filter_map(|p| file_item(CATEGORY, p)));
    items
}

/// authorized_keys is parsed per key; config and *.pub are hashed; every other
/// file (private keys) is recorded by name + mtime only, never read.
pub fn collect_ssh_dir(dir: &Path) -> Vec<Item> {
    let mut items = Vec::new();
    let paths = list_dir(CATEGORY, dir, &mut items);
    for path in paths {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let key = path.to_string_lossy();
        if name.starts_with("known_hosts") {
            continue; // grows on every new host; not a trust grant
        } else if name == "authorized_keys" || name == "authorized_keys2" {
            match std::fs::read_to_string(&path) {
                Ok(text) => items.extend(parse_authorized_keys(&text, &key)),
                Err(e) => items.push(error_item(CATEGORY, &key, &e.to_string())),
            }
        } else if name == "config" || name.ends_with(".pub") {
            items.extend(file_item(CATEGORY, &path));
        } else {
            items.push(presence_item(&path));
        }
    }
    items
}

fn presence_item(path: &PathBuf) -> Item {
    let key = path.to_string_lossy();
    let mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs());
    match mtime {
        Ok(secs) => Item::new(
            CATEGORY,
            &key,
            BTreeMap::from([("mtime".to_string(), secs.to_string())]),
        ),
        Err(e) => error_item(CATEGORY, &key, &e.to_string()),
    }
}

/// One Item per key line. Key = "<source>: <type> <sha256 of the base64 blob, 16 hex>".
pub fn parse_authorized_keys(text: &str, source: &str) -> Vec<Item> {
    let mut items = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let is_type =
            |t: &&str| t.starts_with("ssh-") || t.starts_with("ecdsa-") || t.starts_with("sk-");
        let Some(i) = tokens.iter().position(is_type) else {
            items.push(error_item(
                CATEGORY,
                &format!("{source}: {line}"),
                "no key type",
            ));
            continue;
        };
        let blob = tokens.get(i + 1).copied().unwrap_or("");
        let fp = crate::hex(&Sha256::digest(blob.as_bytes()));
        let mut fields = BTreeMap::from([("type".to_string(), tokens[i].to_string())]);
        if i > 0 {
            fields.insert("options".into(), tokens[..i].join(" "));
        }
        if tokens.len() > i + 2 {
            fields.insert("comment".into(), tokens[i + 2..].join(" "));
        }
        let key = format!("{source}: {} {}", tokens[i], &fp[..16]);
        items.push(Item::new(CATEGORY, &key, fields));
    }
    items
}
