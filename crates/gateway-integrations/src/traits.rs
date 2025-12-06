//! Trait definitions for integration adapters.
//!
//! These traits define the consume-from interfaces that adapters implement
//! to integrate with LLM-Dev-Ops ecosystem services.

use crate::error::IntegrationResult;
use async_trait::async_trait;
use gateway_core::{GatewayRequest, GatewayResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

// ============================================================================
// Provider Routing (LLM-Connector-Hub)
// ============================================================================

/// Trait for consuming provider routing information from LLM-Connector-Hub.
///
/// This adapter routes requests to different model providers based on
/// capabilities, availability, and configuration from the connector hub.
#[async_trait]
pub trait ProviderRouter: Send + Sync {
    /// Get the recommended provider for a request.
    async fn get_provider_recommendation(
        &self,
        request: &GatewayRequest,
    ) -> IntegrationResult<ProviderRecommendation>;

    /// Consume provider discovery updates.
    async fn consume_provider_updates(&self) -> IntegrationResult<Vec<ProviderInfo>>;

    /// Get credentials for a provider.
    async fn get_provider_credentials(
        &self,
        provider_id: &str,
    ) -> IntegrationResult<ProviderCredentials>;

    /// Report provider health status.
    async fn report_provider_health(
        &self,
        provider_id: &str,
        status: ProviderHealthReport,
    ) -> IntegrationResult<()>;
}

/// Provider recommendation from connector hub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRecommendation {
    /// Recommended provider ID
    pub provider_id: String,
    /// Confidence score (0.0-1.0)
    pub confidence: f32,
    /// Fallback providers in order of preference
    pub fallbacks: Vec<String>,
    /// Reason for recommendation
    pub reason: String,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Provider information from discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// Provider ID
    pub id: String,
    /// Provider name
    pub name: String,
    /// Provider endpoint
    pub endpoint: String,
    /// Supported models
    pub models: Vec<String>,
    /// Provider capabilities
    pub capabilities: Vec<String>,
    /// Health status
    pub healthy: bool,
}

/// Provider credentials
#[derive(Debug, Clone)]
pub struct ProviderCredentials {
    /// API key or token
    pub api_key: secrecy::SecretString,
    /// Additional headers
    pub headers: HashMap<String, String>,
    /// Credential expiry time
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Provider health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealthReport {
    /// Provider ID
    pub provider_id: String,
    /// Is the provider healthy
    pub healthy: bool,
    /// Latency in milliseconds
    pub latency_ms: Option<u64>,
    /// Error rate (0.0-1.0)
    pub error_rate: Option<f32>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

// ============================================================================
// Safety Filtering (LLM-Shield)
// ============================================================================

/// Trait for consuming safety filtering from LLM-Shield.
///
/// This adapter applies safety filters and validates both
/// input requests and output responses.
#[async_trait]
pub trait SafetyFilter: Send + Sync {
    /// Validate input request for safety.
    async fn validate_input(&self, request: &GatewayRequest) -> IntegrationResult<SafetyResult>;

    /// Validate output response for safety.
    async fn validate_output(&self, response: &GatewayResponse)
        -> IntegrationResult<SafetyResult>;

    /// Check for PII in content.
    async fn check_pii(&self, content: &str) -> IntegrationResult<PiiCheckResult>;

    /// Consume safety policy updates.
    async fn consume_safety_policies(&self) -> IntegrationResult<Vec<SafetyPolicy>>;
}

/// Safety validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyResult {
    /// Is the content safe
    pub safe: bool,
    /// Safety score (0.0-1.0, higher is safer)
    pub score: f32,
    /// Categories that triggered warnings/blocks
    pub triggered_categories: Vec<SafetyCategory>,
    /// Detailed findings
    pub findings: Vec<SafetyFinding>,
    /// Whether the content should be blocked
    pub should_block: bool,
}

/// Safety category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyCategory {
    /// Category name
    pub name: String,
    /// Severity (0-100)
    pub severity: u8,
    /// Confidence (0.0-1.0)
    pub confidence: f32,
}

/// Safety finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyFinding {
    /// Finding type
    pub finding_type: String,
    /// Description
    pub description: String,
    /// Location in content (if applicable)
    pub location: Option<ContentLocation>,
    /// Recommendation
    pub recommendation: Option<String>,
}

/// Content location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentLocation {
    /// Start offset
    pub start: usize,
    /// End offset
    pub end: usize,
}

/// PII check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiCheckResult {
    /// Contains PII
    pub contains_pii: bool,
    /// PII types found
    pub pii_types: Vec<PiiType>,
    /// Redacted content (if requested)
    pub redacted_content: Option<String>,
}

/// PII type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiType {
    /// Type name (email, phone, ssn, etc.)
    pub name: String,
    /// Count found
    pub count: usize,
    /// Locations
    pub locations: Vec<ContentLocation>,
}

/// Safety policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyPolicy {
    /// Policy ID
    pub id: String,
    /// Policy name
    pub name: String,
    /// Policy rules
    pub rules: Vec<SafetyRule>,
    /// Is enabled
    pub enabled: bool,
}

/// Safety rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyRule {
    /// Rule ID
    pub id: String,
    /// Rule type
    pub rule_type: String,
    /// Rule parameters
    pub parameters: HashMap<String, serde_json::Value>,
}

// ============================================================================
// Anomaly Detection (LLM-Sentinel)
// ============================================================================

/// Trait for consuming anomaly alerts from LLM-Sentinel.
///
/// This adapter consumes anomaly detection signals and can
/// trigger fallback behavior when anomalies are detected.
#[async_trait]
pub trait SentinelConsumer: Send + Sync {
    /// Consume the latest anomaly alerts.
    async fn consume_anomalies(&self) -> IntegrationResult<Vec<AnomalyAlert>>;

    /// Check if a provider is experiencing anomalies.
    async fn check_provider_anomalies(&self, provider_id: &str) -> IntegrationResult<AnomalyStatus>;

    /// Get recommended fallback action for an anomaly.
    async fn get_fallback_recommendation(
        &self,
        anomaly: &AnomalyAlert,
    ) -> IntegrationResult<FallbackRecommendation>;

    /// Report an observed anomaly.
    async fn report_anomaly(&self, report: AnomalyReport) -> IntegrationResult<()>;
}

/// Anomaly alert from sentinel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyAlert {
    /// Alert ID
    pub id: String,
    /// Anomaly type
    pub anomaly_type: String,
    /// Severity (0-100)
    pub severity: u8,
    /// Affected provider
    pub provider_id: Option<String>,
    /// Affected model
    pub model: Option<String>,
    /// Description
    pub description: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Recommended action
    pub recommended_action: Option<String>,
    /// Additional context
    pub context: HashMap<String, serde_json::Value>,
}

/// Anomaly status for a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyStatus {
    /// Provider ID
    pub provider_id: String,
    /// Is experiencing anomalies
    pub has_anomalies: bool,
    /// Active anomaly count
    pub active_count: usize,
    /// Maximum severity of active anomalies
    pub max_severity: u8,
    /// Recommended action
    pub action: AnomalyAction,
}

/// Recommended action for anomalies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnomalyAction {
    /// Continue normal operation
    Continue,
    /// Use with caution
    Caution,
    /// Reduce traffic
    Throttle,
    /// Avoid using this provider
    Avoid,
    /// Emergency fallback required
    Fallback,
}

/// Fallback recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackRecommendation {
    /// Should fallback be triggered
    pub should_fallback: bool,
    /// Recommended fallback provider
    pub fallback_provider: Option<String>,
    /// Reason for recommendation
    pub reason: String,
    /// Estimated recovery time
    pub estimated_recovery: Option<Duration>,
}

/// Anomaly report from gateway
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyReport {
    /// Report type
    pub report_type: String,
    /// Provider ID
    pub provider_id: String,
    /// Description
    pub description: String,
    /// Metrics
    pub metrics: HashMap<String, f64>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

// ============================================================================
// Cost Optimization (LLM-CostOps)
// ============================================================================

/// Trait for consuming cost projections from LLM-CostOps.
///
/// This adapter consumes cost information and enables
/// cost-efficient request routing.
#[async_trait]
pub trait CostConsumer: Send + Sync {
    /// Get cost projection for a request.
    async fn get_cost_projection(&self, request: &GatewayRequest)
        -> IntegrationResult<CostProjection>;

    /// Consume cost-efficient provider ranking.
    async fn get_cost_efficient_providers(
        &self,
        request: &GatewayRequest,
    ) -> IntegrationResult<Vec<CostRankedProvider>>;

    /// Report actual usage for cost tracking.
    async fn report_usage(&self, usage: UsageReport) -> IntegrationResult<()>;

    /// Consume budget status.
    async fn get_budget_status(&self, tenant_id: &str) -> IntegrationResult<BudgetStatus>;
}

/// Cost projection for a request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostProjection {
    /// Estimated cost in dollars
    pub estimated_cost: f64,
    /// Cost breakdown
    pub breakdown: CostBreakdown,
    /// Confidence (0.0-1.0)
    pub confidence: f32,
    /// Provider used for estimate
    pub provider_id: String,
}

/// Cost breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    /// Input token cost
    pub input_cost: f64,
    /// Output token cost
    pub output_cost: f64,
    /// Base/fixed cost
    pub base_cost: f64,
}

/// Cost-ranked provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostRankedProvider {
    /// Provider ID
    pub provider_id: String,
    /// Estimated cost
    pub estimated_cost: f64,
    /// Cost efficiency score (higher is better)
    pub efficiency_score: f32,
    /// Quality score (0.0-1.0)
    pub quality_score: f32,
    /// Rank (1 = best)
    pub rank: u32,
}

/// Usage report for cost tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    /// Request ID
    pub request_id: String,
    /// Provider ID
    pub provider_id: String,
    /// Model used
    pub model: String,
    /// Input tokens
    pub input_tokens: u32,
    /// Output tokens
    pub output_tokens: u32,
    /// Actual cost (if known)
    pub actual_cost: Option<f64>,
    /// Tenant ID
    pub tenant_id: Option<String>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Budget status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    /// Tenant ID
    pub tenant_id: String,
    /// Budget limit
    pub budget_limit: f64,
    /// Amount used
    pub used: f64,
    /// Remaining budget
    pub remaining: f64,
    /// Usage percentage (0.0-1.0)
    pub usage_percentage: f32,
    /// Is over budget
    pub over_budget: bool,
    /// Budget period
    pub period: String,
}

// ============================================================================
// Observability (LLM-Observatory)
// ============================================================================

/// Trait for emitting and consuming telemetry via LLM-Observatory.
///
/// This adapter handles trace emission, metrics, latency profiles,
/// and performance feedback consumption.
#[async_trait]
pub trait ObservabilityEmitter: Send + Sync {
    /// Emit a trace span.
    async fn emit_trace(&self, trace: TraceSpan) -> IntegrationResult<()>;

    /// Emit metrics.
    async fn emit_metrics(&self, metrics: Vec<Metric>) -> IntegrationResult<()>;

    /// Emit latency profile.
    async fn emit_latency_profile(&self, profile: LatencyProfile) -> IntegrationResult<()>;

    /// Consume performance feedback.
    async fn consume_performance_feedback(&self)
        -> IntegrationResult<Vec<PerformanceFeedback>>;

    /// Flush pending telemetry.
    async fn flush(&self) -> IntegrationResult<()>;
}

/// Trace span for observability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSpan {
    /// Trace ID
    pub trace_id: String,
    /// Span ID
    pub span_id: String,
    /// Parent span ID
    pub parent_span_id: Option<String>,
    /// Operation name
    pub operation: String,
    /// Start time
    pub start_time: chrono::DateTime<chrono::Utc>,
    /// End time
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Status
    pub status: SpanStatus,
    /// Attributes
    pub attributes: HashMap<String, serde_json::Value>,
    /// Events
    pub events: Vec<SpanEvent>,
}

/// Span status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    /// Unset status
    Unset,
    /// Success
    Ok,
    /// Error occurred
    Error,
}

/// Span event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    /// Event name
    pub name: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Attributes
    pub attributes: HashMap<String, serde_json::Value>,
}

/// Metric for observability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    /// Metric name
    pub name: String,
    /// Metric type
    pub metric_type: MetricType,
    /// Value
    pub value: f64,
    /// Labels
    pub labels: HashMap<String, String>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Metric type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    /// Counter (monotonically increasing)
    Counter,
    /// Gauge (can go up or down)
    Gauge,
    /// Histogram
    Histogram,
    /// Summary
    Summary,
}

/// Latency profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyProfile {
    /// Request ID
    pub request_id: String,
    /// Provider ID
    pub provider_id: String,
    /// Model
    pub model: String,
    /// Total latency in milliseconds
    pub total_ms: u64,
    /// Time to first token (for streaming)
    pub ttft_ms: Option<u64>,
    /// Breakdown by phase
    pub breakdown: LatencyBreakdown,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Latency breakdown by phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyBreakdown {
    /// Queue time
    pub queue_ms: u64,
    /// Routing decision time
    pub routing_ms: u64,
    /// Provider processing time
    pub provider_ms: u64,
    /// Response transformation time
    pub transform_ms: u64,
}

/// Performance feedback from observatory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceFeedback {
    /// Feedback type
    pub feedback_type: String,
    /// Provider ID
    pub provider_id: Option<String>,
    /// Model
    pub model: Option<String>,
    /// Observation
    pub observation: String,
    /// Recommendation
    pub recommendation: Option<String>,
    /// Metrics
    pub metrics: HashMap<String, f64>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

// ============================================================================
// Optimization Hints (LLM-Auto-Optimizer)
// ============================================================================

/// Trait for consuming optimization hints from LLM-Auto-Optimizer.
///
/// This adapter consumes optimization recommendations and
/// feedback for continuous improvement.
#[async_trait]
pub trait OptimizationConsumer: Send + Sync {
    /// Consume optimization hints for a request.
    async fn get_optimization_hints(
        &self,
        request: &GatewayRequest,
    ) -> IntegrationResult<OptimizationHints>;

    /// Consume recommendation feedback.
    async fn consume_recommendations(&self) -> IntegrationResult<Vec<Recommendation>>;

    /// Report optimization outcome.
    async fn report_outcome(&self, outcome: OptimizationOutcome) -> IntegrationResult<()>;
}

/// Optimization hints for a request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationHints {
    /// Suggested model override
    pub suggested_model: Option<String>,
    /// Suggested provider
    pub suggested_provider: Option<String>,
    /// Parameter adjustments
    pub parameter_adjustments: ParameterAdjustments,
    /// Caching recommendation
    pub cache_recommendation: CacheRecommendation,
    /// Confidence (0.0-1.0)
    pub confidence: f32,
    /// Reason for hints
    pub reason: String,
}

/// Parameter adjustments
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParameterAdjustments {
    /// Suggested temperature adjustment
    pub temperature: Option<f32>,
    /// Suggested max_tokens adjustment
    pub max_tokens: Option<u32>,
    /// Suggested top_p adjustment
    pub top_p: Option<f32>,
}

/// Cache recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheRecommendation {
    /// Should cache the response
    pub should_cache: bool,
    /// Cache TTL in seconds
    pub ttl_seconds: Option<u64>,
    /// Cache key hint
    pub cache_key_hint: Option<String>,
}

/// Recommendation from auto-optimizer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    /// Recommendation ID
    pub id: String,
    /// Recommendation type
    pub recommendation_type: String,
    /// Target (provider, model, etc.)
    pub target: String,
    /// Current value
    pub current_value: serde_json::Value,
    /// Recommended value
    pub recommended_value: serde_json::Value,
    /// Expected improvement
    pub expected_improvement: String,
    /// Priority (0-100)
    pub priority: u8,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Optimization outcome report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationOutcome {
    /// Request ID
    pub request_id: String,
    /// Applied hints
    pub applied_hints: Vec<String>,
    /// Outcome metrics
    pub metrics: HashMap<String, f64>,
    /// Was successful
    pub successful: bool,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

// ============================================================================
// Policy Enforcement (LLM-Policy-Engine)
// ============================================================================

/// Trait for consuming policy decisions from LLM-Policy-Engine.
///
/// This adapter consumes policy enforcement decisions before
/// and after request execution.
#[async_trait]
pub trait PolicyConsumer: Send + Sync {
    /// Consume policy decision for a request (pre-execution).
    async fn evaluate_request(&self, request: &GatewayRequest)
        -> IntegrationResult<PolicyDecision>;

    /// Consume policy decision for a response (post-execution).
    async fn evaluate_response(
        &self,
        request: &GatewayRequest,
        response: &GatewayResponse,
    ) -> IntegrationResult<PolicyDecision>;

    /// Consume policy updates.
    async fn consume_policy_updates(&self) -> IntegrationResult<Vec<Policy>>;

    /// Report policy enforcement outcome.
    async fn report_enforcement(&self, report: EnforcementReport) -> IntegrationResult<()>;
}

/// Policy decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDecision {
    /// Decision result
    pub allowed: bool,
    /// Matched policies
    pub matched_policies: Vec<String>,
    /// Violations (if any)
    pub violations: Vec<PolicyViolation>,
    /// Required actions
    pub required_actions: Vec<PolicyAction>,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Policy violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyViolation {
    /// Policy ID
    pub policy_id: String,
    /// Policy name
    pub policy_name: String,
    /// Violation description
    pub description: String,
    /// Severity (0-100)
    pub severity: u8,
}

/// Policy action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyAction {
    /// Action type
    pub action_type: PolicyActionType,
    /// Parameters
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Policy action type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyActionType {
    /// Block the request
    Block,
    /// Allow with warning
    Warn,
    /// Log the action
    Log,
    /// Rate limit
    RateLimit,
    /// Redirect to different provider
    Redirect,
    /// Transform request/response
    Transform,
}

/// Policy definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    /// Policy ID
    pub id: String,
    /// Policy name
    pub name: String,
    /// Policy description
    pub description: Option<String>,
    /// Policy rules
    pub rules: Vec<PolicyRule>,
    /// Is enabled
    pub enabled: bool,
    /// Priority (higher = evaluated first)
    pub priority: u32,
}

/// Policy rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Rule ID
    pub id: String,
    /// Condition
    pub condition: serde_json::Value,
    /// Action on match
    pub action: PolicyActionType,
    /// Action parameters
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Enforcement report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnforcementReport {
    /// Request ID
    pub request_id: String,
    /// Policy decisions made
    pub decisions: Vec<PolicyDecision>,
    /// Enforcement outcome
    pub outcome: EnforcementOutcome,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Enforcement outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnforcementOutcome {
    /// Request allowed
    Allowed,
    /// Request blocked
    Blocked,
    /// Request modified
    Modified,
    /// Warning issued
    Warned,
}
