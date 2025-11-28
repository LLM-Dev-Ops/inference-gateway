//! # Gateway Migrations
//!
//! Database migration system for the LLM Inference Gateway.
//!
//! This crate provides:
//! - Version-controlled database migrations
//! - Support for PostgreSQL and SQLite
//! - Migration checksum verification
//! - Rollback capabilities
//! - Migration status tracking
//!
//! ## Example
//!
//! ```rust,no_run
//! use gateway_migrations::{Migrator, MigrationConfig, DatabaseType};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = MigrationConfig::builder()
//!         .database_url("postgres://localhost/gateway")
//!         .database_type(DatabaseType::PostgreSQL)
//!         .build()?;
//!
//!     let migrator = Migrator::new(config).await?;
//!     migrator.run_pending().await?;
//!
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
pub mod error;
pub mod migration;
pub mod migrator;
pub mod pool;
pub mod schema;

pub use config::{DatabaseType, MigrationConfig, MigrationConfigBuilder};
pub use error::{MigrationError, Result};
pub use migration::{Migration, MigrationRecord, MigrationStatus};
pub use migrator::Migrator;
pub use pool::{DatabasePool, PoolConfig};

/// Re-export sqlx types for convenience
pub use sqlx;
