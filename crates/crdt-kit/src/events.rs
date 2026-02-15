//! Operation-based CRDT trait for event sourcing.
//!
//! Op-based CRDTs produce **operations** (events) that can be:
//! - Persisted as an append-only event log
//! - Broadcast to other replicas for synchronization
//! - Replayed to reconstruct state
//!
//! This trait bridges the gap between CRDTs and event sourcing:
//! each CRDT operation IS a domain event.

use crate::Crdt;

/// A CRDT that can express its mutations as discrete operations.
///
/// Unlike state-based CRDTs (which merge full state), op-based CRDTs
/// exchange individual operations. Each operation is an event that
/// can be persisted, broadcast, and replayed.
///
/// # Relationship to Event Sourcing
///
/// ```text
/// Traditional Event Sourcing:
///   Command → Validate → Event → Persist → Apply → State
///
/// CRDT Event Sourcing (this trait):
///   Operation → apply_op() → State
///   (No validation needed — CRDTs guarantee convergence)
/// ```
///
/// # Example
///
/// ```
/// use crdt_kit::events::OpCrdt;
/// use crdt_kit::prelude::*;
/// // OpCrdt implementors produce typed operations
/// // that can be serialized and stored as events.
/// ```
pub trait OpCrdt: Crdt {
    /// The operation type this CRDT produces.
    ///
    /// Operations must be serializable (for persistence and network transfer)
    /// and deterministically applicable (for convergence).
    type Op;

    /// Apply an operation to the current state.
    ///
    /// This is the core function: it takes an operation (from local or remote)
    /// and updates the CRDT state. For convergence, `apply_op` must be:
    /// - **Commutative**: order of operations doesn't matter
    /// - **Idempotent**: applying the same op twice has no additional effect
    fn apply_op(&mut self, op: &Self::Op);
}
