//! Integration tests for the proc macros and the full migration pipeline.
//!
//! Tests the complete flow: define schemas with macros, register migrations,
//! save v1 data, load as v3 with automatic chain migration.

use crdt_migrate::{crdt_schema, migration, MigrationEngine, VersionedEnvelope};
use crdt_store::{CrdtDb, CrdtVersioned, MemoryStore, StateStore};
use serde::{Deserialize, Serialize};

// ── Schema definitions using macros ─────────────────────────────────

#[crdt_schema(version = 1, table = "sensors")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SensorV1 {
    device_id: String,
    temperature: f32,
}

#[crdt_schema(version = 2, table = "sensors")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SensorV2 {
    device_id: String,
    temperature: f32,
    humidity: Option<f32>,
}

#[crdt_schema(version = 3, table = "sensors")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SensorV3 {
    device_id: String,
    temperature: f32,
    humidity: Option<f32>,
    location: Option<String>,
}

// ── Migration functions using macros ─────────────────────────────────

#[migration(from = 1, to = 2)]
fn add_humidity(old: SensorV1) -> SensorV2 {
    SensorV2 {
        device_id: old.device_id,
        temperature: old.temperature,
        humidity: None,
    }
}

#[migration(from = 2, to = 3)]
fn add_location(old: SensorV2) -> SensorV3 {
    SensorV3 {
        device_id: old.device_id,
        temperature: old.temperature,
        humidity: old.humidity,
        location: None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[test]
fn crdt_schema_generates_versioned_impl() {
    assert_eq!(SensorV1::SCHEMA_VERSION, 1);
    assert_eq!(SensorV2::SCHEMA_VERSION, 2);
    assert_eq!(SensorV3::SCHEMA_VERSION, 3);
}

#[test]
fn crdt_schema_generates_schema_impl() {
    use crdt_migrate::Schema;

    assert_eq!(SensorV1::VERSION, 1);
    assert_eq!(SensorV1::NAMESPACE, "sensors");
    assert_eq!(SensorV1::MIN_SUPPORTED_VERSION, 1);

    assert_eq!(SensorV2::VERSION, 2);
    assert_eq!(SensorV2::NAMESPACE, "sensors");

    assert_eq!(SensorV3::VERSION, 3);
    assert_eq!(SensorV3::NAMESPACE, "sensors");
}

#[test]
fn migration_macro_single_step() {
    let step = register_add_humidity();
    assert_eq!(step.source_version(), 1);
    assert_eq!(step.target_version(), 2);

    let v1 = SensorV1 {
        device_id: "s1".into(),
        temperature: 22.5,
    };
    let v1_bytes = postcard::to_allocvec(&v1).unwrap();
    let v2_bytes = step.migrate(&v1_bytes).unwrap();
    let v2: SensorV2 = postcard::from_bytes(&v2_bytes).unwrap();

    assert_eq!(v2.device_id, "s1");
    assert_eq!(v2.temperature, 22.5);
    assert_eq!(v2.humidity, None);
}

#[test]
fn migration_chain_v1_to_v3() {
    let mut engine = MigrationEngine::new(3);
    engine.register(register_add_humidity());
    engine.register(register_add_location());

    assert!(engine.validate_chain(1).is_ok());

    let v1 = SensorV1 {
        device_id: "sensor-42".into(),
        temperature: 22.5,
    };
    let v1_bytes = postcard::to_allocvec(&v1).unwrap();
    let v3_bytes = engine.migrate_to_current(&v1_bytes, 1).unwrap();
    let v3: SensorV3 = postcard::from_bytes(&v3_bytes).unwrap();

    assert_eq!(v3.device_id, "sensor-42");
    assert_eq!(v3.temperature, 22.5);
    assert_eq!(v3.humidity, None);
    assert_eq!(v3.location, None);
}

#[test]
fn migration_chain_v2_to_v3() {
    let mut engine = MigrationEngine::new(3);
    engine.register(register_add_humidity());
    engine.register(register_add_location());

    let v2 = SensorV2 {
        device_id: "sensor-42".into(),
        temperature: 22.5,
        humidity: Some(55.0),
    };
    let v2_bytes = postcard::to_allocvec(&v2).unwrap();
    let v3_bytes = engine.migrate_to_current(&v2_bytes, 2).unwrap();
    let v3: SensorV3 = postcard::from_bytes(&v3_bytes).unwrap();

    assert_eq!(v3.device_id, "sensor-42");
    assert_eq!(v3.humidity, Some(55.0)); // preserved from v2
    assert_eq!(v3.location, None);
}

#[test]
fn migration_is_deterministic() {
    let mut engine = MigrationEngine::new(3);
    engine.register(register_add_humidity());
    engine.register(register_add_location());

    let v1 = SensorV1 {
        device_id: "s1".into(),
        temperature: 22.5,
    };
    let v1_bytes = postcard::to_allocvec(&v1).unwrap();

    // Run migration twice — must produce identical results
    let result1 = engine.migrate_to_current(&v1_bytes, 1).unwrap();
    let result2 = engine.migrate_to_current(&v1_bytes, 1).unwrap();
    assert_eq!(result1, result2);
}

#[test]
fn migration_is_idempotent_at_current_version() {
    let mut engine = MigrationEngine::new(3);
    engine.register(register_add_humidity());
    engine.register(register_add_location());

    let v3 = SensorV3 {
        device_id: "s1".into(),
        temperature: 22.5,
        humidity: Some(55.0),
        location: Some("Miami".into()),
    };
    let v3_bytes = postcard::to_allocvec(&v3).unwrap();

    // Already at v3 — should return same data
    let result = engine.migrate_to_current(&v3_bytes, 3).unwrap();
    assert_eq!(result, v3_bytes);
}

#[test]
fn end_to_end_with_crdtdb() {
    // Simulate: app v1 saves data, app upgrades to v3 and reads it

    // Step 1: App v1 saves sensor data
    let mut db_v1 = CrdtDb::with_store(MemoryStore::new());
    let sensor = SensorV1 {
        device_id: "sensor-42".into(),
        temperature: 22.5,
    };
    db_v1.save("sensor-42", &sensor).unwrap();

    // Extract raw bytes from the store (simulating persistence across app versions)
    let raw_bytes = db_v1.store().get("default", "sensor-42").unwrap().unwrap();

    // Step 2: App v3 loads the data — migration should happen automatically
    let mut store_v3 = MemoryStore::new();
    store_v3.put("default", "sensor-42", &raw_bytes).unwrap();

    let mut db_v3 = CrdtDb::builder(store_v3, 3)
        .register_migration(register_add_humidity())
        .register_migration(register_add_location())
        .build();

    let loaded: Option<SensorV3> = db_v3.load("sensor-42").unwrap();
    let v3 = loaded.unwrap();

    assert_eq!(v3.device_id, "sensor-42");
    assert_eq!(v3.temperature, 22.5);
    assert_eq!(v3.humidity, None);
    assert_eq!(v3.location, None);

    // Step 3: Verify write-back — raw bytes should now be v3
    let raw_after = db_v3.store().get("default", "sensor-42").unwrap().unwrap();
    let env = VersionedEnvelope::from_bytes(&raw_after).unwrap();
    assert_eq!(env.version, 3);
}

#[test]
fn end_to_end_preserves_data_through_chain() {
    // v2 data with humidity set should survive migration to v3
    let mut db_v2 = CrdtDb::builder(MemoryStore::new(), 2)
        .register_migration(register_add_humidity())
        .build();

    let sensor = SensorV2 {
        device_id: "s1".into(),
        temperature: 22.5,
        humidity: Some(60.0),
    };
    db_v2.save("s1", &sensor).unwrap();

    let raw = db_v2.store().get("default", "s1").unwrap().unwrap();

    // Upgrade to v3
    let mut store_v3 = MemoryStore::new();
    store_v3.put("default", "s1", &raw).unwrap();

    let mut db_v3 = CrdtDb::builder(store_v3, 3)
        .register_migration(register_add_humidity())
        .register_migration(register_add_location())
        .build();

    let loaded: Option<SensorV3> = db_v3.load("s1").unwrap();
    let v3 = loaded.unwrap();

    assert_eq!(v3.temperature, 22.5);
    assert_eq!(v3.humidity, Some(60.0)); // preserved!
    assert_eq!(v3.location, None);
}

#[test]
fn min_version_attribute() {
    #[crdt_schema(version = 5, table = "test", min_version = 3)]
    #[derive(Debug, Serialize, Deserialize)]
    struct TestSchema {
        value: u32,
    }

    use crdt_migrate::Schema;
    assert_eq!(TestSchema::VERSION, 5);
    assert_eq!(TestSchema::MIN_SUPPORTED_VERSION, 3);
    assert_eq!(TestSchema::NAMESPACE, "test");
}
