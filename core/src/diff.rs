use crate::Item;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    pub old: Option<Item>,
    pub new: Option<Item>,
}

/// Compare two snapshots by (category, key). Output is sorted by (category, key).
pub fn diff(old: &[Item], new: &[Item]) -> Vec<Change> {
    let index = |items: &[Item]| -> BTreeMap<(String, String), Item> {
        items
            .iter()
            .map(|i| ((i.category.clone(), i.key.clone()), i.clone()))
            .collect()
    };
    let mut old = index(old);
    let mut changes = Vec::new();
    for (id, n) in index(new) {
        match old.remove(&id) {
            None => changes.push((id, ChangeKind::Added, None, Some(n))),
            Some(o) if o.hash != n.hash => {
                changes.push((id, ChangeKind::Modified, Some(o), Some(n)))
            }
            Some(_) => {}
        }
    }
    for (id, o) in old {
        changes.push((id, ChangeKind::Removed, Some(o), None));
    }
    changes.sort_by(|a, b| a.0.cmp(&b.0));
    changes
        .into_iter()
        .map(|(_, kind, old, new)| Change { kind, old, new })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: &str, label: &str) -> Item {
        let fields = BTreeMap::from([("Label".to_string(), label.to_string())]);
        Item::new("launchd", key, fields)
    }

    #[test]
    fn added() {
        let c = diff(&[], &[item("a", "x")]);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].kind, ChangeKind::Added);
        assert!(c[0].old.is_none());
        assert_eq!(c[0].new.as_ref().unwrap().key, "a");
    }

    #[test]
    fn removed() {
        let c = diff(&[item("a", "x")], &[]);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].kind, ChangeKind::Removed);
        assert!(c[0].new.is_none());
    }

    #[test]
    fn modified() {
        let c = diff(&[item("a", "x")], &[item("a", "y")]);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].kind, ChangeKind::Modified);
        assert_eq!(c[0].old.as_ref().unwrap().fields["Label"], "x");
        assert_eq!(c[0].new.as_ref().unwrap().fields["Label"], "y");
    }

    #[test]
    fn unchanged() {
        assert!(diff(&[item("a", "x")], &[item("a", "x")]).is_empty());
    }

    #[test]
    fn sorted_by_key() {
        let c = diff(
            &[item("b", "x"), item("c", "x")],
            &[item("c", "y"), item("a", "x")],
        );
        let keys: Vec<_> = c
            .iter()
            .map(|c| c.new.as_ref().or(c.old.as_ref()).unwrap().key.as_str())
            .collect();
        assert_eq!(keys, ["a", "b", "c"]);
    }
}
