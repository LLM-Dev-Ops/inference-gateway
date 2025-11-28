//! Migration types and utilities.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Migration status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MigrationStatus {
    /// Migration is pending.
    Pending,
    /// Migration is currently running.
    Running,
    /// Migration completed successfully.
    Applied,
    /// Migration failed.
    Failed,
    /// Migration was rolled back.
    RolledBack,
}

impl fmt::Display for MigrationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Applied => write!(f, "applied"),
            Self::Failed => write!(f, "failed"),
            Self::RolledBack => write!(f, "rolled_back"),
        }
    }
}

/// A database migration.
#[derive(Debug, Clone)]
pub struct Migration {
    /// Migration version (timestamp-based recommended).
    pub version: i64,
    /// Migration name/description.
    pub name: String,
    /// SQL to apply the migration.
    pub up_sql: String,
    /// SQL to roll back the migration (optional).
    pub down_sql: Option<String>,
    /// Checksum of the migration SQL.
    pub checksum: String,
    /// Whether this migration is repeatable.
    pub repeatable: bool,
    /// Tags for categorizing migrations.
    pub tags: Vec<String>,
}

impl Migration {
    /// Create a new migration.
    #[must_use]
    pub fn new(version: i64, name: impl Into<String>, up_sql: impl Into<String>) -> Self {
        let up_sql = up_sql.into();
        let checksum = Self::compute_checksum(&up_sql);

        Self {
            version,
            name: name.into(),
            up_sql,
            down_sql: None,
            checksum,
            repeatable: false,
            tags: Vec::new(),
        }
    }

    /// Create a migration builder.
    #[must_use]
    pub fn builder(version: i64, name: impl Into<String>) -> MigrationBuilder {
        MigrationBuilder::new(version, name)
    }

    /// Set the down SQL for rollback.
    #[must_use]
    pub fn with_down(mut self, down_sql: impl Into<String>) -> Self {
        self.down_sql = Some(down_sql.into());
        self
    }

    /// Mark as repeatable.
    #[must_use]
    pub fn repeatable(mut self) -> Self {
        self.repeatable = true;
        self
    }

    /// Add tags.
    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// Compute checksum for SQL content.
    #[must_use]
    pub fn compute_checksum(sql: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(sql.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Verify the checksum.
    #[must_use]
    pub fn verify_checksum(&self) -> bool {
        Self::compute_checksum(&self.up_sql) == self.checksum
    }

    /// Check if rollback is supported.
    #[must_use]
    pub fn supports_rollback(&self) -> bool {
        self.down_sql.is_some()
    }

    /// Get a formatted version string.
    #[must_use]
    pub fn version_string(&self) -> String {
        format!("V{}", self.version)
    }
}

impl fmt::Display for Migration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "V{} - {}", self.version, self.name)
    }
}

/// Builder for migrations.
#[derive(Debug)]
pub struct MigrationBuilder {
    version: i64,
    name: String,
    up_sql: Option<String>,
    down_sql: Option<String>,
    repeatable: bool,
    tags: Vec<String>,
}

impl MigrationBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new(version: i64, name: impl Into<String>) -> Self {
        Self {
            version,
            name: name.into(),
            up_sql: None,
            down_sql: None,
            repeatable: false,
            tags: Vec::new(),
        }
    }

    /// Set the up SQL.
    #[must_use]
    pub fn up(mut self, sql: impl Into<String>) -> Self {
        self.up_sql = Some(sql.into());
        self
    }

    /// Set the down SQL.
    #[must_use]
    pub fn down(mut self, sql: impl Into<String>) -> Self {
        self.down_sql = Some(sql.into());
        self
    }

    /// Mark as repeatable.
    #[must_use]
    pub fn repeatable(mut self) -> Self {
        self.repeatable = true;
        self
    }

    /// Add a tag.
    #[must_use]
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add multiple tags.
    #[must_use]
    pub fn tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Build the migration.
    ///
    /// # Panics
    /// Panics if up SQL is not set.
    #[must_use]
    pub fn build(self) -> Migration {
        let up_sql = self.up_sql.expect("up SQL is required");
        let checksum = Migration::compute_checksum(&up_sql);

        Migration {
            version: self.version,
            name: self.name,
            up_sql,
            down_sql: self.down_sql,
            checksum,
            repeatable: self.repeatable,
            tags: self.tags,
        }
    }
}

/// Record of an applied migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRecord {
    /// Migration version.
    pub version: i64,
    /// Migration name.
    pub name: String,
    /// Checksum when applied.
    pub checksum: String,
    /// When the migration was applied.
    pub applied_at: DateTime<Utc>,
    /// Execution time in milliseconds.
    pub execution_time_ms: i64,
    /// Current status.
    pub status: MigrationStatus,
    /// Error message if failed.
    pub error: Option<String>,
    /// Who/what applied the migration.
    pub applied_by: Option<String>,
}

impl MigrationRecord {
    /// Create a new migration record.
    #[must_use]
    pub fn new(migration: &Migration) -> Self {
        Self {
            version: migration.version,
            name: migration.name.clone(),
            checksum: migration.checksum.clone(),
            applied_at: Utc::now(),
            execution_time_ms: 0,
            status: MigrationStatus::Pending,
            error: None,
            applied_by: None,
        }
    }

    /// Mark as running.
    #[must_use]
    pub fn running(mut self) -> Self {
        self.status = MigrationStatus::Running;
        self
    }

    /// Mark as applied with execution time.
    #[must_use]
    pub fn applied(mut self, execution_time_ms: i64) -> Self {
        self.status = MigrationStatus::Applied;
        self.execution_time_ms = execution_time_ms;
        self.applied_at = Utc::now();
        self
    }

    /// Mark as failed with error.
    #[must_use]
    pub fn failed(mut self, error: impl Into<String>) -> Self {
        self.status = MigrationStatus::Failed;
        self.error = Some(error.into());
        self
    }

    /// Mark as rolled back.
    #[must_use]
    pub fn rolled_back(mut self) -> Self {
        self.status = MigrationStatus::RolledBack;
        self
    }

    /// Set who applied the migration.
    #[must_use]
    pub fn applied_by(mut self, by: impl Into<String>) -> Self {
        self.applied_by = Some(by.into());
        self
    }

    /// Check if this record matches a migration.
    #[must_use]
    pub fn matches(&self, migration: &Migration) -> bool {
        self.version == migration.version && self.checksum == migration.checksum
    }

    /// Check if the migration was successful.
    #[must_use]
    pub fn is_successful(&self) -> bool {
        self.status == MigrationStatus::Applied
    }
}

impl fmt::Display for MigrationRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "V{} - {} [{}] ({}ms)",
            self.version, self.name, self.status, self.execution_time_ms
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_creation() {
        let m = Migration::new(1, "create_users", "CREATE TABLE users (id INT);");

        assert_eq!(m.version, 1);
        assert_eq!(m.name, "create_users");
        assert!(!m.checksum.is_empty());
        assert!(!m.supports_rollback());
    }

    #[test]
    fn test_migration_with_down() {
        let m = Migration::new(1, "create_users", "CREATE TABLE users (id INT);")
            .with_down("DROP TABLE users;");

        assert!(m.supports_rollback());
        assert_eq!(m.down_sql.unwrap(), "DROP TABLE users;");
    }

    #[test]
    fn test_migration_builder() {
        let m = Migration::builder(1, "create_users")
            .up("CREATE TABLE users (id INT);")
            .down("DROP TABLE users;")
            .tag("schema")
            .tag("users")
            .build();

        assert_eq!(m.version, 1);
        assert_eq!(m.name, "create_users");
        assert!(m.supports_rollback());
        assert_eq!(m.tags.len(), 2);
    }

    #[test]
    fn test_checksum_verification() {
        let m = Migration::new(1, "test", "SELECT 1;");
        assert!(m.verify_checksum());

        let checksum1 = Migration::compute_checksum("SELECT 1;");
        let checksum2 = Migration::compute_checksum("SELECT 2;");
        assert_ne!(checksum1, checksum2);
    }

    #[test]
    fn test_migration_record() {
        let m = Migration::new(1, "test", "SELECT 1;");
        let record = MigrationRecord::new(&m);

        assert_eq!(record.version, 1);
        assert_eq!(record.status, MigrationStatus::Pending);
        assert!(record.matches(&m));
    }

    #[test]
    fn test_migration_record_lifecycle() {
        let m = Migration::new(1, "test", "SELECT 1;");
        let record = MigrationRecord::new(&m)
            .running()
            .applied(100);

        assert_eq!(record.status, MigrationStatus::Applied);
        assert_eq!(record.execution_time_ms, 100);
        assert!(record.is_successful());
    }

    #[test]
    fn test_migration_record_failed() {
        let m = Migration::new(1, "test", "SELECT 1;");
        let record = MigrationRecord::new(&m)
            .running()
            .failed("Syntax error");

        assert_eq!(record.status, MigrationStatus::Failed);
        assert_eq!(record.error.as_ref().unwrap(), "Syntax error");
        assert!(!record.is_successful());
    }

    #[test]
    fn test_migration_status_display() {
        assert_eq!(MigrationStatus::Pending.to_string(), "pending");
        assert_eq!(MigrationStatus::Applied.to_string(), "applied");
        assert_eq!(MigrationStatus::Failed.to_string(), "failed");
    }

    #[test]
    fn test_migration_display() {
        let m = Migration::new(20240101000000, "create_users", "CREATE TABLE users;");
        assert!(m.to_string().contains("V20240101000000"));
        assert!(m.to_string().contains("create_users"));
    }
}
