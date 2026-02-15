use alloc::collections::BTreeSet;

use crate::Crdt;

/// A grow-only set (G-Set).
///
/// Elements can be added but never removed. Merge is simply the union
/// of both sets. This is the simplest set CRDT.
///
/// # Example
///
/// ```
/// use crdt_kit::prelude::*;
///
/// let mut s1 = GSet::new();
/// s1.insert("apple");
/// s1.insert("banana");
///
/// let mut s2 = GSet::new();
/// s2.insert("cherry");
///
/// s1.merge(&s2);
/// assert_eq!(s1.len(), 3);
/// assert!(s1.contains(&"cherry"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GSet<T: Ord + Clone> {
    elements: BTreeSet<T>,
}

impl<T: Ord + Clone> GSet<T> {
    /// Create a new empty G-Set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            elements: BTreeSet::new(),
        }
    }

    /// Insert an element into the set.
    ///
    /// Returns `true` if the element was newly inserted.
    pub fn insert(&mut self, value: T) -> bool {
        self.elements.insert(value)
    }

    /// Check if the set contains an element.
    #[must_use]
    pub fn contains(&self, value: &T) -> bool {
        self.elements.contains(value)
    }

    /// Get the number of elements in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Check if the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Iterate over the elements in the set.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.elements.iter()
    }
}

impl<T: Ord + Clone> Default for GSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord + Clone> Crdt for GSet<T> {
    fn merge(&mut self, other: &Self) {
        for elem in &other.elements {
            self.elements.insert(elem.clone());
        }
    }
}

impl<T: Ord + Clone> IntoIterator for GSet<T> {
    type Item = T;
    type IntoIter = alloc::collections::btree_set::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.elements.into_iter()
    }
}

impl<T: Ord + Clone> FromIterator<T> for GSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self {
            elements: BTreeSet::from_iter(iter),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_set_is_empty() {
        let s = GSet::<String>::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn insert_and_contains() {
        let mut s = GSet::new();
        assert!(s.insert("a"));
        assert!(s.contains(&"a"));
        assert!(!s.contains(&"b"));
    }

    #[test]
    fn insert_duplicate_returns_false() {
        let mut s = GSet::new();
        assert!(s.insert("a"));
        assert!(!s.insert("a"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn merge_is_union() {
        let mut s1 = GSet::new();
        s1.insert(1);
        s1.insert(2);

        let mut s2 = GSet::new();
        s2.insert(2);
        s2.insert(3);

        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.contains(&1));
        assert!(s1.contains(&2));
        assert!(s1.contains(&3));
    }

    #[test]
    fn merge_is_commutative() {
        let mut s1 = GSet::new();
        s1.insert("a");

        let mut s2 = GSet::new();
        s2.insert("b");

        let mut left = s1.clone();
        left.merge(&s2);

        let mut right = s2.clone();
        right.merge(&s1);

        assert_eq!(left, right);
    }

    #[test]
    fn merge_is_idempotent() {
        let mut s1 = GSet::new();
        s1.insert(1);

        let mut s2 = GSet::new();
        s2.insert(2);

        s1.merge(&s2);
        let after_first = s1.clone();
        s1.merge(&s2);

        assert_eq!(s1, after_first);
    }

    #[test]
    fn from_iterator() {
        let s: GSet<i32> = vec![1, 2, 3].into_iter().collect();
        assert_eq!(s.len(), 3);
    }
}
