use alloc::collections::BTreeSet;

use crate::Crdt;

/// A two-phase set (2P-Set).
///
/// Elements can be added and removed, but once removed, they cannot be
/// re-added. This is implemented with two G-Sets: one for additions
/// and one for removals (tombstones).
///
/// # Example
///
/// ```
/// use crdt_kit::prelude::*;
///
/// let mut s1 = TwoPSet::new();
/// s1.insert("apple");
/// s1.insert("banana");
/// s1.remove(&"banana");
///
/// assert!(s1.contains(&"apple"));
/// assert!(!s1.contains(&"banana")); // removed
///
/// let mut s2 = TwoPSet::new();
/// s2.insert("banana"); // trying to re-add on another replica
///
/// s1.merge(&s2);
/// assert!(!s1.contains(&"banana")); // still removed (tombstone wins)
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TwoPSet<T: Ord + Clone> {
    added: BTreeSet<T>,
    removed: BTreeSet<T>,
}

impl<T: Ord + Clone> TwoPSet<T> {
    /// Create a new empty 2P-Set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            added: BTreeSet::new(),
            removed: BTreeSet::new(),
        }
    }

    /// Insert an element.
    ///
    /// Returns `true` if the element was newly added (not previously
    /// removed). If the element was already removed, it cannot be re-added
    /// and this returns `false`.
    pub fn insert(&mut self, value: T) -> bool {
        if self.removed.contains(&value) {
            return false;
        }
        self.added.insert(value)
    }

    /// Remove an element.
    ///
    /// The element must have been added first. Once removed, it can never
    /// be re-added. Returns `true` if the element was present and is now removed.
    pub fn remove(&mut self, value: &T) -> bool {
        if self.added.contains(value) && !self.removed.contains(value) {
            self.removed.insert(value.clone());
            true
        } else {
            false
        }
    }

    /// Check if the set contains an element (added and not removed).
    #[must_use]
    pub fn contains(&self, value: &T) -> bool {
        self.added.contains(value) && !self.removed.contains(value)
    }

    /// Get the number of active elements (added minus removed).
    #[must_use]
    pub fn len(&self) -> usize {
        self.added.difference(&self.removed).count()
    }

    /// Check if the set has no active elements.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over active elements (added and not removed).
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.added.difference(&self.removed)
    }
}

impl<T: Ord + Clone> Default for TwoPSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord + Clone> Crdt for TwoPSet<T> {
    fn merge(&mut self, other: &Self) {
        for elem in &other.added {
            self.added.insert(elem.clone());
        }
        for elem in &other.removed {
            self.removed.insert(elem.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_set_is_empty() {
        let s = TwoPSet::<String>::new();
        assert!(s.is_empty());
    }

    #[test]
    fn insert_and_contains() {
        let mut s = TwoPSet::new();
        s.insert("a");
        assert!(s.contains(&"a"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn remove_element() {
        let mut s = TwoPSet::new();
        s.insert("a");
        assert!(s.remove(&"a"));
        assert!(!s.contains(&"a"));
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn cannot_readd_removed_element() {
        let mut s = TwoPSet::new();
        s.insert("a");
        s.remove(&"a");
        assert!(!s.insert("a")); // returns false
        assert!(!s.contains(&"a"));
    }

    #[test]
    fn remove_wins_on_merge() {
        let mut s1 = TwoPSet::new();
        s1.insert("a");
        s1.remove(&"a");

        let mut s2 = TwoPSet::new();
        s2.insert("a"); // concurrent add

        s1.merge(&s2);
        assert!(!s1.contains(&"a")); // remove wins
    }

    #[test]
    fn merge_is_commutative() {
        let mut s1 = TwoPSet::new();
        s1.insert("a");
        s1.insert("b");
        s1.remove(&"a");

        let mut s2 = TwoPSet::new();
        s2.insert("b");
        s2.insert("c");

        let mut left = s1.clone();
        left.merge(&s2);

        let mut right = s2.clone();
        right.merge(&s1);

        assert_eq!(left, right);
    }

    #[test]
    fn merge_is_idempotent() {
        let mut s1 = TwoPSet::new();
        s1.insert("a");

        let mut s2 = TwoPSet::new();
        s2.insert("b");

        s1.merge(&s2);
        let after_first = s1.clone();
        s1.merge(&s2);

        assert_eq!(s1, after_first);
    }

    #[test]
    fn iterate_active_elements() {
        let mut s = TwoPSet::new();
        s.insert(1);
        s.insert(2);
        s.insert(3);
        s.remove(&2);

        let active: Vec<&i32> = s.iter().collect();
        assert_eq!(active, vec![&1, &3]);
    }
}
