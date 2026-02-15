/// Trait for types with a versioned schema.
///
/// Every CRDT model that supports migration implements this trait.
/// It provides the metadata the migration engine needs to determine
/// whether data needs migration and which steps to run.
///
/// In the future, the `#[crdt_schema]` proc macro will generate
/// this implementation automatically.
pub trait Schema: Sized {
    /// Current schema version of this type.
    const VERSION: u32;

    /// Minimum schema version that can be migrated to current.
    /// Data older than this version cannot be read.
    const MIN_SUPPORTED_VERSION: u32;

    /// Name of the storage namespace (table) for this model.
    const NAMESPACE: &'static str;
}
