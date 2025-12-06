//! LLM-Auto-Optimizer adapter for optimization hints.
//!
//! This adapter consumes optimization hints and recommendation feedback
//! from LLM-Auto-Optimizer for continuous improvement.

use crate::config::AutoOptimizerConfig;
use crate::error::{IntegrationError, IntegrationResult};
use crate::traits::{
    CacheRecommendation, OptimizationConsumer, OptimizationHints, OptimizationOutcome,
    ParameterAdjustments, Recommendation,
};
use async_trait::async_trait;
use dashmap::DashMap;
use gateway_core::GatewayRequest;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, instrument, warn};

/// Adapter for consuming optimization hints from LLM-Auto-Optimizer.
///
/// This is a thin wrapper that consumes optimization recommendations
/// and feedback for continuous improvement.
pub struct AutoOptimizerAdapter {
    /// Configuration
    config: AutoOptimizerConfig,
    /// Cache for optimization hints by model
    hints_cache: DashMap<String, CachedHints>,
    /// Pending recommendations
    pending_recommendations: DashMap<String, Recommendation>,
}

/// Cached optimization hints
struct CachedHints {
    hints: OptimizationHints,
    cached_at: Instant,
}

impl AutoOptimizerAdapter {
    /// Create a new auto-optimizer adapter.
    pub fn new(config: AutoOptimizerConfig) -> Self {
        Self {
            config,
            hints_cache: DashMap::new(),
            pending_recommendations: DashMap::new(),
        }
    }

    /// Check if the adapter is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if hint consumption is enabled.
    pub fn consumes_hints(&self) -> bool {
        self.config.consume_hints
    }

    /// Check if auto-apply is enabled.
    pub fn auto_apply_enabled(&self) -> bool {
        self.config.auto_apply
    }

    /// Get cache key for a request.
    fn cache_key(request: &GatewayRequest) -> String {
        format!(
            "{}:{}",
            request.model,
            request.temperature.map(|t| t.to_string()).unwrap_or_default()
        )
    }

    /// Check if cached hints are still valid.
    fn is_cache_valid(cached_at: Instant) -> bool {
        cached_at.elapsed() < std::time::Duration::from_secs(300) // 5 minute TTL
    }
}

#[async_trait]
impl OptimizationConsumer for AutoOptimizerAdapter {
    #[instrument(skip(self, request), fields(model = %request.model))]
    async fn get_optimization_hints(
        &self,
        request: &GatewayRequest,
    ) -> IntegrationResult<OptimizationHints> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("auto-optimizer".to_string()));
        }

        if !self.config.consume_hints {
            debug!("Hint consumption is disabled");
            return Ok(OptimizationHints {
                suggested_model: None,
                suggested_provider: None,
                parameter_adjustments: ParameterAdjustments::default(),
                cache_recommendation: CacheRecommendation {
                    should_cache: false,
                    ttl_seconds: None,
                    cache_key_hint: None,
                },
                confidence: 0.0,
                reason: "Optimization hints disabled".to_string(),
            });
        }

        let cache_key = Self::cache_key(request);
        debug!(
            model = %request.model,
            cache_key = %cache_key,
            "Getting optimization hints from auto-optimizer"
        );

        // Check cache first
        if let Some(cached) = self.hints_cache.get(&cache_key) {
            if Self::is_cache_valid(cached.cached_at) {
                return Ok(cached.hints.clone());
            }
        }

        // Phase 2B: Optimization hints interface ready.
        // Actual auto-optimizer client would fetch hints here.

        let hints = OptimizationHints {
            suggested_model: None,
            suggested_provider: None,
            parameter_adjustments: ParameterAdjustments::default(),
            cache_recommendation: CacheRecommendation {
                should_cache: false,
                ttl_seconds: None,
                cache_key_hint: None,
            },
            confidence: 0.0,
            reason: "Auto-optimizer integration pending full implementation".to_string(),
        };

        // Cache the hints
        self.hints_cache.insert(
            cache_key,
            CachedHints {
                hints: hints.clone(),
                cached_at: Instant::now(),
            },
        );

        Ok(hints)
    }

    #[instrument(skip(self))]
    async fn consume_recommendations(&self) -> IntegrationResult<Vec<Recommendation>> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("auto-optimizer".to_string()));
        }

        if !self.config.consume_feedback {
            debug!("Recommendation feedback consumption is disabled");
            return Ok(Vec::new());
        }

        debug!("Consuming recommendations from auto-optimizer");

        // Phase 2B: Recommendation consumption interface ready.
        // Actual recommendation fetching would go here.

        // Return any pending recommendations
        let recommendations: Vec<Recommendation> = self
            .pending_recommendations
            .iter()
            .map(|entry| entry.value().clone())
            .collect();

        Ok(recommendations)
    }

    #[instrument(skip(self, outcome), fields(request_id = %outcome.request_id))]
    async fn report_outcome(&self, outcome: OptimizationOutcome) -> IntegrationResult<()> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("auto-optimizer".to_string()));
        }

        debug!(
            request_id = %outcome.request_id,
            successful = outcome.successful,
            hints_applied = outcome.applied_hints.len(),
            "Reporting optimization outcome to auto-optimizer"
        );

        // Phase 2B: Outcome reporting interface ready.
        // Actual report submission would go here.

        Ok(())
    }
}

impl std::fmt::Debug for AutoOptimizerAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutoOptimizerAdapter")
            .field("enabled", &self.config.enabled)
            .field("consume_hints", &self.config.consume_hints)
            .field("auto_apply", &self.config.auto_apply)
            .field("cached_hints", &self.hints_cache.len())
            .finish()
    }
}

/// Builder for `AutoOptimizerAdapter`
pub struct AutoOptimizerAdapterBuilder {
    config: AutoOptimizerConfig,
}

impl AutoOptimizerAdapterBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: AutoOptimizerConfig::default(),
        }
    }

    /// Set the configuration.
    pub fn config(mut self, config: AutoOptimizerConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable the adapter.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Enable/disable hint consumption.
    pub fn consume_hints(mut self, enabled: bool) -> Self {
        self.config.consume_hints = enabled;
        self
    }

    /// Enable/disable auto-apply.
    pub fn auto_apply(mut self, enabled: bool) -> Self {
        self.config.auto_apply = enabled;
        self
    }

    /// Build the adapter.
    pub fn build(self) -> AutoOptimizerAdapter {
        AutoOptimizerAdapter::new(self.config)
    }
}

impl Default for AutoOptimizerAdapterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_disabled_by_default() {
        let adapter = AutoOptimizerAdapter::new(AutoOptimizerConfig::default());
        assert!(!adapter.is_enabled());
    }

    #[test]
    fn test_adapter_builder() {
        let adapter = AutoOptimizerAdapterBuilder::new()
            .enabled(true)
            .consume_hints(true)
            .auto_apply(false)
            .build();

        assert!(adapter.is_enabled());
        assert!(adapter.consumes_hints());
        assert!(!adapter.auto_apply_enabled());
    }

    #[test]
    fn test_cache_key_generation() {
        let request = gateway_core::GatewayRequest::builder()
            .model("gpt-4")
            .message(gateway_core::ChatMessage::user("test"))
            .temperature(0.7)
            .build()
            .unwrap();

        let key = AutoOptimizerAdapter::cache_key(&request);
        assert!(key.contains("gpt-4"));
        assert!(key.contains("0.7"));
    }

    #[tokio::test]
    async fn test_disabled_returns_not_enabled() {
        let adapter = AutoOptimizerAdapter::new(AutoOptimizerConfig::default());
        let request = gateway_core::GatewayRequest::builder()
            .model("gpt-4")
            .message(gateway_core::ChatMessage::user("test"))
            .build()
            .unwrap();

        let result = adapter.get_optimization_hints(&request).await;
        assert!(matches!(result, Err(IntegrationError::NotEnabled(_))));
    }
}
