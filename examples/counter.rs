//! Example: Distributed counter across three devices.

use crdt_kit::prelude::*;

fn main() {
    println!("=== G-Counter Example ===\n");

    // Simulate three devices counting page views
    let mut device_a = GCounter::new("phone");
    let mut device_b = GCounter::new("tablet");
    let mut device_c = GCounter::new("laptop");

    // Each device records views independently (offline)
    device_a.increment_by(5);
    device_b.increment_by(3);
    device_c.increment_by(8);

    println!("Phone views:  {}", device_a.value());
    println!("Tablet views: {}", device_b.value());
    println!("Laptop views: {}", device_c.value());

    // When they sync, merge all states
    device_a.merge(&device_b);
    device_a.merge(&device_c);

    println!("\nAfter sync, total views: {}", device_a.value());

    println!("\n=== PN-Counter Example ===\n");

    // Simulate a distributed inventory counter
    let mut warehouse = PNCounter::new("warehouse");
    let mut store = PNCounter::new("store");

    // Warehouse adds stock
    warehouse.increment();
    warehouse.increment();
    warehouse.increment();
    println!("Warehouse added 3 items: {}", warehouse.value());

    // Store sells items
    store.decrement();
    store.decrement();
    println!("Store sold 2 items: {}", store.value());

    // Sync
    warehouse.merge(&store);
    println!("After sync, net stock change: {}", warehouse.value());
}
