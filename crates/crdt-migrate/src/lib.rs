//! # crdt-migrate
//!
//! Transparent schema migrations for [`crdt-kit`](https://docs.rs/crdt-kit).
//!
//! When your data schema evolves between versions, `crdt-migrate` ensures that
//! persisted data is automatically and transparently migrated — no downtime,
//! no manual intervention, no data loss.
//!
//! ## How It Works
//!
//! 1. Every serialized CRDT is wrapped in a **version envelope** (3-byte header).
//! 2. When data is loaded, the envelope version is compared to the current version.
//! 3. If they differ, a **chain of migration steps** runs automatically.
//! 4. Optionally, the migrated data is written back to storage.
//!
//! ## Key Concepts
//!
//! - **Lazy migration**: Data is migrated on read, not eagerly on startup.
//! - **Deterministic**: Two devices migrating the same data produce identical results.
//! - **Linear chain**: Migrations run v1→v2→v3→...→current, never skipping steps.
//! - **Compiled in**: All migrations are embedded in the binary at compile time.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod engine;
mod envelope;
mod schema;

pub use engine::{MigrationConfig, MigrationEngine, MigrationError, MigrationStep};
pub use envelope::{CrdtType, VersionedEnvelope, ENVELOPE_HEADER_SIZE, MAGIC_BYTE};
pub use schema::Schema;

// Re-export proc macros when the `macros` feature is enabled.
#[cfg(feature = "macros")]
pub use crdt_migrate_macros::{crdt_schema, migration};
