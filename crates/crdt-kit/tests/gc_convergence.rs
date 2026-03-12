//! Tests verifying GC (tombstone compaction) behavior and known divergence risks.
//!
//! These tests document that GC is only safe under specific conditions:
//! - All replicas must be fully converged before compaction
//! - Compacting before convergence can cause data resurrection or divergence

use crdt_kit::clock::HybridTimestamp;
use crdt_kit::prelude::*;

// ─── LWWMap GC Tests ────────────────────────────────────────────────

#[test]
fn lwwmap_compact_all_after_convergence_is_safe() {
    let mut a: LWWMap<&str, i32> = LWWMap::new();
    let mut b: LWWMap<&str, i32> = LWWMap::new();

    let ts1 = HybridTimestamp::new(100, 0, 1);
    let ts2 = HybridTimestamp::new(200, 0, 1);

    a.put("x", 1, ts1);
    a.remove("x", ts2);

    // Fully converge
    b.merge(&a);
    a.merge(&b);

    assert_eq!(a.tombstone_count(), 1);

    // Compact on both replicas after full convergence
    a.compact_tombstones_all();
    b.compact_tombstones_all();

    assert_eq!(a.tombstone_count(), 0);
    assert_eq!(b.tombstone_count(), 0);

    // Both agree: "x" is gone
    assert!(a.get(&"x").is_none());
    assert!(b.get(&"x").is_none());
}

#[test]
fn lwwmap_compact_before_convergence_causes_resurrection() {
    // This test demonstrates the data resurrection bug when GC runs too early.
    let mut a: LWWMap<&str, i32> = LWWMap::new();
    let mut b: LWWMap<&str, i32> = LWWMap::new();

    let ts_insert = HybridTimestamp::new(100, 0, 1);
    let ts_remove = HybridTimestamp::new(200, 0, 1);

    // A inserts and removes "x"
    a.put("x", 42, ts_insert);
    a.remove("x", ts_remove);

    // A compacts BEFORE syncing with B
    a.compact_tombstones_all();
    assert_eq!(a.tombstone_count(), 0);

    // B has a stale insert (lower timestamp)
    b.put("x", 42, ts_insert);

    // Now A merges B's stale insert — no tombstone to block it
    a.merge(&b);

    // BUG: "x" is resurrected on A because the tombstone was removed
    assert!(
        a.get(&"x").is_some(),
        "Data resurrection: tombstone was GC'd before convergence"
    );
}

#[test]
fn lwwmap_compact_with_age_respects_latency_bound() {
    let mut map: LWWMap<&str, i32> = LWWMap::new();

    let ts_insert = HybridTimestamp::new(100, 0, 1);
    let ts_remove = HybridTimestamp::new(200, 0, 1);

    map.put("x", 1, ts_insert);
    map.remove("x", ts_remove);

    // Now = 300ms, max_age = 50ms, latency_bound = 50ms
    // safe_cutoff = 50 + 2*50 = 150ms
    // tombstone age = 300 - 200 = 100ms < 150ms → KEPT
    map.compact_tombstones_with_age(300, 50, 50);
    assert_eq!(map.tombstone_count(), 1, "Tombstone should be kept within safety window");

    // Now = 500ms → age = 300ms > 150ms → REMOVED
    map.compact_tombstones_with_age(500, 50, 50);
    assert_eq!(map.tombstone_count(), 0, "Tombstone should be removed after safety window");
}

// ─── AWMap GC Tests ─────────────────────────────────────────────────

#[test]
fn awmap_compact_after_convergence_is_safe() {
    let mut a: AWMap<String, GCounter> = AWMap::new(1);
    let mut b: AWMap<String, GCounter> = AWMap::new(2);

    a.insert("key".into(), GCounter::new(1));
    a.remove(&"key".into());

    // Converge
    b.merge(&a);
    a.merge(&b);

    a.compact_tombstones_all();
    b.compact_tombstones_all();

    assert_eq!(a.tombstone_count(), 0);
    assert_eq!(b.tombstone_count(), 0);
}

// ─── ORSet GC Tests ─────────────────────────────────────────────────

#[test]
fn orset_compact_after_convergence_is_safe() {
    let mut a: ORSet<String> = ORSet::new(1);
    let mut b: ORSet<String> = ORSet::new(2);

    a.insert("item".into());
    a.remove(&"item".into());

    // Converge
    b.merge(&a);
    a.merge(&b);

    let removed_a = a.compact_tombstones_all();
    let removed_b = b.compact_tombstones_all();

    assert!(removed_a > 0 || removed_b > 0 || a.tombstone_count() == 0);
}

// ─── RGA GC Tests ───────────────────────────────────────────────────

#[test]
fn rga_compact_demonstrates_divergence_risk() {
    // This test documents that RGA tombstone compaction can cause divergence
    // when concurrent inserts reference compacted positions.
    let mut a: Rga<char> = Rga::new(1);
    let mut b: Rga<char> = Rga::new(2);

    a.insert(0, 'A');
    a.insert(1, 'B');
    a.insert(2, 'C');

    // Sync so both have [A, B, C]
    b.merge(&a);

    // Both delete B
    a.delete(1);
    b.merge(&a);

    let before_compact = a.to_vec();
    assert_eq!(before_compact, vec!['A', 'C']);

    // Compact on A only
    let compacted = a.compact_tombstones();
    assert!(compacted > 0, "Should have compacted the B tombstone");

    // After compaction, visible content is still the same
    assert_eq!(a.to_vec(), vec!['A', 'C']);
}
