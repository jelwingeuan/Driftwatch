use super::{error_item, file_item, list_files, run};
use crate::Item;
use std::collections::BTreeMap;
use std::path::Path;

pub const CATEGORY: &str = "network";

/// /etc/hosts entries, /etc/resolver/* files and the DNS servers in use.
pub fn collect() -> Vec<Item> {
    let mut items = match std::fs::read_to_string("/etc/hosts") {
        Ok(text) => parse_hosts(&text),
        Err(e) => vec![error_item(CATEGORY, "/etc/hosts", &e.to_string())],
    };
    let paths = list_files(CATEGORY, Path::new("/etc/resolver"), &mut items);
    items.extend(paths.iter().filter_map(|p| file_item(CATEGORY, p)));
    match run("scutil", &["--dns"]) {
        Ok(out) => items.extend(parse_dns(&out)),
        Err(e) => items.push(error_item(CATEGORY, "scutil --dns", &e)),
    }
    items
}

/// One Item per non-comment hosts line, whitespace collapsed. Key = "hosts: <line>".
pub fn parse_hosts(text: &str) -> Vec<Item> {
    text.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let fields = BTreeMap::from([("source".to_string(), "/etc/hosts".to_string())]);
            Item::new(CATEGORY, &format!("hosts: {l}"), fields)
        })
        .collect()
}

/// Nameservers from `scutil --dns`. Key = "dns[ scoped] resolver #N: <nameserver>".
pub fn parse_dns(text: &str) -> Vec<Item> {
    let mut items = Vec::new();
    let mut section = "dns";
    let mut resolver = "";
    let mut domain = String::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("DNS configuration") {
            section = if t.contains("scoped") {
                "dns scoped"
            } else {
                "dns"
            };
        } else if t.starts_with("resolver #") {
            resolver = t;
            domain.clear();
        } else if let Some((k, v)) = t.split_once(':') {
            match k.trim() {
                "domain" => domain = v.trim().to_string(),
                k if k.starts_with("nameserver[") => {
                    let ns = v.trim();
                    let mut fields = BTreeMap::from([("nameserver".to_string(), ns.to_string())]);
                    if !domain.is_empty() {
                        fields.insert("domain".into(), domain.clone());
                    }
                    let key = format!("{section} {resolver}: {ns}");
                    items.push(Item::new(CATEGORY, &key, fields));
                }
                _ => {}
            }
        }
    }
    items
}
