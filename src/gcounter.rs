use std::collections::BTreeMap;

use crate::Crdt;

/// A grow-only counter (G-Counter).
///
/// Each replica maintains its own count. The total value is the sum of all
/// replica counts. This counter can only be incremented, never decremented.
///
/// # Example
///
/// ```
/// use crdt_kit::prelude::*;
///
/// let mut c1 = GCounter::new("node-1");
/// c1.increment();
/// c1.increment();
///
/// let mut c2 = GCounter::new("node-2");
/// c2.increment();
///
/// c1.merge(&c2);
/// assert_eq!(c1.value(), 3);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GCounter {
    actor: String,
    counts: BTreeMap<String, u64>,
}

impl GCounter {
    /// Create a new G-Counter for the given actor/replica ID.
    pub fn new(actor: impl Into<String>) -> Self {
        Self {
            actor: actor.into(),
            counts: BTreeMap::new(),
        }
    }

    /// Increment this replica's count by 1.
    pub fn increment(&mut self) {
        *self.counts.entry(self.actor.clone()).or_insert(0) += 1;
    }

    /// Increment this replica's count by `n`.
    pub fn increment_by(&mut self, n: u64) {
        *self.counts.entry(self.actor.clone()).or_insert(0) += n;
    }

    /// Get the total counter value across all replicas.
    #[must_use]
    pub fn value(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Get this replica's actor ID.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }

    /// Get the count for a specific actor.
    #[must_use]
    pub fn count_for(&self, actor: &str) -> u64 {
        self.counts.get(actor).copied().unwrap_or(0)
    }
}

impl Crdt for GCounter {
    fn merge(&mut self, other: &Self) {
        for (actor, &count) in &other.counts {
            let entry = self.counts.entry(actor.clone()).or_insert(0);
            *entry = (*entry).max(count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_counter_is_zero() {
        let c = GCounter::new("a");
        assert_eq!(c.value(), 0);
    }

    #[test]
    fn increment_increases_value() {
        let mut c = GCounter::new("a");
        c.increment();
        assert_eq!(c.value(), 1);
        c.increment();
        assert_eq!(c.value(), 2);
    }

    #[test]
    fn increment_by() {
        let mut c = GCounter::new("a");
        c.increment_by(5);
        assert_eq!(c.value(), 5);
    }

    #[test]
    fn merge_takes_max() {
        let mut c1 = GCounter::new("a");
        c1.increment();
        c1.increment();

        let mut c2 = GCounter::new("a");
        c2.increment();

        // c1 has a=2, c2 has a=1, merge should keep a=2
        c1.merge(&c2);
        assert_eq!(c1.value(), 2);
    }

    #[test]
    fn merge_different_actors() {
        let mut c1 = GCounter::new("a");
        c1.increment();

        let mut c2 = GCounter::new("b");
        c2.increment();
        c2.increment();

        c1.merge(&c2);
        assert_eq!(c1.value(), 3);
    }

    #[test]
    fn merge_is_commutative() {
        let mut c1 = GCounter::new("a");
        c1.increment();

        let mut c2 = GCounter::new("b");
        c2.increment();
        c2.increment();

        let mut left = c1.clone();
        left.merge(&c2);

        let mut right = c2.clone();
        right.merge(&c1);

        assert_eq!(left.value(), right.value());
    }

    #[test]
    fn merge_is_idempotent() {
        let mut c1 = GCounter::new("a");
        c1.increment();

        let mut c2 = GCounter::new("b");
        c2.increment();

        c1.merge(&c2);
        let after_first = c1.clone();
        c1.merge(&c2);

        assert_eq!(c1, after_first);
    }

    #[test]
    fn count_for_actor() {
        let mut c = GCounter::new("a");
        c.increment();
        c.increment();
        assert_eq!(c.count_for("a"), 2);
        assert_eq!(c.count_for("b"), 0);
    }
}
