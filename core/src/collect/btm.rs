use super::{error_item, run};
use crate::Item;
use std::collections::BTreeMap;

pub const CATEGORY: &str = "btm";

/// Background Task Management items (login items, background helpers) from `sfltool dumpbtm`.
pub fn collect() -> Vec<Item> {
    match run("sfltool", &["dumpbtm"]) {
        Ok(out) if out.contains("Records for UID") => parse(&out),
        Ok(_) => vec![error_item(
            CATEGORY,
            "sfltool dumpbtm",
            "no records returned (needs admin rights)",
        )],
        Err(e) => vec![error_item(
            CATEGORY,
            "sfltool dumpbtm",
            &format!("{e} (needs admin rights)"),
        )],
    }
}

/// Parse `sfltool dumpbtm` text. Key = "uid <uid>: <Identifier>". Volatile lines
/// (UUID, Generation, Last Use) are left out so they do not show as changes.
pub fn parse(text: &str) -> Vec<Item> {
    let mut items = Vec::new();
    let mut uid = String::new();
    let mut cur: Option<BTreeMap<String, String>> = None;
    let mut flush = |cur: &mut Option<BTreeMap<String, String>>, uid: &str| {
        if let Some(mut f) = cur.take() {
            let id = f
                .remove("identifier")
                .or_else(|| f.get("executable").or(f.get("url")).cloned())
                .unwrap_or_default();
            f.insert("uid".into(), uid.to_string());
            let mut item = Item::new(CATEGORY, &format!("uid {uid}: {id}"), f);
            item.team_id = item.fields.get("team_id").cloned();
            items.push(item);
        }
    };
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Records for UID ") {
            flush(&mut cur, &uid);
            uid = rest.split(' ').next().unwrap_or("").to_string();
            continue;
        }
        // Items start at " #N:" with one leading space; embedded lists are indented more.
        if line.starts_with(" #") && t.ends_with(':') {
            flush(&mut cur, &uid);
            cur = Some(BTreeMap::new());
            continue;
        }
        let (Some(f), Some((k, v))) = (cur.as_mut(), t.split_once(": ")) else {
            continue;
        };
        let name = match k {
            "Name" => "name",
            "Developer Name" => "developer",
            "Team Identifier" => "team_id",
            "Type" => "type",
            "Disposition" => "disposition",
            "Identifier" => "identifier",
            "URL" => "url",
            "Executable Path" => "executable",
            "Bundle Identifier" => "bundle_id",
            "Parent Identifier" => "parent",
            _ => continue,
        };
        if v != "(null)" {
            f.insert(name.into(), v.to_string());
        }
    }
    flush(&mut cur, &uid);
    items
}
