//! # Collaborative Editing — Multi-node CRDT merge with persistence
//!
//! Demonstrates how two nodes can independently edit data using CRDTs,
//! persist their state, and merge to converge on the same result.
//!
//! Uses `GCounter` for view counts and `LWWRegister` for document title,
//! showing that different CRDT types compose naturally.
//!
//! Run: `cargo run -p crdt-store --features sqlite --example collaborative`

use crdt_kit::{Crdt, GCounter, LWWRegister};
use crdt_store::{CrdtDb, CrdtVersioned, MemoryStore, StateStore};
use serde::{Deserialize, Serialize};

// ── Shared document type ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Document {
    title: LWWRegister<String>,
    views_a: GCounter,
    views_b: GCounter,
}

impl CrdtVersioned for Document {
    const SCHEMA_VERSION: u8 = 1;
}

impl Document {
    fn new(title: &str, node: &str, ts: u64) -> Self {
        Self {
            title: LWWRegister::with_timestamp(node, title.to_string(), ts),
            views_a: GCounter::new("node-a"),
            views_b: GCounter::new("node-b"),
        }
    }

    fn total_views(&self) -> u64 {
        self.views_a.value() + self.views_b.value()
    }
}

// ── Main ────────────────────────────────────────────────────────────

fn main() {
    println!("=== Collaborative Editing Example ===\n");

    // ── Step 1: Node A creates the document ─────────────────────────
    println!("1. Node A creates a document...");

    let mut db_a = CrdtDb::with_store(MemoryStore::new());
    let mut doc_a = Document::new("Draft: CRDT Architecture", "node-a", 100);
    doc_a.views_a.increment();
    doc_a.views_a.increment();

    db_a.save("doc-1", &doc_a).unwrap();
    println!(
        "   A: title={:?}, views={}",
        doc_a.title.value(),
        doc_a.total_views()
    );

    // ── Step 2: Node B gets a copy and edits independently ──────────
    println!("\n2. Node B gets a copy and edits independently...");

    // Simulate replication: copy raw bytes from A's store.
    let raw = db_a.store().get("default", "doc-1").unwrap().unwrap();
    let mut store_b = MemoryStore::new();
    store_b.put("default", "doc-1", &raw).unwrap();
    let mut db_b = CrdtDb::with_store(store_b);

    let mut doc_b: Document = db_b.load("doc-1").unwrap().unwrap();
    // B updates the title (higher timestamp wins in LWW).
    doc_b
        .title
        .set_with_timestamp("CRDT Architecture Guide".to_string(), 200);
    doc_b.views_b.increment();
    doc_b.views_b.increment();
    doc_b.views_b.increment();

    db_b.save("doc-1", &doc_b).unwrap();
    println!(
        "   B: title={:?}, views={}",
        doc_b.title.value(),
        doc_b.total_views()
    );

    // ── Step 3: Meanwhile, A keeps editing ──────────────────────────
    println!("\n3. Meanwhile, Node A keeps editing...");

    doc_a.views_a.increment();
    doc_a.views_a.increment();
    doc_a.views_a.increment();
    // A tries to update title too, but with an earlier timestamp.
    doc_a
        .title
        .set_with_timestamp("CRDT Arch Guide (A's version)".to_string(), 150);

    db_a.save("doc-1", &doc_a).unwrap();
    println!(
        "   A: title={:?}, views={}",
        doc_a.title.value(),
        doc_a.total_views()
    );

    // ── Step 4: Merge — CRDTs guarantee convergence ─────────────────
    println!("\n4. Merging A and B (CRDTs converge automatically)...");

    doc_a.title.merge(&doc_b.title);
    doc_a.views_a.merge(&doc_b.views_a);
    doc_a.views_b.merge(&doc_b.views_b);

    doc_b.title.merge(&doc_a.title);
    doc_b.views_a.merge(&doc_a.views_a);
    doc_b.views_b.merge(&doc_a.views_b);

    println!(
        "   A after merge: title={:?}, views={}",
        doc_a.title.value(),
        doc_a.total_views()
    );
    println!(
        "   B after merge: title={:?}, views={}",
        doc_b.title.value(),
        doc_b.total_views()
    );

    // Both nodes converge to the same state.
    assert_eq!(doc_a.title.value(), doc_b.title.value());
    assert_eq!(doc_a.total_views(), doc_b.total_views());
    // B's title wins (timestamp 200 > 150).
    assert_eq!(doc_a.title.value(), "CRDT Architecture Guide");
    // Views: A had 5, B had 3 → total 8.
    assert_eq!(doc_a.total_views(), 8);

    println!("\n   Convergence verified: both nodes agree!");

    // ── Step 5: Persist merged state ────────────────────────────────
    println!("\n5. Persisting merged state...");

    db_a.save("doc-1", &doc_a).unwrap();
    let final_raw = db_a.store().get("default", "doc-1").unwrap().unwrap();
    println!("   Stored {} bytes", final_raw.len());

    println!("\n=== Done! ===");
}
