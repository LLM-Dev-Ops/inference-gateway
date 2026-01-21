//! Contract error types for the agentics system.
//!
//! Defines all error types that can occur during agent execution,
//! providing structured error information for debugging and user feedback.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during agent execution.
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum AgentError {
    /// Input validation failed.
    #[error("Validation error: {message}")]
    Validation {
        /// Description of the validation failure.
        message: String,
        /// Field that failed validation (if applicable).
        field: Option<String>,
    },

    /// No healthy providers are available for routing.
    #[error("No healthy providers available")]
    NoHealthyProviders,

    /// The requested model is not supported by any provider.
    #[error("Model not supported: {model}")]
    ModelNotSupported {
        /// The unsupported model identifier.
        model: String,
        /// List of supported models (if available).
        supported_models: Option<Vec<String>>,
    },

    /// The requested provider is not available.
    #[error("Provider not available: {provider}")]
    ProviderNotAvailable {
        /// The unavailable provider identifier.
        provider: String,
        /// Reason for unavailability.
        reason: Option<String>,
    },

    /// A required capability is not available.
    #[error("Required capability not available: {capability}")]
    CapabilityNotAvailable {
        /// The missing capability.
        capability: String,
        /// Providers that were checked.
        checked_providers: Vec<String>,
    },

    /// Policy violation prevented the routing.
    #[error("Policy violation: {policy_id} - {message}")]
    PolicyViolation {
        /// The policy that was violated.
        policy_id: String,
        /// Description of the violation.
        message: String,
    },

    /// Rate limit exceeded.
    #[error("Rate limit exceeded: {limit_type}")]
    RateLimitExceeded {
        /// Type of rate limit (e.g., "requests", "tokens").
        limit_type: String,
        /// Retry after this many seconds (if available).
        retry_after_secs: Option<u64>,
    },

    /// Quota exceeded for the tenant.
    #[error("Quota exceeded for tenant: {tenant_id}")]
    QuotaExceeded {
        /// The tenant identifier.
        tenant_id: String,
        /// Type of quota exceeded.
        quota_type: String,
    },

    /// Agent execution timed out.
    #[error("Agent execution timed out after {timeout_ms}ms")]
    Timeout {
        /// Timeout duration in milliseconds.
        timeout_ms: u64,
    },

    /// Configuration error in the agent.
    #[error("Configuration error: {message}")]
    Configuration {
        /// Description of the configuration error.
        message: String,
    },

    /// Internal agent error.
    #[error("Internal agent error: {message}")]
    Internal {
        /// Description of the internal error.
        message: String,
        /// Optional error code.
        code: Option<String>,
    },

    /// Serialization or deserialization error.
    #[error("Serialization error: {message}")]
    Serialization {
        /// Description of the serialization error.
        message: String,
    },

    /// All fallback providers failed.
    #[error("All fallback providers exhausted")]
    FallbackExhausted {
        /// Providers that were tried.
        tried_providers: Vec<String>,
        /// Failures for each provider.
        failures: Vec<ProviderFailure>,
    },

    /// Constraint evaluation failed.
    #[error("Constraint evaluation failed: {constraint_type}")]
    ConstraintEvaluationFailed {
        /// Type of constraint that failed.
        constraint_type: String,
        /// Details about the failure.
        details: Option<String>,
    },
}

impl AgentError {
    /// Creates a validation error.
    #[must_use]
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
            field: None,
        }
    }

    /// Creates a validation error for a specific field.
    #[must_use]
    pub fn validation_field(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
            field: Some(field.into()),
        }
    }

    /// Creates a model not supported error.
    #[must_use]
    pub fn model_not_supported(model: impl Into<String>) -> Self {
        Self::ModelNotSupported {
            model: model.into(),
            supported_models: None,
        }
    }

    /// Creates a provider not available error.
    #[must_use]
    pub fn provider_not_available(provider: impl Into<String>) -> Self {
        Self::ProviderNotAvailable {
            provider: provider.into(),
            reason: None,
        }
    }

    /// Creates an internal error.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            code: None,
        }
    }

    /// Creates a serialization error.
    #[must_use]
    pub fn serialization(message: impl Into<String>) -> Self {
        Self::Serialization {
            message: message.into(),
        }
    }

    /// Creates a timeout error.
    #[must_use]
    pub fn timeout(timeout_ms: u64) -> Self {
        Self::Timeout { timeout_ms }
    }

    /// Creates a configuration error.
    #[must_use]
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Returns the error code for this error type.
    #[must_use]
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Validation { .. } => "VALIDATION_ERROR",
            Self::NoHealthyProviders => "NO_HEALTHY_PROVIDERS",
            Self::ModelNotSupported { .. } => "MODEL_NOT_SUPPORTED",
            Self::ProviderNotAvailable { .. } => "PROVIDER_NOT_AVAILABLE",
            Self::CapabilityNotAvailable { .. } => "CAPABILITY_NOT_AVAILABLE",
            Self::PolicyViolation { .. } => "POLICY_VIOLATION",
            Self::RateLimitExceeded { .. } => "RATE_LIMIT_EXCEEDED",
            Self::QuotaExceeded { .. } => "QUOTA_EXCEEDED",
            Self::Timeout { .. } => "TIMEOUT",
            Self::Configuration { .. } => "CONFIGURATION_ERROR",
            Self::Internal { .. } => "INTERNAL_ERROR",
            Self::Serialization { .. } => "SERIALIZATION_ERROR",
            Self::FallbackExhausted { .. } => "FALLBACK_EXHAUSTED",
            Self::ConstraintEvaluationFailed { .. } => "CONSTRAINT_EVALUATION_FAILED",
        }
    }

    /// Returns whether this error is retryable.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::ProviderNotAvailable { .. }
                | Self::Timeout { .. }
                | Self::RateLimitExceeded { .. }
                | Self::Internal { .. }
        )
    }
}

/// Details about a provider failure during fallback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderFailure {
    /// The provider identifier.
    pub provider_id: String,
    /// The error that occurred.
    pub error: String,
    /// Whether this failure is retryable.
    pub retryable: bool,
}

impl ProviderFailure {
    /// Creates a new provider failure.
    #[must_use]
    pub fn new(
        provider_id: impl Into<String>,
        error: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            error: error.into(),
            retryable,
        }
    }
}

/// Result type for agent operations.
pub type AgentResult<T> = Result<T, AgentError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_constructors() {
        let err = AgentError::validation("Invalid input");
        assert!(matches!(err, AgentError::Validation { .. }));

        let err = AgentError::model_not_supported("invalid-model");
        assert!(matches!(err, AgentError::ModelNotSupported { .. }));

        let err = AgentError::timeout(5000);
        assert!(matches!(err, AgentError::Timeout { timeout_ms: 5000 }));
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(
            AgentError::NoHealthyProviders.error_code(),
            "NO_HEALTHY_PROVIDERS"
        );
        assert_eq!(
            AgentError::validation("test").error_code(),
            "VALIDATION_ERROR"
        );
    }

    #[test]
    fn test_is_retryable() {
        assert!(AgentError::timeout(1000).is_retryable());
        assert!(AgentError::provider_not_available("openai").is_retryable());
        assert!(!AgentError::validation("bad input").is_retryable());
        assert!(!AgentError::NoHealthyProviders.is_retryable());
    }

    #[test]
    fn test_error_display() {
        let err = AgentError::ModelNotSupported {
            model: "gpt-5".to_string(),
            supported_models: None,
        };
        assert_eq!(err.to_string(), "Model not supported: gpt-5");
    }

    #[test]
    fn test_serialization() {
        let err = AgentError::PolicyViolation {
            policy_id: "pol-123".to_string(),
            message: "Access denied".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"policy_id\":\"pol-123\""));
    }

    #[test]
    fn test_provider_failure() {
        let failure = ProviderFailure::new("openai", "Connection timeout", true);
        assert_eq!(failure.provider_id, "openai");
        assert!(failure.retryable);
    }
}
