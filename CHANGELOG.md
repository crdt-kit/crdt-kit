# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.1] - 2026-03-10

### Added

- **`VersionedEnvelope`** — Binary envelope format for serialized CRDTs with 3-byte header (`[0xCF][version][crdt_type][payload]`)
- `VersionedEnvelope::to_bytes()` / `from_bytes()` — Serialize and parse envelopes
- `VersionedEnvelope::peek_version()` — Read version without full deserialization
- `VersionedEnvelope::is_versioned()` — Detect if data has a versioned envelope
- **`EnvelopeError`** — Typed errors: `TooShort`, `InvalidMagic`, `UnknownCrdtType`
- `CrdtType::from_byte()` — Deserialize CRDT type from raw byte (all 11 types)
- Constants: `MAGIC_BYTE` (0xCF), `ENVELOPE_HEADER_SIZE` (3)
- `version` module is now public (`pub mod version`)
- `VersionedEnvelope` re-exported in prelude
- 9 new unit tests for envelope roundtrip, error handling, and type coverage
- `#[cfg(feature = "std")] impl std::error::Error` for `EnvelopeError`

## [0.5.0] - 2026-03-10

### Added

- **LWWMap** — Last-writer-wins map with per-key `HybridTimestamp` resolution, tombstone-based removes
- **AWMap** — Add-wins map with OR-Set key semantics (concurrent add beats remove)
- **`Versioned` trait** — Schema versioning with `CrdtType` enum, implemented for all 11 CRDT types
- **`VersionError`** type for versioned serialization errors
- `IntoIterator` for `AWMap`
- `HybridClock` now derives `Debug` and `Clone`
- `#[cfg(feature = "std")] impl std::error::Error` for `RgaError`, `TextError`, `VersionError`
- Memory footprint benchmarks (`bench_memory_footprint`) for 6 CRDT types
- WASM bindings for LWWMap and AWMap operations

### Changed

- **BREAKING: `NodeId` is now `u64`** (was `String`) — zero heap allocations on every operation. Critical for embedded/IoT targets.
- **BREAKING: `HybridClock::new()` accepts `NodeId` (u64)** — lower 16 bits used for timestamp tiebreaking, documented in API
- **BREAKING: `Rga::remove()` returns `Result<T, RgaError>`** (was `Option<T>`) — proper error handling for out-of-bounds
- **`LWWMap::remove()` no longer requires `V: Default`** — uses `Option<V>` internally for tombstones
- **`LWWMap::iter()` uses `filter_map`** instead of `.filter().map(.unwrap())`
- **RGA merge/apply_delta optimized** — two-phase approach (tombstones then inserts) eliminates O(k) per-insert index-shift loop. Total cost reduced from O(m·n + m²) to O(m·n).
- `TextCrdt::remove_range()` simplified — removes from `start` index repeatedly (indices shift automatically)

### Removed

- Deleted `events.rs` placeholder module (was unreferenced one-line file)

### Fixed

- `compact_tombstones()` documentation now accurately describes behavior (local-only GC, safe at any time)
- WASM binding `WasmLWWRegister::new()` now correctly converts `u16` node_id to `NodeId` (u64)

## [0.4.0-beta.1] - 2026-03-09

### Changed

- **BREAKING: Rga rewritten** with flat `Vec<RgaNode>` architecture — eliminates `rebuild_sequence()` bottleneck. Insert x1000 improved from 278ms to 265μs (**1040x faster**). `RgaNode` fields changed: now `id`, `value`, `deleted` (removed `parent`). `RgaDelta` fields changed to `new_elements`, `tombstoned_ids`, `version`.
- **TextCrdt** now caches `visible_len` — `len()` is O(1) instead of O(n). Reduces overhead on every insert/remove call.
- **Rga** now has `fork()` method for creating independent replicas (same as TextCrdt).
- **Rga** now tracks version vectors for efficient delta sync.

### Added

- **Comparative benchmarks** (`benches/comparative.rs`) vs Automerge 0.7 and Yrs 0.25 — crdt-kit is 37–700x faster across all categories:
  - Counter increment: **700x** faster than Automerge
  - Text insert: **37x** faster than Yrs, **199x** faster than Automerge
  - List insert: **62x** faster than Yrs, **130x** faster than Automerge
  - Set insert: **136x** faster than Automerge
- New dev-dependencies: `automerge = "0.7"`, `yrs = "0.25"` (bench only)
- Property-based tests (`proptest`) for 9 CRDT types — 32 tests covering commutativity, associativity, idempotency, and delta equivalence

## [0.4.0] - 2026-03-05

### Added

- **crdt-cli** (v0.4.0) `new` subcommand — Interactive project scaffolding with platform and template selection
  - Polished interactive CLI experience inspired by Dioxus (`dx new`) and Tauri (`cargo tauri init`)
  - 4 platform targets: CLI App, Dioxus Client, IoT Device, Edge Computing
  - 3 project templates: Minimal (single entity), Full (events + sync), Empty (skeleton only)
  - Configurable entity name, event sourcing, and delta sync toggles
  - Auto-generates `Cargo.toml`, `crdt-schema.toml`, `src/main.rs`, platform-specific files, and `.gitignore`
  - Runs `crdt-codegen` automatically to generate the persistence layer from the schema
  - Colored output with step-by-step progress feedback
- **crdt-cli** `dev` subcommand — Development runtime combining app execution with Dev UI dashboard
  - Launches `cargo run` and Dev UI web panel in parallel
  - Auto-detects `crdt-schema.toml` and database path from the schema
  - Timestamped, color-coded log streaming (`[crdt]`, `[app]`, `[ui]` prefixes)
  - Schema change detection between restarts (auto-runs codegen when schema changes)
  - `--watch` mode for automatic restart on file changes
  - `--open` flag to auto-open Dev UI in the default browser
  - Dev UI remains running after app exits for database inspection (press Ctrl+C to stop)
  - Database snapshot display on app exit
- **crdt-cli** New dependencies: `inquire` (interactive prompts), `console` (colored output), `open` (browser launch), `chrono` (timestamps)

## [0.3.0] - 2026-02-14

### Added

- **crdt-codegen** (v0.2.0) — Complete persistence layer generation from TOML schemas
  - `repositories/` — Repository traits (ports) and `CrdtDb`-backed implementations (adapters) for hexagonal architecture
  - `store.rs` — Unified `Persistence<S>` entry point with scoped, borrow-checked repository access
  - `events/` — Event sourcing types (`{Entity}Event`, `{Entity}Snapshot`, `{Entity}FieldUpdate`), configurable snapshot policies
  - `sync/` — Delta sync (`compute_*_delta`, `apply_*_delta`) for `DeltaCrdt`-capable fields + state-based `merge_*` for all CRDT fields
  - Nested directory structure: `models/`, `migrations/`, `repositories/`, `events/`, `sync/`
  - Conditional generation: `events` and `sync` modules only when enabled in schema config
- **crdt-codegen** Schema config extensions — `[config.events]` (enabled, snapshot_threshold) and `[config.sync]` (enabled)
- **crdt-codegen** CRDT type annotations — Schema fields support `crdt = "LWWRegister"` etc., generating wrapped types (`LWWRegister<String>`) and auto-migration defaults
- **crdt-codegen** Entity relations — Schema fields support `relation = "Project"`, generating typed `find_by_*` methods
- **crdt-codegen** Delta type mapping — GCounter, PNCounter, ORSet mapped to delta types; LWWRegister, MVRegister, GSet, TwoPSet use state-based merge only
- **crdt-cli** (v0.3.0) `generate` subcommand — Generate code from schema files via `crdt generate --schema crdt-schema.toml` with `--dry-run` support and nested directory creation
- **crdt-cli** `dev-ui` subcommand — Launch an embedded Axum web panel for visual database inspection
- **crdt-store** (v0.2.0) `RedbStore` backend — Pure-Rust embedded key-value store (no C deps), implementing `StateStore`, `EventStore`, and `BatchOps`
- **crdt-store** 3 platform examples: `iot_sensor` (schema migration on OTA), `collaborative` (multi-node CRDT merge), `event_sourcing` (event log + snapshot + compaction)
- **crdt-migrate** (v0.2.0) — Versioned envelope serialization with transparent schema migrations and proc macros
- **crdt-dev-ui** (v0.2.0) — Embedded web panel for database inspection during development
- **crdt-example-tasks** — Complete example demonstrating all features: repository pattern, v1→v2 migration, CRDT fields, entity relations, delta sync, and event sourcing
- Schema validation — `snapshot_threshold > 0`, `sync.enabled` requires CRDT fields, contiguous versions, type checking
- CI feature-matrix job testing `no_std`, `serde`, `sqlite`, `redb`, and `no-macros` feature combinations
- 268 tests across the workspace

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

[Unreleased]: https://github.com/crdt-kit/crdt-kit/compare/v0.5.1...HEAD
[0.5.1]: https://github.com/crdt-kit/crdt-kit/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/crdt-kit/crdt-kit/compare/v0.4.0-beta.1...v0.5.0
[0.4.0-beta.1]: https://github.com/crdt-kit/crdt-kit/compare/v0.4.0...v0.4.0-beta.1
[0.4.0]: https://github.com/crdt-kit/crdt-kit/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/crdt-kit/crdt-kit/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/crdt-kit/crdt-kit/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/crdt-kit/crdt-kit/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/crdt-kit/crdt-kit/releases/tag/v0.1.0
