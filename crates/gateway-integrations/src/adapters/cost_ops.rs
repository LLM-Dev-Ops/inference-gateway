//! LLM-CostOps adapter for cost-based routing.
//!
//! This adapter consumes cost projections from LLM-CostOps
//! to enable cost-efficient request routing.

use crate::config::CostOpsConfig;
use crate::error::{IntegrationError, IntegrationResult};
use crate::traits::{
    BudgetStatus, CostBreakdown, CostConsumer, CostProjection, CostRankedProvider, UsageReport,
};
use async_trait::async_trait;
use dashmap::DashMap;
use gateway_core::GatewayRequest;
use std::sync::Arc;
use tracing::{debug, instrument, warn};

/// Adapter for consuming cost information from LLM-CostOps.
///
/// This is a thin wrapper that consumes cost projections and
/// enables cost-efficient routing decisions.
pub struct CostOpsAdapter {
    /// Configuration
    config: CostOpsConfig,
    /// Budget status cache
    budget_cache: DashMap<String, CachedBudget>,
}

/// Cached budget status
struct CachedBudget {
    status: BudgetStatus,
    cached_at: std::time::Instant,
}

impl CostOpsAdapter {
    /// Create a new cost-ops adapter.
    pub fn new(config: CostOpsConfig) -> Self {
        Self {
            config,
            budget_cache: DashMap::new(),
        }
    }

    /// Check if the adapter is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if cost-based routing is enabled.
    pub fn cost_routing_enabled(&self) -> bool {
        self.config.cost_based_routing
    }

    /// Get the maximum cost per request.
    pub fn max_cost_per_request(&self) -> Option<f64> {
        self.config.max_cost_per_request
    }

    /// Estimate token count from request (simplified).
    fn estimate_tokens(request: &GatewayRequest) -> u32 {
        let content: String = request
            .messages
            .iter()
            .filter_map(|msg| msg.text_content())
            .collect::<Vec<_>>()
            .join(" ");

        // Rough estimate: ~4 characters per token
        (content.len() / 4).max(1) as u32
    }
}

#[async_trait]
impl CostConsumer for CostOpsAdapter {
    #[instrument(skip(self, request), fields(model = %request.model))]
    async fn get_cost_projection(
        &self,
        request: &GatewayRequest,
    ) -> IntegrationResult<CostProjection> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("cost-ops".to_string()));
        }

        debug!(model = %request.model, "Getting cost projection from cost-ops");

        let estimated_input_tokens = Self::estimate_tokens(request);
        let estimated_output_tokens = request.max_tokens.unwrap_or(1000);

        // Phase 2B: Cost projection interface ready.
        // Actual cost-ops client would calculate real costs here.
        // Using placeholder pricing for interface demonstration.

        let input_cost = (estimated_input_tokens as f64) * 0.00001; // $0.01 per 1K tokens
        let output_cost = (estimated_output_tokens as f64) * 0.00003; // $0.03 per 1K tokens

        Ok(CostProjection {
            estimated_cost: input_cost + output_cost,
            breakdown: CostBreakdown {
                input_cost,
                output_cost,
                base_cost: 0.0,
            },
            confidence: 0.5, // Low confidence - placeholder estimate
            provider_id: "unknown".to_string(),
        })
    }

    #[instrument(skip(self, request), fields(model = %request.model))]
    async fn get_cost_efficient_providers(
        &self,
        request: &GatewayRequest,
    ) -> IntegrationResult<Vec<CostRankedProvider>> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("cost-ops".to_string()));
        }

        if !self.config.cost_based_routing {
            debug!("Cost-based routing is disabled");
            return Ok(Vec::new());
        }

        debug!(model = %request.model, "Getting cost-efficient providers from cost-ops");

        // Phase 2B: Provider ranking interface ready.
        // Actual cost-ops client would fetch rankings here.

        Ok(Vec::new())
    }

    #[instrument(skip(self, usage), fields(request_id = %usage.request_id))]
    async fn report_usage(&self, usage: UsageReport) -> IntegrationResult<()> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("cost-ops".to_string()));
        }

        debug!(
            request_id = %usage.request_id,
            provider_id = %usage.provider_id,
            input_tokens = usage.input_tokens,
            output_tokens = usage.output_tokens,
            "Reporting usage to cost-ops"
        );

        // Phase 2B: Usage reporting interface ready.
        // Actual report submission would go here.

        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id))]
    async fn get_budget_status(&self, tenant_id: &str) -> IntegrationResult<BudgetStatus> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("cost-ops".to_string()));
        }

        debug!(tenant_id = %tenant_id, "Getting budget status from cost-ops");

        // Check cache first
        if let Some(cached) = self.budget_cache.get(tenant_id) {
            if cached.cached_at.elapsed() < std::time::Duration::from_secs(60) {
                return Ok(cached.status.clone());
            }
        }

        // Phase 2B: Budget status interface ready.
        // Actual cost-ops client would fetch status here.

        let status = BudgetStatus {
            tenant_id: tenant_id.to_string(),
            budget_limit: 1000.0,
            used: 0.0,
            remaining: 1000.0,
            usage_percentage: 0.0,
            over_budget: false,
            period: "monthly".to_string(),
        };

        // Cache the result
        self.budget_cache.insert(
            tenant_id.to_string(),
            CachedBudget {
                status: status.clone(),
                cached_at: std::time::Instant::now(),
            },
        );

        Ok(status)
    }
}

impl std::fmt::Debug for CostOpsAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CostOpsAdapter")
            .field("enabled", &self.config.enabled)
            .field("cost_based_routing", &self.config.cost_based_routing)
            .field("max_cost_per_request", &self.config.max_cost_per_request)
            .field("cached_budgets", &self.budget_cache.len())
            .finish()
    }
}

/// Builder for `CostOpsAdapter`
pub struct CostOpsAdapterBuilder {
    config: CostOpsConfig,
}

impl CostOpsAdapterBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: CostOpsConfig::default(),
        }
    }

    /// Set the configuration.
    pub fn config(mut self, config: CostOpsConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable the adapter.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Enable/disable cost-based routing.
    pub fn cost_based_routing(mut self, enabled: bool) -> Self {
        self.config.cost_based_routing = enabled;
        self
    }

    /// Set maximum cost per request.
    pub fn max_cost_per_request(mut self, max_cost: f64) -> Self {
        self.config.max_cost_per_request = Some(max_cost);
        self
    }

    /// Build the adapter.
    pub fn build(self) -> CostOpsAdapter {
        CostOpsAdapter::new(self.config)
    }
}

impl Default for CostOpsAdapterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_disabled_by_default() {
        let adapter = CostOpsAdapter::new(CostOpsConfig::default());
        assert!(!adapter.is_enabled());
    }

    #[test]
    fn test_token_estimation() {
        let request = gateway_core::GatewayRequest::builder()
            .model("gpt-4")
            .message(gateway_core::ChatMessage::user("Hello, how are you today?"))
            .build()
            .unwrap();

        let tokens = CostOpsAdapter::estimate_tokens(&request);
        assert!(tokens > 0);
    }

    #[tokio::test]
    async fn test_disabled_returns_not_enabled() {
        let adapter = CostOpsAdapter::new(CostOpsConfig::default());
        let request = gateway_core::GatewayRequest::builder()
            .model("gpt-4")
            .message(gateway_core::ChatMessage::user("test"))
            .build()
            .unwrap();

        let result = adapter.get_cost_projection(&request).await;
        assert!(matches!(result, Err(IntegrationError::NotEnabled(_))));
    }
}
