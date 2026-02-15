use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// A stored event from the event log.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct StoredEvent {
    /// Monotonically increasing sequence number within an entity.
    pub sequence: u64,
    /// The entity this event belongs to.
    pub entity_id: String,
    /// The namespace (table) this entity belongs to.
    pub namespace: String,
    /// Serialized event payload.
    pub data: Vec<u8>,
    /// Hybrid Logical Clock timestamp.
    pub timestamp: u64,
    /// Node that generated this event.
    pub node_id: String,
}

/// A persisted snapshot for fast recovery.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct Snapshot {
    /// Serialized CRDT state.
    pub state: Vec<u8>,
    /// The event sequence number this snapshot was taken at.
    pub at_sequence: u64,
    /// Schema version of the serialized state.
    pub version: u8,
}

/// Summary information about a stored namespace.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct NamespaceInfo {
    /// Name of the namespace.
    pub name: String,
    /// Number of distinct entities.
    pub entity_count: u64,
    /// Total number of events across all entities.
    pub event_count: u64,
    /// Number of snapshots.
    pub snapshot_count: u64,
}

/// Summary information about the entire database.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct DbInfo {
    /// Total number of entities across all namespaces.
    pub total_entities: u64,
    /// Total number of events.
    pub total_events: u64,
    /// Total number of snapshots.
    pub total_snapshots: u64,
    /// Per-namespace breakdown.
    pub namespaces: Vec<NamespaceInfo>,
}

/// Core trait for state persistence (snapshots).
///
/// Every backend implements this trait. It provides simple key-value
/// operations scoped by a namespace (analogous to a table).
///
/// Data is stored as opaque bytes — the store does not interpret
/// the CRDT structure. Versioning and migration are handled by
/// [`crdt-migrate`](https://docs.rs/crdt-migrate).
pub trait StateStore {
    /// Error type for this backend.
    type Error: fmt::Debug + fmt::Display;

    /// Store a value under `(namespace, key)`.
    fn put(&mut self, namespace: &str, key: &str, value: &[u8]) -> Result<(), Self::Error>;

    /// Retrieve a value by `(namespace, key)`.
    /// Returns `None` if the key does not exist.
    fn get(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>, Self::Error>;

    /// Delete a value by `(namespace, key)`.
    fn delete(&mut self, namespace: &str, key: &str) -> Result<(), Self::Error>;

    /// List all keys in a namespace.
    fn list_keys(&self, namespace: &str) -> Result<Vec<String>, Self::Error>;

    /// Check if a key exists in a namespace.
    fn exists(&self, namespace: &str, key: &str) -> Result<bool, Self::Error> {
        Ok(self.get(namespace, key)?.is_some())
    }
}

/// Extension trait for backends that support append-only event logs.
///
/// The event store enables event sourcing: each CRDT operation is
/// persisted as an immutable event. State can be reconstructed by
/// replaying events from the last snapshot.
pub trait EventStore: StateStore {
    /// Append an event to the log for a given entity.
    /// Returns the assigned sequence number.
    fn append_event(
        &mut self,
        namespace: &str,
        entity_id: &str,
        data: &[u8],
        timestamp: u64,
        node_id: &str,
    ) -> Result<u64, Self::Error>;

    /// Read events for an entity since a given sequence number (exclusive).
    fn events_since(
        &self,
        namespace: &str,
        entity_id: &str,
        since_sequence: u64,
    ) -> Result<Vec<StoredEvent>, Self::Error>;

    /// Count total events for an entity (useful for compaction decisions).
    fn event_count(&self, namespace: &str, entity_id: &str) -> Result<u64, Self::Error>;

    /// Save a snapshot of an entity's state at a given event sequence.
    fn save_snapshot(
        &mut self,
        namespace: &str,
        entity_id: &str,
        state: &[u8],
        at_sequence: u64,
        version: u8,
    ) -> Result<(), Self::Error>;

    /// Load the latest snapshot for an entity.
    fn load_snapshot(
        &self,
        namespace: &str,
        entity_id: &str,
    ) -> Result<Option<Snapshot>, Self::Error>;

    /// Delete events before a given sequence (for compaction).
    /// The snapshot at `before_sequence` must exist first.
    fn truncate_events_before(
        &mut self,
        namespace: &str,
        entity_id: &str,
        before_sequence: u64,
    ) -> Result<u64, Self::Error>;
}

/// Extension trait for backends that support atomic transactions.
pub trait Transactional: StateStore {
    /// Execute a closure within an atomic transaction.
    /// If the closure returns `Err`, all changes are rolled back.
    fn transaction<F, R>(&mut self, f: F) -> Result<R, Self::Error>
    where
        F: FnOnce(&mut Self) -> Result<R, Self::Error>;
}

/// Extension trait for efficient batch operations.
pub trait BatchOps: StateStore {
    /// Store multiple key-value pairs atomically.
    fn put_batch(&mut self, namespace: &str, entries: &[(&str, &[u8])]) -> Result<(), Self::Error>;
}
