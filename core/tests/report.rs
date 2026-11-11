use driftwatch::diff::diff;
use driftwatch::report::{change_json, history, item_json, scan_json};
use driftwatch::store::Store;
use driftwatch::Item;
use serde_json::Value;
use std::collections::BTreeMap;

fn item(category: &str, key: &str, fields: &[(&str, &str)]) -> Item {
    let fields: BTreeMap<String, String> = fields
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Item::new(category, key, fields)
}

fn keys(v: &Value) -> Vec<&str> {
    v.as_object().unwrap().keys().map(String::as_str).collect()
}

fn is_rfc3339_utc(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 20 && b[4] == b'-' && b[10] == b'T' && b[13] == b':' && b[19] == b'Z'
}

#[test]
fn item_json_explains_the_item() {
    let plist = "/Library/LaunchAgents/com.example.agent.plist";
    let v = item_json(&item(
        "launchd",
        plist,
        &[
            ("Label", "com.example"),
            ("RunAtLoad", "true"),
            ("odd", "x"),
        ],
    ));
    assert_eq!(v["title"], "com.example");
    assert_eq!(v["summary"], "Starts a program every time you log in.");
    assert_eq!(v["risk"], "medium");
    assert!(v["risk_reason"].is_string());
    assert!(v["added_by"].is_null());
    assert!(v["confidence"].is_null());
    assert_eq!(v["field_labels"]["Label"], "Name");
    assert_eq!(v["field_labels"]["RunAtLoad"], "Starts at login");
    assert_eq!(v["field_labels"]["odd"], "odd");
    let broken = item_json(&item("launchd", plist, &[("error", "bad")]));
    assert_eq!(broken["title"], "Couldn’t read startup items");
    assert_eq!(broken["summary"], "Bad.");
    assert_eq!(broken["fields"]["error"], "bad");
    assert_eq!(broken["risk"], "low");
}

#[test]
fn change_json_shape() {
    let old = item(
        "launchd",
        "/a.plist",
        &[("Label", "a"), ("Program", "/old")],
    );
    let new = item(
        "launchd",
        "/a.plist",
        &[("Label", "a"), ("Program", "/new")],
    );
    let modified = change_json(&diff(std::slice::from_ref(&old), &[new])[0]);
    assert_eq!(
        keys(&modified),
        [
            "added_by",
            "category",
            "confidence",
            "field_labels",
            "fields",
            "key",
            "kind",
            "old_fields",
            "risk",
            "risk_reason",
            "summary",
            "team_id",
            "title"
        ]
    );
    assert_eq!(modified["kind"], "modified");
    assert_eq!(modified["title"], "a");
    assert_eq!(modified["fields"]["Program"], "/new");
    assert_eq!(modified["old_fields"]["Program"], "/old");
    assert_eq!(modified["field_labels"]["Program"], "Program");
    assert!(modified["team_id"].is_null());

    let removed = change_json(&diff(&[old], &[])[0]);
    assert_eq!(removed["kind"], "removed");
    assert_eq!(removed["fields"]["Program"], "/old");
    assert!(removed["old_fields"].is_null());
}

#[test]
fn scan_json_shape() {
    let mut store = Store::open_in_memory().unwrap();
    let snapshot = store.save_snapshot(&[]).unwrap();
    let added = diff(&[], &[item("cron", "job", &[])]);
    let v = scan_json(&snapshot, false, &added);
    assert_eq!(keys(&v), ["baseline", "changes", "snapshot_id", "taken_at"]);
    assert_eq!(v["snapshot_id"], snapshot.id);
    assert!(is_rfc3339_utc(v["taken_at"].as_str().unwrap()));
    assert_eq!(v["baseline"], false);
    assert_eq!(v["changes"][0]["kind"], "added");
}

#[test]
fn history_of_empty_store() {
    let store = Store::open_in_memory().unwrap();
    let v = history(&store, 10, None).unwrap();
    assert!(v["latest"].is_null());
    assert_eq!(v["items_total"], 0);
    assert_eq!(v["categories"].as_array().unwrap().len(), 0);
    assert_eq!(v["entries"].as_array().unwrap().len(), 0);
    assert_eq!(v["history_intact"], true);
}

#[test]
fn history_counts_items_per_category() {
    let mut store = Store::open_in_memory().unwrap();
    store
        .save_snapshot(&[
            item("launchd", "/a.plist", &[]),
            item("launchd", "/b.plist", &[]),
            item("cron", "job", &[]),
        ])
        .unwrap();
    let v = history(&store, 10, None).unwrap();
    assert_eq!(
        v["categories"],
        serde_json::json!([
            {"category": "cron", "items": 1},
            {"category": "launchd", "items": 2},
        ])
    );
}

#[test]
fn history_is_newest_first_and_skips_scans_without_changes() {
    let a1 = item("launchd", "/a.plist", &[("Label", "a"), ("v", "1")]);
    let a2 = item("launchd", "/a.plist", &[("Label", "a"), ("v", "2")]);
    let b = item("cron", "job", &[]);
    let err = item("tcc", "/TCC.db", &[("error", "needs Full Disk Access")]);
    let mut store = Store::open_in_memory().unwrap();
    store.save_snapshot(std::slice::from_ref(&a1)).unwrap(); // baseline
    store.save_snapshot(std::slice::from_ref(&a1)).unwrap(); // no change
    let s3 = store.save_snapshot(&[a1, b.clone()]).unwrap(); // b added
    let s4 = store.save_snapshot(&[a2, b, err]).unwrap(); // a modified, error added

    let v = history(&store, 10, None).unwrap();
    assert_eq!(
        keys(&v),
        [
            "categories",
            "entries",
            "errors",
            "history_intact",
            "items_total",
            "latest"
        ]
    );
    assert_eq!(v["history_intact"], true);
    assert_eq!(v["latest"]["snapshot_id"], s4.id);
    assert_eq!(v["latest"]["taken_at"], s4.taken_at.as_str());
    assert_eq!(v["items_total"], 3);
    assert_eq!(v["errors"].as_array().unwrap().len(), 1);
    assert_eq!(v["errors"][0]["category"], "tcc");
    // The latest scan's items, the unreadable one included.
    assert_eq!(v["categories"][2]["category"], "tcc");
    assert_eq!(v["categories"][2]["items"], 1);

    let entries = v["entries"].as_array().unwrap();
    let ids: Vec<i64> = entries
        .iter()
        .map(|e| e["snapshot_id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, [s4.id, s3.id]);
    assert_eq!(keys(&entries[0]), ["changes", "snapshot_id", "taken_at"]);
    assert!(is_rfc3339_utc(entries[0]["taken_at"].as_str().unwrap()));
    let kinds: Vec<&str> = entries[0]["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["modified", "added"]); // sorted by (category, key): launchd < tcc
    assert_eq!(entries[1]["changes"][0]["kind"], "added");
    assert_eq!(entries[1]["changes"][0]["key"], "job");

    let limited = history(&store, 1, None).unwrap();
    assert_eq!(limited["entries"].as_array().unwrap().len(), 1);
    assert_eq!(limited["entries"][0]["snapshot_id"], s4.id);
}

#[test]
fn history_since_goes_past_the_limit() {
    let mut store = Store::open_in_memory().unwrap();
    store.save_snapshot(&[]).unwrap(); // baseline
    let s2 = store.save_snapshot(&[item("cron", "a", &[])]).unwrap();
    store.save_snapshot(&[item("cron", "b", &[])]).unwrap();
    let count = |since: Option<&str>| {
        history(&store, 1, since).unwrap()["entries"]
            .as_array()
            .unwrap()
            .len()
    };
    // The limit alone would drop s2's change; everything since s2 is kept.
    assert_eq!(count(None), 1);
    assert_eq!(count(Some(&s2.taken_at)), 2);
    // A since after every scan leaves the limit as it was.
    assert_eq!(count(Some("9999-12-31T23:59:59Z")), 1);
}

#[test]
fn history_says_when_it_was_edited_outside_driftwatch() {
    let dir = std::env::temp_dir().join(format!("driftwatch-edited-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("driftwatch.db");
    let mut store = Store::open(&path).unwrap();
    store.save_snapshot(&[item("cron", "job", &[])]).unwrap();
    store.save_snapshot(&[]).unwrap();
    assert_eq!(history(&store, 10, None).unwrap()["history_intact"], true);
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute("DELETE FROM items", [])
        .unwrap();
    assert_eq!(history(&store, 10, None).unwrap()["history_intact"], false);
    std::fs::remove_dir_all(&dir).unwrap();
}
