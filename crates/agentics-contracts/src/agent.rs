//! Agent metadata and trait definitions.
//!
//! Defines the core traits and metadata structures that all agents must implement,
//! ensuring consistent behavior and auditability across the gateway.

use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::decision_event::DecisionEvent;
use crate::error::AgentError;

/// Metadata describing an agent instance.
///
/// This metadata is used for:
/// - Agent registration and discovery
/// - Decision event attribution
/// - Capability-based routing
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AgentMetadata {
    /// Unique identifier for the agent instance.
    #[validate(length(min = 1, max = 128))]
    pub id: String,

    /// Semantic version of the agent (e.g., "1.0.0").
    #[validate(length(min = 5, max = 32))]
    pub version: String,

    /// The type of agent.
    pub agent_type: AgentType,

    /// List of capabilities provided by this agent.
    pub capabilities: Vec<String>,

    /// Human-readable description of the agent.
    pub description: Option<String>,

    /// Configuration version (for tracking config changes).
    pub config_version: Option<String>,
}

impl AgentMetadata {
    /// Creates new agent metadata with required fields.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        agent_type: AgentType,
    ) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            agent_type,
            capabilities: Vec::new(),
            description: None,
            config_version: None,
        }
    }

    /// Adds capabilities to the agent metadata.
    #[must_use]
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Adds a description to the agent metadata.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the config version.
    #[must_use]
    pub fn with_config_version(mut self, version: impl Into<String>) -> Self {
        self.config_version = Some(version.into());
        self
    }

    /// Checks if the agent has a specific capability.
    #[must_use]
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }
}

/// The type of agent in the gateway system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    /// Inference routing agent - selects providers and models.
    InferenceRouting,

    /// Policy enforcement agent - applies access control and quotas.
    PolicyEnforcement,

    /// Cost estimation agent - estimates request costs.
    CostEstimation,

    /// Health monitoring agent - tracks provider health.
    HealthMonitoring,

    /// Model resolution agent - resolves model aliases.
    ModelResolution,

    /// Load balancing agent - distributes load across providers.
    LoadBalancing,

    /// Rate limiting agent - enforces rate limits.
    RateLimiting,

    /// Content filtering agent - filters content for safety.
    ContentFiltering,

    /// Audit logging agent - logs decisions for compliance.
    AuditLogging,

    /// Custom agent type (for extensions).
    Custom,
}

impl AgentType {
    /// Returns the string representation of the agent type.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InferenceRouting => "inference_routing",
            Self::PolicyEnforcement => "policy_enforcement",
            Self::CostEstimation => "cost_estimation",
            Self::HealthMonitoring => "health_monitoring",
            Self::ModelResolution => "model_resolution",
            Self::LoadBalancing => "load_balancing",
            Self::RateLimiting => "rate_limiting",
            Self::ContentFiltering => "content_filtering",
            Self::AuditLogging => "audit_logging",
            Self::Custom => "custom",
        }
    }
}

/// Trait that all agents must implement.
///
/// This trait defines the core interface for agent execution,
/// ensuring all agents:
/// - Expose their metadata for discovery
/// - Accept serialized input and return structured decision events
/// - Handle errors consistently
pub trait Agent: Send + Sync {
    /// Returns the agent's metadata.
    fn metadata(&self) -> &AgentMetadata;

    /// Executes the agent with the given input.
    ///
    /// # Arguments
    ///
    /// * `input` - Serialized input data (JSON bytes)
    ///
    /// # Returns
    ///
    /// A `DecisionEvent` capturing the routing decision, or an error.
    ///
    /// # Errors
    ///
    /// Returns `AgentError` if:
    /// - Input validation fails
    /// - No healthy providers are available
    /// - The requested model is not supported
    /// - Internal agent error occurs
    fn execute(&self, input: &[u8]) -> Result<DecisionEvent, AgentError>;

    /// Returns the agent's unique identifier.
    fn id(&self) -> &str {
        &self.metadata().id
    }

    /// Returns the agent's version.
    fn version(&self) -> &str {
        &self.metadata().version
    }

    /// Returns the agent's type.
    fn agent_type(&self) -> AgentType {
        self.metadata().agent_type
    }

    /// Checks if the agent has a specific capability.
    fn has_capability(&self, capability: &str) -> bool {
        self.metadata().has_capability(capability)
    }
}

/// Extension trait for agents that support async execution.
#[allow(async_fn_in_trait)]
pub trait AsyncAgent: Agent {
    /// Executes the agent asynchronously.
    ///
    /// # Arguments
    ///
    /// * `input` - Serialized input data (JSON bytes)
    ///
    /// # Returns
    ///
    /// A `DecisionEvent` capturing the routing decision, or an error.
    async fn execute_async(&self, input: &[u8]) -> Result<DecisionEvent, AgentError>;
}

/// Configuration for agent execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentConfig {
    /// Timeout for agent execution in milliseconds.
    pub timeout_ms: Option<u64>,

    /// Whether to enable detailed tracing.
    pub enable_tracing: bool,

    /// Maximum retries on transient failures.
    pub max_retries: Option<u32>,

    /// Custom configuration parameters.
    #[serde(default)]
    pub custom: std::collections::HashMap<String, serde_json::Value>,
}

impl AgentConfig {
    /// Creates a new default agent configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the execution timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Enables tracing.
    #[must_use]
    pub fn with_tracing(mut self) -> Self {
        self.enable_tracing = true;
        self
    }

    /// Sets the maximum retries.
    #[must_use]
    pub fn with_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = Some(max_retries);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_metadata_builder() {
        let metadata = AgentMetadata::new("routing-agent", "1.0.0", AgentType::InferenceRouting)
            .with_capabilities(vec!["model_resolution".to_string(), "fallback".to_string()])
            .with_description("Primary routing agent");

        assert_eq!(metadata.id, "routing-agent");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(metadata.agent_type, AgentType::InferenceRouting);
        assert!(metadata.has_capability("model_resolution"));
        assert!(!metadata.has_capability("cost_estimation"));
    }

    #[test]
    fn test_agent_type_as_str() {
        assert_eq!(AgentType::InferenceRouting.as_str(), "inference_routing");
        assert_eq!(AgentType::PolicyEnforcement.as_str(), "policy_enforcement");
    }

    #[test]
    fn test_agent_config_builder() {
        let config = AgentConfig::new()
            .with_timeout(5000)
            .with_tracing()
            .with_retries(3);

        assert_eq!(config.timeout_ms, Some(5000));
        assert!(config.enable_tracing);
        assert_eq!(config.max_retries, Some(3));
    }

    #[test]
    fn test_serialization() {
        let metadata = AgentMetadata::new("test", "1.0.0", AgentType::InferenceRouting);
        let json = serde_json::to_string(&metadata).unwrap();

        assert!(json.contains("\"agent_type\":\"inference_routing\""));
    }
}
