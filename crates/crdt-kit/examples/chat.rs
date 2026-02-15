//! Example: Simple P2P chat with conflict detection using MV-Register.

use crdt_kit::prelude::*;

fn main() {
    println!("=== Chat Status with Conflict Detection (MV-Register) ===\n");

    // Two users updating a shared "room topic" register
    let mut alice_view = MVRegister::new("alice");
    let mut bob_view = MVRegister::new("bob");

    // Alice sets the topic
    alice_view.set("Project kickoff meeting".to_string());
    println!("Alice sets topic: {:?}", alice_view.values());

    // Bob receives Alice's update
    bob_view.merge(&alice_view);
    println!("Bob sees topic:   {:?}", bob_view.values());

    // Now both update concurrently (offline)
    alice_view.set("Sprint planning".to_string());
    bob_view.set("Design review".to_string());

    println!("\n--- Concurrent updates ---");
    println!("Alice's topic: {:?}", alice_view.values());
    println!("Bob's topic:   {:?}", bob_view.values());

    // Sync - both values are preserved
    alice_view.merge(&bob_view);
    println!("\n--- After sync ---");
    println!("Values: {:?}", alice_view.values());
    println!("Conflict detected: {}", alice_view.is_conflicted());

    // Alice resolves the conflict
    alice_view.set("Sprint planning + Design review".to_string());
    println!("\n--- Alice resolves conflict ---");
    println!("Topic: {:?}", alice_view.values());
    println!("Conflict: {}", alice_view.is_conflicted());

    println!("\n=== LWW-Register (auto-resolve by timestamp) ===\n");

    let mut r1 = LWWRegister::with_timestamp("node-1", "value-a", 100);
    let r2 = LWWRegister::with_timestamp("node-2", "value-b", 200);

    println!("Node 1: {:?} (ts={})", r1.value(), r1.timestamp());
    println!("Node 2: {:?} (ts={})", r2.value(), r2.timestamp());

    r1.merge(&r2);
    println!("After merge: {:?} (latest timestamp wins)", r1.value());
}
