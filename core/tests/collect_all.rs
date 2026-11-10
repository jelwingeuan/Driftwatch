use driftwatch::diff::{diff, Change, ChangeKind};
use driftwatch::{collect_all, Collector, Item};
use std::collections::BTreeMap;
use ChangeKind::{Added, Modified, Removed};

fn item(category: &str, key: &str, fields: &[(&str, &str)]) -> Item {
    let fields: BTreeMap<String, String> = fields
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Item::new(category, key, fields)
}

fn login_item(id: &str) -> Item {
    item("btm", &format!("uid 501: {id}"), &[("name", id)])
}

fn btm_error() -> Item {
    item(
        "btm",
        "sfltool dumpbtm",
        &[("error", "no records returned (needs admin rights)")],
    )
}

fn plist(name: &str) -> Item {
    item("launchd", &format!("/{name}.plist"), &[("Label", name)])
}

fn launchd_ok() -> Vec<Item> {
    vec![plist("a"), plist("b")]
}

fn launchd_a_unreadable() -> Vec<Item> {
    vec![item(
        "launchd",
        "/a.plist",
        &[("error", "Permission denied")],
    )]
}

fn btm_ok() -> Vec<Item> {
    ["a", "b", "c"].map(login_item).to_vec()
}

fn btm_one_more() -> Vec<Item> {
    ["a", "b", "c", "d"].map(login_item).to_vec()
}

fn btm_failed() -> Vec<Item> {
    vec![btm_error()]
}

fn btm_partly_read() -> Vec<Item> {
    vec![login_item("a"), btm_error()]
}

fn nothing() -> Vec<Item> {
    Vec::new()
}

fn not_read() -> Vec<Item> {
    panic!("a skipped category is read")
}

/// (kind, key) of each change.
fn kinds(changes: &[Change]) -> Vec<(ChangeKind, &str)> {
    changes
        .iter()
        .map(|c| {
            let item = c.new.as_ref().or(c.old.as_ref()).unwrap();
            (c.kind, item.key.as_str())
        })
        .collect()
}

#[test]
fn failed_read_shows_only_the_error_and_recovery_only_real_changes() {
    let ok: [Collector; 2] = [("launchd", launchd_ok), ("btm", btm_ok)];
    let failed: [Collector; 2] = [("launchd", launchd_ok), ("btm", btm_failed)];
    let recovered: [Collector; 2] = [("launchd", launchd_ok), ("btm", btm_one_more)];

    let baseline = collect_all(&ok, &[], &[]);
    let during = collect_all(&failed, &baseline, &[]);
    assert_eq!(
        kinds(&diff(&baseline, &during)),
        [(Added, "sfltool dumpbtm")]
    );
    // The kept items are the previous ones, hashes and all.
    assert!(baseline.iter().all(|i| during.contains(i)));

    let still = collect_all(&failed, &during, &[]);
    assert!(diff(&during, &still).is_empty());

    let after = collect_all(&recovered, &still, &[]);
    assert_eq!(
        kinds(&diff(&still, &after)),
        [(Removed, "sfltool dumpbtm"), (Added, "uid 501: d")]
    );
}

#[test]
fn real_removals_still_show() {
    let baseline = collect_all(&[("btm", btm_ok)], &[], &[]);
    // Read fine and nothing is there: everything is removed.
    let empty = collect_all(&[("btm", nothing)], &baseline, &[]);
    assert_eq!(
        kinds(&diff(&baseline, &empty)),
        [
            (Removed, "uid 501: a"),
            (Removed, "uid 501: b"),
            (Removed, "uid 501: c")
        ]
    );
    // Partly read: what was not read is removed.
    let partial = collect_all(&[("btm", btm_partly_read)], &baseline, &[]);
    assert_eq!(
        kinds(&diff(&baseline, &partial)),
        [
            (Added, "sfltool dumpbtm"),
            (Removed, "uid 501: b"),
            (Removed, "uid 501: c")
        ]
    );
}

#[test]
fn an_error_takes_the_place_of_the_kept_item_with_its_key() {
    let baseline = collect_all(&[("launchd", launchd_ok)], &[], &[]);
    let failed = collect_all(&[("launchd", launchd_a_unreadable)], &baseline, &[]);
    assert_eq!(failed.len(), 2);
    assert_eq!(kinds(&diff(&baseline, &failed)), [(Modified, "/a.plist")]);
}

#[test]
fn a_skipped_category_keeps_its_last_items_as_they_were() {
    let baseline = collect_all(
        &[("launchd", launchd_ok), ("btm", btm_partly_read)],
        &[],
        &[],
    );
    let skipped = collect_all(
        &[("launchd", launchd_a_unreadable), ("btm", not_read)],
        &baseline,
        &["btm"],
    );
    // Error item included; the other categories are still read.
    let btm = |items: &[Item]| -> Vec<Item> {
        items
            .iter()
            .filter(|i| i.category == "btm")
            .cloned()
            .collect()
    };
    assert_eq!(btm(&skipped), btm(&baseline));
    assert_eq!(kinds(&diff(&baseline, &skipped)), [(Modified, "/a.plist")]);
}

#[test]
fn a_skipped_category_with_no_last_items_is_read() {
    assert_eq!(collect_all(&[("btm", btm_ok)], &[], &["btm"]).len(), 3);
    let no_btm = collect_all(&[("launchd", launchd_ok)], &[], &[]);
    let first = collect_all(&[("btm", btm_ok)], &no_btm, &["btm"]);
    assert_eq!(first.len(), 3);
}
