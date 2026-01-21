//! Adapter implementations for LLM-Dev-Ops ecosystem integrations.
//!
//! Each adapter provides thin runtime wrappers for consuming from
//! upstream LLM-Dev-Ops services without modifying existing gateway APIs.

pub mod connector_hub;
pub mod cost_ops;
pub mod auto_optimizer;
pub mod observatory;
pub mod policy_engine;
pub mod router;
pub mod ruvector;
pub mod sentinel;
pub mod shield;

mod manager;

pub use manager::IntegrationManager;

// Re-export adapter types
pub use connector_hub::ConnectorHubAdapter;
pub use cost_ops::CostOpsAdapter;
pub use auto_optimizer::AutoOptimizerAdapter;
pub use observatory::ObservatoryAdapter;
pub use policy_engine::PolicyEngineAdapter;
pub use router::RouterAdapter;
pub use ruvector::{DecisionEvent, EventQuery, RuVectorClient, RuVectorPersistence};
pub use sentinel::SentinelAdapter;
pub use shield::ShieldAdapter;
