//! # Gateway Integrations
//!
//! Thin runtime adapters for integrating with the LLM-Dev-Ops ecosystem.
//!
//! This crate provides additive, consume-from integrations for:
//! - **LLM-Connector-Hub**: Route requests to different model providers
//! - **LLM-Shield**: Apply safety filters and output validation
//! - **LLM-Sentinel**: Consume anomaly alerts and trigger fallback behavior
//! - **LLM-CostOps**: Consume cost projections for cost-efficient routing
//! - **LLM-Observatory**: Emit and consume telemetry traces and metrics
//! - **Router**: Consume routing rules and decision graphs
//! - **LLM-Auto-Optimizer**: Consume optimization hints and recommendations
//! - **LLM-Policy-Engine**: Consume policy enforcement decisions
//! - **RuVector-Service**: Persist DecisionEvents (never direct DB access)
//!
//! All adapters are designed as thin wrappers that:
//! - Do not modify existing gateway APIs
//! - Do not introduce circular imports
//! - Consume from upstream services without tight coupling

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod adapters;
pub mod config;
pub mod error;
pub mod traits;

// Re-export commonly used types
pub use adapters::IntegrationManager;
pub use adapters::{DecisionEvent, EventQuery, RuVectorClient, RuVectorPersistence};
pub use config::{IntegrationsConfig, RuVectorConfig};
pub use error::{IntegrationError, IntegrationResult};
pub use traits::{
    CostConsumer, ObservabilityEmitter, OptimizationConsumer, PolicyConsumer, ProviderRouter,
    SafetyFilter, SentinelConsumer,
};
