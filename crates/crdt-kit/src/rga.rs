use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use crate::Crdt;

/// A unique node identifier: `(actor, counter)`.
type NodeId = (String, u64);
/// Map from parent id to list of child ids used during sequence rebuild.
type ChildrenMap = BTreeMap<Option<NodeId>, Vec<NodeId>>;

/// A Replicated Growable Array (RGA) — an ordered sequence CRDT.
///
/// RGA supports insert and delete at arbitrary positions while
/// guaranteeing convergence across replicas. Each element is assigned
/// a unique identifier `(actor, counter)` which determines causal
/// ordering. When two replicas concurrently insert at the same
/// position, the conflict is resolved deterministically by comparing
/// the unique identifiers, ensuring all replicas converge to the
/// same sequence after merging.
///
/// # Example
///
/// ```
/// use crdt_kit::prelude::*;
///
/// let mut list1 = Rga::new("node-1");
/// list1.insert_at(0, 'H');
/// list1.insert_at(1, 'i');
///
/// let mut list2 = Rga::new("node-2");
/// list2.insert_at(0, '!');
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
    actor: String,
    counter: u64,
    /// Ordered backbone: each node is identified by a unique `(actor, counter)`.
    /// The value is `(element, parent_id)` where `parent_id` is the id of the
    /// node after which this node was inserted (`None` for a head insert).
    nodes: BTreeMap<(String, u64), RgaNode<T>>,
    /// Set of ids that have been tombstoned (logically deleted).
    tombstones: BTreeSet<(String, u64)>,
    /// Cached linear order of node ids. Recomputed on mutation.
    sequence: Vec<(String, u64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct RgaNode<T: Clone + Ord> {
    value: T,
    /// The id of the node after which this node was inserted.
    /// `None` means it was inserted at the head of the list.
    parent: Option<(String, u64)>,
}

impl<T: Clone + Ord> Rga<T> {
    /// Create a new empty RGA for the given actor.
    pub fn new(actor: impl Into<String>) -> Self {
        Self {
            actor: actor.into(),
            counter: 0,
            nodes: BTreeMap::new(),
            tombstones: BTreeSet::new(),
            sequence: Vec::new(),
        }
    }

    /// Insert a value at the given index in the visible sequence.
    ///
    /// # Panics
    ///
    /// Panics if `index > self.len()`.
    pub fn insert_at(&mut self, index: usize, value: T) {
        let visible = self.visible_sequence();
        assert!(
            index <= visible.len(),
            "index {} out of bounds for length {}",
            index,
            visible.len()
        );

        let parent = if index == 0 {
            None
        } else {
            Some(visible[index - 1].clone())
        };

        self.counter += 1;
        let id = (self.actor.clone(), self.counter);

        self.nodes.insert(
            id,
            RgaNode {
                value,
                parent: parent.clone(),
            },
        );

        self.rebuild_sequence();
    }

    /// Remove the element at the given index from the visible sequence.
    ///
    /// Returns the removed value, or `None` if the index is out of bounds.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        let visible = self.visible_sequence();
        if index >= visible.len() {
            return None;
        }

        let id = visible[index].clone();
        self.tombstones.insert(id.clone());
        self.rebuild_sequence();

        self.nodes.get(&id).map(|node| node.value.clone())
    }

    /// Get a reference to the element at the given index in the visible sequence.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        let visible = self.visible_sequence();
        visible
            .get(index)
            .and_then(|id| self.nodes.get(id))
            .map(|node| &node.value)
    }

    /// Get the number of visible (non-tombstoned) elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.visible_sequence().len()
    }

    /// Check if the visible sequence is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over the visible elements in order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let visible = self.visible_sequence();
        visible
            .into_iter()
            .filter_map(move |id| self.nodes.get(&id).map(|node| &node.value))
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// Get this replica's actor ID.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }

    /// Collect visible elements into a `Vec`.
    #[must_use]
    pub fn to_vec(&self) -> Vec<T> {
        self.iter().cloned().collect()
    }

    /// Return the ordered list of ids for visible (non-tombstoned) elements.
    fn visible_sequence(&self) -> Vec<(String, u64)> {
        self.sequence
            .iter()
            .filter(|id| !self.tombstones.contains(id))
            .cloned()
            .collect()
    }

    /// Rebuild the linearised sequence from the DAG of nodes.
    ///
    /// The algorithm works by grouping all nodes by their parent id, then
    /// performing a depth-first traversal starting from nodes whose parent
    /// is `None` (i.e. inserted at the head). Among siblings (nodes sharing
    /// the same parent), we sort by id in *reverse* order so that the most
    /// recent / highest-priority node appears first — this matches RGA
    /// semantics where a newer concurrent insert at the same position
    /// appears before older ones.
    fn rebuild_sequence(&mut self) {
        // Group children by parent id.
        let mut children: ChildrenMap = BTreeMap::new();
        for (id, node) in &self.nodes {
            children
                .entry(node.parent.clone())
                .or_default()
                .push(id.clone());
        }

        // Sort each sibling group so that higher ids come first.
        // Higher id = larger counter, or same counter but lexicographically
        // larger actor, which ensures deterministic total ordering.
        for siblings in children.values_mut() {
            siblings.sort_by(|a, b| b.cmp(a));
        }

        // DFS traversal to build the linear sequence.
        let mut sequence = Vec::with_capacity(self.nodes.len());
        let mut stack: Vec<(String, u64)> = Vec::new();

        // Push root children (parent == None) onto the stack.
        if let Some(roots) = children.get(&None) {
            // Push in reverse so that the highest-priority node is processed first.
            for id in roots.iter().rev() {
                stack.push(id.clone());
            }
        }

        while let Some(id) = stack.pop() {
            sequence.push(id.clone());
            // Push this node's children in reverse order.
            if let Some(kids) = children.get(&Some(id)) {
                for kid in kids.iter().rev() {
                    stack.push(kid.clone());
                }
            }
        }

        self.sequence = sequence;
    }
}

impl<T: Clone + Ord> Crdt for Rga<T> {
    fn merge(&mut self, other: &Self) {
        // Import all nodes from the other replica that we don't have yet.
        for (id, node) in &other.nodes {
            self.nodes.entry(id.clone()).or_insert_with(|| node.clone());
        }

        // Merge tombstones (union).
        self.tombstones.extend(other.tombstones.iter().cloned());

        // Update our counter to be at least as high as the other's.
        self.counter = self.counter.max(other.counter);

        // Rebuild the sequence to reflect the merged state.
        self.rebuild_sequence();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rga_is_empty() {
        let rga = Rga::<String>::new("a");
        assert!(rga.is_empty());
        assert_eq!(rga.len(), 0);
        assert_eq!(rga.get(0), None);
    }

    #[test]
    fn insert_at_head() {
        let mut rga = Rga::new("a");
        rga.insert_at(0, 'H');
        rga.insert_at(1, 'i');
        assert_eq!(rga.len(), 2);
        assert_eq!(rga.get(0), Some(&'H'));
        assert_eq!(rga.get(1), Some(&'i'));
    }

    #[test]
    fn insert_at_middle() {
        let mut rga = Rga::new("a");
        rga.insert_at(0, 'a');
        rga.insert_at(1, 'c');
        rga.insert_at(1, 'b');
        assert_eq!(rga.to_vec(), vec!['a', 'b', 'c']);
    }

    #[test]
    fn insert_at_end() {
        let mut rga = Rga::new("a");
        rga.insert_at(0, 1);
        rga.insert_at(1, 2);
        rga.insert_at(2, 3);
        assert_eq!(rga.to_vec(), vec![1, 2, 3]);
    }

    #[test]
    #[should_panic(expected = "index 5 out of bounds")]
    fn insert_out_of_bounds_panics() {
        let mut rga = Rga::new("a");
        rga.insert_at(0, 'x');
        rga.insert_at(5, 'y');
    }

    #[test]
    fn remove_element() {
        let mut rga = Rga::new("a");
        rga.insert_at(0, 'a');
        rga.insert_at(1, 'b');
        rga.insert_at(2, 'c');

        let removed = rga.remove(1);
        assert_eq!(removed, Some('b'));
        assert_eq!(rga.len(), 2);
        assert_eq!(rga.to_vec(), vec!['a', 'c']);
    }

    #[test]
    fn remove_out_of_bounds_returns_none() {
        let mut rga = Rga::new("a");
        rga.insert_at(0, 'a');
        assert_eq!(rga.remove(5), None);
        assert_eq!(rga.len(), 1);
    }

    #[test]
    fn remove_first_and_last() {
        let mut rga = Rga::new("a");
        rga.insert_at(0, 'a');
        rga.insert_at(1, 'b');
        rga.insert_at(2, 'c');

        rga.remove(0);
        assert_eq!(rga.to_vec(), vec!['b', 'c']);

        rga.remove(1);
        assert_eq!(rga.to_vec(), vec!['b']);

        rga.remove(0);
        assert!(rga.is_empty());
    }

    #[test]
    fn get_returns_correct_values() {
        let mut rga = Rga::new("a");
        rga.insert_at(0, "hello");
        rga.insert_at(1, "world");
        assert_eq!(rga.get(0), Some(&"hello"));
        assert_eq!(rga.get(1), Some(&"world"));
        assert_eq!(rga.get(2), None);
    }

    #[test]
    fn iterate_elements() {
        let mut rga = Rga::new("a");
        rga.insert_at(0, 10);
        rga.insert_at(1, 20);
        rga.insert_at(2, 30);
        rga.remove(1);

        let elems: Vec<&i32> = rga.iter().collect();
        assert_eq!(elems, vec![&10, &30]);
    }

    #[test]
    fn actor_returns_id() {
        let rga = Rga::<i32>::new("node-42");
        assert_eq!(rga.actor(), "node-42");
    }

    // --- Merge tests ---

    #[test]
    fn merge_disjoint_inserts() {
        let mut r1 = Rga::new("a");
        r1.insert_at(0, 'x');

        let mut r2 = Rga::new("b");
        r2.insert_at(0, 'y');

        r1.merge(&r2);
        assert_eq!(r1.len(), 2);
        // Both elements present
        let v = r1.to_vec();
        assert!(v.contains(&'x'));
        assert!(v.contains(&'y'));
    }

    #[test]
    fn merge_concurrent_inserts_at_same_position() {
        // Both replicas start empty and insert at position 0.
        let mut r1 = Rga::new("a");
        r1.insert_at(0, 'A');

        let mut r2 = Rga::new("b");
        r2.insert_at(0, 'B');

        let mut r1_copy = r1.clone();
        let mut r2_copy = r2.clone();

        r1_copy.merge(&r2);
        r2_copy.merge(&r1);

        // Both replicas must converge to the same order.
        assert_eq!(r1_copy.to_vec(), r2_copy.to_vec());
        assert_eq!(r1_copy.len(), 2);
    }

    #[test]
    fn merge_concurrent_inserts_after_shared_prefix() {
        // Both replicas share a prefix and then insert at the same position.
        let mut r1 = Rga::new("a");
        r1.insert_at(0, 'H');
        r1.insert_at(1, 'e');

        let mut r2 = r1.clone();
        // Change the actor for r2 so it generates distinct ids.
        r2.actor = "b".to_string();

        // r1 inserts 'X' after 'e'
        r1.insert_at(2, 'X');
        // r2 inserts 'Y' after 'e'
        r2.insert_at(2, 'Y');

        let mut r1_merged = r1.clone();
        r1_merged.merge(&r2);

        let mut r2_merged = r2.clone();
        r2_merged.merge(&r1);

        assert_eq!(r1_merged.to_vec(), r2_merged.to_vec());
        assert_eq!(r1_merged.len(), 4);

        // Prefix is preserved.
        assert_eq!(r1_merged.get(0), Some(&'H'));
        assert_eq!(r1_merged.get(1), Some(&'e'));
    }

    #[test]
    fn merge_with_deletions() {
        let mut r1 = Rga::new("a");
        r1.insert_at(0, 'a');
        r1.insert_at(1, 'b');
        r1.insert_at(2, 'c');

        let mut r2 = r1.clone();
        r2.actor = "b".to_string();

        // r1 removes 'b'
        r1.remove(1);
        // r2 inserts 'd' at end
        r2.insert_at(3, 'd');

        r1.merge(&r2);
        // 'b' should be tombstoned, 'd' should be added
        assert!(!r1.to_vec().contains(&'b'));
        assert!(r1.to_vec().contains(&'d'));
        assert_eq!(r1.len(), 3); // a, c, d
    }

    #[test]
    fn merge_is_commutative() {
        let mut r1 = Rga::new("a");
        r1.insert_at(0, 1);
        r1.insert_at(1, 2);

        let mut r2 = Rga::new("b");
        r2.insert_at(0, 3);
        r2.insert_at(1, 4);

        let mut left = r1.clone();
        left.merge(&r2);

        let mut right = r2.clone();
        right.merge(&r1);

        assert_eq!(left.to_vec(), right.to_vec());
    }

    #[test]
    fn merge_commutativity_with_deletions() {
        let mut r1 = Rga::new("a");
        r1.insert_at(0, 'x');
        r1.insert_at(1, 'y');

        let mut r2 = r1.clone();
        r2.actor = "b".to_string();

        r1.remove(0); // remove 'x'
        r2.insert_at(2, 'z');

        let mut left = r1.clone();
        left.merge(&r2);

        let mut right = r2.clone();
        right.merge(&r1);

        assert_eq!(left.to_vec(), right.to_vec());
    }

    #[test]
    fn merge_is_associative() {
        let mut r1 = Rga::new("a");
        r1.insert_at(0, 'A');

        let mut r2 = Rga::new("b");
        r2.insert_at(0, 'B');

        let mut r3 = Rga::new("c");
        r3.insert_at(0, 'C');

        // (r1 merge r2) merge r3
        let mut left = r1.clone();
        left.merge(&r2);
        left.merge(&r3);

        // r1 merge (r2 merge r3)
        let mut r2_r3 = r2.clone();
        r2_r3.merge(&r3);
        let mut right = r1.clone();
        right.merge(&r2_r3);

        assert_eq!(left.to_vec(), right.to_vec());
    }

    #[test]
    fn merge_is_idempotent() {
        let mut r1 = Rga::new("a");
        r1.insert_at(0, 'x');
        r1.insert_at(1, 'y');

        let mut r2 = Rga::new("b");
        r2.insert_at(0, 'z');

        r1.merge(&r2);
        let after_first = r1.clone();

        r1.merge(&r2);
        assert_eq!(r1.to_vec(), after_first.to_vec());
        assert_eq!(r1, after_first);
    }

    #[test]
    fn merge_self_is_idempotent() {
        let mut rga = Rga::new("a");
        rga.insert_at(0, 1);
        rga.insert_at(1, 2);
        rga.remove(0);

        let snapshot = rga.clone();
        rga.merge(&snapshot);

        assert_eq!(rga, snapshot);
    }

    #[test]
    fn causal_ordering_preserved() {
        // Build a sequence on one replica, then merge into another.
        let mut r1 = Rga::new("a");
        r1.insert_at(0, 'H');
        r1.insert_at(1, 'e');
        r1.insert_at(2, 'l');
        r1.insert_at(3, 'l');
        r1.insert_at(4, 'o');

        let mut r2 = Rga::new("b");
        r2.merge(&r1);

        assert_eq!(r2.to_vec(), vec!['H', 'e', 'l', 'l', 'o']);
    }

    #[test]
    fn causal_ordering_insert_between() {
        let mut rga = Rga::new("a");
        rga.insert_at(0, 1);
        rga.insert_at(1, 3);
        rga.insert_at(1, 2); // insert 2 between 1 and 3

        assert_eq!(rga.to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn three_way_merge_convergence() {
        // Three replicas each insert at position 0 concurrently.
        let mut r1 = Rga::new("a");
        r1.insert_at(0, 'A');

        let mut r2 = Rga::new("b");
        r2.insert_at(0, 'B');

        let mut r3 = Rga::new("c");
        r3.insert_at(0, 'C');

        let mut m1 = r1.clone();
        m1.merge(&r2);
        m1.merge(&r3);

        let mut m2 = r2.clone();
        m2.merge(&r1);
        m2.merge(&r3);

        let mut m3 = r3.clone();
        m3.merge(&r1);
        m3.merge(&r2);

        assert_eq!(m1.to_vec(), m2.to_vec());
        assert_eq!(m2.to_vec(), m3.to_vec());
        assert_eq!(m1.len(), 3);
    }

    #[test]
    fn concurrent_delete_same_element() {
        let mut r1 = Rga::new("a");
        r1.insert_at(0, 'x');

        let mut r2 = r1.clone();
        r2.actor = "b".to_string();

        // Both replicas delete the same element.
        r1.remove(0);
        r2.remove(0);

        r1.merge(&r2);
        assert!(r1.is_empty());
    }

    #[test]
    fn merge_preserves_existing_order() {
        let mut r1 = Rga::new("a");
        r1.insert_at(0, 1);
        r1.insert_at(1, 2);
        r1.insert_at(2, 3);
        r1.insert_at(3, 4);

        let snapshot = r1.to_vec();

        let mut r2 = Rga::new("b");
        r2.insert_at(0, 10);

        r1.merge(&r2);

        // Original elements should still appear in their original relative order.
        let merged = r1.to_vec();
        let original_positions: Vec<usize> = snapshot
            .iter()
            .map(|v| merged.iter().position(|x| x == v).unwrap())
            .collect();

        // The original order should be strictly increasing.
        for w in original_positions.windows(2) {
            assert!(w[0] < w[1]);
        }
    }

    #[test]
    fn empty_merge_empty() {
        let mut r1 = Rga::<i32>::new("a");
        let r2 = Rga::<i32>::new("b");
        r1.merge(&r2);
        assert!(r1.is_empty());
    }

    #[test]
    fn merge_into_empty() {
        let mut r1 = Rga::<char>::new("a");
        let mut r2 = Rga::new("b");
        r2.insert_at(0, 'z');

        r1.merge(&r2);
        assert_eq!(r1.to_vec(), vec!['z']);
    }

    #[test]
    fn repeated_insert_remove_cycles() {
        let mut rga = Rga::new("a");
        for i in 0..5 {
            rga.insert_at(0, i);
        }
        // rga is [4, 3, 2, 1, 0]
        assert_eq!(rga.len(), 5);

        // Remove all
        while !rga.is_empty() {
            rga.remove(0);
        }
        assert!(rga.is_empty());

        // Re-insert
        rga.insert_at(0, 99);
        assert_eq!(rga.to_vec(), vec![99]);
    }
}
