//! # crdt-kit
//!
//! CRDTs optimized for edge computing and local-first applications.
//!
//! A CRDT (Conflict-free Replicated Data Type) is a data structure that can be
//! replicated across multiple devices and updated independently. When replicas
//! are merged, they are guaranteed to converge to the same state without
//! requiring coordination or consensus.
//!
//! ## `no_std` Support
//!
//! This crate supports `no_std` environments with the `alloc` crate.
//! Disable the default `std` feature in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! crdt-kit = { version = "0.2", default-features = false }
//! ```
//!
//! Note: [`LWWRegister::new`] and [`LWWRegister::set`] require the `std`
//! feature for automatic timestamps via `SystemTime`.
//!
//! ## Quick Start
//!
//! ```
//! use crdt_kit::prelude::*;
//!
//! // Grow-only counter
//! let mut c1 = GCounter::new("device-1");
//! c1.increment();
//!
//! let mut c2 = GCounter::new("device-2");
//! c2.increment();
//!
//! c1.merge(&c2);
//! assert_eq!(c1.value(), 2);
//! ```
//!
//! ## Available CRDTs
//!
//! ### Counters
//! - [`GCounter`] - Grow-only counter (increment only)
//! - [`PNCounter`] - Positive-negative counter (increment and decrement)
//!
//! ### Registers
//! - [`LWWRegister`] - Last-writer-wins register (timestamp-based resolution)
//! - [`MVRegister`] - Multi-value register (preserves concurrent writes)
//!
//! ### Sets
//! - [`GSet`] - Grow-only set (add only)
//! - [`TwoPSet`] - Two-phase set (add and remove, remove is permanent)
//! - [`ORSet`] - Observed-remove set (add and remove freely)
//!
//! ## The `Crdt` Trait
//!
//! All types implement the [`Crdt`] trait, which provides the [`Crdt::merge`]
//! method. Merge is guaranteed to be commutative, associative, and idempotent.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

mod crdt;
mod gcounter;
mod gset;
mod lww_register;
mod mv_register;
mod or_set;
mod pncounter;
mod rga;
mod text;
mod twop_set;
mod version;
#[cfg(feature = "wasm")]
mod wasm;

pub mod clock;
pub mod events;
pub mod prelude;

pub use crdt::{Crdt, DeltaCrdt};
pub use gcounter::{GCounter, GCounterDelta};
pub use gset::GSet;
pub use lww_register::LWWRegister;
pub use mv_register::MVRegister;
pub use or_set::{ORSet, ORSetDelta};
pub use pncounter::{PNCounter, PNCounterDelta};
pub use rga::Rga;
pub use text::TextCrdt;
pub use twop_set::TwoPSet;
pub use version::{CrdtType, VersionError, Versioned};
