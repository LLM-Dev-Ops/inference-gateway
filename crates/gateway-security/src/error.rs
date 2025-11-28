//! Security error types.

/// Result type for security operations.
pub type Result<T> = std::result::Result<T, SecurityError>;

/// Security error type.
#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    /// Validation error.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Input contains forbidden content.
    #[error("Forbidden content detected: {0}")]
    ForbiddenContent(String),

    /// Invalid signature.
    #[error("Invalid signature")]
    InvalidSignature,

    /// Signature expired.
    #[error("Signature expired")]
    SignatureExpired,

    /// Missing required header.
    #[error("Missing required header: {0}")]
    MissingHeader(String),

    /// IP address blocked.
    #[error("IP address blocked: {0}")]
    IpBlocked(String),

    /// IP address not in allowlist.
    #[error("IP address not allowed: {0}")]
    IpNotAllowed(String),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded for {0}")]
    RateLimitExceeded(String),

    /// Encryption error.
    #[error("Encryption error: {0}")]
    Encryption(String),

    /// Decryption error.
    #[error("Decryption error: {0}")]
    Decryption(String),

    /// Key derivation error.
    #[error("Key derivation error: {0}")]
    KeyDerivation(String),

    /// Secret not found.
    #[error("Secret not found: {0}")]
    SecretNotFound(String),

    /// Secret expired.
    #[error("Secret expired: {0}")]
    SecretExpired(String),

    /// Invalid secret format.
    #[error("Invalid secret format: {0}")]
    InvalidSecretFormat(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Internal error.
    #[error("Internal security error: {0}")]
    Internal(String),
}

impl SecurityError {
    /// Create a validation error.
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    /// Create a forbidden content error.
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::ForbiddenContent(msg.into())
    }

    /// Create a config error.
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create an internal error.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Check if error is a client error (4xx).
    #[must_use]
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            Self::Validation(_)
                | Self::ForbiddenContent(_)
                | Self::InvalidSignature
                | Self::SignatureExpired
                | Self::MissingHeader(_)
                | Self::IpBlocked(_)
                | Self::IpNotAllowed(_)
                | Self::RateLimitExceeded(_)
        )
    }

    /// Get HTTP status code for this error.
    #[must_use]
    pub fn status_code(&self) -> u16 {
        match self {
            Self::Validation(_) => 400,
            Self::ForbiddenContent(_) => 400,
            Self::InvalidSignature => 401,
            Self::SignatureExpired => 401,
            Self::MissingHeader(_) => 400,
            Self::IpBlocked(_) => 403,
            Self::IpNotAllowed(_) => 403,
            Self::RateLimitExceeded(_) => 429,
            Self::Encryption(_) | Self::Decryption(_) => 500,
            Self::KeyDerivation(_) => 500,
            Self::SecretNotFound(_) => 404,
            Self::SecretExpired(_) => 401,
            Self::InvalidSecretFormat(_) => 400,
            Self::Config(_) | Self::Internal(_) => 500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = SecurityError::validation("invalid input");
        assert!(err.to_string().contains("Validation error"));

        let err = SecurityError::IpBlocked("1.2.3.4".to_string());
        assert!(err.to_string().contains("1.2.3.4"));
    }

    #[test]
    fn test_is_client_error() {
        assert!(SecurityError::Validation("test".to_string()).is_client_error());
        assert!(SecurityError::IpBlocked("test".to_string()).is_client_error());
        assert!(!SecurityError::Internal("test".to_string()).is_client_error());
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(SecurityError::Validation("".to_string()).status_code(), 400);
        assert_eq!(SecurityError::InvalidSignature.status_code(), 401);
        assert_eq!(SecurityError::IpBlocked("".to_string()).status_code(), 403);
        assert_eq!(SecurityError::RateLimitExceeded("".to_string()).status_code(), 429);
        assert_eq!(SecurityError::Internal("".to_string()).status_code(), 500);
    }
}
