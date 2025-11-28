//! Database connection pool management.

use crate::config::{DatabaseType, MigrationConfig};
use crate::error::{MigrationError, Result};
use serde::{Deserialize, Serialize};
use sqlx::{AnyPool, any::AnyPoolOptions};
use std::sync::Arc;
use std::time::Duration;

/// Pool configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// Maximum number of connections.
    pub max_connections: u32,
    /// Minimum number of connections.
    pub min_connections: u32,
    /// Connection timeout.
    #[serde(with = "humantime_serde")]
    pub connect_timeout: Duration,
    /// Idle timeout for connections.
    #[serde(with = "humantime_serde")]
    pub idle_timeout: Duration,
    /// Maximum lifetime for a connection.
    #[serde(with = "humantime_serde")]
    pub max_lifetime: Duration,
    /// Whether to test connections on checkout.
    pub test_on_acquire: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 1,
            connect_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(1800),
            test_on_acquire: true,
        }
    }
}

impl PoolConfig {
    /// Create a new builder.
    #[must_use]
    pub fn builder() -> PoolConfigBuilder {
        PoolConfigBuilder::default()
    }
}

/// Builder for pool configuration.
#[derive(Debug, Default)]
pub struct PoolConfigBuilder {
    config: PoolConfig,
}

impl PoolConfigBuilder {
    /// Set maximum connections.
    #[must_use]
    pub fn max_connections(mut self, max: u32) -> Self {
        self.config.max_connections = max;
        self
    }

    /// Set minimum connections.
    #[must_use]
    pub fn min_connections(mut self, min: u32) -> Self {
        self.config.min_connections = min;
        self
    }

    /// Set connection timeout.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = timeout;
        self
    }

    /// Set idle timeout.
    #[must_use]
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.config.idle_timeout = timeout;
        self
    }

    /// Set max lifetime.
    #[must_use]
    pub fn max_lifetime(mut self, lifetime: Duration) -> Self {
        self.config.max_lifetime = lifetime;
        self
    }

    /// Set test on acquire.
    #[must_use]
    pub fn test_on_acquire(mut self, test: bool) -> Self {
        self.config.test_on_acquire = test;
        self
    }

    /// Build the configuration.
    #[must_use]
    pub fn build(self) -> PoolConfig {
        self.config
    }
}

/// Database connection pool.
pub struct DatabasePool {
    pool: AnyPool,
    database_type: DatabaseType,
    config: Arc<MigrationConfig>,
}

impl DatabasePool {
    /// Create a new database pool.
    pub async fn new(config: MigrationConfig) -> Result<Self> {
        let pool_options = AnyPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(1)
            .acquire_timeout(config.connect_timeout);

        let pool = pool_options
            .connect(&config.database_url)
            .await
            .map_err(|e| MigrationError::Connection(e.to_string()))?;

        Ok(Self {
            pool,
            database_type: config.database_type,
            config: Arc::new(config),
        })
    }

    /// Create a pool with custom pool configuration.
    pub async fn with_pool_config(
        migration_config: MigrationConfig,
        pool_config: PoolConfig,
    ) -> Result<Self> {
        let pool_options = AnyPoolOptions::new()
            .max_connections(pool_config.max_connections)
            .min_connections(pool_config.min_connections)
            .acquire_timeout(pool_config.connect_timeout)
            .idle_timeout(Some(pool_config.idle_timeout))
            .max_lifetime(Some(pool_config.max_lifetime))
            .test_before_acquire(pool_config.test_on_acquire);

        let pool = pool_options
            .connect(&migration_config.database_url)
            .await
            .map_err(|e| MigrationError::Connection(e.to_string()))?;

        Ok(Self {
            pool,
            database_type: migration_config.database_type,
            config: Arc::new(migration_config),
        })
    }

    /// Get a reference to the underlying pool.
    #[must_use]
    pub fn inner(&self) -> &AnyPool {
        &self.pool
    }

    /// Get the database type.
    #[must_use]
    pub fn database_type(&self) -> DatabaseType {
        self.database_type
    }

    /// Get the configuration.
    #[must_use]
    pub fn config(&self) -> &MigrationConfig {
        &self.config
    }

    /// Check if the pool is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.pool.is_closed()
    }

    /// Close the pool.
    pub async fn close(&self) {
        self.pool.close().await;
    }

    /// Get pool statistics.
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            size: self.pool.size(),
            idle: self.pool.num_idle(),
            max_connections: self.config.max_connections,
        }
    }

    /// Acquire a connection with timeout.
    pub async fn acquire(&self) -> Result<sqlx::pool::PoolConnection<sqlx::Any>> {
        self.pool
            .acquire()
            .await
            .map_err(|e| MigrationError::Pool(e.to_string()))
    }

    /// Test the connection.
    pub async fn test_connection(&self) -> Result<()> {
        let query = match self.database_type {
            DatabaseType::PostgreSQL => "SELECT 1",
            DatabaseType::SQLite => "SELECT 1",
        };

        sqlx::query(query)
            .execute(&self.pool)
            .await
            .map_err(|e| MigrationError::Connection(e.to_string()))?;

        Ok(())
    }
}

impl std::fmt::Debug for DatabasePool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabasePool")
            .field("database_type", &self.database_type)
            .field("stats", &self.stats())
            .finish()
    }
}

/// Pool statistics.
#[derive(Debug, Clone, Serialize)]
pub struct PoolStats {
    /// Current number of connections.
    pub size: u32,
    /// Number of idle connections.
    pub idle: usize,
    /// Maximum connections allowed.
    pub max_connections: u32,
}

impl PoolStats {
    /// Get the number of active connections.
    #[must_use]
    pub fn active(&self) -> usize {
        self.size as usize - self.idle
    }

    /// Get the utilization percentage.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.max_connections == 0 {
            return 0.0;
        }
        (self.active() as f64 / self.max_connections as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_defaults() {
        let config = PoolConfig::default();
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.min_connections, 1);
        assert!(config.test_on_acquire);
    }

    #[test]
    fn test_pool_config_builder() {
        let config = PoolConfig::builder()
            .max_connections(20)
            .min_connections(5)
            .connect_timeout(Duration::from_secs(60))
            .test_on_acquire(false)
            .build();

        assert_eq!(config.max_connections, 20);
        assert_eq!(config.min_connections, 5);
        assert_eq!(config.connect_timeout, Duration::from_secs(60));
        assert!(!config.test_on_acquire);
    }

    #[test]
    fn test_pool_stats() {
        let stats = PoolStats {
            size: 5,
            idle: 3,
            max_connections: 10,
        };

        assert_eq!(stats.active(), 2);
        assert!((stats.utilization() - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_pool_stats_zero_max() {
        let stats = PoolStats {
            size: 0,
            idle: 0,
            max_connections: 0,
        };

        assert_eq!(stats.utilization(), 0.0);
    }
}
