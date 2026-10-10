//! # Gateway Configuration
//!
//! Configuration management for the LLM Inference Gateway, including:
//! - Configuration schema and validation
//! - Loading from YAML/TOML files
//! - Hot reload support via file watching
//! - Environment variable substitution

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod hot_reload;
pub mod loader;
pub mod schema;

// Re-export main types
pub use hot_reload::ConfigWatcher;
pub use loader::{load_config, ConfigError, ConfigLoader, ConfigSource};
pub use schema::{
    AuthConfig, CircuitBreakerConfig, GatewayConfig, ObservabilityConfig, ProviderConfig,
    RateLimitConfig, RateLimitKeyBy, ResilienceConfig, RetryConfig, RoutingConfig, SecurityConfig,
    ServerConfig, TlsConfig,
};
