//! Migration error types.

/// Result type for migration operations.
pub type Result<T> = std::result::Result<T, MigrationError>;

/// Migration error type.
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    /// Database connection error.
    #[error("Database connection error: {0}")]
    Connection(String),

    /// SQL execution error.
    #[error("SQL execution error: {0}")]
    Execution(String),

    /// Migration not found.
    #[error("Migration not found: {version}")]
    NotFound {
        /// Migration version that was not found.
        version: i64,
    },

    /// Migration checksum mismatch.
    #[error("Checksum mismatch for migration {version}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// Migration version.
        version: i64,
        /// Expected checksum.
        expected: String,
        /// Actual checksum.
        actual: String,
    },

    /// Migration already applied.
    #[error("Migration {version} has already been applied")]
    AlreadyApplied {
        /// Migration version.
        version: i64,
    },

    /// Migration failed.
    #[error("Migration {version} failed: {reason}")]
    Failed {
        /// Migration version.
        version: i64,
        /// Failure reason.
        reason: String,
    },

    /// Rollback not supported.
    #[error("Migration {version} does not support rollback")]
    RollbackNotSupported {
        /// Migration version.
        version: i64,
    },

    /// Invalid migration order.
    #[error("Invalid migration order: {0}")]
    InvalidOrder(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Pool error.
    #[error("Connection pool error: {0}")]
    Pool(String),

    /// Timeout error.
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Lock acquisition failed.
    #[error("Failed to acquire migration lock: {0}")]
    LockFailed(String),

    /// Database not supported.
    #[error("Database type not supported: {0}")]
    UnsupportedDatabase(String),
}

impl MigrationError {
    /// Create a connection error.
    pub fn connection(msg: impl Into<String>) -> Self {
        Self::Connection(msg.into())
    }

    /// Create an execution error.
    pub fn execution(msg: impl Into<String>) -> Self {
        Self::Execution(msg.into())
    }

    /// Create a config error.
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Check if the error is retryable.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Connection(_) | Self::Timeout(_) | Self::LockFailed(_)
        )
    }
}

impl From<sqlx::Error> for MigrationError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::Configuration(e) => Self::Config(e.to_string()),
            sqlx::Error::Database(e) => Self::Execution(e.to_string()),
            sqlx::Error::Io(e) => Self::Io(e),
            sqlx::Error::PoolTimedOut => Self::Timeout("Connection pool timed out".to_string()),
            sqlx::Error::PoolClosed => Self::Pool("Connection pool is closed".to_string()),
            _ => Self::Execution(err.to_string()),
        }
    }
}

impl From<serde_json::Error> for MigrationError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = MigrationError::NotFound { version: 1 };
        assert!(err.to_string().contains("Migration not found: 1"));

        let err = MigrationError::ChecksumMismatch {
            version: 2,
            expected: "abc".to_string(),
            actual: "def".to_string(),
        };
        assert!(err.to_string().contains("Checksum mismatch"));
    }

    #[test]
    fn test_error_retryable() {
        assert!(MigrationError::Connection("test".to_string()).is_retryable());
        assert!(MigrationError::Timeout("test".to_string()).is_retryable());
        assert!(MigrationError::LockFailed("test".to_string()).is_retryable());
        assert!(!MigrationError::NotFound { version: 1 }.is_retryable());
    }

    #[test]
    fn test_error_constructors() {
        let err = MigrationError::connection("connection failed");
        assert!(matches!(err, MigrationError::Connection(_)));

        let err = MigrationError::execution("query failed");
        assert!(matches!(err, MigrationError::Execution(_)));

        let err = MigrationError::config("invalid config");
        assert!(matches!(err, MigrationError::Config(_)));
    }
}
