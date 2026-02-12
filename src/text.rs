use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::Crdt;

/// A collaborative text CRDT based on RGA (Replicated Growable Array) principles.
///
/// Each character is assigned a unique identifier `(actor, counter)` and is
/// stored in an internal sequence. Deletions use tombstones: characters are
/// marked as deleted but remain in the internal list so that concurrent
/// operations from different replicas can be merged deterministically.
///
/// Ordering of concurrent inserts at the same position is resolved by
/// comparing `(counter, actor)` tuples — higher counters come first, and
/// ties are broken lexicographically by actor ID.
///
/// # Example
///
/// ```
/// use crdt_kit::prelude::*;
///
/// let mut t1 = TextCrdt::new("alice");
/// t1.insert_str(0, "hello");
///
/// let mut t2 = TextCrdt::new("bob");
/// t2.insert_str(0, "world");
///
/// t1.merge(&t2);
/// t2.merge(&t1);
///
/// // Both replicas converge to the same text.
/// assert_eq!(t1.to_string(), t2.to_string());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextCrdt {
    actor: String,
    counter: u64,
    /// The ordered sequence of elements (including tombstones).
    elements: Vec<Element>,
    /// Tracks the maximum counter observed per actor, used during merge to
    /// avoid re-inserting elements that are already present.
    version: BTreeMap<String, u64>,
}

/// A single element in the text sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Element {
    /// Unique identifier: (actor, counter).
    id: (String, u64),
    /// The character value.
    value: char,
    /// Whether this element has been tombstoned (logically deleted).
    deleted: bool,
}

impl TextCrdt {
    /// Create a new empty text CRDT for the given actor.
    pub fn new(actor: impl Into<String>) -> Self {
        Self {
            actor: actor.into(),
            counter: 0,
            elements: Vec::new(),
            version: BTreeMap::new(),
        }
    }

    /// Create a fork of this replica with a different actor ID.
    ///
    /// The returned replica contains an identical copy of the current content
    /// and version state, but subsequent inserts will use the new actor,
    /// preventing ID collisions between the two replicas.
    pub fn fork(&self, new_actor: impl Into<String>) -> Self {
        Self {
            actor: new_actor.into(),
            counter: self.counter,
            elements: self.elements.clone(),
            version: self.version.clone(),
        }
    }

    /// Insert a character at the given visible index.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than `self.len()`.
    pub fn insert(&mut self, index: usize, ch: char) {
        assert!(
            index <= self.len(),
            "index {index} out of bounds for text of length {}",
            self.len()
        );

        self.counter += 1;
        let id = (self.actor.clone(), self.counter);
        self.version
            .entry(self.actor.clone())
            .and_modify(|c| *c = (*c).max(self.counter))
            .or_insert(self.counter);

        let elem = Element {
            id,
            value: ch,
            deleted: false,
        };

        let raw_index = self.raw_index_for_insert(index);
        self.elements.insert(raw_index, elem);
    }

    /// Insert a string at the given visible index.
    ///
    /// Characters are inserted left-to-right so that the resulting visible
    /// text contains the string starting at `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than `self.len()`.
    pub fn insert_str(&mut self, index: usize, s: &str) {
        assert!(
            index <= self.len(),
            "index {index} out of bounds for text of length {}",
            self.len()
        );

        for (i, ch) in s.chars().enumerate() {
            self.insert(index + i, ch);
        }
    }

    /// Remove (tombstone) the character at the given visible index.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than or equal to `self.len()`.
    pub fn remove(&mut self, index: usize) {
        assert!(
            index < self.len(),
            "index {index} out of bounds for text of length {}",
            self.len()
        );

        let raw = self.visible_to_raw(index);
        self.elements[raw].deleted = true;
    }

    /// Remove a range of characters starting at `start` with the given `len`.
    ///
    /// # Panics
    ///
    /// Panics if `start + len` is greater than `self.len()`.
    pub fn remove_range(&mut self, start: usize, len: usize) {
        assert!(
            start + len <= self.len(),
            "range {}..{} out of bounds for text of length {}",
            start,
            start + len,
            self.len()
        );

        // Remove from right to left so that indices remain valid.
        for i in (0..len).rev() {
            self.remove(start + i);
        }
    }

    /// Return the number of visible (non-deleted) characters.
    #[must_use]
    pub fn len(&self) -> usize {
        self.elements.iter().filter(|e| !e.deleted).count()
    }

    /// Check whether the visible text is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get this replica's actor ID.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }

    // ---- internal helpers ----

    /// Convert a visible index to a raw index in `self.elements`.
    ///
    /// Returns the raw index of the `n`-th visible element.
    fn visible_to_raw(&self, visible: usize) -> usize {
        let mut seen = 0;
        for (raw, elem) in self.elements.iter().enumerate() {
            if !elem.deleted {
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

    /// Determine the raw position at which to insert a new element so that it
    /// appears at the given visible index.
    ///
    /// If `index == self.len()` (append), the new element goes at the end.
    /// Otherwise it is placed just before the element currently at `index`.
    fn raw_index_for_insert(&self, visible_index: usize) -> usize {
        if visible_index == 0 {
            return 0;
        }

        let visible_count = self.len();
        if visible_index >= visible_count {
            // For append, go past all existing elements (including trailing
            // tombstones that belong after the last visible character).
            return self.elements.len();
        }

        // Insert just before the element that is currently at visible_index.
        self.visible_to_raw(visible_index)
    }

    /// Find the raw index of an element with the given `id`, or `None`.
    fn find_by_id(&self, id: &(String, u64)) -> Option<usize> {
        self.elements.iter().position(|e| &e.id == id)
    }

    /// Determine where an element from a remote replica should be inserted
    /// based on RGA ordering.
    ///
    /// We scan from `start` looking for the correct position. Among
    /// consecutive elements with higher or equal `(counter, actor)` we skip
    /// forward; the new element is placed before the first element that is
    /// strictly less.
    fn find_insert_position(&self, elem: &Element, after_raw: Option<usize>) -> usize {
        let start = match after_raw {
            Some(idx) => idx + 1,
            None => 0,
        };

        let new_key = (elem.id.1, &elem.id.0); // (counter, actor)

        for i in start..self.elements.len() {
            let existing = &self.elements[i];
            let existing_key = (existing.id.1, &existing.id.0);
            // The new element goes before any element that has a strictly
            // smaller ordering key. We use reverse ordering: larger counter
            // first, then larger actor first, so the new element is inserted
            // *before* the first element whose key is strictly less.
            if existing_key < new_key {
                return i;
            }
        }

        self.elements.len()
    }

    /// Find the raw index in `self` of the causal predecessor of `elem` from
    /// `other`. The predecessor is the element that directly precedes `elem`
    /// in `other.elements`.
    fn find_predecessor_raw(&self, other: &Self, elem: &Element) -> Option<usize> {
        // Find the position of `elem` inside `other`.
        let other_pos = other.elements.iter().position(|e| e.id == elem.id)?;

        if other_pos == 0 {
            return None;
        }

        // Walk backwards in `other` to find the first predecessor that exists
        // in `self`.
        for i in (0..other_pos).rev() {
            let pred_id = &other.elements[i].id;
            if let Some(raw) = self.find_by_id(pred_id) {
                return Some(raw);
            }
        }

        None
    }
}

impl Crdt for TextCrdt {
    fn merge(&mut self, other: &Self) {
        // We integrate the remote elements one by one, in the order they
        // appear in `other.elements`. For each remote element we either:
        //   - update the tombstone flag if the element already exists locally,
        //   - or insert it at the correct RGA position if it is new.

        for other_elem in &other.elements {
            if let Some(raw) = self.find_by_id(&other_elem.id) {
                // Element already present — propagate tombstones (delete wins).
                if other_elem.deleted {
                    self.elements[raw].deleted = true;
                }
            } else {
                // New element — figure out where to place it.
                //
                // In RGA the element is positioned *after* its causal
                // predecessor (the element that was directly before it in the
                // originating replica). We approximate this by looking at the
                // element that precedes `other_elem` in `other.elements`.
                let predecessor_raw = self.find_predecessor_raw(other, other_elem);
                let pos = self.find_insert_position(other_elem, predecessor_raw);
                self.elements.insert(pos, other_elem.clone());
            }
        }

        // Merge version vectors.
        for (actor, &cnt) in &other.version {
            let entry = self.version.entry(actor.clone()).or_insert(0);
            *entry = (*entry).max(cnt);
        }

        // Advance local counter past everything we have seen.
        if let Some(&max_cnt) = self.version.values().max() {
            self.counter = self.counter.max(max_cnt);
        }
    }
}

impl core::fmt::Display for TextCrdt {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for elem in &self.elements {
            if !elem.deleted {
                write!(f, "{}", elem.value)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_text_is_empty() {
        let t = TextCrdt::new("a");
        assert!(t.is_empty());
        assert_eq!(t.len(), 0);
        assert_eq!(t.to_string(), "");
    }

    #[test]
    fn insert_single_char() {
        let mut t = TextCrdt::new("a");
        t.insert(0, 'x');
        assert_eq!(t.to_string(), "x");
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn insert_at_beginning_middle_end() {
        let mut t = TextCrdt::new("a");
        t.insert(0, 'a'); // "a"
        t.insert(1, 'c'); // "ac"
        t.insert(1, 'b'); // "abc"
        t.insert(0, 'z'); // "zabc"
        assert_eq!(t.to_string(), "zabc");
    }

    #[test]
    fn delete_char() {
        let mut t = TextCrdt::new("a");
        t.insert_str(0, "hello");
        assert_eq!(t.to_string(), "hello");

        t.remove(1); // remove 'e'
        assert_eq!(t.to_string(), "hllo");
        assert_eq!(t.len(), 4);
    }

    #[test]
    fn insert_str_basic() {
        let mut t = TextCrdt::new("a");
        t.insert_str(0, "hello");
        assert_eq!(t.to_string(), "hello");
        assert_eq!(t.len(), 5);
    }

    #[test]
    fn insert_str_at_middle() {
        let mut t = TextCrdt::new("a");
        t.insert_str(0, "hd");
        t.insert_str(1, "ello worl");
        assert_eq!(t.to_string(), "hello world");
    }

    #[test]
    fn remove_range_basic() {
        let mut t = TextCrdt::new("a");
        t.insert_str(0, "hello world");
        t.remove_range(5, 6); // remove " world"
        assert_eq!(t.to_string(), "hello");
    }

    #[test]
    fn remove_range_from_start() {
        let mut t = TextCrdt::new("a");
        t.insert_str(0, "hello");
        t.remove_range(0, 3); // remove "hel"
        assert_eq!(t.to_string(), "lo");
    }

    #[test]
    fn remove_all() {
        let mut t = TextCrdt::new("a");
        t.insert_str(0, "abc");
        t.remove_range(0, 3);
        assert!(t.is_empty());
        assert_eq!(t.to_string(), "");
    }

    #[test]
    fn merge_disjoint_inserts() {
        let mut t1 = TextCrdt::new("alice");
        t1.insert_str(0, "hello");

        let mut t2 = TextCrdt::new("bob");
        t2.insert_str(0, "world");

        t1.merge(&t2);

        // Both sets of characters should be present.
        let result = t1.to_string();
        assert!(result.contains("hello") || result.contains("world"));
        assert_eq!(t1.len(), 10);
    }

    #[test]
    fn merge_propagates_tombstones() {
        let mut t1 = TextCrdt::new("alice");
        t1.insert_str(0, "abc");

        // Fork with a different actor to simulate a second replica that
        // received the same state. Only deletes here, so no new IDs needed,
        // but fork is still the safe pattern.
        let mut t2 = t1.fork("bob");
        t2.remove(1); // delete 'b' on t2

        t1.merge(&t2);
        assert_eq!(t1.to_string(), "ac");
    }

    #[test]
    fn merge_commutativity() {
        let mut t1 = TextCrdt::new("alice");
        t1.insert_str(0, "hello");

        let mut t2 = TextCrdt::new("bob");
        t2.insert_str(0, "world");

        let mut left = t1.clone();
        left.merge(&t2);

        let mut right = t2.clone();
        right.merge(&t1);

        assert_eq!(left.to_string(), right.to_string());
    }

    #[test]
    fn merge_idempotency() {
        let mut t1 = TextCrdt::new("alice");
        t1.insert_str(0, "hello");

        let mut t2 = TextCrdt::new("bob");
        t2.insert_str(0, "world");

        t1.merge(&t2);
        let after_first = t1.clone();
        t1.merge(&t2);

        assert_eq!(t1.to_string(), after_first.to_string());
        assert_eq!(t1.len(), after_first.len());
    }

    #[test]
    fn concurrent_inserts_at_same_position() {
        // Both replicas start empty and insert at position 0.
        let mut t1 = TextCrdt::new("alice");
        t1.insert(0, 'a');

        let mut t2 = TextCrdt::new("bob");
        t2.insert(0, 'b');

        let mut left = t1.clone();
        left.merge(&t2);

        let mut right = t2.clone();
        right.merge(&t1);

        // The order must be deterministic and identical on both replicas.
        assert_eq!(left.to_string(), right.to_string());
        assert_eq!(left.len(), 2);
        // Both characters must be present.
        let s = left.to_string();
        assert!(s.contains('a'));
        assert!(s.contains('b'));
    }

    #[test]
    fn concurrent_inserts_at_same_position_in_existing_text() {
        // Both replicas share the same base text and insert at the same index.
        let mut t1 = TextCrdt::new("alice");
        t1.insert_str(0, "ac");

        // Fork to a different actor so new inserts get unique IDs.
        let mut t2 = t1.fork("bob");

        // Both insert at position 1 (between 'a' and 'c').
        t1.insert(1, 'X');
        t2.insert(1, 'Y');

        let mut left = t1.clone();
        left.merge(&t2);

        let mut right = t2.clone();
        right.merge(&t1);

        assert_eq!(left.to_string(), right.to_string());
        let s = left.to_string();
        assert!(s.starts_with('a'));
        assert!(s.ends_with('c'));
        assert!(s.contains('X'));
        assert!(s.contains('Y'));
    }

    #[test]
    fn concurrent_insert_and_delete() {
        let mut t1 = TextCrdt::new("alice");
        t1.insert_str(0, "abc");

        let mut t2 = t1.fork("bob");

        // alice deletes 'b'
        t1.remove(1);
        // bob inserts 'X' at position 1
        t2.insert(1, 'X');

        let mut left = t1.clone();
        left.merge(&t2);

        let mut right = t2.clone();
        right.merge(&t1);

        // Both should converge.
        assert_eq!(left.to_string(), right.to_string());
        // 'b' should be deleted but 'X' should be present.
        let s = left.to_string();
        assert!(!s.contains('b'));
        assert!(s.contains('X'));
        assert!(s.contains('a'));
        assert!(s.contains('c'));
    }

    #[test]
    fn merge_after_local_edits_on_both_sides() {
        let mut t1 = TextCrdt::new("alice");
        t1.insert_str(0, "hello");

        let mut t2 = t1.fork("bob");

        // alice appends " world"
        t1.insert_str(5, " world");
        // bob deletes "llo" and inserts "p"
        t2.remove_range(2, 3);
        t2.insert(2, 'p');

        let mut left = t1.clone();
        left.merge(&t2);

        let mut right = t2.clone();
        right.merge(&t1);

        assert_eq!(left.to_string(), right.to_string());
    }

    #[test]
    fn display_trait() {
        let mut t = TextCrdt::new("a");
        t.insert_str(0, "hello");
        assert_eq!(format!("{t}"), "hello");
    }

    #[test]
    fn actor_getter() {
        let t = TextCrdt::new("my-node");
        assert_eq!(t.actor(), "my-node");
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn insert_out_of_bounds_panics() {
        let mut t = TextCrdt::new("a");
        t.insert(1, 'x'); // only index 0 is valid for empty text
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn remove_out_of_bounds_panics() {
        let mut t = TextCrdt::new("a");
        t.remove(0);
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn remove_range_out_of_bounds_panics() {
        let mut t = TextCrdt::new("a");
        t.insert_str(0, "abc");
        t.remove_range(1, 5);
    }

    #[test]
    fn triple_merge_convergence() {
        let mut t1 = TextCrdt::new("alice");
        t1.insert_str(0, "base");

        let mut t2 = t1.fork("bob");
        let mut t3 = t1.fork("carol");

        t1.insert(4, '!');
        t2.insert(0, '>');
        t3.remove(2); // remove 's'

        // Merge in different orders and verify convergence.
        let mut r1 = t1.clone();
        r1.merge(&t2);
        r1.merge(&t3);

        let mut r2 = t2.clone();
        r2.merge(&t3);
        r2.merge(&t1);

        let mut r3 = t3.clone();
        r3.merge(&t1);
        r3.merge(&t2);

        assert_eq!(r1.to_string(), r2.to_string());
        assert_eq!(r2.to_string(), r3.to_string());
    }
}
