//! Error types for the Gateway SDK.

use thiserror::Error;

/// Result type for SDK operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur when using the Gateway SDK.
#[derive(Error, Debug)]
pub enum Error {
    /// Configuration error during client setup.
    #[error("Configuration error: {message}")]
    Configuration {
        /// Error message describing the configuration issue.
        message: String,
    },

    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Server returned an error response.
    #[error("API error ({status}): {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Error message from the server.
        message: String,
        /// Error type/code from the server.
        error_type: Option<String>,
        /// Request ID for debugging.
        request_id: Option<String>,
    },

    /// Rate limit exceeded.
    #[error("Rate limit exceeded: retry after {retry_after:?} seconds")]
    RateLimited {
        /// Number of seconds to wait before retrying.
        retry_after: Option<u64>,
        /// Request ID for debugging.
        request_id: Option<String>,
    },

    /// Authentication failed.
    #[error("Authentication failed: {message}")]
    Authentication {
        /// Error message describing the authentication failure.
        message: String,
    },

    /// Model not found.
    #[error("Model not found: {model}")]
    ModelNotFound {
        /// The model that was not found.
        model: String,
    },

    /// Invalid request parameters.
    #[error("Invalid request: {message}")]
    InvalidRequest {
        /// Error message describing the invalid request.
        message: String,
        /// The parameter that was invalid.
        parameter: Option<String>,
    },

    /// Response parsing failed.
    #[error("Failed to parse response: {message}")]
    ParseError {
        /// Error message describing the parse failure.
        message: String,
    },

    /// Streaming error.
    #[error("Streaming error: {message}")]
    Streaming {
        /// Error message describing the streaming error.
        message: String,
    },

    /// Timeout waiting for response.
    #[error("Request timed out after {duration_ms}ms")]
    Timeout {
        /// Duration in milliseconds before timeout.
        duration_ms: u64,
    },

    /// Connection error.
    #[error("Connection error: {message}")]
    Connection {
        /// Error message describing the connection error.
        message: String,
    },

    /// Retry exhausted.
    #[error("Max retries ({attempts}) exhausted")]
    RetryExhausted {
        /// Number of attempts made.
        attempts: u32,
        /// The last error encountered.
        last_error: Box<Error>,
    },

    /// Server unavailable.
    #[error("Server unavailable: {message}")]
    Unavailable {
        /// Error message describing the unavailability.
        message: String,
    },

    /// Internal SDK error.
    #[error("Internal error: {message}")]
    Internal {
        /// Error message describing the internal error.
        message: String,
    },
}

impl Error {
    /// Create a configuration error.
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create an API error from response details.
    pub fn api(status: u16, message: impl Into<String>) -> Self {
        Self::Api {
            status,
            message: message.into(),
            error_type: None,
            request_id: None,
        }
    }

    /// Create an API error with full details.
    pub fn api_full(
        status: u16,
        message: impl Into<String>,
        error_type: Option<String>,
        request_id: Option<String>,
    ) -> Self {
        Self::Api {
            status,
            message: message.into(),
            error_type,
            request_id,
        }
    }

    /// Create a rate limited error.
    pub fn rate_limited(retry_after: Option<u64>) -> Self {
        Self::RateLimited {
            retry_after,
            request_id: None,
        }
    }

    /// Create an authentication error.
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication {
            message: message.into(),
        }
    }

    /// Create a model not found error.
    pub fn model_not_found(model: impl Into<String>) -> Self {
        Self::ModelNotFound {
            model: model.into(),
        }
    }

    /// Create an invalid request error.
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            message: message.into(),
            parameter: None,
        }
    }

    /// Create a parse error.
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::ParseError {
            message: message.into(),
        }
    }

    /// Create a streaming error.
    pub fn streaming(message: impl Into<String>) -> Self {
        Self::Streaming {
            message: message.into(),
        }
    }

    /// Create a timeout error.
    pub fn timeout(duration_ms: u64) -> Self {
        Self::Timeout { duration_ms }
    }

    /// Create a connection error.
    pub fn connection(message: impl Into<String>) -> Self {
        Self::Connection {
            message: message.into(),
        }
    }

    /// Create a retry exhausted error.
    pub fn retry_exhausted(attempts: u32, last_error: Error) -> Self {
        Self::RetryExhausted {
            attempts,
            last_error: Box::new(last_error),
        }
    }

    /// Create an unavailable error.
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::Unavailable {
            message: message.into(),
        }
    }

    /// Create an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Check if the error is retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http(e) => e.is_timeout() || e.is_connect(),
            Self::RateLimited { .. } => true,
            Self::Unavailable { .. } => true,
            Self::Timeout { .. } => true,
            Self::Connection { .. } => true,
            Self::Api { status, .. } => {
                matches!(status, 429 | 500 | 502 | 503 | 504)
            }
            _ => false,
        }
    }

    /// Get the HTTP status code if available.
    pub fn status_code(&self) -> Option<u16> {
        match self {
            Self::Api { status, .. } => Some(*status),
            Self::RateLimited { .. } => Some(429),
            Self::Authentication { .. } => Some(401),
            Self::ModelNotFound { .. } => Some(404),
            Self::InvalidRequest { .. } => Some(400),
            Self::Unavailable { .. } => Some(503),
            _ => None,
        }
    }

    /// Get the request ID if available.
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Api { request_id, .. } => request_id.as_deref(),
            Self::RateLimited { request_id, .. } => request_id.as_deref(),
            _ => None,
        }
    }

    /// Get the retry-after duration if available.
    pub fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            Self::RateLimited { retry_after, .. } => {
                retry_after.map(std::time::Duration::from_secs)
            }
            _ => None,
        }
    }
}

/// Error response from the API.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ApiErrorResponse {
    /// Error details.
    pub error: ApiErrorDetail,
}

/// Detailed error information from the API.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ApiErrorDetail {
    /// Error type/code.
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    /// Human-readable error message.
    pub message: String,
    /// Error code.
    pub code: Option<String>,
    /// Parameter that caused the error.
    pub param: Option<String>,
}

impl From<ApiErrorResponse> for Error {
    fn from(response: ApiErrorResponse) -> Self {
        Self::Api {
            status: 0, // Will be set by the caller
            message: response.error.message,
            error_type: response.error.error_type.or(response.error.code),
            request_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = Error::configuration("invalid base URL");
        assert!(matches!(err, Error::Configuration { .. }));
        assert!(err.to_string().contains("invalid base URL"));
    }

    #[test]
    fn test_error_retryable() {
        assert!(Error::rate_limited(Some(60)).is_retryable());
        assert!(Error::unavailable("service down").is_retryable());
        assert!(Error::timeout(5000).is_retryable());
        assert!(!Error::authentication("invalid key").is_retryable());
        assert!(!Error::invalid_request("bad param").is_retryable());
    }

    #[test]
    fn test_error_status_code() {
        assert_eq!(Error::api(500, "server error").status_code(), Some(500));
        assert_eq!(Error::rate_limited(None).status_code(), Some(429));
        assert_eq!(Error::authentication("bad key").status_code(), Some(401));
    }

    #[test]
    fn test_retry_after() {
        let err = Error::rate_limited(Some(60));
        assert_eq!(
            err.retry_after(),
            Some(std::time::Duration::from_secs(60))
        );
    }
}
