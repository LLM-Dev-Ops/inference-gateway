//! Agent identity for Phase 7 agents.
//!
//! Provides identity injection into `DecisionEvent` for traceability and audit.

use agentics_contracts::DecisionEvent;
use serde::{Deserialize, Serialize};

/// Agent identity containing all identifying metadata.
///
/// This is injected into all `DecisionEvent` emissions to ensure
/// complete traceability and audit compliance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    /// The source agent identifier.
    pub source_agent: String,
    /// The agent's operational domain.
    pub domain: String,
    /// The phase identifier (must be "phase7").
    pub phase: String,
    /// The layer identifier (must be "layer2").
    pub layer: String,
    /// The agent semantic version.
    pub agent_version: String,
}

impl AgentIdentity {
    /// Create a new agent identity.
    #[must_use]
    pub fn new(
        source_agent: impl Into<String>,
        domain: impl Into<String>,
        phase: impl Into<String>,
        layer: impl Into<String>,
        agent_version: impl Into<String>,
    ) -> Self {
        Self {
            source_agent: source_agent.into(),
            domain: domain.into(),
            phase: phase.into(),
            layer: layer.into(),
            agent_version: agent_version.into(),
        }
    }

    /// Create identity from Phase7Config.
    #[must_use]
    pub fn from_config(config: &super::Phase7Config) -> Self {
        Self {
            source_agent: config.agent_name.clone(),
            domain: config.agent_domain.clone(),
            phase: config.agent_phase.clone(),
            layer: config.agent_layer.clone(),
            agent_version: config.agent_version.clone(),
        }
    }

    /// Inject identity into a `DecisionEvent`.
    ///
    /// This modifies the event's agent_id and agent_version fields
    /// to include the full identity context.
    #[must_use]
    pub fn inject_into_event(&self, mut event: DecisionEvent) -> DecisionEvent {
        // Update agent_id to include full identity context
        event.agent_id = format!(
            "{}:{}:{}:{}",
            self.source_agent, self.domain, self.phase, self.layer
        );
        // Set the version from identity
        event.agent_version = self.agent_version.clone();
        event
    }

    /// Get the fully qualified agent identifier.
    #[must_use]
    pub fn qualified_id(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.source_agent, self.domain, self.phase, self.layer
        )
    }

    /// Convert identity to a structured log-friendly representation.
    #[must_use]
    pub fn to_log_context(&self) -> serde_json::Value {
        serde_json::json!({
            "source_agent": self.source_agent,
            "domain": self.domain,
            "phase": self.phase,
            "layer": self.layer,
            "agent_version": self.agent_version
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentics_contracts::{Confidence, DecisionOutput, DecisionType};

    #[test]
    fn test_identity_creation() {
        let identity = AgentIdentity::new(
            "routing-agent",
            "routing",
            "phase7",
            "layer2",
            "1.0.0",
        );

        assert_eq!(identity.source_agent, "routing-agent");
        assert_eq!(identity.domain, "routing");
        assert_eq!(identity.phase, "phase7");
        assert_eq!(identity.layer, "layer2");
        assert_eq!(identity.agent_version, "1.0.0");
    }

    #[test]
    fn test_qualified_id() {
        let identity = AgentIdentity::new(
            "routing-agent",
            "routing",
            "phase7",
            "layer2",
            "1.0.0",
        );

        assert_eq!(
            identity.qualified_id(),
            "routing-agent:routing:phase7:layer2"
        );
    }

    #[test]
    fn test_inject_into_event() {
        let identity = AgentIdentity::new(
            "routing-agent",
            "routing",
            "phase7",
            "layer2",
            "2.0.0",
        );

        let event = DecisionEvent::new(
            "original-agent",
            "1.0.0",
            DecisionType::RouteSelect,
            "a".repeat(64),
            DecisionOutput::selected(
                "openai",
                "gpt-4",
                false,
                vec!["primary".to_string()],
                vec![],
            ),
            Confidence::full(),
            vec![],
            "exec-ref-123",
        );

        let injected = identity.inject_into_event(event);

        assert_eq!(
            injected.agent_id,
            "routing-agent:routing:phase7:layer2"
        );
        assert_eq!(injected.agent_version, "2.0.0");
    }

    #[test]
    fn test_to_log_context() {
        let identity = AgentIdentity::new(
            "routing-agent",
            "routing",
            "phase7",
            "layer2",
            "1.0.0",
        );

        let context = identity.to_log_context();
        assert_eq!(context["source_agent"], "routing-agent");
        assert_eq!(context["domain"], "routing");
        assert_eq!(context["phase"], "phase7");
        assert_eq!(context["layer"], "layer2");
        assert_eq!(context["agent_version"], "1.0.0");
    }
}
