use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

/// A single migration step that transforms data from one version to the next.
///
/// Migrations form a linear chain: v1→v2, v2→v3, etc.
/// Each step must be **deterministic and pure** — two devices running the
/// same migration on the same data must produce identical results.
pub trait MigrationStep: Send + Sync {
    /// Source version.
    fn source_version(&self) -> u32;
    /// Target version.
    fn target_version(&self) -> u32;
    /// Transform serialized data from source to target version.
    fn migrate(&self, data: &[u8]) -> Result<Vec<u8>, MigrationError>;
}

/// Error during migration.
#[derive(Debug, Clone, PartialEq)]
pub enum MigrationError {
    /// No migration path exists between the source and target versions.
    NoPath { from: u32, to: u32 },
    /// A migration step failed.
    StepFailed { from: u32, to: u32, reason: String },
    /// The version chain has a gap (e.g., v1→v3 without v2).
    GapInChain { missing: u32 },
    /// The source version is newer than the current version (forward compat).
    FutureVersion { found: u32, current: u32 },
    /// Deserialization failed.
    Deserialization(String),
    /// Serialization failed.
    Serialization(String),
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPath { from, to } => {
                write!(f, "no migration path from v{from} to v{to}")
            }
            Self::StepFailed { from, to, reason } => {
                write!(f, "migration v{from}→v{to} failed: {reason}")
            }
            Self::GapInChain { missing } => {
                write!(f, "missing migration step for v{missing}")
            }
            Self::FutureVersion { found, current } => {
                write!(f, "data version v{found} is newer than current v{current}")
            }
            Self::Deserialization(msg) => write!(f, "deserialization error: {msg}"),
            Self::Serialization(msg) => write!(f, "serialization error: {msg}"),
        }
    }
}

/// Configuration for the migration engine.
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    /// If true, re-write migrated data back to storage after reading.
    /// Recommended for SQLite/redb. Not recommended for flash storage.
    pub write_back_on_read: bool,
    /// If true, migrate all data eagerly on startup instead of lazily.
    pub eager_migration: bool,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            write_back_on_read: true,
            eager_migration: false,
        }
    }
}

/// The migration engine that runs a chain of migration steps.
///
/// Steps are registered in order and form a linear chain.
/// When data at version N needs to reach version M (where N < M),
/// the engine runs steps N→N+1, N+1→N+2, ..., M-1→M in sequence.
///
/// # Example
///
/// ```
/// use crdt_migrate::{MigrationEngine, MigrationStep, MigrationError};
///
/// struct AddFieldMigration;
///
/// impl MigrationStep for AddFieldMigration {
///     fn source_version(&self) -> u32 { 1 }
///     fn target_version(&self) -> u32 { 2 }
///     fn migrate(&self, data: &[u8]) -> Result<Vec<u8>, MigrationError> {
///         // In real code: deserialize v1, create v2 with new fields, serialize
///         let mut result = data.to_vec();
///         result.extend_from_slice(b"|humidity=none");
///         Ok(result)
///     }
/// }
///
/// let mut engine = MigrationEngine::new(2); // current version = 2
/// engine.register(Box::new(AddFieldMigration));
///
/// let v1_data = b"temp=22.5";
/// let v2_data = engine.migrate_to_current(v1_data, 1).unwrap();
/// assert_eq!(v2_data, b"temp=22.5|humidity=none");
/// ```
pub struct MigrationEngine {
    current_version: u32,
    steps: Vec<Box<dyn MigrationStep>>,
}

impl MigrationEngine {
    /// Create a new engine targeting `current_version`.
    pub fn new(current_version: u32) -> Self {
        Self {
            current_version,
            steps: Vec::new(),
        }
    }

    /// Register a migration step.
    pub fn register(&mut self, step: Box<dyn MigrationStep>) {
        self.steps.push(step);
        // Keep sorted by from_version for efficient lookup
        self.steps.sort_by_key(|s| s.source_version());
    }

    /// The current (target) schema version.
    pub fn current_version(&self) -> u32 {
        self.current_version
    }

    /// Check if data needs migration.
    pub fn needs_migration(&self, data_version: u32) -> bool {
        data_version != self.current_version
    }

    /// Migrate data from `from_version` to `current_version`.
    ///
    /// Runs the chain of steps sequentially. Each step receives the
    /// output of the previous step.
    pub fn migrate_to_current(
        &self,
        data: &[u8],
        from_version: u32,
    ) -> Result<Vec<u8>, MigrationError> {
        if from_version == self.current_version {
            return Ok(data.to_vec());
        }

        if from_version > self.current_version {
            return Err(MigrationError::FutureVersion {
                found: from_version,
                current: self.current_version,
            });
        }

        let mut current_data = data.to_vec();
        let mut version = from_version;

        while version < self.current_version {
            let step = self
                .steps
                .iter()
                .find(|s| s.source_version() == version)
                .ok_or(MigrationError::GapInChain { missing: version })?;

            current_data = step
                .migrate(&current_data)
                .map_err(|e| MigrationError::StepFailed {
                    from: version,
                    to: step.target_version(),
                    reason: e.to_string(),
                })?;

            version = step.target_version();
        }

        Ok(current_data)
    }

    /// Validate that the migration chain is complete from `min_version` to `current_version`.
    pub fn validate_chain(&self, min_version: u32) -> Result<(), MigrationError> {
        let mut version = min_version;
        while version < self.current_version {
            if !self.steps.iter().any(|s| s.source_version() == version) {
                return Err(MigrationError::GapInChain { missing: version });
            }
            version += 1;
        }
        Ok(())
    }

    /// List all registered migration steps as (from, to) pairs.
    pub fn registered_steps(&self) -> Vec<(u32, u32)> {
        self.steps
            .iter()
            .map(|s| (s.source_version(), s.target_version()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SimpleStep {
        from: u32,
        to: u32,
        suffix: &'static [u8],
    }

    impl MigrationStep for SimpleStep {
        fn source_version(&self) -> u32 {
            self.from
        }
        fn target_version(&self) -> u32 {
            self.to
        }
        fn migrate(&self, data: &[u8]) -> Result<Vec<u8>, MigrationError> {
            let mut result = data.to_vec();
            result.extend_from_slice(self.suffix);
            Ok(result)
        }
    }

    #[test]
    fn no_migration_needed() {
        let engine = MigrationEngine::new(1);
        let data = b"hello";
        let result = engine.migrate_to_current(data, 1).unwrap();
        assert_eq!(result, b"hello");
    }

    #[test]
    fn single_step_migration() {
        let mut engine = MigrationEngine::new(2);
        engine.register(Box::new(SimpleStep {
            from: 1,
            to: 2,
            suffix: b"+v2",
        }));

        let result = engine.migrate_to_current(b"data", 1).unwrap();
        assert_eq!(result, b"data+v2");
    }

    #[test]
    fn multi_step_chain() {
        let mut engine = MigrationEngine::new(4);
        engine.register(Box::new(SimpleStep {
            from: 1,
            to: 2,
            suffix: b"+v2",
        }));
        engine.register(Box::new(SimpleStep {
            from: 2,
            to: 3,
            suffix: b"+v3",
        }));
        engine.register(Box::new(SimpleStep {
            from: 3,
            to: 4,
            suffix: b"+v4",
        }));

        // Full chain: v1 → v4
        let result = engine.migrate_to_current(b"v1", 1).unwrap();
        assert_eq!(result, b"v1+v2+v3+v4");

        // Partial chain: v2 → v4
        let result = engine.migrate_to_current(b"v2", 2).unwrap();
        assert_eq!(result, b"v2+v3+v4");

        // Single step: v3 → v4
        let result = engine.migrate_to_current(b"v3", 3).unwrap();
        assert_eq!(result, b"v3+v4");
    }

    #[test]
    fn future_version_error() {
        let engine = MigrationEngine::new(2);
        let err = engine.migrate_to_current(b"data", 5).unwrap_err();
        assert_eq!(
            err,
            MigrationError::FutureVersion {
                found: 5,
                current: 2
            }
        );
    }

    #[test]
    fn gap_in_chain_error() {
        let mut engine = MigrationEngine::new(3);
        engine.register(Box::new(SimpleStep {
            from: 1,
            to: 2,
            suffix: b"+v2",
        }));
        // Missing v2→v3 step

        let err = engine.migrate_to_current(b"data", 1).unwrap_err();
        assert_eq!(err, MigrationError::GapInChain { missing: 2 });
    }

    #[test]
    fn validate_chain_ok() {
        let mut engine = MigrationEngine::new(3);
        engine.register(Box::new(SimpleStep {
            from: 1,
            to: 2,
            suffix: b"",
        }));
        engine.register(Box::new(SimpleStep {
            from: 2,
            to: 3,
            suffix: b"",
        }));

        assert!(engine.validate_chain(1).is_ok());
    }

    #[test]
    fn validate_chain_gap() {
        let mut engine = MigrationEngine::new(3);
        engine.register(Box::new(SimpleStep {
            from: 1,
            to: 2,
            suffix: b"",
        }));
        // Missing v2→v3

        let err = engine.validate_chain(1).unwrap_err();
        assert_eq!(err, MigrationError::GapInChain { missing: 2 });
    }

    #[test]
    fn needs_migration() {
        let engine = MigrationEngine::new(3);
        assert!(engine.needs_migration(1));
        assert!(engine.needs_migration(2));
        assert!(!engine.needs_migration(3));
    }

    #[test]
    fn registered_steps_list() {
        let mut engine = MigrationEngine::new(3);
        engine.register(Box::new(SimpleStep {
            from: 2,
            to: 3,
            suffix: b"",
        }));
        engine.register(Box::new(SimpleStep {
            from: 1,
            to: 2,
            suffix: b"",
        }));

        let steps = engine.registered_steps();
        assert_eq!(steps, vec![(1, 2), (2, 3)]); // sorted by from
    }
}
