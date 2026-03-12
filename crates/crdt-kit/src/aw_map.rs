use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use crate::{Crdt, DeltaCrdt, NodeId};

/// An add-wins map (AW-Map).
///
/// A key-value map where each key is tracked with OR-Set semantics: concurrent
/// add and remove of the same key resolves in favor of add. Values are updated
/// using a causal version vector per key.
///
/// # Example
///
/// ```
/// use crdt_kit::prelude::*;
///
/// let mut m1 = AWMap::new(1);
/// m1.insert("color", "red");
///
/// let mut m2 = AWMap::new(2);
/// m2.insert("color", "blue");
///
/// m1.merge(&m2);
/// // Both adds are concurrent — the latest by tag ordering wins
/// assert!(m1.contains_key(&"color"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AWMap<K: Ord + Clone, V: Clone + Eq> {
    actor: NodeId,
    counter: u64,
    /// key -> (value, set of unique tags)
    entries: BTreeMap<K, (V, BTreeSet<(NodeId, u64)>)>,
    /// Tombstones: tags that have been removed
    tombstones: BTreeSet<(NodeId, u64)>,
}

impl<K: Ord + Clone, V: Clone + Eq> AWMap<K, V> {
    /// Create a new empty AW-Map for the given node.
    pub fn new(actor: NodeId) -> Self {
        Self {
            actor,
            counter: 0,
            entries: BTreeMap::new(),
            tombstones: BTreeSet::new(),
        }
    }

    /// Insert or update a key-value pair.
    ///
    /// Generates a unique tag for this write. If the key already exists,
    /// the old tags are kept (they accumulate until a remove clears them).
    /// The value is updated to the new value.
    pub fn insert(&mut self, key: K, value: V) {
        self.counter += 1;
        let tag = (self.actor, self.counter);
        let entry = self.entries.entry(key).or_insert_with(|| {
            (value.clone(), BTreeSet::new())
        });
        entry.0 = value;
        entry.1.insert(tag);
    }

    /// Remove a key from the map.
    ///
    /// Only removes the tags that this replica has observed. Concurrent
    /// inserts on other replicas will survive the merge (add wins).
    ///
    /// Returns `true` if the key was present and removed.
    pub fn remove(&mut self, key: &K) -> bool {
        if let Some((_, tags)) = self.entries.remove(key) {
            self.tombstones.extend(tags);
            true
        } else {
            false
        }
    }

    /// Get the value associated with a key, if present.
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key).map(|(v, _)| v)
    }

    /// Check if a key is present in the map.
    #[must_use]
    pub fn contains_key(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    /// Get the number of keys in the map.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the map is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(k, (v, _))| (k, v))
    }

    /// Get all keys.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.keys()
    }

    /// Get all values.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.values().map(|(v, _)| v)
    }

    /// Get this replica's node ID.
    #[must_use]
    pub fn actor(&self) -> NodeId {
        self.actor
    }

    /// Returns the number of tombstones stored.
    #[must_use]
    pub fn tombstone_count(&self) -> usize {
        self.tombstones.len()
    }

    /// Remove tombstones that are not referenced by any active entry's tag set.
    ///
    /// **Caution:** While this only removes "dangling" tombstones (tags not
    /// in any live entry), it can cause tag resurrection in partitioned
    /// networks with 3+ replicas where a stale remove arrives after
    /// compaction. Only call when you have reasonable confidence that all
    /// in-flight remove operations have been delivered.
    pub fn compact_tombstones(&mut self) {
        let active_tags: BTreeSet<(NodeId, u64)> = self
            .entries
            .values()
            .flat_map(|(_, tags)| tags.iter().copied())
            .collect();
        self.tombstones.retain(|t| active_tags.contains(t));
    }

    /// Remove **all** tombstones unconditionally.
    ///
    /// # Safety (logical)
    ///
    /// Only call this after all replicas have fully converged. If called
    /// while replicas are still divergent, a stale remove may fail to
    /// propagate on the next merge.
    pub fn compact_tombstones_all(&mut self) {
        self.tombstones.clear();
    }
}

impl<K: Ord + Clone, V: Clone + Eq> IntoIterator for AWMap<K, V> {
    type Item = (K, V);
    type IntoIter = alloc::vec::IntoIter<(K, V)>;

    fn into_iter(self) -> Self::IntoIter {
        let items: Vec<(K, V)> = self
            .entries
            .into_iter()
            .map(|(k, (v, _))| (k, v))
            .collect();
        items.into_iter()
    }
}

impl<K: Ord + Clone, V: Clone + Eq> Crdt for AWMap<K, V> {
    fn merge(&mut self, other: &Self) {
        // Add entries from other that we don't have tombstoned
        for (key, (other_value, other_tags)) in &other.entries {
            let entry = self.entries.entry(key.clone()).or_insert_with(|| {
                (other_value.clone(), BTreeSet::new())
            });
            for &tag in other_tags {
                if !self.tombstones.contains(&tag) {
                    entry.1.insert(tag);
                }
            }
            // Use the value from the highest tag for determinism
            if let Some(&max_tag) = entry.1.iter().next_back() {
                if other_tags.contains(&max_tag) {
                    entry.0 = other_value.clone();
                }
            }
        }

        // Apply other's tombstones
        for &tag in &other.tombstones {
            for (_, (_, tags)) in self.entries.iter_mut() {
                tags.remove(&tag);
            }
        }
        self.tombstones.extend(&other.tombstones);

        // Remove entries with no live tags
        self.entries.retain(|_, (_, tags)| !tags.is_empty());

        // Resolve value for each remaining key: value comes from the max tag owner
        // We need to reconcile values when both sides contributed tags
        // Use the value from the side that has the globally highest tag
        for (key, (value, tags)) in self.entries.iter_mut() {
            if let Some(&max_tag) = tags.iter().next_back() {
                if let Some((other_value, other_tags)) = other.entries.get(key) {
                    if other_tags.contains(&max_tag) {
                        *value = other_value.clone();
                    }
                }
            }
        }

        self.counter = self.counter.max(other.counter);
    }
}

/// Delta for [`AWMap`]: new tags and tombstones since another state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AWMapDelta<K: Ord + Clone, V: Clone + Eq> {
    additions: Vec<(K, V, (NodeId, u64))>,
    tombstones: BTreeSet<(NodeId, u64)>,
}

impl<K: Ord + Clone, V: Clone + Eq> DeltaCrdt for AWMap<K, V> {
    type Delta = AWMapDelta<K, V>;

    fn delta(&self, other: &Self) -> AWMapDelta<K, V> {
        let mut additions = Vec::new();
        for (key, (value, self_tags)) in &self.entries {
            let other_tags = other.entries.get(key).map(|(_, t)| t);
            for &tag in self_tags {
                let known = other_tags.is_some_and(|ot| ot.contains(&tag))
                    || other.tombstones.contains(&tag);
                if !known {
                    additions.push((key.clone(), value.clone(), tag));
                }
            }
        }

        let tombstones: BTreeSet<_> = self
            .tombstones
            .difference(&other.tombstones)
            .copied()
            .collect();

        AWMapDelta {
            additions,
            tombstones,
        }
    }

    fn apply_delta(&mut self, delta: &AWMapDelta<K, V>) {
        for (key, value, tag) in &delta.additions {
            if !self.tombstones.contains(tag) {
                let entry = self.entries.entry(key.clone()).or_insert_with(|| {
                    (value.clone(), BTreeSet::new())
                });
                entry.1.insert(*tag);
                // Update value if this is the new max tag
                if let Some(&max_tag) = entry.1.iter().next_back() {
                    if *tag == max_tag {
                        entry.0 = value.clone();
                    }
                }
            }
        }

        for &tag in &delta.tombstones {
            for (_, (_, tags)) in self.entries.iter_mut() {
                tags.remove(&tag);
            }
        }
        self.tombstones.extend(&delta.tombstones);

        self.entries.retain(|_, (_, tags)| !tags.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_map_is_empty() {
        let m = AWMap::<String, String>::new(1);
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn insert_and_get() {
        let mut m = AWMap::new(1);
        m.insert("key", "value");
        assert_eq!(m.get(&"key"), Some(&"value"));
        assert!(m.contains_key(&"key"));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn update_value() {
        let mut m = AWMap::new(1);
        m.insert("k", "v1");
        m.insert("k", "v2");
        assert_eq!(m.get(&"k"), Some(&"v2"));
    }

    #[test]
    fn remove_key() {
        let mut m = AWMap::new(1);
        m.insert("k", "v");
        assert!(m.remove(&"k"));
        assert!(!m.contains_key(&"k"));
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut m = AWMap::<&str, &str>::new(1);
        assert!(!m.remove(&"k"));
    }

    #[test]
    fn readd_after_remove() {
        let mut m = AWMap::new(1);
        m.insert("k", "v1");
        m.remove(&"k");
        m.insert("k", "v2");
        assert_eq!(m.get(&"k"), Some(&"v2"));
    }

    #[test]
    fn concurrent_add_survives_remove() {
        let mut m1 = AWMap::new(1);
        m1.insert("k", "v");
        m1.remove(&"k");

        let mut m2 = AWMap::new(2);
        m2.insert("k", "v");

        m1.merge(&m2);
        assert!(
            m1.contains_key(&"k"),
            "Concurrent add should survive remove (add wins)"
        );
    }

    #[test]
    fn merge_combines_entries() {
        let mut m1 = AWMap::new(1);
        m1.insert("a", 1);

        let mut m2 = AWMap::new(2);
        m2.insert("b", 2);

        m1.merge(&m2);
        assert_eq!(m1.get(&"a"), Some(&1));
        assert_eq!(m1.get(&"b"), Some(&2));
        assert_eq!(m1.len(), 2);
    }

    #[test]
    fn merge_is_commutative() {
        let mut m1 = AWMap::new(1);
        m1.insert("a", 1);
        m1.insert("b", 2);

        let mut m2 = AWMap::new(2);
        m2.insert("b", 3);
        m2.insert("c", 4);

        let mut left = m1.clone();
        left.merge(&m2);
        let left_keys: BTreeSet<_> = left.keys().collect();

        let mut right = m2.clone();
        right.merge(&m1);
        let right_keys: BTreeSet<_> = right.keys().collect();

        assert_eq!(left_keys, right_keys);
    }

    #[test]
    fn merge_is_idempotent() {
        let mut m1 = AWMap::new(1);
        m1.insert("k", "v");

        let m2 = m1.clone();
        m1.merge(&m2);
        let after = m1.clone();
        m1.merge(&m2);
        assert_eq!(m1, after);
    }

    #[test]
    fn merge_propagates_remove() {
        let mut m1 = AWMap::new(1);
        m1.insert("k", "v");

        let mut m2 = m1.clone();
        m2.remove(&"k");

        m1.merge(&m2);
        assert!(!m1.contains_key(&"k"));
    }

    #[test]
    fn delta_apply_equivalent_to_merge() {
        let mut m1 = AWMap::new(1);
        m1.insert("a", 1);
        m1.insert("b", 2);

        let mut m2 = AWMap::new(2);
        m2.insert("b", 3);
        m2.insert("c", 4);

        let mut via_merge = m2.clone();
        via_merge.merge(&m1);

        let mut via_delta = m2.clone();
        let d = m1.delta(&m2);
        via_delta.apply_delta(&d);

        let merge_keys: BTreeSet<_> = via_merge.keys().collect();
        let delta_keys: BTreeSet<_> = via_delta.keys().collect();
        assert_eq!(merge_keys, delta_keys);
    }

    #[test]
    fn delta_carries_tombstones() {
        let mut m1 = AWMap::new(1);
        m1.insert("k", "v");

        let m2 = m1.clone();
        m1.remove(&"k");

        let d = m1.delta(&m2);
        assert!(!d.tombstones.is_empty());

        let mut via_delta = m2.clone();
        via_delta.apply_delta(&d);
        assert!(!via_delta.contains_key(&"k"));
    }

    #[test]
    fn iterate_entries() {
        let mut m = AWMap::new(1);
        m.insert("a", 1);
        m.insert("b", 2);
        m.insert("c", 3);
        m.remove(&"b");

        let keys: Vec<_> = m.keys().collect();
        assert_eq!(keys, vec![&"a", &"c"]);
    }
}
