//! Pure-Rust key-value backend using [`redb`](https://docs.rs/redb).
//!
//! No C dependencies — ideal for edge devices where you can't cross-compile
//! SQLite, or when you want a fully Rust-native stack.
//!
//! Enable with `features = ["redb"]`.
//!
//! ```no_run
//! use crdt_store::{RedbStore, StateStore};
//!
//! let mut store = RedbStore::open("/tmp/crdt.redb").unwrap();
//! store.put("sensors", "s1", b"hello").unwrap();
//! ```

use std::path::Path;

use redb::{Database, ReadableTable, TableDefinition};

use crate::traits::{
    BatchOps, DbInfo, EventStore, NamespaceInfo, Snapshot, StateStore, StoredEvent,
};

// ── Table definitions ───────────────────────────────────────────────

const STATE_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("crdt_state");
const EVENT_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("crdt_events");
const SNAPSHOT_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("crdt_snapshots");
const META_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("crdt_meta");

// ── Error type ──────────────────────────────────────────────────────

/// Errors returned by [`RedbStore`] operations.
#[derive(Debug)]
pub struct RedbError(String);

impl std::fmt::Display for RedbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RedbError {}

fn err(e: impl std::fmt::Display) -> RedbError {
    RedbError(e.to_string())
}

// ── Store ───────────────────────────────────────────────────────────

/// A pure-Rust persistence backend built on [`redb`].
///
/// Uses four internal tables: state, events, snapshots, and metadata.
/// All writes are atomic (each operation runs in its own redb transaction).
pub struct RedbStore {
    db: Database,
}

impl RedbStore {
    /// Open or create a redb database at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, RedbError> {
        let db = Database::create(path).map_err(err)?;
        // Ensure tables exist by opening a write txn.
        let txn = db.begin_write().map_err(err)?;
        txn.open_table(STATE_TABLE).map_err(err)?;
        txn.open_table(EVENT_TABLE).map_err(err)?;
        txn.open_table(SNAPSHOT_TABLE).map_err(err)?;
        txn.open_table(META_TABLE).map_err(err)?;
        txn.commit().map_err(err)?;
        Ok(Self { db })
    }

    /// Create an in-memory redb database (for testing).
    pub fn open_in_memory() -> Result<Self, RedbError> {
        let backend = redb::backends::InMemoryBackend::new();
        let db = Database::builder()
            .create_with_backend(backend)
            .map_err(err)?;
        let txn = db.begin_write().map_err(err)?;
        txn.open_table(STATE_TABLE).map_err(err)?;
        txn.open_table(EVENT_TABLE).map_err(err)?;
        txn.open_table(SNAPSHOT_TABLE).map_err(err)?;
        txn.open_table(META_TABLE).map_err(err)?;
        txn.commit().map_err(err)?;
        Ok(Self { db })
    }

    /// Return summary information about the database.
    pub fn db_info(&self) -> Result<DbInfo, RedbError> {
        let txn = self.db.begin_read().map_err(err)?;
        let state_table = txn.open_table(STATE_TABLE).map_err(err)?;
        let event_table = txn.open_table(EVENT_TABLE).map_err(err)?;
        let snap_table = txn.open_table(SNAPSHOT_TABLE).map_err(err)?;

        // Collect per-namespace counts by scanning state table.
        let mut ns_map: std::collections::BTreeMap<String, (u64, u64, u64)> =
            std::collections::BTreeMap::new();

        let iter = state_table.iter().map_err(err)?;
        for item in iter {
            let (key_guard, _) = item.map_err(err)?;
            if let Some((ns, _)) = parse_state_key(key_guard.value()) {
                let entry = ns_map.entry(ns.to_string()).or_insert((0, 0, 0));
                entry.0 += 1; // entity count
            }
        }

        // Count events per namespace.
        let iter = event_table.iter().map_err(err)?;
        for item in iter {
            let (key_guard, _) = item.map_err(err)?;
            if let Some((ns, _, _)) = parse_event_key(key_guard.value()) {
                let entry = ns_map.entry(ns.to_string()).or_insert((0, 0, 0));
                entry.1 += 1; // event count
            }
        }

        // Count snapshots per namespace.
        let iter = snap_table.iter().map_err(err)?;
        for item in iter {
            let (key_guard, _) = item.map_err(err)?;
            if let Some((ns, _)) = parse_state_key(key_guard.value()) {
                let entry = ns_map.entry(ns.to_string()).or_insert((0, 0, 0));
                entry.2 += 1; // snapshot count
            }
        }

        let mut total_entities = 0u64;
        let mut total_events = 0u64;
        let mut total_snapshots = 0u64;
        let mut namespaces = Vec::new();

        for (name, (ec, evc, sc)) in &ns_map {
            total_entities += ec;
            total_events += evc;
            total_snapshots += sc;
            namespaces.push(NamespaceInfo {
                name: name.clone(),
                entity_count: *ec,
                event_count: *evc,
                snapshot_count: *sc,
            });
        }

        Ok(DbInfo {
            total_entities,
            total_events,
            total_snapshots,
            namespaces,
        })
    }
}

// ── StateStore ──────────────────────────────────────────────────────

impl StateStore for RedbStore {
    type Error = RedbError;

    fn put(&mut self, namespace: &str, key: &str, value: &[u8]) -> Result<(), RedbError> {
        let txn = self.db.begin_write().map_err(err)?;
        {
            let mut table = txn.open_table(STATE_TABLE).map_err(err)?;
            table
                .insert(state_key(namespace, key).as_slice(), value)
                .map_err(err)?;
        }
        txn.commit().map_err(err)?;
        Ok(())
    }

    fn get(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>, RedbError> {
        let txn = self.db.begin_read().map_err(err)?;
        let table = txn.open_table(STATE_TABLE).map_err(err)?;
        match table
            .get(state_key(namespace, key).as_slice())
            .map_err(err)?
        {
            Some(guard) => Ok(Some(guard.value().to_vec())),
            None => Ok(None),
        }
    }

    fn delete(&mut self, namespace: &str, key: &str) -> Result<(), RedbError> {
        let txn = self.db.begin_write().map_err(err)?;
        {
            let mut table = txn.open_table(STATE_TABLE).map_err(err)?;
            table
                .remove(state_key(namespace, key).as_slice())
                .map_err(err)?;
        }
        txn.commit().map_err(err)?;
        Ok(())
    }

    fn list_keys(&self, namespace: &str) -> Result<Vec<String>, RedbError> {
        let txn = self.db.begin_read().map_err(err)?;
        let table = txn.open_table(STATE_TABLE).map_err(err)?;

        let prefix = state_key_prefix(namespace);
        let upper = state_key_prefix_upper(namespace);
        let range = table
            .range(prefix.as_slice()..upper.as_slice())
            .map_err(err)?;

        let mut keys = Vec::new();
        for item in range {
            let (key_guard, _) = item.map_err(err)?;
            if let Some((_, k)) = parse_state_key(key_guard.value()) {
                keys.push(k.to_string());
            }
        }
        Ok(keys)
    }
}

// ── EventStore ──────────────────────────────────────────────────────

impl EventStore for RedbStore {
    fn append_event(
        &mut self,
        namespace: &str,
        entity_id: &str,
        data: &[u8],
        timestamp: u64,
        node_id: &str,
    ) -> Result<u64, RedbError> {
        let txn = self.db.begin_write().map_err(err)?;
        let seq = {
            // Read current next sequence from meta table.
            let mut meta = txn.open_table(META_TABLE).map_err(err)?;
            let meta_key = seq_meta_key(namespace, entity_id);
            let current = match meta.get(meta_key.as_slice()).map_err(err)? {
                Some(guard) => u64::from_be_bytes(
                    guard
                        .value()
                        .try_into()
                        .map_err(|_| RedbError("invalid seq".into()))?,
                ),
                None => 0,
            };
            let seq = current + 1;

            // Write the event.
            let mut events = txn.open_table(EVENT_TABLE).map_err(err)?;
            let ek = event_key(namespace, entity_id, seq);
            let ev = encode_event_value(timestamp, node_id, data);
            events.insert(ek.as_slice(), ev.as_slice()).map_err(err)?;

            // Update next sequence.
            meta.insert(meta_key.as_slice(), seq.to_be_bytes().as_slice())
                .map_err(err)?;

            seq
        };
        txn.commit().map_err(err)?;
        Ok(seq)
    }

    fn events_since(
        &self,
        namespace: &str,
        entity_id: &str,
        since_sequence: u64,
    ) -> Result<Vec<StoredEvent>, RedbError> {
        let txn = self.db.begin_read().map_err(err)?;
        let table = txn.open_table(EVENT_TABLE).map_err(err)?;

        let start_seq = since_sequence.saturating_add(1);
        let start = event_key(namespace, entity_id, start_seq);
        let end = event_key(namespace, entity_id, u64::MAX);
        let range = table
            .range(start.as_slice()..=end.as_slice())
            .map_err(err)?;

        let mut events = Vec::new();
        for item in range {
            let (key_guard, val_guard) = item.map_err(err)?;
            if let Some((_, _, seq)) = parse_event_key(key_guard.value()) {
                if let Some((ts, nid, data)) = decode_event_value(val_guard.value()) {
                    events.push(StoredEvent {
                        sequence: seq,
                        entity_id: entity_id.to_string(),
                        namespace: namespace.to_string(),
                        data,
                        timestamp: ts,
                        node_id: nid,
                    });
                }
            }
        }
        Ok(events)
    }

    fn event_count(&self, namespace: &str, entity_id: &str) -> Result<u64, RedbError> {
        let txn = self.db.begin_read().map_err(err)?;
        let table = txn.open_table(EVENT_TABLE).map_err(err)?;

        let start = event_key(namespace, entity_id, 0);
        let end = event_key(namespace, entity_id, u64::MAX);
        let range = table
            .range(start.as_slice()..=end.as_slice())
            .map_err(err)?;

        let mut count = 0u64;
        for item in range {
            let _ = item.map_err(err)?;
            count += 1;
        }
        Ok(count)
    }

    fn save_snapshot(
        &mut self,
        namespace: &str,
        entity_id: &str,
        state: &[u8],
        at_sequence: u64,
        version: u8,
    ) -> Result<(), RedbError> {
        let txn = self.db.begin_write().map_err(err)?;
        {
            let mut table = txn.open_table(SNAPSHOT_TABLE).map_err(err)?;
            let key = state_key(namespace, entity_id);
            let val = encode_snapshot_value(at_sequence, version, state);
            table.insert(key.as_slice(), val.as_slice()).map_err(err)?;
        }
        txn.commit().map_err(err)?;
        Ok(())
    }

    fn load_snapshot(
        &self,
        namespace: &str,
        entity_id: &str,
    ) -> Result<Option<Snapshot>, RedbError> {
        let txn = self.db.begin_read().map_err(err)?;
        let table = txn.open_table(SNAPSHOT_TABLE).map_err(err)?;
        let key = state_key(namespace, entity_id);
        match table.get(key.as_slice()).map_err(err)? {
            Some(guard) => Ok(decode_snapshot_value(guard.value())),
            None => Ok(None),
        }
    }

    fn truncate_events_before(
        &mut self,
        namespace: &str,
        entity_id: &str,
        before_sequence: u64,
    ) -> Result<u64, RedbError> {
        let txn = self.db.begin_write().map_err(err)?;
        let removed = {
            let mut table = txn.open_table(EVENT_TABLE).map_err(err)?;

            // Collect keys to remove (can't mutate while iterating).
            let start = event_key(namespace, entity_id, 0);
            let end = event_key(namespace, entity_id, before_sequence.saturating_sub(1));
            let range = table
                .range(start.as_slice()..=end.as_slice())
                .map_err(err)?;

            let keys_to_remove: Vec<Vec<u8>> = range
                .map(|item| {
                    let (key_guard, _) = item.expect("read error");
                    key_guard.value().to_vec()
                })
                .collect();

            let count = keys_to_remove.len() as u64;
            for key in &keys_to_remove {
                table.remove(key.as_slice()).map_err(err)?;
            }
            count
        };
        txn.commit().map_err(err)?;
        Ok(removed)
    }
}

// ── BatchOps ────────────────────────────────────────────────────────

impl BatchOps for RedbStore {
    fn put_batch(&mut self, namespace: &str, entries: &[(&str, &[u8])]) -> Result<(), RedbError> {
        let txn = self.db.begin_write().map_err(err)?;
        {
            let mut table = txn.open_table(STATE_TABLE).map_err(err)?;
            for (key, value) in entries {
                table
                    .insert(state_key(namespace, key).as_slice(), *value)
                    .map_err(err)?;
            }
        }
        txn.commit().map_err(err)?;
        Ok(())
    }
}

// ── Key encoding helpers ────────────────────────────────────────────

/// State / snapshot key: `namespace \0 key`
fn state_key(namespace: &str, key: &str) -> Vec<u8> {
    let mut k = Vec::with_capacity(namespace.len() + 1 + key.len());
    k.extend_from_slice(namespace.as_bytes());
    k.push(0);
    k.extend_from_slice(key.as_bytes());
    k
}

/// Lower bound for all state keys in a namespace.
fn state_key_prefix(namespace: &str) -> Vec<u8> {
    let mut k = Vec::with_capacity(namespace.len() + 1);
    k.extend_from_slice(namespace.as_bytes());
    k.push(0);
    k
}

/// Upper bound (exclusive) for all state keys in a namespace.
fn state_key_prefix_upper(namespace: &str) -> Vec<u8> {
    let mut k = Vec::with_capacity(namespace.len() + 1);
    k.extend_from_slice(namespace.as_bytes());
    k.push(1); // \x01 > \x00, captures everything in range
    k
}

/// Parse a state/snapshot key back into `(namespace, key)`.
fn parse_state_key(key: &[u8]) -> Option<(&str, &str)> {
    let pos = key.iter().position(|&b| b == 0)?;
    let ns = std::str::from_utf8(&key[..pos]).ok()?;
    let k = std::str::from_utf8(&key[pos + 1..]).ok()?;
    Some((ns, k))
}

/// Event key: `namespace \0 entity_id \0 sequence_be(8)`
fn event_key(namespace: &str, entity_id: &str, sequence: u64) -> Vec<u8> {
    let mut k = Vec::with_capacity(namespace.len() + 1 + entity_id.len() + 1 + 8);
    k.extend_from_slice(namespace.as_bytes());
    k.push(0);
    k.extend_from_slice(entity_id.as_bytes());
    k.push(0);
    k.extend_from_slice(&sequence.to_be_bytes());
    k
}

/// Parse an event key into `(namespace, entity_id, sequence)`.
fn parse_event_key(key: &[u8]) -> Option<(&str, &str, u64)> {
    // Last 8 bytes are sequence.
    if key.len() < 10 {
        return None;
    }
    let seq_bytes: [u8; 8] = key[key.len() - 8..].try_into().ok()?;
    let seq = u64::from_be_bytes(seq_bytes);
    // Remaining bytes (minus trailing \0 before seq): namespace \0 entity_id
    let prefix = &key[..key.len() - 9]; // strip \0 + 8 seq bytes
    let pos = prefix.iter().position(|&b| b == 0)?;
    let ns = std::str::from_utf8(&prefix[..pos]).ok()?;
    let eid = std::str::from_utf8(&prefix[pos + 1..]).ok()?;
    Some((ns, eid, seq))
}

/// Meta key for the next-sequence counter.
fn seq_meta_key(namespace: &str, entity_id: &str) -> Vec<u8> {
    let mut k = Vec::with_capacity(4 + namespace.len() + 1 + entity_id.len());
    k.extend_from_slice(b"seq:");
    k.extend_from_slice(namespace.as_bytes());
    k.push(0);
    k.extend_from_slice(entity_id.as_bytes());
    k
}

// ── Value encoding helpers ──────────────────────────────────────────

/// Encode an event value: `timestamp(8) + node_id_len(2) + node_id + data`
fn encode_event_value(timestamp: u64, node_id: &str, data: &[u8]) -> Vec<u8> {
    let nid = node_id.as_bytes();
    let mut v = Vec::with_capacity(10 + nid.len() + data.len());
    v.extend_from_slice(&timestamp.to_be_bytes());
    v.extend_from_slice(&(nid.len() as u16).to_be_bytes());
    v.extend_from_slice(nid);
    v.extend_from_slice(data);
    v
}

/// Decode an event value back into `(timestamp, node_id, data)`.
fn decode_event_value(value: &[u8]) -> Option<(u64, String, Vec<u8>)> {
    if value.len() < 10 {
        return None;
    }
    let timestamp = u64::from_be_bytes(value[..8].try_into().ok()?);
    let node_len = u16::from_be_bytes(value[8..10].try_into().ok()?) as usize;
    if value.len() < 10 + node_len {
        return None;
    }
    let node_id = String::from_utf8(value[10..10 + node_len].to_vec()).ok()?;
    let data = value[10 + node_len..].to_vec();
    Some((timestamp, node_id, data))
}

/// Encode a snapshot: `at_sequence(8) + version(1) + state`
fn encode_snapshot_value(at_sequence: u64, version: u8, state: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(9 + state.len());
    v.extend_from_slice(&at_sequence.to_be_bytes());
    v.push(version);
    v.extend_from_slice(state);
    v
}

/// Decode a snapshot value.
fn decode_snapshot_value(value: &[u8]) -> Option<Snapshot> {
    if value.len() < 9 {
        return None;
    }
    let at_sequence = u64::from_be_bytes(value[..8].try_into().ok()?);
    let version = value[8];
    let state = value[9..].to_vec();
    Some(Snapshot {
        state,
        at_sequence,
        version,
    })
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn new_store() -> RedbStore {
        RedbStore::open_in_memory().unwrap()
    }

    #[test]
    fn state_put_get_delete() {
        let mut store = new_store();
        store.put("ns", "k1", b"hello").unwrap();
        assert_eq!(store.get("ns", "k1").unwrap(), Some(b"hello".to_vec()));

        store.delete("ns", "k1").unwrap();
        assert_eq!(store.get("ns", "k1").unwrap(), None);
    }

    #[test]
    fn state_list_keys() {
        let mut store = new_store();
        store.put("ns", "b", b"2").unwrap();
        store.put("ns", "a", b"1").unwrap();
        store.put("ns", "c", b"3").unwrap();
        store.put("other", "x", b"4").unwrap();

        let keys = store.list_keys("ns").unwrap();
        assert_eq!(keys, vec!["a", "b", "c"]); // sorted by redb
    }

    #[test]
    fn state_exists() {
        let mut store = new_store();
        assert!(!store.exists("ns", "k1").unwrap());
        store.put("ns", "k1", b"val").unwrap();
        assert!(store.exists("ns", "k1").unwrap());
    }

    #[test]
    fn state_namespace_isolation() {
        let mut store = new_store();
        store.put("a", "k", b"1").unwrap();
        store.put("b", "k", b"2").unwrap();
        assert_eq!(store.get("a", "k").unwrap(), Some(b"1".to_vec()));
        assert_eq!(store.get("b", "k").unwrap(), Some(b"2".to_vec()));
    }

    #[test]
    fn event_append_and_read() {
        let mut store = new_store();
        let s1 = store
            .append_event("ns", "e1", b"op1", 1000, "node-a")
            .unwrap();
        let s2 = store
            .append_event("ns", "e1", b"op2", 2000, "node-b")
            .unwrap();
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);

        let events = store.events_since("ns", "e1", 0).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, 1);
        assert_eq!(events[0].timestamp, 1000);
        assert_eq!(events[0].node_id, "node-a");
        assert_eq!(events[0].data, b"op1");
        assert_eq!(events[1].sequence, 2);

        // events_since(1) → only seq 2
        let events = store.events_since("ns", "e1", 1).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 2);
    }

    #[test]
    fn event_count() {
        let mut store = new_store();
        assert_eq!(store.event_count("ns", "e1").unwrap(), 0);
        store.append_event("ns", "e1", b"a", 1, "n").unwrap();
        store.append_event("ns", "e1", b"b", 2, "n").unwrap();
        store.append_event("ns", "e1", b"c", 3, "n").unwrap();
        assert_eq!(store.event_count("ns", "e1").unwrap(), 3);
    }

    #[test]
    fn entity_isolation() {
        let mut store = new_store();
        store.append_event("ns", "e1", b"a", 1, "n").unwrap();
        store.append_event("ns", "e2", b"b", 2, "n").unwrap();
        assert_eq!(store.event_count("ns", "e1").unwrap(), 1);
        assert_eq!(store.event_count("ns", "e2").unwrap(), 1);
    }

    #[test]
    fn snapshot_save_load() {
        let mut store = new_store();
        assert!(store.load_snapshot("ns", "e1").unwrap().is_none());

        store
            .save_snapshot("ns", "e1", b"state-data", 42, 3)
            .unwrap();
        let snap = store.load_snapshot("ns", "e1").unwrap().unwrap();
        assert_eq!(snap.state, b"state-data");
        assert_eq!(snap.at_sequence, 42);
        assert_eq!(snap.version, 3);
    }

    #[test]
    fn truncate_events() {
        let mut store = new_store();
        for i in 0..5 {
            store
                .append_event("ns", "e1", &[i], (i + 1) as u64, "n")
                .unwrap();
        }
        assert_eq!(store.event_count("ns", "e1").unwrap(), 5);

        // Truncate events before seq 3 (removes seq 1 and 2).
        let removed = store.truncate_events_before("ns", "e1", 3).unwrap();
        assert_eq!(removed, 2);
        assert_eq!(store.event_count("ns", "e1").unwrap(), 3);

        let events = store.events_since("ns", "e1", 0).unwrap();
        assert_eq!(events[0].sequence, 3);
    }

    #[test]
    fn batch_put() {
        let mut store = new_store();
        store
            .put_batch("ns", &[("a", b"1"), ("b", b"2"), ("c", b"3")])
            .unwrap();
        assert_eq!(store.get("ns", "a").unwrap(), Some(b"1".to_vec()));
        assert_eq!(store.get("ns", "b").unwrap(), Some(b"2".to_vec()));
        assert_eq!(store.get("ns", "c").unwrap(), Some(b"3".to_vec()));
    }

    #[test]
    fn batch_put_overwrites() {
        let mut store = new_store();
        store.put("ns", "a", b"old").unwrap();
        store.put_batch("ns", &[("a", b"new")]).unwrap();
        assert_eq!(store.get("ns", "a").unwrap(), Some(b"new".to_vec()));
    }

    #[test]
    fn db_info_counts() {
        let mut store = new_store();
        store.put("sensors", "s1", b"a").unwrap();
        store.put("sensors", "s2", b"b").unwrap();
        store.put("users", "u1", b"c").unwrap();
        store.append_event("sensors", "s1", b"ev", 1, "n").unwrap();
        store.save_snapshot("sensors", "s1", b"snap", 1, 1).unwrap();

        let info = store.db_info().unwrap();
        assert_eq!(info.total_entities, 3);
        assert_eq!(info.total_events, 1);
        assert_eq!(info.total_snapshots, 1);
        assert_eq!(info.namespaces.len(), 2);

        let sensors = info
            .namespaces
            .iter()
            .find(|n| n.name == "sensors")
            .unwrap();
        assert_eq!(sensors.entity_count, 2);
        assert_eq!(sensors.event_count, 1);
        assert_eq!(sensors.snapshot_count, 1);
    }

    #[test]
    fn open_file_based() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.redb");
        {
            let mut store = RedbStore::open(&path).unwrap();
            store.put("ns", "k", b"value").unwrap();
        }
        // Reopen
        let store = RedbStore::open(&path).unwrap();
        assert_eq!(store.get("ns", "k").unwrap(), Some(b"value".to_vec()));
    }
}
