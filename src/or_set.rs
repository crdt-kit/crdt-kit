use std::collections::{BTreeMap, BTreeSet};

use crate::Crdt;

/// An observed-remove set (OR-Set), also known as an add-wins set.
///
/// Unlike the 2P-Set, elements can be freely added and removed, and
/// re-added after removal. Each add operation generates a unique tag.
/// Remove only removes the tags that the remover has observed, so
/// concurrent adds are preserved.
///
/// # Example
///
/// ```
/// use crdt_kit::prelude::*;
///
/// let mut s1 = ORSet::new("node-1");
/// s1.insert("apple");
/// s1.insert("banana");
/// s1.remove(&"banana");
///
/// let mut s2 = ORSet::new("node-2");
/// s2.insert("banana"); // concurrent add
///
/// s1.merge(&s2);
/// // banana is present because s2's add was concurrent with s1's remove
/// assert!(s1.contains(&"banana"));
/// assert!(s1.contains(&"apple"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ORSet<T: Ord + Clone> {
    actor: String,
    counter: u64,
    /// element -> set of unique tags (actor, counter)
    elements: BTreeMap<T, BTreeSet<(String, u64)>>,
    /// Tombstones: tags that have been removed
    tombstones: BTreeSet<(String, u64)>,
}

impl<T: Ord + Clone> ORSet<T> {
    /// Create a new empty OR-Set for the given actor.
    pub fn new(actor: impl Into<String>) -> Self {
        Self {
            actor: actor.into(),
            counter: 0,
            elements: BTreeMap::new(),
            tombstones: BTreeSet::new(),
        }
    }

    /// Insert an element into the set.
    ///
    /// Generates a unique tag for this insertion. Even if the element
    /// was previously removed, this new tag allows it to be re-added.
    pub fn insert(&mut self, value: T) {
        self.counter += 1;
        let tag = (self.actor.clone(), self.counter);
        self.elements.entry(value).or_default().insert(tag);
    }

    /// Remove an element from the set.
    ///
    /// Only removes the tags that this replica has observed. Concurrent
    /// adds on other replicas will survive the merge.
    ///
    /// Returns `true` if the element was present and removed.
    pub fn remove(&mut self, value: &T) -> bool {
        if let Some(tags) = self.elements.remove(value) {
            self.tombstones.extend(tags);
            true
        } else {
            false
        }
    }

    /// Check if the set contains an element.
    #[must_use]
    pub fn contains(&self, value: &T) -> bool {
        self.elements
            .get(value)
            .is_some_and(|tags| !tags.is_empty())
    }

    /// Get the number of distinct elements in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.elements
            .values()
            .filter(|tags| !tags.is_empty())
            .count()
    }

    /// Check if the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over the elements in the set.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.elements
            .iter()
            .filter(|(_, tags)| !tags.is_empty())
            .map(|(v, _)| v)
    }

    /// Get this replica's actor ID.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }
}

impl<T: Ord + Clone> Crdt for ORSet<T> {
    fn merge(&mut self, other: &Self) {
        // Merge all elements and their tags
        for (value, other_tags) in &other.elements {
            let self_tags = self.elements.entry(value.clone()).or_default();
            for tag in other_tags {
                // Only add tag if it's not in our tombstones
                if !self.tombstones.contains(tag) {
                    self_tags.insert(tag.clone());
                }
            }
        }

        // Apply other's tombstones to our elements
        for tag in &other.tombstones {
            for tags in self.elements.values_mut() {
                tags.remove(tag);
            }
        }

        // Merge tombstones
        self.tombstones.extend(other.tombstones.iter().cloned());

        // Clean up empty tag sets
        self.elements.retain(|_, tags| !tags.is_empty());

        // Update counter to be at least as high as the other
        self.counter = self.counter.max(other.counter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_set_is_empty() {
        let s = ORSet::<String>::new("a");
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn insert_and_contains() {
        let mut s = ORSet::new("a");
        s.insert("x");
        assert!(s.contains(&"x"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn remove_element() {
        let mut s = ORSet::new("a");
        s.insert("x");
        assert!(s.remove(&"x"));
        assert!(!s.contains(&"x"));
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn can_readd_after_remove() {
        let mut s = ORSet::new("a");
        s.insert("x");
        s.remove(&"x");
        assert!(!s.contains(&"x"));

        s.insert("x");
        assert!(s.contains(&"x"));
    }

    #[test]
    fn concurrent_add_survives_remove() {
        let mut s1 = ORSet::new("a");
        s1.insert("x");

        // s1 removes x
        s1.remove(&"x");

        // s2 concurrently adds x (new unique tag from different replica)
        let mut s2 = ORSet::new("b");
        s2.insert("x");

        s1.merge(&s2);
        // s2's add should survive because s1 didn't observe that tag
        assert!(s1.contains(&"x"));
    }

    #[test]
    fn merge_is_commutative() {
        let mut s1 = ORSet::new("a");
        s1.insert("x");
        s1.insert("y");

        let mut s2 = ORSet::new("b");
        s2.insert("y");
        s2.insert("z");

        let mut left = s1.clone();
        left.merge(&s2);

        let mut right = s2.clone();
        right.merge(&s1);

        let left_elems: BTreeSet<_> = left.iter().collect();
        let right_elems: BTreeSet<_> = right.iter().collect();
        assert_eq!(left_elems, right_elems);
    }

    #[test]
    fn merge_is_idempotent() {
        let mut s1 = ORSet::new("a");
        s1.insert("x");

        let mut s2 = ORSet::new("b");
        s2.insert("y");

        s1.merge(&s2);
        let after_first = s1.clone();
        s1.merge(&s2);

        assert_eq!(s1, after_first);
    }

    #[test]
    fn add_wins_semantics() {
        // Simulate: s1 has "x" and removes it, s2 adds "x" concurrently
        let mut s1 = ORSet::new("a");
        s1.insert("x");
        s1.remove(&"x");

        // Different node adds "x" concurrently (new unique tag)
        let mut s2 = ORSet::new("b");
        s2.insert("x");

        s1.merge(&s2);
        // Add wins: "x" should be present because of s2_new's concurrent add
        assert!(s1.contains(&"x"));
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut s = ORSet::<&str>::new("a");
        assert!(!s.remove(&"x"));
    }

    #[test]
    fn iterate_elements() {
        let mut s = ORSet::new("a");
        s.insert(1);
        s.insert(2);
        s.insert(3);
        s.remove(&2);

        let elems: Vec<&i32> = s.iter().collect();
        assert_eq!(elems, vec![&1, &3]);
    }
}
