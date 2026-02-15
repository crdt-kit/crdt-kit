//! High-level database API for CRDT persistence.
//!
//! `CrdtDb` wraps a storage backend with automatic versioning and migration.
//! It serializes CRDTs with a version envelope so that schema changes are
//! handled transparently on read.
//!
//! # Example
//!
//! ```
//! use crdt_store::{CrdtDb, CrdtVersioned, MemoryStore};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Debug, PartialEq, Serialize, Deserialize)]
//! struct Sensor { temperature: f32 }
//!
//! impl CrdtVersioned for Sensor {
//!     const SCHEMA_VERSION: u8 = 1;
//! }
//!
//! let mut db = CrdtDb::with_store(MemoryStore::new());
//! db.save("s1", &Sensor { temperature: 22.5 }).unwrap();
//!
//! let loaded: Option<Sensor> = db.load("s1").unwrap();
//! assert_eq!(loaded.unwrap().temperature, 22.5);
//! ```

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use crdt_migrate::{MigrationConfig, MigrationEngine, MigrationStep, VersionedEnvelope};
use serde::{de::DeserializeOwned, Serialize};

use crate::traits::{EventStore, Snapshot, StateStore, StoredEvent};

/// The default namespace used when none is specified.
const DEFAULT_NAMESPACE: &str = "default";

/// Error type for `CrdtDb` operations.
#[derive(Debug)]
pub enum DbError<E: fmt::Debug + fmt::Display> {
    /// Error from the underlying storage backend.
    Store(E),
    /// Serialization failed.
    Serialize(String),
    /// Deserialization failed.
    Deserialize(String),
    /// Version envelope error.
    Envelope(String),
    /// Migration error.
    Migration(String),
}

impl<E: fmt::Debug + fmt::Display> fmt::Display for DbError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(e) => write!(f, "store error: {e}"),
            Self::Serialize(msg) => write!(f, "serialization error: {msg}"),
            Self::Deserialize(msg) => write!(f, "deserialization error: {msg}"),
            Self::Envelope(msg) => write!(f, "envelope error: {msg}"),
            Self::Migration(msg) => write!(f, "migration error: {msg}"),
        }
    }
}

/// High-level CRDT database with automatic versioning and migration.
///
/// Wraps any [`StateStore`] backend. When data is saved, it is serialized
/// with postcard and wrapped in a [`VersionedEnvelope`]. When loaded, the
/// version is checked and migrations run automatically if needed.
pub struct CrdtDb<S: StateStore> {
    store: S,
    migration_engine: MigrationEngine,
    config: CrdtDbConfig,
}

/// Configuration for `CrdtDb`.
#[derive(Debug, Clone)]
pub struct CrdtDbConfig {
    /// Migration configuration.
    pub migration: MigrationConfig,
    /// Default namespace for save/load without explicit namespace.
    pub default_namespace: String,
}

impl Default for CrdtDbConfig {
    fn default() -> Self {
        Self {
            migration: MigrationConfig::default(),
            default_namespace: DEFAULT_NAMESPACE.to_string(),
        }
    }
}

/// Builder for constructing a `CrdtDb` with custom configuration.
pub struct CrdtDbBuilder<S: StateStore> {
    store: S,
    migration_engine: MigrationEngine,
    config: CrdtDbConfig,
}

impl<S: StateStore> CrdtDbBuilder<S> {
    /// Set the migration configuration.
    pub fn migration_config(mut self, config: MigrationConfig) -> Self {
        self.config.migration = config;
        self
    }

    /// Set the default namespace.
    pub fn default_namespace(mut self, ns: &str) -> Self {
        self.config.default_namespace = ns.to_string();
        self
    }

    /// Register a migration step.
    pub fn register_migration(mut self, step: alloc::boxed::Box<dyn MigrationStep>) -> Self {
        self.migration_engine.register(step);
        self
    }

    /// Build the `CrdtDb`.
    pub fn build(self) -> CrdtDb<S> {
        CrdtDb {
            store: self.store,
            migration_engine: self.migration_engine,
            config: self.config,
        }
    }
}

impl<S: StateStore> CrdtDb<S> {
    /// Create a `CrdtDb` wrapping the given store with default config.
    ///
    /// The current schema version defaults to 1 (no migrations registered).
    pub fn with_store(store: S) -> Self {
        Self {
            store,
            migration_engine: MigrationEngine::new(1),
            config: CrdtDbConfig::default(),
        }
    }

    /// Create a builder for advanced configuration.
    ///
    /// `current_version` is the schema version your app currently writes.
    pub fn builder(store: S, current_version: u32) -> CrdtDbBuilder<S> {
        CrdtDbBuilder {
            store,
            migration_engine: MigrationEngine::new(current_version),
            config: CrdtDbConfig::default(),
        }
    }

    /// Get a reference to the underlying store.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Get a mutable reference to the underlying store.
    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    /// Get the migration engine.
    pub fn migration_engine(&self) -> &MigrationEngine {
        &self.migration_engine
    }

    /// Save a serializable value in the default namespace.
    ///
    /// The value is serialized with postcard and wrapped in a version envelope.
    pub fn save<T: Serialize + CrdtVersioned>(
        &mut self,
        key: &str,
        value: &T,
    ) -> Result<(), DbError<S::Error>> {
        let ns = self.config.default_namespace.clone();
        self.save_ns(&ns, key, value)
    }

    /// Save a serializable value in a specific namespace.
    pub fn save_ns<T: Serialize + CrdtVersioned>(
        &mut self,
        namespace: &str,
        key: &str,
        value: &T,
    ) -> Result<(), DbError<S::Error>> {
        let payload =
            postcard::to_allocvec(value).map_err(|e| DbError::Serialize(e.to_string()))?;

        let envelope =
            VersionedEnvelope::new(T::SCHEMA_VERSION, crdt_migrate::CrdtType::Custom, payload);

        self.store
            .put(namespace, key, &envelope.to_bytes())
            .map_err(DbError::Store)
    }

    /// Load a deserializable value from the default namespace.
    ///
    /// If the stored data has an older version, migrations are applied
    /// automatically before deserialization.
    pub fn load<T: DeserializeOwned + CrdtVersioned>(
        &mut self,
        key: &str,
    ) -> Result<Option<T>, DbError<S::Error>> {
        let ns = self.config.default_namespace.clone();
        self.load_ns(&ns, key)
    }

    /// Load a deserializable value from a specific namespace.
    pub fn load_ns<T: DeserializeOwned + CrdtVersioned>(
        &mut self,
        namespace: &str,
        key: &str,
    ) -> Result<Option<T>, DbError<S::Error>> {
        let raw = self.store.get(namespace, key).map_err(DbError::Store)?;

        let raw = match raw {
            Some(data) => data,
            None => return Ok(None),
        };

        let payload = if VersionedEnvelope::is_versioned(&raw) {
            let envelope = VersionedEnvelope::from_bytes(&raw)
                .map_err(|e| DbError::Envelope(e.to_string()))?;

            let stored_version = envelope.version as u32;
            let current_version = T::SCHEMA_VERSION as u32;

            if stored_version != current_version
                && self.migration_engine.needs_migration(stored_version)
            {
                let migrated = self
                    .migration_engine
                    .migrate_to_current(&envelope.payload, stored_version)
                    .map_err(|e| DbError::Migration(e.to_string()))?;

                // Write back migrated data if configured
                if self.config.migration.write_back_on_read {
                    let new_envelope = VersionedEnvelope::new(
                        T::SCHEMA_VERSION,
                        envelope.crdt_type,
                        migrated.clone(),
                    );
                    self.store
                        .put(namespace, key, &new_envelope.to_bytes())
                        .map_err(DbError::Store)?;
                }

                migrated
            } else {
                envelope.payload
            }
        } else {
            // Raw data without envelope — treat as current version
            raw
        };

        let value: T =
            postcard::from_bytes(&payload).map_err(|e| DbError::Deserialize(e.to_string()))?;
        Ok(Some(value))
    }

    /// Delete a value from the default namespace.
    pub fn delete(&mut self, key: &str) -> Result<(), DbError<S::Error>> {
        let ns = self.config.default_namespace.clone();
        self.delete_ns(&ns, key)
    }

    /// Delete a value from a specific namespace.
    pub fn delete_ns(&mut self, namespace: &str, key: &str) -> Result<(), DbError<S::Error>> {
        self.store.delete(namespace, key).map_err(DbError::Store)
    }

    /// List all keys in the default namespace.
    pub fn list_keys(&self) -> Result<Vec<String>, DbError<S::Error>> {
        self.store
            .list_keys(&self.config.default_namespace)
            .map_err(DbError::Store)
    }

    /// List all keys in a specific namespace.
    pub fn list_keys_ns(&self, namespace: &str) -> Result<Vec<String>, DbError<S::Error>> {
        self.store.list_keys(namespace).map_err(DbError::Store)
    }

    /// Check if a key exists in the default namespace.
    pub fn exists(&self, key: &str) -> Result<bool, DbError<S::Error>> {
        self.store
            .exists(&self.config.default_namespace, key)
            .map_err(DbError::Store)
    }

    /// Check if a key exists in a specific namespace.
    pub fn exists_ns(&self, namespace: &str, key: &str) -> Result<bool, DbError<S::Error>> {
        self.store.exists(namespace, key).map_err(DbError::Store)
    }
}

/// Event sourcing methods — available when the backend supports event logs.
impl<S: EventStore> CrdtDb<S> {
    /// Append a serializable event to the log.
    ///
    /// Returns the assigned sequence number.
    pub fn append_event<T: Serialize>(
        &mut self,
        namespace: &str,
        entity_id: &str,
        event: &T,
        timestamp: u64,
        node_id: &str,
    ) -> Result<u64, DbError<S::Error>> {
        let data = postcard::to_allocvec(event).map_err(|e| DbError::Serialize(e.to_string()))?;

        self.store
            .append_event(namespace, entity_id, &data, timestamp, node_id)
            .map_err(DbError::Store)
    }

    /// Read events since a given sequence number.
    pub fn events_since(
        &self,
        namespace: &str,
        entity_id: &str,
        since_sequence: u64,
    ) -> Result<Vec<StoredEvent>, DbError<S::Error>> {
        self.store
            .events_since(namespace, entity_id, since_sequence)
            .map_err(DbError::Store)
    }

    /// Get the event count for an entity.
    pub fn event_count(&self, namespace: &str, entity_id: &str) -> Result<u64, DbError<S::Error>> {
        self.store
            .event_count(namespace, entity_id)
            .map_err(DbError::Store)
    }

    /// Save a snapshot for an entity.
    pub fn save_snapshot(
        &mut self,
        namespace: &str,
        entity_id: &str,
        state: &[u8],
        at_sequence: u64,
        version: u8,
    ) -> Result<(), DbError<S::Error>> {
        self.store
            .save_snapshot(namespace, entity_id, state, at_sequence, version)
            .map_err(DbError::Store)
    }

    /// Load the latest snapshot for an entity.
    pub fn load_snapshot(
        &self,
        namespace: &str,
        entity_id: &str,
    ) -> Result<Option<Snapshot>, DbError<S::Error>> {
        self.store
            .load_snapshot(namespace, entity_id)
            .map_err(DbError::Store)
    }

    /// Compact an entity: save snapshot + truncate old events.
    ///
    /// `state` is the current serialized state. Events before the latest
    /// sequence number will be removed.
    pub fn compact(
        &mut self,
        namespace: &str,
        entity_id: &str,
        state: &[u8],
        version: u8,
    ) -> Result<u64, DbError<S::Error>> {
        // Get current max sequence
        let events = self
            .store
            .events_since(namespace, entity_id, 0)
            .map_err(DbError::Store)?;

        let max_seq = events.last().map(|e| e.sequence).unwrap_or(0);

        if max_seq == 0 {
            return Ok(0);
        }

        // Save snapshot at current sequence
        self.store
            .save_snapshot(namespace, entity_id, state, max_seq, version)
            .map_err(DbError::Store)?;

        // Truncate old events (keep the latest one as boundary)
        self.store
            .truncate_events_before(namespace, entity_id, max_seq)
            .map_err(DbError::Store)
    }
}

/// Marker trait for types that carry schema version information.
///
/// Implement this for your CRDT types to enable versioned save/load.
/// The version is embedded in the stored data for automatic migration.
///
/// # Example
///
/// ```
/// use crdt_store::CrdtVersioned;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct SensorReading {
///     temperature: f32,
/// }
///
/// impl CrdtVersioned for SensorReading {
///     const SCHEMA_VERSION: u8 = 1;
/// }
/// ```
pub trait CrdtVersioned {
    /// The current schema version for this type.
    const SCHEMA_VERSION: u8;
}

// Blanket impl for crdt-kit types that implement Versioned
impl<T: crdt_kit::Versioned> CrdtVersioned for T {
    const SCHEMA_VERSION: u8 = T::CURRENT_VERSION;
}

/// Helper to deserialize an event payload.
pub fn deserialize_event<T: DeserializeOwned>(event: &StoredEvent) -> Result<T, String> {
    postcard::from_bytes(&event.data).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryStore;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct SensorV1 {
        temperature: f32,
    }

    impl CrdtVersioned for SensorV1 {
        const SCHEMA_VERSION: u8 = 1;
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct SensorV2 {
        temperature: f32,
        humidity: Option<f32>,
    }

    impl CrdtVersioned for SensorV2 {
        const SCHEMA_VERSION: u8 = 2;
    }

    #[test]
    fn save_and_load_basic() {
        let mut db = CrdtDb::with_store(MemoryStore::new());

        let sensor = SensorV1 { temperature: 22.5 };
        db.save("s1", &sensor).unwrap();

        let loaded: Option<SensorV1> = db.load("s1").unwrap();
        assert_eq!(loaded, Some(sensor));
    }

    #[test]
    fn load_nonexistent_returns_none() {
        let mut db = CrdtDb::with_store(MemoryStore::new());

        let loaded: Option<SensorV1> = db.load("nope").unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn save_overwrites() {
        let mut db = CrdtDb::with_store(MemoryStore::new());

        db.save("s1", &SensorV1 { temperature: 20.0 }).unwrap();
        db.save("s1", &SensorV1 { temperature: 25.0 }).unwrap();

        let loaded: Option<SensorV1> = db.load("s1").unwrap();
        assert_eq!(loaded.unwrap().temperature, 25.0);
    }

    #[test]
    fn namespace_isolation() {
        let mut db = CrdtDb::with_store(MemoryStore::new());

        let s1 = SensorV1 { temperature: 10.0 };
        let s2 = SensorV1 { temperature: 20.0 };

        db.save_ns("indoor", "s1", &s1).unwrap();
        db.save_ns("outdoor", "s1", &s2).unwrap();

        let indoor: SensorV1 = db.load_ns("indoor", "s1").unwrap().unwrap();
        let outdoor: SensorV1 = db.load_ns("outdoor", "s1").unwrap().unwrap();

        assert_eq!(indoor.temperature, 10.0);
        assert_eq!(outdoor.temperature, 20.0);
    }

    #[test]
    fn delete_removes_value() {
        let mut db = CrdtDb::with_store(MemoryStore::new());

        db.save("s1", &SensorV1 { temperature: 22.5 }).unwrap();
        assert!(db.exists("s1").unwrap());

        db.delete("s1").unwrap();
        assert!(!db.exists("s1").unwrap());

        let loaded: Option<SensorV1> = db.load("s1").unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn list_keys_works() {
        let mut db = CrdtDb::with_store(MemoryStore::new());

        db.save("b", &SensorV1 { temperature: 1.0 }).unwrap();
        db.save("a", &SensorV1 { temperature: 2.0 }).unwrap();

        let keys = db.list_keys().unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn migration_on_load() {
        use alloc::boxed::Box;
        use crdt_migrate::MigrationError;

        // Migration: SensorV1 -> SensorV2 (add humidity=None)
        struct SensorMigration;

        impl MigrationStep for SensorMigration {
            fn source_version(&self) -> u32 {
                1
            }
            fn target_version(&self) -> u32 {
                2
            }
            fn migrate(&self, data: &[u8]) -> Result<Vec<u8>, MigrationError> {
                let v1: SensorV1 = postcard::from_bytes(data)
                    .map_err(|e| MigrationError::Deserialization(e.to_string()))?;
                let v2 = SensorV2 {
                    temperature: v1.temperature,
                    humidity: None,
                };
                postcard::to_allocvec(&v2).map_err(|e| MigrationError::Serialization(e.to_string()))
            }
        }

        let mut db = CrdtDb::builder(MemoryStore::new(), 2)
            .register_migration(Box::new(SensorMigration))
            .build();

        // Save as v1 by writing raw envelope bytes
        let v1 = SensorV1 { temperature: 22.5 };
        let payload = postcard::to_allocvec(&v1).unwrap();
        let envelope = VersionedEnvelope::new(1, crdt_migrate::CrdtType::Custom, payload);
        db.store_mut()
            .put("default", "s1", &envelope.to_bytes())
            .unwrap();

        // Load as v2 — should migrate automatically
        let loaded: Option<SensorV2> = db.load("s1").unwrap();
        let v2 = loaded.unwrap();
        assert_eq!(v2.temperature, 22.5);
        assert_eq!(v2.humidity, None);

        // Verify write-back: reading raw bytes again should show v2 envelope
        let raw = db.store().get("default", "s1").unwrap().unwrap();
        let env = VersionedEnvelope::from_bytes(&raw).unwrap();
        assert_eq!(env.version, 2);
    }

    #[test]
    fn event_sourcing_roundtrip() {
        let mut db = CrdtDb::with_store(MemoryStore::new());

        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        enum Op {
            SetTemp(f32),
            SetHumidity(f32),
        }

        db.append_event("sensors", "s1", &Op::SetTemp(22.5), 1000, "node-1")
            .unwrap();
        db.append_event("sensors", "s1", &Op::SetHumidity(55.0), 1001, "node-1")
            .unwrap();

        let events = db.events_since("sensors", "s1", 0).unwrap();
        assert_eq!(events.len(), 2);

        let op1: Op = deserialize_event(&events[0]).unwrap();
        let op2: Op = deserialize_event(&events[1]).unwrap();

        assert_eq!(op1, Op::SetTemp(22.5));
        assert_eq!(op2, Op::SetHumidity(55.0));
    }

    #[test]
    fn compact_saves_snapshot_and_truncates() {
        let mut db = CrdtDb::with_store(MemoryStore::new());

        db.append_event("ns", "e1", &"op1", 100, "n").unwrap();
        db.append_event("ns", "e1", &"op2", 101, "n").unwrap();
        db.append_event("ns", "e1", &"op3", 102, "n").unwrap();

        assert_eq!(db.event_count("ns", "e1").unwrap(), 3);

        let removed = db.compact("ns", "e1", b"snapshot-state", 1).unwrap();
        assert_eq!(removed, 2); // first 2 events removed

        let snap = db.load_snapshot("ns", "e1").unwrap().unwrap();
        assert_eq!(snap.state, b"snapshot-state");

        assert_eq!(db.event_count("ns", "e1").unwrap(), 1);
    }

    #[test]
    fn versioned_envelope_is_stored() {
        let mut db = CrdtDb::with_store(MemoryStore::new());

        let sensor = SensorV1 { temperature: 22.5 };
        db.save("s1", &sensor).unwrap();

        let raw = db.store().get("default", "s1").unwrap().unwrap();
        assert!(VersionedEnvelope::is_versioned(&raw));

        let envelope = VersionedEnvelope::from_bytes(&raw).unwrap();
        assert_eq!(envelope.version, 1);
    }

    #[test]
    fn builder_with_config() {
        let mut db = CrdtDb::builder(MemoryStore::new(), 1)
            .default_namespace("sensors")
            .migration_config(MigrationConfig {
                write_back_on_read: false,
                eager_migration: false,
            })
            .build();

        let sensor = SensorV1 { temperature: 22.5 };
        db.save("s1", &sensor).unwrap();

        let raw = db.store().get("sensors", "s1").unwrap().unwrap();
        assert!(VersionedEnvelope::is_versioned(&raw));
    }
}
