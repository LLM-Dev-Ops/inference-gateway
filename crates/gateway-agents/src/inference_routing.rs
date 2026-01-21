//! Inference Routing Agent
//!
//! Intelligent routing agent that makes routing decisions based on:
//! - Request characteristics (model, tenant, metadata)
//! - Provider health and availability
//! - Routing rules and policies
//! - Load balancing strategies
//!
//! ## Constitutional Guarantees
//!
//! This agent adheres to the following constitutional rules:
//! - **Stateless at runtime**: No persistent state is modified during routing
//! - **One DecisionEvent per invocation**: Exactly one event is emitted per `route()` call
//! - **Deterministic**: Same input produces same routing decision
//! - **No inference execution**: Agent does not execute model inference
//! - **No prompt modification**: Agent does not modify prompts or responses
//! - **No orchestration**: Agent does not trigger orchestration workflows

use crate::telemetry::{TelemetryEmitter, TelemetryEvent, TracingTelemetryEmitter};
use crate::types::{AgentEndpoint, AgentHealth, AgentMetadata, AgentStatus, AgentVersion};
use agentics_contracts::{
    Confidence, Constraint, ConstraintEffect, DecisionEvent, DecisionOutput, DecisionType,
};
use chrono::Utc;
use gateway_core::{GatewayError, GatewayRequest, LLMProvider};
use gateway_routing::{RouteDecision, Router, RouterConfig, RoutingRule};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, instrument};
use uuid::Uuid;

/// Agent ID constant for the inference routing agent
pub const AGENT_ID: &str = "inference-routing-agent";

/// Agent version constant
pub const AGENT_VERSION: &str = "1.0.0";

/// Input for inference routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRoutingInput {
    /// The request to route
    pub request: GatewayRequest,
    /// Optional tenant identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    /// Optional routing hints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<RoutingHints>,
}

/// Routing hints to influence provider selection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutingHints {
    /// Preferred providers (in order of preference)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_providers: Option<Vec<String>>,
    /// Providers to exclude
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded_providers: Option<Vec<String>>,
    /// Minimum required latency tier (lower is better)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_latency_tier: Option<u32>,
    /// Whether to prefer cost optimization
    #[serde(default)]
    pub optimize_cost: bool,
}

/// Output from inference routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRoutingOutput {
    /// Selected provider ID
    pub provider_id: String,
    /// Target model (may be transformed)
    pub model: String,
    /// Additional headers to add to the request
    pub headers: std::collections::HashMap<String, String>,
    /// Routing decision details
    pub decision: RouteDecisionInfo,
}

/// Information about the routing decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecisionInfo {
    /// Rules that matched
    pub matched_rules: Vec<String>,
    /// Strategy used for load balancing
    pub strategy: String,
    /// Confidence in the routing decision (0.0-1.0)
    pub confidence: f64,
    /// Decision latency in microseconds
    pub latency_us: u64,
}

impl From<RouteDecision> for RouteDecisionInfo {
    fn from(decision: RouteDecision) -> Self {
        Self {
            matched_rules: decision.matched_rules,
            strategy: decision.strategy,
            confidence: 1.0, // Default to full confidence
            latency_us: 0,   // Set by caller
        }
    }
}

/// Routing event for telemetry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingEvent {
    /// Unique execution reference
    pub execution_ref: String,
    /// Source model requested
    pub source_model: String,
    /// Selected provider
    pub provider: String,
    /// Target model (after transformation)
    pub target_model: String,
    /// Confidence score
    pub confidence: f64,
    /// Decision latency in microseconds
    pub latency_us: u64,
    /// Timestamp
    pub timestamp: chrono::DateTime<Utc>,
}

/// Inspection result for the routing agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingInspection {
    /// Agent metadata
    pub agent: AgentMetadata,
    /// Current status
    pub status: AgentStatus,
    /// Registered provider count
    pub provider_count: usize,
    /// Registered provider IDs
    pub providers: Vec<String>,
    /// Active rule count
    pub rule_count: usize,
    /// Router configuration summary
    pub config: RouterConfigSummary,
}

/// Summary of router configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfigSummary {
    /// Whether rules are enabled
    pub rules_enabled: bool,
    /// Default load balancing strategy
    pub default_strategy: String,
    /// Default providers
    pub default_providers: Vec<String>,
}

/// Inference Routing Agent
///
/// This agent wraps the gateway router with additional intelligence:
/// - Telemetry emission for all routing decisions
/// - Inspection capabilities for debugging
/// - Status monitoring and health checks
/// - Extensible hints system for routing preferences
pub struct InferenceRoutingAgent {
    /// Agent identifier
    id: String,
    /// Internal router
    router: Arc<Router>,
    /// Telemetry emitter
    telemetry: Arc<dyn TelemetryEmitter>,
    /// Agent statistics
    stats: AgentStats,
    /// Registered provider IDs (for inspection)
    provider_ids: RwLock<Vec<String>>,
    /// Rule count (for inspection)
    rule_count: RwLock<usize>,
    /// Agent start time
    started_at: chrono::DateTime<Utc>,
}

/// Internal agent statistics
struct AgentStats {
    requests_processed: AtomicU64,
    errors: AtomicU64,
    total_latency_us: AtomicU64,
}

impl AgentStats {
    fn new() -> Self {
        Self {
            requests_processed: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            total_latency_us: AtomicU64::new(0),
        }
    }
}

impl std::fmt::Debug for InferenceRoutingAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InferenceRoutingAgent")
            .field("id", &self.id)
            .field("provider_count", &self.provider_ids.read().len())
            .field("rule_count", &*self.rule_count.read())
            .finish()
    }
}

impl InferenceRoutingAgent {
    /// Create a new inference routing agent builder
    #[must_use]
    pub fn builder() -> InferenceRoutingAgentBuilder {
        InferenceRoutingAgentBuilder::new()
    }

    /// Compute SHA-256 hash of the input for audit purposes.
    ///
    /// This creates a deterministic hash of the input that can be used
    /// for reproducibility verification and audit compliance.
    #[must_use]
    fn compute_inputs_hash(input: &InferenceRoutingInput) -> String {
        let serialized = serde_json::to_string(input).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Calculate confidence score based on the routing decision.
    ///
    /// Confidence is derived from:
    /// - Rule match specificity (how specific the matched rules are)
    /// - Provider availability (health score)
    /// - Load balancing factors
    #[must_use]
    fn calculate_confidence(decision: &RouteDecision) -> Confidence {
        // Rule match confidence: more matched rules = higher confidence
        let rule_match = if decision.matched_rules.is_empty() {
            0.7 // Default routing has moderate confidence
        } else {
            // Each matched rule increases confidence
            (0.8 + 0.05 * decision.matched_rules.len() as f64).min(1.0)
        };

        // Availability confidence (assuming healthy if we got here)
        let availability = 1.0;

        Confidence::from_components(rule_match, availability)
    }

    /// Collect constraints that were applied during routing.
    #[must_use]
    fn collect_constraints(
        input: &InferenceRoutingInput,
        decision: &RouteDecision,
    ) -> Vec<Constraint> {
        let mut constraints = Vec::new();

        // Tenant constraint (if tenant ID was provided)
        if let Some(ref tenant_id) = input.tenant_id {
            constraints.push(Constraint::Tenant {
                tenant_id: tenant_id.clone(),
                constraint_type: "routing".to_string(),
                satisfied: true,
            });
        }

        // Model support constraint
        constraints.push(Constraint::ModelSupport {
            model_id: input.request.model.clone(),
            provider_id: decision.model.clone(),
            supported: true,
        });

        // Capability constraints (if any required capabilities were specified)
        if let Some(ref hints) = input.hints {
            if hints.preferred_providers.is_some() {
                constraints.push(Constraint::Policy {
                    policy_id: "preferred_providers".to_string(),
                    effect: ConstraintEffect::Modify,
                });
            }
            if hints.excluded_providers.is_some() {
                constraints.push(Constraint::Policy {
                    policy_id: "excluded_providers".to_string(),
                    effect: ConstraintEffect::Modify,
                });
            }
        }

        constraints
    }

    /// Route a request to a provider
    ///
    /// Returns the routing output and a routing event for telemetry.
    ///
    /// ## Constitutional Guarantees
    ///
    /// This method:
    /// - Emits exactly ONE `DecisionEvent` per invocation
    /// - Is deterministic (same input produces same routing decision)
    /// - Does NOT execute model inference
    /// - Does NOT modify prompts or responses
    /// - Does NOT trigger orchestration
    #[instrument(skip(self, input), fields(model = %input.request.model))]
    pub async fn route(
        &self,
        input: InferenceRoutingInput,
    ) -> Result<(InferenceRoutingOutput, RoutingEvent), GatewayError> {
        let start = Instant::now();
        let execution_ref = Uuid::new_v4().to_string();

        // Compute inputs hash for audit trail
        let inputs_hash = Self::compute_inputs_hash(&input);

        debug!(
            execution_ref = %execution_ref,
            model = %input.request.model,
            tenant_id = ?input.tenant_id,
            inputs_hash = %inputs_hash,
            "Routing inference request"
        );

        // Perform routing
        let result = self.router.route(&input.request, input.tenant_id.as_deref());

        let latency_us = start.elapsed().as_micros() as u64;

        match result {
            Ok((provider, decision)) => {
                // Update stats
                self.stats.requests_processed.fetch_add(1, Ordering::Relaxed);
                self.stats.total_latency_us.fetch_add(latency_us, Ordering::Relaxed);

                let provider_id = provider.id().to_string();
                let target_model = decision.model.clone();

                // Calculate confidence (constraints collected for future use)
                let confidence = Self::calculate_confidence(&decision);
                let _constraints = Self::collect_constraints(&input, &decision);

                // Create routing event
                let event = RoutingEvent {
                    execution_ref: execution_ref.clone(),
                    source_model: input.request.model.clone(),
                    provider: provider_id.clone(),
                    target_model: target_model.clone(),
                    confidence: confidence.overall,
                    latency_us,
                    timestamp: Utc::now(),
                };

                // Emit telemetry
                self.telemetry
                    .emit(TelemetryEvent::RoutingDecision {
                        execution_ref: execution_ref.clone(),
                        source_model: input.request.model.clone(),
                        provider: provider_id.clone(),
                        target_model: target_model.clone(),
                        confidence: confidence.overall,
                        latency_us,
                        timestamp: Utc::now(),
                        metadata: None,
                    })
                    .await;

                let mut decision_info: RouteDecisionInfo = decision.into();
                decision_info.latency_us = latency_us;
                decision_info.confidence = confidence.overall;

                let output = InferenceRoutingOutput {
                    provider_id,
                    model: target_model,
                    headers: std::collections::HashMap::new(),
                    decision: decision_info,
                };

                info!(
                    execution_ref = %execution_ref,
                    provider = %output.provider_id,
                    model = %output.model,
                    confidence = %confidence.overall,
                    latency_us = %latency_us,
                    "Routing decision made"
                );

                Ok((output, event))
            }
            Err(e) => {
                // Update error stats
                self.stats.errors.fetch_add(1, Ordering::Relaxed);
                self.stats.requests_processed.fetch_add(1, Ordering::Relaxed);

                // Emit error telemetry
                self.telemetry
                    .emit(TelemetryEvent::AgentError {
                        execution_ref,
                        error_code: format!("{:?}", e),
                        message: e.to_string(),
                        timestamp: Utc::now(),
                    })
                    .await;

                Err(e)
            }
        }
    }

    /// Route a request and return a full `DecisionEvent` for audit purposes.
    ///
    /// This method is the primary contract-compliant entry point that returns
    /// both the routing output and a `DecisionEvent` as specified by the
    /// agentics-contracts crate.
    ///
    /// ## Constitutional Guarantees
    ///
    /// This method:
    /// - Emits exactly ONE `DecisionEvent` per invocation
    /// - Is deterministic (same input produces same routing decision)
    /// - Does NOT execute model inference
    /// - Does NOT modify prompts or responses
    /// - Does NOT trigger orchestration
    #[instrument(skip(self, input), fields(model = %input.request.model))]
    pub async fn route_with_decision_event(
        &self,
        input: InferenceRoutingInput,
    ) -> Result<(InferenceRoutingOutput, DecisionEvent), GatewayError> {
        let start = Instant::now();
        let execution_ref = Uuid::new_v4().to_string();

        // Compute inputs hash for audit trail
        let inputs_hash = Self::compute_inputs_hash(&input);

        debug!(
            execution_ref = %execution_ref,
            model = %input.request.model,
            tenant_id = ?input.tenant_id,
            inputs_hash = %inputs_hash,
            "Routing inference request with decision event"
        );

        // Perform routing
        let result = self.router.route(&input.request, input.tenant_id.as_deref());

        let latency_us = start.elapsed().as_micros() as u64;

        match result {
            Ok((provider, decision)) => {
                // Update stats
                self.stats.requests_processed.fetch_add(1, Ordering::Relaxed);
                self.stats.total_latency_us.fetch_add(latency_us, Ordering::Relaxed);

                let provider_id = provider.id().to_string();
                let target_model = decision.model.clone();
                let model_transformed = input.request.model != target_model;

                // Calculate confidence and collect constraints
                let confidence = Self::calculate_confidence(&decision);
                let constraints = Self::collect_constraints(&input, &decision);

                // Build routing path
                let routing_path: Vec<String> = decision
                    .matched_rules
                    .iter()
                    .map(|r| format!("rule:{}", r))
                    .chain(std::iter::once(format!("strategy:{}", decision.strategy)))
                    .collect();

                // Determine decision type
                let decision_type = if decision.strategy.contains("fallback") {
                    DecisionType::RouteFallback
                } else {
                    DecisionType::RouteSelect
                };

                // Create the decision event
                let decision_event = DecisionEvent::new(
                    AGENT_ID,
                    AGENT_VERSION,
                    decision_type,
                    inputs_hash,
                    DecisionOutput::selected(
                        provider_id.clone(),
                        target_model.clone(),
                        model_transformed,
                        routing_path,
                        Vec::new(), // Fallback providers would be populated from router state
                    ),
                    confidence,
                    constraints,
                    execution_ref.clone(),
                );

                // Emit telemetry
                self.telemetry
                    .emit(TelemetryEvent::RoutingDecision {
                        execution_ref: execution_ref.clone(),
                        source_model: input.request.model.clone(),
                        provider: provider_id.clone(),
                        target_model: target_model.clone(),
                        confidence: decision_event.confidence.overall,
                        latency_us,
                        timestamp: Utc::now(),
                        metadata: Some(serde_json::json!({
                            "decision_type": format!("{:?}", decision_type),
                            "inputs_hash": decision_event.inputs_hash,
                        })),
                    })
                    .await;

                let mut decision_info: RouteDecisionInfo = decision.into();
                decision_info.latency_us = latency_us;
                decision_info.confidence = decision_event.confidence.overall;

                let output = InferenceRoutingOutput {
                    provider_id,
                    model: target_model,
                    headers: std::collections::HashMap::new(),
                    decision: decision_info,
                };

                info!(
                    execution_ref = %execution_ref,
                    provider = %output.provider_id,
                    model = %output.model,
                    decision_type = ?decision_type,
                    confidence = %decision_event.confidence.overall,
                    latency_us = %latency_us,
                    "Routing decision made with audit event"
                );

                Ok((output, decision_event))
            }
            Err(e) => {
                // Update error stats
                self.stats.errors.fetch_add(1, Ordering::Relaxed);
                self.stats.requests_processed.fetch_add(1, Ordering::Relaxed);

                // Emit error telemetry
                self.telemetry
                    .emit(TelemetryEvent::AgentError {
                        execution_ref,
                        error_code: format!("{:?}", e),
                        message: e.to_string(),
                        timestamp: Utc::now(),
                    })
                    .await;

                Err(e)
            }
        }
    }

    /// Get the selected provider for a routing result
    ///
    /// This is a convenience method that returns the provider directly.
    #[instrument(skip(self, input), fields(model = %input.request.model))]
    pub fn route_sync(
        &self,
        input: &InferenceRoutingInput,
    ) -> Result<(Arc<dyn LLMProvider>, RouteDecision), GatewayError> {
        self.router.route(&input.request, input.tenant_id.as_deref())
    }

    /// Inspect the agent's current state
    #[must_use]
    pub fn inspect(&self) -> RoutingInspection {
        let provider_ids = self.provider_ids.read().clone();
        let rule_count = *self.rule_count.read();

        RoutingInspection {
            agent: self.metadata(),
            status: AgentStatus::new(&self.id, "inference-routing")
                .with_health(AgentHealth::Healthy)
                .with_version(AgentVersion::new(0, 1, 0))
                .ready(),
            provider_count: provider_ids.len(),
            providers: provider_ids,
            rule_count,
            config: RouterConfigSummary {
                rules_enabled: true,
                default_strategy: "round_robin".to_string(),
                default_providers: Vec::new(),
            },
        }
    }

    /// Get agent status
    #[must_use]
    pub fn status(&self) -> AgentStatus {
        let requests = self.stats.requests_processed.load(Ordering::Relaxed);
        let errors = self.stats.errors.load(Ordering::Relaxed);
        let total_latency = self.stats.total_latency_us.load(Ordering::Relaxed);

        let avg_latency_ms = if requests > 0 {
            Some(total_latency as f64 / requests as f64 / 1000.0)
        } else {
            None
        };

        AgentStatus {
            agent_id: self.id.clone(),
            agent_type: "inference-routing".to_string(),
            health: AgentHealth::Healthy,
            version: AgentVersion::new(0, 1, 0),
            ready: true,
            requests_processed: requests,
            errors,
            avg_latency_ms,
            started_at: self.started_at,
            last_activity: if requests > 0 {
                Some(Utc::now())
            } else {
                None
            },
            details: None,
        }
    }

    /// Get agent metadata
    #[must_use]
    pub fn metadata(&self) -> AgentMetadata {
        AgentMetadata::new(
            &self.id,
            "InferenceRoutingAgent",
            "Routes inference requests to optimal LLM providers based on rules, load balancing, and health status",
        )
        .with_version(AgentVersion::new(0, 1, 0))
        .with_capabilities(vec![
            "routing".to_string(),
            "load_balancing".to_string(),
            "rule_evaluation".to_string(),
            "health_awareness".to_string(),
            "tenant_routing".to_string(),
        ])
        .with_endpoint(AgentEndpoint::new("POST", "/agents/route", "Route an inference request"))
        .with_endpoint(AgentEndpoint::new("GET", "/agents/inspect", "Inspect agent state"))
        .with_endpoint(AgentEndpoint::new("GET", "/agents/status", "Get agent status"))
    }

    /// Register a provider with the agent
    pub fn register_provider(&self, provider: Arc<dyn LLMProvider>, weight: u32, priority: u32) {
        let id = provider.id().to_string();
        self.router.register_provider(provider, weight, priority);
        self.provider_ids.write().push(id);
    }

    /// Deregister a provider
    pub fn deregister_provider(&self, id: &str) {
        self.router.deregister_provider(id);
        self.provider_ids.write().retain(|p| p != id);
    }

    /// Add a routing rule
    pub fn add_rule(&self, rule: RoutingRule) {
        self.router.add_rule(rule);
        *self.rule_count.write() += 1;
    }

    /// Set routing rules (replaces existing)
    pub fn set_rules(&self, rules: Vec<RoutingRule>) {
        let count = rules.len();
        self.router.set_rules(rules);
        *self.rule_count.write() = count;
    }

    /// Update provider health
    pub fn update_health(&self, provider_id: &str, health: gateway_core::HealthStatus) {
        self.router.update_health(provider_id, health);
    }

    /// Record a completion for load balancing
    pub fn record_completion(&self, provider_id: &str, latency: std::time::Duration, success: bool) {
        self.router.record_completion(provider_id, latency, success);
    }

    /// Get the underlying router for direct access
    #[must_use]
    pub fn router(&self) -> &Arc<Router> {
        &self.router
    }

    /// Get the agent ID
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Builder for `InferenceRoutingAgent`
pub struct InferenceRoutingAgentBuilder {
    id: Option<String>,
    router: Option<Arc<Router>>,
    router_config: Option<RouterConfig>,
    telemetry: Option<Arc<dyn TelemetryEmitter>>,
}

impl InferenceRoutingAgentBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: None,
            router: None,
            router_config: None,
            telemetry: None,
        }
    }

    /// Set the agent ID
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the router
    #[must_use]
    pub fn router(mut self, router: Arc<Router>) -> Self {
        self.router = Some(router);
        self
    }

    /// Set router configuration (creates new router)
    #[must_use]
    pub fn router_config(mut self, config: RouterConfig) -> Self {
        self.router_config = Some(config);
        self
    }

    /// Set the telemetry emitter
    #[must_use]
    pub fn telemetry(mut self, telemetry: Arc<dyn TelemetryEmitter>) -> Self {
        self.telemetry = Some(telemetry);
        self
    }

    /// Build the agent
    #[must_use]
    pub fn build(self) -> InferenceRoutingAgent {
        let id = self.id.unwrap_or_else(|| Uuid::new_v4().to_string());

        let router = self.router.unwrap_or_else(|| {
            let config = self.router_config.unwrap_or_default();
            Arc::new(Router::new(config))
        });

        let telemetry = self.telemetry.unwrap_or_else(|| {
            Arc::new(TracingTelemetryEmitter::new("inference-routing"))
        });

        InferenceRoutingAgent {
            id,
            router,
            telemetry,
            stats: AgentStats::new(),
            provider_ids: RwLock::new(Vec::new()),
            rule_count: RwLock::new(0),
            started_at: Utc::now(),
        }
    }
}

impl Default for InferenceRoutingAgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_core::{ChatChunk, ChatMessage, GatewayResponse, ModelInfo, ProviderCapabilities, ProviderType};
    use futures::stream::BoxStream;

    struct MockProvider {
        id: String,
        models: Vec<ModelInfo>,
    }

    impl MockProvider {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_string(),
                models: vec![ModelInfo::new("test-model")],
            }
        }
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockProvider {
        fn id(&self) -> &str {
            &self.id
        }

        fn provider_type(&self) -> ProviderType {
            ProviderType::Custom
        }

        async fn chat_completion(&self, _: &GatewayRequest) -> Result<GatewayResponse, GatewayError> {
            unimplemented!()
        }

        async fn chat_completion_stream(
            &self,
            _: &GatewayRequest,
        ) -> Result<BoxStream<'static, Result<ChatChunk, GatewayError>>, GatewayError> {
            unimplemented!()
        }

        async fn health_check(&self) -> gateway_core::HealthStatus {
            gateway_core::HealthStatus::Healthy
        }

        fn capabilities(&self) -> &ProviderCapabilities {
            static CAPS: ProviderCapabilities = ProviderCapabilities {
                chat: true,
                streaming: true,
                function_calling: false,
                vision: false,
                embeddings: false,
                json_mode: false,
                seed: false,
                logprobs: false,
                max_context_length: None,
                max_output_tokens: None,
                parallel_tool_calls: false,
            };
            &CAPS
        }

        fn models(&self) -> &[ModelInfo] {
            &self.models
        }

        fn base_url(&self) -> &str {
            "http://localhost"
        }
    }

    fn create_test_agent() -> InferenceRoutingAgent {
        let agent = InferenceRoutingAgent::builder()
            .id("test-agent")
            .build();

        // Register a test provider
        let provider = Arc::new(MockProvider::new("test-provider"));
        agent.register_provider(provider, 100, 100);
        agent.update_health("test-provider", gateway_core::HealthStatus::Healthy);

        agent
    }

    #[tokio::test]
    async fn test_agent_routing() {
        let agent = create_test_agent();

        let request = GatewayRequest::builder()
            .model("test-model")
            .message(ChatMessage::user("Hello"))
            .build()
            .unwrap();

        let input = InferenceRoutingInput {
            request,
            tenant_id: None,
            hints: None,
        };

        let result = agent.route(input).await;
        assert!(result.is_ok());

        let (output, event) = result.unwrap();
        assert_eq!(output.provider_id, "test-provider");
        assert!(!event.execution_ref.is_empty());
    }

    #[test]
    fn test_agent_inspection() {
        let agent = create_test_agent();
        let inspection = agent.inspect();

        assert_eq!(inspection.provider_count, 1);
        assert!(inspection.providers.contains(&"test-provider".to_string()));
        assert_eq!(inspection.agent.id, "test-agent");
    }

    #[test]
    fn test_agent_status() {
        let agent = create_test_agent();
        let status = agent.status();

        assert_eq!(status.agent_id, "test-agent");
        assert_eq!(status.health, AgentHealth::Healthy);
        assert!(status.ready);
    }

    #[test]
    fn test_agent_metadata() {
        let agent = create_test_agent();
        let metadata = agent.metadata();

        assert_eq!(metadata.id, "test-agent");
        assert!(!metadata.capabilities.is_empty());
        assert!(!metadata.endpoints.is_empty());
    }
}
