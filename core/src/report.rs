//! JSON for the app: one scan's changes (`scan --json`) and the timeline (`history --json`).
use crate::diff::{diff, Change, ChangeKind};
use crate::store::{Snapshot, Store};
use crate::Item;
use serde_json::{json, Value};
use std::path::Path;

/// The best human name for an item: its Label, name, label, client or identifier,
/// else the key. A path is shortened to its file name.
pub fn title(item: &Item) -> String {
    let best = ["Label", "name", "label", "client", "identifier"]
        .iter()
        .find_map(|f| item.fields.get(*f).filter(|v| !v.is_empty()))
        .unwrap_or(&item.key);
    match Path::new(best).file_name() {
        Some(name) if best.starts_with('/') => name.to_string_lossy().into_owned(),
        _ => best.clone(),
    }
}

/// `{category, key, title, fields, team_id}`
pub fn item_json(item: &Item) -> Value {
    json!({
        "category": item.category,
        "key": item.key,
        "title": title(item),
        "fields": item.fields,
        "team_id": item.team_id,
    })
}

/// `item_json` of the new (or, when removed, the old) item, plus `kind` and
/// `old_fields` (the previous fields when modified, else null).
pub fn change_json(c: &Change) -> Value {
    let item = c
        .new
        .as_ref()
        .or(c.old.as_ref())
        .expect("change has an item");
    let (kind, old_fields) = match c.kind {
        ChangeKind::Added => ("added", None),
        ChangeKind::Removed => ("removed", None),
        ChangeKind::Modified => ("modified", c.old.as_ref().map(|o| &o.fields)),
    };
    let mut v = item_json(item);
    v["kind"] = json!(kind);
    v["old_fields"] = json!(old_fields);
    v
}

fn changes_json(changes: &[Change]) -> Vec<Value> {
    changes.iter().map(change_json).collect()
}

/// Output of `scan --json`.
pub fn scan_json(snapshot: &Snapshot, baseline: bool, changes: &[Change]) -> Value {
    json!({
        "snapshot_id": snapshot.id,
        "taken_at": snapshot.taken_at,
        "baseline": baseline,
        "changes": changes_json(changes),
    })
}

/// Output of `history --json`: the newest scan (`latest`, null before the first
/// scan), its item count and error items, and up to `limit` `entries`, newest
/// first, one per scan that changed something compared with the scan before it.
pub fn history(store: &Store, limit: usize) -> rusqlite::Result<Value> {
    let snapshots = store.snapshots()?;
    let Some(latest) = snapshots.first() else {
        return Ok(json!({"latest": null, "items_total": 0, "errors": [], "entries": []}));
    };
    let mut newer = store.items(latest.id)?;
    let items_total = newer.len();
    let errors: Vec<Value> = newer
        .iter()
        .filter(|i| i.fields.contains_key("error"))
        .map(item_json)
        .collect();
    let mut entries = Vec::new();
    for pair in snapshots.windows(2) {
        if entries.len() >= limit {
            break;
        }
        let older = store.items(pair[1].id)?;
        let changes = diff(&older, &newer);
        if !changes.is_empty() {
            entries.push(json!({
                "snapshot_id": pair[0].id,
                "taken_at": pair[0].taken_at,
                "changes": changes_json(&changes),
            }));
        }
        newer = older;
    }
    Ok(json!({
        "latest": {"snapshot_id": latest.id, "taken_at": latest.taken_at},
        "items_total": items_total,
        "errors": errors,
        "entries": entries,
    }))
}
