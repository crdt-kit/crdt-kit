//! # IoT Sensor — Edge persistence with automatic migration
//!
//! Demonstrates a typical IoT scenario: sensor devices collect readings,
//! persist them locally (SQLite), and the schema evolves between firmware
//! versions. When a device is updated, old data is transparently migrated.
//!
//! ```text
//! Firmware v1 saves:  { device_id, temperature }
//! Firmware v2 loads:  { device_id, temperature, humidity: None }  ← auto-migrated
//! ```
//!
//! Run: `cargo run -p crdt-store --features sqlite --example iot_sensor`

use crdt_migrate::{crdt_schema, migration, VersionedEnvelope};
use crdt_store::{CrdtDb, MemoryStore, StateStore};
use serde::{Deserialize, Serialize};

// ── Schema v1 ───────────────────────────────────────────────────────

#[crdt_schema(version = 1, table = "sensors")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensorReadingV1 {
    device_id: String,
    temperature: f32,
}

// ── Schema v2 (added humidity field) ────────────────────────────────

#[crdt_schema(version = 2, table = "sensors")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensorReadingV2 {
    device_id: String,
    temperature: f32,
    humidity: Option<f32>,
}

// ── Migration v1 → v2 ──────────────────────────────────────────────

#[migration(from = 1, to = 2)]
fn add_humidity(old: SensorReadingV1) -> SensorReadingV2 {
    SensorReadingV2 {
        device_id: old.device_id,
        temperature: old.temperature,
        humidity: None, // sensor doesn't have humidity yet
    }
}

// ── Main ────────────────────────────────────────────────────────────

fn main() {
    println!("=== IoT Sensor Example ===\n");

    // ── Step 1: Firmware v1 writes sensor data ──────────────────────
    println!("1. Firmware v1 saves sensor readings...");

    let mut db_v1 = CrdtDb::with_store(MemoryStore::new());

    let readings = [
        SensorReadingV1 {
            device_id: "sensor-42".into(),
            temperature: 22.5,
        },
        SensorReadingV1 {
            device_id: "sensor-17".into(),
            temperature: 18.3,
        },
    ];

    for r in &readings {
        db_v1.save(&r.device_id, r).unwrap();
        println!("   Saved: {} → {:.1}°C", r.device_id, r.temperature);
    }

    // Extract raw bytes (simulating persistent storage between firmware versions).
    let raw_42 = db_v1.store().get("default", "sensor-42").unwrap().unwrap();
    let raw_17 = db_v1.store().get("default", "sensor-17").unwrap().unwrap();

    println!("   Raw envelope: {} bytes (v{})", raw_42.len(), {
        let env = VersionedEnvelope::from_bytes(&raw_42).unwrap();
        env.version
    });

    // ── Step 2: Device gets OTA update to firmware v2 ───────────────
    println!("\n2. OTA update to firmware v2...");

    let mut store_v2 = MemoryStore::new();
    store_v2.put("default", "sensor-42", &raw_42).unwrap();
    store_v2.put("default", "sensor-17", &raw_17).unwrap();

    let mut db_v2 = CrdtDb::builder(store_v2, 2)
        .register_migration(register_add_humidity())
        .build();

    // ── Step 3: Load with automatic migration ───────────────────────
    println!("\n3. Loading data — migration happens transparently...");

    let s42: SensorReadingV2 = db_v2.load("sensor-42").unwrap().unwrap();
    let s17: SensorReadingV2 = db_v2.load("sensor-17").unwrap().unwrap();

    println!(
        "   sensor-42: {:.1}°C, humidity={:?}",
        s42.temperature, s42.humidity
    );
    println!(
        "   sensor-17: {:.1}°C, humidity={:?}",
        s17.temperature, s17.humidity
    );

    // Verify migration happened.
    assert_eq!(s42.temperature, 22.5);
    assert_eq!(s42.humidity, None);

    // ── Step 4: New readings with humidity ───────────────────────────
    println!("\n4. New readings with humidity sensor...");

    let new_reading = SensorReadingV2 {
        device_id: "sensor-42".into(),
        temperature: 23.1,
        humidity: Some(65.0),
    };
    db_v2.save("sensor-42", &new_reading).unwrap();

    let loaded: SensorReadingV2 = db_v2.load("sensor-42").unwrap().unwrap();
    println!(
        "   sensor-42: {:.1}°C, humidity={:.1}%",
        loaded.temperature,
        loaded.humidity.unwrap()
    );

    // Verify write-back is at v2.
    let raw = db_v2.store().get("default", "sensor-42").unwrap().unwrap();
    let env = VersionedEnvelope::from_bytes(&raw).unwrap();
    assert_eq!(env.version, 2);
    println!("   Stored as envelope v{}", env.version);

    println!("\n=== Done! ===");
}
