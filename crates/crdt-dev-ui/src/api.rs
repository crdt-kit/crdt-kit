use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    Json,
};
use crdt_migrate::VersionedEnvelope;
use crdt_store::{EventStore, SqliteStore, StateStore};
use serde::Serialize;

use crate::AppState;

const INDEX_HTML: &str = include_str!("../static/index.html");

// ── Page handler ────────────────────────────────────────────────────

pub async fn index() -> impl IntoResponse {
    ([(header::CACHE_CONTROL, "no-cache")], Html(INDEX_HTML))
}

// ── API types ───────────────────────────────────────────────────────

#[derive(Serialize)]
struct StatusResponse {
    database: String,
    size_bytes: u64,
    size_human: String,
    journal_mode: String,
    total_entities: u64,
    total_events: u64,
    total_snapshots: u64,
    namespaces: Vec<NamespaceEntry>,
}

#[derive(Serialize)]
struct NamespaceEntry {
    name: String,
    entity_count: u64,
    event_count: u64,
    snapshot_count: u64,
}

#[derive(Serialize)]
struct EntityEntry {
    key: String,
    size: usize,
    version: Option<u8>,
    crdt_type: Option<String>,
}

#[derive(Serialize)]
struct EntityDetail {
    key: String,
    namespace: String,
    size: usize,
    version: Option<u8>,
    crdt_type: Option<String>,
    event_count: u64,
    snapshot: Option<SnapshotInfo>,
}

#[derive(Serialize)]
struct SnapshotInfo {
    at_sequence: u64,
    version: u8,
    size: usize,
}

#[derive(Serialize)]
struct EventEntry {
    sequence: u64,
    timestamp: u64,
    node_id: String,
    data_size: usize,
}

#[derive(Serialize)]
struct EventsResponse {
    total: u64,
    showing: usize,
    events: Vec<EventEntry>,
}

#[derive(serde::Deserialize)]
pub struct EventsQuery {
    last: Option<usize>,
}

// ── API handlers ────────────────────────────────────────────────────

pub async fn status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match build_status(&state) {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn list_entities(
    State(state): State<Arc<AppState>>,
    Path(ns): Path<String>,
) -> impl IntoResponse {
    match build_entity_list(&state.store, &ns) {
        Ok(entries) => Json(entries).into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn get_entity(
    State(state): State<Arc<AppState>>,
    Path((ns, key)): Path<(String, String)>,
) -> impl IntoResponse {
    match build_entity_detail(&state.store, &ns, &key) {
        Ok(Some(detail)) => Json(detail).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "entity not found"})),
        )
            .into_response(),
        Err(e) => error_response(e),
    }
}

pub async fn get_events(
    State(state): State<Arc<AppState>>,
    Path((ns, key)): Path<(String, String)>,
    Query(query): Query<EventsQuery>,
) -> impl IntoResponse {
    let last = query.last.unwrap_or(50);
    match build_events(&state.store, &ns, &key, last) {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => error_response(e),
    }
}

// ── Builders (sync, run on the store) ───────────────────────────────

fn build_status(state: &AppState) -> Result<StatusResponse, String> {
    let info = state.store.db_info().map_err(|e| e.to_string())?;
    let size_bytes = state.store.file_size().unwrap_or(0);
    let journal_mode = state.store.journal_mode().unwrap_or_default();

    Ok(StatusResponse {
        database: state.db_path.clone(),
        size_bytes,
        size_human: format_bytes(size_bytes),
        journal_mode,
        total_entities: info.total_entities,
        total_events: info.total_events,
        total_snapshots: info.total_snapshots,
        namespaces: info
            .namespaces
            .into_iter()
            .map(|ns| NamespaceEntry {
                name: ns.name,
                entity_count: ns.entity_count,
                event_count: ns.event_count,
                snapshot_count: ns.snapshot_count,
            })
            .collect(),
    })
}

fn build_entity_list(store: &SqliteStore, namespace: &str) -> Result<Vec<EntityEntry>, String> {
    let keys = store.list_keys(namespace).map_err(|e| e.to_string())?;
    let mut entries = Vec::with_capacity(keys.len());

    for key in &keys {
        let data = store.get(namespace, key).map_err(|e| e.to_string())?;
        let (size, version, crdt_type) = match data {
            Some(d) => {
                let size = d.len();
                if VersionedEnvelope::is_versioned(&d) {
                    match VersionedEnvelope::from_bytes(&d) {
                        Ok(env) => (
                            size,
                            Some(env.version),
                            Some(format!("{:?}", env.crdt_type)),
                        ),
                        Err(_) => (size, None, None),
                    }
                } else {
                    (size, None, None)
                }
            }
            None => (0, None, None),
        };
        entries.push(EntityEntry {
            key: key.clone(),
            size,
            version,
            crdt_type,
        });
    }

    Ok(entries)
}

fn build_entity_detail(
    store: &SqliteStore,
    namespace: &str,
    key: &str,
) -> Result<Option<EntityDetail>, String> {
    let data = store.get(namespace, key).map_err(|e| e.to_string())?;
    let data = match data {
        Some(d) => d,
        None => return Ok(None),
    };

    let size = data.len();
    let (version, crdt_type) = if VersionedEnvelope::is_versioned(&data) {
        match VersionedEnvelope::from_bytes(&data) {
            Ok(env) => (Some(env.version), Some(format!("{:?}", env.crdt_type))),
            Err(_) => (None, None),
        }
    } else {
        (None, None)
    };

    let event_count = store
        .event_count(namespace, key)
        .map_err(|e| e.to_string())?;

    let snapshot = store
        .load_snapshot(namespace, key)
        .map_err(|e| e.to_string())?
        .map(|s| SnapshotInfo {
            at_sequence: s.at_sequence,
            version: s.version,
            size: s.state.len(),
        });

    Ok(Some(EntityDetail {
        key: key.to_string(),
        namespace: namespace.to_string(),
        size,
        version,
        crdt_type,
        event_count,
        snapshot,
    }))
}

fn build_events(
    store: &SqliteStore,
    namespace: &str,
    key: &str,
    last: usize,
) -> Result<EventsResponse, String> {
    let total = store
        .event_count(namespace, key)
        .map_err(|e| e.to_string())?;
    let all_events = store
        .events_since(namespace, key, 0)
        .map_err(|e| e.to_string())?;

    let events: Vec<_> = if all_events.len() > last {
        all_events[all_events.len() - last..].to_vec()
    } else {
        all_events
    };

    let entries: Vec<EventEntry> = events
        .iter()
        .map(|e| EventEntry {
            sequence: e.sequence,
            timestamp: e.timestamp,
            node_id: e.node_id.clone(),
            data_size: e.data.len(),
        })
        .collect();

    Ok(EventsResponse {
        total,
        showing: entries.len(),
        events: entries,
    })
}

// ── Helpers ─────────────────────────────────────────────────────────

fn error_response(msg: String) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": msg})),
    )
        .into_response()
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
