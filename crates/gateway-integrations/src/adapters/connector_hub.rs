//! LLM-Connector-Hub adapter for provider routing.
//!
//! This adapter consumes provider routing information from LLM-Connector-Hub
//! to route requests to different model providers based on capabilities,
//! availability, and configuration.

use crate::config::ConnectorHubConfig;
use crate::error::{IntegrationError, IntegrationResult};
use crate::traits::{
    ProviderCredentials, ProviderHealthReport, ProviderInfo, ProviderRecommendation,
    ProviderRouter,
};
use async_trait::async_trait;
use dashmap::DashMap;
use gateway_core::GatewayRequest;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, instrument, warn};

/// Adapter for consuming provider routing from LLM-Connector-Hub.
///
/// This is a thin wrapper that consumes from the connector hub service
/// without modifying existing gateway routing logic.
pub struct ConnectorHubAdapter {
    /// Configuration
    config: ConnectorHubConfig,
    /// Cache for provider info
    provider_cache: DashMap<String, CachedProvider>,
    /// Last discovery timestamp
    last_discovery: std::sync::atomic::AtomicU64,
}

/// Cached provider information
struct CachedProvider {
    info: ProviderInfo,
    cached_at: Instant,
}

impl ConnectorHubAdapter {
    /// Create a new connector hub adapter.
    pub fn new(config: ConnectorHubConfig) -> Self {
        Self {
            config,
            provider_cache: DashMap::new(),
            last_discovery: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Check if the adapter is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the configured endpoint.
    pub fn endpoint(&self) -> Option<&str> {
        self.config.endpoint.as_deref()
    }

    /// Check if cache needs refresh.
    fn needs_refresh(&self) -> bool {
        let last = self.last_discovery.load(std::sync::atomic::Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        now - last > self.config.credential_refresh_interval.as_secs()
    }

    /// Update last discovery timestamp.
    fn update_discovery_timestamp(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.last_discovery.store(now, std::sync::atomic::Ordering::Relaxed);
    }
}

#[async_trait]
impl ProviderRouter for ConnectorHubAdapter {
    #[instrument(skip(self, request), fields(model = %request.model))]
    async fn get_provider_recommendation(
        &self,
        request: &GatewayRequest,
    ) -> IntegrationResult<ProviderRecommendation> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("connector-hub".to_string()));
        }

        debug!(
            model = %request.model,
            "Consuming provider recommendation from connector hub"
        );

        // In Phase 2B, we provide the integration interface.
        // Actual connector hub client implementation would go here.
        // For now, return a default recommendation that doesn't change behavior.

        Ok(ProviderRecommendation {
            provider_id: request
                .metadata
                .as_ref()
                .and_then(|m| m.preferred_provider.clone())
                .unwrap_or_else(|| "default".to_string()),
            confidence: 0.0, // Zero confidence indicates no recommendation
            fallbacks: request
                .metadata
                .as_ref()
                .and_then(|m| m.fallback_providers.clone())
                .unwrap_or_default(),
            reason: "Connector hub integration pending full implementation".to_string(),
            metadata: std::collections::HashMap::new(),
        })
    }

    #[instrument(skip(self))]
    async fn consume_provider_updates(&self) -> IntegrationResult<Vec<ProviderInfo>> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("connector-hub".to_string()));
        }

        if !self.config.auto_discover {
            debug!("Provider auto-discovery is disabled");
            return Ok(Vec::new());
        }

        debug!("Consuming provider updates from connector hub");

        // Return cached providers if available and fresh
        if !self.needs_refresh() {
            let providers: Vec<ProviderInfo> = self
                .provider_cache
                .iter()
                .map(|entry| entry.value().info.clone())
                .collect();

            if !providers.is_empty() {
                return Ok(providers);
            }
        }

        // Phase 2B: Integration interface ready.
        // Actual connector hub discovery would populate this.
        self.update_discovery_timestamp();

        Ok(Vec::new())
    }

    #[instrument(skip(self), fields(provider_id = %provider_id))]
    async fn get_provider_credentials(
        &self,
        provider_id: &str,
    ) -> IntegrationResult<ProviderCredentials> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("connector-hub".to_string()));
        }

        debug!(provider_id = %provider_id, "Fetching credentials from connector hub");

        // Phase 2B: Integration interface ready.
        // Actual credential fetching would go here.
        // Return an error indicating credentials should come from existing config.

        Err(IntegrationError::connector_hub(format!(
            "Credential management for provider '{}' via connector hub pending implementation",
            provider_id
        )))
    }

    #[instrument(skip(self, status), fields(provider_id = %status.provider_id))]
    async fn report_provider_health(
        &self,
        provider_id: &str,
        status: ProviderHealthReport,
    ) -> IntegrationResult<()> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("connector-hub".to_string()));
        }

        debug!(
            provider_id = %provider_id,
            healthy = status.healthy,
            latency_ms = ?status.latency_ms,
            "Reporting provider health to connector hub"
        );

        // Phase 2B: Health reporting interface ready.
        // Actual reporting would send to connector hub service.

        Ok(())
    }
}

impl std::fmt::Debug for ConnectorHubAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectorHubAdapter")
            .field("enabled", &self.config.enabled)
            .field("endpoint", &self.config.endpoint)
            .field("cached_providers", &self.provider_cache.len())
            .finish()
    }
}

/// Builder for `ConnectorHubAdapter`
pub struct ConnectorHubAdapterBuilder {
    config: ConnectorHubConfig,
}

impl ConnectorHubAdapterBuilder {
    /// Create a new builder with default config.
    pub fn new() -> Self {
        Self {
            config: ConnectorHubConfig::default(),
        }
    }

    /// Set the configuration.
    pub fn config(mut self, config: ConnectorHubConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable the adapter.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Set the endpoint.
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.endpoint = Some(endpoint.into());
        self
    }

    /// Build the adapter.
    pub fn build(self) -> ConnectorHubAdapter {
        ConnectorHubAdapter::new(self.config)
    }
}

impl Default for ConnectorHubAdapterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_disabled_by_default() {
        let adapter = ConnectorHubAdapter::new(ConnectorHubConfig::default());
        assert!(!adapter.is_enabled());
    }

    #[test]
    fn test_adapter_builder() {
        let adapter = ConnectorHubAdapterBuilder::new()
            .enabled(true)
            .endpoint("http://localhost:8080")
            .build();

        assert!(adapter.is_enabled());
        assert_eq!(adapter.endpoint(), Some("http://localhost:8080"));
    }

    #[tokio::test]
    async fn test_disabled_returns_not_enabled() {
        let adapter = ConnectorHubAdapter::new(ConnectorHubConfig::default());
        let request = gateway_core::GatewayRequest::builder()
            .model("gpt-4")
            .message(gateway_core::ChatMessage::user("test"))
            .build()
            .unwrap();

        let result = adapter.get_provider_recommendation(&request).await;
        assert!(matches!(result, Err(IntegrationError::NotEnabled(_))));
    }
}
