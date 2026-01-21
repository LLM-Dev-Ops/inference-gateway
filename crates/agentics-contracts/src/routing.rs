//! Routing agent input/output contracts.
//!
//! Defines the data structures for inference routing requests and responses,
//! enabling deterministic routing decisions with full audit trail support.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use validator::Validate;

/// Input contract for the inference routing agent.
///
/// This structure captures all information needed to make a routing decision,
/// including request metadata, constraints, and fallback preferences.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct InferenceRoutingInput {
    /// Unique identifier for the request.
    #[validate(length(min = 1, max = 128))]
    pub request_id: String,

    /// The model identifier requested by the client.
    ///
    /// This may be an alias (e.g., "gpt-4") that gets resolved to a specific
    /// provider model (e.g., "openai/gpt-4-turbo").
    #[validate(length(min = 1, max = 256))]
    pub model_requested: String,

    /// Optional tenant identifier for multi-tenant routing.
    pub tenant_id: Option<String>,

    /// Request headers that may influence routing (e.g., X-Provider-Hint).
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Optional list of provider constraints.
    ///
    /// If provided, only these providers will be considered for routing.
    pub provider_constraints: Option<Vec<String>>,

    /// Whether fallback to alternative providers is enabled.
    #[serde(default = "default_fallback_enabled")]
    pub fallback_enabled: bool,

    /// Priority hint for routing (lower = higher priority).
    pub priority: Option<u32>,

    /// Required capabilities for the route (e.g., "vision", "function_calling").
    #[serde(default)]
    pub required_capabilities: Vec<String>,

    /// Maximum latency threshold in milliseconds.
    pub max_latency_ms: Option<u64>,

    /// Cost budget constraint (provider-specific units).
    pub cost_budget: Option<f64>,
}

fn default_fallback_enabled() -> bool {
    true
}

impl InferenceRoutingInput {
    /// Creates a new routing input with minimal required fields.
    #[must_use]
    pub fn new(request_id: impl Into<String>, model_requested: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            model_requested: model_requested.into(),
            tenant_id: None,
            headers: HashMap::new(),
            provider_constraints: None,
            fallback_enabled: true,
            priority: None,
            required_capabilities: Vec::new(),
            max_latency_ms: None,
            cost_budget: None,
        }
    }

    /// Sets the tenant ID for this routing input.
    #[must_use]
    pub fn with_tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    /// Sets the provider constraints for this routing input.
    #[must_use]
    pub fn with_provider_constraints(mut self, constraints: Vec<String>) -> Self {
        self.provider_constraints = Some(constraints);
        self
    }

    /// Disables fallback routing.
    #[must_use]
    pub fn without_fallback(mut self) -> Self {
        self.fallback_enabled = false;
        self
    }

    /// Adds required capabilities.
    #[must_use]
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.required_capabilities = capabilities;
        self
    }

    /// Adds a header to the routing input.
    #[must_use]
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
}

/// Output contract for the inference routing agent.
///
/// Contains the routing decision including selected provider/model,
/// transformation details, and fallback options.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct InferenceRoutingOutput {
    /// The selected provider identifier.
    #[validate(length(min = 1, max = 128))]
    pub selected_provider: String,

    /// The selected model identifier (may differ from requested model).
    #[validate(length(min = 1, max = 256))]
    pub selected_model: String,

    /// Whether the model was transformed from the original request.
    ///
    /// This is true when the requested model alias was mapped to a
    /// provider-specific model identifier.
    pub model_transformed: bool,

    /// The routing path taken to reach this decision.
    ///
    /// Each step represents a phase in the routing evaluation.
    pub routing_path: Vec<RoutingStep>,

    /// List of fallback providers in priority order.
    ///
    /// These providers can be used if the selected provider fails.
    pub fallback_providers: Vec<String>,

    /// Estimated latency for the selected route in milliseconds.
    pub estimated_latency_ms: Option<u64>,

    /// Estimated cost for the selected route (provider-specific units).
    pub estimated_cost: Option<f64>,

    /// Provider endpoint URL (if different from default).
    pub endpoint_override: Option<String>,

    /// Additional metadata for the routing decision.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl InferenceRoutingOutput {
    /// Creates a new routing output with required fields.
    #[must_use]
    pub fn new(
        selected_provider: impl Into<String>,
        selected_model: impl Into<String>,
        model_transformed: bool,
    ) -> Self {
        Self {
            selected_provider: selected_provider.into(),
            selected_model: selected_model.into(),
            model_transformed,
            routing_path: Vec::new(),
            fallback_providers: Vec::new(),
            estimated_latency_ms: None,
            estimated_cost: None,
            endpoint_override: None,
            metadata: HashMap::new(),
        }
    }

    /// Adds routing steps to the output.
    #[must_use]
    pub fn with_routing_path(mut self, path: Vec<RoutingStep>) -> Self {
        self.routing_path = path;
        self
    }

    /// Adds fallback providers to the output.
    #[must_use]
    pub fn with_fallbacks(mut self, fallbacks: Vec<String>) -> Self {
        self.fallback_providers = fallbacks;
        self
    }

    /// Sets the estimated latency.
    #[must_use]
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.estimated_latency_ms = Some(latency_ms);
        self
    }

    /// Sets the estimated cost.
    #[must_use]
    pub fn with_cost(mut self, cost: f64) -> Self {
        self.estimated_cost = Some(cost);
        self
    }
}

/// A step in the routing evaluation path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingStep {
    /// Name of the routing phase (e.g., "model_resolution", "provider_selection").
    pub phase: String,

    /// Action taken in this phase.
    pub action: RoutingAction,

    /// Details about the step.
    pub details: Option<String>,

    /// Duration of this step in microseconds.
    pub duration_us: Option<u64>,
}

impl RoutingStep {
    /// Creates a new routing step.
    #[must_use]
    pub fn new(phase: impl Into<String>, action: RoutingAction) -> Self {
        Self {
            phase: phase.into(),
            action,
            details: None,
            duration_us: None,
        }
    }

    /// Adds details to the routing step.
    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Adds duration to the routing step.
    #[must_use]
    pub fn with_duration(mut self, duration_us: u64) -> Self {
        self.duration_us = Some(duration_us);
        self
    }
}

/// Action taken in a routing step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingAction {
    /// Model alias was resolved to provider-specific model.
    ResolveModel,
    /// Provider was selected from available options.
    SelectProvider,
    /// Constraint was evaluated.
    EvaluateConstraint,
    /// Provider was filtered out.
    FilterProvider,
    /// Fallback provider was selected.
    Fallback,
    /// Policy was applied.
    ApplyPolicy,
    /// Health check was performed.
    CheckHealth,
    /// Capability was verified.
    VerifyCapability,
    /// Cost was estimated.
    EstimateCost,
    /// Route was finalized.
    Finalize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_input_builder() {
        let input = InferenceRoutingInput::new("req-123", "gpt-4")
            .with_tenant("tenant-abc")
            .with_capabilities(vec!["vision".to_string()])
            .with_header("X-Provider-Hint", "openai")
            .without_fallback();

        assert_eq!(input.request_id, "req-123");
        assert_eq!(input.model_requested, "gpt-4");
        assert_eq!(input.tenant_id, Some("tenant-abc".to_string()));
        assert!(!input.fallback_enabled);
        assert_eq!(input.required_capabilities, vec!["vision"]);
        assert_eq!(
            input.headers.get("X-Provider-Hint"),
            Some(&"openai".to_string())
        );
    }

    #[test]
    fn test_routing_output_builder() {
        let output = InferenceRoutingOutput::new("openai", "gpt-4-turbo", true)
            .with_fallbacks(vec!["anthropic".to_string(), "azure".to_string()])
            .with_latency(150)
            .with_cost(0.003);

        assert_eq!(output.selected_provider, "openai");
        assert!(output.model_transformed);
        assert_eq!(output.fallback_providers.len(), 2);
        assert_eq!(output.estimated_latency_ms, Some(150));
        assert_eq!(output.estimated_cost, Some(0.003));
    }

    #[test]
    fn test_routing_step() {
        let step = RoutingStep::new("model_resolution", RoutingAction::ResolveModel)
            .with_details("Resolved gpt-4 to gpt-4-turbo")
            .with_duration(100);

        assert_eq!(step.phase, "model_resolution");
        assert_eq!(step.action, RoutingAction::ResolveModel);
        assert!(step.details.is_some());
        assert_eq!(step.duration_us, Some(100));
    }

    #[test]
    fn test_serialization() {
        let output = InferenceRoutingOutput::new("openai", "gpt-4", false);
        let json = serde_json::to_string(&output).unwrap();

        assert!(json.contains("\"selected_provider\":\"openai\""));
        assert!(json.contains("\"model_transformed\":false"));
    }
}
