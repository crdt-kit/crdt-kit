use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use crate::traits::{EventStore, Snapshot, StateStore, StoredEvent};

/// In-memory storage backend.
///
/// All data is stored in `BTreeMap`s — nothing touches disk.
/// Ideal for testing and prototyping.
///
/// # Example
///
/// ```
/// use crdt_store::{MemoryStore, StateStore};
///
/// let mut store = MemoryStore::new();
/// store.put("sensors", "s1", b"temp=22.5").unwrap();
///
/// let data = store.get("sensors", "s1").unwrap().unwrap();
/// assert_eq!(data, b"temp=22.5");
/// ```
pub struct MemoryStore {
    /// State store: (namespace, key) -> value
    state: BTreeMap<(String, String), Vec<u8>>,
    /// Event log: (namespace, entity_id) -> sorted events
    events: BTreeMap<(String, String), Vec<StoredEvent>>,
    /// Snapshots: (namespace, entity_id) -> latest snapshot
    snapshots: BTreeMap<(String, String), Snapshot>,
    /// Global sequence counter
    next_sequence: u64,
}

/// Error type for the in-memory backend.
///
/// This backend never actually fails, but the trait requires an error type.
#[derive(Debug, Clone)]
pub struct MemoryError(String);

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MemoryStore error: {}", self.0)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MemoryError {}

impl MemoryStore {
    /// Create a new empty in-memory store.
    pub fn new() -> Self {
        Self {
            state: BTreeMap::new(),
            events: BTreeMap::new(),
            snapshots: BTreeMap::new(),
            next_sequence: 1,
        }
    }

    /// Returns the total number of state entries across all namespaces.
    pub fn state_count(&self) -> usize {
        self.state.len()
    }

    /// Returns the total number of events across all entities.
    pub fn total_event_count(&self) -> usize {
        self.events.values().map(|v| v.len()).sum()
    }

    fn ns_key(namespace: &str, key: &str) -> (String, String) {
        (namespace.to_string(), key.to_string())
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl StateStore for MemoryStore {
    type Error = MemoryError;

    fn put(&mut self, namespace: &str, key: &str, value: &[u8]) -> Result<(), Self::Error> {
        self.state
            .insert(Self::ns_key(namespace, key), value.to_vec());
        Ok(())
    }

    fn get(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.state.get(&Self::ns_key(namespace, key)).cloned())
    }

    fn delete(&mut self, namespace: &str, key: &str) -> Result<(), Self::Error> {
        self.state.remove(&Self::ns_key(namespace, key));
        Ok(())
    }

    fn list_keys(&self, namespace: &str) -> Result<Vec<String>, Self::Error> {
        let keys = self
            .state
            .keys()
            .filter(|(ns, _)| ns == namespace)
            .map(|(_, k)| k.clone())
            .collect();
        Ok(keys)
    }

    fn exists(&self, namespace: &str, key: &str) -> Result<bool, Self::Error> {
        Ok(self.state.contains_key(&Self::ns_key(namespace, key)))
    }
}

impl EventStore for MemoryStore {
    fn append_event(
        &mut self,
        namespace: &str,
        entity_id: &str,
        data: &[u8],
        timestamp: u64,
        node_id: &str,
    ) -> Result<u64, Self::Error> {
        let seq = self.next_sequence;
        self.next_sequence += 1;

        let event = StoredEvent {
            sequence: seq,
            entity_id: entity_id.to_string(),
            namespace: namespace.to_string(),
            data: data.to_vec(),
            timestamp,
            node_id: node_id.to_string(),
        };

        self.events
            .entry(Self::ns_key(namespace, entity_id))
            .or_default()
            .push(event);

        Ok(seq)
    }

    fn events_since(
        &self,
        namespace: &str,
        entity_id: &str,
        since_sequence: u64,
    ) -> Result<Vec<StoredEvent>, Self::Error> {
        let events = self
            .events
            .get(&Self::ns_key(namespace, entity_id))
            .map(|evts| {
                evts.iter()
                    .filter(|e| e.sequence > since_sequence)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        Ok(events)
    }

    fn event_count(&self, namespace: &str, entity_id: &str) -> Result<u64, Self::Error> {
        let count = self
            .events
            .get(&Self::ns_key(namespace, entity_id))
            .map(|evts| evts.len() as u64)
            .unwrap_or(0);
        Ok(count)
    }

    fn save_snapshot(
        &mut self,
        namespace: &str,
        entity_id: &str,
        state: &[u8],
        at_sequence: u64,
        version: u8,
    ) -> Result<(), Self::Error> {
        self.snapshots.insert(
            Self::ns_key(namespace, entity_id),
            Snapshot {
                state: state.to_vec(),
                at_sequence,
                version,
            },
        );
        Ok(())
    }

    fn load_snapshot(
        &self,
        namespace: &str,
        entity_id: &str,
    ) -> Result<Option<Snapshot>, Self::Error> {
        Ok(self
            .snapshots
            .get(&Self::ns_key(namespace, entity_id))
            .cloned())
    }

    fn truncate_events_before(
        &mut self,
        namespace: &str,
        entity_id: &str,
        before_sequence: u64,
    ) -> Result<u64, Self::Error> {
        let key = Self::ns_key(namespace, entity_id);
        if let Some(events) = self.events.get_mut(&key) {
            let before_len = events.len() as u64;
            events.retain(|e| e.sequence >= before_sequence);
            let after_len = events.len() as u64;
            Ok(before_len - after_len)
        } else {
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_put_get_delete() {
        let mut store = MemoryStore::new();

        store.put("ns", "k1", b"hello").unwrap();
        assert_eq!(store.get("ns", "k1").unwrap(), Some(b"hello".to_vec()));

        store.put("ns", "k1", b"world").unwrap();
        assert_eq!(store.get("ns", "k1").unwrap(), Some(b"world".to_vec()));

        store.delete("ns", "k1").unwrap();
        assert_eq!(store.get("ns", "k1").unwrap(), None);
    }

    #[test]
    fn state_namespace_isolation() {
        let mut store = MemoryStore::new();
        store.put("a", "k1", b"alpha").unwrap();
        store.put("b", "k1", b"beta").unwrap();

        assert_eq!(store.get("a", "k1").unwrap(), Some(b"alpha".to_vec()));
        assert_eq!(store.get("b", "k1").unwrap(), Some(b"beta".to_vec()));
    }

    #[test]
    fn state_list_keys() {
        let mut store = MemoryStore::new();
        store.put("ns", "a", b"1").unwrap();
        store.put("ns", "b", b"2").unwrap();
        store.put("other", "c", b"3").unwrap();

        let mut keys = store.list_keys("ns").unwrap();
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn state_exists() {
        let mut store = MemoryStore::new();
        assert!(!store.exists("ns", "k").unwrap());
        store.put("ns", "k", b"v").unwrap();
        assert!(store.exists("ns", "k").unwrap());
    }

    #[test]
    fn event_append_and_read() {
        let mut store = MemoryStore::new();

        let seq1 = store
            .append_event("ns", "e1", b"op1", 100, "node-a")
            .unwrap();
        let seq2 = store
            .append_event("ns", "e1", b"op2", 101, "node-a")
            .unwrap();
        let seq3 = store
            .append_event("ns", "e1", b"op3", 102, "node-b")
            .unwrap();

        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);
        assert_eq!(seq3, 3);

        // Read all events
        let events = store.events_since("ns", "e1", 0).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].data, b"op1");
        assert_eq!(events[2].node_id, "node-b");

        // Read events since seq 1
        let events = store.events_since("ns", "e1", 1).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, 2);
    }

    #[test]
    fn event_count() {
        let mut store = MemoryStore::new();
        assert_eq!(store.event_count("ns", "e1").unwrap(), 0);

        store.append_event("ns", "e1", b"op1", 100, "n").unwrap();
        store.append_event("ns", "e1", b"op2", 101, "n").unwrap();
        assert_eq!(store.event_count("ns", "e1").unwrap(), 2);

        // Different entity
        assert_eq!(store.event_count("ns", "e2").unwrap(), 0);
    }

    #[test]
    fn snapshot_save_load() {
        let mut store = MemoryStore::new();

        assert!(store.load_snapshot("ns", "e1").unwrap().is_none());

        store
            .save_snapshot("ns", "e1", b"state-data", 42, 2)
            .unwrap();

        let snap = store.load_snapshot("ns", "e1").unwrap().unwrap();
        assert_eq!(snap.state, b"state-data");
        assert_eq!(snap.at_sequence, 42);
        assert_eq!(snap.version, 2);
    }

    #[test]
    fn truncate_events() {
        let mut store = MemoryStore::new();
        store.append_event("ns", "e1", b"op1", 100, "n").unwrap();
        store.append_event("ns", "e1", b"op2", 101, "n").unwrap();
        store.append_event("ns", "e1", b"op3", 102, "n").unwrap();
        store.append_event("ns", "e1", b"op4", 103, "n").unwrap();

        let removed = store.truncate_events_before("ns", "e1", 3).unwrap();
        assert_eq!(removed, 2); // seq 1 and 2 removed

        let events = store.events_since("ns", "e1", 0).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, 3);
    }

    #[test]
    fn entity_isolation() {
        let mut store = MemoryStore::new();
        store.append_event("ns", "e1", b"op1", 100, "n").unwrap();
        store.append_event("ns", "e2", b"op2", 101, "n").unwrap();

        assert_eq!(store.event_count("ns", "e1").unwrap(), 1);
        assert_eq!(store.event_count("ns", "e2").unwrap(), 1);
    }
}
