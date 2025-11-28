//! Enterprise health check system for the gateway.
//!
//! Provides comprehensive health monitoring including:
//! - Liveness probes (for Kubernetes)
//! - Readiness probes (for load balancing)
//! - Startup probes (for slow initialization)
//! - Deep health checks (provider connectivity)
//! - Health aggregation and scoring

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Health check configuration
#[derive(Debug, Clone)]
pub struct HealthConfig {
    /// Enable detailed health information in responses
    pub detailed_response: bool,
    /// Cache health check results for this duration
    pub cache_duration: Duration,
    /// Timeout for individual provider checks
    pub provider_check_timeout: Duration,
    /// Whether to include provider health in readiness
    pub include_providers_in_readiness: bool,
    /// Minimum healthy providers for readiness
    pub min_healthy_providers: usize,
    /// Whether to run health checks in parallel
    pub parallel_checks: bool,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            detailed_response: true,
            cache_duration: Duration::from_secs(5),
            provider_check_timeout: Duration::from_secs(5),
            include_providers_in_readiness: true,
            min_healthy_providers: 1,
            parallel_checks: true,
        }
    }
}

impl HealthConfig {
    /// Create a new health configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set detailed response
    #[must_use]
    pub fn with_detailed_response(mut self, detailed: bool) -> Self {
        self.detailed_response = detailed;
        self
    }

    /// Set cache duration
    #[must_use]
    pub fn with_cache_duration(mut self, duration: Duration) -> Self {
        self.cache_duration = duration;
        self
    }

    /// Set provider check timeout
    #[must_use]
    pub fn with_provider_check_timeout(mut self, timeout: Duration) -> Self {
        self.provider_check_timeout = timeout;
        self
    }
}

/// Health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Fully healthy
    Healthy,
    /// Degraded but operational
    Degraded,
    /// Unhealthy
    Unhealthy,
    /// Unknown status
    Unknown,
}

impl HealthStatus {
    /// Check if status represents a healthy state
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }

    /// Get HTTP status code for this health status
    #[must_use]
    pub fn http_status_code(&self) -> u16 {
        match self {
            Self::Healthy => 200,
            Self::Degraded => 200,
            Self::Unhealthy => 503,
            Self::Unknown => 503,
        }
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded => write!(f, "degraded"),
            Self::Unhealthy => write!(f, "unhealthy"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Component health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component name
    pub name: String,
    /// Component status
    pub status: HealthStatus,
    /// Time taken for the check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_duration_ms: Option<u64>,
    /// Last check timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_check: Option<String>,
    /// Error message if unhealthy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Additional details
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub details: HashMap<String, serde_json::Value>,
}

impl ComponentHealth {
    /// Create a healthy component
    #[must_use]
    pub fn healthy(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Healthy,
            check_duration_ms: None,
            last_check: None,
            error: None,
            details: HashMap::new(),
        }
    }

    /// Create an unhealthy component
    #[must_use]
    pub fn unhealthy(name: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Unhealthy,
            check_duration_ms: None,
            last_check: None,
            error: Some(error.into()),
            details: HashMap::new(),
        }
    }

    /// Set check duration
    #[must_use]
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.check_duration_ms = Some(duration.as_millis() as u64);
        self
    }

    /// Add detail
    #[must_use]
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.details.insert(key.into(), json_value);
        }
        self
    }
}

/// Aggregated health response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Overall status
    pub status: HealthStatus,
    /// Service version
    pub version: String,
    /// Server uptime in seconds
    pub uptime_seconds: u64,
    /// Components health (when detailed)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ComponentHealth>,
    /// Health score (0-100)
    pub health_score: u8,
    /// Timestamp
    pub timestamp: String,
}

impl HealthResponse {
    /// Create a new health response
    #[must_use]
    pub fn new(status: HealthStatus, uptime: Duration) -> Self {
        Self {
            status,
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: uptime.as_secs(),
            components: Vec::new(),
            health_score: if status.is_healthy() { 100 } else { 0 },
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Add component health
    #[must_use]
    pub fn with_component(mut self, component: ComponentHealth) -> Self {
        self.components.push(component);
        self.recalculate_score();
        self
    }

    /// Add multiple components
    #[must_use]
    pub fn with_components(mut self, components: Vec<ComponentHealth>) -> Self {
        self.components.extend(components);
        self.recalculate_score();
        self
    }

    /// Recalculate health score
    fn recalculate_score(&mut self) {
        if self.components.is_empty() {
            self.health_score = if self.status.is_healthy() { 100 } else { 0 };
            return;
        }

        let healthy_count = self
            .components
            .iter()
            .filter(|c| c.status.is_healthy())
            .count();

        self.health_score = ((healthy_count * 100) / self.components.len()) as u8;

        // Update overall status based on components
        if healthy_count == self.components.len() {
            self.status = HealthStatus::Healthy;
        } else if healthy_count > 0 {
            self.status = HealthStatus::Degraded;
        } else {
            self.status = HealthStatus::Unhealthy;
        }
    }
}

/// Liveness response (minimal)
#[derive(Debug, Clone, Serialize)]
pub struct LivenessResponse {
    /// Status
    pub status: String,
}

/// Readiness response
#[derive(Debug, Clone, Serialize)]
pub struct ReadinessResponse {
    /// Ready status
    pub ready: bool,
    /// Reason if not ready
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Provider count
    pub providers: usize,
    /// Healthy provider count
    pub healthy_providers: usize,
}

/// Startup probe response
#[derive(Debug, Clone, Serialize)]
pub struct StartupResponse {
    /// Started status
    pub started: bool,
    /// Initialization progress (0-100)
    pub progress: u8,
    /// Components initialized
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub initialized: Vec<String>,
    /// Components pending
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pending: Vec<String>,
}

/// Health checker for the gateway
pub struct HealthChecker {
    config: HealthConfig,
    /// Startup timestamp
    startup_time: Instant,
    /// Cached health result
    cached_health: RwLock<Option<(Instant, HealthResponse)>>,
    /// Whether startup is complete
    startup_complete: AtomicBool,
    /// Startup progress (0-100)
    startup_progress: AtomicU64,
    /// Initialized components
    initialized_components: RwLock<Vec<String>>,
    /// Required startup components
    required_components: Vec<String>,
    /// Whether shutdown is in progress
    shutting_down: AtomicBool,
}

impl HealthChecker {
    /// Create a new health checker
    #[must_use]
    pub fn new(config: HealthConfig) -> Self {
        Self {
            config,
            startup_time: Instant::now(),
            cached_health: RwLock::new(None),
            startup_complete: AtomicBool::new(false),
            startup_progress: AtomicU64::new(0),
            initialized_components: RwLock::new(Vec::new()),
            required_components: vec![
                "config".to_string(),
                "providers".to_string(),
                "router".to_string(),
                "metrics".to_string(),
            ],
            shutting_down: AtomicBool::new(false),
        }
    }

    /// Create with default configuration
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(HealthConfig::default())
    }

    /// Get uptime
    #[must_use]
    pub fn uptime(&self) -> Duration {
        self.startup_time.elapsed()
    }

    /// Mark a component as initialized
    pub async fn mark_initialized(&self, component: &str) {
        let mut initialized = self.initialized_components.write().await;
        if !initialized.contains(&component.to_string()) {
            initialized.push(component.to_string());
            info!(component = component, "Component initialized");
        }

        // Update progress
        let total = self.required_components.len();
        let completed = initialized
            .iter()
            .filter(|c| self.required_components.contains(c))
            .count();
        let progress = ((completed * 100) / total.max(1)) as u64;
        self.startup_progress.store(progress, Ordering::SeqCst);

        // Check if startup is complete
        if completed >= total {
            self.startup_complete.store(true, Ordering::SeqCst);
            info!("Startup complete");
        }
    }

    /// Mark shutdown as in progress
    pub fn mark_shutting_down(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        info!("Health checker marked as shutting down");
    }

    /// Check if startup is complete
    #[must_use]
    pub fn is_startup_complete(&self) -> bool {
        self.startup_complete.load(Ordering::SeqCst)
    }

    /// Check if shutting down
    #[must_use]
    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    /// Perform liveness check (fast, minimal)
    #[must_use]
    pub fn check_liveness(&self) -> LivenessResponse {
        LivenessResponse {
            status: if self.is_shutting_down() {
                "shutting_down".to_string()
            } else {
                "alive".to_string()
            },
        }
    }

    /// Perform startup check
    pub async fn check_startup(&self) -> StartupResponse {
        let initialized = self.initialized_components.read().await;
        let pending: Vec<String> = self
            .required_components
            .iter()
            .filter(|c| !initialized.contains(c))
            .cloned()
            .collect();

        StartupResponse {
            started: self.is_startup_complete(),
            progress: self.startup_progress.load(Ordering::SeqCst) as u8,
            initialized: initialized.clone(),
            pending,
        }
    }

    /// Perform readiness check
    pub async fn check_readiness(&self, provider_count: usize, healthy_provider_count: usize) -> ReadinessResponse {
        // Not ready if shutting down
        if self.is_shutting_down() {
            return ReadinessResponse {
                ready: false,
                reason: Some("shutting down".to_string()),
                providers: provider_count,
                healthy_providers: healthy_provider_count,
            };
        }

        // Not ready if startup not complete
        if !self.is_startup_complete() {
            return ReadinessResponse {
                ready: false,
                reason: Some("startup in progress".to_string()),
                providers: provider_count,
                healthy_providers: healthy_provider_count,
            };
        }

        // Check provider requirements
        if self.config.include_providers_in_readiness {
            if provider_count == 0 {
                return ReadinessResponse {
                    ready: false,
                    reason: Some("no providers configured".to_string()),
                    providers: provider_count,
                    healthy_providers: healthy_provider_count,
                };
            }

            if healthy_provider_count < self.config.min_healthy_providers {
                return ReadinessResponse {
                    ready: false,
                    reason: Some(format!(
                        "insufficient healthy providers: {} < {}",
                        healthy_provider_count, self.config.min_healthy_providers
                    )),
                    providers: provider_count,
                    healthy_providers: healthy_provider_count,
                };
            }
        }

        ReadinessResponse {
            ready: true,
            reason: None,
            providers: provider_count,
            healthy_providers: healthy_provider_count,
        }
    }

    /// Perform deep health check
    pub async fn check_deep(&self, components: Vec<ComponentHealth>) -> HealthResponse {
        // Check cache first
        {
            let cache = self.cached_health.read().await;
            if let Some((cached_at, ref response)) = *cache {
                if cached_at.elapsed() < self.config.cache_duration {
                    debug!("Returning cached health response");
                    return response.clone();
                }
            }
        }

        // Build response
        let status = if self.is_shutting_down() {
            HealthStatus::Unhealthy
        } else if !self.is_startup_complete() {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let mut response = HealthResponse::new(status, self.uptime());

        // Add core system component
        let core_health = if self.is_shutting_down() {
            ComponentHealth::unhealthy("core", "shutting down")
        } else {
            ComponentHealth::healthy("core")
                .with_detail("uptime_seconds", self.uptime().as_secs())
                .with_detail("startup_complete", self.is_startup_complete())
        };
        response = response.with_component(core_health);

        // Add provided components
        if self.config.detailed_response {
            response = response.with_components(components);
        }

        // Update cache
        {
            let mut cache = self.cached_health.write().await;
            *cache = Some((Instant::now(), response.clone()));
        }

        response
    }

    /// Clear health cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cached_health.write().await;
        *cache = None;
    }
}

/// Provider health check result
#[derive(Debug, Clone)]
pub struct ProviderHealthResult {
    /// Provider ID
    pub provider_id: String,
    /// Health status
    pub status: HealthStatus,
    /// Response time
    pub response_time: Option<Duration>,
    /// Error message
    pub error: Option<String>,
    /// Last successful check
    pub last_success: Option<Instant>,
}

impl ProviderHealthResult {
    /// Create a healthy result
    #[must_use]
    pub fn healthy(provider_id: impl Into<String>, response_time: Duration) -> Self {
        Self {
            provider_id: provider_id.into(),
            status: HealthStatus::Healthy,
            response_time: Some(response_time),
            error: None,
            last_success: Some(Instant::now()),
        }
    }

    /// Create an unhealthy result
    #[must_use]
    pub fn unhealthy(provider_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            status: HealthStatus::Unhealthy,
            response_time: None,
            error: Some(error.into()),
            last_success: None,
        }
    }
}

/// Aggregate provider health results into a component
#[must_use]
pub fn aggregate_provider_health(results: &[ProviderHealthResult]) -> ComponentHealth {
    if results.is_empty() {
        return ComponentHealth::unhealthy("providers", "no providers configured");
    }

    let healthy_count = results.iter().filter(|r| r.status.is_healthy()).count();
    let total_count = results.len();

    let status = if healthy_count == total_count {
        HealthStatus::Healthy
    } else if healthy_count > 0 {
        HealthStatus::Degraded
    } else {
        HealthStatus::Unhealthy
    };

    let avg_response_time: Option<Duration> = {
        let times: Vec<Duration> = results.iter().filter_map(|r| r.response_time).collect();
        if times.is_empty() {
            None
        } else {
            Some(times.iter().sum::<Duration>() / times.len() as u32)
        }
    };

    let mut component = ComponentHealth {
        name: "providers".to_string(),
        status,
        check_duration_ms: avg_response_time.map(|d| d.as_millis() as u64),
        last_check: Some(chrono::Utc::now().to_rfc3339()),
        error: None,
        details: HashMap::new(),
    };

    component
        .details
        .insert("total".to_string(), serde_json::json!(total_count));
    component
        .details
        .insert("healthy".to_string(), serde_json::json!(healthy_count));

    // Add individual provider status
    let provider_details: HashMap<String, String> = results
        .iter()
        .map(|r| (r.provider_id.clone(), r.status.to_string()))
        .collect();
    component.details.insert(
        "providers".to_string(),
        serde_json::to_value(provider_details).unwrap_or_default(),
    );

    component
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_is_healthy() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(HealthStatus::Degraded.is_healthy());
        assert!(!HealthStatus::Unhealthy.is_healthy());
        assert!(!HealthStatus::Unknown.is_healthy());
    }

    #[test]
    fn test_health_status_http_code() {
        assert_eq!(HealthStatus::Healthy.http_status_code(), 200);
        assert_eq!(HealthStatus::Degraded.http_status_code(), 200);
        assert_eq!(HealthStatus::Unhealthy.http_status_code(), 503);
        assert_eq!(HealthStatus::Unknown.http_status_code(), 503);
    }

    #[test]
    fn test_component_health_builders() {
        let healthy = ComponentHealth::healthy("test");
        assert_eq!(healthy.status, HealthStatus::Healthy);
        assert!(healthy.error.is_none());

        let unhealthy = ComponentHealth::unhealthy("test", "error message");
        assert_eq!(unhealthy.status, HealthStatus::Unhealthy);
        assert_eq!(unhealthy.error, Some("error message".to_string()));
    }

    #[test]
    fn test_health_response_score() {
        let response = HealthResponse::new(HealthStatus::Healthy, Duration::from_secs(100))
            .with_component(ComponentHealth::healthy("a"))
            .with_component(ComponentHealth::healthy("b"))
            .with_component(ComponentHealth::unhealthy("c", "error"));

        // 2/3 healthy = 66%
        assert_eq!(response.health_score, 66);
        assert_eq!(response.status, HealthStatus::Degraded);
    }

    #[test]
    fn test_health_response_all_healthy() {
        let response = HealthResponse::new(HealthStatus::Healthy, Duration::from_secs(100))
            .with_component(ComponentHealth::healthy("a"))
            .with_component(ComponentHealth::healthy("b"));

        assert_eq!(response.health_score, 100);
        assert_eq!(response.status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_health_checker_liveness() {
        let checker = HealthChecker::with_defaults();
        let response = checker.check_liveness();
        assert_eq!(response.status, "alive");

        checker.mark_shutting_down();
        let response = checker.check_liveness();
        assert_eq!(response.status, "shutting_down");
    }

    #[tokio::test]
    async fn test_health_checker_startup() {
        let checker = HealthChecker::with_defaults();

        // Initially not started
        let response = checker.check_startup().await;
        assert!(!response.started);
        assert_eq!(response.progress, 0);

        // Mark components
        checker.mark_initialized("config").await;
        checker.mark_initialized("providers").await;

        let response = checker.check_startup().await;
        assert_eq!(response.progress, 50);

        // Complete startup
        checker.mark_initialized("router").await;
        checker.mark_initialized("metrics").await;

        let response = checker.check_startup().await;
        assert!(response.started);
        assert_eq!(response.progress, 100);
    }

    #[tokio::test]
    async fn test_health_checker_readiness() {
        let checker = HealthChecker::with_defaults();

        // Not ready before startup complete
        let response = checker.check_readiness(2, 2).await;
        assert!(!response.ready);

        // Complete startup
        for comp in &["config", "providers", "router", "metrics"] {
            checker.mark_initialized(comp).await;
        }

        // Ready after startup
        let response = checker.check_readiness(2, 2).await;
        assert!(response.ready);

        // Not ready if no healthy providers
        let response = checker.check_readiness(2, 0).await;
        assert!(!response.ready);
    }

    #[tokio::test]
    async fn test_health_checker_deep() {
        let checker = HealthChecker::with_defaults();

        let components = vec![
            ComponentHealth::healthy("database"),
            ComponentHealth::healthy("cache"),
        ];

        let response = checker.check_deep(components).await;
        assert!(!response.components.is_empty());
    }

    #[test]
    fn test_provider_health_result() {
        let healthy = ProviderHealthResult::healthy("openai", Duration::from_millis(100));
        assert_eq!(healthy.status, HealthStatus::Healthy);
        assert!(healthy.response_time.is_some());

        let unhealthy = ProviderHealthResult::unhealthy("anthropic", "timeout");
        assert_eq!(unhealthy.status, HealthStatus::Unhealthy);
        assert!(unhealthy.error.is_some());
    }

    #[test]
    fn test_aggregate_provider_health() {
        let results = vec![
            ProviderHealthResult::healthy("openai", Duration::from_millis(100)),
            ProviderHealthResult::healthy("anthropic", Duration::from_millis(150)),
            ProviderHealthResult::unhealthy("azure", "connection failed"),
        ];

        let component = aggregate_provider_health(&results);
        assert_eq!(component.status, HealthStatus::Degraded);
        assert_eq!(component.details.get("total"), Some(&serde_json::json!(3)));
        assert_eq!(component.details.get("healthy"), Some(&serde_json::json!(2)));
    }

    #[test]
    fn test_aggregate_provider_health_empty() {
        let results: Vec<ProviderHealthResult> = vec![];
        let component = aggregate_provider_health(&results);
        assert_eq!(component.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_config_builder() {
        let config = HealthConfig::new()
            .with_detailed_response(false)
            .with_cache_duration(Duration::from_secs(10))
            .with_provider_check_timeout(Duration::from_secs(3));

        assert!(!config.detailed_response);
        assert_eq!(config.cache_duration, Duration::from_secs(10));
        assert_eq!(config.provider_check_timeout, Duration::from_secs(3));
    }

    #[tokio::test]
    async fn test_health_cache() {
        let config = HealthConfig {
            cache_duration: Duration::from_millis(100),
            ..Default::default()
        };
        let checker = HealthChecker::new(config);

        // First call should compute
        let response1 = checker.check_deep(vec![]).await;

        // Second call should use cache
        let response2 = checker.check_deep(vec![]).await;

        // Timestamps should be the same (cached)
        assert_eq!(response1.timestamp, response2.timestamp);

        // Wait for cache to expire
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should recompute
        let response3 = checker.check_deep(vec![]).await;
        assert_ne!(response1.timestamp, response3.timestamp);
    }
}
