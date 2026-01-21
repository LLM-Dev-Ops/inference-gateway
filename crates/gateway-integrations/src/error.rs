//! Error types for integration adapters.

use thiserror::Error;

/// Result type for integration operations
pub type IntegrationResult<T> = Result<T, IntegrationError>;

/// Errors that can occur during integration operations
#[derive(Debug, Error)]
pub enum IntegrationError {
    /// Error from connector hub integration
    #[error("Connector hub error: {message}")]
    ConnectorHub {
        /// Error message
        message: String,
        /// Optional source error
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Error from shield integration
    #[error("Shield error: {message}")]
    Shield {
        /// Error message
        message: String,
        /// Whether the content was blocked
        blocked: bool,
    },

    /// Error from sentinel integration
    #[error("Sentinel error: {message}")]
    Sentinel {
        /// Error message
        message: String,
        /// Anomaly severity level (0-100)
        severity: Option<u8>,
    },

    /// Error from cost-ops integration
    #[error("CostOps error: {message}")]
    CostOps {
        /// Error message
        message: String,
        /// Estimated cost that exceeded budget
        estimated_cost: Option<f64>,
    },

    /// Error from observatory integration
    #[error("Observatory error: {message}")]
    Observatory {
        /// Error message
        message: String,
    },

    /// Error from auto-optimizer integration
    #[error("Auto-optimizer error: {message}")]
    AutoOptimizer {
        /// Error message
        message: String,
    },

    /// Error from policy engine integration
    #[error("Policy engine error: {message}")]
    PolicyEngine {
        /// Error message
        message: String,
        /// Policy that was violated
        violated_policy: Option<String>,
    },

    /// Error from router integration
    #[error("Router error: {message}")]
    Router {
        /// Error message
        message: String,
    },

    /// Error from RuVector service integration
    #[error("RuVector error: {message}")]
    RuVector {
        /// Error message
        message: String,
        /// Whether the error is retryable
        retryable: bool,
    },

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Connection error
    #[error("Connection error: {0}")]
    Connection(String),

    /// Timeout error
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Integration not enabled
    #[error("Integration not enabled: {0}")]
    NotEnabled(String),
}

impl IntegrationError {
    /// Create a new connector hub error
    pub fn connector_hub(message: impl Into<String>) -> Self {
        Self::ConnectorHub {
            message: message.into(),
            source: None,
        }
    }

    /// Create a new shield error
    pub fn shield(message: impl Into<String>, blocked: bool) -> Self {
        Self::Shield {
            message: message.into(),
            blocked,
        }
    }

    /// Create a new sentinel error
    pub fn sentinel(message: impl Into<String>, severity: Option<u8>) -> Self {
        Self::Sentinel {
            message: message.into(),
            severity,
        }
    }

    /// Create a new cost-ops error
    pub fn cost_ops(message: impl Into<String>, estimated_cost: Option<f64>) -> Self {
        Self::CostOps {
            message: message.into(),
            estimated_cost,
        }
    }

    /// Create a new observatory error
    pub fn observatory(message: impl Into<String>) -> Self {
        Self::Observatory {
            message: message.into(),
        }
    }

    /// Create a new auto-optimizer error
    pub fn auto_optimizer(message: impl Into<String>) -> Self {
        Self::AutoOptimizer {
            message: message.into(),
        }
    }

    /// Create a new policy engine error
    pub fn policy_engine(message: impl Into<String>, violated_policy: Option<String>) -> Self {
        Self::PolicyEngine {
            message: message.into(),
            violated_policy,
        }
    }

    /// Create a new router error
    pub fn router(message: impl Into<String>) -> Self {
        Self::Router {
            message: message.into(),
        }
    }

    /// Create a new RuVector error
    pub fn ruvector(message: impl Into<String>) -> Self {
        Self::RuVector {
            message: message.into(),
            retryable: false,
        }
    }

    /// Create a new retryable RuVector error
    pub fn ruvector_retryable(message: impl Into<String>) -> Self {
        Self::RuVector {
            message: message.into(),
            retryable: true,
        }
    }

    /// Check if this error indicates content was blocked
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Shield { blocked: true, .. })
    }

    /// Check if this error indicates a policy violation
    pub fn is_policy_violation(&self) -> bool {
        matches!(self, Self::PolicyEngine { .. })
    }
}
