use alloc::string::String;

use crate::Crdt;

/// A last-writer-wins register (LWW-Register).
///
/// Resolves concurrent writes by keeping the value with the highest timestamp.
/// Ties are broken by comparing actor IDs lexicographically.
///
/// # Example
///
/// ```
/// use crdt_kit::prelude::*;
///
/// let mut r1 = LWWRegister::new("node-1", "hello");
/// let mut r2 = LWWRegister::new("node-2", "world");
///
/// // The register with the later timestamp wins
/// r1.merge(&r2);
/// // Value is either "hello" or "world" depending on timestamps
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LWWRegister<T: Clone> {
    actor: String,
    value: T,
    timestamp: u64,
}

impl<T: Clone + PartialEq> PartialEq for LWWRegister<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.timestamp == other.timestamp
    }
}

impl<T: Clone + Eq> Eq for LWWRegister<T> {}

impl<T: Clone> LWWRegister<T> {
    /// Create a new LWW-Register with an initial value.
    ///
    /// The timestamp is automatically set to the current system time.
    ///
    /// This method requires the `std` feature. In `no_std` environments, use
    /// [`LWWRegister::with_timestamp`] instead.
    #[cfg(feature = "std")]
    pub fn new(actor: impl Into<String>, value: T) -> Self {
        Self {
            actor: actor.into(),
            value,
            timestamp: now(),
        }
    }

    /// Create a new LWW-Register with an explicit timestamp.
    ///
    /// Useful for testing or when you need deterministic behavior.
    /// This is the only constructor available in `no_std` environments.
    pub fn with_timestamp(actor: impl Into<String>, value: T, timestamp: u64) -> Self {
        Self {
            actor: actor.into(),
            value,
            timestamp,
        }
    }

    /// Update the register's value.
    ///
    /// The timestamp is automatically set to the current system time.
    ///
    /// This method requires the `std` feature. In `no_std` environments, use
    /// [`LWWRegister::set_with_timestamp`] instead.
    #[cfg(feature = "std")]
    pub fn set(&mut self, value: T) {
        self.value = value;
        self.timestamp = now();
    }

    /// Update the register's value with an explicit timestamp.
    pub fn set_with_timestamp(&mut self, value: T, timestamp: u64) {
        if timestamp >= self.timestamp {
            self.value = value;
            self.timestamp = timestamp;
        }
    }

    /// Get the current value.
    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Get the current timestamp.
    #[must_use]
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Get this replica's actor ID.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }
}

impl<T: Clone> Crdt for LWWRegister<T> {
    fn merge(&mut self, other: &Self) {
        if other.timestamp > self.timestamp
            || (other.timestamp == self.timestamp && other.actor > self.actor)
        {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
        }
    }
}

#[cfg(feature = "std")]
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_register_holds_value() {
        let r = LWWRegister::with_timestamp("a", 42, 1);
        assert_eq!(*r.value(), 42);
    }

    #[test]
    fn set_updates_value() {
        let mut r = LWWRegister::with_timestamp("a", 1, 1);
        r.set_with_timestamp(2, 2);
        assert_eq!(*r.value(), 2);
    }

    #[test]
    fn merge_keeps_later_timestamp() {
        let mut r1 = LWWRegister::with_timestamp("a", "old", 1);
        let r2 = LWWRegister::with_timestamp("b", "new", 2);

        r1.merge(&r2);
        assert_eq!(*r1.value(), "new");
    }

    #[test]
    fn merge_keeps_self_if_later() {
        let mut r1 = LWWRegister::with_timestamp("a", "new", 2);
        let r2 = LWWRegister::with_timestamp("b", "old", 1);

        r1.merge(&r2);
        assert_eq!(*r1.value(), "new");
    }

    #[test]
    fn merge_breaks_tie_by_actor() {
        let mut r1 = LWWRegister::with_timestamp("a", "first", 1);
        let r2 = LWWRegister::with_timestamp("b", "second", 1);

        r1.merge(&r2);
        // "b" > "a", so r2 wins the tie
        assert_eq!(*r1.value(), "second");
    }

    #[test]
    fn merge_is_idempotent() {
        let mut r1 = LWWRegister::with_timestamp("a", "x", 1);
        let r2 = LWWRegister::with_timestamp("b", "y", 2);

        r1.merge(&r2);
        let after_first = r1.clone();
        r1.merge(&r2);

        assert_eq!(r1, after_first);
    }
}
