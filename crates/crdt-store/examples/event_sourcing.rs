//! # Event Sourcing — Operation log with snapshots and compaction
//!
//! Demonstrates the event-sourcing pattern: every operation is appended
//! to an immutable log, periodic snapshots are taken for fast recovery,
//! and old events are compacted to reclaim storage.
//!
//! This pattern is the foundation for delta-sync between nodes.
//!
//! Run: `cargo run -p crdt-store --features sqlite --example event_sourcing`

use crdt_store::{EventStore, MemoryStore, StateStore};

fn main() {
    println!("=== Event Sourcing Example ===\n");

    let mut store = MemoryStore::new();
    let ns = "inventory";
    let entity = "product-42";

    // ── Step 1: Append operations as events ─────────────────────────
    println!("1. Appending inventory operations...");

    let operations = [
        ("node-a", b"AddStock(50)" as &[u8]),
        ("node-b", b"AddStock(30)"),
        ("node-a", b"Sell(5)"),
        ("node-b", b"Sell(3)"),
        ("node-a", b"AddStock(20)"),
        ("node-a", b"Sell(10)"),
        ("node-b", b"Sell(7)"),
        ("node-a", b"AddStock(15)"),
    ];

    for (i, (node, op)) in operations.iter().enumerate() {
        let ts = (i as u64 + 1) * 1000; // monotonic timestamps
        let seq = store.append_event(ns, entity, op, ts, node).unwrap();
        println!("   seq={seq}: {node} → {}", String::from_utf8_lossy(op));
    }

    let total = store.event_count(ns, entity).unwrap();
    println!("\n   Total events: {total}");

    // ── Step 2: Read events since a sequence ────────────────────────
    println!("\n2. Reading events since seq 5 (delta sync)...");

    let recent = store.events_since(ns, entity, 5).unwrap();
    println!("   Got {} events:", recent.len());
    for ev in &recent {
        println!(
            "   seq={}: {} → {}",
            ev.sequence,
            ev.node_id,
            String::from_utf8_lossy(&ev.data)
        );
    }

    // ── Step 3: Save a snapshot ─────────────────────────────────────
    println!("\n3. Taking snapshot at current state...");

    // In a real app, you'd serialize the current CRDT state.
    let snapshot_state = b"Inventory { stock: 90 }";
    let at_seq = total; // snapshot at the latest event
    store
        .save_snapshot(ns, entity, snapshot_state, at_seq, 1)
        .unwrap();
    println!(
        "   Snapshot saved at seq={at_seq}: {} bytes",
        snapshot_state.len()
    );

    // ── Step 4: Compact old events ──────────────────────────────────
    println!("\n4. Compacting events before seq {at_seq}...");

    let before = store.event_count(ns, entity).unwrap();
    let removed = store.truncate_events_before(ns, entity, at_seq).unwrap();
    let after = store.event_count(ns, entity).unwrap();
    println!("   Before: {before} events");
    println!("   Removed: {removed} events");
    println!("   After: {after} events");

    // ── Step 5: Recovery from snapshot ───────────────────────────────
    println!("\n5. Simulating recovery from snapshot...");

    let snap = store.load_snapshot(ns, entity).unwrap().unwrap();
    println!(
        "   Loaded snapshot at seq={}, v{}: {:?}",
        snap.at_sequence,
        snap.version,
        String::from_utf8_lossy(&snap.state)
    );

    // Replay events since snapshot for full state.
    let replay = store.events_since(ns, entity, snap.at_sequence).unwrap();
    println!("   Events to replay: {}", replay.len());

    // ── Step 6: New node joins — delta sync ─────────────────────────
    println!("\n6. New node joins — gets snapshot + recent events...");

    // The new node gets: snapshot + events since snapshot.
    // This is much cheaper than replaying the full event history.
    println!(
        "   Sync payload: 1 snapshot ({} bytes) + {} events",
        snap.state.len(),
        replay.len()
    );
    println!("   (vs {} events without snapshots)", before);

    // Also store the current state.
    store.put(ns, entity, snapshot_state).unwrap();
    let state = store.get(ns, entity).unwrap().unwrap();
    println!("   Current state: {:?}", String::from_utf8_lossy(&state));

    println!("\n=== Done! ===");
}
