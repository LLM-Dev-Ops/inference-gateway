# LLM-Inference-Gateway: Configuration and Hot Reload System Pseudocode

> **Status**: Production-Ready Design
> **Language**: Rust (Thread-Safe, Zero-Downtime, Enterprise-Grade)
> **Version**: 1.0.0
> **Last Updated**: 2025-11-27

---

## Table of Contents

1. [Configuration Schema](#1-configuration-schema)
2. [Configuration Loader](#2-configuration-loader)
3. [Configuration Validator](#3-configuration-validator)
4. [Hot Reload Manager](#4-hot-reload-manager)
5. [Secrets Manager Integration](#5-secrets-manager-integration)
6. [Configuration Diff and Versioning](#6-configuration-diff-and-versioning)
7. [Configuration Sources](#7-configuration-sources)
8. [Integration Patterns](#8-integration-patterns)

---

## 1. Configuration Schema

### 1.1 Root Configuration Structure

```rust
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use validator::Validate;
use url::Url;

/// Root gateway configuration with all subsystems
/// Thread-safe with Arc for zero-copy sharing across components
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct GatewayConfig {
    /// Configuration version for tracking changes
    #[serde(default = "default_version")]
    pub version: String,

    /// Configuration metadata
    #[serde(default)]
    pub metadata: ConfigMetadata,

    /// Server/HTTP configuration
    #[validate(nested)]
    pub server: ServerConfig,

    /// Provider configurations
    #[validate]
    #[validate(length(min = 1, message = "At least one provider required"))]
    pub providers: Vec<ProviderConfig>,

    /// Routing and load balancing configuration
    #[validate(nested)]
    pub routing: RoutingConfig,

    /// Resilience configuration (retries, circuit breakers, timeouts)
    #[validate(nested)]
    pub resilience: ResilienceConfig,

    /// Observability configuration (logging, metrics, tracing)
    #[validate(nested)]
    pub observability: ObservabilityConfig,

    /// Security configuration (auth, TLS, rate limiting)
    #[validate(nested)]
    pub security: SecurityConfig,

    /// Secrets manager configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(nested)]
    pub secrets: Option<SecretsConfig>,

    /// Feature flags
    #[serde(default)]
    pub features: FeatureFlags,

    /// Custom extensions (provider-specific or experimental settings)
    #[serde(default)]
    pub extensions: HashMap<String, serde_json::Value>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// Configuration metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigMetadata {
    /// Human-readable configuration name
    pub name: Option<String>,

    /// Environment (dev, staging, production)
    pub environment: Option<String>,

    /// Configuration author/owner
    pub owner: Option<String>,

    /// Configuration description
    pub description: Option<String>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: HashMap<String, String>,

    /// Last modified timestamp
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,

    /// Checksum for integrity verification
    pub checksum: Option<String>,
}

impl GatewayConfig {
    /// Create a new config with minimal defaults
    pub fn new() -> Self {
        Self {
            version: default_version(),
            metadata: ConfigMetadata::default(),
            server: ServerConfig::default(),
            providers: Vec::new(),
            routing: RoutingConfig::default(),
            resilience: ResilienceConfig::default(),
            observability: ObservabilityConfig::default(),
            security: SecurityConfig::default(),
            secrets: None,
            features: FeatureFlags::default(),
            extensions: HashMap::new(),
        }
    }

    /// Validate configuration and check cross-field constraints
    pub fn validate_full(&self) -> Result<(), ConfigValidationError> {
        // Run validator derive validation
        self.validate()
            .map_err(|e| ConfigValidationError::SchemaValidation(e.to_string()))?;

        // Cross-field validation
        self.validate_provider_references()?;
        self.validate_routing_consistency()?;
        self.validate_secrets_consistency()?;

        Ok(())
    }

    /// Validate provider references in routing rules
    fn validate_provider_references(&self) -> Result<(), ConfigValidationError> {
        let provider_ids: std::collections::HashSet<_> =
            self.providers.iter().map(|p| &p.id).collect();

        for rule in &self.routing.policy.rules {
            if let RoutingAction::RouteToProvider { provider_id, fallbacks } = &rule.action {
                if !provider_ids.contains(provider_id) {
                    return Err(ConfigValidationError::InvalidReference {
                        field: "routing.policy.rules.action.provider_id",
                        reference: provider_id.clone(),
                    });
                }

                for fallback in fallbacks {
                    if !provider_ids.contains(fallback) {
                        return Err(ConfigValidationError::InvalidReference {
                            field: "routing.policy.rules.action.fallbacks",
                            reference: fallback.clone(),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate routing configuration consistency
    fn validate_routing_consistency(&self) -> Result<(), ConfigValidationError> {
        // Ensure no duplicate routing rule IDs
        let mut seen_ids = std::collections::HashSet::new();
        for rule in &self.routing.policy.rules {
            if !seen_ids.insert(&rule.id) {
                return Err(ConfigValidationError::DuplicateId {
                    field: "routing.policy.rules.id",
                    id: rule.id.clone(),
                });
            }
        }

        Ok(())
    }

    /// Validate secrets configuration consistency
    fn validate_secrets_consistency(&self) -> Result<(), ConfigValidationError> {
        // If secrets manager is configured, ensure providers reference it correctly
        if self.secrets.is_some() {
            for provider in &self.providers {
                // Validate auth config references secrets correctly
                match &provider.auth {
                    AuthConfig::ApiKey { key, .. } => {
                        if key.starts_with("${secret:") && !key.ends_with("}") {
                            return Err(ConfigValidationError::InvalidSecretReference {
                                provider: provider.id.clone(),
                                reference: key.clone(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// Calculate configuration checksum
    pub fn calculate_checksum(&self) -> String {
        use sha2::{Sha256, Digest};

        let serialized = serde_json::to_string(self)
            .unwrap_or_default();

        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigValidationError {
    #[error("Schema validation failed: {0}")]
    SchemaValidation(String),

    #[error("Invalid reference in {field}: {reference}")]
    InvalidReference { field: &'static str, reference: String },

    #[error("Duplicate ID in {field}: {id}")]
    DuplicateId { field: &'static str, id: String },

    #[error("Invalid secret reference in provider {provider}: {reference}")]
    InvalidSecretReference { provider: String, reference: String },

    #[error("Business rule violation: {0}")]
    BusinessRuleViolation(String),

    #[error("Configuration conflict: {0}")]
    Conflict(String),
}
```

### 1.2 Server Configuration

```rust
/// Server configuration for HTTP/gRPC endpoints
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ServerConfig {
    /// Server bind address
    #[validate(custom = "validate_bind_address")]
    pub bind_address: String,

    /// Server port
    #[validate(range(min = 1, max = 65535))]
    pub port: u16,

    /// Enable TLS
    #[serde(default)]
    pub tls_enabled: bool,

    /// TLS certificate path (file or secret reference)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_cert_path: Option<String>,

    /// TLS private key path (file or secret reference)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_key_path: Option<String>,

    /// Request timeout
    #[serde(with = "humantime_serde")]
    #[validate(custom = "validate_duration")]
    pub request_timeout: Duration,

    /// Keep-alive timeout
    #[serde(with = "humantime_serde")]
    pub keep_alive_timeout: Duration,

    /// Maximum concurrent connections
    #[validate(range(min = 1))]
    pub max_connections: usize,

    /// Request body size limit (bytes)
    #[validate(range(min = 1024))] // At least 1KB
    pub max_body_size: usize,

    /// Enable HTTP/2
    #[serde(default = "default_true")]
    pub http2_enabled: bool,

    /// Enable gRPC endpoint
    #[serde(default)]
    pub grpc_enabled: bool,

    /// gRPC port (if different from HTTP)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_port: Option<u16>,

    /// Graceful shutdown timeout
    #[serde(with = "humantime_serde")]
    pub shutdown_timeout: Duration,

    /// CORS configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cors: Option<CorsConfig>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
            request_timeout: Duration::from_secs(60),
            keep_alive_timeout: Duration::from_secs(90),
            max_connections: 10000,
            max_body_size: 10 * 1024 * 1024, // 10MB
            http2_enabled: true,
            grpc_enabled: false,
            grpc_port: None,
            shutdown_timeout: Duration::from_secs(30),
            cors: None,
        }
    }
}

fn default_true() -> bool {
    true
}

fn validate_bind_address(address: &str) -> Result<(), validator::ValidationError> {
    if address == "0.0.0.0" || address == "127.0.0.1" || address == "localhost" {
        return Ok(());
    }

    // Validate as IP address
    address.parse::<std::net::IpAddr>()
        .map(|_| ())
        .map_err(|_| validator::ValidationError::new("invalid_ip_address"))
}

fn validate_duration(duration: &Duration) -> Result<(), validator::ValidationError> {
    if duration.as_secs() == 0 {
        return Err(validator::ValidationError::new("duration_too_short"));
    }
    Ok(())
}

/// CORS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    /// Allowed origins
    pub allowed_origins: Vec<String>,

    /// Allowed methods
    pub allowed_methods: Vec<String>,

    /// Allowed headers
    pub allowed_headers: Vec<String>,

    /// Expose headers
    #[serde(default)]
    pub expose_headers: Vec<String>,

    /// Max age for preflight cache
    #[serde(with = "humantime_serde")]
    pub max_age: Duration,

    /// Allow credentials
    #[serde(default)]
    pub allow_credentials: bool,
}
```

### 1.3 Routing Configuration

```rust
/// Routing configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RoutingConfig {
    /// Routing policy with rules
    #[validate(nested)]
    pub policy: RoutingPolicy,

    /// Default load balancing strategy
    pub default_strategy: RoutingStrategy,

    /// Enable automatic failover
    #[serde(default = "default_true")]
    pub enable_failover: bool,

    /// Failover timeout
    #[serde(with = "humantime_serde")]
    pub failover_timeout: Duration,

    /// Enable request caching
    #[serde(default)]
    pub enable_caching: bool,

    /// Cache configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(nested)]
    pub cache: Option<CacheConfig>,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            policy: RoutingPolicy {
                id: "default".to_string(),
                name: "Default Policy".to_string(),
                version: "1.0".to_string(),
                rules: Vec::new(),
                default_strategy: RoutingStrategy::RoundRobin,
                default_provider_filter: None,
                updated_at: chrono::Utc::now(),
            },
            default_strategy: RoutingStrategy::RoundRobin,
            enable_failover: true,
            failover_timeout: Duration::from_millis(100),
            enable_caching: false,
            cache: None,
        }
    }
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CacheConfig {
    /// Cache backend type
    pub backend: CacheBackend,

    /// Default TTL for cached responses
    #[serde(with = "humantime_serde")]
    pub default_ttl: Duration,

    /// Maximum cache size (entries)
    #[validate(range(min = 1))]
    pub max_entries: usize,

    /// Enable semantic caching (similarity-based)
    #[serde(default)]
    pub semantic_caching: bool,

    /// Similarity threshold for semantic cache hits (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity_threshold: Option<f32>,

    /// Redis configuration (if backend is Redis)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redis: Option<RedisConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheBackend {
    Memory,
    Redis,
    Memcached,
}

/// Redis configuration for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis connection URL
    pub url: String,

    /// Connection pool size
    pub pool_size: usize,

    /// Connection timeout
    #[serde(with = "humantime_serde")]
    pub connect_timeout: Duration,

    /// Key prefix
    #[serde(default = "default_redis_prefix")]
    pub key_prefix: String,
}

fn default_redis_prefix() -> String {
    "llm_gateway:cache:".to_string()
}
```

### 1.4 Resilience Configuration

```rust
/// Resilience configuration (circuit breakers, retries, timeouts)
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ResilienceConfig {
    /// Global circuit breaker configuration
    #[validate(nested)]
    pub circuit_breaker: CircuitBreakerConfig,

    /// Global retry policy
    #[validate(nested)]
    pub retry_policy: RetryPolicy,

    /// Global timeout configuration
    #[validate(nested)]
    pub timeouts: TimeoutConfig,

    /// Health check configuration
    #[validate(nested)]
    pub health_check: HealthCheckConfig,

    /// Enable request queuing
    #[serde(default = "default_true")]
    pub enable_queuing: bool,

    /// Maximum queue size per provider
    #[validate(range(min = 1))]
    pub max_queue_size: usize,

    /// Queue timeout
    #[serde(with = "humantime_serde")]
    pub queue_timeout: Duration,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            circuit_breaker: CircuitBreakerConfig::default(),
            retry_policy: RetryPolicy::default(),
            timeouts: TimeoutConfig::default(),
            health_check: HealthCheckConfig::default(),
            enable_queuing: true,
            max_queue_size: 10000,
            queue_timeout: Duration::from_secs(30),
        }
    }
}
```

### 1.5 Observability Configuration

```rust
/// Observability configuration (logging, metrics, tracing)
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ObservabilityConfig {
    /// Logging configuration
    #[validate(nested)]
    pub logging: LoggingConfig,

    /// Metrics configuration
    #[validate(nested)]
    pub metrics: MetricsConfig,

    /// Distributed tracing configuration
    #[validate(nested)]
    pub tracing: TracingConfig,

    /// Audit logging configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(nested)]
    pub audit: Option<AuditConfig>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            logging: LoggingConfig::default(),
            metrics: MetricsConfig::default(),
            tracing: TracingConfig::default(),
            audit: None,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct LoggingConfig {
    /// Log level (error, warn, info, debug, trace)
    pub level: LogLevel,

    /// Log format (json, text)
    pub format: LogFormat,

    /// Enable request/response logging
    #[serde(default = "default_true")]
    pub log_requests: bool,

    /// Enable PII redaction
    #[serde(default = "default_true")]
    pub redact_pii: bool,

    /// Fields to redact
    #[serde(default)]
    pub redact_fields: Vec<String>,

    /// Log output destinations
    pub outputs: Vec<LogOutput>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            format: LogFormat::Json,
            log_requests: true,
            redact_pii: true,
            redact_fields: vec![
                "api_key".to_string(),
                "token".to_string(),
                "password".to_string(),
            ],
            outputs: vec![LogOutput::Stdout],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Json,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LogOutput {
    Stdout,
    Stderr,
    File { path: String },
    Syslog { address: String },
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct MetricsConfig {
    /// Enable metrics export
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Metrics backend
    pub backend: MetricsBackend,

    /// Metrics export endpoint
    pub endpoint: String,

    /// Export interval
    #[serde(with = "humantime_serde")]
    pub export_interval: Duration,

    /// Histogram buckets for latency metrics
    #[serde(default = "default_latency_buckets")]
    pub latency_buckets: Vec<f64>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: MetricsBackend::Prometheus,
            endpoint: "/metrics".to_string(),
            export_interval: Duration::from_secs(15),
            latency_buckets: default_latency_buckets(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricsBackend {
    Prometheus,
    OpenTelemetry,
    Datadog,
    Statsd,
}

fn default_latency_buckets() -> Vec<f64> {
    vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
}

/// Tracing configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TracingConfig {
    /// Enable distributed tracing
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Tracing backend
    pub backend: TracingBackend,

    /// Trace exporter endpoint
    pub endpoint: String,

    /// Sampling rate (0.0 - 1.0)
    #[validate(range(min = 0.0, max = 1.0))]
    pub sampling_rate: f64,

    /// Enable trace context propagation
    #[serde(default = "default_true")]
    pub propagate_context: bool,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: TracingBackend::OpenTelemetry,
            endpoint: "http://localhost:4317".to_string(),
            sampling_rate: 1.0,
            propagate_context: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TracingBackend {
    OpenTelemetry,
    Jaeger,
    Zipkin,
}

/// Audit logging configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AuditConfig {
    /// Enable audit logging
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Audit events to log
    pub events: Vec<AuditEventType>,

    /// Audit log output
    pub output: AuditOutput,

    /// Include request/response bodies
    #[serde(default)]
    pub include_bodies: bool,

    /// Redact sensitive fields
    #[serde(default = "default_true")]
    pub redact_sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuditOutput {
    File { path: String },
    Database { connection_string: String },
    S3 { bucket: String, prefix: String },
}
```

### 1.6 Security Configuration

```rust
/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SecurityConfig {
    /// Authentication configuration
    #[validate(nested)]
    pub authentication: AuthenticationConfig,

    /// Authorization configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(nested)]
    pub authorization: Option<AuthorizationConfig>,

    /// Rate limiting configuration
    #[validate(nested)]
    pub rate_limiting: RateLimitingConfig,

    /// TLS configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(nested)]
    pub tls: Option<TlsConfig>,

    /// Request validation
    #[serde(default = "default_true")]
    pub validate_requests: bool,

    /// Content filtering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_filtering: Option<ContentFilteringConfig>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            authentication: AuthenticationConfig::default(),
            authorization: None,
            rate_limiting: RateLimitingConfig::default(),
            tls: None,
            validate_requests: true,
            content_filtering: None,
        }
    }
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AuthenticationConfig {
    /// Enable authentication
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Authentication providers
    pub providers: Vec<AuthProvider>,

    /// Default authentication provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<String>,

    /// API key header name
    #[serde(default = "default_api_key_header")]
    pub api_key_header: String,

    /// Bearer token header name
    #[serde(default = "default_bearer_header")]
    pub bearer_token_header: String,
}

impl Default for AuthenticationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            providers: Vec::new(),
            default_provider: None,
            api_key_header: default_api_key_header(),
            bearer_token_header: default_bearer_header(),
        }
    }
}

fn default_api_key_header() -> String {
    "X-API-Key".to_string()
}

fn default_bearer_header() -> String {
    "Authorization".to_string()
}

/// Authentication provider
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthProvider {
    ApiKey {
        name: String,
        keys: Vec<ApiKeyConfig>,
    },
    OAuth2 {
        name: String,
        issuer: String,
        audience: String,
        jwks_uri: String,
    },
    Jwt {
        name: String,
        secret: String,
        algorithm: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    pub key: String,
    pub description: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub metadata: HashMap<String, String>,
}

/// Authorization configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AuthorizationConfig {
    /// Enable authorization
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Authorization policies
    pub policies: Vec<AuthorizationPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationPolicy {
    pub id: String,
    pub name: String,
    pub subjects: Vec<String>, // User IDs, roles, or groups
    pub resources: Vec<String>, // Models, providers, or endpoints
    pub actions: Vec<String>,   // read, write, execute
    pub effect: PolicyEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicyEffect {
    Allow,
    Deny,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RateLimitingConfig {
    /// Enable rate limiting
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Global rate limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global: Option<RateLimitRule>,

    /// Per-user rate limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_user: Option<RateLimitRule>,

    /// Per-tenant rate limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_tenant: Option<RateLimitRule>,

    /// Custom rate limit rules
    #[serde(default)]
    pub custom_rules: Vec<CustomRateLimitRule>,

    /// Rate limit backend
    pub backend: RateLimitBackend,
}

impl Default for RateLimitingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            global: Some(RateLimitRule {
                requests_per_minute: 1000,
                requests_per_hour: Some(50000),
                requests_per_day: None,
                burst: Some(100),
            }),
            per_user: None,
            per_tenant: None,
            custom_rules: Vec::new(),
            backend: RateLimitBackend::Memory,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitRule {
    pub requests_per_minute: u32,
    pub requests_per_hour: Option<u32>,
    pub requests_per_day: Option<u32>,
    pub burst: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRateLimitRule {
    pub id: String,
    pub matcher: RateLimitMatcher,
    pub limit: RateLimitRule,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RateLimitMatcher {
    Header { name: String, value: String },
    Path { pattern: String },
    Method { method: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitBackend {
    Memory,
    Redis,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TlsConfig {
    /// Minimum TLS version
    pub min_version: TlsVersion,

    /// Cipher suites (empty = default secure set)
    #[serde(default)]
    pub cipher_suites: Vec<String>,

    /// Client certificate authentication
    #[serde(default)]
    pub client_auth: bool,

    /// Client CA certificate path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_ca_cert_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsVersion {
    Tls12,
    Tls13,
}

/// Content filtering configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFilteringConfig {
    /// Enable PII detection
    pub detect_pii: bool,

    /// PII action
    pub pii_action: ContentFilterAction,

    /// Enable toxicity detection
    pub detect_toxicity: bool,

    /// Toxicity threshold (0.0-1.0)
    pub toxicity_threshold: f32,

    /// Toxicity action
    pub toxicity_action: ContentFilterAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFilterAction {
    Log,
    Redact,
    Reject,
}
```

### 1.7 Secrets Configuration

```rust
/// Secrets manager configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SecretsConfig {
    /// Secrets backend
    pub backend: SecretsBackend,

    /// Backend-specific configuration
    #[serde(flatten)]
    pub config: SecretsBackendConfig,

    /// Cache TTL for secrets
    #[serde(with = "humantime_serde")]
    pub cache_ttl: Duration,

    /// Enable secret rotation
    #[serde(default = "default_true")]
    pub enable_rotation: bool,

    /// Rotation check interval
    #[serde(with = "humantime_serde", skip_serializing_if = "Option::is_none")]
    pub rotation_interval: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretsBackend {
    Vault,
    AwsSecretsManager,
    GcpSecretManager,
    AzureKeyVault,
    Environment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "backend", rename_all = "snake_case")]
pub enum SecretsBackendConfig {
    Vault {
        address: String,
        token: Option<String>,
        namespace: Option<String>,
        mount_path: String,
    },
    AwsSecretsManager {
        region: String,
        role_arn: Option<String>,
    },
    GcpSecretManager {
        project_id: String,
        credentials_path: Option<String>,
    },
    AzureKeyVault {
        vault_url: String,
        tenant_id: String,
        client_id: String,
        client_secret: Option<String>,
    },
    Environment {
        prefix: Option<String>,
    },
}
```

### 1.8 Feature Flags

```rust
/// Feature flags for experimental or optional features
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeatureFlags {
    /// Enable semantic caching
    #[serde(default)]
    pub semantic_caching: bool,

    /// Enable speculative execution
    #[serde(default)]
    pub speculative_execution: bool,

    /// Enable A/B testing
    #[serde(default)]
    pub ab_testing: bool,

    /// Enable cost optimization
    #[serde(default)]
    pub cost_optimization: bool,

    /// Enable adaptive routing
    #[serde(default)]
    pub adaptive_routing: bool,

    /// Enable request deduplication
    #[serde(default)]
    pub request_deduplication: bool,

    /// Custom feature flags
    #[serde(default)]
    pub custom: HashMap<String, bool>,
}
```

---

## 2. Configuration Loader

### 2.1 Multi-Source Configuration Loader

```rust
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use anyhow::{Context, Result};

/// Configuration loader with multi-source support and priority merging
pub struct ConfigLoader {
    /// Configuration sources in priority order (higher priority first)
    sources: Vec<ConfigSource>,

    /// Secrets manager for resolving secret references
    secrets_manager: Option<Arc<dyn SecretsManager>>,

    /// Environment variable prefix
    env_prefix: String,
}

impl ConfigLoader {
    /// Create a new config loader
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            secrets_manager: None,
            env_prefix: "GATEWAY_".to_string(),
        }
    }

    /// Add a file source
    pub fn add_file<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.sources.push(ConfigSource::File(path.into()));
        self
    }

    /// Add environment variable source
    pub fn add_env(mut self) -> Self {
        self.sources.push(ConfigSource::Environment);
        self
    }

    /// Add remote configuration source
    pub fn add_remote(mut self, url: String, backend: RemoteBackend) -> Self {
        self.sources.push(ConfigSource::Remote { url, backend });
        self
    }

    /// Set secrets manager
    pub fn with_secrets_manager(mut self, manager: Arc<dyn SecretsManager>) -> Self {
        self.secrets_manager = Some(manager);
        self
    }

    /// Set environment variable prefix
    pub fn with_env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.env_prefix = prefix.into();
        self
    }

    /// Load configuration from all sources with priority merging
    pub async fn load(&self) -> Result<GatewayConfig> {
        let mut merged_config: Option<serde_json::Value> = None;

        // Load from each source in reverse priority order (lowest to highest)
        for source in self.sources.iter().rev() {
            let config_value = self.load_from_source(source).await
                .with_context(|| format!("Failed to load config from {:?}", source))?;

            merged_config = match merged_config {
                None => Some(config_value),
                Some(base) => Some(self.merge_configs(base, config_value)),
            };
        }

        let config_value = merged_config
            .ok_or_else(|| anyhow::anyhow!("No configuration sources available"))?;

        // Deserialize to GatewayConfig
        let mut config: GatewayConfig = serde_json::from_value(config_value)
            .context("Failed to deserialize configuration")?;

        // Resolve secret references
        if let Some(ref secrets_manager) = self.secrets_manager {
            self.resolve_secrets(&mut config, secrets_manager).await?;
        }

        // Validate configuration
        config.validate_full()
            .context("Configuration validation failed")?;

        Ok(config)
    }

    /// Load configuration from a single source
    async fn load_from_source(&self, source: &ConfigSource) -> Result<serde_json::Value> {
        match source {
            ConfigSource::File(path) => self.load_from_file(path).await,
            ConfigSource::Environment => self.load_from_env(),
            ConfigSource::Remote { url, backend } => self.load_from_remote(url, backend).await,
        }
    }

    /// Load from file (YAML or TOML)
    async fn load_from_file(&self, path: &Path) -> Result<serde_json::Value> {
        let content = fs::read_to_string(path).await
            .with_context(|| format!("Failed to read config file: {:?}", path))?;

        let extension = path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("yaml");

        match extension {
            "yaml" | "yml" => {
                let yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)
                    .context("Failed to parse YAML")?;
                serde_json::to_value(yaml_value)
                    .context("Failed to convert YAML to JSON")
            }
            "toml" => {
                let toml_value: toml::Value = toml::from_str(&content)
                    .context("Failed to parse TOML")?;
                serde_json::to_value(toml_value)
                    .context("Failed to convert TOML to JSON")
            }
            "json" => {
                serde_json::from_str(&content)
                    .context("Failed to parse JSON")
            }
            ext => Err(anyhow::anyhow!("Unsupported file extension: {}", ext)),
        }
    }

    /// Load from environment variables
    fn load_from_env(&self) -> Result<serde_json::Value> {
        let mut config = serde_json::Map::new();

        for (key, value) in std::env::vars() {
            if !key.starts_with(&self.env_prefix) {
                continue;
            }

            // Remove prefix and convert to lowercase
            let key_without_prefix = key.strip_prefix(&self.env_prefix).unwrap();
            let path = key_without_prefix.to_lowercase().replace("__", ".");

            // Set nested value
            self.set_nested_value(&mut config, &path, value);
        }

        Ok(serde_json::Value::Object(config))
    }

    /// Set nested value in JSON object from dot-separated path
    fn set_nested_value(&self, obj: &mut serde_json::Map<String, serde_json::Value>, path: &str, value: String) {
        let parts: Vec<&str> = path.split('.').collect();

        if parts.is_empty() {
            return;
        }

        let mut current = obj;

        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // Last part - set the value
                let parsed_value = self.parse_env_value(&value);
                current.insert(part.to_string(), parsed_value);
            } else {
                // Intermediate part - create nested object
                current = current
                    .entry(part.to_string())
                    .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
                    .as_object_mut()
                    .unwrap();
            }
        }
    }

    /// Parse environment variable value to JSON value
    fn parse_env_value(&self, value: &str) -> serde_json::Value {
        // Try to parse as JSON first
        if let Ok(json_value) = serde_json::from_str(value) {
            return json_value;
        }

        // Try to parse as number
        if let Ok(num) = value.parse::<i64>() {
            return serde_json::Value::Number(num.into());
        }

        if let Ok(num) = value.parse::<f64>() {
            if let Some(num) = serde_json::Number::from_f64(num) {
                return serde_json::Value::Number(num);
            }
        }

        // Try to parse as boolean
        match value.to_lowercase().as_str() {
            "true" => return serde_json::Value::Bool(true),
            "false" => return serde_json::Value::Bool(false),
            _ => {}
        }

        // Default to string
        serde_json::Value::String(value.to_string())
    }

    /// Load from remote source (etcd, Consul, etc.)
    async fn load_from_remote(&self, url: &str, backend: &RemoteBackend) -> Result<serde_json::Value> {
        match backend {
            RemoteBackend::Etcd => self.load_from_etcd(url).await,
            RemoteBackend::Consul => self.load_from_consul(url).await,
            RemoteBackend::Http => self.load_from_http(url).await,
        }
    }

    /// Load from etcd
    async fn load_from_etcd(&self, _url: &str) -> Result<serde_json::Value> {
        // TODO: Implement etcd client integration
        Err(anyhow::anyhow!("etcd backend not yet implemented"))
    }

    /// Load from Consul
    async fn load_from_consul(&self, _url: &str) -> Result<serde_json::Value> {
        // TODO: Implement Consul client integration
        Err(anyhow::anyhow!("Consul backend not yet implemented"))
    }

    /// Load from HTTP endpoint
    async fn load_from_http(&self, url: &str) -> Result<serde_json::Value> {
        let client = reqwest::Client::new();
        let response = client.get(url).send().await
            .context("Failed to fetch remote config")?;

        response.json::<serde_json::Value>().await
            .context("Failed to parse remote config as JSON")
    }

    /// Merge two configuration JSON values (right takes priority)
    fn merge_configs(&self, mut base: serde_json::Value, override_value: serde_json::Value) -> serde_json::Value {
        match (&mut base, override_value) {
            (serde_json::Value::Object(base_map), serde_json::Value::Object(override_map)) => {
                for (key, value) in override_map {
                    if let Some(base_value) = base_map.get_mut(&key) {
                        *base_value = self.merge_configs(base_value.clone(), value);
                    } else {
                        base_map.insert(key, value);
                    }
                }
                base
            }
            (_, override_value) => override_value,
        }
    }

    /// Resolve secret references in configuration
    async fn resolve_secrets(&self, config: &mut GatewayConfig, secrets_manager: &Arc<dyn SecretsManager>) -> Result<()> {
        // Resolve provider secrets
        for provider in &mut config.providers {
            if let AuthConfig::ApiKey { key, .. } = &mut provider.auth {
                if key.starts_with("${secret:") && key.ends_with("}") {
                    let secret_path = &key[9..key.len()-1]; // Extract path from ${secret:path}
                    let secret_value = secrets_manager.get_secret(secret_path).await
                        .with_context(|| format!("Failed to resolve secret: {}", secret_path))?;
                    *key = secret_value;
                }
            }
        }

        // Resolve TLS secrets
        if let Some(ref tls_cert_path) = config.server.tls_cert_path {
            if tls_cert_path.starts_with("${secret:") {
                let secret_path = &tls_cert_path[9..tls_cert_path.len()-1];
                let secret_value = secrets_manager.get_secret(secret_path).await?;
                config.server.tls_cert_path = Some(secret_value);
            }
        }

        if let Some(ref tls_key_path) = config.server.tls_key_path {
            if tls_key_path.starts_with("${secret:") {
                let secret_path = &tls_key_path[9..tls_key_path.len()-1];
                let secret_value = secrets_manager.get_secret(secret_path).await?;
                config.server.tls_key_path = Some(secret_value);
            }
        }

        Ok(())
    }
}

/// Configuration source
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// File (YAML, TOML, JSON)
    File(PathBuf),

    /// Environment variables
    Environment,

    /// Remote configuration store
    Remote {
        url: String,
        backend: RemoteBackend,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteBackend {
    Etcd,
    Consul,
    Http,
}
```

---

## 3. Configuration Validator

### 3.1 Comprehensive Validation Engine

```rust
use validator::{Validate, ValidationErrors};
use jsonschema::{JSONSchema, Draft};

/// Configuration validator with JSON Schema and business rule validation
pub struct ConfigValidator {
    /// JSON Schema for structural validation
    schema: Option<JSONSchema>,

    /// Business rule validators
    rule_validators: Vec<Box<dyn BusinessRuleValidator>>,
}

impl ConfigValidator {
    /// Create a new validator
    pub fn new() -> Self {
        Self {
            schema: None,
            rule_validators: Vec::new(),
        }
    }

    /// Load JSON Schema from file
    pub fn with_json_schema(mut self, schema_path: &Path) -> Result<Self> {
        let schema_content = std::fs::read_to_string(schema_path)
            .context("Failed to read schema file")?;

        let schema_value: serde_json::Value = serde_json::from_str(&schema_content)
            .context("Failed to parse schema JSON")?;

        let compiled_schema = JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&schema_value)
            .map_err(|e| anyhow::anyhow!("Failed to compile JSON schema: {}", e))?;

        self.schema = Some(compiled_schema);
        Ok(self)
    }

    /// Add a business rule validator
    pub fn add_rule_validator(mut self, validator: Box<dyn BusinessRuleValidator>) -> Self {
        self.rule_validators.push(validator);
        self
    }

    /// Validate configuration
    pub fn validate(&self, config: &GatewayConfig) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // 1. Validate with JSON Schema
        if let Some(ref schema) = self.schema {
            let config_value = serde_json::to_value(config)
                .context("Failed to serialize config for schema validation")?;

            if let Err(errors) = schema.validate(&config_value) {
                for error in errors {
                    report.add_error(ValidationError::SchemaViolation {
                        path: error.instance_path.to_string(),
                        message: error.to_string(),
                    });
                }
            }
        }

        // 2. Validate with validator derive
        if let Err(validation_errors) = config.validate() {
            for (field, errors) in validation_errors.field_errors() {
                for error in errors {
                    report.add_error(ValidationError::FieldValidation {
                        field: field.to_string(),
                        message: error.to_string(),
                    });
                }
            }
        }

        // 3. Run full validation (cross-field checks)
        if let Err(e) = config.validate_full() {
            report.add_error(ValidationError::CrossFieldValidation {
                message: e.to_string(),
            });
        }

        // 4. Run business rule validators
        for validator in &self.rule_validators {
            if let Err(e) = validator.validate(config) {
                report.add_error(ValidationError::BusinessRule {
                    rule: validator.name().to_string(),
                    message: e.to_string(),
                });
            }
        }

        Ok(report)
    }
}

/// Validation report
#[derive(Debug, Clone)]
pub struct ValidationReport {
    errors: Vec<ValidationError>,
    warnings: Vec<ValidationWarning>,
}

impl ValidationReport {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn errors(&self) -> &[ValidationError] {
        &self.errors
    }

    pub fn warnings(&self) -> &[ValidationWarning] {
        &self.warnings
    }
}

#[derive(Debug, Clone)]
pub enum ValidationError {
    SchemaViolation {
        path: String,
        message: String,
    },
    FieldValidation {
        field: String,
        message: String,
    },
    CrossFieldValidation {
        message: String,
    },
    BusinessRule {
        rule: String,
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub category: String,
    pub message: String,
}

/// Business rule validator trait
pub trait BusinessRuleValidator: Send + Sync {
    /// Validator name
    fn name(&self) -> &str;

    /// Validate configuration
    fn validate(&self, config: &GatewayConfig) -> Result<()>;
}

/// Example: Provider redundancy validator
pub struct ProviderRedundancyValidator {
    min_providers: usize,
}

impl ProviderRedundancyValidator {
    pub fn new(min_providers: usize) -> Self {
        Self { min_providers }
    }
}

impl BusinessRuleValidator for ProviderRedundancyValidator {
    fn name(&self) -> &str {
        "provider_redundancy"
    }

    fn validate(&self, config: &GatewayConfig) -> Result<()> {
        let enabled_providers = config.providers.iter()
            .filter(|p| p.enabled)
            .count();

        if enabled_providers < self.min_providers {
            return Err(anyhow::anyhow!(
                "Insufficient provider redundancy: {} enabled providers, {} required",
                enabled_providers,
                self.min_providers
            ));
        }

        Ok(())
    }
}

/// Example: Model coverage validator
pub struct ModelCoverageValidator;

impl BusinessRuleValidator for ModelCoverageValidator {
    fn name(&self) -> &str {
        "model_coverage"
    }

    fn validate(&self, config: &GatewayConfig) -> Result<()> {
        // Ensure each routing rule references a model that exists in at least one provider
        for rule in &config.routing.policy.rules {
            // Check if rule has model-specific conditions
            if let RuleCondition::ModelName { pattern } = &rule.condition {
                let model_available = config.providers.iter()
                    .any(|p| p.models.iter().any(|m| {
                        glob::Pattern::new(pattern)
                            .ok()
                            .map(|pat| pat.matches(&m.id))
                            .unwrap_or(false)
                    }));

                if !model_available {
                    return Err(anyhow::anyhow!(
                        "Routing rule '{}' references model pattern '{}' that is not available in any provider",
                        rule.id,
                        pattern
                    ));
                }
            }
        }

        Ok(())
    }
}
```

---

## 4. Hot Reload Manager

### 4.1 File Watcher with Debouncing

```rust
use notify::{Watcher, RecursiveMode, Event, EventKind};
use tokio::sync::{mpsc, RwLock as TokioRwLock, broadcast};
use std::sync::Arc;
use std::time::{Duration, Instant};
use arc_swap::ArcSwap;

/// Hot reload manager with file watching and atomic config updates
pub struct HotReloadManager {
    /// Current configuration (atomic swap for zero-downtime updates)
    current_config: Arc<ArcSwap<GatewayConfig>>,

    /// Configuration file path
    config_path: PathBuf,

    /// Configuration loader
    loader: Arc<ConfigLoader>,

    /// Configuration validator
    validator: Arc<ConfigValidator>,

    /// File watcher
    watcher: Option<Box<dyn Watcher>>,

    /// Event channel
    event_tx: mpsc::UnboundedSender<ReloadEvent>,
    event_rx: Arc<TokioRwLock<mpsc::UnboundedReceiver<ReloadEvent>>>,

    /// Subscriber broadcast channel
    subscriber_tx: broadcast::Sender<ConfigChangeNotification>,

    /// Debounce duration
    debounce_duration: Duration,

    /// Last reload timestamp
    last_reload: Arc<TokioRwLock<Option<Instant>>>,

    /// Reload history (for rollback)
    history: Arc<TokioRwLock<ConfigHistory>>,

    /// Audit logger
    audit_logger: Arc<dyn AuditLogger>,
}

impl HotReloadManager {
    /// Create a new hot reload manager
    pub fn new(
        initial_config: GatewayConfig,
        config_path: PathBuf,
        loader: Arc<ConfigLoader>,
        validator: Arc<ConfigValidator>,
        audit_logger: Arc<dyn AuditLogger>,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (subscriber_tx, _) = broadcast::channel(1000);

        Self {
            current_config: Arc::new(ArcSwap::new(Arc::new(initial_config))),
            config_path,
            loader,
            validator,
            watcher: None,
            event_tx,
            event_rx: Arc::new(TokioRwLock::new(event_rx)),
            subscriber_tx,
            debounce_duration: Duration::from_millis(500),
            last_reload: Arc::new(TokioRwLock::new(None)),
            history: Arc::new(TokioRwLock::new(ConfigHistory::new(10))),
            audit_logger,
        }
    }

    /// Start watching for configuration changes
    pub async fn start_watching(&mut self) -> Result<()> {
        // Setup file watcher
        let event_tx = self.event_tx.clone();
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    if let EventKind::Modify(_) | EventKind::Create(_) = event.kind {
                        let _ = event_tx.send(ReloadEvent::FileChanged);
                    }
                }
                Err(e) => {
                    eprintln!("File watcher error: {:?}", e);
                }
            }
        })?;

        // Watch the config file directory
        let watch_path = self.config_path.parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid config path"))?;

        watcher.watch(watch_path, RecursiveMode::NonRecursive)?;

        self.watcher = Some(Box::new(watcher));

        // Start event processing loop
        self.start_event_loop().await;

        Ok(())
    }

    /// Start event processing loop with debouncing
    async fn start_event_loop(&self) {
        let event_rx = Arc::clone(&self.event_rx);
        let last_reload = Arc::clone(&self.last_reload);
        let debounce_duration = self.debounce_duration;
        let reload_manager = Arc::new(self.clone_for_reload());

        tokio::spawn(async move {
            let mut rx = event_rx.write().await;
            let mut pending_reload = false;

            loop {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        match event {
                            ReloadEvent::FileChanged => {
                                pending_reload = true;
                            }
                            ReloadEvent::ManualReload => {
                                if let Err(e) = reload_manager.reload_config().await {
                                    eprintln!("Manual reload failed: {:?}", e);
                                }
                            }
                        }
                    }
                    _ = tokio::time::sleep(debounce_duration), if pending_reload => {
                        // Check if enough time has passed since last reload
                        let should_reload = {
                            let last = last_reload.read().await;
                            match *last {
                                None => true,
                                Some(instant) => instant.elapsed() >= debounce_duration,
                            }
                        };

                        if should_reload {
                            if let Err(e) = reload_manager.reload_config().await {
                                eprintln!("Auto-reload failed: {:?}", e);
                            }
                            pending_reload = false;
                        }
                    }
                }
            }
        });
    }

    /// Helper to clone fields needed for reload task
    fn clone_for_reload(&self) -> HotReloadManagerReloadTask {
        HotReloadManagerReloadTask {
            current_config: Arc::clone(&self.current_config),
            config_path: self.config_path.clone(),
            loader: Arc::clone(&self.loader),
            validator: Arc::clone(&self.validator),
            subscriber_tx: self.subscriber_tx.clone(),
            last_reload: Arc::clone(&self.last_reload),
            history: Arc::clone(&self.history),
            audit_logger: Arc::clone(&self.audit_logger),
        }
    }

    /// Get current configuration (cheap Arc clone)
    pub fn current(&self) -> Arc<GatewayConfig> {
        self.current_config.load_full()
    }

    /// Subscribe to configuration change notifications
    pub fn subscribe(&self) -> broadcast::Receiver<ConfigChangeNotification> {
        self.subscriber_tx.subscribe()
    }

    /// Trigger manual reload
    pub fn trigger_reload(&self) -> Result<()> {
        self.event_tx.send(ReloadEvent::ManualReload)?;
        Ok(())
    }

    /// Rollback to previous configuration version
    pub async fn rollback(&self, version: usize) -> Result<()> {
        let mut history = self.history.write().await;

        let previous_config = history.get(version)
            .ok_or_else(|| anyhow::anyhow!("Version {} not found in history", version))?
            .clone();

        // Validate previous config
        let validation_report = self.validator.validate(&previous_config)?;
        if !validation_report.is_valid() {
            return Err(anyhow::anyhow!("Rollback validation failed: {:?}", validation_report.errors()));
        }

        // Calculate diff
        let current = self.current();
        let diff = ConfigDiff::compute(&current, &previous_config);

        // Swap configuration
        self.current_config.store(Arc::new(previous_config.clone()));

        // Audit log
        self.audit_logger.log_config_rollback(&current, &previous_config, version).await;

        // Notify subscribers
        let _ = self.subscriber_tx.send(ConfigChangeNotification {
            timestamp: chrono::Utc::now(),
            change_type: ConfigChangeType::Rollback,
            diff,
            restart_required: false,
        });

        Ok(())
    }
}

/// Reload task helper struct
struct HotReloadManagerReloadTask {
    current_config: Arc<ArcSwap<GatewayConfig>>,
    config_path: PathBuf,
    loader: Arc<ConfigLoader>,
    validator: Arc<ConfigValidator>,
    subscriber_tx: broadcast::Sender<ConfigChangeNotification>,
    last_reload: Arc<TokioRwLock<Option<Instant>>>,
    history: Arc<TokioRwLock<ConfigHistory>>,
    audit_logger: Arc<dyn AuditLogger>,
}

impl HotReloadManagerReloadTask {
    /// Reload configuration from disk
    async fn reload_config(&self) -> Result<()> {
        tracing::info!("Reloading configuration from {:?}", self.config_path);

        // Load new configuration
        let new_config = self.loader.load().await
            .context("Failed to load new configuration")?;

        // Validate new configuration
        let validation_report = self.validator.validate(&new_config)?;
        if !validation_report.is_valid() {
            return Err(anyhow::anyhow!(
                "Configuration validation failed: {:?}",
                validation_report.errors()
            ));
        }

        // Get current config
        let current_config = self.current_config.load_full();

        // Calculate diff
        let diff = ConfigDiff::compute(&current_config, &new_config);

        // Check if restart is required
        let restart_required = diff.requires_restart();

        // Add current config to history
        let mut history = self.history.write().await;
        history.add((*current_config).clone());
        drop(history);

        // Atomic swap to new configuration
        self.current_config.store(Arc::new(new_config.clone()));

        // Update last reload timestamp
        *self.last_reload.write().await = Some(Instant::now());

        // Audit log
        self.audit_logger.log_config_change(&current_config, &new_config, &diff).await;

        // Notify subscribers
        let notification = ConfigChangeNotification {
            timestamp: chrono::Utc::now(),
            change_type: if restart_required {
                ConfigChangeType::RestartRequired
            } else {
                ConfigChangeType::HotReload
            },
            diff,
            restart_required,
        };

        let _ = self.subscriber_tx.send(notification);

        tracing::info!("Configuration reloaded successfully");

        if restart_required {
            tracing::warn!("Configuration change requires restart for full effect");
        }

        Ok(())
    }
}

/// Reload events
#[derive(Debug, Clone)]
enum ReloadEvent {
    FileChanged,
    ManualReload,
}

/// Configuration change notification
#[derive(Debug, Clone)]
pub struct ConfigChangeNotification {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub change_type: ConfigChangeType,
    pub diff: ConfigDiff,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigChangeType {
    HotReload,
    RestartRequired,
    Rollback,
}

/// Configuration history for rollback
struct ConfigHistory {
    configs: Vec<GatewayConfig>,
    max_size: usize,
}

impl ConfigHistory {
    fn new(max_size: usize) -> Self {
        Self {
            configs: Vec::with_capacity(max_size),
            max_size,
        }
    }

    fn add(&mut self, config: GatewayConfig) {
        if self.configs.len() >= self.max_size {
            self.configs.remove(0);
        }
        self.configs.push(config);
    }

    fn get(&self, version: usize) -> Option<&GatewayConfig> {
        if version < self.configs.len() {
            Some(&self.configs[self.configs.len() - version - 1])
        } else {
            None
        }
    }
}
```

---

## 5. Secrets Manager Integration

### 5.1 Secrets Manager Trait and Implementations

```rust
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Secrets manager trait for retrieving secrets from various backends
#[async_trait]
pub trait SecretsManager: Send + Sync {
    /// Get a secret by path
    async fn get_secret(&self, path: &str) -> Result<String>;

    /// Get multiple secrets
    async fn get_secrets(&self, paths: &[&str]) -> Result<HashMap<String, String>>;

    /// Refresh cached secrets
    async fn refresh(&self) -> Result<()>;
}

/// HashiCorp Vault secrets manager
pub struct VaultSecretsManager {
    client: vaultrs::client::VaultClient,
    mount_path: String,
    cache: Arc<RwLock<SecretCache>>,
    cache_ttl: Duration,
}

impl VaultSecretsManager {
    /// Create a new Vault secrets manager
    pub async fn new(config: &SecretsBackendConfig) -> Result<Self> {
        let (address, token, namespace, mount_path) = match config {
            SecretsBackendConfig::Vault { address, token, namespace, mount_path } => {
                (address, token.as_ref(), namespace.as_ref(), mount_path)
            }
            _ => return Err(anyhow::anyhow!("Invalid config for Vault backend")),
        };

        let mut client = vaultrs::client::VaultClient::new(
            vaultrs::client::VaultClientSettingsBuilder::default()
                .address(address)
                .token(token.ok_or_else(|| anyhow::anyhow!("Vault token required"))?)
                .build()?
        )?;

        if let Some(ns) = namespace {
            client = client.with_namespace(ns);
        }

        Ok(Self {
            client,
            mount_path: mount_path.clone(),
            cache: Arc::new(RwLock::new(SecretCache::new())),
            cache_ttl: Duration::from_secs(300),
        })
    }
}

#[async_trait]
impl SecretsManager for VaultSecretsManager {
    async fn get_secret(&self, path: &str) -> Result<String> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached_value) = cache.get(path, self.cache_ttl) {
                return Ok(cached_value.clone());
            }
        }

        // Fetch from Vault
        let full_path = format!("{}/{}", self.mount_path, path);
        let secret: HashMap<String, String> = vaultrs::kv2::read(&self.client, &self.mount_path, path).await?;

        let value = secret.get("value")
            .ok_or_else(|| anyhow::anyhow!("Secret value not found at path: {}", full_path))?
            .clone();

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.set(path.to_string(), value.clone());
        }

        Ok(value)
    }

    async fn get_secrets(&self, paths: &[&str]) -> Result<HashMap<String, String>> {
        let mut results = HashMap::new();

        for path in paths {
            let value = self.get_secret(path).await?;
            results.insert(path.to_string(), value);
        }

        Ok(results)
    }

    async fn refresh(&self) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.clear();
        Ok(())
    }
}

/// AWS Secrets Manager implementation
pub struct AwsSecretsManager {
    client: aws_sdk_secretsmanager::Client,
    cache: Arc<RwLock<SecretCache>>,
    cache_ttl: Duration,
}

impl AwsSecretsManager {
    pub async fn new(config: &SecretsBackendConfig) -> Result<Self> {
        let region = match config {
            SecretsBackendConfig::AwsSecretsManager { region, .. } => region,
            _ => return Err(anyhow::anyhow!("Invalid config for AWS Secrets Manager")),
        };

        let aws_config = aws_config::from_env()
            .region(aws_config::Region::new(region.clone()))
            .load()
            .await;

        let client = aws_sdk_secretsmanager::Client::new(&aws_config);

        Ok(Self {
            client,
            cache: Arc::new(RwLock::new(SecretCache::new())),
            cache_ttl: Duration::from_secs(300),
        })
    }
}

#[async_trait]
impl SecretsManager for AwsSecretsManager {
    async fn get_secret(&self, path: &str) -> Result<String> {
        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some(cached_value) = cache.get(path, self.cache_ttl) {
                return Ok(cached_value.clone());
            }
        }

        // Fetch from AWS
        let response = self.client
            .get_secret_value()
            .secret_id(path)
            .send()
            .await?;

        let value = response.secret_string()
            .ok_or_else(|| anyhow::anyhow!("Secret string not found: {}", path))?
            .to_string();

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.set(path.to_string(), value.clone());
        }

        Ok(value)
    }

    async fn get_secrets(&self, paths: &[&str]) -> Result<HashMap<String, String>> {
        let mut results = HashMap::new();

        for path in paths {
            let value = self.get_secret(path).await?;
            results.insert(path.to_string(), value);
        }

        Ok(results)
    }

    async fn refresh(&self) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.clear();
        Ok(())
    }
}

/// Environment variable secrets manager
pub struct EnvSecretsManager {
    prefix: Option<String>,
}

impl EnvSecretsManager {
    pub fn new(config: &SecretsBackendConfig) -> Result<Self> {
        let prefix = match config {
            SecretsBackendConfig::Environment { prefix } => prefix.clone(),
            _ => return Err(anyhow::anyhow!("Invalid config for Environment backend")),
        };

        Ok(Self { prefix })
    }
}

#[async_trait]
impl SecretsManager for EnvSecretsManager {
    async fn get_secret(&self, path: &str) -> Result<String> {
        let env_var_name = if let Some(ref prefix) = self.prefix {
            format!("{}{}", prefix, path.to_uppercase().replace('/', "_"))
        } else {
            path.to_uppercase().replace('/', "_")
        };

        std::env::var(&env_var_name)
            .with_context(|| format!("Environment variable not found: {}", env_var_name))
    }

    async fn get_secrets(&self, paths: &[&str]) -> Result<HashMap<String, String>> {
        let mut results = HashMap::new();

        for path in paths {
            let value = self.get_secret(path).await?;
            results.insert(path.to_string(), value);
        }

        Ok(results)
    }

    async fn refresh(&self) -> Result<()> {
        // No-op for environment variables
        Ok(())
    }
}

/// Secret cache with TTL
struct SecretCache {
    entries: HashMap<String, CachedSecret>,
}

struct CachedSecret {
    value: String,
    cached_at: Instant,
}

impl SecretCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    fn get(&self, key: &str, ttl: Duration) -> Option<&String> {
        self.entries.get(key).and_then(|entry| {
            if entry.cached_at.elapsed() < ttl {
                Some(&entry.value)
            } else {
                None
            }
        })
    }

    fn set(&mut self, key: String, value: String) {
        self.entries.insert(key, CachedSecret {
            value,
            cached_at: Instant::now(),
        });
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}
```

---

## 6. Configuration Diff and Versioning

### 6.1 Configuration Diff Calculator

```rust
use serde_json::Value as JsonValue;

/// Configuration diff calculator
pub struct ConfigDiff {
    /// Changed fields with old and new values
    pub changes: Vec<ConfigChange>,

    /// Added fields
    pub additions: Vec<String>,

    /// Removed fields
    pub removals: Vec<String>,
}

impl ConfigDiff {
    /// Compute diff between two configurations
    pub fn compute(old: &GatewayConfig, new: &GatewayConfig) -> Self {
        let old_json = serde_json::to_value(old).unwrap();
        let new_json = serde_json::to_value(new).unwrap();

        let mut changes = Vec::new();
        let mut additions = Vec::new();
        let mut removals = Vec::new();

        Self::diff_recursive("", &old_json, &new_json, &mut changes, &mut additions, &mut removals);

        Self {
            changes,
            additions,
            removals,
        }
    }

    /// Recursive diff of JSON values
    fn diff_recursive(
        path: &str,
        old: &JsonValue,
        new: &JsonValue,
        changes: &mut Vec<ConfigChange>,
        additions: &mut Vec<String>,
        removals: &mut Vec<String>,
    ) {
        match (old, new) {
            (JsonValue::Object(old_map), JsonValue::Object(new_map)) => {
                // Check for changes and removals
                for (key, old_value) in old_map {
                    let new_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path, key)
                    };

                    if let Some(new_value) = new_map.get(key) {
                        if old_value != new_value {
                            Self::diff_recursive(&new_path, old_value, new_value, changes, additions, removals);
                        }
                    } else {
                        removals.push(new_path);
                    }
                }

                // Check for additions
                for key in new_map.keys() {
                    if !old_map.contains_key(key) {
                        let new_path = if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{}.{}", path, key)
                        };
                        additions.push(new_path);
                    }
                }
            }
            (JsonValue::Array(old_arr), JsonValue::Array(new_arr)) => {
                if old_arr != new_arr {
                    changes.push(ConfigChange {
                        path: path.to_string(),
                        old_value: old.clone(),
                        new_value: new.clone(),
                        change_type: ChangeType::Modified,
                    });
                }
            }
            _ => {
                if old != new {
                    changes.push(ConfigChange {
                        path: path.to_string(),
                        old_value: old.clone(),
                        new_value: new.clone(),
                        change_type: ChangeType::Modified,
                    });
                }
            }
        }
    }

    /// Check if diff requires restart
    pub fn requires_restart(&self) -> bool {
        // Certain configuration changes require restart
        let restart_required_paths = [
            "server.bind_address",
            "server.port",
            "server.tls_enabled",
            "server.http2_enabled",
            "server.grpc_enabled",
            "observability.tracing.backend",
            "observability.metrics.backend",
        ];

        self.changes.iter().any(|change| {
            restart_required_paths.iter().any(|path| change.path.starts_with(path))
        }) || self.additions.iter().any(|path| {
            restart_required_paths.iter().any(|req_path| path.starts_with(req_path))
        })
    }

    /// Get summary of changes
    pub fn summary(&self) -> String {
        format!(
            "{} changes, {} additions, {} removals",
            self.changes.len(),
            self.additions.len(),
            self.removals.len()
        )
    }
}

/// Single configuration change
#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub path: String,
    pub old_value: JsonValue,
    pub new_value: JsonValue,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Modified,
    Added,
    Removed,
}
```

---

## 7. Configuration Sources

### 7.1 Kubernetes ConfigMap Integration

```rust
/// Kubernetes ConfigMap watcher
pub struct K8sConfigMapWatcher {
    client: kube::Client,
    namespace: String,
    configmap_name: String,
    reload_trigger: mpsc::UnboundedSender<ReloadEvent>,
}

impl K8sConfigMapWatcher {
    pub async fn new(
        namespace: String,
        configmap_name: String,
        reload_trigger: mpsc::UnboundedSender<ReloadEvent>,
    ) -> Result<Self> {
        let client = kube::Client::try_default().await?;

        Ok(Self {
            client,
            namespace,
            configmap_name,
            reload_trigger,
        })
    }

    pub async fn start_watching(&self) -> Result<()> {
        use kube::runtime::watcher;
        use futures::StreamExt;

        let configmaps: kube::Api<k8s_openapi::api::core::v1::ConfigMap> =
            kube::Api::namespaced(self.client.clone(), &self.namespace);

        let watcher_config = watcher::Config::default()
            .fields(&format!("metadata.name={}", self.configmap_name));

        let mut stream = watcher(configmaps, watcher_config).boxed();

        let reload_trigger = self.reload_trigger.clone();

        tokio::spawn(async move {
            while let Some(event) = stream.next().await {
                match event {
                    Ok(watcher::Event::Applied(_)) | Ok(watcher::Event::Deleted(_)) => {
                        let _ = reload_trigger.send(ReloadEvent::ManualReload);
                    }
                    Err(e) => {
                        tracing::error!("ConfigMap watcher error: {:?}", e);
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }
}
```

---

## 8. Integration Patterns

### 8.1 Complete Usage Example

```rust
/// Complete configuration system initialization
pub async fn initialize_config_system(config_path: PathBuf) -> Result<(Arc<GatewayConfig>, HotReloadManager)> {
    // 1. Setup secrets manager
    let secrets_config = SecretsBackendConfig::Vault {
        address: "https://vault.example.com".to_string(),
        token: Some(std::env::var("VAULT_TOKEN")?),
        namespace: None,
        mount_path: "secret/gateway".to_string(),
    };

    let secrets_manager = Arc::new(
        VaultSecretsManager::new(&secrets_config).await?
    ) as Arc<dyn SecretsManager>;

    // 2. Setup configuration loader
    let loader = Arc::new(
        ConfigLoader::new()
            .add_file(&config_path)
            .add_env()
            .with_secrets_manager(Arc::clone(&secrets_manager))
            .with_env_prefix("GATEWAY_")
    );

    // 3. Setup configuration validator
    let validator = Arc::new(
        ConfigValidator::new()
            .with_json_schema(Path::new("config-schema.json"))?
            .add_rule_validator(Box::new(ProviderRedundancyValidator::new(2)))
            .add_rule_validator(Box::new(ModelCoverageValidator))
    );

    // 4. Load initial configuration
    let initial_config = loader.load().await?;

    // Validate
    let validation_report = validator.validate(&initial_config)?;
    if !validation_report.is_valid() {
        return Err(anyhow::anyhow!(
            "Configuration validation failed: {:?}",
            validation_report.errors()
        ));
    }

    // 5. Setup audit logger
    let audit_logger = Arc::new(FileAuditLogger::new("logs/config-audit.jsonl")?);

    // 6. Setup hot reload manager
    let mut reload_manager = HotReloadManager::new(
        initial_config.clone(),
        config_path,
        loader,
        validator,
        audit_logger,
    );

    // 7. Start watching for changes
    reload_manager.start_watching().await?;

    // 8. Subscribe to config changes
    let mut change_rx = reload_manager.subscribe();
    tokio::spawn(async move {
        while let Ok(notification) = change_rx.recv().await {
            tracing::info!(
                "Configuration changed: {:?} (restart_required: {})",
                notification.change_type,
                notification.restart_required
            );

            if notification.restart_required {
                tracing::warn!("Configuration change requires restart: {}", notification.diff.summary());
            }
        }
    });

    let config = reload_manager.current();

    Ok((config, reload_manager))
}

/// Audit logger trait
#[async_trait]
pub trait AuditLogger: Send + Sync {
    async fn log_config_change(&self, old: &GatewayConfig, new: &GatewayConfig, diff: &ConfigDiff);
    async fn log_config_rollback(&self, current: &GatewayConfig, rolled_back: &GatewayConfig, version: usize);
}

/// File-based audit logger
pub struct FileAuditLogger {
    file_path: PathBuf,
    writer: Arc<TokioRwLock<tokio::fs::File>>,
}

impl FileAuditLogger {
    pub fn new(file_path: impl Into<PathBuf>) -> Result<Self> {
        let file_path = file_path.into();

        // Create parent directory if needed
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        Ok(Self {
            file_path: file_path.clone(),
            writer: Arc::new(TokioRwLock::new(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(file_path)
                    .map(tokio::fs::File::from_std)?
            )),
        })
    }
}

#[async_trait]
impl AuditLogger for FileAuditLogger {
    async fn log_config_change(&self, old: &GatewayConfig, new: &GatewayConfig, diff: &ConfigDiff) {
        use tokio::io::AsyncWriteExt;

        let entry = serde_json::json!({
            "timestamp": chrono::Utc::now(),
            "event": "config_change",
            "diff": {
                "summary": diff.summary(),
                "changes": diff.changes.len(),
                "additions": diff.additions.len(),
                "removals": diff.removals.len(),
                "restart_required": diff.requires_restart(),
            },
            "old_version": old.version,
            "new_version": new.version,
            "old_checksum": old.calculate_checksum(),
            "new_checksum": new.calculate_checksum(),
        });

        let mut writer = self.writer.write().await;
        let _ = writer.write_all(entry.to_string().as_bytes()).await;
        let _ = writer.write_all(b"\n").await;
        let _ = writer.flush().await;
    }

    async fn log_config_rollback(&self, current: &GatewayConfig, rolled_back: &GatewayConfig, version: usize) {
        use tokio::io::AsyncWriteExt;

        let entry = serde_json::json!({
            "timestamp": chrono::Utc::now(),
            "event": "config_rollback",
            "version": version,
            "current_version": current.version,
            "rolled_back_version": rolled_back.version,
            "current_checksum": current.calculate_checksum(),
            "rolled_back_checksum": rolled_back.calculate_checksum(),
        });

        let mut writer = self.writer.write().await;
        let _ = writer.write_all(entry.to_string().as_bytes()).await;
        let _ = writer.write_all(b"\n").await;
        let _ = writer.flush().await;
    }
}
```

---

## Summary

This comprehensive pseudocode provides a production-ready, enterprise-grade Configuration and Hot Reload system for the LLM-Inference-Gateway with:

**Key Features:**
- **Comprehensive Configuration Schema**: Type-safe, validated configuration with all gateway subsystems
- **Multi-Source Loading**: Priority-based merging from files (YAML/TOML/JSON), environment variables, and remote stores
- **JSON Schema + Business Rule Validation**: Structural validation and cross-field consistency checks
- **Hot Reload with Debouncing**: File watching with atomic config swaps and zero-downtime updates
- **Secrets Manager Integration**: HashiCorp Vault, AWS Secrets Manager, environment variables with caching
- **Configuration Diff and Versioning**: Change detection, restart-required detection, and rollback capability
- **Audit Logging**: Complete audit trail of all configuration changes
- **Thread-Safe Access**: ArcSwap for lock-free reads, RwLock for controlled writes

**Production-Ready Patterns:**
- Zero-copy configuration access with Arc
- Atomic configuration updates with ArcSwap
- File watcher with debouncing to prevent reload storms
- Secret caching with TTL for performance
- Configuration history for rollback
- Subscriber notification pattern for reactive updates
- Comprehensive validation at multiple levels

The design follows Rust best practices and integrates seamlessly with the existing gateway architecture.
