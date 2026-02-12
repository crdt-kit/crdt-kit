use crate::{Crdt, GCounter};

/// A positive-negative counter (PN-Counter).
///
/// Supports both increment and decrement operations by maintaining two
/// internal G-Counters: one for increments and one for decrements.
/// The value is `increments - decrements`.
///
/// # Example
///
/// ```
/// use crdt_kit::prelude::*;
///
/// let mut c1 = PNCounter::new("node-1");
/// c1.increment();
/// c1.increment();
/// c1.decrement();
/// assert_eq!(c1.value(), 1);
///
/// let mut c2 = PNCounter::new("node-2");
/// c2.decrement();
///
/// c1.merge(&c2);
/// assert_eq!(c1.value(), 0);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PNCounter {
    increments: GCounter,
    decrements: GCounter,
}

impl PNCounter {
    /// Create a new PN-Counter for the given actor/replica ID.
    pub fn new(actor: impl Into<String>) -> Self {
        let actor = actor.into();
        Self {
            increments: GCounter::new(actor.clone()),
            decrements: GCounter::new(actor),
        }
    }

    /// Increment the counter by 1.
    pub fn increment(&mut self) {
        self.increments.increment();
    }

    /// Decrement the counter by 1.
    pub fn decrement(&mut self) {
        self.decrements.increment();
    }

    /// Get the current counter value (increments - decrements).
    #[must_use]
    pub fn value(&self) -> i64 {
        self.increments.value() as i64 - self.decrements.value() as i64
    }
}

impl Crdt for PNCounter {
    fn merge(&mut self, other: &Self) {
        self.increments.merge(&other.increments);
        self.decrements.merge(&other.decrements);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_counter_is_zero() {
        let c = PNCounter::new("a");
        assert_eq!(c.value(), 0);
    }

    #[test]
    fn increment_and_decrement() {
        let mut c = PNCounter::new("a");
        c.increment();
        c.increment();
        c.decrement();
        assert_eq!(c.value(), 1);
    }

    #[test]
    fn can_go_negative() {
        let mut c = PNCounter::new("a");
        c.decrement();
        c.decrement();
        assert_eq!(c.value(), -2);
    }

    #[test]
    fn merge_different_actors() {
        let mut c1 = PNCounter::new("a");
        c1.increment();
        c1.increment();

        let mut c2 = PNCounter::new("b");
        c2.decrement();

        c1.merge(&c2);
        assert_eq!(c1.value(), 1); // 2 - 1
    }

    #[test]
    fn merge_is_commutative() {
        let mut c1 = PNCounter::new("a");
        c1.increment();

        let mut c2 = PNCounter::new("b");
        c2.decrement();
        c2.decrement();

        let mut left = c1.clone();
        left.merge(&c2);

        let mut right = c2.clone();
        right.merge(&c1);

        assert_eq!(left.value(), right.value());
    }

    #[test]
    fn merge_is_idempotent() {
        let mut c1 = PNCounter::new("a");
        c1.increment();

        let mut c2 = PNCounter::new("b");
        c2.decrement();

        c1.merge(&c2);
        let after_first = c1.clone();
        c1.merge(&c2);

        assert_eq!(c1, after_first);
    }
}
