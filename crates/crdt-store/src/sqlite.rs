//! SQLite persistence backend using rusqlite.
//!
//! This is the primary backend for edge, mobile, and desktop applications.
//! Uses WAL mode by default for concurrent read/write performance.
//!
//! # Example
//!
//! ```no_run
//! use crdt_store::{SqliteStore, StateStore, EventStore};
//!
//! let mut store = SqliteStore::open("my_app.db").unwrap();
//! store.put("sensors", "s1", b"temp=22.5").unwrap();
//!
//! let data = store.get("sensors", "s1").unwrap().unwrap();
//! assert_eq!(data, b"temp=22.5");
//! ```

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::traits::{BatchOps, EventStore, Snapshot, StateStore, StoredEvent, Transactional};

/// SQLite configuration options.
#[derive(Debug, Clone)]
pub struct SqliteConfig {
    /// SQLite journal mode. Defaults to WAL.
    pub journal_mode: JournalMode,
    /// Busy timeout in milliseconds. Defaults to 5000.
    pub busy_timeout_ms: u32,
    /// SQLite page size. Defaults to 4096.
    pub page_size: u32,
}

impl Default for SqliteConfig {
    fn default() -> Self {
        Self {
            journal_mode: JournalMode::Wal,
            busy_timeout_ms: 5000,
            page_size: 4096,
        }
    }
}

/// SQLite journal mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalMode {
    /// Write-Ahead Logging — allows concurrent reads during writes.
    Wal,
    /// Traditional rollback journal.
    Delete,
    /// In-memory journal (fastest, no crash recovery).
    Memory,
}

impl JournalMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Wal => "WAL",
            Self::Delete => "DELETE",
            Self::Memory => "MEMORY",
        }
    }
}

/// Error type for the SQLite backend.
#[derive(Debug)]
pub enum SqliteError {
    /// An error from rusqlite.
    Sqlite(rusqlite::Error),
    /// Lock poisoned.
    LockPoisoned,
}

impl std::fmt::Display for SqliteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "sqlite error: {e}"),
            Self::LockPoisoned => write!(f, "sqlite lock poisoned"),
        }
    }
}

impl std::error::Error for SqliteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(e) => Some(e),
            Self::LockPoisoned => None,
        }
    }
}

impl From<rusqlite::Error> for SqliteError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

/// SQLite persistence backend.
///
/// Wraps a `rusqlite::Connection` behind a `Mutex` for safe shared access.
/// Creates the schema automatically on first open.
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// Open (or create) a SQLite database at the given path with default config.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SqliteError> {
        Self::open_with_config(path, SqliteConfig::default())
    }

    /// Open with custom configuration.
    pub fn open_with_config<P: AsRef<Path>>(
        path: P,
        config: SqliteConfig,
    ) -> Result<Self, SqliteError> {
        let conn = Connection::open(path)?;
        Self::init_connection(&conn, &config)?;
        Self::create_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory database (useful for testing).
    pub fn open_in_memory() -> Result<Self, SqliteError> {
        let conn = Connection::open_in_memory()?;
        Self::init_connection(&conn, &SqliteConfig::default())?;
        Self::create_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn init_connection(conn: &Connection, config: &SqliteConfig) -> Result<(), SqliteError> {
        conn.execute_batch(&format!(
            "PRAGMA journal_mode = {};
             PRAGMA busy_timeout = {};
             PRAGMA page_size = {};
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;",
            config.journal_mode.as_str(),
            config.busy_timeout_ms,
            config.page_size,
        ))?;
        Ok(())
    }

    fn create_schema(conn: &Connection) -> Result<(), SqliteError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS crdt_state (
                namespace   TEXT NOT NULL,
                key         TEXT NOT NULL,
                data        BLOB NOT NULL,
                version     INTEGER NOT NULL DEFAULT 0,
                updated_at  INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (namespace, key)
            );

            CREATE TABLE IF NOT EXISTS crdt_events (
                sequence    INTEGER PRIMARY KEY AUTOINCREMENT,
                namespace   TEXT NOT NULL,
                entity_id   TEXT NOT NULL,
                data        BLOB NOT NULL,
                timestamp   INTEGER NOT NULL,
                node_id     TEXT NOT NULL,
                created_at  INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );

            CREATE INDEX IF NOT EXISTS idx_events_entity
                ON crdt_events(namespace, entity_id, sequence);

            CREATE TABLE IF NOT EXISTS crdt_snapshots (
                namespace   TEXT NOT NULL,
                entity_id   TEXT NOT NULL,
                data        BLOB NOT NULL,
                at_sequence INTEGER NOT NULL,
                version     INTEGER NOT NULL DEFAULT 0,
                created_at  INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                PRIMARY KEY (namespace, entity_id)
            );",
        )?;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, SqliteError> {
        self.conn.lock().map_err(|_| SqliteError::LockPoisoned)
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Get summary information about the database.
    pub fn db_info(&self) -> Result<crate::DbInfo, SqliteError> {
        let conn = self.lock()?;

        // Get namespaces from state table
        let mut ns_stmt =
            conn.prepare("SELECT DISTINCT namespace FROM crdt_state ORDER BY namespace")?;
        let namespaces: Vec<String> = ns_stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;

        let mut ns_infos = Vec::new();
        let mut total_entities = 0u64;
        let mut total_events = 0u64;
        let mut total_snapshots = 0u64;

        for ns in &namespaces {
            let entity_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM crdt_state WHERE namespace = ?1",
                params![ns],
                |row| row.get(0),
            )?;

            let event_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM crdt_events WHERE namespace = ?1",
                params![ns],
                |row| row.get(0),
            )?;

            let snapshot_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM crdt_snapshots WHERE namespace = ?1",
                params![ns],
                |row| row.get(0),
            )?;

            total_entities += entity_count as u64;
            total_events += event_count as u64;
            total_snapshots += snapshot_count as u64;

            ns_infos.push(crate::NamespaceInfo {
                name: ns.clone(),
                entity_count: entity_count as u64,
                event_count: event_count as u64,
                snapshot_count: snapshot_count as u64,
            });
        }

        Ok(crate::DbInfo {
            total_entities,
            total_events,
            total_snapshots,
            namespaces: ns_infos,
        })
    }

    /// Get the database file size in bytes (0 for in-memory).
    pub fn file_size(&self) -> Result<u64, SqliteError> {
        let conn = self.lock()?;
        let page_count: i64 = conn.query_row("PRAGMA page_count", [], |row| row.get(0))?;
        let page_size: i64 = conn.query_row("PRAGMA page_size", [], |row| row.get(0))?;
        Ok((page_count * page_size) as u64)
    }

    /// Get the current journal mode.
    pub fn journal_mode(&self) -> Result<String, SqliteError> {
        let conn = self.lock()?;
        let mode: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
        Ok(mode)
    }
}

impl StateStore for SqliteStore {
    type Error = SqliteError;

    fn put(&mut self, namespace: &str, key: &str, value: &[u8]) -> Result<(), Self::Error> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO crdt_state (namespace, key, data, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(namespace, key)
             DO UPDATE SET data = excluded.data, updated_at = excluded.updated_at",
            params![namespace, key, value, Self::now_ms() as i64],
        )?;
        Ok(())
    }

    fn get(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
        let conn = self.lock()?;
        let result = conn
            .query_row(
                "SELECT data FROM crdt_state WHERE namespace = ?1 AND key = ?2",
                params![namespace, key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(result)
    }

    fn delete(&mut self, namespace: &str, key: &str) -> Result<(), Self::Error> {
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM crdt_state WHERE namespace = ?1 AND key = ?2",
            params![namespace, key],
        )?;
        Ok(())
    }

    fn list_keys(&self, namespace: &str) -> Result<Vec<String>, Self::Error> {
        let conn = self.lock()?;
        let mut stmt =
            conn.prepare("SELECT key FROM crdt_state WHERE namespace = ?1 ORDER BY key")?;
        let keys = stmt
            .query_map(params![namespace], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(keys)
    }

    fn exists(&self, namespace: &str, key: &str) -> Result<bool, Self::Error> {
        let conn = self.lock()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM crdt_state WHERE namespace = ?1 AND key = ?2",
            params![namespace, key],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

impl EventStore for SqliteStore {
    fn append_event(
        &mut self,
        namespace: &str,
        entity_id: &str,
        data: &[u8],
        timestamp: u64,
        node_id: &str,
    ) -> Result<u64, Self::Error> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO crdt_events (namespace, entity_id, data, timestamp, node_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![namespace, entity_id, data, timestamp as i64, node_id],
        )?;
        Ok(conn.last_insert_rowid() as u64)
    }

    fn events_since(
        &self,
        namespace: &str,
        entity_id: &str,
        since_sequence: u64,
    ) -> Result<Vec<StoredEvent>, Self::Error> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT sequence, namespace, entity_id, data, timestamp, node_id
             FROM crdt_events
             WHERE namespace = ?1 AND entity_id = ?2 AND sequence > ?3
             ORDER BY sequence",
        )?;
        let events = stmt
            .query_map(
                params![namespace, entity_id, since_sequence as i64],
                |row| {
                    Ok(StoredEvent {
                        sequence: row.get::<_, i64>(0)? as u64,
                        namespace: row.get(1)?,
                        entity_id: row.get(2)?,
                        data: row.get(3)?,
                        timestamp: row.get::<_, i64>(4)? as u64,
                        node_id: row.get(5)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(events)
    }

    fn event_count(&self, namespace: &str, entity_id: &str) -> Result<u64, Self::Error> {
        let conn = self.lock()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM crdt_events WHERE namespace = ?1 AND entity_id = ?2",
            params![namespace, entity_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    fn save_snapshot(
        &mut self,
        namespace: &str,
        entity_id: &str,
        state: &[u8],
        at_sequence: u64,
        version: u8,
    ) -> Result<(), Self::Error> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO crdt_snapshots (namespace, entity_id, data, at_sequence, version)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(namespace, entity_id)
             DO UPDATE SET data = excluded.data,
                           at_sequence = excluded.at_sequence,
                           version = excluded.version,
                           created_at = strftime('%s','now')",
            params![
                namespace,
                entity_id,
                state,
                at_sequence as i64,
                version as i64
            ],
        )?;
        Ok(())
    }

    fn load_snapshot(
        &self,
        namespace: &str,
        entity_id: &str,
    ) -> Result<Option<Snapshot>, Self::Error> {
        let conn = self.lock()?;
        let result = conn
            .query_row(
                "SELECT data, at_sequence, version FROM crdt_snapshots
                 WHERE namespace = ?1 AND entity_id = ?2",
                params![namespace, entity_id],
                |row| {
                    Ok(Snapshot {
                        state: row.get(0)?,
                        at_sequence: row.get::<_, i64>(1)? as u64,
                        version: row.get::<_, i64>(2)? as u8,
                    })
                },
            )
            .optional()?;
        Ok(result)
    }

    fn truncate_events_before(
        &mut self,
        namespace: &str,
        entity_id: &str,
        before_sequence: u64,
    ) -> Result<u64, Self::Error> {
        let conn = self.lock()?;
        let deleted = conn.execute(
            "DELETE FROM crdt_events
             WHERE namespace = ?1 AND entity_id = ?2 AND sequence < ?3",
            params![namespace, entity_id, before_sequence as i64],
        )?;
        Ok(deleted as u64)
    }
}

impl Transactional for SqliteStore {
    fn transaction<F, R>(&mut self, f: F) -> Result<R, Self::Error>
    where
        F: FnOnce(&mut Self) -> Result<R, Self::Error>,
    {
        {
            let conn = self.lock()?;
            conn.execute_batch("BEGIN")?;
        }
        match f(self) {
            Ok(result) => {
                let conn = self.lock()?;
                conn.execute_batch("COMMIT")?;
                Ok(result)
            }
            Err(e) => {
                if let Ok(conn) = self.lock() {
                    let _ = conn.execute_batch("ROLLBACK");
                }
                Err(e)
            }
        }
    }
}

impl BatchOps for SqliteStore {
    fn put_batch(&mut self, namespace: &str, entries: &[(&str, &[u8])]) -> Result<(), Self::Error> {
        let conn = self.lock()?;
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO crdt_state (namespace, key, data, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(namespace, key)
                 DO UPDATE SET data = excluded.data, updated_at = excluded.updated_at",
            )?;
            let now = Self::now_ms() as i64;
            for (key, value) in entries {
                stmt.execute(params![namespace, key, value, now])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> SqliteStore {
        SqliteStore::open_in_memory().unwrap()
    }

    #[test]
    fn state_put_get_delete() {
        let mut store = test_store();

        store.put("ns", "k1", b"hello").unwrap();
        assert_eq!(store.get("ns", "k1").unwrap(), Some(b"hello".to_vec()));

        store.put("ns", "k1", b"world").unwrap();
        assert_eq!(store.get("ns", "k1").unwrap(), Some(b"world".to_vec()));

        store.delete("ns", "k1").unwrap();
        assert_eq!(store.get("ns", "k1").unwrap(), None);
    }

    #[test]
    fn state_namespace_isolation() {
        let mut store = test_store();
        store.put("a", "k1", b"alpha").unwrap();
        store.put("b", "k1", b"beta").unwrap();

        assert_eq!(store.get("a", "k1").unwrap(), Some(b"alpha".to_vec()));
        assert_eq!(store.get("b", "k1").unwrap(), Some(b"beta".to_vec()));
    }

    #[test]
    fn state_list_keys() {
        let mut store = test_store();
        store.put("ns", "b", b"2").unwrap();
        store.put("ns", "a", b"1").unwrap();
        store.put("other", "c", b"3").unwrap();

        let keys = store.list_keys("ns").unwrap();
        assert_eq!(keys, vec!["a", "b"]); // sorted
    }

    #[test]
    fn state_exists() {
        let mut store = test_store();
        assert!(!store.exists("ns", "k").unwrap());
        store.put("ns", "k", b"v").unwrap();
        assert!(store.exists("ns", "k").unwrap());
    }

    #[test]
    fn event_append_and_read() {
        let mut store = test_store();

        let seq1 = store
            .append_event("ns", "e1", b"op1", 100, "node-a")
            .unwrap();
        let seq2 = store
            .append_event("ns", "e1", b"op2", 101, "node-a")
            .unwrap();
        let seq3 = store
            .append_event("ns", "e1", b"op3", 102, "node-b")
            .unwrap();

        assert!(seq1 < seq2);
        assert!(seq2 < seq3);

        // Read all events
        let events = store.events_since("ns", "e1", 0).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].data, b"op1");
        assert_eq!(events[2].node_id, "node-b");

        // Read events since seq1
        let events = store.events_since("ns", "e1", seq1).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, seq2);
    }

    #[test]
    fn event_count() {
        let mut store = test_store();
        assert_eq!(store.event_count("ns", "e1").unwrap(), 0);

        store.append_event("ns", "e1", b"op1", 100, "n").unwrap();
        store.append_event("ns", "e1", b"op2", 101, "n").unwrap();
        assert_eq!(store.event_count("ns", "e1").unwrap(), 2);

        // Different entity
        assert_eq!(store.event_count("ns", "e2").unwrap(), 0);
    }

    #[test]
    fn snapshot_save_load() {
        let mut store = test_store();

        assert!(store.load_snapshot("ns", "e1").unwrap().is_none());

        store
            .save_snapshot("ns", "e1", b"state-data", 42, 2)
            .unwrap();

        let snap = store.load_snapshot("ns", "e1").unwrap().unwrap();
        assert_eq!(snap.state, b"state-data");
        assert_eq!(snap.at_sequence, 42);
        assert_eq!(snap.version, 2);

        // Overwrite snapshot
        store
            .save_snapshot("ns", "e1", b"new-state", 100, 3)
            .unwrap();
        let snap = store.load_snapshot("ns", "e1").unwrap().unwrap();
        assert_eq!(snap.state, b"new-state");
        assert_eq!(snap.at_sequence, 100);
    }

    #[test]
    fn truncate_events() {
        let mut store = test_store();
        let s1 = store.append_event("ns", "e1", b"op1", 100, "n").unwrap();
        let _s2 = store.append_event("ns", "e1", b"op2", 101, "n").unwrap();
        let s3 = store.append_event("ns", "e1", b"op3", 102, "n").unwrap();
        let _s4 = store.append_event("ns", "e1", b"op4", 103, "n").unwrap();

        // Truncate events before s3
        let removed = store.truncate_events_before("ns", "e1", s3).unwrap();
        assert_eq!(removed, 2); // s1 and s2 removed

        let events = store.events_since("ns", "e1", 0).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, s3);

        // Truncating nothing
        let removed = store.truncate_events_before("ns", "e1", s1).unwrap();
        assert_eq!(removed, 0);
    }

    #[test]
    fn entity_isolation() {
        let mut store = test_store();
        store.append_event("ns", "e1", b"op1", 100, "n").unwrap();
        store.append_event("ns", "e2", b"op2", 101, "n").unwrap();

        assert_eq!(store.event_count("ns", "e1").unwrap(), 1);
        assert_eq!(store.event_count("ns", "e2").unwrap(), 1);
    }

    #[test]
    fn batch_put() {
        let mut store = test_store();

        store
            .put_batch("ns", &[("a", b"1"), ("b", b"2"), ("c", b"3")])
            .unwrap();

        assert_eq!(store.get("ns", "a").unwrap(), Some(b"1".to_vec()));
        assert_eq!(store.get("ns", "b").unwrap(), Some(b"2".to_vec()));
        assert_eq!(store.get("ns", "c").unwrap(), Some(b"3".to_vec()));
        assert_eq!(store.list_keys("ns").unwrap().len(), 3);
    }

    #[test]
    fn batch_put_overwrites() {
        let mut store = test_store();
        store.put("ns", "a", b"old").unwrap();

        store
            .put_batch("ns", &[("a", b"new"), ("b", b"fresh")])
            .unwrap();

        assert_eq!(store.get("ns", "a").unwrap(), Some(b"new".to_vec()));
        assert_eq!(store.get("ns", "b").unwrap(), Some(b"fresh".to_vec()));
    }

    #[test]
    fn transaction_commit() {
        let mut store = test_store();

        store
            .transaction(|s| {
                s.put("ns", "k1", b"v1")?;
                s.put("ns", "k2", b"v2")?;
                Ok(())
            })
            .unwrap();

        assert_eq!(store.get("ns", "k1").unwrap(), Some(b"v1".to_vec()));
        assert_eq!(store.get("ns", "k2").unwrap(), Some(b"v2".to_vec()));
    }

    #[test]
    fn transaction_rollback() {
        let mut store = test_store();
        store.put("ns", "k1", b"original").unwrap();

        let result: Result<(), SqliteError> = store.transaction(|s| {
            s.put("ns", "k1", b"modified")?;
            Err(SqliteError::LockPoisoned) // simulate error
        });

        assert!(result.is_err());
        assert_eq!(store.get("ns", "k1").unwrap(), Some(b"original".to_vec()));
    }

    #[test]
    fn open_file_based() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        {
            let mut store = SqliteStore::open(&db_path).unwrap();
            store.put("ns", "k1", b"persist").unwrap();
        }

        // Reopen and verify data persisted
        let store = SqliteStore::open(&db_path).unwrap();
        assert_eq!(store.get("ns", "k1").unwrap(), Some(b"persist".to_vec()));
    }

    #[test]
    fn wal_mode_enabled() {
        let store = test_store();
        let conn = store.lock().unwrap();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        // In-memory databases may report "memory" instead of "wal"
        assert!(mode == "wal" || mode == "memory", "got: {mode}");
    }
}
