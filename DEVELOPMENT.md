# Development Flow

This document explains how to develop with crdt-kit's schema-driven code generation, from defining entities to using the generated persistence layer.

## Overview

```
crdt-schema.toml  -->  crdt generate  -->  src/persistence/  -->  your app
   (define)              (generate)          (auto-generated)      (use)
```

The workflow has three phases:

1. **Define** entities in a TOML schema file
2. **Generate** the persistence layer with `crdt generate`
3. **Use** the generated repositories, events, and sync helpers in your application

---

## 1. Define the Schema

Create a `crdt-schema.toml` file in your crate root:

```toml
[config]
output = "src/persistence"

# Optional: enable event sourcing
[config.events]
enabled = true
snapshot_threshold = 100

# Optional: enable delta sync for CRDT entities
[config.sync]
enabled = true

# Define entities with versioned schemas
[[entity]]
name = "Project"
table = "projects"

[[entity.versions]]
version = 1
fields = [
    { name = "name", type = "String", crdt = "LWWRegister" },
    { name = "members", type = "String", crdt = "ORSet" },
]

[[entity]]
name = "Task"
table = "tasks"

[[entity.versions]]
version = 1
fields = [
    { name = "title", type = "String" },
    { name = "done", type = "bool" },
]

# Add a new version when the schema evolves
[[entity.versions]]
version = 2
fields = [
    { name = "title", type = "String" },
    { name = "done", type = "bool" },
    { name = "priority", type = "Option<u8>", default = "None" },
    { name = "tags", type = "Vec<String>", default = "Vec::new()" },
    { name = "project_id", type = "String", default = "String::new()", relation = "Project" },
]
```

### Schema Rules

- **Entity names**: PascalCase (e.g., `Task`, `Project`)
- **Field names**: snake_case (e.g., `project_id`, `created_at`)
- **Supported types**: `String`, `bool`, `u8`-`u64`, `i8`-`i64`, `f32`, `f64`, `Option<T>`, `Vec<T>`
- **Versions**: Must start at 1 and be contiguous (1, 2, 3, ...)
- **New fields in later versions**: Must have a `default` value (or a `crdt` type, which provides auto-defaults)
- **CRDT types**: `GCounter`, `PNCounter`, `LWWRegister`, `MVRegister`, `GSet`, `TwoPSet`, `ORSet`
- **Relations**: Reference another entity name to generate `find_by_*` methods

---

## 2. Generate Code

```bash
# Generate the persistence layer
crdt generate --schema crdt-schema.toml

# Preview without writing files
crdt generate --schema crdt-schema.toml --dry-run
```

### Generated Directory Structure

```
src/persistence/
  mod.rs                          # Top-level re-exports
  store.rs                        # Persistence<S> struct + factory
  models/
    mod.rs
    task.rs                       # TaskV1, TaskV2, type Task = TaskV2
    project.rs                    # ProjectV1, type Project = ProjectV1
  migrations/
    mod.rs
    helpers.rs                    # create_db(), create_memory_db()
    task_migrations.rs            # migrate_task_v1_to_v2()
  repositories/
    mod.rs
    traits.rs                    # TaskRepository trait, ProjectRepository trait
    task_repo.rs                 # TaskRepositoryAccess<S> (adapter)
    project_repo.rs              # ProjectRepositoryAccess<S> (adapter)
  events/                        # Only if [config.events] enabled = true
    mod.rs
    policies.rs                  # SnapshotPolicy, DEFAULT_POLICY
    task_events.rs               # TaskEvent, TaskSnapshot, TaskFieldUpdate
    project_events.rs            # ProjectEvent, ProjectSnapshot, ProjectFieldUpdate
  sync/                          # Only if [config.sync] enabled = true
    mod.rs
    project_sync.rs              # ProjectDelta, compute/apply/merge functions
```

---

## 3. Use the Generated Code

### Basic Setup

Add the persistence module to your crate:

```rust
mod persistence;

use persistence::*;
```

### Repository Pattern

```rust
// Create the persistence layer (in-memory for testing)
let mut persistence = create_memory_persistence();

// Use typed repository access
let mut tasks = persistence.tasks();
tasks.save("task-1", &task)?;
let loaded = tasks.get("task-1")?;
let all = tasks.list()?;
let exists = tasks.exists("task-1")?;
tasks.delete("task-1")?;

// Relation queries
let project_tasks = tasks.find_by_project_id("proj-1")?;
```

### CRDT Fields and Merge

```rust
// Create entities with CRDT fields
let mut project = Project {
    name: LWWRegister::new("node-a", "My Project".to_string()),
    members: ORSet::new("node-a"),
};
project.members.insert("Alice".to_string());

// Merge concurrent edits
merge_project(&mut local, &remote);
```

### Delta Sync

```rust
// Compute delta (what remote has that local doesn't)
let delta = compute_project_delta(&local, &remote);

// Apply delta to local replica
apply_project_delta(&mut local, &delta);
```

### Event Sourcing

```rust
// Create typed events
let event = TaskEvent::Created(TaskSnapshot {
    title: "Deploy v2".into(),
    done: false,
    priority: Some(1),
    tags: vec!["devops".into()],
    project_id: String::new(),
});

// Check snapshot policy
let should_snap = DEFAULT_POLICY.should_snapshot(event_count);

// Use the CrdtDb event API
let db = persistence.db_mut();
db.append_event("tasks", "task-1", &data, timestamp, "node-a")?;
db.compact("tasks", "task-1", &snapshot_bytes, since_seq)?;
```

### Automatic Migration

```rust
// Old v1 data is transparently migrated to v2 on read
let mut persistence = create_memory_persistence();
let task: Task = persistence.tasks().get("old-key")?.unwrap();
// task.priority == None  (auto-filled by migration)
// task.tags == []         (auto-filled by migration)
```

---

## 4. Schema Evolution

When your schema needs to change:

1. Add a new `[[entity.versions]]` block with the next version number
2. Include all fields (both old and new)
3. New fields **must** have a `default` value
4. Re-run `crdt generate --schema crdt-schema.toml`
5. The generated migration function handles data upgrade automatically

If a migration is too complex for auto-generation (e.g., field removal, type change), the generator produces a `todo!()` skeleton that you must implement manually.

---

## Architecture

The generated persistence layer follows **hexagonal architecture** (ports and adapters):

```
+-------------------+
|   Domain Layer    |  <-- Your business logic
+-------------------+
         |
         | depends on traits (ports)
         v
+-------------------+
| Repository Traits |  <-- TaskRepository, ProjectRepository
+-------------------+
         ^
         | implemented by adapters
         |
+-------------------+
| RepositoryAccess  |  <-- TaskRepositoryAccess<S>, backed by CrdtDb<S>
+-------------------+
         |
         v
+-------------------+
|     CrdtDb<S>     |  <-- Versioned, migration-aware store
+-------------------+
         |
         v
+-------------------+
|   StateStore S    |  <-- MemoryStore, SqliteStore, RedbStore
+-------------------+
```

- **Ports** (`repositories/traits.rs`): Define the interface your domain depends on
- **Adapters** (`repositories/*_repo.rs`): Implement traits using `CrdtDb<S>`
- **Persistence** (`store.rs`): Single entry point owning one `CrdtDb<S>`

---

## Workspace Crates

| Crate | Version | Description |
|-------|---------|-------------|
| `crdt-kit` | 0.3.0 | Core CRDT types (GCounter, PNCounter, LWWRegister, MVRegister, GSet, TwoPSet, ORSet, Rga, TextCrdt) |
| `crdt-store` | 0.2.0 | Persistence backends (MemoryStore, SqliteStore, RedbStore) |
| `crdt-migrate` | 0.2.0 | Versioned serialization and automatic schema migrations |
| `crdt-migrate-macros` | 0.2.0 | Proc macros: `#[crdt_schema]` and `#[migration]` |
| `crdt-codegen` | 0.2.0 | Code generation from TOML schemas |
| `crdt-cli` | 0.3.0 | CLI tool: `crdt generate`, `crdt dev-ui`, `crdt inspect` |
| `crdt-dev-ui` | 0.2.0 | Embedded web panel for database inspection |

---

## Running Tests

```bash
# Full workspace tests
cargo test --workspace --all-features

# Specific crate tests
cargo test -p crdt-codegen
cargo test -p crdt-kit

# Lint checks
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Format check
cargo fmt --all -- --check

# Run the example
cargo run -p crdt-example-tasks
```
