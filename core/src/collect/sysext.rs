use super::{error_item, run};
use crate::Item;
use std::collections::BTreeMap;

pub const CATEGORY: &str = "sysext";

/// System extensions from `systemextensionsctl list`.
pub fn collect() -> Vec<Item> {
    match run("systemextensionsctl", &["list"]) {
        Ok(out) => parse(&out),
        Err(e) => vec![error_item(CATEGORY, "systemextensionsctl list", &e)],
    }
}

/// Parse the tab-separated rows: enabled, active, teamID, "bundleID (version)", name, [state].
/// Key = "<bundle id> <team id>".
pub fn parse(text: &str) -> Vec<Item> {
    let mut items = Vec::new();
    let mut kind = "";
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("--- ") {
            kind = rest.split(' ').next().unwrap_or("");
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        let [enabled, active, team, id_ver, name, state, ..] = cols[..] else {
            continue;
        };
        let Some((bundle, version)) = id_ver.split_once(" (") else {
            continue;
        };
        if team == "teamID" {
            continue; // header row
        }
        let fields = BTreeMap::from([
            ("type".to_string(), kind.to_string()),
            ("enabled".to_string(), (enabled == "*").to_string()),
            ("active".to_string(), (active == "*").to_string()),
            ("team_id".to_string(), team.to_string()),
            (
                "version".to_string(),
                version.trim_end_matches(')').to_string(),
            ),
            ("name".to_string(), name.to_string()),
            (
                "state".to_string(),
                state.trim_matches(['[', ']']).to_string(),
            ),
        ]);
        let mut item = Item::new(CATEGORY, &format!("{bundle} {team}"), fields);
        item.team_id = Some(team.to_string());
        items.push(item);
    }
    items
}
