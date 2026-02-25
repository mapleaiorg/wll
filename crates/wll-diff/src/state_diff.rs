//! State-level diff: compare two worldline state maps.
//!
//! States are represented as `BTreeMap<String, serde_json::Value>`. The diff
//! detects key additions, removals, and value modifications.

use std::collections::BTreeMap;

use serde_json::Value;

/// The result of comparing two state maps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateDiff {
    /// The list of state changes.
    pub changes: Vec<StateChange>,
}

impl StateDiff {
    /// Create an empty state diff.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if there are no changes.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Number of changes.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Number of added keys.
    pub fn additions(&self) -> usize {
        self.changes
            .iter()
            .filter(|c| matches!(c, StateChange::Added { .. }))
            .count()
    }

    /// Number of removed keys.
    pub fn removals(&self) -> usize {
        self.changes
            .iter()
            .filter(|c| matches!(c, StateChange::Removed { .. }))
            .count()
    }

    /// Number of modified keys.
    pub fn modifications(&self) -> usize {
        self.changes
            .iter()
            .filter(|c| matches!(c, StateChange::Modified { .. }))
            .count()
    }
}

/// A single change in a state map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateChange {
    /// A new key-value pair was added.
    Added { key: String, value: Value },
    /// An existing key-value pair was removed.
    Removed { key: String, value: Value },
    /// An existing key's value was modified.
    Modified {
        key: String,
        old: Value,
        new: Value,
    },
}

/// Compute the diff between two state maps.
///
/// Keys present only in `new` are `Added`, keys present only in `old` are
/// `Removed`, and keys present in both but with different values are `Modified`.
pub fn diff_states(
    old: &BTreeMap<String, Value>,
    new: &BTreeMap<String, Value>,
) -> StateDiff {
    let mut changes = Vec::new();

    // Check for removed and modified keys.
    for (key, old_val) in old {
        match new.get(key) {
            Some(new_val) => {
                if old_val != new_val {
                    changes.push(StateChange::Modified {
                        key: key.clone(),
                        old: old_val.clone(),
                        new: new_val.clone(),
                    });
                }
            }
            None => {
                changes.push(StateChange::Removed {
                    key: key.clone(),
                    value: old_val.clone(),
                });
            }
        }
    }

    // Check for added keys.
    for (key, new_val) in new {
        if !old.contains_key(key) {
            changes.push(StateChange::Added {
                key: key.clone(),
                value: new_val.clone(),
            });
        }
    }

    StateDiff { changes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_state(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn identical_states_no_diff() {
        let state = make_state(&[("a", json!(1)), ("b", json!("hello"))]);
        let diff = diff_states(&state, &state);
        assert!(diff.is_empty());
    }

    #[test]
    fn empty_to_populated() {
        let old = BTreeMap::new();
        let new = make_state(&[("x", json!(42)), ("y", json!("new"))]);

        let diff = diff_states(&old, &new);
        assert_eq!(diff.len(), 2);
        assert_eq!(diff.additions(), 2);
        assert_eq!(diff.removals(), 0);
    }

    #[test]
    fn populated_to_empty() {
        let old = make_state(&[("x", json!(42))]);
        let new = BTreeMap::new();

        let diff = diff_states(&old, &new);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff.removals(), 1);
    }

    #[test]
    fn single_key_modification() {
        let old = make_state(&[("count", json!(1))]);
        let new = make_state(&[("count", json!(2))]);

        let diff = diff_states(&old, &new);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff.modifications(), 1);

        match &diff.changes[0] {
            StateChange::Modified { key, old, new } => {
                assert_eq!(key, "count");
                assert_eq!(*old, json!(1));
                assert_eq!(*new, json!(2));
            }
            other => panic!("expected Modified, got {:?}", other),
        }
    }

    #[test]
    fn mixed_changes() {
        let old = make_state(&[
            ("keep", json!(true)),
            ("modify", json!("old")),
            ("remove", json!(42)),
        ]);
        let new = make_state(&[
            ("keep", json!(true)),
            ("modify", json!("new")),
            ("added", json!([1, 2, 3])),
        ]);

        let diff = diff_states(&old, &new);
        assert_eq!(diff.len(), 3); // modified, removed, added
        assert_eq!(diff.additions(), 1);
        assert_eq!(diff.removals(), 1);
        assert_eq!(diff.modifications(), 1);
    }

    #[test]
    fn nested_value_modification() {
        let old = make_state(&[("config", json!({"debug": false, "port": 8080}))]);
        let new = make_state(&[("config", json!({"debug": true, "port": 8080}))]);

        let diff = diff_states(&old, &new);
        assert_eq!(diff.modifications(), 1);
    }

    #[test]
    fn type_change_detected() {
        let old = make_state(&[("value", json!(42))]);
        let new = make_state(&[("value", json!("forty-two"))]);

        let diff = diff_states(&old, &new);
        assert_eq!(diff.modifications(), 1);
    }

    #[test]
    fn null_value_handling() {
        let old = make_state(&[("nullable", json!(null))]);
        let new = make_state(&[("nullable", json!("not null"))]);

        let diff = diff_states(&old, &new);
        assert_eq!(diff.modifications(), 1);
    }
}
