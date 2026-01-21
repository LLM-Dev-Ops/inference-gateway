//! Decision event schema for agent audit logging.
//!
//! Every agent decision is captured as a `DecisionEvent` that provides:
//! - Full traceability via agent ID, version, and execution reference
//! - Decision transparency via confidence scores and constraint application
//! - Audit compliance via cryptographic input hashing and timestamps

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// A decision event emitted by an agent for every routing decision.
///
/// This is the core audit record that captures the complete context
/// of a routing decision, including inputs, outputs, and constraints.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DecisionEvent {
    /// Unique identifier for the agent instance.
    #[validate(length(min = 1, max = 128))]
    pub agent_id: String,

    /// Semantic version of the agent (e.g., "1.0.0").
    #[validate(length(min = 5, max = 32))]
    pub agent_version: String,

    /// Type of decision made by the agent.
    pub decision_type: DecisionType,

    /// SHA-256 hash of the input data for integrity verification.
    #[validate(length(equal = 64))]
    pub inputs_hash: String,

    /// The output of the decision (selected provider, model, etc.).
    pub outputs: DecisionOutput,

    /// Confidence scores for the decision.
    #[validate(nested)]
    pub confidence: Confidence,

    /// List of constraints that were evaluated and applied.
    pub constraints_applied: Vec<Constraint>,

    /// Reference to the execution context (e.g., request ID, trace ID).
    #[validate(length(min = 1, max = 256))]
    pub execution_ref: String,

    /// Timestamp when the decision was made.
    pub timestamp: DateTime<Utc>,
}

impl DecisionEvent {
    /// Creates a new decision event with the current timestamp.
    #[must_use]
    pub fn new(
        agent_id: impl Into<String>,
        agent_version: impl Into<String>,
        decision_type: DecisionType,
        inputs_hash: impl Into<String>,
        outputs: DecisionOutput,
        confidence: Confidence,
        constraints_applied: Vec<Constraint>,
        execution_ref: impl Into<String>,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            agent_version: agent_version.into(),
            decision_type,
            inputs_hash: inputs_hash.into(),
            outputs,
            confidence,
            constraints_applied,
            execution_ref: execution_ref.into(),
            timestamp: Utc::now(),
        }
    }
}

/// The type of routing decision made by the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionType {
    /// A provider was successfully selected from the primary route.
    RouteSelect,

    /// Primary route failed; a fallback provider was selected.
    RouteFallback,

    /// The request was rejected (no suitable providers available).
    RouteReject,
}

/// The output of a routing decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionOutput {
    /// The selected provider identifier (if any).
    pub selected_provider: Option<String>,

    /// The selected model identifier (may differ from requested model).
    pub selected_model: Option<String>,

    /// Whether the model was transformed (mapped from alias).
    pub model_transformed: bool,

    /// The routing path taken to reach the decision.
    pub routing_path: Vec<String>,

    /// Fallback providers that were considered.
    pub fallback_providers: Vec<String>,

    /// Rejection reason (if decision type is RouteReject).
    pub rejection_reason: Option<String>,
}

impl DecisionOutput {
    /// Creates a successful route selection output.
    #[must_use]
    pub fn selected(
        provider: impl Into<String>,
        model: impl Into<String>,
        model_transformed: bool,
        routing_path: Vec<String>,
        fallback_providers: Vec<String>,
    ) -> Self {
        Self {
            selected_provider: Some(provider.into()),
            selected_model: Some(model.into()),
            model_transformed,
            routing_path,
            fallback_providers,
            rejection_reason: None,
        }
    }

    /// Creates a rejection output.
    #[must_use]
    pub fn rejected(reason: impl Into<String>) -> Self {
        Self {
            selected_provider: None,
            selected_model: None,
            model_transformed: false,
            routing_path: Vec::new(),
            fallback_providers: Vec::new(),
            rejection_reason: Some(reason.into()),
        }
    }
}

/// Confidence scores for a routing decision.
///
/// All scores are normalized to the range [0.0, 1.0].
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Confidence {
    /// Confidence in the rule match (how well the request matches routing rules).
    #[validate(range(min = 0.0, max = 1.0))]
    pub rule_match: f64,

    /// Confidence in provider availability (health score of selected provider).
    #[validate(range(min = 0.0, max = 1.0))]
    pub availability: f64,

    /// Combined overall confidence score.
    #[validate(range(min = 0.0, max = 1.0))]
    pub overall: f64,
}

impl Confidence {
    /// Creates a new confidence score with all values set.
    #[must_use]
    pub fn new(rule_match: f64, availability: f64, overall: f64) -> Self {
        Self {
            rule_match,
            availability,
            overall,
        }
    }

    /// Creates a confidence score from rule match and availability.
    ///
    /// The overall score is computed as the geometric mean of the two scores.
    #[must_use]
    pub fn from_components(rule_match: f64, availability: f64) -> Self {
        let overall = (rule_match * availability).sqrt();
        Self {
            rule_match,
            availability,
            overall,
        }
    }

    /// Creates a full confidence score (all values set to 1.0).
    #[must_use]
    pub fn full() -> Self {
        Self {
            rule_match: 1.0,
            availability: 1.0,
            overall: 1.0,
        }
    }

    /// Creates a zero confidence score (all values set to 0.0).
    #[must_use]
    pub fn zero() -> Self {
        Self {
            rule_match: 0.0,
            availability: 0.0,
            overall: 0.0,
        }
    }
}

/// A constraint that was evaluated during routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Constraint {
    /// A policy constraint (e.g., access control, rate limiting).
    Policy {
        /// Unique identifier for the policy.
        policy_id: String,
        /// The effect of the policy on the routing decision.
        effect: ConstraintEffect,
    },

    /// A provider-specific constraint.
    Provider {
        /// The provider identifier.
        provider_id: String,
        /// The reason for the constraint (e.g., "rate limited", "maintenance").
        reason: String,
    },

    /// A capability requirement constraint.
    Capability {
        /// The required capability (e.g., "vision", "function_calling").
        capability: String,
        /// Whether the capability was satisfied.
        satisfied: bool,
    },

    /// An availability constraint based on provider health.
    Availability {
        /// The provider identifier.
        provider_id: String,
        /// The health score of the provider (0.0 to 1.0).
        health_score: f64,
        /// Minimum required health score.
        threshold: f64,
    },

    /// A model support constraint.
    ModelSupport {
        /// The requested model identifier.
        model_id: String,
        /// The provider identifier.
        provider_id: String,
        /// Whether the model is supported.
        supported: bool,
    },

    /// A tenant-specific constraint.
    Tenant {
        /// The tenant identifier.
        tenant_id: String,
        /// The constraint type (e.g., "quota", "allowlist").
        constraint_type: String,
        /// Whether the constraint was satisfied.
        satisfied: bool,
    },
}

/// The effect of a constraint on routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintEffect {
    /// The constraint allows the route.
    Allow,
    /// The constraint denies the route.
    Deny,
    /// The constraint modifies the route (e.g., fallback, transform).
    Modify,
    /// The constraint was skipped (not applicable).
    Skip,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_event_creation() {
        let event = DecisionEvent::new(
            "routing-agent",
            "1.0.0",
            DecisionType::RouteSelect,
            "a".repeat(64),
            DecisionOutput::selected(
                "openai",
                "gpt-4",
                false,
                vec!["primary".to_string()],
                vec!["anthropic".to_string()],
            ),
            Confidence::full(),
            vec![],
            "req-123",
        );

        assert_eq!(event.agent_id, "routing-agent");
        assert_eq!(event.decision_type, DecisionType::RouteSelect);
        assert!(event.outputs.selected_provider.is_some());
    }

    #[test]
    fn test_confidence_from_components() {
        let confidence = Confidence::from_components(0.9, 0.8);
        assert!((confidence.overall - 0.848528).abs() < 0.001);
    }

    #[test]
    fn test_decision_output_rejected() {
        let output = DecisionOutput::rejected("No healthy providers");
        assert!(output.selected_provider.is_none());
        assert_eq!(
            output.rejection_reason,
            Some("No healthy providers".to_string())
        );
    }

    #[test]
    fn test_constraint_serialization() {
        let constraint = Constraint::Policy {
            policy_id: "pol-123".to_string(),
            effect: ConstraintEffect::Allow,
        };

        let json = serde_json::to_string(&constraint).unwrap();
        assert!(json.contains("\"type\":\"policy\""));
        assert!(json.contains("\"effect\":\"allow\""));
    }
}
