# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-02-13

### Added

- `IntoIterator` for `TwoPSet<T>` and `ORSet<T>` (Rust API Guidelines compliance)
- `#![warn(missing_docs)]` lint to enforce documentation on all public items
- Compile-time `Send + Sync` assertions for all CRDT types
- 133 tests total (111 unit + 10 integration + 12 doctests)

## [0.2.0] - 2026-02-12

### Added

- `Rga` - Replicated Growable Array (ordered sequence CRDT) for lists and playlists
- `TextCrdt` - Collaborative text CRDT with `fork()`, `insert_str()`, and `remove_range()`
- `DeltaCrdt` trait for efficient delta-state synchronization
- `GCounterDelta`, `PNCounterDelta`, `ORSetDelta` delta types
- `no_std` support — all types work with `#![no_std]` + `alloc` (disable `std` feature)
- `serde` feature — `Serialize`/`Deserialize` for all CRDT types via `#[cfg_attr]`
- `wasm` feature — WebAssembly bindings via `wasm-bindgen` for GCounter, PNCounter, LWWRegister, GSet, ORSet, TextCrdt
- E-commerce example (`examples/ecommerce.rs`) with 6 real-world business scenarios
- 132 tests total (111 unit + 9 integration + 12 doctests)

### Changed

- All `std::collections` imports replaced with `alloc::collections` for `no_std` compatibility
- `LWWRegister::new()` and `LWWRegister::set()` now require `std` feature (use `with_timestamp()` / `set_with_timestamp()` in `no_std`)
- README rewritten with comparison table, architecture diagram, and real-world examples

## [0.1.0] - 2026-02-12

### Added

- `GCounter` - Grow-only counter
- `PNCounter` - Positive-negative counter (increment and decrement)
- `LWWRegister` - Last-writer-wins register
- `MVRegister` - Multi-value register (preserves concurrent writes)
- `GSet` - Grow-only set
- `TwoPSet` - Two-phase set (add and remove, remove is permanent)
- `ORSet` - Observed-remove set (add and remove freely)
- `Crdt` trait for unified merge/convergence interface
- Prelude module for convenient imports
- Property-based convergence guarantees
- Comprehensive test suite
- Benchmark suite comparing operations

[Unreleased]: https://github.com/abdielLopezpy/crdt-kit/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/abdielLopezpy/crdt-kit/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/abdielLopezpy/crdt-kit/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/abdielLopezpy/crdt-kit/releases/tag/v0.1.0
