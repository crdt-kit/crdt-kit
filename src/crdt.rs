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
