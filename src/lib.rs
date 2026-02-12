//! # crdt-kit
//!
//! CRDTs optimized for edge computing and local-first applications.
//!
//! A CRDT (Conflict-free Replicated Data Type) is a data structure that can be
//! replicated across multiple devices and updated independently. When replicas
//! are merged, they are guaranteed to converge to the same state without
//! requiring coordination or consensus.
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

mod crdt;
mod gcounter;
mod gset;
mod lww_register;
mod mv_register;
mod or_set;
mod pncounter;
mod twop_set;

pub mod prelude;

pub use crdt::Crdt;
pub use gcounter::GCounter;
pub use gset::GSet;
pub use lww_register::LWWRegister;
pub use mv_register::MVRegister;
pub use or_set::ORSet;
pub use pncounter::PNCounter;
pub use twop_set::TwoPSet;
