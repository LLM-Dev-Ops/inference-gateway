//! LLM-Shield adapter for safety filtering.
//!
//! This adapter consumes safety filtering decisions from LLM-Shield
//! to apply safety filters and output validation.

use crate::config::ShieldConfig;
use crate::error::{IntegrationError, IntegrationResult};
use crate::traits::{
    ContentLocation, PiiCheckResult, PiiType, SafetyCategory, SafetyFilter, SafetyFinding,
    SafetyPolicy, SafetyResult,
};
use async_trait::async_trait;
use gateway_core::{GatewayRequest, GatewayResponse};
use std::sync::Arc;
use tracing::{debug, instrument};

/// Adapter for consuming safety filtering from LLM-Shield.
///
/// This is a thin wrapper that consumes safety decisions without
/// modifying existing gateway request handling.
pub struct ShieldAdapter {
    /// Configuration
    config: ShieldConfig,
}

impl ShieldAdapter {
    /// Create a new shield adapter.
    pub fn new(config: ShieldConfig) -> Self {
        Self { config }
    }

    /// Check if the adapter is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the safety threshold.
    pub fn safety_threshold(&self) -> f32 {
        self.config.safety_threshold
    }

    /// Check if input validation is enabled.
    pub fn validates_input(&self) -> bool {
        self.config.validate_input
    }

    /// Check if output validation is enabled.
    pub fn validates_output(&self) -> bool {
        self.config.validate_output
    }

    /// Extract text content from a request for validation.
    fn extract_request_content(request: &GatewayRequest) -> String {
        request
            .messages
            .iter()
            .filter_map(|msg| msg.text_content())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[async_trait]
impl SafetyFilter for ShieldAdapter {
    #[instrument(skip(self, request), fields(model = %request.model))]
    async fn validate_input(&self, request: &GatewayRequest) -> IntegrationResult<SafetyResult> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("shield".to_string()));
        }

        if !self.config.validate_input {
            debug!("Input validation is disabled");
            return Ok(SafetyResult {
                safe: true,
                score: 1.0,
                triggered_categories: Vec::new(),
                findings: Vec::new(),
                should_block: false,
            });
        }

        let content = Self::extract_request_content(request);
        debug!(
            content_len = content.len(),
            "Validating input content via shield"
        );

        // Phase 2B: Safety validation interface ready.
        // Actual shield validation would process the content here.
        // For now, return a pass-through result.

        Ok(SafetyResult {
            safe: true,
            score: 1.0,
            triggered_categories: Vec::new(),
            findings: Vec::new(),
            should_block: false,
        })
    }

    #[instrument(skip(self, response), fields(model = %response.model))]
    async fn validate_output(
        &self,
        response: &GatewayResponse,
    ) -> IntegrationResult<SafetyResult> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("shield".to_string()));
        }

        if !self.config.validate_output {
            debug!("Output validation is disabled");
            return Ok(SafetyResult {
                safe: true,
                score: 1.0,
                triggered_categories: Vec::new(),
                findings: Vec::new(),
                should_block: false,
            });
        }

        let content = response.content().unwrap_or("");
        debug!(
            content_len = content.len(),
            "Validating output content via shield"
        );

        // Phase 2B: Safety validation interface ready.
        // Actual shield validation would process the response here.

        Ok(SafetyResult {
            safe: true,
            score: 1.0,
            triggered_categories: Vec::new(),
            findings: Vec::new(),
            should_block: false,
        })
    }

    #[instrument(skip(self, content), fields(content_len = content.len()))]
    async fn check_pii(&self, content: &str) -> IntegrationResult<PiiCheckResult> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("shield".to_string()));
        }

        debug!("Checking content for PII via shield");

        // Phase 2B: PII detection interface ready.
        // Actual shield PII detection would process the content here.

        Ok(PiiCheckResult {
            contains_pii: false,
            pii_types: Vec::new(),
            redacted_content: None,
        })
    }

    #[instrument(skip(self))]
    async fn consume_safety_policies(&self) -> IntegrationResult<Vec<SafetyPolicy>> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("shield".to_string()));
        }

        debug!("Consuming safety policies from shield");

        // Phase 2B: Policy consumption interface ready.
        // Actual policy fetch would go here.

        Ok(Vec::new())
    }
}

impl std::fmt::Debug for ShieldAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShieldAdapter")
            .field("enabled", &self.config.enabled)
            .field("validate_input", &self.config.validate_input)
            .field("validate_output", &self.config.validate_output)
            .field("safety_threshold", &self.config.safety_threshold)
            .finish()
    }
}

/// Builder for `ShieldAdapter`
pub struct ShieldAdapterBuilder {
    config: ShieldConfig,
}

impl ShieldAdapterBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: ShieldConfig::default(),
        }
    }

    /// Set the configuration.
    pub fn config(mut self, config: ShieldConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable the adapter.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Set the safety threshold.
    pub fn safety_threshold(mut self, threshold: f32) -> Self {
        self.config.safety_threshold = threshold;
        self
    }

    /// Enable/disable input validation.
    pub fn validate_input(mut self, validate: bool) -> Self {
        self.config.validate_input = validate;
        self
    }

    /// Enable/disable output validation.
    pub fn validate_output(mut self, validate: bool) -> Self {
        self.config.validate_output = validate;
        self
    }

    /// Build the adapter.
    pub fn build(self) -> ShieldAdapter {
        ShieldAdapter::new(self.config)
    }
}

impl Default for ShieldAdapterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_disabled_by_default() {
        let adapter = ShieldAdapter::new(ShieldConfig::default());
        assert!(!adapter.is_enabled());
    }

    #[test]
    fn test_adapter_builder() {
        let adapter = ShieldAdapterBuilder::new()
            .enabled(true)
            .safety_threshold(0.9)
            .validate_input(true)
            .validate_output(false)
            .build();

        assert!(adapter.is_enabled());
        assert_eq!(adapter.safety_threshold(), 0.9);
        assert!(adapter.validates_input());
        assert!(!adapter.validates_output());
    }

    #[tokio::test]
    async fn test_disabled_returns_not_enabled() {
        let adapter = ShieldAdapter::new(ShieldConfig::default());
        let request = gateway_core::GatewayRequest::builder()
            .model("gpt-4")
            .message(gateway_core::ChatMessage::user("test"))
            .build()
            .unwrap();

        let result = adapter.validate_input(&request).await;
        assert!(matches!(result, Err(IntegrationError::NotEnabled(_))));
    }
}
