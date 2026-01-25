//! Decision event schema for agent audit logging.
//!
//! Every agent decision is captured as a `DecisionEvent` that provides:
//! - Full traceability via agent ID, version, and execution reference
//! - Decision transparency via confidence scores and constraint application
//! - Audit compliance via cryptographic input hashing and timestamps
//!
//! ## Phase 7 Intelligence Signals
//!
//! Phase 7 agents emit *signals*, not conclusions. These signals represent
//! intermediate intelligence inputs that feed into downstream decision-making:
//!
//! - `HypothesisSignal`: A proposed hypothesis for evaluation
//! - `SimulationOutcomeSignal`: Results from a simulation run
//! - `ScenarioComparisonSignal`: Comparative analysis between scenarios
//! - `ConfidenceDeltaSignal`: Change in confidence measurement
//! - `UncertaintySignal`: Quantified uncertainty in analysis
//! - `ResearchInsightSignal`: Insight derived from research
//! - `AbortSignal`: Performance budget exceeded, abort operation

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// A decision event emitted by an agent for every routing decision.
///
/// This is the core audit record that captures the complete context
/// of a routing decision, including inputs, outputs, and constraints.
///
/// For Phase 7 agents, this also captures intelligence signals with
/// full provenance via `evidence_refs` and agent identity metadata.
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

    /// Evidence references (run IDs, telemetry IDs, dataset refs).
    ///
    /// Phase 7 signals must include evidence references for provenance.
    /// These can include:
    /// - Run IDs from simulation or benchmark runs
    /// - Telemetry trace IDs for observability correlation
    /// - Dataset references for reproducibility
    /// - External system identifiers
    #[serde(default)]
    pub evidence_refs: Vec<String>,

    /// Source agent name (Phase 7 agent identity).
    ///
    /// The human-readable name of the agent that emitted this event.
    #[serde(default)]
    pub source_agent: String,

    /// Agent domain (Phase 7 agent identity).
    ///
    /// The functional domain of the agent (e.g., "routing", "research", "simulation").
    #[serde(default)]
    pub domain: String,

    /// Phase identifier (Phase 7 agent identity).
    ///
    /// The phase of the agent in the agentic pipeline (e.g., "phase7").
    #[serde(default)]
    pub phase: String,

    /// Layer identifier (Phase 7 agent identity).
    ///
    /// The layer within the phase (e.g., "layer1", "layer2").
    #[serde(default)]
    pub layer: String,
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
            evidence_refs: Vec::new(),
            source_agent: String::new(),
            domain: String::new(),
            phase: String::new(),
            layer: String::new(),
        }
    }

    /// Creates a new Phase 7 signal event with full agent identity metadata.
    #[must_use]
    pub fn new_phase7_signal(
        agent_id: impl Into<String>,
        agent_version: impl Into<String>,
        decision_type: DecisionType,
        inputs_hash: impl Into<String>,
        outputs: DecisionOutput,
        confidence: Confidence,
        execution_ref: impl Into<String>,
        evidence_refs: Vec<String>,
        source_agent: impl Into<String>,
        domain: impl Into<String>,
        phase: impl Into<String>,
        layer: impl Into<String>,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            agent_version: agent_version.into(),
            decision_type,
            inputs_hash: inputs_hash.into(),
            outputs,
            confidence,
            constraints_applied: Vec::new(),
            execution_ref: execution_ref.into(),
            timestamp: Utc::now(),
            evidence_refs,
            source_agent: source_agent.into(),
            domain: domain.into(),
            phase: phase.into(),
            layer: layer.into(),
        }
    }

    /// Creates an abort signal when performance budget is exceeded.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - The agent emitting the abort signal
    /// * `agent_version` - Semantic version of the agent
    /// * `reason` - Human-readable reason for the abort
    /// * `execution_ref` - Reference to the execution context
    /// * `evidence_refs` - Evidence supporting the abort decision (e.g., timing data)
    #[must_use]
    pub fn new_abort_signal(
        agent_id: impl Into<String>,
        agent_version: impl Into<String>,
        reason: impl Into<String>,
        execution_ref: impl Into<String>,
        evidence_refs: Vec<String>,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            agent_version: agent_version.into(),
            decision_type: DecisionType::AbortSignal,
            inputs_hash: "0".repeat(64), // No input hash for abort signals
            outputs: DecisionOutput::rejected(reason),
            confidence: Confidence::zero(),
            constraints_applied: Vec::new(),
            execution_ref: execution_ref.into(),
            timestamp: Utc::now(),
            evidence_refs,
            source_agent: String::new(),
            domain: String::new(),
            phase: "phase7".to_string(),
            layer: String::new(),
        }
    }

    /// Returns true if this event is a Phase 7 intelligence signal.
    ///
    /// Phase 7 signals are intermediate intelligence inputs, not final decisions.
    /// They include hypotheses, simulation outcomes, comparisons, confidence
    /// deltas, uncertainty measurements, research insights, and abort signals.
    #[must_use]
    pub fn is_phase7_signal(&self) -> bool {
        matches!(
            self.decision_type,
            DecisionType::HypothesisSignal
                | DecisionType::SimulationOutcomeSignal
                | DecisionType::ScenarioComparisonSignal
                | DecisionType::ConfidenceDeltaSignal
                | DecisionType::UncertaintySignal
                | DecisionType::ResearchInsightSignal
                | DecisionType::AbortSignal
        )
    }

    /// Sets the Phase 7 agent identity metadata.
    ///
    /// This is a builder-style method for setting agent identity after creation.
    #[must_use]
    pub fn with_agent_identity(
        mut self,
        source_agent: impl Into<String>,
        domain: impl Into<String>,
        phase: impl Into<String>,
        layer: impl Into<String>,
    ) -> Self {
        self.source_agent = source_agent.into();
        self.domain = domain.into();
        self.phase = phase.into();
        self.layer = layer.into();
        self
    }

    /// Adds evidence references to the event.
    ///
    /// This is a builder-style method for adding evidence after creation.
    #[must_use]
    pub fn with_evidence(mut self, evidence_refs: Vec<String>) -> Self {
        self.evidence_refs = evidence_refs;
        self
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

    // =========================================================================
    // Phase 7 Intelligence Signal Types
    //
    // These represent SIGNALS (inputs), not CONCLUSIONS (outputs).
    // Phase 7 agents provide intermediate intelligence that feeds into
    // downstream decision-making processes.
    // =========================================================================

    /// A proposed hypothesis for evaluation.
    ///
    /// Agents emit this signal when they have formulated a hypothesis
    /// that needs to be tested or validated. The hypothesis itself is
    /// captured in the `outputs` field.
    HypothesisSignal,

    /// Results from a simulation run.
    ///
    /// Emitted after completing a simulation, with outcomes captured
    /// in the `outputs` field and simulation run IDs in `evidence_refs`.
    SimulationOutcomeSignal,

    /// Comparative analysis between scenarios.
    ///
    /// Emitted when comparing multiple scenarios or approaches. The
    /// comparison results are in `outputs`, with scenario identifiers
    /// in `evidence_refs`.
    ScenarioComparisonSignal,

    /// Change in confidence measurement.
    ///
    /// Signals a significant change in confidence level for a prior
    /// belief or hypothesis. The delta is captured in `confidence`,
    /// with the reason in `outputs`.
    ConfidenceDeltaSignal,

    /// Quantified uncertainty in analysis.
    ///
    /// Emitted when uncertainty has been measured and quantified.
    /// Uncertainty bounds are captured in `outputs`, with methodology
    /// references in `evidence_refs`.
    UncertaintySignal,

    /// Insight derived from research.
    ///
    /// Signals a research finding or insight. The insight content is
    /// in `outputs`, with source references in `evidence_refs`.
    ResearchInsightSignal,

    /// Performance budget exceeded, abort operation.
    ///
    /// Emitted when an operation must be aborted due to exceeding
    /// time, cost, or resource budgets. The reason is in `outputs`,
    /// with timing/resource data in `evidence_refs`.
    AbortSignal,
}

impl DecisionType {
    /// Returns true if this decision type is a Phase 7 signal.
    #[must_use]
    pub fn is_phase7_signal(&self) -> bool {
        matches!(
            self,
            DecisionType::HypothesisSignal
                | DecisionType::SimulationOutcomeSignal
                | DecisionType::ScenarioComparisonSignal
                | DecisionType::ConfidenceDeltaSignal
                | DecisionType::UncertaintySignal
                | DecisionType::ResearchInsightSignal
                | DecisionType::AbortSignal
        )
    }

    /// Returns the signal name as a string (for logging/display).
    #[must_use]
    pub fn signal_name(&self) -> &'static str {
        match self {
            DecisionType::RouteSelect => "route_select",
            DecisionType::RouteFallback => "route_fallback",
            DecisionType::RouteReject => "route_reject",
            DecisionType::HypothesisSignal => "hypothesis_signal",
            DecisionType::SimulationOutcomeSignal => "simulation_outcome_signal",
            DecisionType::ScenarioComparisonSignal => "scenario_comparison_signal",
            DecisionType::ConfidenceDeltaSignal => "confidence_delta_signal",
            DecisionType::UncertaintySignal => "uncertainty_signal",
            DecisionType::ResearchInsightSignal => "research_insight_signal",
            DecisionType::AbortSignal => "abort_signal",
        }
    }
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

    /// Creates a Phase 7 signal output with custom routing path.
    ///
    /// This is used for signals where the routing path represents
    /// the logical path through the analysis, not a provider path.
    #[must_use]
    pub fn signal(routing_path: Vec<String>) -> Self {
        Self {
            selected_provider: None,
            selected_model: None,
            model_transformed: false,
            routing_path,
            fallback_providers: Vec::new(),
            rejection_reason: None,
        }
    }

    /// Creates a Phase 7 signal output with a message/content.
    ///
    /// The message is stored in the rejection_reason field for signals
    /// that need to carry textual content (hypotheses, insights, etc.).
    #[must_use]
    pub fn signal_with_content(content: impl Into<String>) -> Self {
        Self {
            selected_provider: None,
            selected_model: None,
            model_transformed: false,
            routing_path: Vec::new(),
            fallback_providers: Vec::new(),
            rejection_reason: Some(content.into()),
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

    /// Creates a confidence score for a Phase 7 signal.
    ///
    /// For signals, we use the overall score as the primary indicator
    /// and set rule_match and availability to the same value.
    #[must_use]
    pub fn signal(confidence: f64) -> Self {
        Self {
            rule_match: confidence,
            availability: confidence,
            overall: confidence,
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

    /// A Phase 7 performance budget constraint.
    PerformanceBudget {
        /// Budget type (e.g., "time_ms", "cost_usd", "tokens").
        budget_type: String,
        /// Budget limit.
        limit: f64,
        /// Current usage.
        current: f64,
        /// Whether the budget was exceeded.
        exceeded: bool,
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
        assert!(event.evidence_refs.is_empty());
        assert!(!event.is_phase7_signal());
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

    #[test]
    fn test_phase7_signal_types() {
        assert!(DecisionType::HypothesisSignal.is_phase7_signal());
        assert!(DecisionType::SimulationOutcomeSignal.is_phase7_signal());
        assert!(DecisionType::ScenarioComparisonSignal.is_phase7_signal());
        assert!(DecisionType::ConfidenceDeltaSignal.is_phase7_signal());
        assert!(DecisionType::UncertaintySignal.is_phase7_signal());
        assert!(DecisionType::ResearchInsightSignal.is_phase7_signal());
        assert!(DecisionType::AbortSignal.is_phase7_signal());

        assert!(!DecisionType::RouteSelect.is_phase7_signal());
        assert!(!DecisionType::RouteFallback.is_phase7_signal());
        assert!(!DecisionType::RouteReject.is_phase7_signal());
    }

    #[test]
    fn test_abort_signal_creation() {
        let event = DecisionEvent::new_abort_signal(
            "research-agent",
            "2.0.0",
            "Time budget exceeded: 5000ms > 3000ms limit",
            "exec-456",
            vec!["timing-run-001".to_string(), "telemetry-xyz".to_string()],
        );

        assert_eq!(event.agent_id, "research-agent");
        assert_eq!(event.decision_type, DecisionType::AbortSignal);
        assert!(event.is_phase7_signal());
        assert_eq!(event.phase, "phase7");
        assert_eq!(event.evidence_refs.len(), 2);
        assert!(event
            .outputs
            .rejection_reason
            .as_ref()
            .unwrap()
            .contains("Time budget exceeded"));
    }

    #[test]
    fn test_phase7_signal_event() {
        let event = DecisionEvent::new_phase7_signal(
            "hypothesis-agent",
            "1.0.0",
            DecisionType::HypothesisSignal,
            "b".repeat(64),
            DecisionOutput::signal_with_content("Provider A outperforms B under load"),
            Confidence::signal(0.85),
            "exec-789",
            vec!["benchmark-run-001".to_string()],
            "hypothesis-generator",
            "research",
            "phase7",
            "layer2",
        );

        assert!(event.is_phase7_signal());
        assert_eq!(event.source_agent, "hypothesis-generator");
        assert_eq!(event.domain, "research");
        assert_eq!(event.phase, "phase7");
        assert_eq!(event.layer, "layer2");
        assert_eq!(event.evidence_refs.len(), 1);
    }

    #[test]
    fn test_decision_event_with_builders() {
        let event = DecisionEvent::new(
            "agent-1",
            "1.0.0",
            DecisionType::ResearchInsightSignal,
            "c".repeat(64),
            DecisionOutput::signal_with_content("Found optimization opportunity"),
            Confidence::signal(0.9),
            vec![],
            "exec-100",
        )
        .with_agent_identity("research-bot", "analysis", "phase7", "layer1")
        .with_evidence(vec!["dataset-ref-1".to_string()]);

        assert!(event.is_phase7_signal());
        assert_eq!(event.source_agent, "research-bot");
        assert_eq!(event.domain, "analysis");
        assert_eq!(event.evidence_refs.len(), 1);
    }

    #[test]
    fn test_signal_name() {
        assert_eq!(DecisionType::RouteSelect.signal_name(), "route_select");
        assert_eq!(
            DecisionType::HypothesisSignal.signal_name(),
            "hypothesis_signal"
        );
        assert_eq!(DecisionType::AbortSignal.signal_name(), "abort_signal");
    }

    #[test]
    fn test_phase7_serialization() {
        let event = DecisionEvent::new_phase7_signal(
            "sim-agent",
            "1.0.0",
            DecisionType::SimulationOutcomeSignal,
            "d".repeat(64),
            DecisionOutput::signal(vec!["sim-path-1".to_string()]),
            Confidence::signal(0.75),
            "exec-200",
            vec!["sim-run-001".to_string()],
            "simulator",
            "simulation",
            "phase7",
            "layer2",
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"decision_type\":\"simulation_outcome_signal\""));
        assert!(json.contains("\"evidence_refs\":[\"sim-run-001\"]"));
        assert!(json.contains("\"source_agent\":\"simulator\""));
        assert!(json.contains("\"phase\":\"phase7\""));
    }

    #[test]
    fn test_performance_budget_constraint() {
        let constraint = Constraint::PerformanceBudget {
            budget_type: "time_ms".to_string(),
            limit: 3000.0,
            current: 3500.0,
            exceeded: true,
        };

        let json = serde_json::to_string(&constraint).unwrap();
        assert!(json.contains("\"type\":\"performance_budget\""));
        assert!(json.contains("\"exceeded\":true"));
    }

    #[test]
    fn test_confidence_signal() {
        let confidence = Confidence::signal(0.85);
        assert_eq!(confidence.rule_match, 0.85);
        assert_eq!(confidence.availability, 0.85);
        assert_eq!(confidence.overall, 0.85);
    }
}
