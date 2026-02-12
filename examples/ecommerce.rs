//! Example: Distributed e-commerce system with real business entities.
//!
//! Demonstrates how CRDTs solve real-world problems in a multi-store
//! e-commerce platform where devices go offline and sync later.
//!
//! Run: `cargo run --example ecommerce`

use crdt_kit::prelude::*;

fn main() {
    println!("╔══════════════════════════════════════════════════════╗");
    println!("║   crdt-kit — Distributed E-Commerce Demo            ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    inventory_sync();
    shopping_cart();
    product_pricing();
    collaborative_description();
    analytics_delta_sync();
    order_tracking();
}

/// Scenario 1: Multi-store inventory that stays consistent even offline.
fn inventory_sync() {
    println!("━━━ 1. DISTRIBUTED INVENTORY ━━━\n");

    // Each store tracks stock independently using a PN-Counter.
    // Increments = received stock, decrements = sold items.
    let mut warehouse = PNCounter::new("warehouse");
    let mut store_nyc = PNCounter::new("store-nyc");
    let mut store_la = PNCounter::new("store-la");

    // Warehouse ships 100 units total
    for _ in 0..100 {
        warehouse.increment();
    }
    println!("  Warehouse shipped:   +100 units");

    // NYC store sells 35 items while offline
    for _ in 0..35 {
        store_nyc.decrement();
    }
    println!("  NYC store sold:       -35 units (offline)");

    // LA store sells 22 items while offline
    for _ in 0..22 {
        store_la.decrement();
    }
    println!("  LA store sold:        -22 units (offline)");

    // HQ syncs everyone — merge order doesn't matter!
    warehouse.merge(&store_nyc);
    warehouse.merge(&store_la);

    println!("  ─────────────────────────────");
    println!(
        "  After sync, total stock: {} units  (100 - 35 - 22 = 43)",
        warehouse.value()
    );
    assert_eq!(warehouse.value(), 43);

    // Verify: syncing in reverse order gives the same result
    let mut reverse = store_la.clone();
    reverse.merge(&store_nyc);
    reverse.merge(&warehouse);
    println!(
        "  Reverse merge:           {} units  (commutativity verified)",
        reverse.value()
    );
    assert_eq!(warehouse.value(), reverse.value());
    println!();
}

/// Scenario 2: Shopping cart that handles concurrent add/remove across devices.
fn shopping_cart() {
    println!("━━━ 2. SHOPPING CART (OR-Set) ━━━\n");

    // User has the same cart open on phone and laptop
    let mut phone = ORSet::new("phone");
    let mut laptop = ORSet::new("laptop");

    // On the phone: add items while commuting
    phone.insert("Wireless Earbuds - $49.99");
    phone.insert("Phone Case - $19.99");
    phone.insert("USB-C Cable - $9.99");
    println!("  Phone adds:  Wireless Earbuds, Phone Case, USB-C Cable");

    // On the laptop: add different items at home
    laptop.insert("Mechanical Keyboard - $89.99");
    laptop.insert("Mouse Pad - $14.99");
    println!("  Laptop adds: Mechanical Keyboard, Mouse Pad");

    // Phone removes USB-C Cable (changed mind)
    phone.remove(&"USB-C Cable - $9.99");
    println!("  Phone removes: USB-C Cable");

    // But laptop adds it back (found a better deal)
    laptop.insert("USB-C Cable - $9.99");
    println!("  Laptop re-adds: USB-C Cable (concurrent with remove)");

    // Sync both devices
    phone.merge(&laptop);
    laptop.merge(&phone);

    println!("\n  Cart after sync ({} items):", phone.len());
    for item in phone.iter() {
        println!("    - {item}");
    }
    println!("  USB-C Cable survived! (add wins over concurrent remove)");
    assert!(phone.contains(&"USB-C Cable - $9.99"));
    assert_eq!(phone.len(), 5);
    println!();
}

/// Scenario 3: Product pricing with automatic conflict resolution.
fn product_pricing() {
    println!("━━━ 3. PRODUCT PRICING (LWW-Register) ━━━\n");

    // Multiple systems can update a product's price.
    // The most recent write always wins (Last-Writer-Wins).

    // Admin sets base price at 09:00
    let mut price = LWWRegister::with_timestamp("admin", "USD 79.99", 900);
    println!("  09:00 Admin sets price:     {}", price.value());

    // Marketing runs a flash sale at 10:00
    let flash_sale = LWWRegister::with_timestamp("marketing", "USD 59.99 (SALE!)", 1000);
    price.merge(&flash_sale);
    println!("  10:00 Marketing flash sale: {}", price.value());

    // Pricing algorithm adjusts at 11:00
    let algo = LWWRegister::with_timestamp("algo-v2", "USD 64.99", 1100);
    price.merge(&algo);
    println!("  11:00 Algorithm adjusts:    {}", price.value());

    // Admin override at 08:00 (stale — won't win because timestamp is older)
    let stale = LWWRegister::with_timestamp("admin-2", "USD 99.99", 800);
    price.merge(&stale);
    println!("  08:00 Stale admin override: ignored (older timestamp)");
    println!("  Final price:                {}", price.value());
    assert_eq!(*price.value(), "USD 64.99");
    println!();
}

/// Scenario 4: Collaborative product description editing.
fn collaborative_description() {
    println!("━━━ 4. COLLABORATIVE DESCRIPTION (TextCrdt) ━━━\n");

    // Product manager creates the initial description
    let mut pm = TextCrdt::new("product-manager");
    pm.insert_str(0, "Ergonomic office chair");
    println!("  PM writes:        \"{}\"", pm);

    // Designer and copywriter fork to make concurrent edits
    let mut designer = pm.fork("designer");
    let mut copywriter = pm.fork("copywriter");

    // Designer adds material info at the end
    let len = designer.len();
    designer.insert_str(len, " with lumbar support");
    println!("  Designer adds:    \"{}\"", designer);

    // Copywriter adds a prefix
    copywriter.insert_str(0, "[BESTSELLER] ");
    println!("  Copywriter adds:  \"{}\"", copywriter);

    // Merge all edits — both changes preserved
    pm.merge(&designer);
    pm.merge(&copywriter);
    println!("\n  Merged result:    \"{}\"", pm);
    println!("  (All concurrent edits converge deterministically)");
    println!();
}

/// Scenario 5: Analytics with efficient delta sync.
fn analytics_delta_sync() {
    println!("━━━ 5. ANALYTICS WITH DELTA SYNC (DeltaCrdt) ━━━\n");

    // Edge nodes collect page view counts locally
    let mut edge_us = GCounter::new("edge-us-east");
    let mut edge_eu = GCounter::new("edge-eu-west");
    let mut central = GCounter::new("central-analytics");

    // US node tracks 5000 views
    edge_us.increment_by(5000);
    // EU node tracks 3200 views
    edge_eu.increment_by(3200);

    println!("  US edge node:  {} views", edge_us.value());
    println!("  EU edge node:  {} views", edge_eu.value());
    println!("  Central (old): {} views", central.value());

    // Instead of sending full state, generate minimal deltas
    let delta_us = edge_us.delta(&central);
    let delta_eu = edge_eu.delta(&central);

    // Apply deltas to central — same result as full merge
    central.apply_delta(&delta_us);
    central.apply_delta(&delta_eu);

    println!("\n  After delta sync:");
    println!("  Central total: {} views", central.value());
    assert_eq!(central.value(), 8200);

    // Verify: a second delta sync produces empty deltas (no redundant data)
    let delta_again = edge_us.delta(&central);
    let is_empty = delta_again == GCounter::new("dummy").delta(&GCounter::new("dummy"));
    // The delta only contains entries where self > other
    println!(
        "  Re-sync needed:  {}  (delta is already up to date)",
        if is_empty { "No" } else { "Yes" }
    );
    println!();
}

/// Scenario 6: Order status tracking with MV-Register (conflict detection).
fn order_tracking() {
    println!("━━━ 6. ORDER STATUS TRACKING (MV-Register) ━━━\n");

    // Multiple systems update order status
    let mut warehouse_view = MVRegister::new("warehouse");
    let mut shipping_view = MVRegister::new("shipping");

    // Warehouse marks order as "packed"
    warehouse_view.set("packed".to_string());
    println!("  Warehouse sets: {:?}", warehouse_view.values());

    // Shipping receives the update
    shipping_view.merge(&warehouse_view);
    println!("  Shipping sees:  {:?}", shipping_view.values());
    println!("  Conflict?       {}", shipping_view.is_conflicted());

    // Now both update concurrently (network partition)
    warehouse_view.set("ready-for-pickup".to_string());
    shipping_view.set("in-transit".to_string());

    println!("\n  [Network partition — concurrent updates]");
    println!("  Warehouse sets: {:?}", warehouse_view.values());
    println!("  Shipping sets:  {:?}", shipping_view.values());

    // Sync — both values preserved, conflict detected
    warehouse_view.merge(&shipping_view);
    println!("\n  After sync:");
    println!("  Values:   {:?}", warehouse_view.values());
    println!(
        "  Conflict? {}  (app can show alert to ops team)",
        warehouse_view.is_conflicted()
    );

    // Ops team resolves
    warehouse_view.set("in-transit".to_string());
    println!("\n  Ops resolves to: {:?}", warehouse_view.values());
    println!("  Conflict? {}", warehouse_view.is_conflicted());
    println!();
}
