use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub mod collect;
pub mod diff;
pub mod report;
pub mod store;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub category: String,
    pub key: String,
    pub fields: BTreeMap<String, String>,
    pub team_id: Option<String>,
    pub hash: String,
}

impl Item {
    pub fn new(category: &str, key: &str, fields: BTreeMap<String, String>) -> Item {
        let hash = hash_of(category, &fields);
        Item {
            category: category.to_string(),
            key: key.to_string(),
            fields,
            team_id: None,
            hash,
        }
    }
}

/// Make (category, key) unique so diff and store never collapse items: the 2nd,
/// 3rd… repeat of a key (in input order) gets " #2", " #3"… appended.
pub fn uniquify_keys(items: &mut [Item]) {
    let mut seen: std::collections::HashMap<(String, String), usize> = Default::default();
    for item in items {
        let n = seen
            .entry((item.category.clone(), item.key.clone()))
            .or_default();
        *n += 1;
        if *n > 1 {
            item.key = format!("{} #{n}", item.key);
        }
    }
}

/// SHA-256 hex of the category and the (already sorted) fields, NUL-separated.
fn hash_of(category: &str, fields: &BTreeMap<String, String>) -> String {
    let mut h = Sha256::new();
    h.update(category.as_bytes());
    for (k, v) in fields {
        h.update([0]);
        h.update(k.as_bytes());
        h.update([0]);
        h.update(v.as_bytes());
    }
    hex(&h.finalize())
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
