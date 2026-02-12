/// Core trait that all CRDTs must implement.
///
/// A CRDT (Conflict-free Replicated Data Type) guarantees that concurrent
/// updates on different replicas will converge to the same state after merging,
/// without requiring coordination.
///
/// # Properties
///
/// All implementations must satisfy:
/// - **Commutativity:** `a.merge(b) == b.merge(a)`
/// - **Associativity:** `a.merge(b.merge(c)) == a.merge(b).merge(c)`
/// - **Idempotency:** `a.merge(a) == a`
pub trait Crdt {
    /// Merge another replica's state into this one.
    ///
    /// After merging, `self` contains the least upper bound of both states.
    /// This operation is commutative, associative, and idempotent.
    fn merge(&mut self, other: &Self);
}

/// Extension trait for delta-state CRDTs.
///
/// Delta-state CRDTs can produce compact deltas representing only the
/// changes between two states. This enables efficient synchronization:
/// instead of transferring the full state, replicas exchange small deltas.
///
/// # Example
///
/// ```
/// use crdt_kit::prelude::*;
///
/// let mut c1 = GCounter::new("a");
/// c1.increment();
/// c1.increment();
///
/// let mut c2 = GCounter::new("b");
/// c2.increment();
///
/// // Generate a delta from c1 that c2 doesn't have
/// let delta = c1.delta(&c2);
///
/// // Apply just the delta instead of full state merge
/// c2.apply_delta(&delta);
/// assert_eq!(c2.value(), 3); // both counts included
/// ```
pub trait DeltaCrdt: Crdt {
    /// The type of delta produced by this CRDT.
    type Delta;

    /// Generate a delta containing changes in `self` that `other` does not have.
    ///
    /// The returned delta is the minimal set of information needed to bring
    /// a replica at state `other` up to date with `self`.
    fn delta(&self, other: &Self) -> Self::Delta;

    /// Apply a delta to this replica's state.
    ///
    /// This is equivalent to merging the state that produced the delta,
    /// but typically much more efficient in terms of data transferred.
    fn apply_delta(&mut self, delta: &Self::Delta);
}
