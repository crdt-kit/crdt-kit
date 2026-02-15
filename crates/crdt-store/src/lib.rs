//! # crdt-store
//!
//! Persistence backends for [`crdt-kit`](https://docs.rs/crdt-kit).
//!
//! Provides a unified storage abstraction for persisting CRDT state and
//! operation event logs across different backends: SQLite, redb, in-memory,
//! and raw flash (for `no_std` IoT).
//!
//! ## Quick Start
//!
//! ```
//! use crdt_store::{MemoryStore, StateStore};
//!
//! let mut store = MemoryStore::new();
//! store.put("sensors", "sensor-42", b"hello").unwrap();
//! let data = store.get("sensors", "sensor-42").unwrap();
//! assert_eq!(data.as_deref(), Some(b"hello".as_slice()));
//! ```
//!
//! ## Backends
//!
//! | Backend | Feature flag | Use case |
//! |---------|-------------|----------|
//! | [`MemoryStore`] | *(always available)* | Testing, prototyping |
//! | `SqliteStore` | `sqlite` | Edge Linux, mobile, desktop |
//! | `RedbStore` | `redb` | Pure-Rust edge without C deps |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod db;
mod memory;
#[cfg(feature = "redb")]
mod redb;
#[cfg(feature = "sqlite")]
mod sqlite;
mod traits;

pub use db::{deserialize_event, CrdtDb, CrdtDbBuilder, CrdtDbConfig, CrdtVersioned, DbError};
pub use memory::MemoryStore;
#[cfg(feature = "redb")]
pub use redb::{RedbError, RedbStore};
#[cfg(feature = "sqlite")]
pub use sqlite::{JournalMode, SqliteConfig, SqliteError, SqliteStore};
pub use traits::*;
