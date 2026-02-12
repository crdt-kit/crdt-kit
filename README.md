<div align="center">

# crdt-kit

**Conflict-free Replicated Data Types for edge computing and local-first applications**

[![Crates.io](https://img.shields.io/crates/v/crdt-kit.svg?style=for-the-badge&logo=rust)](https://crates.io/crates/crdt-kit)
[![Docs.rs](https://img.shields.io/docsrs/crdt-kit?style=for-the-badge&logo=docs.rs)](https://docs.rs/crdt-kit)
[![CI](https://img.shields.io/github/actions/workflow/status/abdielLopezpy/crdt-kit/ci.yml?style=for-the-badge&logo=github&label=CI)](https://github.com/abdielLopezpy/crdt-kit/actions)
[![License](https://img.shields.io/crates/l/crdt-kit.svg?style=for-the-badge)](LICENSE-MIT)

[Docs](https://docs.rs/crdt-kit) | [Crate](https://crates.io/crates/crdt-kit) | [Examples](./examples) | [Contributing](CONTRIBUTING.md)

</div>

---

## Why crdt-kit?

**Problem:** Existing CRDT libraries (Automerge, Yjs) are powerful but not optimized for
resource-constrained environments like Raspberry Pi, mobile devices, or IoT.

**Solution:** `crdt-kit` provides battle-tested CRDT implementations designed for:

| Use Case | Description |
|----------|-------------|
| **Edge devices** | Raspberry Pi, embedded systems, IoT sensors |
| **Mobile apps** | Offline-first with reliable sync |
| **P2P networks** | Bandwidth-efficient delta state |
| **Low latency** | Minimal overhead for real-time collaboration |

## Quick Start

```rust
use crdt_kit::prelude::*;

// Create a distributed counter
let mut counter = GCounter::new("device-1");
counter.increment();

// On another device
let mut counter2 = GCounter::new("device-2");
counter2.increment();

// Merge - they converge automatically
counter.merge(&counter2);
assert_eq!(counter.value(), 2);
```

## Available CRDTs

### Counters

| Type | Description | Use Case |
|------|-------------|----------|
| `GCounter` | Grow-only counter | Page views, event counts |
| `PNCounter` | Increment & decrement | Inventory, likes/dislikes |

### Registers

| Type | Description | Use Case |
|------|-------------|----------|
| `LWWRegister` | Last-writer-wins | User profile fields |
| `MVRegister` | Multi-value (conflict-aware) | Collaborative editing |

### Sets

| Type | Description | Use Case |
|------|-------------|----------|
| `GSet` | Grow-only set | Tag collections, seen IDs |
| `TwoPSet` | Add & remove (permanent remove) | Blocklists |
| `ORSet` | Add & remove freely | Todo lists, shopping carts |

## Installation

```toml
[dependencies]
crdt-kit = "0.1"
```

## Examples

```rust
use crdt_kit::prelude::*;

// OR-Set: collaborative todo list
let mut alice = ORSet::new("alice");
let mut bob = ORSet::new("bob");

alice.insert("Buy groceries");
bob.insert("Fix bike");

// Sync - both items present
alice.merge(&bob);
assert!(alice.contains(&"Buy groceries"));
assert!(alice.contains(&"Fix bike"));

// Remove and concurrent add - add wins!
alice.remove(&"Buy groceries");
bob.insert("Buy groceries");
alice.merge(&bob);
assert!(alice.contains(&"Buy groceries")); // concurrent add survives
```

```rust
use crdt_kit::prelude::*;

// MV-Register: detect conflicts
let mut r1 = MVRegister::new("node-1");
let mut r2 = MVRegister::new("node-2");

r1.set("Alice's edit");
r2.set("Bob's edit");

r1.merge(&r2);
assert!(r1.is_conflicted()); // both values preserved
// Application can show conflict to user for resolution
```

Run the included examples:

```bash
cargo run --example counter
cargo run --example todo_list
cargo run --example chat
```

## Benchmarks

Measured with `cargo bench` (Criterion, optimized build):

| Operation | Time |
|-----------|------|
| GCounter increment x1000 | **53 µs** |
| GCounter merge 10 replicas | **1.1 µs** |
| GCounter merge 100 replicas | **17.8 µs** |
| PNCounter inc+dec x1000 | **60 µs** |
| ORSet insert x1000 | **187 µs** |
| ORSet merge 500+500 elements | **191 µs** |
| GSet merge 1000+1000 elements | **102 µs** |
| LWWRegister merge 100 replicas | **11.5 µs** |

Run benchmarks yourself: `cargo bench`

## Guarantees

All CRDTs in this library satisfy the **Strong Eventual Consistency** properties:

- **Commutativity** - `merge(a, b) == merge(b, a)`
- **Associativity** - `merge(a, merge(b, c)) == merge(merge(a, b), c)`
- **Idempotency** - `merge(a, a) == a`

These properties are verified by **70 tests** (52 unit + 9 integration + 9 doctests).

## Roadmap

- [x] G-Counter, PN-Counter
- [x] LWW-Register, MV-Register
- [x] G-Set, 2P-Set, OR-Set
- [ ] RGA List (ordered sequence)
- [ ] Text CRDT (collaborative text editing)
- [ ] `no_std` support (embedded systems)
- [ ] `serde` serialization support
- [ ] Delta-state optimization
- [ ] WASM bindings

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) before
submitting a pull request.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
