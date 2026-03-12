use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::clock::HybridTimestamp;
use crate::{Crdt, DeltaCrdt};

/// A last-writer-wins map (LWW-Map).
///
/// Each key maps to a value with a [`HybridTimestamp`]. Concurrent writes
/// to the same key resolve by keeping the value with the highest timestamp.
/// Keys can be removed; a remove is stored as a tombstone with a timestamp
/// so that stale puts don't resurrect deleted keys.
///
/// # Example
///
/// ```
/// use crdt_kit::prelude::*;
/// use crdt_kit::clock::HybridTimestamp;
///
/// let ts = |ms, node| HybridTimestamp { physical: ms, logical: 0, node_id: node };
///
/// let mut m1 = LWWMap::new();
/// m1.insert("color", "red", ts(100, 1));
///
/// let mut m2 = LWWMap::new();
/// m2.insert("color", "blue", ts(200, 2));
///
/// m1.merge(&m2);
/// assert_eq!(m1.get(&"color"), Some(&"blue")); // later timestamp wins
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LWWMap<K: Ord + Clone, V: Clone> {
    /// Each key maps to (value, timestamp, alive).
    /// `alive` is `true` for puts, `false` for removes.
    entries: BTreeMap<K, Entry<V>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Entry<V: Clone> {
    value: Option<V>,
    timestamp: HybridTimestamp,
    alive: bool,
}

impl<K: Ord + Clone, V: Clone> LWWMap<K, V> {
    /// Create a new empty LWW-Map.
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Insert or update a key-value pair with the given timestamp.
    ///
    /// If the key already exists with a newer or equal timestamp, this is a no-op.
    pub fn insert(&mut self, key: K, value: V, timestamp: HybridTimestamp) {
        match self.entries.get(&key) {
            Some(entry) if entry.timestamp >= timestamp => {}
            _ => {
                self.entries.insert(
                    key,
                    Entry {
                        value: Some(value),
                        timestamp,
                        alive: true,
                    },
                );
            }
        }
    }

    /// Remove a key with the given timestamp.
    ///
    /// The removal only takes effect if its timestamp is greater than the
    /// current entry's timestamp. Returns `true` if the key was removed.
    pub fn remove(&mut self, key: &K, timestamp: HybridTimestamp) -> bool {
        match self.entries.get(key) {
            Some(entry) if entry.timestamp >= timestamp => false,
            _ => {
                self.entries.insert(
                    key.clone(),
                    Entry {
                        value: None,
                        timestamp,
                        alive: false,
                    },
                );
                true
            }
        }
    }

    /// Get the value associated with a key, if it exists and is alive.
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries
            .get(key)
            .filter(|e| e.alive)
            .and_then(|e| e.value.as_ref())
    }

    /// Check if a key is present and alive in the map.
    #[must_use]
    pub fn contains_key(&self, key: &K) -> bool {
        self.entries.get(key).is_some_and(|e| e.alive)
    }

    /// Get the number of alive keys.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.values().filter(|e| e.alive).count()
    }

    /// Check if the map has no alive keys.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over alive key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries
            .iter()
            .filter_map(|(k, e)| {
                if e.alive {
                    e.value.as_ref().map(|v| (k, v))
                } else {
                    None
                }
            })
    }

    /// Get all alive keys.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(k, _)| k)
    }

    /// Get all alive values.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }
}

    /// Returns the number of tombstoned (removed) entries.
    #[must_use]
    pub fn tombstone_count(&self) -> usize {
        self.entries.values().filter(|e| !e.alive).count()
    }

    /// Remove all tombstoned entries unconditionally.
    ///
    /// # Safety (logical)
    ///
    /// Only call after all replicas have fully converged. If a stale
    /// replica later sends a remove with a timestamp matching a purged
    /// tombstone, the key could be incorrectly resurrected.
    pub fn compact_tombstones_all(&mut self) {
        self.entries.retain(|_, e| e.alive);
    }

    /// Remove tombstoned entries older than `max_age_ms + 2 * sync_latency_bound_ms`.
    ///
    /// **WARNING:** This method can violate LWW semantics if the latency bound
    /// is wrong. Only use when you have a **hard guarantee** on maximum network
    /// latency and clock skew across all replicas.
    ///
    /// The safety margin (`2 * sync_latency_bound_ms`) accounts for:
    /// - One-way network delay for the remove to propagate
    /// - One-way network delay for a stale put to arrive
    ///
    /// If you cannot bound sync latency, use `compact_tombstones_all()` only
    /// after confirmed full convergence.
    pub fn compact_tombstones_with_age(
        &mut self,
        now_physical: u64,
        max_age_ms: u64,
        sync_latency_bound_ms: u64,
    ) {
        let safe_cutoff = max_age_ms.saturating_add(2 * sync_latency_bound_ms);
        self.entries.retain(|_, e| {
            if e.alive {
                return true;
            }
            // Keep tombstones within the safety window
            now_physical.saturating_sub(e.timestamp.physical) < safe_cutoff
        });
    }
}

impl<K: Ord + Clone, V: Clone> Default for LWWMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord + Clone, V: Clone> Crdt for LWWMap<K, V> {
    fn merge(&mut self, other: &Self) {
        for (key, other_entry) in &other.entries {
            match self.entries.get(key) {
                Some(self_entry) if self_entry.timestamp >= other_entry.timestamp => {}
                _ => {
                    self.entries.insert(key.clone(), other_entry.clone());
                }
            }
        }
    }
}

/// Delta for [`LWWMap`]: entries that are newer in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LWWMapDelta<K: Ord + Clone, V: Clone> {
    entries: Vec<(K, Option<V>, HybridTimestamp, bool)>,
}

impl<K: Ord + Clone, V: Clone> DeltaCrdt for LWWMap<K, V> {
    type Delta = LWWMapDelta<K, V>;

    fn delta(&self, other: &Self) -> LWWMapDelta<K, V> {
        let mut entries = Vec::new();
        for (key, self_entry) in &self.entries {
            let dominated = other
                .entries
                .get(key)
                .is_some_and(|oe| oe.timestamp >= self_entry.timestamp);
            if !dominated {
                entries.push((
                    key.clone(),
                    self_entry.value.clone(),
                    self_entry.timestamp,
                    self_entry.alive,
                ));
            }
        }
        LWWMapDelta { entries }
    }

    fn apply_delta(&mut self, delta: &LWWMapDelta<K, V>) {
        for (key, value, timestamp, alive) in &delta.entries {
            match self.entries.get(key) {
                Some(entry) if entry.timestamp >= *timestamp => {}
                _ => {
                    self.entries.insert(
                        key.clone(),
                        Entry {
                            value: value.clone(),
                            timestamp: *timestamp,
                            alive: *alive,
                        },
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(physical: u64, node: u16) -> HybridTimestamp {
        HybridTimestamp {
            physical,
            logical: 0,
            node_id: node,
        }
    }

    #[test]
    fn new_map_is_empty() {
        let m = LWWMap::<String, String>::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn insert_and_get() {
        let mut m = LWWMap::new();
        m.insert("key", "value", ts(1, 1));
        assert_eq!(m.get(&"key"), Some(&"value"));
        assert!(m.contains_key(&"key"));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn later_write_wins() {
        let mut m = LWWMap::new();
        m.insert("k", "old", ts(1, 1));
        m.insert("k", "new", ts(2, 1));
        assert_eq!(m.get(&"k"), Some(&"new"));
    }

    #[test]
    fn stale_write_ignored() {
        let mut m = LWWMap::new();
        m.insert("k", "new", ts(2, 1));
        m.insert("k", "old", ts(1, 1));
        assert_eq!(m.get(&"k"), Some(&"new"));
    }

    #[test]
    fn remove_hides_key() {
        let mut m = LWWMap::new();
        m.insert("k", "v", ts(1, 1));
        assert!(m.remove(&"k", ts(2, 1)));
        assert!(!m.contains_key(&"k"));
        assert_eq!(m.get(&"k"), None);
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn stale_remove_ignored() {
        let mut m = LWWMap::new();
        m.insert("k", "v", ts(2, 1));
        assert!(!m.remove(&"k", ts(1, 1)));
        assert!(m.contains_key(&"k"));
    }

    #[test]
    fn insert_after_remove() {
        let mut m = LWWMap::new();
        m.insert("k", "v1", ts(1, 1));
        m.remove(&"k", ts(2, 1));
        m.insert("k", "v2", ts(3, 1));
        assert_eq!(m.get(&"k"), Some(&"v2"));
    }

    #[test]
    fn merge_later_wins() {
        let mut m1 = LWWMap::new();
        m1.insert("k", "old", ts(1, 1));

        let mut m2 = LWWMap::new();
        m2.insert("k", "new", ts(2, 2));

        m1.merge(&m2);
        assert_eq!(m1.get(&"k"), Some(&"new"));
    }

    #[test]
    fn merge_is_commutative() {
        let mut m1 = LWWMap::new();
        m1.insert("a", 1, ts(1, 1));
        m1.insert("b", 2, ts(2, 1));

        let mut m2 = LWWMap::new();
        m2.insert("b", 3, ts(3, 2));
        m2.insert("c", 4, ts(1, 2));

        let mut left = m1.clone();
        left.merge(&m2);

        let mut right = m2.clone();
        right.merge(&m1);

        assert_eq!(left, right);
    }

    #[test]
    fn merge_is_idempotent() {
        let mut m1 = LWWMap::new();
        m1.insert("k", "v", ts(1, 1));

        let m2 = m1.clone();
        m1.merge(&m2);
        let after = m1.clone();
        m1.merge(&m2);
        assert_eq!(m1, after);
    }

    #[test]
    fn merge_propagates_remove() {
        let mut m1 = LWWMap::new();
        m1.insert("k", "v", ts(1, 1));

        let mut m2 = m1.clone();
        m2.remove(&"k", ts(2, 2));

        m1.merge(&m2);
        assert!(!m1.contains_key(&"k"));
    }

    #[test]
    fn delta_apply_equivalent_to_merge() {
        let mut m1 = LWWMap::new();
        m1.insert("a", 1, ts(1, 1));
        m1.insert("b", 2, ts(3, 1));

        let mut m2 = LWWMap::new();
        m2.insert("b", 3, ts(2, 2));
        m2.insert("c", 4, ts(1, 2));

        let mut via_merge = m2.clone();
        via_merge.merge(&m1);

        let mut via_delta = m2.clone();
        let d = m1.delta(&m2);
        via_delta.apply_delta(&d);

        assert_eq!(via_merge, via_delta);
    }

    #[test]
    fn delta_is_empty_when_dominated() {
        let mut m1 = LWWMap::new();
        m1.insert("k", "old", ts(1, 1));

        let mut m2 = LWWMap::new();
        m2.insert("k", "new", ts(2, 2));

        let d = m1.delta(&m2);
        assert!(d.entries.is_empty());
    }

    #[test]
    fn iterate_alive_entries() {
        let mut m = LWWMap::new();
        m.insert("a", 1, ts(1, 1));
        m.insert("b", 2, ts(2, 1));
        m.insert("c", 3, ts(3, 1));
        m.remove(&"b", ts(4, 1));

        let keys: Vec<_> = m.keys().collect();
        assert_eq!(keys, vec![&"a", &"c"]);
    }
}
