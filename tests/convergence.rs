//! Integration tests verifying CRDT convergence properties.
//!
//! For any CRDT, merging replicas in any order must produce the same result.

use crdt_kit::prelude::*;

#[test]
fn gcounter_three_way_convergence() {
    let mut a = GCounter::new("a");
    let mut b = GCounter::new("b");
    let mut c = GCounter::new("c");

    a.increment_by(10);
    b.increment_by(20);
    c.increment_by(30);

    // Merge in different orders
    let mut order1 = a.clone();
    order1.merge(&b);
    order1.merge(&c);

    let mut order2 = c.clone();
    order2.merge(&a);
    order2.merge(&b);

    let mut order3 = b.clone();
    order3.merge(&c);
    order3.merge(&a);

    assert_eq!(order1.value(), 60);
    assert_eq!(order2.value(), 60);
    assert_eq!(order3.value(), 60);
}

#[test]
fn pncounter_convergence_with_concurrent_ops() {
    let mut a = PNCounter::new("a");
    let mut b = PNCounter::new("b");

    // Concurrent operations
    a.increment();
    a.increment();
    a.decrement();

    b.decrement();
    b.decrement();
    b.increment();

    let mut ab = a.clone();
    ab.merge(&b);

    let mut ba = b.clone();
    ba.merge(&a);

    assert_eq!(ab.value(), ba.value());
    assert_eq!(ab.value(), 0); // (2-1) + (1-2) = 1 + (-1) = 0
}

#[test]
fn orset_concurrent_add_remove_convergence() {
    // This is the classic OR-Set scenario:
    // One replica adds, another removes, concurrent add should survive

    let mut shared = ORSet::new("init");
    shared.insert("item");

    // Both replicas start with "item"
    let mut alice = shared.clone();
    let mut bob_set = ORSet::new("bob");
    bob_set.insert("item");

    // Alice removes "item"
    alice.remove(&"item");

    // Bob adds "item" concurrently (fresh add with new tag)
    // Since Bob is a new ORSet with actor "bob", his add has a different tag

    // Merge: Bob's concurrent add should survive Alice's remove
    alice.merge(&bob_set);
    assert!(
        alice.contains(&"item"),
        "Concurrent add should survive remove in OR-Set"
    );
}

#[test]
fn twopset_remove_wins_over_concurrent_add() {
    let mut a = TwoPSet::new();
    a.insert("x");
    a.remove(&"x");

    let mut b = TwoPSet::new();
    b.insert("x"); // concurrent add

    a.merge(&b);
    // In 2P-Set, once removed, always removed
    assert!(!a.contains(&"x"), "2P-Set: remove should be permanent");
}

#[test]
fn mvregister_preserves_concurrent_writes() {
    let mut a = MVRegister::new("a");
    let mut b = MVRegister::new("b");

    a.set(1);
    b.set(2);

    a.merge(&b);

    let values = a.values();
    assert_eq!(
        values.len(),
        2,
        "Both concurrent values should be preserved"
    );
    assert!(values.contains(&&1));
    assert!(values.contains(&&2));
}

#[test]
fn mvregister_causal_write_supersedes() {
    let mut a = MVRegister::new("a");
    a.set("first");

    // b sees a's state (causal dependency)
    let mut b = a.clone();
    b.set("second");

    a.merge(&b);
    assert_eq!(a.values(), vec![&"second"]);
    assert!(!a.is_conflicted());
}

#[test]
fn lww_register_deterministic_resolution() {
    let r1 = LWWRegister::with_timestamp("a", "x", 100);
    let r2 = LWWRegister::with_timestamp("b", "y", 200);

    let mut merged1 = r1.clone();
    merged1.merge(&r2);

    let mut merged2 = r2.clone();
    merged2.merge(&r1);

    assert_eq!(merged1.value(), merged2.value());
    assert_eq!(*merged1.value(), "y"); // later timestamp wins
}

#[test]
fn gset_union_convergence() {
    let sets: Vec<GSet<u32>> = (0..5)
        .map(|i| {
            let mut s = GSet::new();
            for j in (i * 10)..((i + 1) * 10) {
                s.insert(j);
            }
            s
        })
        .collect();

    // Merge all into first
    let mut result = sets[0].clone();
    for s in &sets[1..] {
        result.merge(s);
    }

    assert_eq!(result.len(), 50);
    for i in 0..50 {
        assert!(result.contains(&i), "Missing element {i}");
    }
}

#[test]
fn repeated_merge_is_idempotent() {
    let mut a = ORSet::new("a");
    a.insert(1);
    a.insert(2);

    let mut b = ORSet::new("b");
    b.insert(2);
    b.insert(3);

    a.merge(&b);
    let snapshot = a.clone();

    // Merging again should not change anything
    a.merge(&b);
    assert_eq!(a, snapshot, "Merge should be idempotent");

    a.merge(&b);
    assert_eq!(a, snapshot, "Merge should be idempotent (3rd time)");
}
