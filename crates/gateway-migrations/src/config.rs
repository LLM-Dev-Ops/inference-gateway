//! Migration configuration.

use crate::error::{MigrationError, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Database type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    /// PostgreSQL database.
    #[default]
    PostgreSQL,
    /// SQLite database.
    SQLite,
}

impl DatabaseType {
    /// Parse from a database URL.
    #[must_use]
    pub fn from_url(url: &str) -> Option<Self> {
        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            Some(Self::PostgreSQL)
        } else if url.starts_with("sqlite://") || url.starts_with("sqlite:") {
            Some(Self::SQLite)
        } else {
            None
        }
    }
}

impl std::fmt::Display for DatabaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PostgreSQL => write!(f, "postgresql"),
            Self::SQLite => write!(f, "sqlite"),
        }
    }
}

/// Migration configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    /// Database connection URL.
    pub database_url: String,

    /// Database type (auto-detected if not specified).
    pub database_type: DatabaseType,

    /// Schema name for migrations table (PostgreSQL only).
    #[serde(default = "default_schema")]
    pub schema: String,

    /// Migrations table name.
    #[serde(default = "default_table_name")]
    pub table_name: String,

    /// Connection timeout.
    #[serde(with = "humantime_serde", default = "default_connect_timeout")]
    pub connect_timeout: Duration,

    /// Migration execution timeout.
    #[serde(with = "humantime_serde", default = "default_migration_timeout")]
    pub migration_timeout: Duration,

    /// Maximum connection pool size.
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Whether to run migrations in a transaction.
    #[serde(default = "default_true")]
    pub use_transactions: bool,

    /// Whether to verify checksums.
    #[serde(default = "default_true")]
    pub verify_checksums: bool,

    /// Whether to allow out-of-order migrations.
    #[serde(default)]
    pub allow_out_of_order: bool,

    /// Lock timeout for migration lock.
    #[serde(with = "humantime_serde", default = "default_lock_timeout")]
    pub lock_timeout: Duration,
}

fn default_schema() -> String {
    "public".to_string()
}

fn default_table_name() -> String {
    "_migrations".to_string()
}

fn default_connect_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_migration_timeout() -> Duration {
    Duration::from_secs(300)
}

fn default_max_connections() -> u32 {
    5
}

fn default_true() -> bool {
    true
}

fn default_lock_timeout() -> Duration {
    Duration::from_secs(60)
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            database_url: String::new(),
            database_type: DatabaseType::PostgreSQL,
            schema: default_schema(),
            table_name: default_table_name(),
            connect_timeout: default_connect_timeout(),
            migration_timeout: default_migration_timeout(),
            max_connections: default_max_connections(),
            use_transactions: true,
            verify_checksums: true,
            allow_out_of_order: false,
            lock_timeout: default_lock_timeout(),
        }
    }
}

impl MigrationConfig {
    /// Create a new configuration builder.
    #[must_use]
    pub fn builder() -> MigrationConfigBuilder {
        MigrationConfigBuilder::new()
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<()> {
        if self.database_url.is_empty() {
            return Err(MigrationError::config("Database URL is required"));
        }

        if self.table_name.is_empty() {
            return Err(MigrationError::config("Table name is required"));
        }

        if self.max_connections == 0 {
            return Err(MigrationError::config(
                "Max connections must be greater than 0",
            ));
        }

        Ok(())
    }

    /// Get the full table name with schema.
    #[must_use]
    pub fn full_table_name(&self) -> String {
        match self.database_type {
            DatabaseType::PostgreSQL => format!("{}.{}", self.schema, self.table_name),
            DatabaseType::SQLite => self.table_name.clone(),
        }
    }
}

/// Builder for migration configuration.
#[derive(Debug, Default)]
pub struct MigrationConfigBuilder {
    config: MigrationConfig,
}

impl MigrationConfigBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the database URL.
    #[must_use]
    pub fn database_url(mut self, url: impl Into<String>) -> Self {
        let url = url.into();
        // Auto-detect database type if possible
        if let Some(db_type) = DatabaseType::from_url(&url) {
            self.config.database_type = db_type;
        }
        self.config.database_url = url;
        self
    }

    /// Set the database type.
    #[must_use]
    pub fn database_type(mut self, db_type: DatabaseType) -> Self {
        self.config.database_type = db_type;
        self
    }

    /// Set the schema name.
    #[must_use]
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.config.schema = schema.into();
        self
    }

    /// Set the migrations table name.
    #[must_use]
    pub fn table_name(mut self, name: impl Into<String>) -> Self {
        self.config.table_name = name.into();
        self
    }

    /// Set the connection timeout.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = timeout;
        self
    }

    /// Set the migration timeout.
    #[must_use]
    pub fn migration_timeout(mut self, timeout: Duration) -> Self {
        self.config.migration_timeout = timeout;
        self
    }

    /// Set maximum connections.
    #[must_use]
    pub fn max_connections(mut self, max: u32) -> Self {
        self.config.max_connections = max;
        self
    }

    /// Enable or disable transactions.
    #[must_use]
    pub fn use_transactions(mut self, use_tx: bool) -> Self {
        self.config.use_transactions = use_tx;
        self
    }

    /// Enable or disable checksum verification.
    #[must_use]
    pub fn verify_checksums(mut self, verify: bool) -> Self {
        self.config.verify_checksums = verify;
        self
    }

    /// Allow out-of-order migrations.
    #[must_use]
    pub fn allow_out_of_order(mut self, allow: bool) -> Self {
        self.config.allow_out_of_order = allow;
        self
    }

    /// Set the lock timeout.
    #[must_use]
    pub fn lock_timeout(mut self, timeout: Duration) -> Self {
        self.config.lock_timeout = timeout;
        self
    }

    /// Build the configuration.
    pub fn build(self) -> Result<MigrationConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_type_from_url() {
        assert_eq!(
            DatabaseType::from_url("postgres://localhost/db"),
            Some(DatabaseType::PostgreSQL)
        );
        assert_eq!(
            DatabaseType::from_url("postgresql://localhost/db"),
            Some(DatabaseType::PostgreSQL)
        );
        assert_eq!(
            DatabaseType::from_url("sqlite://test.db"),
            Some(DatabaseType::SQLite)
        );
        assert_eq!(
            DatabaseType::from_url("sqlite:test.db"),
            Some(DatabaseType::SQLite)
        );
        assert_eq!(DatabaseType::from_url("mysql://localhost/db"), None);
    }

    #[test]
    fn test_database_type_display() {
        assert_eq!(DatabaseType::PostgreSQL.to_string(), "postgresql");
        assert_eq!(DatabaseType::SQLite.to_string(), "sqlite");
    }

    #[test]
    fn test_config_builder() {
        let config = MigrationConfig::builder()
            .database_url("postgres://localhost/gateway")
            .schema("app")
            .table_name("migrations")
            .max_connections(10)
            .use_transactions(true)
            .build()
            .unwrap();

        assert_eq!(config.database_url, "postgres://localhost/gateway");
        assert_eq!(config.database_type, DatabaseType::PostgreSQL);
        assert_eq!(config.schema, "app");
        assert_eq!(config.table_name, "migrations");
        assert_eq!(config.max_connections, 10);
        assert!(config.use_transactions);
    }

    #[test]
    fn test_config_validation() {
        let result = MigrationConfig::builder().build();
        assert!(result.is_err());

        let result = MigrationConfig::builder()
            .database_url("postgres://localhost/db")
            .table_name("")
            .build();
        assert!(result.is_err());

        let result = MigrationConfig::builder()
            .database_url("postgres://localhost/db")
            .max_connections(0)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_full_table_name() {
        let config = MigrationConfig {
            database_type: DatabaseType::PostgreSQL,
            schema: "app".to_string(),
            table_name: "migrations".to_string(),
            ..Default::default()
        };
        assert_eq!(config.full_table_name(), "app.migrations");

        let config = MigrationConfig {
            database_type: DatabaseType::SQLite,
            table_name: "migrations".to_string(),
            ..Default::default()
        };
        assert_eq!(config.full_table_name(), "migrations");
    }

    #[test]
    fn test_default_config() {
        let config = MigrationConfig::default();
        assert_eq!(config.schema, "public");
        assert_eq!(config.table_name, "_migrations");
        assert_eq!(config.max_connections, 5);
        assert!(config.use_transactions);
        assert!(config.verify_checksums);
        assert!(!config.allow_out_of_order);
    }
}
