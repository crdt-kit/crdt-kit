//! # Task Management Example
//!
//! Demonstrates the full crdt-kit stack with a code-generated persistence layer:
//!
//! 1. **Repository pattern** — typed CRUD via `Persistence<S>` and repository traits
//! 2. **Automatic migration** — v1 data transparently upgraded to v2 on read
//! 3. **CRDT fields** — Project entity with conflict-free replicated fields
//! 4. **Relations** — find tasks by project using `find_by_project_id`
//! 5. **Delta sync** — incremental sync for CRDT entities
//! 6. **Event sourcing** — typed events, snapshots, and snapshot policies
//!
//! The entire persistence layer (`src/persistence/`) is auto-generated from
//! `crdt-schema.toml`. The domain layer only depends on repository traits (ports).
//!
//! Run: `cargo run -p crdt-example-tasks`

mod persistence;

use crdt_kit::prelude::*;
use crdt_migrate::VersionedEnvelope;
use crdt_store::{CrdtDb, MemoryStore, StateStore};
use persistence::*;

fn main() {
    println!("=== Task Management Example (Persistence Layer) ===\n");

    demo_repository_pattern();
    demo_migration();
    demo_crdt_fields();
    demo_relations();
    demo_delta_sync();
    demo_event_sourcing();

    println!("\n=== Done! ===");
}

// ── Section 1: Repository Pattern ────────────────────────────────

fn demo_repository_pattern() {
    println!("1. Repository pattern with generated persistence layer...\n");

    // Create persistence layer (owns CrdtDb, provides scoped repository access).
    let mut persistence = create_memory_persistence();

    // Save a task using the repository.
    let task = Task {
        title: "Write documentation".into(),
        done: false,
        priority: Some(1),
        tags: vec!["docs".into(), "important".into()],
        project_id: String::new(),
    };

    {
        let mut tasks = persistence.tasks();
        tasks.save("task-1", &task).unwrap();

        let loaded = tasks.get("task-1").unwrap().unwrap();
        println!("   Saved and loaded: {:?}", loaded);
        assert_eq!(loaded, task);

        let exists = tasks.exists("task-1").unwrap();
        println!("   Exists: {exists}");
        assert!(exists);

        let all = tasks.list().unwrap();
        println!("   Total tasks: {}", all.len());
        assert_eq!(all.len(), 1);
    }

    // Repository access is scoped — we can switch between repositories.
    {
        let mut projects = persistence.projects();
        let project = Project {
            name: LWWRegister::new("node-a", "My Project".to_string()),
            members: ORSet::new("node-a"),
        };
        projects.save("proj-1", &project).unwrap();
        println!("   Project saved via repository");
    }

    println!();
}

// ── Section 2: Migration v1 → v2 ──────────────────────────────────

fn demo_migration() {
    println!("2. Automatic migration v1 -> v2...\n");

    // Simulate: v1 firmware wrote this data.
    let mut db_v1 = CrdtDb::with_store(MemoryStore::new());
    let old_task = TaskV1 {
        title: "Buy groceries".into(),
        done: false,
    };
    db_v1.save_ns("tasks", "task-old", &old_task).unwrap();

    // Extract raw bytes (simulating persistent storage).
    let raw = db_v1.store().get("tasks", "task-old").unwrap().unwrap();
    let env = VersionedEnvelope::from_bytes(&raw).unwrap();
    println!("   V1 data: version={}, {} bytes", env.version, raw.len());

    // Load into v2 database with migration registered.
    let mut store_v2 = MemoryStore::new();
    store_v2.put("tasks", "task-old", &raw).unwrap();
    let mut db_v2 = create_db(store_v2);

    let migrated: TaskV2 = db_v2.load_ns("tasks", "task-old").unwrap().unwrap();
    println!("   Migrated to v2: {:?}", migrated);

    assert_eq!(migrated.title, "Buy groceries");
    assert!(!migrated.done);
    assert_eq!(migrated.priority, None);
    assert!(migrated.tags.is_empty());
    assert!(migrated.project_id.is_empty());

    println!("   Migration successful!\n");
}

// ── Section 3: CRDT fields ────────────────────────────────────────

fn demo_crdt_fields() {
    println!("3. CRDT fields with conflict-free merge...\n");

    let mut persistence = create_memory_persistence();

    // Create a project with CRDT fields (LWWRegister + ORSet).
    let mut project_a = Project {
        name: LWWRegister::new("node-a", "Alpha Project".to_string()),
        members: ORSet::new("node-a"),
    };
    project_a.members.insert("Alice".to_string());
    project_a.members.insert("Bob".to_string());

    println!("   Node A project: name={:?}", project_a.name.value());
    println!(
        "   Node A members: {:?}",
        project_a.members.iter().collect::<Vec<_>>()
    );

    // Simulate a concurrent edit on node B.
    let mut project_b = project_a.clone();
    project_b.name = LWWRegister::new("node-b", "Beta Project".to_string());
    project_b.members.insert("Charlie".to_string());

    println!("   Node B renamed to: {:?}", project_b.name.value());

    // Merge using the generated merge function.
    merge_project(&mut project_a, &project_b);

    println!("   After merge: name={:?}", project_a.name.value());
    println!(
        "   After merge: members={:?}",
        project_a.members.iter().collect::<Vec<_>>()
    );

    // ORSet should contain all three members.
    assert_eq!(project_a.members.iter().count(), 3);

    // Save and load via repository.
    {
        let mut projects = persistence.projects();
        projects.save("proj-1", &project_a).unwrap();
        let loaded = projects.get("proj-1").unwrap().unwrap();
        assert_eq!(loaded.members.iter().count(), 3);
    }
    println!("   Stored and loaded project via repository!\n");
}

// ── Section 4: Relations ──────────────────────────────────────────

fn demo_relations() {
    println!("4. Entity relations (Task -> Project) with find_by...\n");

    let mut persistence = create_memory_persistence();

    // Save a project.
    {
        let mut projects = persistence.projects();
        let project = Project {
            name: LWWRegister::new("node-a", "Backend Rewrite".to_string()),
            members: ORSet::new("node-a"),
        };
        projects.save("proj-backend", &project).unwrap();
    }

    // Save tasks linked to the project.
    {
        let mut tasks = persistence.tasks();

        let task_data = vec![
            (
                "task-1",
                Task {
                    title: "Design API schema".into(),
                    done: true,
                    priority: Some(1),
                    tags: vec!["api".into()],
                    project_id: "proj-backend".into(),
                },
            ),
            (
                "task-2",
                Task {
                    title: "Implement auth middleware".into(),
                    done: false,
                    priority: Some(2),
                    tags: vec!["auth".into(), "middleware".into()],
                    project_id: "proj-backend".into(),
                },
            ),
            (
                "task-3",
                Task {
                    title: "Write tests".into(),
                    done: false,
                    priority: Some(3),
                    tags: vec!["testing".into()],
                    project_id: "proj-backend".into(),
                },
            ),
            (
                "task-unlinked",
                Task {
                    title: "Unrelated task".into(),
                    done: false,
                    priority: None,
                    tags: vec![],
                    project_id: String::new(),
                },
            ),
        ];

        for (key, task) in &task_data {
            tasks.save(key, task).unwrap();
        }

        // Use the generated find_by_project_id method.
        let project_tasks = tasks.find_by_project_id("proj-backend").unwrap();

        println!("   Tasks in 'Backend Rewrite' project:");
        for (key, task) in &project_tasks {
            println!(
                "     [{key}] {} (priority: {:?}, done: {})",
                task.title, task.priority, task.done
            );
        }
        assert_eq!(project_tasks.len(), 3);
    }

    println!("   Relation query via find_by_project_id successful!\n");
}

// ── Section 5: Delta Sync ──────────────────────────────────────────

fn demo_delta_sync() {
    println!("5. Delta sync for CRDT entities...\n");

    // Create two replicas of the same project.
    let mut project_local = Project {
        name: LWWRegister::new("node-a", "Shared Project".to_string()),
        members: ORSet::new("node-a"),
    };
    project_local.members.insert("Alice".to_string());
    project_local.members.insert("Bob".to_string());

    let mut project_remote = project_local.clone();
    project_remote.members.insert("Charlie".to_string());

    // Compute delta between local and remote.
    let delta = compute_project_delta(&project_local, &project_remote);
    println!("   Computed delta: {:?}", delta);

    // Apply delta to local replica.
    apply_project_delta(&mut project_local, &delta);
    println!(
        "   After applying delta, local members: {:?}",
        project_local.members.iter().collect::<Vec<_>>()
    );

    // Full-state merge for comparison.
    let mut project_merge_test = Project {
        name: LWWRegister::new("node-a", "Shared Project".to_string()),
        members: ORSet::new("node-a"),
    };
    project_merge_test.members.insert("Alice".to_string());
    project_merge_test.members.insert("Bob".to_string());
    merge_project(&mut project_merge_test, &project_remote);
    println!(
        "   After full merge, members: {:?}",
        project_merge_test.members.iter().collect::<Vec<_>>()
    );

    println!("   Delta sync successful!\n");
}

// ── Section 6: Event Sourcing ──────────────────────────────────────

fn demo_event_sourcing() {
    println!("6. Event sourcing with generated types and snapshot policy...\n");

    let mut persistence = create_memory_persistence();

    // Use the generated event types.
    let events: Vec<TaskEvent> = vec![
        TaskEvent::Created(TaskSnapshot {
            title: "Deploy v2".into(),
            done: false,
            priority: Some(1),
            tags: vec!["devops".into()],
            project_id: String::new(),
        }),
        TaskEvent::FieldUpdated(TaskFieldUpdate::Done(true)),
        TaskEvent::FieldUpdated(TaskFieldUpdate::Tags(vec![
            "devops".into(),
            "release".into(),
        ])),
    ];

    // Append events to the database.
    let db = persistence.db_mut();
    for (i, ev) in events.iter().enumerate() {
        let data = postcard::to_allocvec(ev).unwrap();
        let seq = db
            .append_event("tasks", "task-2", &data, (i as u64 + 1) * 1000, "node-a")
            .unwrap();
        println!("   seq={seq}: {:?}", ev);
    }

    // Read events back.
    let stored = db.events_since("tasks", "task-2", 0).unwrap();
    println!("\n   Total events: {}", stored.len());
    assert_eq!(stored.len(), 3);

    // Check snapshot policy.
    let policy = &DEFAULT_POLICY;
    let should_snap = policy.should_snapshot(stored.len() as u64);
    println!(
        "   Snapshot policy: threshold={}, count={}, should_snapshot={}",
        policy.event_threshold,
        stored.len(),
        should_snap
    );
    assert!(!should_snap); // 3 < 100

    // Compact with snapshot.
    let task_state = Task {
        title: "Deploy v2".into(),
        done: true,
        priority: Some(1),
        tags: vec!["devops".into(), "release".into()],
        project_id: String::new(),
    };
    let state_bytes = postcard::to_allocvec(&task_state).unwrap();

    let removed = db.compact("tasks", "task-2", &state_bytes, 1).unwrap();
    let remaining = db.event_count("tasks", "task-2").unwrap();
    println!("   Compacted: removed {removed} events, {remaining} remaining");

    // Load snapshot.
    let snap = db.load_snapshot("tasks", "task-2").unwrap().unwrap();
    let recovered: Task = postcard::from_bytes(&snap.state).unwrap();
    println!("   Recovered from snapshot: {:?}", recovered);
    assert_eq!(recovered, task_state);
}
