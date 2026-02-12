use std::collections::BTreeMap;

use crate::Crdt;

/// A multi-value register (MV-Register).
///
/// Unlike LWW-Register, this preserves all concurrently written values.
/// When concurrent writes occur, all values are kept until a subsequent
/// write supersedes them. This is useful when you want to detect conflicts
/// rather than silently resolving them.
///
/// # Example
///
/// ```
/// use crdt_kit::prelude::*;
///
/// let mut r1 = MVRegister::new("node-1");
/// r1.set("alice");
///
/// let mut r2 = MVRegister::new("node-2");
/// r2.set("bob");
///
/// r1.merge(&r2);
/// // Both values are preserved as concurrent writes
/// let values = r1.values();
/// assert!(values.contains(&&"alice"));
/// assert!(values.contains(&&"bob"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MVRegister<T: Clone + Ord> {
    actor: String,
    /// Version vector: actor -> counter
    version: BTreeMap<String, u64>,
    /// Each entry: (value, version_at_write)
    entries: Vec<(T, BTreeMap<String, u64>)>,
}

impl<T: Clone + Ord> MVRegister<T> {
    /// Create a new empty MV-Register for the given actor.
    pub fn new(actor: impl Into<String>) -> Self {
        Self {
            actor: actor.into(),
            version: BTreeMap::new(),
            entries: Vec::new(),
        }
    }

    /// Set a new value, superseding all current values.
    pub fn set(&mut self, value: T) {
        let counter = self.version.entry(self.actor.clone()).or_insert(0);
        *counter += 1;

        self.entries.clear();
        self.entries.push((value, self.version.clone()));
    }

    /// Get all current values.
    ///
    /// Returns a single value during normal operation, or multiple values
    /// when concurrent writes have been merged without a subsequent write.
    #[must_use]
    pub fn values(&self) -> Vec<&T> {
        let mut vals: Vec<&T> = self.entries.iter().map(|(v, _)| v).collect();
        vals.sort();
        vals.dedup();
        vals
    }

    /// Returns `true` if there are concurrent (conflicting) values.
    #[must_use]
    pub fn is_conflicted(&self) -> bool {
        self.entries.len() > 1
    }

    /// Get this replica's actor ID.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }
}

/// Check if version `a` dominates (is strictly greater than or equal to) version `b`.
fn dominates(a: &BTreeMap<String, u64>, b: &BTreeMap<String, u64>) -> bool {
    for (actor, &count) in b {
        if a.get(actor).copied().unwrap_or(0) < count {
            return false;
        }
    }
    true
}

impl<T: Clone + Ord> Crdt for MVRegister<T> {
    fn merge(&mut self, other: &Self) {
        // Save self's version before merging for correct dominance checks.
        let self_version = self.version.clone();

        let mut new_entries = Vec::new();

        // Keep entries from self that are either:
        // - not dominated by other's version (concurrent or newer), OR
        // - also present in other's entries (both sides still hold it)
        for entry in &self.entries {
            if !dominates(&other.version, &entry.1)
                || other.entries.iter().any(|e| e.1 == entry.1)
            {
                new_entries.push(entry.clone());
            }
        }

        // Keep entries from other that are not dominated by self's original
        // version, and avoid duplicates already added from self.
        for entry in &other.entries {
            if !dominates(&self_version, &entry.1)
                && !new_entries.iter().any(|e| e.1 == entry.1)
            {
                new_entries.push(entry.clone());
            }
        }

        // Merge version vectors (take max for each actor)
        for (actor, &count) in &other.version {
            let entry = self.version.entry(actor.clone()).or_insert(0);
            *entry = (*entry).max(count);
        }

        self.entries = new_entries;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_register_is_empty() {
        let r = MVRegister::<String>::new("a");
        assert!(r.values().is_empty());
        assert!(!r.is_conflicted());
    }

    #[test]
    fn set_replaces_value() {
        let mut r = MVRegister::new("a");
        r.set("hello");
        assert_eq!(r.values(), vec![&"hello"]);

        r.set("world");
        assert_eq!(r.values(), vec![&"world"]);
        assert!(!r.is_conflicted());
    }

    #[test]
    fn concurrent_writes_preserved() {
        let mut r1 = MVRegister::new("a");
        r1.set("alice");

        let mut r2 = MVRegister::new("b");
        r2.set("bob");

        r1.merge(&r2);
        let vals = r1.values();
        assert_eq!(vals.len(), 2);
        assert!(vals.contains(&&"alice"));
        assert!(vals.contains(&&"bob"));
        assert!(r1.is_conflicted());
    }

    #[test]
    fn subsequent_write_resolves_conflict() {
        let mut r1 = MVRegister::new("a");
        r1.set("alice");

        let mut r2 = MVRegister::new("b");
        r2.set("bob");

        r1.merge(&r2);
        assert!(r1.is_conflicted());

        // New write after merge supersedes both
        r1.set("resolved");
        assert_eq!(r1.values(), vec![&"resolved"]);
        assert!(!r1.is_conflicted());
    }

    #[test]
    fn merge_is_commutative() {
        let mut r1 = MVRegister::new("a");
        r1.set("x");

        let mut r2 = MVRegister::new("b");
        r2.set("y");

        let mut left = r1.clone();
        left.merge(&r2);

        let mut right = r2.clone();
        right.merge(&r1);

        let mut lv = left.values();
        lv.sort();
        let mut rv = right.values();
        rv.sort();
        assert_eq!(lv, rv);
    }

    #[test]
    fn merge_is_idempotent() {
        let mut r1 = MVRegister::new("a");
        r1.set("x");

        let mut r2 = MVRegister::new("b");
        r2.set("y");

        r1.merge(&r2);
        let after_first = r1.clone();
        r1.merge(&r2);

        assert_eq!(r1, after_first);
    }

    #[test]
    fn causal_write_supersedes() {
        let mut r1 = MVRegister::new("a");
        r1.set("first");

        let mut r2 = r1.clone();
        // r2 saw r1's write, so its write causally supersedes
        r2.set("second");

        r1.merge(&r2);
        assert_eq!(r1.values(), vec![&"second"]);
        assert!(!r1.is_conflicted());
    }
}
