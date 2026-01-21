//! Integration manager for coordinating all adapters.
//!
//! This module provides a unified interface for managing and using
//! all LLM-Dev-Ops ecosystem integrations.

use crate::config::IntegrationsConfig;
use crate::error::IntegrationResult;
use crate::traits::*;
use std::sync::Arc;
use tracing::{debug, instrument, warn};

use super::{
    AutoOptimizerAdapter, ConnectorHubAdapter, CostOpsAdapter, ObservatoryAdapter,
    PolicyEngineAdapter, RouterAdapter, RuVectorClient, SentinelAdapter, ShieldAdapter,
};

/// Manager for all integration adapters.
///
/// This provides a unified interface for accessing all LLM-Dev-Ops
/// ecosystem integrations without modifying existing gateway APIs.
pub struct IntegrationManager {
    /// Connector hub adapter for provider routing
    connector_hub: Arc<ConnectorHubAdapter>,
    /// Shield adapter for safety filtering
    shield: Arc<ShieldAdapter>,
    /// Sentinel adapter for anomaly detection
    sentinel: Arc<SentinelAdapter>,
    /// CostOps adapter for cost-based routing
    cost_ops: Arc<CostOpsAdapter>,
    /// Observatory adapter for telemetry
    observatory: Arc<ObservatoryAdapter>,
    /// Router adapter for routing rules
    router: Arc<RouterAdapter>,
    /// Auto-optimizer adapter for optimization hints
    auto_optimizer: Arc<AutoOptimizerAdapter>,
    /// Policy engine adapter for policy enforcement
    policy_engine: Arc<PolicyEngineAdapter>,
    /// RuVector client for persistence (DecisionEvents)
    ruvector: Option<Arc<RuVectorClient>>,
    /// Overall enabled state
    enabled: bool,
}

impl IntegrationManager {
    /// Create a new integration manager from configuration.
    pub fn new(config: IntegrationsConfig) -> Self {
        // Create RuVector client if configured
        let ruvector = if config.ruvector.enabled {
            match RuVectorClient::new(config.ruvector.clone()) {
                Ok(client) => Some(Arc::new(client)),
                Err(e) => {
                    warn!(error = %e, "Failed to create RuVector client, persistence disabled");
                    None
                }
            }
        } else {
            None
        };

        Self {
            connector_hub: Arc::new(ConnectorHubAdapter::new(config.connector_hub)),
            shield: Arc::new(ShieldAdapter::new(config.shield)),
            sentinel: Arc::new(SentinelAdapter::new(config.sentinel)),
            cost_ops: Arc::new(CostOpsAdapter::new(config.cost_ops)),
            observatory: Arc::new(ObservatoryAdapter::new(config.observatory)),
            router: Arc::new(RouterAdapter::new(config.router)),
            auto_optimizer: Arc::new(AutoOptimizerAdapter::new(config.auto_optimizer)),
            policy_engine: Arc::new(PolicyEngineAdapter::new(config.policy_engine)),
            ruvector,
            enabled: config.enabled,
        }
    }

    /// Create a disabled integration manager (no-op).
    pub fn disabled() -> Self {
        Self::new(IntegrationsConfig::default())
    }

    /// Check if integrations are enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    // =========================================================================
    // Adapter access
    // =========================================================================

    /// Get the connector hub adapter.
    pub fn connector_hub(&self) -> &Arc<ConnectorHubAdapter> {
        &self.connector_hub
    }

    /// Get the shield adapter.
    pub fn shield(&self) -> &Arc<ShieldAdapter> {
        &self.shield
    }

    /// Get the sentinel adapter.
    pub fn sentinel(&self) -> &Arc<SentinelAdapter> {
        &self.sentinel
    }

    /// Get the cost-ops adapter.
    pub fn cost_ops(&self) -> &Arc<CostOpsAdapter> {
        &self.cost_ops
    }

    /// Get the observatory adapter.
    pub fn observatory(&self) -> &Arc<ObservatoryAdapter> {
        &self.observatory
    }

    /// Get the router adapter.
    pub fn router(&self) -> &Arc<RouterAdapter> {
        &self.router
    }

    /// Get the auto-optimizer adapter.
    pub fn auto_optimizer(&self) -> &Arc<AutoOptimizerAdapter> {
        &self.auto_optimizer
    }

    /// Get the policy engine adapter.
    pub fn policy_engine(&self) -> &Arc<PolicyEngineAdapter> {
        &self.policy_engine
    }

    /// Get the RuVector client for persistence operations.
    ///
    /// Returns None if RuVector is not configured or failed to initialize.
    pub fn ruvector(&self) -> Option<&Arc<RuVectorClient>> {
        self.ruvector.as_ref()
    }

    /// Check if RuVector persistence is available.
    pub fn has_ruvector(&self) -> bool {
        self.ruvector.is_some()
    }

    // =========================================================================
    // High-level integration workflows
    // =========================================================================

    /// Pre-request processing workflow.
    ///
    /// This consumes from multiple integrations to prepare a request:
    /// 1. Policy evaluation (if enabled)
    /// 2. Safety filtering on input (if enabled)
    /// 3. Cost projection (if enabled)
    /// 4. Provider recommendation (if enabled)
    /// 5. Optimization hints (if enabled)
    #[instrument(skip(self, request), fields(model = %request.model))]
    pub async fn pre_request(
        &self,
        request: &gateway_core::GatewayRequest,
    ) -> IntegrationResult<PreRequestResult> {
        if !self.enabled {
            return Ok(PreRequestResult::passthrough());
        }

        debug!(model = %request.model, "Running pre-request integrations");

        let mut result = PreRequestResult::default();

        // Policy check
        if self.policy_engine.is_enabled() && self.policy_engine.pre_request_enabled() {
            match self.policy_engine.evaluate_request(request).await {
                Ok(decision) => {
                    result.policy_decision = Some(decision.clone());
                    if !decision.allowed && self.policy_engine.blocks_on_violation() {
                        result.blocked = true;
                        result.block_reason =
                            Some("Policy violation".to_string());
                        return Ok(result);
                    }
                }
                Err(e) => {
                    debug!(error = %e, "Policy evaluation failed, continuing");
                }
            }
        }

        // Safety check on input
        if self.shield.is_enabled() && self.shield.validates_input() {
            match self.shield.validate_input(request).await {
                Ok(safety) => {
                    result.safety_result = Some(safety.clone());
                    if safety.should_block {
                        result.blocked = true;
                        result.block_reason = Some("Safety violation".to_string());
                        return Ok(result);
                    }
                }
                Err(e) => {
                    debug!(error = %e, "Safety validation failed, continuing");
                }
            }
        }

        // Cost projection
        if self.cost_ops.is_enabled() {
            match self.cost_ops.get_cost_projection(request).await {
                Ok(projection) => {
                    result.cost_projection = Some(projection);
                }
                Err(e) => {
                    debug!(error = %e, "Cost projection failed, continuing");
                }
            }
        }

        // Provider recommendation
        if self.connector_hub.is_enabled() {
            match self.connector_hub.get_provider_recommendation(request).await {
                Ok(recommendation) => {
                    result.provider_recommendation = Some(recommendation);
                }
                Err(e) => {
                    debug!(error = %e, "Provider recommendation failed, continuing");
                }
            }
        }

        // Optimization hints
        if self.auto_optimizer.is_enabled() && self.auto_optimizer.consumes_hints() {
            match self.auto_optimizer.get_optimization_hints(request).await {
                Ok(hints) => {
                    result.optimization_hints = Some(hints);
                }
                Err(e) => {
                    debug!(error = %e, "Optimization hints failed, continuing");
                }
            }
        }

        // Anomaly check for preferred provider
        if self.sentinel.is_enabled() {
            if let Some(provider_id) = request
                .metadata
                .as_ref()
                .and_then(|m| m.preferred_provider.as_ref())
            {
                match self.sentinel.check_provider_anomalies(provider_id).await {
                    Ok(status) => {
                        result.anomaly_status = Some(status);
                    }
                    Err(e) => {
                        debug!(error = %e, "Anomaly check failed, continuing");
                    }
                }
            }
        }

        Ok(result)
    }

    /// Post-response processing workflow.
    ///
    /// This consumes from multiple integrations after a response:
    /// 1. Safety filtering on output (if enabled)
    /// 2. Policy evaluation (if enabled)
    /// 3. Usage reporting (if enabled)
    /// 4. Telemetry emission (if enabled)
    #[instrument(skip(self, request, response), fields(model = %request.model))]
    pub async fn post_response(
        &self,
        request: &gateway_core::GatewayRequest,
        response: &gateway_core::GatewayResponse,
    ) -> IntegrationResult<PostResponseResult> {
        if !self.enabled {
            return Ok(PostResponseResult::passthrough());
        }

        debug!(model = %request.model, "Running post-response integrations");

        let mut result = PostResponseResult::default();

        // Safety check on output
        if self.shield.is_enabled() && self.shield.validates_output() {
            match self.shield.validate_output(response).await {
                Ok(safety) => {
                    result.safety_result = Some(safety.clone());
                    if safety.should_block {
                        result.blocked = true;
                        result.block_reason = Some("Output safety violation".to_string());
                    }
                }
                Err(e) => {
                    debug!(error = %e, "Output safety validation failed, continuing");
                }
            }
        }

        // Policy check on response
        if self.policy_engine.is_enabled() && self.policy_engine.post_response_enabled() {
            match self.policy_engine.evaluate_response(request, response).await {
                Ok(decision) => {
                    result.policy_decision = Some(decision.clone());
                    if !decision.allowed && self.policy_engine.blocks_on_violation() {
                        result.blocked = true;
                        result.block_reason = Some("Response policy violation".to_string());
                    }
                }
                Err(e) => {
                    debug!(error = %e, "Response policy evaluation failed, continuing");
                }
            }
        }

        // Report usage
        if self.cost_ops.is_enabled() {
            let usage_report = UsageReport {
                request_id: request.id.to_string(),
                provider_id: response.provider.clone().unwrap_or_default(),
                model: response.model.clone(),
                input_tokens: response.usage.prompt_tokens,
                output_tokens: response.usage.completion_tokens,
                actual_cost: None,
                tenant_id: request.metadata.as_ref().and_then(|m| m.tenant_id.clone()),
                timestamp: chrono::Utc::now(),
            };

            if let Err(e) = self.cost_ops.report_usage(usage_report).await {
                debug!(error = %e, "Usage reporting failed, continuing");
            }
        }

        Ok(result)
    }

    /// Emit telemetry for a request.
    #[instrument(skip(self, profile))]
    pub async fn emit_telemetry(&self, profile: LatencyProfile) -> IntegrationResult<()> {
        if !self.enabled || !self.observatory.is_enabled() {
            return Ok(());
        }

        self.observatory.emit_latency_profile(profile).await
    }

    /// Flush all pending telemetry.
    pub async fn flush_telemetry(&self) -> IntegrationResult<()> {
        if !self.enabled || !self.observatory.is_enabled() {
            return Ok(());
        }

        self.observatory.flush().await
    }

    /// Get integration status for health checks.
    pub fn status(&self) -> IntegrationStatus {
        IntegrationStatus {
            enabled: self.enabled,
            connector_hub_enabled: self.connector_hub.is_enabled(),
            shield_enabled: self.shield.is_enabled(),
            sentinel_enabled: self.sentinel.is_enabled(),
            cost_ops_enabled: self.cost_ops.is_enabled(),
            observatory_enabled: self.observatory.is_enabled(),
            router_enabled: self.router.is_enabled(),
            auto_optimizer_enabled: self.auto_optimizer.is_enabled(),
            policy_engine_enabled: self.policy_engine.is_enabled(),
            ruvector_enabled: self.ruvector.as_ref().map(|r| r.is_enabled()).unwrap_or(false),
        }
    }
}

impl std::fmt::Debug for IntegrationManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntegrationManager")
            .field("enabled", &self.enabled)
            .field("connector_hub", &self.connector_hub)
            .field("shield", &self.shield)
            .field("sentinel", &self.sentinel)
            .field("cost_ops", &self.cost_ops)
            .field("observatory", &self.observatory)
            .field("router", &self.router)
            .field("auto_optimizer", &self.auto_optimizer)
            .field("policy_engine", &self.policy_engine)
            .field("ruvector", &self.ruvector)
            .finish()
    }
}

/// Result of pre-request processing.
#[derive(Debug, Clone, Default)]
pub struct PreRequestResult {
    /// Whether the request should be blocked
    pub blocked: bool,
    /// Reason for blocking
    pub block_reason: Option<String>,
    /// Policy decision
    pub policy_decision: Option<PolicyDecision>,
    /// Safety validation result
    pub safety_result: Option<SafetyResult>,
    /// Cost projection
    pub cost_projection: Option<CostProjection>,
    /// Provider recommendation
    pub provider_recommendation: Option<ProviderRecommendation>,
    /// Optimization hints
    pub optimization_hints: Option<OptimizationHints>,
    /// Anomaly status
    pub anomaly_status: Option<AnomalyStatus>,
}

impl PreRequestResult {
    /// Create a passthrough result (no integrations applied).
    pub fn passthrough() -> Self {
        Self::default()
    }

    /// Check if any integration was applied.
    pub fn has_integrations(&self) -> bool {
        self.policy_decision.is_some()
            || self.safety_result.is_some()
            || self.cost_projection.is_some()
            || self.provider_recommendation.is_some()
            || self.optimization_hints.is_some()
            || self.anomaly_status.is_some()
    }
}

/// Result of post-response processing.
#[derive(Debug, Clone, Default)]
pub struct PostResponseResult {
    /// Whether the response should be blocked
    pub blocked: bool,
    /// Reason for blocking
    pub block_reason: Option<String>,
    /// Safety validation result
    pub safety_result: Option<SafetyResult>,
    /// Policy decision
    pub policy_decision: Option<PolicyDecision>,
}

impl PostResponseResult {
    /// Create a passthrough result (no integrations applied).
    pub fn passthrough() -> Self {
        Self::default()
    }
}

/// Integration status for health checks.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IntegrationStatus {
    /// Overall integrations enabled
    pub enabled: bool,
    /// Connector hub enabled
    pub connector_hub_enabled: bool,
    /// Shield enabled
    pub shield_enabled: bool,
    /// Sentinel enabled
    pub sentinel_enabled: bool,
    /// CostOps enabled
    pub cost_ops_enabled: bool,
    /// Observatory enabled
    pub observatory_enabled: bool,
    /// Router enabled
    pub router_enabled: bool,
    /// Auto-optimizer enabled
    pub auto_optimizer_enabled: bool,
    /// Policy engine enabled
    pub policy_engine_enabled: bool,
    /// RuVector persistence enabled
    pub ruvector_enabled: bool,
}

/// Builder for `IntegrationManager`
pub struct IntegrationManagerBuilder {
    config: IntegrationsConfig,
}

impl IntegrationManagerBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: IntegrationsConfig::default(),
        }
    }

    /// Set the full configuration.
    pub fn config(mut self, config: IntegrationsConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable all integrations.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Build the manager.
    pub fn build(self) -> IntegrationManager {
        IntegrationManager::new(self.config)
    }
}

impl Default for IntegrationManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_disabled_by_default() {
        let manager = IntegrationManager::disabled();
        assert!(!manager.is_enabled());
    }

    #[test]
    fn test_status() {
        let manager = IntegrationManager::disabled();
        let status = manager.status();

        assert!(!status.enabled);
        assert!(!status.connector_hub_enabled);
        assert!(!status.shield_enabled);
    }

    #[tokio::test]
    async fn test_pre_request_passthrough_when_disabled() {
        let manager = IntegrationManager::disabled();
        let request = gateway_core::GatewayRequest::builder()
            .model("gpt-4")
            .message(gateway_core::ChatMessage::user("test"))
            .build()
            .unwrap();

        let result = manager.pre_request(&request).await.unwrap();
        assert!(!result.blocked);
        assert!(!result.has_integrations());
    }
}
