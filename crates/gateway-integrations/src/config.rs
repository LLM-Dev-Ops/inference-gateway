//! Configuration for integration adapters.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for all integrations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationsConfig {
    /// Enable all integrations
    #[serde(default)]
    pub enabled: bool,

    /// Connector Hub configuration
    #[serde(default)]
    pub connector_hub: ConnectorHubConfig,

    /// Shield configuration
    #[serde(default)]
    pub shield: ShieldConfig,

    /// Sentinel configuration
    #[serde(default)]
    pub sentinel: SentinelConfig,

    /// CostOps configuration
    #[serde(default)]
    pub cost_ops: CostOpsConfig,

    /// Observatory configuration
    #[serde(default)]
    pub observatory: ObservatoryConfig,

    /// Auto-Optimizer configuration
    #[serde(default)]
    pub auto_optimizer: AutoOptimizerConfig,

    /// Policy Engine configuration
    #[serde(default)]
    pub policy_engine: PolicyEngineConfig,

    /// Router configuration
    #[serde(default)]
    pub router: RouterConfig,

    /// RuVector service configuration
    #[serde(default)]
    pub ruvector: RuVectorConfig,
}

impl Default for IntegrationsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            connector_hub: ConnectorHubConfig::default(),
            shield: ShieldConfig::default(),
            sentinel: SentinelConfig::default(),
            cost_ops: CostOpsConfig::default(),
            observatory: ObservatoryConfig::default(),
            auto_optimizer: AutoOptimizerConfig::default(),
            policy_engine: PolicyEngineConfig::default(),
            router: RouterConfig::default(),
            ruvector: RuVectorConfig::default(),
        }
    }
}

/// Connector Hub configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorHubConfig {
    /// Enable connector hub integration
    #[serde(default)]
    pub enabled: bool,

    /// Endpoint URL for connector hub service
    #[serde(default)]
    pub endpoint: Option<String>,

    /// Connection timeout
    #[serde(default = "default_timeout", with = "humantime_serde")]
    pub timeout: Duration,

    /// Enable provider discovery
    #[serde(default = "default_true")]
    pub auto_discover: bool,

    /// Credential refresh interval
    #[serde(default = "default_refresh_interval", with = "humantime_serde")]
    pub credential_refresh_interval: Duration,
}

impl Default for ConnectorHubConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            timeout: default_timeout(),
            auto_discover: true,
            credential_refresh_interval: default_refresh_interval(),
        }
    }
}

/// Shield configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldConfig {
    /// Enable shield integration
    #[serde(default)]
    pub enabled: bool,

    /// Endpoint URL for shield service
    #[serde(default)]
    pub endpoint: Option<String>,

    /// Enable input validation
    #[serde(default = "default_true")]
    pub validate_input: bool,

    /// Enable output validation
    #[serde(default = "default_true")]
    pub validate_output: bool,

    /// Block on PII detection
    #[serde(default)]
    pub block_on_pii: bool,

    /// Safety threshold (0.0-1.0)
    #[serde(default = "default_safety_threshold")]
    pub safety_threshold: f32,

    /// Connection timeout
    #[serde(default = "default_timeout", with = "humantime_serde")]
    pub timeout: Duration,
}

impl Default for ShieldConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            validate_input: true,
            validate_output: true,
            block_on_pii: false,
            safety_threshold: default_safety_threshold(),
            timeout: default_timeout(),
        }
    }
}

/// Sentinel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelConfig {
    /// Enable sentinel integration
    #[serde(default)]
    pub enabled: bool,

    /// Endpoint URL for sentinel service
    #[serde(default)]
    pub endpoint: Option<String>,

    /// Enable anomaly detection consumption
    #[serde(default = "default_true")]
    pub consume_anomalies: bool,

    /// Anomaly severity threshold for fallback (0-100)
    #[serde(default = "default_severity_threshold")]
    pub severity_threshold: u8,

    /// Enable automatic fallback on anomalies
    #[serde(default)]
    pub auto_fallback: bool,

    /// Connection timeout
    #[serde(default = "default_timeout", with = "humantime_serde")]
    pub timeout: Duration,
}

impl Default for SentinelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            consume_anomalies: true,
            severity_threshold: default_severity_threshold(),
            auto_fallback: false,
            timeout: default_timeout(),
        }
    }
}

/// CostOps configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostOpsConfig {
    /// Enable cost-ops integration
    #[serde(default)]
    pub enabled: bool,

    /// Endpoint URL for cost-ops service
    #[serde(default)]
    pub endpoint: Option<String>,

    /// Enable cost-based routing
    #[serde(default = "default_true")]
    pub cost_based_routing: bool,

    /// Maximum cost per request (in dollars)
    #[serde(default)]
    pub max_cost_per_request: Option<f64>,

    /// Budget alert threshold (0.0-1.0)
    #[serde(default = "default_budget_threshold")]
    pub budget_alert_threshold: f32,

    /// Connection timeout
    #[serde(default = "default_timeout", with = "humantime_serde")]
    pub timeout: Duration,
}

impl Default for CostOpsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            cost_based_routing: true,
            max_cost_per_request: None,
            budget_alert_threshold: default_budget_threshold(),
            timeout: default_timeout(),
        }
    }
}

/// Observatory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservatoryConfig {
    /// Enable observatory integration
    #[serde(default)]
    pub enabled: bool,

    /// Endpoint URL for observatory service
    #[serde(default)]
    pub endpoint: Option<String>,

    /// Enable trace emission
    #[serde(default = "default_true")]
    pub emit_traces: bool,

    /// Enable metrics emission
    #[serde(default = "default_true")]
    pub emit_metrics: bool,

    /// Enable latency profiling
    #[serde(default = "default_true")]
    pub emit_latency_profiles: bool,

    /// Consume performance feedback
    #[serde(default = "default_true")]
    pub consume_performance_feedback: bool,

    /// Batch size for telemetry emission
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Flush interval for telemetry
    #[serde(default = "default_flush_interval", with = "humantime_serde")]
    pub flush_interval: Duration,

    /// Connection timeout
    #[serde(default = "default_timeout", with = "humantime_serde")]
    pub timeout: Duration,
}

impl Default for ObservatoryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            emit_traces: true,
            emit_metrics: true,
            emit_latency_profiles: true,
            consume_performance_feedback: true,
            batch_size: default_batch_size(),
            flush_interval: default_flush_interval(),
            timeout: default_timeout(),
        }
    }
}

/// Auto-Optimizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoOptimizerConfig {
    /// Enable auto-optimizer integration
    #[serde(default)]
    pub enabled: bool,

    /// Endpoint URL for auto-optimizer service
    #[serde(default)]
    pub endpoint: Option<String>,

    /// Consume optimization hints
    #[serde(default = "default_true")]
    pub consume_hints: bool,

    /// Apply recommendations automatically
    #[serde(default)]
    pub auto_apply: bool,

    /// Consume recommendation feedback
    #[serde(default = "default_true")]
    pub consume_feedback: bool,

    /// Connection timeout
    #[serde(default = "default_timeout", with = "humantime_serde")]
    pub timeout: Duration,
}

impl Default for AutoOptimizerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            consume_hints: true,
            auto_apply: false,
            consume_feedback: true,
            timeout: default_timeout(),
        }
    }
}

/// Policy Engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEngineConfig {
    /// Enable policy engine integration
    #[serde(default)]
    pub enabled: bool,

    /// Endpoint URL for policy engine service
    #[serde(default)]
    pub endpoint: Option<String>,

    /// Consume policy decisions before requests
    #[serde(default = "default_true")]
    pub pre_request_check: bool,

    /// Consume policy decisions after responses
    #[serde(default)]
    pub post_response_check: bool,

    /// Block on policy violation
    #[serde(default = "default_true")]
    pub block_on_violation: bool,

    /// Cache policy decisions
    #[serde(default = "default_true")]
    pub cache_decisions: bool,

    /// Policy cache TTL
    #[serde(default = "default_cache_ttl", with = "humantime_serde")]
    pub cache_ttl: Duration,

    /// Connection timeout
    #[serde(default = "default_timeout", with = "humantime_serde")]
    pub timeout: Duration,
}

impl Default for PolicyEngineConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            pre_request_check: true,
            post_response_check: false,
            block_on_violation: true,
            cache_decisions: true,
            cache_ttl: default_cache_ttl(),
            timeout: default_timeout(),
        }
    }
}

/// Router configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    /// Enable router integration (Layer 2 module)
    #[serde(default)]
    pub enabled: bool,

    /// Consume routing rules from external source
    #[serde(default = "default_true")]
    pub consume_rules: bool,

    /// Consume decision graphs
    #[serde(default = "default_true")]
    pub consume_decision_graphs: bool,

    /// Rule refresh interval
    #[serde(default = "default_refresh_interval", with = "humantime_serde")]
    pub rule_refresh_interval: Duration,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            consume_rules: true,
            consume_decision_graphs: true,
            rule_refresh_interval: default_refresh_interval(),
        }
    }
}

// Default value functions

fn default_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_refresh_interval() -> Duration {
    Duration::from_secs(300) // 5 minutes
}

fn default_flush_interval() -> Duration {
    Duration::from_secs(10)
}

fn default_cache_ttl() -> Duration {
    Duration::from_secs(60)
}

fn default_batch_size() -> usize {
    100
}

fn default_safety_threshold() -> f32 {
    0.8
}

fn default_severity_threshold() -> u8 {
    70
}

fn default_budget_threshold() -> f32 {
    0.9
}

fn default_true() -> bool {
    true
}

fn default_pool_size() -> u32 {
    10
}

fn default_retry_count() -> u32 {
    3
}

/// RuVector service configuration
///
/// Configuration for the RuVector service client adapter.
/// RuVector-service is backed by Google SQL (Postgres) and is the
/// ONLY persistence layer for DecisionEvents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuVectorConfig {
    /// Enable RuVector integration
    #[serde(default)]
    pub enabled: bool,

    /// Endpoint URL for ruvector-service
    #[serde(default)]
    pub endpoint: Option<String>,

    /// API key for authentication
    #[serde(default)]
    pub api_key: Option<String>,

    /// Connection timeout
    #[serde(default = "default_timeout", with = "humantime_serde")]
    pub timeout: Duration,

    /// Number of retry attempts for failed requests
    #[serde(default = "default_retry_count")]
    pub retry_count: u32,

    /// Connection pool size
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,

    /// Enable batch persistence mode
    #[serde(default = "default_true")]
    pub batch_enabled: bool,

    /// Maximum batch size for event persistence
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Flush interval for batched events
    #[serde(default = "default_flush_interval", with = "humantime_serde")]
    pub flush_interval: Duration,
}

impl Default for RuVectorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            api_key: None,
            timeout: default_timeout(),
            retry_count: default_retry_count(),
            pool_size: default_pool_size(),
            batch_enabled: true,
            batch_size: default_batch_size(),
            flush_interval: default_flush_interval(),
        }
    }
}
