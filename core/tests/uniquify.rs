use driftwatch::diff::{diff, ChangeKind};
use driftwatch::{uniquify_keys, Item};
use std::collections::BTreeMap;

fn item(category: &str, key: &str, v: &str) -> Item {
    Item::new(
        category,
        key,
        BTreeMap::from([("v".to_string(), v.to_string())]),
    )
}

#[test]
fn duplicate_keys_survive_into_diff() {
    let mut items = vec![
        item("network", "hosts: x", "1"),
        item("network", "hosts: x", "2"),
        item("cron", "hosts: x", "3"),
        item("network", "hosts: x", "4"),
    ];
    uniquify_keys(&mut items);
    let keys: Vec<&str> = items.iter().map(|i| i.key.as_str()).collect();
    assert_eq!(keys, ["hosts: x", "hosts: x #2", "hosts: x", "hosts: x #3"]);

    let changes = diff(&[], &items);
    assert_eq!(changes.len(), 4);
    assert!(changes.iter().all(|c| c.kind == ChangeKind::Added));
}
