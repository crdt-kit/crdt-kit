<div align="center">

<br>

# `crdt-kit`

### Conflict-free Replicated Data Types for Rust

**Build offline-first, real-time, and distributed applications that just work.**

<br>

[![Crates.io](https://img.shields.io/crates/v/crdt-kit.svg?style=for-the-badge&logo=rust&logoColor=white&color=e6522c)](https://crates.io/crates/crdt-kit)
[![Downloads](https://img.shields.io/crates/d/crdt-kit.svg?style=for-the-badge&logo=rust&logoColor=white&color=e6522c)](https://crates.io/crates/crdt-kit)
[![Docs.rs](https://img.shields.io/docsrs/crdt-kit?style=for-the-badge&logo=docs.rs&logoColor=white&color=1a6fe6)](https://docs.rs/crdt-kit)
[![CI](https://img.shields.io/github/actions/workflow/status/abdielLopezpy/crdt-kit/ci.yml?branch=master&style=for-the-badge&logo=github&logoColor=white&label=CI)](https://github.com/abdielLopezpy/crdt-kit/actions)
[![License](https://img.shields.io/crates/l/crdt-kit.svg?style=for-the-badge&color=blue)](LICENSE-MIT)

<br>

[**Documentation**](https://docs.rs/crdt-kit) &bull; [**Crate**](https://crates.io/crates/crdt-kit) &bull; [**Examples**](./examples) &bull; [**Contributing**](CONTRIBUTING.md)

<br>

</div>

## Why crdt-kit?

Traditional sync solutions break when devices go offline. CRDTs solve this at the data structure level — every replica can be updated independently, and **merges always converge to the same result**, guaranteed by math, not by servers.

`crdt-kit` is built specifically for **resource-constrained, latency-sensitive environments** where existing solutions (Automerge, Yjs) add too much overhead:

```
+-----------+     +-----------+     +-----------+
|  Device A |     |  Device B |     |  Device C |
|  (offline)|     |  (offline)|     |  (offline)|
+-----+-----+     +-----+-----+     +-----+-----+
      |                 |                 |
      |  local edits    |  local edits    |  local edits
      |                 |                 |
      +--------+--------+--------+--------+
               |                 |
               v                 v
         +-----------+     +-----------+
         |   merge   |     |   merge   |
         +-----------+     +-----------+
               |                 |
               v                 v
          Same state!       Same state!    <-- Strong Eventual Consistency
```

### Key Advantages

| | `crdt-kit` | Automerge | Yjs |
|---|---|---|---|
| **Zero dependencies** (core) | Yes | No (30+) | No (N/A — JS) |
| **`no_std` / embedded** | Yes | No | No |
| **WASM-ready** | Yes | Partial | Native JS |
| **Delta sync** | Yes | Yes | Yes |
| **Serde integration** | Yes | Custom | N/A |
| **Pure Rust** | Yes | Yes | No (JS) |
| **Binary size** (release) | ~50 KB | ~2 MB | N/A |

### Built For

| Environment | Why it matters |
|---|---|
| **IoT / Embedded** | `no_std` support — runs on bare metal, Raspberry Pi, ESP32 |
| **Mobile apps** | Offline-first with automatic conflict resolution on reconnect |
| **Edge computing** | Delta sync minimizes bandwidth between edge nodes |
| **P2P networks** | No central server needed — every peer is equal |
| **Real-time collaboration** | Concurrent edits merge without coordination |
| **WASM / Browser** | First-class WebAssembly bindings for web apps |

---

## Quick Start

```toml
[dependencies]
crdt-kit = "0.2"
```

```rust
use crdt_kit::prelude::*;

// Two devices, working offline
let mut phone = GCounter::new("phone");
phone.increment();
phone.increment();

let mut laptop = GCounter::new("laptop");
laptop.increment();

// When they reconnect — merge. Always converges.
phone.merge(&laptop);
assert_eq!(phone.value(), 3);
```

---

## Edge Computing & IoT

`crdt-kit` is purpose-built for edge environments. Import it with `no_std` for bare metal or with `serde` for network serialization:

```toml
# Raspberry Pi / ESP32 / bare metal (no standard library)
[dependencies]
crdt-kit = { version = "0.2", default-features = false }

# Edge node with JSON sync over MQTT/HTTP
[dependencies]
crdt-kit = { version = "0.2", features = ["serde"] }
serde_json = "1"
```

```rust
use crdt_kit::prelude::*;

// Edge sensor node collects temperature readings
let mut sensor_a = GCounter::new("sensor-a");
let mut sensor_b = GCounter::new("sensor-b");

// Each sensor counts events independently (no network needed)
sensor_a.increment_by(142); // 142 events detected
sensor_b.increment_by(89);  // 89 events detected

// When the gateway collects data — merge. Order doesn't matter.
sensor_a.merge(&sensor_b);
assert_eq!(sensor_a.value(), 231); // exact total, no double-counting

// Delta sync: only send what changed (saves bandwidth on LoRa/BLE)
let mut gateway = GCounter::new("gateway");
let delta = sensor_a.delta(&gateway);  // minimal payload
gateway.apply_delta(&delta);            // gateway is up to date
assert_eq!(gateway.value(), 231);
```

**Why this matters for edge:**

| Challenge | How crdt-kit solves it |
|---|---|
| Intermittent connectivity | Devices work offline, merge when connected |
| Limited bandwidth (LoRa, BLE) | Delta sync sends only changes, not full state |
| No central server | Peer-to-peer merge — any device can sync with any other |
| Memory constraints | `no_std` + `alloc` — no heap fragmentation from `std` |
| Unreliable message delivery | Idempotent merge — safe to retry, no duplicates |

---

## Available CRDTs

### Counters

| Type | Description | Real-world use |
|---|---|---|
| [`GCounter`](https://docs.rs/crdt-kit/latest/crdt_kit/struct.GCounter.html) | Grow-only counter | Page views, IoT sensor events, download counts |
| [`PNCounter`](https://docs.rs/crdt-kit/latest/crdt_kit/struct.PNCounter.html) | Increment & decrement | Inventory stock, likes/dislikes, seat reservations |

### Registers

| Type | Description | Real-world use |
|---|---|---|
| [`LWWRegister`](https://docs.rs/crdt-kit/latest/crdt_kit/struct.LWWRegister.html) | Last-writer-wins | User profile fields, config settings, GPS location |
| [`MVRegister`](https://docs.rs/crdt-kit/latest/crdt_kit/struct.MVRegister.html) | Multi-value (shows conflicts) | Collaborative fields, version tracking |

### Sets

| Type | Description | Real-world use |
|---|---|---|
| [`GSet`](https://docs.rs/crdt-kit/latest/crdt_kit/struct.GSet.html) | Grow-only set | Seen message IDs, tags, audit logs |
| [`TwoPSet`](https://docs.rs/crdt-kit/latest/crdt_kit/struct.TwoPSet.html) | Add & permanent remove | Blocklists, revoked tokens |
| [`ORSet`](https://docs.rs/crdt-kit/latest/crdt_kit/struct.ORSet.html) | Add & remove freely | Shopping carts, todo lists, chat members |

### Sequences

| Type | Description | Real-world use |
|---|---|---|
| [`Rga`](https://docs.rs/crdt-kit/latest/crdt_kit/struct.Rga.html) | Replicated Growable Array | Playlists, kanban boards, ordered lists |
| [`TextCrdt`](https://docs.rs/crdt-kit/latest/crdt_kit/struct.TextCrdt.html) | Collaborative text | Google Docs-style editing, shared notes |

### Traits

| Trait | Description |
|---|---|
| [`Crdt`](https://docs.rs/crdt-kit/latest/crdt_kit/trait.Crdt.html) | Core merge semantics (commutative, associative, idempotent) |
| [`DeltaCrdt`](https://docs.rs/crdt-kit/latest/crdt_kit/trait.DeltaCrdt.html) | Efficient delta sync — send only what changed |

---

## Real-World Example: Distributed E-Commerce

A complete example showing CRDTs powering an offline-first e-commerce system across multiple stores:

```rust
use crdt_kit::prelude::*;

// === Distributed Inventory ===

// Each store manages stock independently, even offline
let mut store_nyc = PNCounter::new("nyc");
let mut store_la  = PNCounter::new("la");

// NYC receives 50 units, sells 12
for _ in 0..50 { store_nyc.increment(); }
for _ in 0..12 { store_nyc.decrement(); }

// LA receives 30 units, sells 8
for _ in 0..30 { store_la.increment(); }
for _ in 0..8  { store_la.decrement(); }

// HQ syncs both — always correct, no conflicts
store_nyc.merge(&store_la);
assert_eq!(store_nyc.value(), 60); // (50-12) + (30-8) = 60

// === Shopping Cart (add wins over remove) ===

let mut cart_phone  = ORSet::new("phone");
let mut cart_laptop = ORSet::new("laptop");

cart_phone.insert("Blue T-Shirt");
cart_phone.insert("Headphones");
cart_laptop.insert("Running Shoes");

// User removes Headphones on phone, re-adds on laptop
cart_phone.remove(&"Headphones");
cart_laptop.insert("Headphones");

// Sync: add wins! No lost items.
cart_phone.merge(&cart_laptop);
assert!(cart_phone.contains(&"Headphones"));
assert!(cart_phone.contains(&"Running Shoes"));

// === Product Price (last write wins) ===

let mut price_admin = LWWRegister::with_timestamp("admin", 29_99u64, 1000);
let mut price_promo = LWWRegister::with_timestamp("promo-engine", 19_99u64, 1001);

price_admin.merge(&price_promo);
assert_eq!(*price_admin.value(), 19_99); // promo wins (later timestamp)

// === Collaborative Product Description ===

let mut desc_alice = TextCrdt::new("alice");
desc_alice.insert_str(0, "Premium wireless headphones");

let mut desc_bob = desc_alice.fork("bob");
desc_alice.insert_str(26, " with noise cancellation");
desc_bob.insert_str(0, "[NEW] ");

desc_alice.merge(&desc_bob);
// Both edits preserved — deterministic convergence

// === Delta Sync (bandwidth-efficient) ===

let mut views_edge = GCounter::new("edge-node-1");
for _ in 0..1000 { views_edge.increment(); }

let mut views_cloud = GCounter::new("cloud");

// Send only the diff, not the full state
let delta = views_edge.delta(&views_cloud);
views_cloud.apply_delta(&delta);
assert_eq!(views_cloud.value(), 1000);
```

See the full runnable example:

```bash
cargo run --example ecommerce       # E-commerce entities
cargo run --example counter         # Distributed counters
cargo run --example todo_list       # Collaborative todo list
cargo run --example chat            # Chat with conflict detection
```

---

## Feature Flags

| Feature | Default | Description |
|---|---|---|
| `std` | **Yes** | Standard library support |
| `serde` | No | `Serialize` / `Deserialize` for all types |
| `wasm` | No | WebAssembly bindings via `wasm-bindgen` |

```toml
# Embedded / no_std (bare metal, ESP32, Raspberry Pi Pico)
crdt-kit = { version = "0.2", default-features = false }

# With serde (JSON, MessagePack, Bincode, etc.)
crdt-kit = { version = "0.2", features = ["serde"] }

# For web applications (WASM)
crdt-kit = { version = "0.2", features = ["wasm"] }
```

---

## Performance

Measured with [Criterion](https://github.com/bheisler/criterion.rs) on optimized builds:

| Operation | Time | Throughput |
|---|---|---|
| GCounter increment x1000 | **53 µs** | ~19M ops/sec |
| GCounter merge 10 replicas | **1.1 µs** | ~9M merges/sec |
| GCounter merge 100 replicas | **17.8 µs** | — |
| PNCounter inc+dec x1000 | **60 µs** | ~16M ops/sec |
| ORSet insert x1000 | **187 µs** | ~5M ops/sec |
| ORSet merge 500+500 elements | **191 µs** | — |
| GSet merge 1000+1000 elements | **102 µs** | — |
| LWWRegister merge 100 replicas | **11.5 µs** | ~8M merges/sec |

```bash
cargo bench  # Run benchmarks yourself
```

---

## Guarantees

All CRDTs satisfy **Strong Eventual Consistency (SEC)**:

| Property | Meaning | Why it matters |
|---|---|---|
| **Commutativity** | `merge(a, b) == merge(b, a)` | Order of sync doesn't matter |
| **Associativity** | `merge(a, merge(b, c)) == merge(merge(a, b), c)` | Group syncs however you want |
| **Idempotency** | `merge(a, a) == a` | Safe to retry — no duplicates |

Verified by **132 tests** (111 unit + 9 integration + 12 doctests).

---

## Architecture

```
crdt-kit
├── Crdt           trait  — core merge semantics
├── DeltaCrdt      trait  — delta sync extension
├── GCounter             — grow-only counter        + DeltaCrdt
├── PNCounter            — positive-negative counter + DeltaCrdt
├── LWWRegister<T>       — last-writer-wins register
├── MVRegister<T>        — multi-value register
├── GSet<T>              — grow-only set
├── TwoPSet<T>           — two-phase set
├── ORSet<T>             — observed-remove set      + DeltaCrdt
├── Rga<T>               — replicated growable array
├── TextCrdt             — collaborative text
└── wasm::*              — WASM bindings (opt-in)
```

---

## Roadmap

- [x] G-Counter, PN-Counter
- [x] LWW-Register, MV-Register
- [x] G-Set, 2P-Set, OR-Set
- [x] RGA List (ordered sequence)
- [x] Text CRDT (collaborative text editing)
- [x] `no_std` support (embedded / bare metal)
- [x] `serde` serialization support
- [x] Delta-state optimization
- [x] WASM bindings
- [ ] Persistent storage adapters (sled, SQLite)
- [ ] Network transport layer (TCP, WebSocket, QUIC)
- [ ] Benchmarks against Automerge / Yrs

---

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting a pull request.

## License

Dual-licensed under your choice of:

- **MIT** — [LICENSE-MIT](LICENSE-MIT)
- **Apache 2.0** — [LICENSE-APACHE](LICENSE-APACHE)
