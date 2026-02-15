//! Example: Collaborative offline todo list using OR-Set.

use crdt_kit::prelude::*;

fn main() {
    println!("=== Collaborative Todo List (OR-Set) ===\n");

    // Alice and Bob each have a replica of the shared todo list
    let mut alice = ORSet::new("alice");
    let mut bob = ORSet::new("bob");

    // Alice adds tasks while offline
    alice.insert("Buy groceries");
    alice.insert("Walk the dog");
    alice.insert("Write report");
    println!("Alice's list:");
    for item in alice.iter() {
        println!("  - {item}");
    }

    // Bob adds tasks while offline
    bob.insert("Fix bike");
    bob.insert("Buy groceries"); // same task, added independently
    println!("\nBob's list:");
    for item in bob.iter() {
        println!("  - {item}");
    }

    // They sync up
    alice.merge(&bob);
    bob.merge(&alice);

    println!("\n--- After sync ---");
    println!("Shared list ({} items):", alice.len());
    for item in alice.iter() {
        println!("  - {item}");
    }

    // Alice completes a task
    alice.remove(&"Buy groceries");
    println!("\nAlice completed 'Buy groceries'");

    // Bob concurrently adds it back (didn't see the remove yet)
    bob.insert("Buy groceries");

    // Sync again - add wins!
    alice.merge(&bob);
    println!("\nAfter sync (add wins):");
    println!(
        "'Buy groceries' present: {}",
        alice.contains(&"Buy groceries")
    );
    println!("Total items: {}", alice.len());
}
