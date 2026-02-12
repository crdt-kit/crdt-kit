# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/abdielLopezpy/crdt-kit/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/abdielLopezpy/crdt-kit/releases/tag/v0.1.0
