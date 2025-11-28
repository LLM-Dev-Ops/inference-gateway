//! Client configuration for the Gateway SDK.

use secrecy::{ExposeSecret, Secret};
use std::time::Duration;
use url::Url;

/// Configuration for the Gateway SDK client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Base URL of the gateway server.
    pub(crate) base_url: Url,
    /// API key for authentication.
    pub(crate) api_key: Option<Secret<String>>,
    /// Request timeout duration.
    pub(crate) timeout: Duration,
    /// Connection timeout duration.
    pub(crate) connect_timeout: Duration,
    /// Maximum number of retry attempts.
    pub(crate) max_retries: u32,
    /// Initial retry delay.
    pub(crate) retry_initial_delay: Duration,
    /// Maximum retry delay.
    pub(crate) retry_max_delay: Duration,
    /// User agent string.
    pub(crate) user_agent: String,
    /// Default model to use.
    pub(crate) default_model: Option<String>,
    /// Custom headers to include in requests.
    pub(crate) custom_headers: Vec<(String, String)>,
    /// Enable request tracing.
    pub(crate) enable_tracing: bool,
    /// Tenant ID for multi-tenant deployments.
    pub(crate) tenant_id: Option<String>,
}

impl ClientConfig {
    /// Default request timeout (30 seconds).
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
    /// Default connection timeout (10 seconds).
    pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
    /// Default maximum retries.
    pub const DEFAULT_MAX_RETRIES: u32 = 3;
    /// Default initial retry delay (1 second).
    pub const DEFAULT_RETRY_INITIAL_DELAY: Duration = Duration::from_secs(1);
    /// Default maximum retry delay (30 seconds).
    pub const DEFAULT_RETRY_MAX_DELAY: Duration = Duration::from_secs(30);
    /// Default user agent.
    pub const DEFAULT_USER_AGENT: &'static str = concat!(
        "gateway-sdk-rust/",
        env!("CARGO_PKG_VERSION")
    );

    /// Create a new configuration with default values.
    pub fn new(base_url: Url) -> Self {
        Self {
            base_url,
            api_key: None,
            timeout: Self::DEFAULT_TIMEOUT,
            connect_timeout: Self::DEFAULT_CONNECT_TIMEOUT,
            max_retries: Self::DEFAULT_MAX_RETRIES,
            retry_initial_delay: Self::DEFAULT_RETRY_INITIAL_DELAY,
            retry_max_delay: Self::DEFAULT_RETRY_MAX_DELAY,
            user_agent: Self::DEFAULT_USER_AGENT.to_string(),
            default_model: None,
            custom_headers: Vec::new(),
            enable_tracing: false,
            tenant_id: None,
        }
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Check if an API key is configured.
    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// Get the API key (exposed for use in requests).
    pub(crate) fn api_key_value(&self) -> Option<&str> {
        self.api_key.as_ref().map(|s| s.expose_secret().as_str())
    }

    /// Get the request timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Get the connection timeout.
    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    /// Get the maximum number of retries.
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }

    /// Get the user agent.
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Get the default model.
    pub fn default_model(&self) -> Option<&str> {
        self.default_model.as_deref()
    }

    /// Get custom headers.
    pub fn custom_headers(&self) -> &[(String, String)] {
        &self.custom_headers
    }

    /// Check if tracing is enabled.
    pub fn tracing_enabled(&self) -> bool {
        self.enable_tracing
    }

    /// Get the tenant ID.
    pub fn tenant_id(&self) -> Option<&str> {
        self.tenant_id.as_deref()
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self::new(
            Url::parse("http://localhost:8080").expect("valid default URL"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ClientConfig::default();
        assert_eq!(config.base_url.as_str(), "http://localhost:8080/");
        assert!(!config.has_api_key());
        assert_eq!(config.timeout(), ClientConfig::DEFAULT_TIMEOUT);
        assert_eq!(config.max_retries(), ClientConfig::DEFAULT_MAX_RETRIES);
    }

    #[test]
    fn test_config_with_custom_url() {
        let url = Url::parse("https://api.example.com").unwrap();
        let config = ClientConfig::new(url.clone());
        assert_eq!(config.base_url(), &url);
    }
}
