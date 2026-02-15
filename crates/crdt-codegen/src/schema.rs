use serde::Deserialize;

/// Top-level schema file structure parsed from `crdt-schema.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct SchemaFile {
    /// Global configuration.
    pub config: SchemaConfig,
    /// Entity definitions.
    #[serde(rename = "entity")]
    pub entities: Vec<Entity>,
}

/// Global code-generation configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SchemaConfig {
    /// Output directory relative to the project root.
    pub output: String,
    /// Event sourcing configuration (optional).
    pub events: Option<EventsConfig>,
    /// Sync/delta configuration (optional).
    pub sync: Option<SyncConfig>,
}

/// Configuration for event sourcing code generation.
#[derive(Debug, Clone, Deserialize)]
pub struct EventsConfig {
    /// Whether to generate event types and event sourcing helpers.
    #[serde(default)]
    pub enabled: bool,
    /// Number of events before a snapshot is recommended.
    #[serde(default = "default_snapshot_threshold")]
    pub snapshot_threshold: u64,
}

fn default_snapshot_threshold() -> u64 {
    100
}

/// Configuration for delta sync code generation.
#[derive(Debug, Clone, Deserialize)]
pub struct SyncConfig {
    /// Whether to generate delta sync helpers for entities with CRDT fields.
    #[serde(default)]
    pub enabled: bool,
}

/// A single entity definition with one or more versioned schemas.
#[derive(Debug, Clone, Deserialize)]
pub struct Entity {
    /// Entity name in PascalCase (e.g., `"Task"`).
    pub name: String,
    /// Storage namespace / table name (e.g., `"tasks"`).
    pub table: String,
    /// Ordered list of schema versions. Must start at 1 and be contiguous.
    pub versions: Vec<EntityVersion>,
}

/// A specific version of an entity's schema.
#[derive(Debug, Clone, Deserialize)]
pub struct EntityVersion {
    /// Version number (1, 2, 3, ...).
    pub version: u32,
    /// Fields in this version.
    pub fields: Vec<Field>,
}

/// A single field within an entity version.
#[derive(Debug, Clone, Deserialize)]
pub struct Field {
    /// Field name in snake_case.
    pub name: String,
    /// Rust type as a string (e.g., `"String"`, `"Option<u8>"`, `"Vec<String>"`).
    #[serde(rename = "type")]
    pub field_type: String,
    /// Default value expression (Rust literal). Required for fields added in
    /// later versions so that automatic migration can fill them in.
    pub default: Option<String>,
    /// CRDT type wrapping this field (e.g., `"LWWRegister"`, `"GCounter"`, `"ORSet"`).
    ///
    /// When set, the generated Rust type becomes `CrdtType<field_type>` (or just
    /// `CrdtType` for counter types). Migration defaults are auto-generated.
    pub crdt: Option<String>,
    /// Relation to another entity (e.g., `"Project"` means this field is a key
    /// referencing a Project entity). Generates typed lookup helpers.
    pub relation: Option<String>,
}

/// Supported CRDT type names and their properties.
pub const SUPPORTED_CRDTS: &[CrdtInfo] = &[
    CrdtInfo {
        name: "GCounter",
        is_generic: false,
        default_expr: "GCounter::new(\"_migrated\")",
    },
    CrdtInfo {
        name: "PNCounter",
        is_generic: false,
        default_expr: "PNCounter::new(\"_migrated\")",
    },
    CrdtInfo {
        name: "LWWRegister",
        is_generic: true,
        default_expr: "LWWRegister::with_timestamp(\"_migrated\", Default::default(), 0)",
    },
    CrdtInfo {
        name: "MVRegister",
        is_generic: true,
        default_expr: "MVRegister::new(\"_migrated\")",
    },
    CrdtInfo {
        name: "GSet",
        is_generic: true,
        default_expr: "GSet::new()",
    },
    CrdtInfo {
        name: "TwoPSet",
        is_generic: true,
        default_expr: "TwoPSet::new()",
    },
    CrdtInfo {
        name: "ORSet",
        is_generic: true,
        default_expr: "ORSet::new(\"_migrated\")",
    },
];

/// Metadata about a supported CRDT type.
pub struct CrdtInfo {
    /// CRDT type name (e.g., `"GCounter"`).
    pub name: &'static str,
    /// Whether the CRDT is generic over a type parameter (e.g., `LWWRegister<T>`).
    pub is_generic: bool,
    /// Default expression for migration (creates an empty/default instance).
    pub default_expr: &'static str,
}

/// Look up a CRDT info by name.
pub fn lookup_crdt(name: &str) -> Option<&'static CrdtInfo> {
    SUPPORTED_CRDTS.iter().find(|c| c.name == name)
}

/// CRDTs that implement `DeltaCrdt` and their delta type names.
pub const DELTA_CRDTS: &[(&str, &str)] = &[
    ("GCounter", "GCounterDelta"),
    ("PNCounter", "PNCounterDelta"),
    ("ORSet", "ORSetDelta"),
];

/// Look up the delta type name for a CRDT, if it supports `DeltaCrdt`.
pub fn lookup_delta_type(crdt_name: &str) -> Option<&'static str> {
    DELTA_CRDTS
        .iter()
        .find(|(name, _)| *name == crdt_name)
        .map(|(_, delta)| *delta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_schema() {
        let toml = r#"
[config]
output = "src/generated"

[[entity]]
name = "Task"
table = "tasks"

[[entity.versions]]
version = 1
fields = [
    { name = "title", type = "String" },
    { name = "done", type = "bool" },
]
"#;
        let schema: SchemaFile = toml::from_str(toml).unwrap();
        assert_eq!(schema.config.output, "src/generated");
        assert_eq!(schema.entities.len(), 1);
        assert_eq!(schema.entities[0].name, "Task");
        assert_eq!(schema.entities[0].table, "tasks");
        assert_eq!(schema.entities[0].versions[0].fields.len(), 2);
    }

    #[test]
    fn parse_crdt_and_relation_fields() {
        let toml = r#"
[config]
output = "out"

[[entity]]
name = "Task"
table = "tasks"

[[entity.versions]]
version = 1
fields = [
    { name = "title", type = "String", crdt = "LWWRegister" },
    { name = "views", type = "u64", crdt = "GCounter" },
    { name = "project_id", type = "String", relation = "Project" },
]
"#;
        let schema: SchemaFile = toml::from_str(toml).unwrap();
        let fields = &schema.entities[0].versions[0].fields;
        assert_eq!(fields[0].crdt.as_deref(), Some("LWWRegister"));
        assert_eq!(fields[1].crdt.as_deref(), Some("GCounter"));
        assert_eq!(fields[2].relation.as_deref(), Some("Project"));
    }

    #[test]
    fn parse_events_and_sync_config() {
        let toml = r#"
[config]
output = "src/persistence"

[config.events]
enabled = true
snapshot_threshold = 200

[config.sync]
enabled = true

[[entity]]
name = "Task"
table = "tasks"

[[entity.versions]]
version = 1
fields = [
    { name = "title", type = "String" },
]
"#;
        let schema: SchemaFile = toml::from_str(toml).unwrap();
        let events = schema.config.events.unwrap();
        assert!(events.enabled);
        assert_eq!(events.snapshot_threshold, 200);
        let sync = schema.config.sync.unwrap();
        assert!(sync.enabled);
    }

    #[test]
    fn parse_config_without_events_sync() {
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
"#;
        let schema: SchemaFile = toml::from_str(toml).unwrap();
        assert!(schema.config.events.is_none());
        assert!(schema.config.sync.is_none());
    }

    #[test]
    fn lookup_delta_type_works() {
        assert_eq!(lookup_delta_type("GCounter"), Some("GCounterDelta"));
        assert_eq!(lookup_delta_type("PNCounter"), Some("PNCounterDelta"));
        assert_eq!(lookup_delta_type("ORSet"), Some("ORSetDelta"));
        assert_eq!(lookup_delta_type("LWWRegister"), None);
        assert_eq!(lookup_delta_type("GSet"), None);
    }

    #[test]
    fn parse_multi_version_schema() {
        let toml = r#"
[config]
output = "out"

[[entity]]
name = "Sensor"
table = "sensors"

[[entity.versions]]
version = 1
fields = [
    { name = "device_id", type = "String" },
    { name = "temperature", type = "f32" },
]

[[entity.versions]]
version = 2
fields = [
    { name = "device_id", type = "String" },
    { name = "temperature", type = "f32" },
    { name = "humidity", type = "Option<f32>", default = "None" },
]
"#;
        let schema: SchemaFile = toml::from_str(toml).unwrap();
        let sensor = &schema.entities[0];
        assert_eq!(sensor.versions.len(), 2);
        assert_eq!(sensor.versions[1].fields[2].name, "humidity");
        assert_eq!(
            sensor.versions[1].fields[2].default.as_deref(),
            Some("None")
        );
    }
}
