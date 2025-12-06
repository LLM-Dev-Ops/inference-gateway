//! Router adapter for consuming routing rules.
//!
//! This adapter consumes routing rules and decision graphs
//! from external sources (Layer 2 module integration).

use crate::config::RouterConfig;
use crate::error::{IntegrationError, IntegrationResult};
use async_trait::async_trait;
use dashmap::DashMap;
use gateway_core::GatewayRequest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, instrument, warn};

/// Adapter for consuming routing rules (Layer 2 module).
///
/// This adapter integrates with the existing gateway-routing crate
/// by consuming external routing rules and decision graphs.
pub struct RouterAdapter {
    /// Configuration
    config: RouterConfig,
    /// Cached routing rules
    rules_cache: DashMap<String, CachedRule>,
    /// Cached decision graphs
    graphs_cache: DashMap<String, CachedGraph>,
    /// Last rule refresh timestamp
    last_refresh: std::sync::atomic::AtomicU64,
}

/// Cached routing rule
struct CachedRule {
    rule: RoutingRule,
    cached_at: Instant,
}

/// Cached decision graph
struct CachedGraph {
    graph: DecisionGraph,
    cached_at: Instant,
}

/// Routing rule consumed from external source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    /// Rule ID
    pub id: String,
    /// Rule name
    pub name: String,
    /// Rule priority (higher = evaluated first)
    pub priority: u32,
    /// Conditions for matching
    pub conditions: Vec<RuleCondition>,
    /// Action to take on match
    pub action: RuleAction,
    /// Is the rule enabled
    pub enabled: bool,
}

/// Rule condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCondition {
    /// Field to match on
    pub field: String,
    /// Operator
    pub operator: ConditionOperator,
    /// Value to compare
    pub value: serde_json::Value,
}

/// Condition operator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionOperator {
    /// Equals
    Eq,
    /// Not equals
    Ne,
    /// Contains
    Contains,
    /// Starts with
    StartsWith,
    /// Ends with
    EndsWith,
    /// Greater than
    Gt,
    /// Less than
    Lt,
    /// In list
    In,
    /// Regex match
    Regex,
}

/// Rule action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleAction {
    /// Action type
    pub action_type: RuleActionType,
    /// Target provider (for route actions)
    pub target_provider: Option<String>,
    /// Additional parameters
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Rule action type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleActionType {
    /// Route to specific provider
    Route,
    /// Block the request
    Block,
    /// Allow with modification
    Modify,
    /// Load balance across providers
    LoadBalance,
    /// Fallback to next rule
    Fallback,
}

/// Decision graph for complex routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionGraph {
    /// Graph ID
    pub id: String,
    /// Graph name
    pub name: String,
    /// Root node ID
    pub root_node: String,
    /// Nodes in the graph
    pub nodes: HashMap<String, DecisionNode>,
}

/// Decision graph node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionNode {
    /// Node ID
    pub id: String,
    /// Node type
    pub node_type: NodeType,
    /// Condition (for decision nodes)
    pub condition: Option<RuleCondition>,
    /// True branch (for decision nodes)
    pub true_branch: Option<String>,
    /// False branch (for decision nodes)
    pub false_branch: Option<String>,
    /// Action (for action nodes)
    pub action: Option<RuleAction>,
}

/// Node type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    /// Decision node (has condition and branches)
    Decision,
    /// Action node (terminal, has action)
    Action,
}

/// Trait for consuming routing rules and decision graphs
#[async_trait]
pub trait RoutingRuleConsumer: Send + Sync {
    /// Consume routing rules from external source.
    async fn consume_rules(&self) -> IntegrationResult<Vec<RoutingRule>>;

    /// Consume decision graphs from external source.
    async fn consume_decision_graphs(&self) -> IntegrationResult<Vec<DecisionGraph>>;

    /// Evaluate a request against routing rules.
    async fn evaluate_request(
        &self,
        request: &GatewayRequest,
    ) -> IntegrationResult<RoutingDecision>;
}

/// Routing decision result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    /// Matched rule ID (if any)
    pub matched_rule: Option<String>,
    /// Selected provider
    pub provider_id: Option<String>,
    /// Action to take
    pub action: RuleActionType,
    /// Confidence score
    pub confidence: f32,
    /// Decision reason
    pub reason: String,
}

impl RouterAdapter {
    /// Create a new router adapter.
    pub fn new(config: RouterConfig) -> Self {
        Self {
            config,
            rules_cache: DashMap::new(),
            graphs_cache: DashMap::new(),
            last_refresh: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Check if the adapter is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if rule consumption is enabled.
    pub fn consumes_rules(&self) -> bool {
        self.config.consume_rules
    }

    /// Check if cache needs refresh.
    fn needs_refresh(&self) -> bool {
        let last = self.last_refresh.load(std::sync::atomic::Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        now - last > self.config.rule_refresh_interval.as_secs()
    }

    /// Update refresh timestamp.
    fn update_refresh_timestamp(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.last_refresh.store(now, std::sync::atomic::Ordering::Relaxed);
    }
}

#[async_trait]
impl RoutingRuleConsumer for RouterAdapter {
    #[instrument(skip(self))]
    async fn consume_rules(&self) -> IntegrationResult<Vec<RoutingRule>> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("router".to_string()));
        }

        if !self.config.consume_rules {
            debug!("Rule consumption is disabled");
            return Ok(Vec::new());
        }

        debug!("Consuming routing rules from external source");

        // Return cached rules if fresh
        if !self.needs_refresh() {
            let rules: Vec<RoutingRule> = self
                .rules_cache
                .iter()
                .map(|entry| entry.value().rule.clone())
                .collect();

            if !rules.is_empty() {
                return Ok(rules);
            }
        }

        // Phase 2B: Rule consumption interface ready.
        // Actual rule fetching would go here.
        self.update_refresh_timestamp();

        Ok(Vec::new())
    }

    #[instrument(skip(self))]
    async fn consume_decision_graphs(&self) -> IntegrationResult<Vec<DecisionGraph>> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("router".to_string()));
        }

        if !self.config.consume_decision_graphs {
            debug!("Decision graph consumption is disabled");
            return Ok(Vec::new());
        }

        debug!("Consuming decision graphs from external source");

        // Return cached graphs if fresh
        if !self.needs_refresh() {
            let graphs: Vec<DecisionGraph> = self
                .graphs_cache
                .iter()
                .map(|entry| entry.value().graph.clone())
                .collect();

            if !graphs.is_empty() {
                return Ok(graphs);
            }
        }

        // Phase 2B: Decision graph consumption interface ready.
        // Actual graph fetching would go here.

        Ok(Vec::new())
    }

    #[instrument(skip(self, request), fields(model = %request.model))]
    async fn evaluate_request(
        &self,
        request: &GatewayRequest,
    ) -> IntegrationResult<RoutingDecision> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("router".to_string()));
        }

        debug!(model = %request.model, "Evaluating request against routing rules");

        // Phase 2B: Request evaluation interface ready.
        // Actual rule evaluation would go here.

        Ok(RoutingDecision {
            matched_rule: None,
            provider_id: request
                .metadata
                .as_ref()
                .and_then(|m| m.preferred_provider.clone()),
            action: RuleActionType::Fallback,
            confidence: 0.0,
            reason: "No external routing rules configured".to_string(),
        })
    }
}

impl std::fmt::Debug for RouterAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouterAdapter")
            .field("enabled", &self.config.enabled)
            .field("consume_rules", &self.config.consume_rules)
            .field("cached_rules", &self.rules_cache.len())
            .field("cached_graphs", &self.graphs_cache.len())
            .finish()
    }
}

/// Builder for `RouterAdapter`
pub struct RouterAdapterBuilder {
    config: RouterConfig,
}

impl RouterAdapterBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: RouterConfig::default(),
        }
    }

    /// Set the configuration.
    pub fn config(mut self, config: RouterConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable the adapter.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Enable/disable rule consumption.
    pub fn consume_rules(mut self, enabled: bool) -> Self {
        self.config.consume_rules = enabled;
        self
    }

    /// Enable/disable decision graph consumption.
    pub fn consume_decision_graphs(mut self, enabled: bool) -> Self {
        self.config.consume_decision_graphs = enabled;
        self
    }

    /// Build the adapter.
    pub fn build(self) -> RouterAdapter {
        RouterAdapter::new(self.config)
    }
}

impl Default for RouterAdapterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_disabled_by_default() {
        let adapter = RouterAdapter::new(RouterConfig::default());
        assert!(!adapter.is_enabled());
    }

    #[test]
    fn test_adapter_builder() {
        let adapter = RouterAdapterBuilder::new()
            .enabled(true)
            .consume_rules(true)
            .consume_decision_graphs(false)
            .build();

        assert!(adapter.is_enabled());
        assert!(adapter.consumes_rules());
    }

    #[tokio::test]
    async fn test_disabled_returns_not_enabled() {
        let adapter = RouterAdapter::new(RouterConfig::default());
        let result = adapter.consume_rules().await;
        assert!(matches!(result, Err(IntegrationError::NotEnabled(_))));
    }
}
