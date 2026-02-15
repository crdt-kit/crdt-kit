//! WebAssembly bindings for crdt-kit.
//!
//! Enable with the `wasm` feature:
//!
//! ```toml
//! [dependencies]
//! crdt-kit = { version = "0.2", features = ["wasm"] }
//! ```
//!
//! All types are exposed as JavaScript classes with ergonomic APIs.

use wasm_bindgen::prelude::*;

use crate::Crdt;

// ── GCounter ────────────────────────────────────────────────────────

/// A grow-only counter for use from JavaScript.
#[wasm_bindgen(js_name = GCounter)]
pub struct WasmGCounter {
    inner: crate::GCounter,
}

#[wasm_bindgen(js_class = GCounter)]
impl WasmGCounter {
    /// Create a new G-Counter with the given actor ID.
    #[wasm_bindgen(constructor)]
    pub fn new(actor: &str) -> Self {
        Self {
            inner: crate::GCounter::new(actor),
        }
    }

    /// Increment this replica's count by 1.
    pub fn increment(&mut self) {
        self.inner.increment();
    }

    /// Increment this replica's count by `n`.
    #[wasm_bindgen(js_name = incrementBy)]
    pub fn increment_by(&mut self, n: u64) {
        self.inner.increment_by(n);
    }

    /// Get the total counter value across all replicas.
    pub fn value(&self) -> u64 {
        self.inner.value()
    }

    /// Merge another G-Counter's state into this one.
    pub fn merge(&mut self, other: &WasmGCounter) {
        self.inner.merge(&other.inner);
    }
}

// ── PNCounter ───────────────────────────────────────────────────────

/// A positive-negative counter for use from JavaScript.
#[wasm_bindgen(js_name = PNCounter)]
pub struct WasmPNCounter {
    inner: crate::PNCounter,
}

#[wasm_bindgen(js_class = PNCounter)]
impl WasmPNCounter {
    /// Create a new PN-Counter with the given actor ID.
    #[wasm_bindgen(constructor)]
    pub fn new(actor: &str) -> Self {
        Self {
            inner: crate::PNCounter::new(actor),
        }
    }

    /// Increment the counter by 1.
    pub fn increment(&mut self) {
        self.inner.increment();
    }

    /// Decrement the counter by 1.
    pub fn decrement(&mut self) {
        self.inner.decrement();
    }

    /// Get the current counter value (increments - decrements).
    pub fn value(&self) -> i64 {
        self.inner.value()
    }

    /// Merge another PN-Counter's state into this one.
    pub fn merge(&mut self, other: &WasmPNCounter) {
        self.inner.merge(&other.inner);
    }
}

// ── LWWRegister ─────────────────────────────────────────────────────

/// A last-writer-wins register for string values, for use from JavaScript.
#[wasm_bindgen(js_name = LWWRegister)]
pub struct WasmLWWRegister {
    inner: crate::LWWRegister<String>,
}

#[wasm_bindgen(js_class = LWWRegister)]
impl WasmLWWRegister {
    /// Create a new LWW-Register with an explicit timestamp.
    #[wasm_bindgen(constructor)]
    pub fn new(actor: &str, value: &str, timestamp: u64) -> Self {
        Self {
            inner: crate::LWWRegister::with_timestamp(actor, value.to_string(), timestamp),
        }
    }

    /// Update the register's value with a timestamp.
    pub fn set(&mut self, value: &str, timestamp: u64) {
        self.inner.set_with_timestamp(value.to_string(), timestamp);
    }

    /// Get the current value.
    pub fn value(&self) -> String {
        self.inner.value().clone()
    }

    /// Get the current timestamp.
    pub fn timestamp(&self) -> u64 {
        self.inner.timestamp()
    }

    /// Merge another LWW-Register's state into this one.
    pub fn merge(&mut self, other: &WasmLWWRegister) {
        self.inner.merge(&other.inner);
    }
}

// ── GSet ────────────────────────────────────────────────────────────

/// A grow-only set of strings for use from JavaScript.
#[wasm_bindgen(js_name = GSet)]
pub struct WasmGSet {
    inner: crate::GSet<String>,
}

impl Default for WasmGSet {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen(js_class = GSet)]
impl WasmGSet {
    /// Create a new empty G-Set.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: crate::GSet::new(),
        }
    }

    /// Insert an element into the set.
    pub fn insert(&mut self, value: &str) -> bool {
        self.inner.insert(value.to_string())
    }

    /// Check if the set contains an element.
    pub fn contains(&self, value: &str) -> bool {
        self.inner.contains(&value.to_string())
    }

    /// Get the number of elements.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the set is empty.
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Merge another G-Set's state into this one.
    pub fn merge(&mut self, other: &WasmGSet) {
        self.inner.merge(&other.inner);
    }

    /// Get all elements as a JSON array string.
    #[wasm_bindgen(js_name = toArray)]
    pub fn to_array(&self) -> Box<[JsValue]> {
        self.inner
            .iter()
            .map(|s| JsValue::from_str(s))
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }
}

// ── ORSet ───────────────────────────────────────────────────────────

/// An observed-remove set of strings for use from JavaScript.
#[wasm_bindgen(js_name = ORSet)]
pub struct WasmORSet {
    inner: crate::ORSet<String>,
}

#[wasm_bindgen(js_class = ORSet)]
impl WasmORSet {
    /// Create a new empty OR-Set for the given actor.
    #[wasm_bindgen(constructor)]
    pub fn new(actor: &str) -> Self {
        Self {
            inner: crate::ORSet::new(actor),
        }
    }

    /// Insert an element into the set.
    pub fn insert(&mut self, value: &str) {
        self.inner.insert(value.to_string());
    }

    /// Remove an element from the set.
    pub fn remove(&mut self, value: &str) -> bool {
        self.inner.remove(&value.to_string())
    }

    /// Check if the set contains an element.
    pub fn contains(&self, value: &str) -> bool {
        self.inner.contains(&value.to_string())
    }

    /// Get the number of elements.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the set is empty.
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Merge another OR-Set's state into this one.
    pub fn merge(&mut self, other: &WasmORSet) {
        self.inner.merge(&other.inner);
    }

    /// Get all elements as a JavaScript array.
    #[wasm_bindgen(js_name = toArray)]
    pub fn to_array(&self) -> Box<[JsValue]> {
        self.inner
            .iter()
            .map(|s| JsValue::from_str(s))
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }
}

// ── TextCrdt ────────────────────────────────────────────────────────

/// A collaborative text CRDT for use from JavaScript.
#[wasm_bindgen(js_name = TextCrdt)]
pub struct WasmTextCrdt {
    inner: crate::TextCrdt,
}

#[wasm_bindgen(js_class = TextCrdt)]
impl WasmTextCrdt {
    /// Create a new empty text CRDT for the given actor.
    #[wasm_bindgen(constructor)]
    pub fn new(actor: &str) -> Self {
        Self {
            inner: crate::TextCrdt::new(actor),
        }
    }

    /// Insert a single character at the given visible position.
    pub fn insert(&mut self, pos: usize, ch: char) {
        self.inner.insert(pos, ch);
    }

    /// Insert a string at the given visible position.
    #[wasm_bindgen(js_name = insertStr)]
    pub fn insert_str(&mut self, pos: usize, text: &str) {
        self.inner.insert_str(pos, text);
    }

    /// Remove the character at the given visible position.
    pub fn remove(&mut self, pos: usize) {
        self.inner.remove(pos);
    }

    /// Get the current visible text.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        use alloc::string::ToString;
        self.inner.to_string()
    }

    /// Get the number of visible characters.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the text is empty.
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Merge another text CRDT's state into this one.
    pub fn merge(&mut self, other: &WasmTextCrdt) {
        self.inner.merge(&other.inner);
    }
}
