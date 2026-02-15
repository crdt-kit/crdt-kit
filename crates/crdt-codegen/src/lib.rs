//! # crdt-codegen
//!
//! Code generation from TOML schema definitions for crdt-kit.
//!
//! Reads a `crdt-schema.toml` file and generates a complete **persistence layer**
//! organized for Clean Architecture / Hexagonal Architecture:
//!
//! - `models/` — Versioned Rust structs with `#[crdt_schema]` annotations
//! - `migrations/` — Migration functions with `#[migration]` annotations + helpers
//! - `repositories/` — Repository traits (ports) and `CrdtDb`-backed implementations (adapters)
//! - `store.rs` — Unified `Persistence<S>` entry point with scoped repository access
//! - `events/` — Event sourcing types, snapshots, and policies (optional)
//! - `sync/` — Delta sync and state-based merge helpers for CRDT entities (optional)
//!
//! All generated files contain a header marking them as auto-generated.
//!
//! # Example
//!
//! ```rust
//! use crdt_codegen::generate_from_str;
//!
//! let toml = r#"
//! [config]
//! output = "src/persistence"
//!
//! [[entity]]
//! name = "Task"
//! table = "tasks"
//!
//! [[entity.versions]]
//! version = 1
//! fields = [
//!     { name = "title", type = "String" },
//!     { name = "done", type = "bool" },
//! ]
//! "#;
//!
//! let output = generate_from_str(toml).unwrap();
//! assert_eq!(output.output_dir, "src/persistence");
//! assert!(!output.files.is_empty());
//! ```

mod schema;
pub mod templates;
mod validator;

pub use schema::{Entity, EntityVersion, Field, SchemaConfig, SchemaFile};
pub use validator::{validate_schema, ValidationError};

use std::fmt;
use std::path::Path;

/// Error type for the code-generation process.
#[derive(Debug)]
pub enum CodegenError {
    /// Failed to read the schema file.
    Io(std::io::Error),
    /// Failed to parse the TOML schema.
    Parse(String),
    /// Schema validation failed.
    Validation(Vec<ValidationError>),
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Parse(msg) => write!(f, "parse error: {msg}"),
            Self::Validation(errs) => {
                writeln!(f, "schema validation failed:")?;
                for e in errs {
                    writeln!(f, "  - {e}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for CodegenError {}

/// A single generated file ready to be written to disk.
#[derive(Debug, Clone)]
pub struct GeneratedFile {
    /// Path relative to the output directory (e.g., `"models/task.rs"`).
    pub relative_path: String,
    /// Full file content including the auto-generated header.
    pub content: String,
}

/// The complete output of the code-generation process.
#[derive(Debug, Clone)]
pub struct GeneratedOutput {
    /// Output directory from the schema config.
    pub output_dir: String,
    /// All generated files.
    pub files: Vec<GeneratedFile>,
}

/// Parse a TOML schema file from disk and generate all code.
pub fn generate(schema_path: &Path) -> Result<GeneratedOutput, CodegenError> {
    let toml_content = std::fs::read_to_string(schema_path).map_err(CodegenError::Io)?;
    generate_from_str(&toml_content)
}

/// Parse a TOML string and generate all code.
pub fn generate_from_str(toml_content: &str) -> Result<GeneratedOutput, CodegenError> {
    let schema: SchemaFile =
        toml::from_str(toml_content).map_err(|e| CodegenError::Parse(e.to_string()))?;
    generate_from_schema(&schema)
}

/// Generate code from an already-parsed schema.
///
/// Produces a nested directory structure:
///
/// ```text
/// {output}/
///   mod.rs
///   store.rs
///   models/
///     mod.rs
///     {entity}.rs ...
///   migrations/
///     mod.rs
///     helpers.rs
///     {entity}_migrations.rs ...
///   repositories/
///     mod.rs
///     traits.rs
///     {entity}_repo.rs ...
///   events/         (if config.events.enabled)
///     mod.rs
///     policies.rs
///     {entity}_events.rs ...
///   sync/           (if config.sync.enabled)
///     mod.rs
///     {entity}_sync.rs ...
/// ```
pub fn generate_from_schema(schema: &SchemaFile) -> Result<GeneratedOutput, CodegenError> {
    validate_schema(schema).map_err(CodegenError::Validation)?;

    let mut files = Vec::new();

    // ── Models ────────────────────────────────────────────────────
    for entity in &schema.entities {
        let (filename, content) = templates::generate_entity_file(entity);
        files.push(GeneratedFile {
            relative_path: format!("models/{filename}"),
            content,
        });
    }
    files.push(GeneratedFile {
        relative_path: "models/mod.rs".into(),
        content: templates::generate_models_mod_file(&schema.entities),
    });

    // ── Migrations ────────────────────────────────────────────────
    for entity in &schema.entities {
        if entity.versions.len() > 1 {
            let (filename, content) = templates::generate_migration_file(entity);
            files.push(GeneratedFile {
                relative_path: format!("migrations/{filename}"),
                content,
            });
        }
    }
    files.push(GeneratedFile {
        relative_path: "migrations/helpers.rs".into(),
        content: templates::generate_helpers_file(&schema.entities),
    });
    files.push(GeneratedFile {
        relative_path: "migrations/mod.rs".into(),
        content: templates::generate_migrations_mod_file(&schema.entities),
    });

    // ── Repositories ──────────────────────────────────────────────
    files.push(GeneratedFile {
        relative_path: "repositories/traits.rs".into(),
        content: templates::generate_repository_traits_file(&schema.entities),
    });
    for entity in &schema.entities {
        let (filename, content) = templates::generate_repository_impl_file(entity);
        files.push(GeneratedFile {
            relative_path: format!("repositories/{filename}"),
            content,
        });
    }
    files.push(GeneratedFile {
        relative_path: "repositories/mod.rs".into(),
        content: templates::generate_repositories_mod_file(&schema.entities),
    });

    // ── Store ─────────────────────────────────────────────────────
    files.push(GeneratedFile {
        relative_path: "store.rs".into(),
        content: templates::generate_store_file(&schema.entities),
    });

    // ── Events (conditional) ──────────────────────────────────────
    let events_enabled = schema
        .config
        .events
        .as_ref()
        .map(|e| e.enabled)
        .unwrap_or(false);

    if events_enabled {
        let threshold = schema
            .config
            .events
            .as_ref()
            .map(|e| e.snapshot_threshold)
            .unwrap_or(100);

        for entity in &schema.entities {
            let (filename, content) = templates::generate_event_types_file(entity);
            files.push(GeneratedFile {
                relative_path: format!("events/{filename}"),
                content,
            });
        }
        files.push(GeneratedFile {
            relative_path: "events/policies.rs".into(),
            content: templates::generate_snapshot_policy_file(threshold),
        });
        files.push(GeneratedFile {
            relative_path: "events/mod.rs".into(),
            content: templates::generate_events_mod_file(&schema.entities),
        });
    }

    // ── Sync (conditional) ────────────────────────────────────────
    let sync_enabled = schema
        .config
        .sync
        .as_ref()
        .map(|s| s.enabled)
        .unwrap_or(false);

    if sync_enabled {
        // Only generate sync files for entities with CRDT fields.
        let crdt_entities: Vec<&schema::Entity> = schema
            .entities
            .iter()
            .filter(|e| {
                e.versions
                    .iter()
                    .any(|v| v.fields.iter().any(|f| f.crdt.is_some()))
            })
            .collect();

        for entity in &crdt_entities {
            let (filename, content) = templates::generate_sync_file(entity);
            files.push(GeneratedFile {
                relative_path: format!("sync/{filename}"),
                content,
            });
        }
        if !crdt_entities.is_empty() {
            files.push(GeneratedFile {
                relative_path: "sync/mod.rs".into(),
                content: templates::generate_sync_mod_file(&schema.entities),
            });
        }
    }

    // ── Top-level mod.rs ──────────────────────────────────────────
    files.push(GeneratedFile {
        relative_path: "mod.rs".into(),
        content: templates::generate_persistence_mod_file(&schema.entities, &schema.config),
    });

    Ok(GeneratedOutput {
        output_dir: schema.config.output.clone(),
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_TOML: &str = r#"
[config]
output = "src/persistence"

[[entity]]
name = "Task"
table = "tasks"

[[entity.versions]]
version = 1
fields = [
    { name = "title", type = "String" },
    { name = "done", type = "bool" },
]

[[entity.versions]]
version = 2
fields = [
    { name = "title", type = "String" },
    { name = "done", type = "bool" },
    { name = "priority", type = "Option<u8>", default = "None" },
    { name = "tags", type = "Vec<String>", default = "Vec::new()" },
]
"#;

    #[test]
    fn end_to_end_generation() {
        let output = generate_from_str(SAMPLE_TOML).unwrap();
        assert_eq!(output.output_dir, "src/persistence");

        let filenames: Vec<&str> = output
            .files
            .iter()
            .map(|f| f.relative_path.as_str())
            .collect();

        // Models.
        assert!(filenames.contains(&"models/task.rs"));
        assert!(filenames.contains(&"models/mod.rs"));
        // Migrations.
        assert!(filenames.contains(&"migrations/task_migrations.rs"));
        assert!(filenames.contains(&"migrations/helpers.rs"));
        assert!(filenames.contains(&"migrations/mod.rs"));
        // Repositories.
        assert!(filenames.contains(&"repositories/traits.rs"));
        assert!(filenames.contains(&"repositories/task_repo.rs"));
        assert!(filenames.contains(&"repositories/mod.rs"));
        // Store.
        assert!(filenames.contains(&"store.rs"));
        // Top-level.
        assert!(filenames.contains(&"mod.rs"));
        // No events or sync by default.
        assert!(!filenames.iter().any(|f| f.starts_with("events/")));
        assert!(!filenames.iter().any(|f| f.starts_with("sync/")));
    }

    #[test]
    fn generated_structs_compile_ready() {
        let output = generate_from_str(SAMPLE_TOML).unwrap();
        let task_file = output
            .files
            .iter()
            .find(|f| f.relative_path == "models/task.rs")
            .unwrap();

        assert!(task_file.content.contains("pub struct TaskV1"));
        assert!(task_file.content.contains("pub struct TaskV2"));
        assert!(task_file.content.contains("pub type Task = TaskV2;"));
        assert!(task_file
            .content
            .contains("#[crdt_schema(version = 1, table = \"tasks\")]"));
        assert!(task_file
            .content
            .contains("#[crdt_schema(version = 2, table = \"tasks\")]"));
    }

    #[test]
    fn generated_migrations_compile_ready() {
        let output = generate_from_str(SAMPLE_TOML).unwrap();
        let mig_file = output
            .files
            .iter()
            .find(|f| f.relative_path == "migrations/task_migrations.rs")
            .unwrap();

        assert!(mig_file.content.contains("#[migration(from = 1, to = 2)]"));
        assert!(mig_file.content.contains("pub fn migrate_task_v1_to_v2"));
        assert!(mig_file.content.contains("priority: None,"));
        assert!(mig_file.content.contains("tags: Vec::new(),"));
    }

    #[test]
    fn invalid_schema_returns_error() {
        let bad_toml = r#"
[config]
output = ""

[[entity]]
name = "task"
table = ""

[[entity.versions]]
version = 1
fields = []
"#;
        let result = generate_from_str(bad_toml);
        assert!(result.is_err());
        if let Err(CodegenError::Validation(errs)) = result {
            assert!(!errs.is_empty());
        } else {
            panic!("expected validation error");
        }
    }

    #[test]
    fn single_version_no_migrations() {
        let toml = r#"
[config]
output = "out"

[[entity]]
name = "Note"
table = "notes"

[[entity.versions]]
version = 1
fields = [
    { name = "text", type = "String" },
]
"#;
        let output = generate_from_str(toml).unwrap();
        let filenames: Vec<&str> = output
            .files
            .iter()
            .map(|f| f.relative_path.as_str())
            .collect();
        assert!(filenames.contains(&"models/note.rs"));
        assert!(!filenames.iter().any(|f| f.contains("note_migrations")));
        assert!(filenames.contains(&"migrations/helpers.rs"));
        assert!(filenames.contains(&"repositories/note_repo.rs"));
    }

    #[test]
    fn multiple_entities() {
        let toml = r#"
[config]
output = "out"

[[entity]]
name = "Task"
table = "tasks"

[[entity.versions]]
version = 1
fields = [
    { name = "title", type = "String" },
]

[[entity]]
name = "User"
table = "users"

[[entity.versions]]
version = 1
fields = [
    { name = "name", type = "String" },
]
"#;
        let output = generate_from_str(toml).unwrap();
        let filenames: Vec<&str> = output
            .files
            .iter()
            .map(|f| f.relative_path.as_str())
            .collect();
        assert!(filenames.contains(&"models/task.rs"));
        assert!(filenames.contains(&"models/user.rs"));
        assert!(filenames.contains(&"repositories/task_repo.rs"));
        assert!(filenames.contains(&"repositories/user_repo.rs"));
    }

    #[test]
    fn events_and_sync_conditional() {
        let toml = r#"
[config]
output = "out"

[config.events]
enabled = true
snapshot_threshold = 50

[config.sync]
enabled = true

[[entity]]
name = "Project"
table = "projects"

[[entity.versions]]
version = 1
fields = [
    { name = "name", type = "String", crdt = "LWWRegister" },
    { name = "members", type = "String", crdt = "ORSet" },
]
"#;
        let output = generate_from_str(toml).unwrap();
        let filenames: Vec<&str> = output
            .files
            .iter()
            .map(|f| f.relative_path.as_str())
            .collect();

        // Events should be generated.
        assert!(filenames.contains(&"events/mod.rs"));
        assert!(filenames.contains(&"events/policies.rs"));
        assert!(filenames.contains(&"events/project_events.rs"));

        // Sync should be generated for CRDT entity.
        assert!(filenames.contains(&"sync/mod.rs"));
        assert!(filenames.contains(&"sync/project_sync.rs"));

        // Check snapshot threshold.
        let policies = output
            .files
            .iter()
            .find(|f| f.relative_path == "events/policies.rs")
            .unwrap();
        assert!(policies.content.contains("event_threshold: 50,"));

        // Check top-level mod includes events/sync.
        let mod_file = output
            .files
            .iter()
            .find(|f| f.relative_path == "mod.rs")
            .unwrap();
        assert!(mod_file.content.contains("pub mod events;"));
        assert!(mod_file.content.contains("pub mod sync;"));
    }
}
