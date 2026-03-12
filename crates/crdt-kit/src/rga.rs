use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::fmt;

use crate::rope::ChunkedVec;
use crate::{Crdt, DeltaCrdt, NodeId};

/// Error type for RGA operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RgaError {
    /// Index is out of bounds for the current visible sequence length.
    IndexOutOfBounds {
        /// The index that was requested.
        index: usize,
        /// The current length of the visible sequence.
        len: usize,
    },
}

impl fmt::Display for RgaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IndexOutOfBounds { index, len } => {
                write!(f, "index {index} out of bounds for length {len}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RgaError {}

/// A single node in the RGA sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RgaNode<T: Clone + Ord> {
    /// Unique identifier: (actor, counter).
    pub id: (NodeId, u64),
    /// The element value.
    pub value: T,
    /// Whether this element has been tombstoned (logically deleted).
    pub deleted: bool,
}

/// A Replicated Growable Array (RGA) — an ordered sequence CRDT.
///
/// RGA supports insert and delete at arbitrary positions while
/// guaranteeing convergence across replicas. Each element is assigned
/// a unique identifier `(actor, counter)` which determines causal
/// ordering.
///
/// # Example
///
/// ```
/// use crdt_kit::prelude::*;
///
/// let mut list1 = Rga::new(1);
/// list1.insert_at(0, 'H').unwrap();
/// list1.insert_at(1, 'i').unwrap();
///
/// let mut list2 = Rga::new(2);
/// list2.insert_at(0, '!').unwrap();
///
/// list1.merge(&list2);
/// list2.merge(&list1);
///
/// // Both replicas converge to the same sequence
/// let v1: Vec<&char> = list1.iter().collect();
/// let v2: Vec<&char> = list2.iter().collect();
/// assert_eq!(v1, v2);
/// assert_eq!(list1.len(), 3);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rga<T: Clone + Ord> {
    actor: NodeId,
    counter: u64,
    /// Ordered sequence of elements (including tombstones), backed by a chunked rope.
    elements: ChunkedVec<RgaNode<T>>,
    /// Version vector: max counter observed per actor.
    version: BTreeMap<NodeId, u64>,
    /// Cached count of visible (non-tombstoned) elements.
    visible_len: usize,
}

impl<T: Clone + Ord> Rga<T> {
    /// Create a new empty RGA for the given node.
    pub fn new(actor: NodeId) -> Self {
        Self {
            actor,
            counter: 0,
            elements: ChunkedVec::new(),
            version: BTreeMap::new(),
            visible_len: 0,
        }
    }

    /// Create a fork of this replica with a different node ID.
    pub fn fork(&self, new_actor: NodeId) -> Self {
        Self {
            actor: new_actor,
            counter: self.counter,
            elements: self.elements.clone(),
            version: self.version.clone(),
            visible_len: self.visible_len,
        }
    }

    /// Insert a value at the given index in the visible sequence.
    pub fn insert_at(&mut self, index: usize, value: T) -> Result<(), RgaError> {
        if index > self.visible_len {
            return Err(RgaError::IndexOutOfBounds {
                index,
                len: self.visible_len,
            });
        }

        self.counter += 1;
        let id = (self.actor, self.counter);
        self.version
            .entry(self.actor)
            .and_modify(|c| *c = (*c).max(self.counter))
            .or_insert(self.counter);

        let node = RgaNode {
            id,
            value,
            deleted: false,
        };

        let raw_index = self.raw_index_for_insert(index);
        self.elements.insert(raw_index, node);
        self.visible_len += 1;
        Ok(())
    }

    /// Remove the element at the given index from the visible sequence.
    pub fn remove(&mut self, index: usize) -> Result<T, RgaError> {
        if index >= self.visible_len {
            return Err(RgaError::IndexOutOfBounds {
                index,
                len: self.visible_len,
            });
        }
        let raw = self.visible_to_raw(index);
        self.elements[raw].deleted = true;
        self.visible_len -= 1;
        Ok(self.elements[raw].value.clone())
    }

    /// Get a reference to the element at the given index in the visible sequence.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.visible_len {
            return None;
        }
        let raw = self.visible_to_raw(index);
        Some(&self.elements[raw].value)
    }

    /// Get the number of visible (non-tombstoned) elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.visible_len
    }

    /// Check if the visible sequence is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.visible_len == 0
    }

    /// Iterate over the visible elements in order.
    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        self.elements
            .iter()
            .filter(|n| !n.deleted)
            .map(|n| &n.value)
    }

    /// Get this replica's node ID.
    #[must_use]
    pub fn actor(&self) -> NodeId {
        self.actor
    }

    /// Collect visible elements into a `Vec`.
    #[must_use]
    pub fn to_vec(&self) -> Vec<T> {
        self.iter().cloned().collect()
    }

    /// Returns the number of tombstoned (deleted) elements.
    #[must_use]
    pub fn tombstone_count(&self) -> usize {
        self.elements.iter().filter(|n| n.deleted).count()
    }

    /// Returns the total number of elements (including tombstones).
    #[must_use]
    pub fn raw_len(&self) -> usize {
        self.elements.len()
    }

    /// Remove all tombstoned elements from the internal storage.
    ///
    /// # UNSAFE — Read before using
    ///
    /// **This method can cause replica divergence.** The RGA insertion
    /// algorithm (`find_insert_position`) depends on tombstoned elements
    /// to determine correct causal ordering. Removing them changes raw
    /// indices and can cause concurrent inserts to land at wrong positions.
    ///
    /// **Only safe when ALL of these conditions are met:**
    /// 1. All replicas have fully converged (identical state)
    /// 2. No in-flight insert/delete operations exist
    /// 3. No replica will ever send operations referencing removed IDs
    /// 4. All replicas compact simultaneously (or no further syncs occur)
    ///
    /// **Prefer not calling this at all.** If storage is critical, consider
    /// archiving the RGA and starting a fresh replica instead.
    ///
    /// Returns the number of tombstones removed.
    pub fn compact_tombstones(&mut self) -> usize {
        let before = self.elements.len();
        let mut kept = ChunkedVec::new();
        for node in self.elements.iter() {
            if !node.deleted {
                kept.push(node.clone());
            }
        }
        let removed = before - kept.len();
        self.elements = kept;
        removed
    }

    // ---- internal helpers ----

    fn visible_to_raw(&self, visible: usize) -> usize {
        let mut seen = 0;
        for (raw, node) in self.elements.iter().enumerate() {
            if !node.deleted {
                if seen == visible {
                    return raw;
                }
                seen += 1;
            }
        }
        panic!(
            "visible index {} not found (only {} visible elements)",
            visible, seen
        );
    }

    fn raw_index_for_insert(&self, visible_index: usize) -> usize {
        if visible_index == 0 {
            return 0;
        }
        if visible_index >= self.visible_len {
            return self.elements.len();
        }
        self.visible_to_raw(visible_index)
    }

    fn find_insert_position(&self, node: &RgaNode<T>, after_raw: Option<usize>) -> usize {
        let start = match after_raw {
            Some(idx) => idx + 1,
            None => 0,
        };

        let new_key = (node.id.1, node.id.0); // (counter, actor)

        for i in start..self.elements.len() {
            let existing = &self.elements[i];
            let existing_key = (existing.id.1, existing.id.0);
            if existing_key < new_key {
                return i;
            }
        }

        self.elements.len()
    }
}

/// Delta for [`Rga`]: elements and tombstones that the other replica is missing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RgaDelta<T: Clone + Ord> {
    /// Elements that the other replica doesn't have yet.
    pub new_elements: Vec<RgaNode<T>>,
    /// IDs of elements that are deleted in source but not in other.
    pub tombstoned_ids: Vec<(NodeId, u64)>,
    /// Version vector of the source.
    pub version: BTreeMap<NodeId, u64>,
}

impl<T: Clone + Ord> DeltaCrdt for Rga<T> {
    type Delta = RgaDelta<T>;

    fn delta(&self, other: &Self) -> RgaDelta<T> {
        let new_elements: Vec<_> = self
            .elements
            .iter()
            .filter(|e| {
                let actor_max = other.version.get(&e.id.0).copied().unwrap_or(0);
                e.id.1 > actor_max
            })
            .cloned()
            .collect();

        let tombstoned_ids: Vec<_> = self
            .elements
            .iter()
            .filter(|e| {
                e.deleted && {
                    let actor_max = other.version.get(&e.id.0).copied().unwrap_or(0);
                    e.id.1 <= actor_max
                }
            })
            .map(|e| e.id)
            .collect();

        RgaDelta {
            new_elements,
            tombstoned_ids,
            version: self.version.clone(),
        }
    }

    fn apply_delta(&mut self, delta: &RgaDelta<T>) {
        // Phase 1: Apply tombstones using a pre-built index (no shifts yet).
        let id_index: BTreeMap<(NodeId, u64), usize> = self
            .elements
            .iter()
            .enumerate()
            .map(|(i, e)| (e.id, i))
            .collect();

        for &id in &delta.tombstoned_ids {
            if let Some(&raw) = id_index.get(&id) {
                if !self.elements[raw].deleted {
                    self.elements[raw].deleted = true;
                    self.visible_len -= 1;
                }
            }
        }

        // Phase 2: Insert new elements. Use a set for dedup; find
        // predecessor positions by scanning self.elements directly,
        // avoiding the O(k) index-shift loop per insertion.
        let mut known_ids: BTreeSet<(NodeId, u64)> =
            self.elements.iter().map(|e| e.id).collect();

        for (delta_idx, elem) in delta.new_elements.iter().enumerate() {
            if !known_ids.contains(&elem.id) {
                let predecessor_raw = if delta_idx == 0 {
                    None
                } else {
                    (0..delta_idx).rev().find_map(|i| {
                        self.elements
                            .iter()
                            .position(|e| e.id == delta.new_elements[i].id)
                    })
                };

                let pos = self.find_insert_position(elem, predecessor_raw);
                self.elements.insert(pos, elem.clone());
                if !elem.deleted {
                    self.visible_len += 1;
                }
                known_ids.insert(elem.id);
            }
        }

        for (&actor, &cnt) in &delta.version {
            let entry = self.version.entry(actor).or_insert(0);
            *entry = (*entry).max(cnt);
        }

        if let Some(&max_cnt) = self.version.values().max() {
            self.counter = self.counter.max(max_cnt);
        }
    }
}

impl<T: Clone + Ord> Crdt for Rga<T> {
    fn merge(&mut self, other: &Self) {
        // Phase 1: Apply tombstones using a pre-built index (no shifts yet).
        let id_index: BTreeMap<(NodeId, u64), usize> = self
            .elements
            .iter()
            .enumerate()
            .map(|(i, e)| (e.id, i))
            .collect();

        for other_elem in other.elements.iter() {
            if other_elem.deleted {
                if let Some(&raw) = id_index.get(&other_elem.id) {
                    if !self.elements[raw].deleted {
                        self.elements[raw].deleted = true;
                        self.visible_len -= 1;
                    }
                }
            }
        }

        // Phase 2: Insert new elements. Use a set for dedup; find
        // predecessor positions by scanning self.elements directly,
        // avoiding the O(k) index-shift loop per insertion.
        let mut known_ids: BTreeSet<(NodeId, u64)> =
            self.elements.iter().map(|e| e.id).collect();

        for (other_idx, other_elem) in other.elements.iter().enumerate() {
            if !known_ids.contains(&other_elem.id) {
                let predecessor_raw = if other_idx == 0 {
                    None
                } else {
                    (0..other_idx).rev().find_map(|i| {
                        self.elements
                            .iter()
                            .position(|e| e.id == other.elements[i].id)
                    })
                };

                let pos = self.find_insert_position(other_elem, predecessor_raw);
                self.elements.insert(pos, other_elem.clone());
                if !other_elem.deleted {
                    self.visible_len += 1;
                }
                known_ids.insert(other_elem.id);
            }
        }

        for (&actor, &cnt) in &other.version {
            let entry = self.version.entry(actor).or_insert(0);
            *entry = (*entry).max(cnt);
        }

        if let Some(&max_cnt) = self.version.values().max() {
            self.counter = self.counter.max(max_cnt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rga_is_empty() {
        let rga = Rga::<String>::new(1);
        assert!(rga.is_empty());
        assert_eq!(rga.len(), 0);
        assert_eq!(rga.get(0), None);
    }

    #[test]
    fn insert_at_head() {
        let mut rga = Rga::new(1);
        rga.insert_at(0, 'H').unwrap();
        rga.insert_at(1, 'i').unwrap();
        assert_eq!(rga.len(), 2);
        assert_eq!(rga.get(0), Some(&'H'));
        assert_eq!(rga.get(1), Some(&'i'));
    }

    #[test]
    fn insert_at_middle() {
        let mut rga = Rga::new(1);
        rga.insert_at(0, 'a').unwrap();
        rga.insert_at(1, 'c').unwrap();
        rga.insert_at(1, 'b').unwrap();
        assert_eq!(rga.to_vec(), vec!['a', 'b', 'c']);
    }

    #[test]
    fn insert_out_of_bounds_returns_error() {
        let mut rga = Rga::new(1);
        rga.insert_at(0, 'x').unwrap();
        let err = rga.insert_at(5, 'y');
        assert_eq!(
            err,
            Err(RgaError::IndexOutOfBounds { index: 5, len: 1 })
        );
    }

    #[test]
    fn remove_element() {
        let mut rga = Rga::new(1);
        rga.insert_at(0, 'a').unwrap();
        rga.insert_at(1, 'b').unwrap();
        rga.insert_at(2, 'c').unwrap();

        let removed = rga.remove(1).unwrap();
        assert_eq!(removed, 'b');
        assert_eq!(rga.len(), 2);
        assert_eq!(rga.to_vec(), vec!['a', 'c']);
    }

    #[test]
    fn merge_disjoint_inserts() {
        let mut r1 = Rga::new(1);
        r1.insert_at(0, 'x').unwrap();

        let mut r2 = Rga::new(2);
        r2.insert_at(0, 'y').unwrap();

        r1.merge(&r2);
        assert_eq!(r1.len(), 2);
        let v = r1.to_vec();
        assert!(v.contains(&'x'));
        assert!(v.contains(&'y'));
    }

    #[test]
    fn merge_concurrent_inserts_at_same_position() {
        let mut r1 = Rga::new(1);
        r1.insert_at(0, 'A').unwrap();

        let mut r2 = Rga::new(2);
        r2.insert_at(0, 'B').unwrap();

        let mut r1_copy = r1.clone();
        let mut r2_copy = r2.clone();

        r1_copy.merge(&r2);
        r2_copy.merge(&r1);

        assert_eq!(r1_copy.to_vec(), r2_copy.to_vec());
        assert_eq!(r1_copy.len(), 2);
    }

    #[test]
    fn merge_concurrent_inserts_after_shared_prefix() {
        let mut r1 = Rga::new(1);
        r1.insert_at(0, 'H').unwrap();
        r1.insert_at(1, 'e').unwrap();

        let mut r2 = r1.fork(2);

        r1.insert_at(2, 'X').unwrap();
        r2.insert_at(2, 'Y').unwrap();

        let mut r1_merged = r1.clone();
        r1_merged.merge(&r2);

        let mut r2_merged = r2.clone();
        r2_merged.merge(&r1);

        assert_eq!(r1_merged.to_vec(), r2_merged.to_vec());
        assert_eq!(r1_merged.len(), 4);
        assert_eq!(r1_merged.get(0), Some(&'H'));
        assert_eq!(r1_merged.get(1), Some(&'e'));
    }

    #[test]
    fn merge_with_deletions() {
        let mut r1 = Rga::new(1);
        r1.insert_at(0, 'a').unwrap();
        r1.insert_at(1, 'b').unwrap();
        r1.insert_at(2, 'c').unwrap();

        let mut r2 = r1.fork(2);

        r1.remove(1).unwrap();
        r2.insert_at(3, 'd').unwrap();

        r1.merge(&r2);
        assert!(!r1.to_vec().contains(&'b'));
        assert!(r1.to_vec().contains(&'d'));
        assert_eq!(r1.len(), 3);
    }

    #[test]
    fn merge_is_commutative() {
        let mut r1 = Rga::new(1);
        r1.insert_at(0, 1).unwrap();
        r1.insert_at(1, 2).unwrap();

        let mut r2 = Rga::new(2);
        r2.insert_at(0, 3).unwrap();
        r2.insert_at(1, 4).unwrap();

        let mut left = r1.clone();
        left.merge(&r2);

        let mut right = r2.clone();
        right.merge(&r1);

        assert_eq!(left.to_vec(), right.to_vec());
    }

    #[test]
    fn merge_is_idempotent() {
        let mut r1 = Rga::new(1);
        r1.insert_at(0, 'x').unwrap();
        r1.insert_at(1, 'y').unwrap();

        let mut r2 = Rga::new(2);
        r2.insert_at(0, 'z').unwrap();

        r1.merge(&r2);
        let after_first = r1.clone();
        r1.merge(&r2);
        assert_eq!(r1, after_first);
    }

    #[test]
    fn delta_apply_equivalent_to_merge() {
        let mut r1 = Rga::new(1);
        r1.insert_at(0, 'H').unwrap();
        r1.insert_at(1, 'i').unwrap();

        let mut r2 = Rga::new(2);
        r2.insert_at(0, '!').unwrap();

        let mut via_merge = r2.clone();
        via_merge.merge(&r1);

        let mut via_delta = r2.clone();
        let d = r1.delta(&r2);
        via_delta.apply_delta(&d);

        assert_eq!(via_merge.to_vec(), via_delta.to_vec());
    }

    #[test]
    fn fork_creates_independent_replica() {
        let mut r1 = Rga::new(1);
        r1.insert_at(0, 'x').unwrap();
        r1.insert_at(1, 'y').unwrap();

        let mut r2 = r1.fork(2);
        r2.insert_at(2, 'z').unwrap();

        assert_eq!(r1.len(), 2);
        assert_eq!(r2.len(), 3);

        r1.merge(&r2);
        assert_eq!(r1.to_vec(), vec!['x', 'y', 'z']);
    }
}
