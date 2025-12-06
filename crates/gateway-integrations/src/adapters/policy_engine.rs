//! LLM-Policy-Engine adapter for policy enforcement.
//!
//! This adapter consumes policy enforcement decisions from LLM-Policy-Engine
//! before and after request execution.

use crate::config::PolicyEngineConfig;
use crate::error::{IntegrationError, IntegrationResult};
use crate::traits::{
    EnforcementOutcome, EnforcementReport, Policy, PolicyAction, PolicyActionType,
    PolicyConsumer, PolicyDecision, PolicyRule, PolicyViolation,
};
use async_trait::async_trait;
use dashmap::DashMap;
use gateway_core::{GatewayRequest, GatewayResponse};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, instrument, warn};

/// Adapter for consuming policy decisions from LLM-Policy-Engine.
///
/// This is a thin wrapper that consumes policy enforcement decisions
/// before and after request execution.
pub struct PolicyEngineAdapter {
    /// Configuration
    config: PolicyEngineConfig,
    /// Cached policies
    policy_cache: DashMap<String, CachedPolicy>,
    /// Decision cache (keyed by request hash)
    decision_cache: DashMap<String, CachedDecision>,
}

/// Cached policy
struct CachedPolicy {
    policy: Policy,
    cached_at: Instant,
}

/// Cached decision
struct CachedDecision {
    decision: PolicyDecision,
    cached_at: Instant,
}

impl PolicyEngineAdapter {
    /// Create a new policy engine adapter.
    pub fn new(config: PolicyEngineConfig) -> Self {
        Self {
            config,
            policy_cache: DashMap::new(),
            decision_cache: DashMap::new(),
        }
    }

    /// Check if the adapter is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if pre-request check is enabled.
    pub fn pre_request_enabled(&self) -> bool {
        self.config.pre_request_check
    }

    /// Check if post-response check is enabled.
    pub fn post_response_enabled(&self) -> bool {
        self.config.post_response_check
    }

    /// Check if blocking on violation is enabled.
    pub fn blocks_on_violation(&self) -> bool {
        self.config.block_on_violation
    }

    /// Generate a cache key for a request.
    fn request_cache_key(request: &GatewayRequest) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        request.model.hash(&mut hasher);
        if let Some(tenant) = request.metadata.as_ref().and_then(|m| m.tenant_id.as_ref()) {
            tenant.hash(&mut hasher);
        }
        format!("req:{:x}", hasher.finish())
    }

    /// Check if cached decision is still valid.
    fn is_cache_valid(&self, cached_at: Instant) -> bool {
        cached_at.elapsed() < self.config.cache_ttl
    }
}

#[async_trait]
impl PolicyConsumer for PolicyEngineAdapter {
    #[instrument(skip(self, request), fields(model = %request.model))]
    async fn evaluate_request(
        &self,
        request: &GatewayRequest,
    ) -> IntegrationResult<PolicyDecision> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("policy-engine".to_string()));
        }

        if !self.config.pre_request_check {
            debug!("Pre-request policy check is disabled");
            return Ok(PolicyDecision {
                allowed: true,
                matched_policies: Vec::new(),
                violations: Vec::new(),
                required_actions: Vec::new(),
                metadata: HashMap::new(),
            });
        }

        let cache_key = Self::request_cache_key(request);
        debug!(
            model = %request.model,
            cache_key = %cache_key,
            "Evaluating request against policies"
        );

        // Check cache if enabled
        if self.config.cache_decisions {
            if let Some(cached) = self.decision_cache.get(&cache_key) {
                if self.is_cache_valid(cached.cached_at) {
                    debug!("Returning cached policy decision");
                    return Ok(cached.decision.clone());
                }
            }
        }

        // Phase 2B: Policy evaluation interface ready.
        // Actual policy engine client would evaluate here.

        let decision = PolicyDecision {
            allowed: true,
            matched_policies: Vec::new(),
            violations: Vec::new(),
            required_actions: Vec::new(),
            metadata: HashMap::new(),
        };

        // Cache the decision
        if self.config.cache_decisions {
            self.decision_cache.insert(
                cache_key,
                CachedDecision {
                    decision: decision.clone(),
                    cached_at: Instant::now(),
                },
            );
        }

        Ok(decision)
    }

    #[instrument(skip(self, request, response), fields(model = %request.model))]
    async fn evaluate_response(
        &self,
        request: &GatewayRequest,
        response: &GatewayResponse,
    ) -> IntegrationResult<PolicyDecision> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("policy-engine".to_string()));
        }

        if !self.config.post_response_check {
            debug!("Post-response policy check is disabled");
            return Ok(PolicyDecision {
                allowed: true,
                matched_policies: Vec::new(),
                violations: Vec::new(),
                required_actions: Vec::new(),
                metadata: HashMap::new(),
            });
        }

        debug!(
            model = %request.model,
            response_model = %response.model,
            "Evaluating response against policies"
        );

        // Phase 2B: Response policy evaluation interface ready.
        // Actual policy engine client would evaluate here.

        Ok(PolicyDecision {
            allowed: true,
            matched_policies: Vec::new(),
            violations: Vec::new(),
            required_actions: Vec::new(),
            metadata: HashMap::new(),
        })
    }

    #[instrument(skip(self))]
    async fn consume_policy_updates(&self) -> IntegrationResult<Vec<Policy>> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("policy-engine".to_string()));
        }

        debug!("Consuming policy updates from policy engine");

        // Return cached policies
        let policies: Vec<Policy> = self
            .policy_cache
            .iter()
            .filter(|entry| self.is_cache_valid(entry.value().cached_at))
            .map(|entry| entry.value().policy.clone())
            .collect();

        if !policies.is_empty() {
            return Ok(policies);
        }

        // Phase 2B: Policy consumption interface ready.
        // Actual policy fetching would go here.

        Ok(Vec::new())
    }

    #[instrument(skip(self, report), fields(request_id = %report.request_id))]
    async fn report_enforcement(&self, report: EnforcementReport) -> IntegrationResult<()> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("policy-engine".to_string()));
        }

        debug!(
            request_id = %report.request_id,
            outcome = ?report.outcome,
            decisions = report.decisions.len(),
            "Reporting enforcement to policy engine"
        );

        // Phase 2B: Enforcement reporting interface ready.
        // Actual report submission would go here.

        Ok(())
    }
}

impl std::fmt::Debug for PolicyEngineAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicyEngineAdapter")
            .field("enabled", &self.config.enabled)
            .field("pre_request_check", &self.config.pre_request_check)
            .field("post_response_check", &self.config.post_response_check)
            .field("block_on_violation", &self.config.block_on_violation)
            .field("cached_policies", &self.policy_cache.len())
            .field("cached_decisions", &self.decision_cache.len())
            .finish()
    }
}

/// Builder for `PolicyEngineAdapter`
pub struct PolicyEngineAdapterBuilder {
    config: PolicyEngineConfig,
}

impl PolicyEngineAdapterBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: PolicyEngineConfig::default(),
        }
    }

    /// Set the configuration.
    pub fn config(mut self, config: PolicyEngineConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable the adapter.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Enable/disable pre-request check.
    pub fn pre_request_check(mut self, enabled: bool) -> Self {
        self.config.pre_request_check = enabled;
        self
    }

    /// Enable/disable post-response check.
    pub fn post_response_check(mut self, enabled: bool) -> Self {
        self.config.post_response_check = enabled;
        self
    }

    /// Enable/disable blocking on violation.
    pub fn block_on_violation(mut self, enabled: bool) -> Self {
        self.config.block_on_violation = enabled;
        self
    }

    /// Enable/disable decision caching.
    pub fn cache_decisions(mut self, enabled: bool) -> Self {
        self.config.cache_decisions = enabled;
        self
    }

    /// Set cache TTL.
    pub fn cache_ttl(mut self, ttl: Duration) -> Self {
        self.config.cache_ttl = ttl;
        self
    }

    /// Build the adapter.
    pub fn build(self) -> PolicyEngineAdapter {
        PolicyEngineAdapter::new(self.config)
    }
}

impl Default for PolicyEngineAdapterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_disabled_by_default() {
        let adapter = PolicyEngineAdapter::new(PolicyEngineConfig::default());
        assert!(!adapter.is_enabled());
    }

    #[test]
    fn test_adapter_builder() {
        let adapter = PolicyEngineAdapterBuilder::new()
            .enabled(true)
            .pre_request_check(true)
            .post_response_check(false)
            .block_on_violation(true)
            .build();

        assert!(adapter.is_enabled());
        assert!(adapter.pre_request_enabled());
        assert!(!adapter.post_response_enabled());
        assert!(adapter.blocks_on_violation());
    }

    #[test]
    fn test_cache_key_generation() {
        let request = gateway_core::GatewayRequest::builder()
            .model("gpt-4")
            .message(gateway_core::ChatMessage::user("test"))
            .build()
            .unwrap();

        let key = PolicyEngineAdapter::request_cache_key(&request);
        assert!(key.starts_with("req:"));
    }

    #[tokio::test]
    async fn test_disabled_returns_not_enabled() {
        let adapter = PolicyEngineAdapter::new(PolicyEngineConfig::default());
        let request = gateway_core::GatewayRequest::builder()
            .model("gpt-4")
            .message(gateway_core::ChatMessage::user("test"))
            .build()
            .unwrap();

        let result = adapter.evaluate_request(&request).await;
        assert!(matches!(result, Err(IntegrationError::NotEnabled(_))));
    }
}
