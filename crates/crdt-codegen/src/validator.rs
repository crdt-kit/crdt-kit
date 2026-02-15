use crate::schema::{lookup_crdt, Entity, SchemaFile};
use std::collections::HashSet;
use std::fmt;

/// A single validation error with context about where it occurred.
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub entity: Option<String>,
    pub version: Option<u32>,
    pub field: Option<String>,
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ctx = Vec::new();
        if let Some(e) = &self.entity {
            ctx.push(format!("entity={e}"));
        }
        if let Some(v) = self.version {
            ctx.push(format!("v{v}"));
        }
        if let Some(field) = &self.field {
            ctx.push(format!("field={field}"));
        }
        if ctx.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "[{}] {}", ctx.join(", "), self.message)
        }
    }
}

/// Supported primitive types for fields.
const SUPPORTED_PRIMITIVES: &[&str] = &[
    "String", "bool", "u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "f32", "f64",
];

/// Validate a parsed schema file. Returns `Ok(())` if valid, or a list of errors.
pub fn validate_schema(schema: &SchemaFile) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    if schema.config.output.is_empty() {
        errors.push(ValidationError {
            entity: None,
            version: None,
            field: None,
            message: "config.output must not be empty".into(),
        });
    }

    if schema.entities.is_empty() {
        errors.push(ValidationError {
            entity: None,
            version: None,
            field: None,
            message: "schema must define at least one entity".into(),
        });
    }

    // Validate events config.
    if let Some(events) = &schema.config.events {
        if events.enabled && events.snapshot_threshold == 0 {
            errors.push(ValidationError {
                entity: None,
                version: None,
                field: None,
                message: "config.events.snapshot_threshold must be > 0".into(),
            });
        }
    }

    // Validate sync config: requires at least one entity with CRDT fields.
    if let Some(sync) = &schema.config.sync {
        if sync.enabled {
            let any_crdt = schema.entities.iter().any(|e| {
                e.versions
                    .iter()
                    .any(|v| v.fields.iter().any(|f| f.crdt.is_some()))
            });
            if !any_crdt {
                errors.push(ValidationError {
                    entity: None,
                    version: None,
                    field: None,
                    message: "config.sync.enabled requires at least one entity with CRDT fields"
                        .into(),
                });
            }
        }
    }

    let all_entity_names: HashSet<&str> = schema.entities.iter().map(|e| e.name.as_str()).collect();

    let mut entity_names = HashSet::new();
    for entity in &schema.entities {
        if !entity_names.insert(&entity.name) {
            errors.push(ValidationError {
                entity: Some(entity.name.clone()),
                version: None,
                field: None,
                message: "duplicate entity name".into(),
            });
        }
        validate_entity(entity, &all_entity_names, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_entity(
    entity: &Entity,
    all_entity_names: &HashSet<&str>,
    errors: &mut Vec<ValidationError>,
) {
    // Check name is non-empty and starts with uppercase.
    if entity.name.is_empty()
        || !entity
            .name
            .chars()
            .next()
            .unwrap_or('a')
            .is_ascii_uppercase()
    {
        errors.push(ValidationError {
            entity: Some(entity.name.clone()),
            version: None,
            field: None,
            message: "entity name must be PascalCase (start with uppercase)".into(),
        });
    }

    if entity.table.is_empty() {
        errors.push(ValidationError {
            entity: Some(entity.name.clone()),
            version: None,
            field: None,
            message: "table name must not be empty".into(),
        });
    }

    if entity.versions.is_empty() {
        errors.push(ValidationError {
            entity: Some(entity.name.clone()),
            version: None,
            field: None,
            message: "entity must have at least one version".into(),
        });
        return;
    }

    // Check versions are contiguous starting at 1.
    for (i, ver) in entity.versions.iter().enumerate() {
        let expected = (i as u32) + 1;
        if ver.version != expected {
            errors.push(ValidationError {
                entity: Some(entity.name.clone()),
                version: Some(ver.version),
                field: None,
                message: format!("expected version {expected}, got {}", ver.version),
            });
        }
    }

    // Validate fields in each version.
    let mut prev_fields: Option<HashSet<String>> = None;
    for ver in &entity.versions {
        let mut field_names = HashSet::new();
        for field in &ver.fields {
            if !field_names.insert(field.name.clone()) {
                errors.push(ValidationError {
                    entity: Some(entity.name.clone()),
                    version: Some(ver.version),
                    field: Some(field.name.clone()),
                    message: "duplicate field name".into(),
                });
            }

            // Check field name is non-empty and looks like snake_case.
            if field.name.is_empty()
                || field
                    .name
                    .chars()
                    .next()
                    .unwrap_or('A')
                    .is_ascii_uppercase()
            {
                errors.push(ValidationError {
                    entity: Some(entity.name.clone()),
                    version: Some(ver.version),
                    field: Some(field.name.clone()),
                    message: "field name must be snake_case (start with lowercase)".into(),
                });
            }

            // Check field type is supported.
            if !is_supported_type(&field.field_type) {
                errors.push(ValidationError {
                    entity: Some(entity.name.clone()),
                    version: Some(ver.version),
                    field: Some(field.name.clone()),
                    message: format!("unsupported type `{}`", field.field_type),
                });
            }

            // Check CRDT type is valid.
            if let Some(crdt_name) = &field.crdt {
                if lookup_crdt(crdt_name).is_none() {
                    errors.push(ValidationError {
                        entity: Some(entity.name.clone()),
                        version: Some(ver.version),
                        field: Some(field.name.clone()),
                        message: format!(
                            "unsupported CRDT type `{crdt_name}` (supported: GCounter, PNCounter, LWWRegister, MVRegister, GSet, TwoPSet, ORSet)"
                        ),
                    });
                }
            }

            // Check relation references a known entity.
            if let Some(rel) = &field.relation {
                if !all_entity_names.contains(rel.as_str()) {
                    errors.push(ValidationError {
                        entity: Some(entity.name.clone()),
                        version: Some(ver.version),
                        field: Some(field.name.clone()),
                        message: format!("relation references unknown entity `{rel}`"),
                    });
                }
            }

            // Check that new fields in later versions have defaults.
            // CRDT fields get auto-defaults, so they don't need explicit ones.
            if let Some(prev) = &prev_fields {
                let has_auto_default = field.crdt.is_some();
                if !prev.contains(&field.name) && field.default.is_none() && !has_auto_default {
                    errors.push(ValidationError {
                        entity: Some(entity.name.clone()),
                        version: Some(ver.version),
                        field: Some(field.name.clone()),
                        message: "field added in a later version must have a `default` value"
                            .into(),
                    });
                }
            }
        }
        prev_fields = Some(field_names);
    }
}

fn is_supported_type(ty: &str) -> bool {
    // Direct primitive match.
    if SUPPORTED_PRIMITIVES.contains(&ty) {
        return true;
    }
    // Option<T>
    if let Some(inner) = ty.strip_prefix("Option<").and_then(|s| s.strip_suffix('>')) {
        return is_supported_type(inner.trim());
    }
    // Vec<T>
    if let Some(inner) = ty.strip_prefix("Vec<").and_then(|s| s.strip_suffix('>')) {
        return is_supported_type(inner.trim());
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;

    fn make_schema(entities: Vec<Entity>) -> SchemaFile {
        SchemaFile {
            config: SchemaConfig {
                output: "src/generated".into(),
                events: None,
                sync: None,
            },
            entities,
        }
    }

    fn make_entity(name: &str, table: &str, versions: Vec<EntityVersion>) -> Entity {
        Entity {
            name: name.into(),
            table: table.into(),
            versions,
        }
    }

    fn make_version(version: u32, fields: Vec<Field>) -> EntityVersion {
        EntityVersion { version, fields }
    }

    fn make_field(name: &str, field_type: &str, default: Option<&str>) -> Field {
        Field {
            name: name.into(),
            field_type: field_type.into(),
            default: default.map(|s| s.into()),
            crdt: None,
            relation: None,
        }
    }

    #[test]
    fn valid_minimal_schema() {
        let schema = make_schema(vec![make_entity(
            "Task",
            "tasks",
            vec![make_version(1, vec![make_field("title", "String", None)])],
        )]);
        assert!(validate_schema(&schema).is_ok());
    }

    #[test]
    fn empty_output_fails() {
        let mut schema = make_schema(vec![make_entity(
            "Task",
            "tasks",
            vec![make_version(1, vec![make_field("title", "String", None)])],
        )]);
        schema.config.output = String::new();
        let errs = validate_schema(&schema).unwrap_err();
        assert!(errs.iter().any(|e| e.message.contains("output")));
    }

    #[test]
    fn non_contiguous_versions_fail() {
        let schema = make_schema(vec![make_entity(
            "Task",
            "tasks",
            vec![
                make_version(1, vec![make_field("title", "String", None)]),
                make_version(3, vec![make_field("title", "String", None)]),
            ],
        )]);
        let errs = validate_schema(&schema).unwrap_err();
        assert!(errs
            .iter()
            .any(|e| e.message.contains("expected version 2")));
    }

    #[test]
    fn new_field_without_default_fails() {
        let schema = make_schema(vec![make_entity(
            "Task",
            "tasks",
            vec![
                make_version(1, vec![make_field("title", "String", None)]),
                make_version(
                    2,
                    vec![
                        make_field("title", "String", None),
                        make_field("priority", "Option<u8>", None), // missing default!
                    ],
                ),
            ],
        )]);
        let errs = validate_schema(&schema).unwrap_err();
        assert!(errs.iter().any(|e| e.message.contains("default")));
    }

    #[test]
    fn new_field_with_default_passes() {
        let schema = make_schema(vec![make_entity(
            "Task",
            "tasks",
            vec![
                make_version(1, vec![make_field("title", "String", None)]),
                make_version(
                    2,
                    vec![
                        make_field("title", "String", None),
                        make_field("priority", "Option<u8>", Some("None")),
                    ],
                ),
            ],
        )]);
        assert!(validate_schema(&schema).is_ok());
    }

    #[test]
    fn unsupported_type_fails() {
        let schema = make_schema(vec![make_entity(
            "Task",
            "tasks",
            vec![make_version(
                1,
                vec![make_field("data", "HashMap<String, String>", None)],
            )],
        )]);
        let errs = validate_schema(&schema).unwrap_err();
        assert!(errs.iter().any(|e| e.message.contains("unsupported type")));
    }

    #[test]
    fn supported_types_pass() {
        let fields = vec![
            make_field("a", "String", None),
            make_field("b", "bool", None),
            make_field("c", "u8", None),
            make_field("d", "u64", None),
            make_field("e", "f32", None),
            make_field("f", "Option<String>", None),
            make_field("g", "Vec<u8>", None),
            make_field("h", "Option<Vec<String>>", None),
        ];
        let schema = make_schema(vec![make_entity(
            "Task",
            "tasks",
            vec![make_version(1, fields)],
        )]);
        assert!(validate_schema(&schema).is_ok());
    }

    #[test]
    fn duplicate_entity_names_fail() {
        let schema = make_schema(vec![
            make_entity(
                "Task",
                "tasks",
                vec![make_version(1, vec![make_field("title", "String", None)])],
            ),
            make_entity(
                "Task",
                "other",
                vec![make_version(1, vec![make_field("name", "String", None)])],
            ),
        ]);
        let errs = validate_schema(&schema).unwrap_err();
        assert!(errs.iter().any(|e| e.message.contains("duplicate entity")));
    }

    #[test]
    fn lowercase_entity_name_fails() {
        let schema = make_schema(vec![make_entity(
            "task",
            "tasks",
            vec![make_version(1, vec![make_field("title", "String", None)])],
        )]);
        let errs = validate_schema(&schema).unwrap_err();
        assert!(errs.iter().any(|e| e.message.contains("PascalCase")));
    }
}
