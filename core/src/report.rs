//! JSON for the app: one scan's changes (`scan --json`) and the timeline (`history --json`).
use crate::diff::{diff, Change, ChangeKind};
use crate::explain::{explain, field_label};
use crate::store::{Snapshot, Store};
use crate::Item;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// `{category, key, fields, team_id, field_labels}` plus the `explain` fields:
/// `title, summary, risk, risk_reason, added_by, confidence`.
pub fn item_json(item: &Item) -> Value {
    let e = explain(item);
    json!({
        "category": item.category,
        "key": item.key,
        "title": e.title,
        "summary": e.summary,
        "risk": e.risk,
        "risk_reason": e.risk_reason,
        "added_by": e.added_by,
        "confidence": e.confidence,
        "fields": item.fields,
        "field_labels": labels(item.fields.keys()),
        "team_id": item.team_id,
    })
}

/// Raw field name → plain label, for each name given.
fn labels<'a>(names: impl Iterator<Item = &'a String>) -> Value {
    let map: Map<String, Value> = names.map(|n| (n.clone(), json!(field_label(n)))).collect();
    Value::Object(map)
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
    if let Some(old) = old_fields {
        v["field_labels"] = labels(item.fields.keys().chain(old.keys()));
    }
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
/// scan), its item count, its item count per category (`categories`, sorted by
/// name) and error items, and up to `limit` `entries`, newest first, one per
/// scan that changed something compared with the scan before it.
pub fn history(store: &Store, limit: usize) -> rusqlite::Result<Value> {
    let snapshots = store.snapshots()?;
    let Some(latest) = snapshots.first() else {
        return Ok(
            json!({"latest": null, "items_total": 0, "categories": [], "errors": [], "entries": []}),
        );
    };
    let mut newer = store.items(latest.id)?;
    let items_total = newer.len();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for item in &newer {
        *counts.entry(&item.category).or_default() += 1;
    }
    let categories: Vec<Value> = counts
        .into_iter()
        .map(|(category, items)| json!({"category": category, "items": items}))
        .collect();
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
        "categories": categories,
        "errors": errors,
        "entries": entries,
    }))
}
