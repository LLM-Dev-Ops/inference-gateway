//! Migration runner.

use crate::config::{DatabaseType, MigrationConfig};
use crate::error::{MigrationError, Result};
use crate::migration::{Migration, MigrationRecord, MigrationStatus};
use crate::pool::DatabasePool;
use chrono::Utc;
use sqlx::{Executor, Row};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// Migration runner.
pub struct Migrator {
    pool: Arc<DatabasePool>,
    migrations: Vec<Migration>,
    config: Arc<MigrationConfig>,
}

impl Migrator {
    /// Create a new migrator.
    pub async fn new(config: MigrationConfig) -> Result<Self> {
        let pool = DatabasePool::new(config.clone()).await?;

        Ok(Self {
            pool: Arc::new(pool),
            migrations: Vec::new(),
            config: Arc::new(config),
        })
    }

    /// Create a migrator with an existing pool.
    #[must_use]
    pub fn with_pool(pool: Arc<DatabasePool>, config: MigrationConfig) -> Self {
        Self {
            pool,
            migrations: Vec::new(),
            config: Arc::new(config),
        }
    }

    /// Add a migration.
    pub fn add_migration(&mut self, migration: Migration) -> &mut Self {
        self.migrations.push(migration);
        self.migrations.sort_by_key(|m| m.version);
        self
    }

    /// Add multiple migrations.
    pub fn add_migrations(&mut self, migrations: impl IntoIterator<Item = Migration>) -> &mut Self {
        self.migrations.extend(migrations);
        self.migrations.sort_by_key(|m| m.version);
        self
    }

    /// Get the list of migrations.
    #[must_use]
    pub fn migrations(&self) -> &[Migration] {
        &self.migrations
    }

    /// Initialize the migrations table.
    pub async fn init(&self) -> Result<()> {
        info!("Initializing migrations table");

        let sql = match self.config.database_type {
            DatabaseType::PostgreSQL => self.postgres_init_sql(),
            DatabaseType::SQLite => self.sqlite_init_sql(),
        };

        sqlx::query(&sql)
            .execute(self.pool.inner())
            .await
            .map_err(|e| MigrationError::Execution(e.to_string()))?;

        debug!("Migrations table initialized");
        Ok(())
    }

    fn postgres_init_sql(&self) -> String {
        format!(
            r#"
            CREATE SCHEMA IF NOT EXISTS {schema};

            CREATE TABLE IF NOT EXISTS {table} (
                version BIGINT PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                checksum VARCHAR(64) NOT NULL,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                execution_time_ms BIGINT NOT NULL DEFAULT 0,
                status VARCHAR(20) NOT NULL DEFAULT 'applied',
                error TEXT,
                applied_by VARCHAR(255)
            );

            CREATE INDEX IF NOT EXISTS idx_{table_name}_status ON {table}(status);
            CREATE INDEX IF NOT EXISTS idx_{table_name}_applied_at ON {table}(applied_at);
            "#,
            schema = self.config.schema,
            table = self.config.full_table_name(),
            table_name = self.config.table_name,
        )
    }

    fn sqlite_init_sql(&self) -> String {
        format!(
            r#"
            CREATE TABLE IF NOT EXISTS {table} (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                checksum TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT (datetime('now')),
                execution_time_ms INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'applied',
                error TEXT,
                applied_by TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_{table}_status ON {table}(status);
            CREATE INDEX IF NOT EXISTS idx_{table}_applied_at ON {table}(applied_at);
            "#,
            table = self.config.table_name,
        )
    }

    /// Get applied migrations.
    pub async fn get_applied(&self) -> Result<Vec<MigrationRecord>> {
        let sql = format!(
            "SELECT version, name, checksum, applied_at, execution_time_ms, status, error, applied_by
             FROM {}
             ORDER BY version",
            self.config.full_table_name()
        );

        let rows = sqlx::query(&sql)
            .fetch_all(self.pool.inner())
            .await
            .map_err(|e| MigrationError::Execution(e.to_string()))?;

        let mut records = Vec::new();
        for row in rows {
            let status_str: String = row.get("status");
            let status = match status_str.as_str() {
                "pending" => MigrationStatus::Pending,
                "running" => MigrationStatus::Running,
                "applied" => MigrationStatus::Applied,
                "failed" => MigrationStatus::Failed,
                "rolled_back" => MigrationStatus::RolledBack,
                _ => MigrationStatus::Applied,
            };

            let applied_at: String = row.get("applied_at");
            let applied_at = chrono::DateTime::parse_from_rfc3339(&applied_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            records.push(MigrationRecord {
                version: row.get("version"),
                name: row.get("name"),
                checksum: row.get("checksum"),
                applied_at,
                execution_time_ms: row.get("execution_time_ms"),
                status,
                error: row.get("error"),
                applied_by: row.get("applied_by"),
            });
        }

        Ok(records)
    }

    /// Get pending migrations.
    pub async fn get_pending(&self) -> Result<Vec<&Migration>> {
        let applied = self.get_applied().await?;
        let applied_versions: std::collections::HashSet<i64> =
            applied.iter().map(|r| r.version).collect();

        let pending: Vec<_> = self
            .migrations
            .iter()
            .filter(|m| !applied_versions.contains(&m.version))
            .collect();

        Ok(pending)
    }

    /// Run all pending migrations.
    pub async fn run_pending(&self) -> Result<Vec<MigrationRecord>> {
        self.init().await?;

        let pending = self.get_pending().await?;
        if pending.is_empty() {
            info!("No pending migrations");
            return Ok(Vec::new());
        }

        info!("Running {} pending migration(s)", pending.len());

        let mut results = Vec::new();
        for migration in pending {
            let result = self.run_migration(migration).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Run a specific migration.
    pub async fn run_migration(&self, migration: &Migration) -> Result<MigrationRecord> {
        info!(version = migration.version, name = %migration.name, "Running migration");

        // Check if already applied
        let applied = self.get_applied().await?;
        if applied.iter().any(|r| r.version == migration.version) {
            return Err(MigrationError::AlreadyApplied {
                version: migration.version,
            });
        }

        // Verify checksum if configured
        if self.config.verify_checksums && !migration.verify_checksum() {
            return Err(MigrationError::ChecksumMismatch {
                version: migration.version,
                expected: migration.checksum.clone(),
                actual: Migration::compute_checksum(&migration.up_sql),
            });
        }

        let start = Instant::now();
        let mut record = MigrationRecord::new(migration).running();

        // Execute migration
        let result = if self.config.use_transactions {
            self.execute_in_transaction(&migration.up_sql).await
        } else {
            self.execute_sql(&migration.up_sql).await
        };

        let execution_time = start.elapsed().as_millis() as i64;

        match result {
            Ok(()) => {
                record = record.applied(execution_time);
                self.save_record(&record).await?;
                info!(
                    version = migration.version,
                    name = %migration.name,
                    execution_time_ms = execution_time,
                    "Migration applied successfully"
                );
            }
            Err(e) => {
                record = record.failed(e.to_string());
                // Try to save the failed record
                if let Err(save_err) = self.save_record(&record).await {
                    warn!("Failed to save migration failure record: {}", save_err);
                }
                error!(
                    version = migration.version,
                    name = %migration.name,
                    error = %e,
                    "Migration failed"
                );
                return Err(e);
            }
        }

        Ok(record)
    }

    /// Roll back a migration.
    pub async fn rollback(&self, version: i64) -> Result<MigrationRecord> {
        let migration = self
            .migrations
            .iter()
            .find(|m| m.version == version)
            .ok_or(MigrationError::NotFound { version })?;

        let down_sql = migration
            .down_sql
            .as_ref()
            .ok_or(MigrationError::RollbackNotSupported { version })?;

        info!(version, name = %migration.name, "Rolling back migration");

        let start = Instant::now();

        let result = if self.config.use_transactions {
            self.execute_in_transaction(down_sql).await
        } else {
            self.execute_sql(down_sql).await
        };

        let execution_time = start.elapsed().as_millis() as i64;

        match result {
            Ok(()) => {
                // Update record to rolled back
                let sql = format!(
                    "UPDATE {} SET status = 'rolled_back' WHERE version = $1",
                    self.config.full_table_name()
                );
                sqlx::query(&sql)
                    .bind(version)
                    .execute(self.pool.inner())
                    .await
                    .map_err(|e| MigrationError::Execution(e.to_string()))?;

                info!(
                    version,
                    name = %migration.name,
                    execution_time_ms = execution_time,
                    "Migration rolled back successfully"
                );

                let mut record = MigrationRecord::new(migration);
                record.status = MigrationStatus::RolledBack;
                record.execution_time_ms = execution_time;
                Ok(record)
            }
            Err(e) => {
                error!(
                    version,
                    name = %migration.name,
                    error = %e,
                    "Rollback failed"
                );
                Err(e)
            }
        }
    }

    /// Roll back the last N migrations.
    pub async fn rollback_last(&self, count: usize) -> Result<Vec<MigrationRecord>> {
        let applied = self.get_applied().await?;
        let to_rollback: Vec<_> = applied
            .iter()
            .rev()
            .filter(|r| r.status == MigrationStatus::Applied)
            .take(count)
            .map(|r| r.version)
            .collect();

        let mut results = Vec::new();
        for version in to_rollback {
            let result = self.rollback(version).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Get migration status.
    pub async fn status(&self) -> Result<MigrationStatus> {
        self.init().await?;

        let applied = self.get_applied().await?;
        let applied_map: HashMap<i64, &MigrationRecord> =
            applied.iter().map(|r| (r.version, r)).collect();

        let mut status = Vec::new();

        for migration in &self.migrations {
            let record_status = if let Some(record) = applied_map.get(&migration.version) {
                // Check checksum
                if self.config.verify_checksums && record.checksum != migration.checksum {
                    warn!(
                        version = migration.version,
                        "Checksum mismatch detected"
                    );
                }
                record.status
            } else {
                MigrationStatus::Pending
            };
            status.push((migration.clone(), record_status));
        }

        // Return overall status
        if status.iter().any(|(_, s)| *s == MigrationStatus::Failed) {
            Ok(MigrationStatus::Failed)
        } else if status.iter().any(|(_, s)| *s == MigrationStatus::Running) {
            Ok(MigrationStatus::Running)
        } else if status.iter().all(|(_, s)| *s == MigrationStatus::Applied) {
            Ok(MigrationStatus::Applied)
        } else {
            Ok(MigrationStatus::Pending)
        }
    }

    /// Get detailed status for all migrations.
    pub async fn detailed_status(&self) -> Result<Vec<(Migration, MigrationStatus, Option<MigrationRecord>)>> {
        self.init().await?;

        let applied = self.get_applied().await?;
        let applied_map: HashMap<i64, MigrationRecord> =
            applied.into_iter().map(|r| (r.version, r)).collect();

        let status: Vec<_> = self
            .migrations
            .iter()
            .map(|m| {
                let (status, record) = if let Some(record) = applied_map.get(&m.version) {
                    (record.status, Some(record.clone()))
                } else {
                    (MigrationStatus::Pending, None)
                };
                (m.clone(), status, record)
            })
            .collect();

        Ok(status)
    }

    /// Validate all migrations.
    pub async fn validate(&self) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        // Check for duplicate versions
        let mut versions = std::collections::HashSet::new();
        for migration in &self.migrations {
            if !versions.insert(migration.version) {
                issues.push(ValidationIssue::DuplicateVersion(migration.version));
            }
        }

        // Verify checksums
        if self.config.verify_checksums {
            for migration in &self.migrations {
                if !migration.verify_checksum() {
                    issues.push(ValidationIssue::InvalidChecksum(migration.version));
                }
            }
        }

        // Check applied migrations
        if let Ok(applied) = self.get_applied().await {
            let applied_map: HashMap<i64, &MigrationRecord> =
                applied.iter().map(|r| (r.version, r)).collect();

            for migration in &self.migrations {
                if let Some(record) = applied_map.get(&migration.version) {
                    if self.config.verify_checksums && record.checksum != migration.checksum {
                        issues.push(ValidationIssue::ChecksumMismatch {
                            version: migration.version,
                            expected: migration.checksum.clone(),
                            actual: record.checksum.clone(),
                        });
                    }
                }
            }
        }

        Ok(issues)
    }

    async fn execute_sql(&self, sql: &str) -> Result<()> {
        sqlx::query(sql)
            .execute(self.pool.inner())
            .await
            .map_err(|e| MigrationError::Execution(e.to_string()))?;
        Ok(())
    }

    async fn execute_in_transaction(&self, sql: &str) -> Result<()> {
        let mut tx = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|e| MigrationError::Execution(e.to_string()))?;

        // Split by semicolons and execute each statement
        for statement in sql.split(';') {
            let statement = statement.trim();
            if !statement.is_empty() {
                tx.execute(sqlx::query(statement))
                    .await
                    .map_err(|e| MigrationError::Execution(e.to_string()))?;
            }
        }

        tx.commit()
            .await
            .map_err(|e| MigrationError::Execution(e.to_string()))?;

        Ok(())
    }

    async fn save_record(&self, record: &MigrationRecord) -> Result<()> {
        let sql = match self.config.database_type {
            DatabaseType::PostgreSQL => format!(
                r#"
                INSERT INTO {} (version, name, checksum, applied_at, execution_time_ms, status, error, applied_by)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (version) DO UPDATE SET
                    status = EXCLUDED.status,
                    execution_time_ms = EXCLUDED.execution_time_ms,
                    error = EXCLUDED.error
                "#,
                self.config.full_table_name()
            ),
            DatabaseType::SQLite => format!(
                r#"
                INSERT OR REPLACE INTO {} (version, name, checksum, applied_at, execution_time_ms, status, error, applied_by)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                self.config.table_name
            ),
        };

        sqlx::query(&sql)
            .bind(record.version)
            .bind(&record.name)
            .bind(&record.checksum)
            .bind(record.applied_at.to_rfc3339())
            .bind(record.execution_time_ms)
            .bind(record.status.to_string())
            .bind(&record.error)
            .bind(&record.applied_by)
            .execute(self.pool.inner())
            .await
            .map_err(|e| MigrationError::Execution(e.to_string()))?;

        Ok(())
    }

    /// Get the database pool.
    #[must_use]
    pub fn pool(&self) -> Arc<DatabasePool> {
        Arc::clone(&self.pool)
    }
}

/// Validation issue.
#[derive(Debug, Clone)]
pub enum ValidationIssue {
    /// Duplicate migration version.
    DuplicateVersion(i64),
    /// Invalid checksum.
    InvalidChecksum(i64),
    /// Checksum mismatch with applied migration.
    ChecksumMismatch {
        /// Migration version.
        version: i64,
        /// Expected checksum.
        expected: String,
        /// Actual checksum.
        actual: String,
    },
    /// Missing rollback SQL.
    MissingRollback(i64),
}

impl std::fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateVersion(v) => write!(f, "Duplicate version: {}", v),
            Self::InvalidChecksum(v) => write!(f, "Invalid checksum for version: {}", v),
            Self::ChecksumMismatch {
                version,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Checksum mismatch for version {}: expected {}, got {}",
                    version, expected, actual
                )
            }
            Self::MissingRollback(v) => write!(f, "Missing rollback SQL for version: {}", v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_issue_display() {
        let issue = ValidationIssue::DuplicateVersion(1);
        assert!(issue.to_string().contains("Duplicate"));

        let issue = ValidationIssue::ChecksumMismatch {
            version: 1,
            expected: "abc".to_string(),
            actual: "def".to_string(),
        };
        assert!(issue.to_string().contains("mismatch"));
    }
}
