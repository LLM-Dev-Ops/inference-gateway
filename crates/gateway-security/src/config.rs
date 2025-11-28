//! Security configuration.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

/// Security configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Whether security features are enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Input validation configuration.
    #[serde(default)]
    pub validation: ValidationConfig,

    /// Security headers configuration.
    #[serde(default)]
    pub headers: HeadersConfig,

    /// IP filtering configuration.
    #[serde(default)]
    pub ip_filter: IpFilterSettings,

    /// Request signing configuration.
    #[serde(default)]
    pub signing: SigningConfig,

    /// Secrets configuration.
    #[serde(default)]
    pub secrets: SecretsConfig,

    /// Rate limiting configuration.
    #[serde(default)]
    pub rate_limit: RateLimitConfig,

    /// Content security configuration.
    #[serde(default)]
    pub content: ContentSecurityConfig,
}

fn default_enabled() -> bool {
    true
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            validation: ValidationConfig::default(),
            headers: HeadersConfig::default(),
            ip_filter: IpFilterSettings::default(),
            signing: SigningConfig::default(),
            secrets: SecretsConfig::default(),
            rate_limit: RateLimitConfig::default(),
            content: ContentSecurityConfig::default(),
        }
    }
}

impl SecurityConfig {
    /// Create a new security config builder.
    #[must_use]
    pub fn builder() -> SecurityConfigBuilder {
        SecurityConfigBuilder::default()
    }

    /// Create a strict security config.
    #[must_use]
    pub fn strict() -> Self {
        Self {
            enabled: true,
            validation: ValidationConfig::strict(),
            headers: HeadersConfig::strict(),
            ip_filter: IpFilterSettings::default(),
            signing: SigningConfig {
                enabled: true,
                ..Default::default()
            },
            secrets: SecretsConfig::default(),
            rate_limit: RateLimitConfig::strict(),
            content: ContentSecurityConfig::strict(),
        }
    }

    /// Create a permissive security config (for development).
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            enabled: true,
            validation: ValidationConfig::permissive(),
            headers: HeadersConfig::permissive(),
            ip_filter: IpFilterSettings::default(),
            signing: SigningConfig::default(),
            secrets: SecretsConfig::default(),
            rate_limit: RateLimitConfig::permissive(),
            content: ContentSecurityConfig::permissive(),
        }
    }
}

/// Builder for security configuration.
#[derive(Debug, Default)]
pub struct SecurityConfigBuilder {
    config: SecurityConfig,
}

impl SecurityConfigBuilder {
    /// Enable or disable security.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Set validation config.
    #[must_use]
    pub fn validation(mut self, config: ValidationConfig) -> Self {
        self.config.validation = config;
        self
    }

    /// Set headers config.
    #[must_use]
    pub fn headers(mut self, config: HeadersConfig) -> Self {
        self.config.headers = config;
        self
    }

    /// Set IP filter config.
    #[must_use]
    pub fn ip_filter(mut self, config: IpFilterSettings) -> Self {
        self.config.ip_filter = config;
        self
    }

    /// Set signing config.
    #[must_use]
    pub fn signing(mut self, config: SigningConfig) -> Self {
        self.config.signing = config;
        self
    }

    /// Set secrets config.
    #[must_use]
    pub fn secrets(mut self, config: SecretsConfig) -> Self {
        self.config.secrets = config;
        self
    }

    /// Set rate limit config.
    #[must_use]
    pub fn rate_limit(mut self, config: RateLimitConfig) -> Self {
        self.config.rate_limit = config;
        self
    }

    /// Set content security config.
    #[must_use]
    pub fn content(mut self, config: ContentSecurityConfig) -> Self {
        self.config.content = config;
        self
    }

    /// Build the configuration.
    #[must_use]
    pub fn build(self) -> SecurityConfig {
        self.config
    }
}

/// Input validation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    /// Maximum request body size in bytes.
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,

    /// Maximum string length.
    #[serde(default = "default_max_string_length")]
    pub max_string_length: usize,

    /// Maximum array length.
    #[serde(default = "default_max_array_length")]
    pub max_array_length: usize,

    /// Maximum nesting depth for JSON.
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,

    /// Allowed content types.
    #[serde(default = "default_content_types")]
    pub allowed_content_types: HashSet<String>,

    /// Whether to strip null bytes.
    #[serde(default = "default_true")]
    pub strip_null_bytes: bool,

    /// Whether to validate UTF-8.
    #[serde(default = "default_true")]
    pub validate_utf8: bool,
}

fn default_max_body_size() -> usize {
    10 * 1024 * 1024 // 10MB
}

fn default_max_string_length() -> usize {
    1_000_000 // 1MB for individual strings
}

fn default_max_array_length() -> usize {
    10_000
}

fn default_max_depth() -> usize {
    32
}

fn default_content_types() -> HashSet<String> {
    let mut set = HashSet::new();
    set.insert("application/json".to_string());
    set.insert("text/plain".to_string());
    set
}

fn default_true() -> bool {
    true
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_body_size: default_max_body_size(),
            max_string_length: default_max_string_length(),
            max_array_length: default_max_array_length(),
            max_depth: default_max_depth(),
            allowed_content_types: default_content_types(),
            strip_null_bytes: true,
            validate_utf8: true,
        }
    }
}

impl ValidationConfig {
    /// Create a strict validation config.
    #[must_use]
    pub fn strict() -> Self {
        Self {
            max_body_size: 1024 * 1024, // 1MB
            max_string_length: 100_000,
            max_array_length: 1000,
            max_depth: 16,
            allowed_content_types: {
                let mut set = HashSet::new();
                set.insert("application/json".to_string());
                set
            },
            strip_null_bytes: true,
            validate_utf8: true,
        }
    }

    /// Create a permissive validation config.
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            max_body_size: 100 * 1024 * 1024, // 100MB
            max_string_length: 10_000_000,
            max_array_length: 100_000,
            max_depth: 64,
            allowed_content_types: {
                let mut set = HashSet::new();
                set.insert("application/json".to_string());
                set.insert("text/plain".to_string());
                set.insert("application/x-www-form-urlencoded".to_string());
                set.insert("multipart/form-data".to_string());
                set
            },
            strip_null_bytes: true,
            validate_utf8: true,
        }
    }
}

/// Security headers configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadersConfig {
    /// Enable HSTS.
    #[serde(default = "default_true")]
    pub hsts_enabled: bool,

    /// HSTS max age in seconds.
    #[serde(default = "default_hsts_max_age")]
    pub hsts_max_age: u64,

    /// Include subdomains in HSTS.
    #[serde(default = "default_true")]
    pub hsts_include_subdomains: bool,

    /// HSTS preload.
    #[serde(default)]
    pub hsts_preload: bool,

    /// X-Content-Type-Options.
    #[serde(default = "default_true")]
    pub nosniff: bool,

    /// X-Frame-Options.
    #[serde(default = "default_frame_options")]
    pub frame_options: String,

    /// X-XSS-Protection.
    #[serde(default = "default_true")]
    pub xss_protection: bool,

    /// Content-Security-Policy.
    #[serde(default)]
    pub content_security_policy: Option<String>,

    /// Referrer-Policy.
    #[serde(default = "default_referrer_policy")]
    pub referrer_policy: String,

    /// Permissions-Policy.
    #[serde(default)]
    pub permissions_policy: Option<String>,

    /// Remove server header.
    #[serde(default = "default_true")]
    pub remove_server_header: bool,

    /// Custom headers.
    #[serde(default)]
    pub custom_headers: Vec<(String, String)>,
}

fn default_hsts_max_age() -> u64 {
    31536000 // 1 year
}

fn default_frame_options() -> String {
    "DENY".to_string()
}

fn default_referrer_policy() -> String {
    "strict-origin-when-cross-origin".to_string()
}

impl Default for HeadersConfig {
    fn default() -> Self {
        Self {
            hsts_enabled: true,
            hsts_max_age: default_hsts_max_age(),
            hsts_include_subdomains: true,
            hsts_preload: false,
            nosniff: true,
            frame_options: default_frame_options(),
            xss_protection: true,
            content_security_policy: Some("default-src 'self'".to_string()),
            referrer_policy: default_referrer_policy(),
            permissions_policy: None,
            remove_server_header: true,
            custom_headers: Vec::new(),
        }
    }
}

impl HeadersConfig {
    /// Create a strict headers config.
    #[must_use]
    pub fn strict() -> Self {
        Self {
            hsts_enabled: true,
            hsts_max_age: 63072000, // 2 years
            hsts_include_subdomains: true,
            hsts_preload: true,
            nosniff: true,
            frame_options: "DENY".to_string(),
            xss_protection: true,
            content_security_policy: Some(
                "default-src 'none'; script-src 'self'; connect-src 'self'; img-src 'self'; style-src 'self'; frame-ancestors 'none'; form-action 'self'".to_string()
            ),
            referrer_policy: "no-referrer".to_string(),
            permissions_policy: Some(
                "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()".to_string()
            ),
            remove_server_header: true,
            custom_headers: Vec::new(),
        }
    }

    /// Create a permissive headers config.
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            hsts_enabled: false,
            hsts_max_age: 0,
            hsts_include_subdomains: false,
            hsts_preload: false,
            nosniff: true,
            frame_options: "SAMEORIGIN".to_string(),
            xss_protection: true,
            content_security_policy: None,
            referrer_policy: "strict-origin-when-cross-origin".to_string(),
            permissions_policy: None,
            remove_server_header: false,
            custom_headers: Vec::new(),
        }
    }
}

/// IP filtering configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IpFilterSettings {
    /// Enable IP filtering.
    #[serde(default)]
    pub enabled: bool,

    /// IP allowlist (CIDR notation supported).
    #[serde(default)]
    pub allowlist: Vec<String>,

    /// IP blocklist (CIDR notation supported).
    #[serde(default)]
    pub blocklist: Vec<String>,

    /// Block private/internal IPs.
    #[serde(default)]
    pub block_private: bool,

    /// Block loopback addresses.
    #[serde(default)]
    pub block_loopback: bool,

    /// Allow localhost in development.
    #[serde(default = "default_true")]
    pub allow_localhost: bool,
}

/// Request signing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningConfig {
    /// Enable request signing.
    #[serde(default)]
    pub enabled: bool,

    /// Signing algorithm.
    #[serde(default = "default_algorithm")]
    pub algorithm: String,

    /// Header name for signature.
    #[serde(default = "default_signature_header")]
    pub signature_header: String,

    /// Header name for timestamp.
    #[serde(default = "default_timestamp_header")]
    pub timestamp_header: String,

    /// Signature validity duration.
    #[serde(with = "humantime_serde", default = "default_signature_validity")]
    pub validity_duration: Duration,

    /// Clock skew tolerance.
    #[serde(with = "humantime_serde", default = "default_clock_skew")]
    pub clock_skew: Duration,
}

fn default_algorithm() -> String {
    "HMAC-SHA256".to_string()
}

fn default_signature_header() -> String {
    "X-Signature".to_string()
}

fn default_timestamp_header() -> String {
    "X-Timestamp".to_string()
}

fn default_signature_validity() -> Duration {
    Duration::from_secs(300) // 5 minutes
}

fn default_clock_skew() -> Duration {
    Duration::from_secs(60) // 1 minute
}

impl Default for SigningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            algorithm: default_algorithm(),
            signature_header: default_signature_header(),
            timestamp_header: default_timestamp_header(),
            validity_duration: default_signature_validity(),
            clock_skew: default_clock_skew(),
        }
    }
}

/// Secrets management configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsConfig {
    /// Secrets backend.
    #[serde(default = "default_secrets_backend")]
    pub backend: String,

    /// Path for file-based secrets.
    #[serde(default)]
    pub path: Option<String>,

    /// Environment variable prefix.
    #[serde(default = "default_env_prefix")]
    pub env_prefix: String,

    /// Secret expiration.
    #[serde(with = "humantime_serde", default = "default_secret_expiry")]
    pub default_expiry: Duration,

    /// Enable secret rotation.
    #[serde(default)]
    pub rotation_enabled: bool,

    /// Rotation check interval.
    #[serde(with = "humantime_serde", default = "default_rotation_interval")]
    pub rotation_interval: Duration,
}

fn default_secrets_backend() -> String {
    "env".to_string()
}

fn default_env_prefix() -> String {
    "LLM_GATEWAY_".to_string()
}

fn default_secret_expiry() -> Duration {
    Duration::from_secs(86400 * 30) // 30 days
}

fn default_rotation_interval() -> Duration {
    Duration::from_secs(3600) // 1 hour
}

impl Default for SecretsConfig {
    fn default() -> Self {
        Self {
            backend: default_secrets_backend(),
            path: None,
            env_prefix: default_env_prefix(),
            default_expiry: default_secret_expiry(),
            rotation_enabled: false,
            rotation_interval: default_rotation_interval(),
        }
    }
}

/// Rate limiting configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Enable rate limiting.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Requests per window.
    #[serde(default = "default_rate_limit")]
    pub requests_per_window: u64,

    /// Window duration.
    #[serde(with = "humantime_serde", default = "default_rate_window")]
    pub window: Duration,

    /// Burst limit.
    #[serde(default = "default_burst")]
    pub burst: u64,

    /// Rate limit by IP.
    #[serde(default = "default_true")]
    pub by_ip: bool,

    /// Rate limit by API key.
    #[serde(default = "default_true")]
    pub by_api_key: bool,

    /// Rate limit by endpoint.
    #[serde(default)]
    pub by_endpoint: bool,

    /// Exempt paths.
    #[serde(default)]
    pub exempt_paths: Vec<String>,
}

fn default_rate_limit() -> u64 {
    1000
}

fn default_rate_window() -> Duration {
    Duration::from_secs(60)
}

fn default_burst() -> u64 {
    100
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_window: default_rate_limit(),
            window: default_rate_window(),
            burst: default_burst(),
            by_ip: true,
            by_api_key: true,
            by_endpoint: false,
            exempt_paths: vec!["/health".to_string(), "/metrics".to_string()],
        }
    }
}

impl RateLimitConfig {
    /// Create a strict rate limit config.
    #[must_use]
    pub fn strict() -> Self {
        Self {
            enabled: true,
            requests_per_window: 100,
            window: Duration::from_secs(60),
            burst: 10,
            by_ip: true,
            by_api_key: true,
            by_endpoint: true,
            exempt_paths: vec!["/health".to_string()],
        }
    }

    /// Create a permissive rate limit config.
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            enabled: true,
            requests_per_window: 10000,
            window: Duration::from_secs(60),
            burst: 1000,
            by_ip: true,
            by_api_key: false,
            by_endpoint: false,
            exempt_paths: Vec::new(),
        }
    }
}

/// Content security configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSecurityConfig {
    /// Enable XSS detection.
    #[serde(default = "default_true")]
    pub xss_detection: bool,

    /// Enable SQL injection detection.
    #[serde(default = "default_true")]
    pub sql_injection_detection: bool,

    /// Enable command injection detection.
    #[serde(default = "default_true")]
    pub command_injection_detection: bool,

    /// Enable path traversal detection.
    #[serde(default = "default_true")]
    pub path_traversal_detection: bool,

    /// Forbidden patterns (regex).
    #[serde(default)]
    pub forbidden_patterns: Vec<String>,

    /// Log security events.
    #[serde(default = "default_true")]
    pub log_events: bool,

    /// Block on detection.
    #[serde(default = "default_true")]
    pub block_on_detection: bool,
}

impl Default for ContentSecurityConfig {
    fn default() -> Self {
        Self {
            xss_detection: true,
            sql_injection_detection: true,
            command_injection_detection: true,
            path_traversal_detection: true,
            forbidden_patterns: Vec::new(),
            log_events: true,
            block_on_detection: true,
        }
    }
}

impl ContentSecurityConfig {
    /// Create a strict content security config.
    #[must_use]
    pub fn strict() -> Self {
        Self {
            xss_detection: true,
            sql_injection_detection: true,
            command_injection_detection: true,
            path_traversal_detection: true,
            forbidden_patterns: vec![
                r"(?i)<script".to_string(),
                r"(?i)javascript:".to_string(),
                r"(?i)on\w+\s*=".to_string(),
            ],
            log_events: true,
            block_on_detection: true,
        }
    }

    /// Create a permissive content security config.
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            xss_detection: false,
            sql_injection_detection: false,
            command_injection_detection: false,
            path_traversal_detection: true,
            forbidden_patterns: Vec::new(),
            log_events: true,
            block_on_detection: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SecurityConfig::default();
        assert!(config.enabled);
        assert!(config.validation.strip_null_bytes);
        assert!(config.headers.hsts_enabled);
    }

    #[test]
    fn test_strict_config() {
        let config = SecurityConfig::strict();
        assert!(config.enabled);
        assert!(config.signing.enabled);
        assert_eq!(config.validation.max_body_size, 1024 * 1024);
    }

    #[test]
    fn test_permissive_config() {
        let config = SecurityConfig::permissive();
        assert!(config.enabled);
        assert!(!config.headers.hsts_enabled);
    }

    #[test]
    fn test_builder() {
        let config = SecurityConfig::builder()
            .enabled(true)
            .validation(ValidationConfig::strict())
            .build();

        assert!(config.enabled);
        assert_eq!(config.validation.max_body_size, 1024 * 1024);
    }

    #[test]
    fn test_rate_limit_config() {
        let strict = RateLimitConfig::strict();
        assert_eq!(strict.requests_per_window, 100);

        let permissive = RateLimitConfig::permissive();
        assert_eq!(permissive.requests_per_window, 10000);
    }
}
