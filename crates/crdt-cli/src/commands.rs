use crdt_migrate::VersionedEnvelope;
use crdt_store::{EventStore, SqliteStore, StateStore};
use serde_json::json;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

/// `crdt status <db>` — Show database status and statistics.
pub fn status(db_path: &str) -> Result {
    let store = SqliteStore::open(db_path)?;
    let info = store.db_info()?;
    let size = store.file_size()?;
    let journal = store.journal_mode()?;

    println!("Database: {db_path} (SQLite, {journal} mode)");
    println!("Size: {}", format_bytes(size));
    println!();

    if info.namespaces.is_empty() {
        println!("  (empty database)");
        return Ok(());
    }

    // Header
    println!(
        "  {:<20} {:>10} {:>10} {:>10}",
        "Namespace", "Entities", "Events", "Snapshots"
    );
    println!("  {}", "-".repeat(54));

    for ns in &info.namespaces {
        println!(
            "  {:<20} {:>10} {:>10} {:>10}",
            ns.name,
            format_num(ns.entity_count),
            format_num(ns.event_count),
            format_num(ns.snapshot_count),
        );
    }

    println!("  {}", "-".repeat(54));
    println!(
        "  {:<20} {:>10} {:>10} {:>10}",
        "Total",
        format_num(info.total_entities),
        format_num(info.total_events),
        format_num(info.total_snapshots),
    );
    println!();

    Ok(())
}

/// `crdt inspect <db> [key]` — Inspect entities or list namespaces.
pub fn inspect(
    db_path: &str,
    key: Option<&str>,
    namespace: Option<&str>,
    show_events: bool,
    last: usize,
) -> Result {
    let store = SqliteStore::open(db_path)?;

    match key {
        Some(key) => inspect_entity(&store, key, namespace, show_events, last),
        None => inspect_list(&store, namespace),
    }
}

fn inspect_entity(
    store: &SqliteStore,
    key: &str,
    namespace: Option<&str>,
    show_events: bool,
    last: usize,
) -> Result {
    // If namespace given, look there. Otherwise search all namespaces.
    let namespaces: Vec<String> = if let Some(ns) = namespace {
        vec![ns.to_string()]
    } else {
        let info = store.db_info()?;
        info.namespaces.into_iter().map(|n| n.name).collect()
    };

    let mut found = false;

    for ns in &namespaces {
        if let Some(data) = store.get(ns, key)? {
            found = true;
            println!("Entity: {key}");
            println!("Namespace: {ns}");
            println!("Size: {} bytes", data.len());

            if VersionedEnvelope::is_versioned(&data) {
                if let Ok(env) = VersionedEnvelope::from_bytes(&data) {
                    println!("Version: v{}", env.version);
                    println!("CRDT type: {:?}", env.crdt_type);
                    println!("Payload: {} bytes", env.payload.len());
                }
            } else {
                println!("Version: (unversioned)");
            }
            println!();

            if show_events {
                inspect_events(store, ns, key, last)?;
            }
            break;
        }
    }

    if !found {
        eprintln!("Entity '{key}' not found");
    }

    Ok(())
}

fn inspect_events(store: &SqliteStore, namespace: &str, entity_id: &str, last: usize) -> Result {
    let all_events = store.events_since(namespace, entity_id, 0)?;
    let total = all_events.len();

    if total == 0 {
        println!("  (no events)");
        return Ok(());
    }

    let events: Vec<_> = if total > last {
        all_events[total - last..].to_vec()
    } else {
        all_events
    };

    println!(
        "Event log ({} of {} events):",
        events.len(),
        format_num(total as u64)
    );
    println!(
        "  {:>8}  {:>14}  {:<16}  {:>10}",
        "Seq", "Timestamp", "Node", "Size"
    );
    println!("  {}", "-".repeat(56));

    for ev in &events {
        println!(
            "  {:>8}  {:>14}  {:<16}  {:>7} B",
            ev.sequence,
            ev.timestamp,
            truncate(&ev.node_id, 16),
            ev.data.len(),
        );
    }

    // Snapshot info
    if let Some(snap) = store.load_snapshot(namespace, entity_id)? {
        println!();
        println!(
            "Snapshot: at seq {}, v{}, {} bytes",
            snap.at_sequence,
            snap.version,
            snap.state.len()
        );
    }

    println!();
    Ok(())
}

fn inspect_list(store: &SqliteStore, namespace: Option<&str>) -> Result {
    let info = store.db_info()?;

    if info.namespaces.is_empty() {
        println!("  (empty database)");
        return Ok(());
    }

    for ns in &info.namespaces {
        if let Some(filter) = namespace {
            if ns.name != filter {
                continue;
            }
        }

        println!("Namespace: {} ({} entities)", ns.name, ns.entity_count);

        let keys = store.list_keys(&ns.name)?;
        for key in &keys {
            let size = store.get(&ns.name, key)?.map(|d| d.len()).unwrap_or(0);
            println!("  {key:<40} {size:>8} B");
        }
        println!();
    }

    Ok(())
}

/// `crdt compact <db>` — Compact event logs.
pub fn compact(db_path: &str, namespace: Option<&str>, threshold: u64) -> Result {
    let mut store = SqliteStore::open(db_path)?;
    let info = store.db_info()?;

    let mut total_removed = 0u64;
    let mut compacted = 0u64;

    for ns in &info.namespaces {
        if let Some(filter) = namespace {
            if ns.name != filter {
                continue;
            }
        }

        // Get all entity IDs in this namespace that have events
        let keys = store.list_keys(&ns.name)?;

        for key in &keys {
            let count = store.event_count(&ns.name, key)?;
            if count < threshold {
                continue;
            }

            // Get current state as snapshot data
            let state = store.get(&ns.name, key)?.unwrap_or_default();

            // Get max sequence
            let events = store.events_since(&ns.name, key, 0)?;
            let max_seq = events.last().map(|e| e.sequence).unwrap_or(0);

            if max_seq == 0 {
                continue;
            }

            // Determine version from envelope
            let version = if VersionedEnvelope::is_versioned(&state) {
                VersionedEnvelope::peek_version(&state).unwrap_or(1)
            } else {
                1
            };

            // Save snapshot
            store.save_snapshot(&ns.name, key, &state, max_seq, version)?;

            // Truncate events before max_seq (keep latest)
            let removed = store.truncate_events_before(&ns.name, key, max_seq)?;

            if removed > 0 {
                println!(
                    "  {}/{}: {} events -> snapshot + {} events",
                    ns.name,
                    key,
                    format_num(count),
                    format_num(count - removed)
                );
                total_removed += removed;
                compacted += 1;
            }
        }
    }

    if compacted == 0 {
        println!("Nothing to compact (threshold: {threshold} events)");
    } else {
        println!();
        println!(
            "Compacted {compacted} entities, removed {} events",
            format_num(total_removed)
        );
    }

    Ok(())
}

/// `crdt export <db>` — Export data as JSON.
pub fn export(db_path: &str, key: Option<&str>, namespace: Option<&str>) -> Result {
    let store = SqliteStore::open(db_path)?;

    match key {
        Some(key) => export_entity(&store, key, namespace),
        None => export_namespace(&store, namespace),
    }
}

fn export_entity(store: &SqliteStore, key: &str, namespace: Option<&str>) -> Result {
    let namespaces: Vec<String> = if let Some(ns) = namespace {
        vec![ns.to_string()]
    } else {
        let info = store.db_info()?;
        info.namespaces.into_iter().map(|n| n.name).collect()
    };

    for ns in &namespaces {
        if let Some(data) = store.get(ns, key)? {
            let entry = if VersionedEnvelope::is_versioned(&data) {
                let env = VersionedEnvelope::from_bytes(&data)
                    .map_err(|e| format!("envelope error: {e}"))?;
                json!({
                    "namespace": ns,
                    "key": key,
                    "version": env.version,
                    "crdt_type": format!("{:?}", env.crdt_type),
                    "payload_size": env.payload.len(),
                    "payload_base64": base64_encode(&env.payload),
                })
            } else {
                json!({
                    "namespace": ns,
                    "key": key,
                    "version": null,
                    "raw_size": data.len(),
                    "raw_base64": base64_encode(&data),
                })
            };

            // Export events too
            let events = store.events_since(ns, key, 0)?;
            let events_json: Vec<_> = events
                .iter()
                .map(|e| {
                    json!({
                        "sequence": e.sequence,
                        "timestamp": e.timestamp,
                        "node_id": e.node_id,
                        "data_size": e.data.len(),
                        "data_base64": base64_encode(&e.data),
                    })
                })
                .collect();

            let output = json!({
                "entity": entry,
                "events": events_json,
            });

            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }
    }

    eprintln!("Entity '{key}' not found");
    Ok(())
}

fn export_namespace(store: &SqliteStore, namespace: Option<&str>) -> Result {
    let info = store.db_info()?;
    let mut entries = Vec::new();

    for ns in &info.namespaces {
        if let Some(filter) = namespace {
            if ns.name != filter {
                continue;
            }
        }

        let keys = store.list_keys(&ns.name)?;
        for key in &keys {
            if let Some(data) = store.get(&ns.name, key)? {
                let entry = if VersionedEnvelope::is_versioned(&data) {
                    let env = VersionedEnvelope::from_bytes(&data)
                        .map_err(|e| format!("envelope error: {e}"))?;
                    json!({
                        "namespace": ns.name,
                        "key": key,
                        "version": env.version,
                        "crdt_type": format!("{:?}", env.crdt_type),
                        "payload_size": env.payload.len(),
                    })
                } else {
                    json!({
                        "namespace": ns.name,
                        "key": key,
                        "raw_size": data.len(),
                    })
                };
                entries.push(entry);
            }
        }
    }

    let output = json!({
        "database": {
            "total_entities": info.total_entities,
            "total_events": info.total_events,
            "total_snapshots": info.total_snapshots,
        },
        "entries": entries,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

/// `crdt generate` — Generate Rust code from a schema definition.
pub fn generate(schema_path: &str, output_override: Option<&str>, dry_run: bool) -> Result {
    let path = std::path::Path::new(schema_path);

    if !path.exists() {
        return Err(format!("Schema file not found: {schema_path}").into());
    }

    let result =
        crdt_codegen::generate(path).map_err(|e| format!("Code generation failed: {e}"))?;

    let output_dir = output_override.unwrap_or(&result.output_dir);

    if dry_run {
        println!(
            "Dry run — would generate {} files in {output_dir}/:\n",
            result.files.len()
        );
        for file in &result.files {
            println!("--- {} ---", file.relative_path);
            println!("{}", file.content);
        }
        return Ok(());
    }

    std::fs::create_dir_all(output_dir)?;

    for file in &result.files {
        let full_path = std::path::Path::new(output_dir).join(&file.relative_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, &file.content)?;
        println!("  Generated: {}", full_path.display());
    }

    println!("\nGenerated {} files in {output_dir}/", result.files.len());
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn format_num(n: u64) -> String {
    if n < 1000 {
        return n.to_string();
    }
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

fn base64_encode(data: &[u8]) -> String {
    // Simple base64 without pulling in a dependency
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
