//! Cost tracking and billing for LLM usage.
//!
//! Provides comprehensive cost tracking including:
//! - Token usage by tenant, model, and provider
//! - Cost calculation based on model pricing
//! - Budget management and alerts
//! - Usage reports and aggregation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Cost tracking configuration
#[derive(Debug, Clone)]
pub struct CostConfig {
    /// Enable cost tracking
    pub enabled: bool,
    /// Default cost per 1K input tokens (USD)
    pub default_input_cost_per_1k: f64,
    /// Default cost per 1K output tokens (USD)
    pub default_output_cost_per_1k: f64,
    /// Maximum events to keep in memory
    pub max_events: usize,
    /// Aggregation interval
    pub aggregation_interval: Duration,
}

impl Default for CostConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_input_cost_per_1k: 0.01,
            default_output_cost_per_1k: 0.03,
            max_events: 10_000,
            aggregation_interval: Duration::from_secs(60),
        }
    }
}

impl CostConfig {
    /// Create a new cost configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable or disable cost tracking
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set default pricing
    #[must_use]
    pub fn with_default_pricing(mut self, input_per_1k: f64, output_per_1k: f64) -> Self {
        self.default_input_cost_per_1k = input_per_1k;
        self.default_output_cost_per_1k = output_per_1k;
        self
    }
}

/// Model pricing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Model ID
    pub model: String,
    /// Provider ID
    pub provider: String,
    /// Cost per 1K input tokens (USD)
    pub input_cost_per_1k: f64,
    /// Cost per 1K output tokens (USD)
    pub output_cost_per_1k: f64,
    /// Currency (default: USD)
    pub currency: String,
}

impl ModelPricing {
    /// Create new model pricing
    #[must_use]
    pub fn new(model: impl Into<String>, provider: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            provider: provider.into(),
            input_cost_per_1k: 0.01,
            output_cost_per_1k: 0.03,
            currency: "USD".to_string(),
        }
    }

    /// Set pricing rates
    #[must_use]
    pub fn with_pricing(mut self, input_per_1k: f64, output_per_1k: f64) -> Self {
        self.input_cost_per_1k = input_per_1k;
        self.output_cost_per_1k = output_per_1k;
        self
    }

    /// Calculate cost for given token counts
    #[must_use]
    pub fn calculate_cost(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        let input_cost = (f64::from(input_tokens) / 1000.0) * self.input_cost_per_1k;
        let output_cost = (f64::from(output_tokens) / 1000.0) * self.output_cost_per_1k;
        input_cost + output_cost
    }
}

/// Usage event for cost tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEvent {
    /// Event timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Request ID
    pub request_id: String,
    /// Tenant ID
    pub tenant_id: Option<String>,
    /// Model used
    pub model: String,
    /// Provider used
    pub provider: String,
    /// Input tokens consumed
    pub input_tokens: u32,
    /// Output tokens produced
    pub output_tokens: u32,
    /// Total tokens
    pub total_tokens: u32,
    /// Calculated cost (USD)
    pub cost: f64,
    /// Whether the request was successful
    pub success: bool,
    /// Request latency
    pub latency_ms: u64,
}

impl UsageEvent {
    /// Create a new usage event
    #[must_use]
    pub fn new(
        request_id: impl Into<String>,
        model: impl Into<String>,
        provider: impl Into<String>,
        input_tokens: u32,
        output_tokens: u32,
        cost: f64,
    ) -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            request_id: request_id.into(),
            tenant_id: None,
            model: model.into(),
            provider: provider.into(),
            input_tokens,
            output_tokens,
            total_tokens: input_tokens + output_tokens,
            cost,
            success: true,
            latency_ms: 0,
        }
    }

    /// Set tenant ID
    #[must_use]
    pub fn with_tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    /// Set latency
    #[must_use]
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency_ms = latency.as_millis() as u64;
        self
    }

    /// Set success status
    #[must_use]
    pub fn with_success(mut self, success: bool) -> Self {
        self.success = success;
        self
    }
}

/// Aggregated usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageStats {
    /// Total requests
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Total input tokens
    pub total_input_tokens: u64,
    /// Total output tokens
    pub total_output_tokens: u64,
    /// Total cost (USD)
    pub total_cost: f64,
    /// Average latency (ms)
    pub avg_latency_ms: f64,
    /// Time period start
    pub period_start: Option<chrono::DateTime<chrono::Utc>>,
    /// Time period end
    pub period_end: Option<chrono::DateTime<chrono::Utc>>,
}

impl UsageStats {
    /// Add a usage event to the stats
    pub fn add_event(&mut self, event: &UsageEvent) {
        self.total_requests += 1;
        if event.success {
            self.successful_requests += 1;
        } else {
            self.failed_requests += 1;
        }
        self.total_input_tokens += u64::from(event.input_tokens);
        self.total_output_tokens += u64::from(event.output_tokens);
        self.total_cost += event.cost;

        // Update average latency
        let total = self.total_requests;
        self.avg_latency_ms =
            (self.avg_latency_ms * (total - 1) as f64 + event.latency_ms as f64) / total as f64;

        // Update time bounds
        if self.period_start.is_none() || event.timestamp < self.period_start.unwrap() {
            self.period_start = Some(event.timestamp);
        }
        if self.period_end.is_none() || event.timestamp > self.period_end.unwrap() {
            self.period_end = Some(event.timestamp);
        }
    }

    /// Merge two stats
    #[must_use]
    pub fn merge(mut self, other: &UsageStats) -> Self {
        self.total_requests += other.total_requests;
        self.successful_requests += other.successful_requests;
        self.failed_requests += other.failed_requests;
        self.total_input_tokens += other.total_input_tokens;
        self.total_output_tokens += other.total_output_tokens;
        self.total_cost += other.total_cost;

        if other.total_requests > 0 {
            let total = self.total_requests as f64;
            let prev_weight = (self.total_requests - other.total_requests) as f64 / total;
            let other_weight = other.total_requests as f64 / total;
            self.avg_latency_ms = self.avg_latency_ms * prev_weight + other.avg_latency_ms * other_weight;
        }

        if let Some(other_start) = other.period_start {
            if self.period_start.is_none() || other_start < self.period_start.unwrap() {
                self.period_start = Some(other_start);
            }
        }
        if let Some(other_end) = other.period_end {
            if self.period_end.is_none() || other_end > self.period_end.unwrap() {
                self.period_end = Some(other_end);
            }
        }

        self
    }
}

/// Budget configuration for a tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    /// Tenant ID
    pub tenant_id: String,
    /// Monthly budget limit (USD)
    pub monthly_limit: f64,
    /// Daily budget limit (USD)
    pub daily_limit: Option<f64>,
    /// Warning threshold (percentage of limit)
    pub warning_threshold: f64,
    /// Hard limit enforcement
    pub enforce_limit: bool,
}

impl Budget {
    /// Create a new budget
    #[must_use]
    pub fn new(tenant_id: impl Into<String>, monthly_limit: f64) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            monthly_limit,
            daily_limit: None,
            warning_threshold: 0.8, // 80% warning
            enforce_limit: false,
        }
    }

    /// Set daily limit
    #[must_use]
    pub fn with_daily_limit(mut self, limit: f64) -> Self {
        self.daily_limit = Some(limit);
        self
    }

    /// Set warning threshold
    #[must_use]
    pub fn with_warning_threshold(mut self, threshold: f64) -> Self {
        self.warning_threshold = threshold;
        self
    }

    /// Enable hard limit enforcement
    #[must_use]
    pub fn with_enforcement(mut self) -> Self {
        self.enforce_limit = true;
        self
    }
}

/// Budget status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    /// Tenant ID
    pub tenant_id: String,
    /// Current month spend
    pub current_spend: f64,
    /// Monthly limit
    pub monthly_limit: f64,
    /// Remaining budget
    pub remaining: f64,
    /// Usage percentage
    pub usage_percentage: f64,
    /// Whether warning threshold is exceeded
    pub warning_exceeded: bool,
    /// Whether limit is exceeded
    pub limit_exceeded: bool,
}

/// Cost tracker for recording and managing usage costs
pub struct CostTracker {
    config: CostConfig,
    /// Model pricing registry
    pricing: RwLock<HashMap<String, ModelPricing>>,
    /// Usage events buffer
    events: RwLock<Vec<UsageEvent>>,
    /// Per-tenant aggregated stats
    tenant_stats: RwLock<HashMap<String, UsageStats>>,
    /// Per-model aggregated stats
    model_stats: RwLock<HashMap<String, UsageStats>>,
    /// Per-provider aggregated stats
    provider_stats: RwLock<HashMap<String, UsageStats>>,
    /// Global stats
    global_stats: RwLock<UsageStats>,
    /// Tenant budgets
    budgets: RwLock<HashMap<String, Budget>>,
    /// Current tenant spend (for budget tracking)
    tenant_spend: RwLock<HashMap<String, f64>>,
    /// Total tracked cost (atomic for fast access)
    total_cost: AtomicU64,
    /// Total tracked tokens
    total_tokens: AtomicU64,
}

impl CostTracker {
    /// Create a new cost tracker
    #[must_use]
    pub fn new(config: CostConfig) -> Self {
        Self {
            config,
            pricing: RwLock::new(HashMap::new()),
            events: RwLock::new(Vec::new()),
            tenant_stats: RwLock::new(HashMap::new()),
            model_stats: RwLock::new(HashMap::new()),
            provider_stats: RwLock::new(HashMap::new()),
            global_stats: RwLock::new(UsageStats::default()),
            budgets: RwLock::new(HashMap::new()),
            tenant_spend: RwLock::new(HashMap::new()),
            total_cost: AtomicU64::new(0),
            total_tokens: AtomicU64::new(0),
        }
    }

    /// Create with default configuration
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(CostConfig::default())
    }

    /// Create a disabled tracker
    #[must_use]
    pub fn disabled() -> Self {
        Self::new(CostConfig {
            enabled: false,
            ..Default::default()
        })
    }

    /// Check if tracking is enabled
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Register model pricing
    pub async fn register_pricing(&self, pricing: ModelPricing) {
        let key = format!("{}:{}", pricing.provider, pricing.model);
        let mut registry = self.pricing.write().await;
        registry.insert(key, pricing);
    }

    /// Get pricing for a model
    pub async fn get_pricing(&self, model: &str, provider: &str) -> Option<ModelPricing> {
        let key = format!("{provider}:{model}");
        let registry = self.pricing.read().await;
        registry.get(&key).cloned()
    }

    /// Calculate cost for a request
    pub async fn calculate_cost(
        &self,
        model: &str,
        provider: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> f64 {
        let pricing = self.get_pricing(model, provider).await;

        match pricing {
            Some(p) => p.calculate_cost(input_tokens, output_tokens),
            None => {
                // Use default pricing
                let input_cost =
                    (f64::from(input_tokens) / 1000.0) * self.config.default_input_cost_per_1k;
                let output_cost =
                    (f64::from(output_tokens) / 1000.0) * self.config.default_output_cost_per_1k;
                input_cost + output_cost
            }
        }
    }

    /// Record a usage event
    pub async fn record_usage(&self, event: UsageEvent) {
        if !self.config.enabled {
            return;
        }

        debug!(
            request_id = %event.request_id,
            model = %event.model,
            tokens = event.total_tokens,
            cost = event.cost,
            "Recording usage event"
        );

        // Update atomic counters
        #[allow(clippy::cast_possible_truncation)]
        let cost_bits = (event.cost * 1_000_000.0) as u64; // Store as micro-dollars
        self.total_cost.fetch_add(cost_bits, Ordering::SeqCst);
        self.total_tokens
            .fetch_add(u64::from(event.total_tokens), Ordering::SeqCst);

        // Update tenant spend
        if let Some(ref tenant_id) = event.tenant_id {
            let mut spend = self.tenant_spend.write().await;
            *spend.entry(tenant_id.clone()).or_insert(0.0) += event.cost;
        }

        // Update stats
        {
            let mut global = self.global_stats.write().await;
            global.add_event(&event);
        }

        {
            let mut tenant_stats = self.tenant_stats.write().await;
            if let Some(ref tenant_id) = event.tenant_id {
                tenant_stats
                    .entry(tenant_id.clone())
                    .or_default()
                    .add_event(&event);
            }
        }

        {
            let mut model_stats = self.model_stats.write().await;
            model_stats
                .entry(event.model.clone())
                .or_default()
                .add_event(&event);
        }

        {
            let mut provider_stats = self.provider_stats.write().await;
            provider_stats
                .entry(event.provider.clone())
                .or_default()
                .add_event(&event);
        }

        // Store event (with eviction)
        {
            let mut events = self.events.write().await;
            if events.len() >= self.config.max_events {
                events.remove(0);
            }
            events.push(event);
        }
    }

    /// Record usage with automatic cost calculation
    pub async fn record(
        &self,
        request_id: impl Into<String>,
        tenant_id: Option<String>,
        model: impl Into<String>,
        provider: impl Into<String>,
        input_tokens: u32,
        output_tokens: u32,
        latency: Duration,
        success: bool,
    ) {
        let model = model.into();
        let provider = provider.into();
        let cost = self.calculate_cost(&model, &provider, input_tokens, output_tokens).await;

        let mut event = UsageEvent::new(
            request_id,
            &model,
            &provider,
            input_tokens,
            output_tokens,
            cost,
        )
        .with_latency(latency)
        .with_success(success);

        if let Some(tenant) = tenant_id {
            event = event.with_tenant(tenant);
        }

        self.record_usage(event).await;
    }

    /// Set a budget for a tenant
    pub async fn set_budget(&self, budget: Budget) {
        let tenant_id = budget.tenant_id.clone();
        let mut budgets = self.budgets.write().await;
        budgets.insert(tenant_id, budget);
    }

    /// Check budget status for a tenant
    pub async fn check_budget(&self, tenant_id: &str) -> Option<BudgetStatus> {
        let budgets = self.budgets.read().await;
        let budget = budgets.get(tenant_id)?;

        let spend = self.tenant_spend.read().await;
        let current_spend = *spend.get(tenant_id).unwrap_or(&0.0);

        let remaining = (budget.monthly_limit - current_spend).max(0.0);
        let usage_percentage = current_spend / budget.monthly_limit;
        let warning_exceeded = usage_percentage >= budget.warning_threshold;
        let limit_exceeded = current_spend >= budget.monthly_limit;

        if warning_exceeded && !limit_exceeded {
            warn!(
                tenant_id = %tenant_id,
                usage_percentage = usage_percentage * 100.0,
                "Tenant approaching budget limit"
            );
        }

        if limit_exceeded {
            warn!(
                tenant_id = %tenant_id,
                current_spend = current_spend,
                limit = budget.monthly_limit,
                "Tenant has exceeded budget limit"
            );
        }

        Some(BudgetStatus {
            tenant_id: tenant_id.to_string(),
            current_spend,
            monthly_limit: budget.monthly_limit,
            remaining,
            usage_percentage,
            warning_exceeded,
            limit_exceeded,
        })
    }

    /// Check if a tenant is over budget (with enforcement)
    pub async fn is_over_budget(&self, tenant_id: &str) -> bool {
        let budgets = self.budgets.read().await;
        let Some(budget) = budgets.get(tenant_id) else {
            return false;
        };

        if !budget.enforce_limit {
            return false;
        }

        let spend = self.tenant_spend.read().await;
        let current_spend = *spend.get(tenant_id).unwrap_or(&0.0);

        current_spend >= budget.monthly_limit
    }

    /// Get total cost tracked
    #[must_use]
    pub fn total_cost(&self) -> f64 {
        let bits = self.total_cost.load(Ordering::SeqCst);
        bits as f64 / 1_000_000.0 // Convert from micro-dollars
    }

    /// Get total tokens tracked
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        self.total_tokens.load(Ordering::SeqCst)
    }

    /// Get global usage stats
    pub async fn global_stats(&self) -> UsageStats {
        self.global_stats.read().await.clone()
    }

    /// Get stats for a specific tenant
    pub async fn tenant_stats(&self, tenant_id: &str) -> Option<UsageStats> {
        let stats = self.tenant_stats.read().await;
        stats.get(tenant_id).cloned()
    }

    /// Get stats for a specific model
    pub async fn model_stats(&self, model: &str) -> Option<UsageStats> {
        let stats = self.model_stats.read().await;
        stats.get(model).cloned()
    }

    /// Get stats for a specific provider
    pub async fn provider_stats(&self, provider: &str) -> Option<UsageStats> {
        let stats = self.provider_stats.read().await;
        stats.get(provider).cloned()
    }

    /// Get all tenant stats
    pub async fn all_tenant_stats(&self) -> HashMap<String, UsageStats> {
        self.tenant_stats.read().await.clone()
    }

    /// Get all model stats
    pub async fn all_model_stats(&self) -> HashMap<String, UsageStats> {
        self.model_stats.read().await.clone()
    }

    /// Get recent usage events
    pub async fn recent_events(&self, limit: usize) -> Vec<UsageEvent> {
        let events = self.events.read().await;
        events.iter().rev().take(limit).cloned().collect()
    }

    /// Clear all tracked data
    pub async fn clear(&self) {
        {
            let mut events = self.events.write().await;
            events.clear();
        }
        {
            let mut stats = self.tenant_stats.write().await;
            stats.clear();
        }
        {
            let mut stats = self.model_stats.write().await;
            stats.clear();
        }
        {
            let mut stats = self.provider_stats.write().await;
            stats.clear();
        }
        {
            let mut stats = self.global_stats.write().await;
            *stats = UsageStats::default();
        }
        {
            let mut spend = self.tenant_spend.write().await;
            spend.clear();
        }
        self.total_cost.store(0, Ordering::SeqCst);
        self.total_tokens.store(0, Ordering::SeqCst);

        info!("Cost tracking data cleared");
    }

    /// Reset monthly spend (for billing cycle reset)
    pub async fn reset_monthly_spend(&self) {
        let mut spend = self.tenant_spend.write().await;
        spend.clear();
        info!("Monthly spend counters reset");
    }
}

/// Cost report summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostReport {
    /// Report title
    pub title: String,
    /// Report period
    pub period: String,
    /// Global stats
    pub global: UsageStats,
    /// Per-tenant breakdown
    pub by_tenant: HashMap<String, UsageStats>,
    /// Per-model breakdown
    pub by_model: HashMap<String, UsageStats>,
    /// Per-provider breakdown
    pub by_provider: HashMap<String, UsageStats>,
    /// Generated timestamp
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

impl CostReport {
    /// Generate a cost report from the tracker
    pub async fn generate(tracker: &CostTracker, title: impl Into<String>, period: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            period: period.into(),
            global: tracker.global_stats().await,
            by_tenant: tracker.all_tenant_stats().await,
            by_model: tracker.all_model_stats().await,
            by_provider: tracker.provider_stats.read().await.clone(),
            generated_at: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_pricing() {
        let pricing = ModelPricing::new("gpt-4", "openai").with_pricing(0.03, 0.06);

        let cost = pricing.calculate_cost(1000, 500);
        assert!((cost - 0.06).abs() < 0.001); // 0.03 + 0.03 = 0.06
    }

    #[test]
    fn test_usage_event() {
        let event = UsageEvent::new("req-1", "gpt-4", "openai", 100, 50, 0.005)
            .with_tenant("tenant-1")
            .with_latency(Duration::from_millis(500))
            .with_success(true);

        assert_eq!(event.request_id, "req-1");
        assert_eq!(event.tenant_id, Some("tenant-1".to_string()));
        assert_eq!(event.total_tokens, 150);
        assert_eq!(event.latency_ms, 500);
    }

    #[test]
    fn test_usage_stats_add_event() {
        let mut stats = UsageStats::default();

        let event1 = UsageEvent::new("req-1", "gpt-4", "openai", 100, 50, 0.005)
            .with_latency(Duration::from_millis(100))
            .with_success(true);

        let event2 = UsageEvent::new("req-2", "gpt-4", "openai", 200, 100, 0.01)
            .with_latency(Duration::from_millis(200))
            .with_success(true);

        stats.add_event(&event1);
        stats.add_event(&event2);

        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.successful_requests, 2);
        assert_eq!(stats.total_input_tokens, 300);
        assert_eq!(stats.total_output_tokens, 150);
        assert!((stats.total_cost - 0.015).abs() < 0.001);
        assert!((stats.avg_latency_ms - 150.0).abs() < 0.1);
    }

    #[test]
    fn test_budget() {
        let budget = Budget::new("tenant-1", 100.0)
            .with_daily_limit(10.0)
            .with_warning_threshold(0.75)
            .with_enforcement();

        assert_eq!(budget.monthly_limit, 100.0);
        assert_eq!(budget.daily_limit, Some(10.0));
        assert_eq!(budget.warning_threshold, 0.75);
        assert!(budget.enforce_limit);
    }

    #[tokio::test]
    async fn test_cost_tracker_creation() {
        let tracker = CostTracker::with_defaults();
        assert!(tracker.is_enabled());
        assert_eq!(tracker.total_cost(), 0.0);
        assert_eq!(tracker.total_tokens(), 0);
    }

    #[tokio::test]
    async fn test_cost_tracker_disabled() {
        let tracker = CostTracker::disabled();
        assert!(!tracker.is_enabled());
    }

    #[tokio::test]
    async fn test_cost_tracker_record_usage() {
        let tracker = CostTracker::with_defaults();

        let event = UsageEvent::new("req-1", "gpt-4", "openai", 100, 50, 0.005)
            .with_tenant("tenant-1")
            .with_success(true);

        tracker.record_usage(event).await;

        assert!(tracker.total_cost() > 0.0);
        assert_eq!(tracker.total_tokens(), 150);

        let global = tracker.global_stats().await;
        assert_eq!(global.total_requests, 1);

        let tenant_stats = tracker.tenant_stats("tenant-1").await;
        assert!(tenant_stats.is_some());
        assert_eq!(tenant_stats.unwrap().total_requests, 1);
    }

    #[tokio::test]
    async fn test_cost_tracker_pricing() {
        let tracker = CostTracker::with_defaults();

        let pricing = ModelPricing::new("gpt-4", "openai").with_pricing(0.03, 0.06);
        tracker.register_pricing(pricing).await;

        let cost = tracker.calculate_cost("gpt-4", "openai", 1000, 500).await;
        assert!((cost - 0.06).abs() < 0.001);

        // Unknown model uses default pricing
        let default_cost = tracker.calculate_cost("unknown", "unknown", 1000, 1000).await;
        assert!(default_cost > 0.0);
    }

    #[tokio::test]
    async fn test_cost_tracker_budget() {
        let tracker = CostTracker::with_defaults();

        let budget = Budget::new("tenant-1", 100.0)
            .with_warning_threshold(0.8)
            .with_enforcement();
        tracker.set_budget(budget).await;

        // Record some usage
        let event = UsageEvent::new("req-1", "gpt-4", "openai", 1000, 500, 50.0)
            .with_tenant("tenant-1");
        tracker.record_usage(event).await;

        let status = tracker.check_budget("tenant-1").await;
        assert!(status.is_some());
        let status = status.unwrap();
        assert_eq!(status.current_spend, 50.0);
        assert_eq!(status.remaining, 50.0);
        assert!(!status.limit_exceeded);

        // Not over budget yet
        assert!(!tracker.is_over_budget("tenant-1").await);

        // Record more to exceed budget
        let event = UsageEvent::new("req-2", "gpt-4", "openai", 1000, 500, 60.0)
            .with_tenant("tenant-1");
        tracker.record_usage(event).await;

        assert!(tracker.is_over_budget("tenant-1").await);
    }

    #[tokio::test]
    async fn test_cost_tracker_record_helper() {
        let tracker = CostTracker::with_defaults();

        tracker
            .record(
                "req-1",
                Some("tenant-1".to_string()),
                "gpt-4",
                "openai",
                100,
                50,
                Duration::from_millis(500),
                true,
            )
            .await;

        let stats = tracker.global_stats().await;
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.total_input_tokens, 100);
        assert_eq!(stats.total_output_tokens, 50);
    }

    #[tokio::test]
    async fn test_cost_tracker_clear() {
        let tracker = CostTracker::with_defaults();

        let event = UsageEvent::new("req-1", "gpt-4", "openai", 100, 50, 0.005);
        tracker.record_usage(event).await;

        assert!(tracker.total_tokens() > 0);

        tracker.clear().await;

        assert_eq!(tracker.total_tokens(), 0);
        assert_eq!(tracker.total_cost(), 0.0);
    }

    #[tokio::test]
    async fn test_cost_report_generation() {
        let tracker = CostTracker::with_defaults();

        let event = UsageEvent::new("req-1", "gpt-4", "openai", 100, 50, 0.005)
            .with_tenant("tenant-1");
        tracker.record_usage(event).await;

        let report = CostReport::generate(&tracker, "Monthly Report", "November 2024").await;

        assert_eq!(report.title, "Monthly Report");
        assert_eq!(report.period, "November 2024");
        assert_eq!(report.global.total_requests, 1);
        assert!(report.by_tenant.contains_key("tenant-1"));
        assert!(report.by_model.contains_key("gpt-4"));
    }

    #[test]
    fn test_cost_config() {
        let config = CostConfig::new()
            .with_enabled(true)
            .with_default_pricing(0.02, 0.04);

        assert!(config.enabled);
        assert_eq!(config.default_input_cost_per_1k, 0.02);
        assert_eq!(config.default_output_cost_per_1k, 0.04);
    }
}
