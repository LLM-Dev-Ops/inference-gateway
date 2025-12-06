//! LLM-Sentinel adapter for anomaly detection.
//!
//! This adapter consumes anomaly alerts from LLM-Sentinel
//! and triggers fallback behavior when anomalies are detected.

use crate::config::SentinelConfig;
use crate::error::{IntegrationError, IntegrationResult};
use crate::traits::{
    AnomalyAction, AnomalyAlert, AnomalyReport, AnomalyStatus, FallbackRecommendation,
    SentinelConsumer,
};
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, instrument, warn};

/// Adapter for consuming anomaly alerts from LLM-Sentinel.
///
/// This is a thin wrapper that consumes anomaly detection signals
/// and can trigger fallback behavior when anomalies are detected.
pub struct SentinelAdapter {
    /// Configuration
    config: SentinelConfig,
    /// Cache of provider anomaly status
    status_cache: DashMap<String, CachedStatus>,
}

/// Cached anomaly status
struct CachedStatus {
    status: AnomalyStatus,
    cached_at: std::time::Instant,
}

impl SentinelAdapter {
    /// Create a new sentinel adapter.
    pub fn new(config: SentinelConfig) -> Self {
        Self {
            config,
            status_cache: DashMap::new(),
        }
    }

    /// Check if the adapter is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the severity threshold.
    pub fn severity_threshold(&self) -> u8 {
        self.config.severity_threshold
    }

    /// Check if auto-fallback is enabled.
    pub fn auto_fallback_enabled(&self) -> bool {
        self.config.auto_fallback
    }

    /// Determine action based on severity.
    fn severity_to_action(&self, severity: u8) -> AnomalyAction {
        if severity >= 90 {
            AnomalyAction::Fallback
        } else if severity >= 70 {
            AnomalyAction::Avoid
        } else if severity >= 50 {
            AnomalyAction::Throttle
        } else if severity >= 30 {
            AnomalyAction::Caution
        } else {
            AnomalyAction::Continue
        }
    }
}

#[async_trait]
impl SentinelConsumer for SentinelAdapter {
    #[instrument(skip(self))]
    async fn consume_anomalies(&self) -> IntegrationResult<Vec<AnomalyAlert>> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("sentinel".to_string()));
        }

        if !self.config.consume_anomalies {
            debug!("Anomaly consumption is disabled");
            return Ok(Vec::new());
        }

        debug!("Consuming anomalies from sentinel");

        // Phase 2B: Anomaly consumption interface ready.
        // Actual sentinel client would fetch anomalies here.

        Ok(Vec::new())
    }

    #[instrument(skip(self), fields(provider_id = %provider_id))]
    async fn check_provider_anomalies(
        &self,
        provider_id: &str,
    ) -> IntegrationResult<AnomalyStatus> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("sentinel".to_string()));
        }

        debug!(provider_id = %provider_id, "Checking provider anomalies via sentinel");

        // Check cache first
        if let Some(cached) = self.status_cache.get(provider_id) {
            if cached.cached_at.elapsed() < Duration::from_secs(30) {
                return Ok(cached.status.clone());
            }
        }

        // Phase 2B: Provider anomaly check interface ready.
        // Actual sentinel check would go here.

        let status = AnomalyStatus {
            provider_id: provider_id.to_string(),
            has_anomalies: false,
            active_count: 0,
            max_severity: 0,
            action: AnomalyAction::Continue,
        };

        // Cache the result
        self.status_cache.insert(
            provider_id.to_string(),
            CachedStatus {
                status: status.clone(),
                cached_at: std::time::Instant::now(),
            },
        );

        Ok(status)
    }

    #[instrument(skip(self, anomaly), fields(anomaly_id = %anomaly.id))]
    async fn get_fallback_recommendation(
        &self,
        anomaly: &AnomalyAlert,
    ) -> IntegrationResult<FallbackRecommendation> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("sentinel".to_string()));
        }

        debug!(
            anomaly_id = %anomaly.id,
            severity = anomaly.severity,
            "Getting fallback recommendation from sentinel"
        );

        let should_fallback = anomaly.severity >= self.config.severity_threshold;
        let _action = self.severity_to_action(anomaly.severity);

        Ok(FallbackRecommendation {
            should_fallback,
            fallback_provider: anomaly.recommended_action.clone(),
            reason: format!(
                "Anomaly severity {} {} threshold {}",
                anomaly.severity,
                if should_fallback { ">=" } else { "<" },
                self.config.severity_threshold
            ),
            estimated_recovery: None,
        })
    }

    #[instrument(skip(self, report), fields(provider_id = %report.provider_id))]
    async fn report_anomaly(&self, report: AnomalyReport) -> IntegrationResult<()> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("sentinel".to_string()));
        }

        debug!(
            provider_id = %report.provider_id,
            report_type = %report.report_type,
            "Reporting anomaly to sentinel"
        );

        // Phase 2B: Anomaly reporting interface ready.
        // Actual report submission would go here.

        Ok(())
    }
}

impl std::fmt::Debug for SentinelAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SentinelAdapter")
            .field("enabled", &self.config.enabled)
            .field("severity_threshold", &self.config.severity_threshold)
            .field("auto_fallback", &self.config.auto_fallback)
            .field("cached_statuses", &self.status_cache.len())
            .finish()
    }
}

/// Builder for `SentinelAdapter`
pub struct SentinelAdapterBuilder {
    config: SentinelConfig,
}

impl SentinelAdapterBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: SentinelConfig::default(),
        }
    }

    /// Set the configuration.
    pub fn config(mut self, config: SentinelConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable the adapter.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Set the severity threshold.
    pub fn severity_threshold(mut self, threshold: u8) -> Self {
        self.config.severity_threshold = threshold;
        self
    }

    /// Enable/disable auto-fallback.
    pub fn auto_fallback(mut self, enabled: bool) -> Self {
        self.config.auto_fallback = enabled;
        self
    }

    /// Build the adapter.
    pub fn build(self) -> SentinelAdapter {
        SentinelAdapter::new(self.config)
    }
}

impl Default for SentinelAdapterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_disabled_by_default() {
        let adapter = SentinelAdapter::new(SentinelConfig::default());
        assert!(!adapter.is_enabled());
    }

    #[test]
    fn test_severity_to_action() {
        let adapter = SentinelAdapter::new(SentinelConfig::default());

        assert_eq!(adapter.severity_to_action(95), AnomalyAction::Fallback);
        assert_eq!(adapter.severity_to_action(75), AnomalyAction::Avoid);
        assert_eq!(adapter.severity_to_action(55), AnomalyAction::Throttle);
        assert_eq!(adapter.severity_to_action(35), AnomalyAction::Caution);
        assert_eq!(adapter.severity_to_action(15), AnomalyAction::Continue);
    }

    #[tokio::test]
    async fn test_disabled_returns_not_enabled() {
        let adapter = SentinelAdapter::new(SentinelConfig::default());
        let result = adapter.consume_anomalies().await;
        assert!(matches!(result, Err(IntegrationError::NotEnabled(_))));
    }
}
