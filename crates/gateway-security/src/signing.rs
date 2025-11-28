//! Request signing and verification.

use crate::config::SigningConfig;
use crate::crypto::HashingService;
use crate::error::{Result, SecurityError};
use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

/// Supported signing algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SigningAlgorithm {
    /// HMAC-SHA256.
    #[serde(rename = "HMAC-SHA256")]
    HmacSha256,
    /// HMAC-SHA512.
    #[serde(rename = "HMAC-SHA512")]
    HmacSha512,
}

impl Default for SigningAlgorithm {
    fn default() -> Self {
        Self::HmacSha256
    }
}

impl std::fmt::Display for SigningAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HmacSha256 => write!(f, "HMAC-SHA256"),
            Self::HmacSha512 => write!(f, "HMAC-SHA512"),
        }
    }
}

impl std::str::FromStr for SigningAlgorithm {
    type Err = SecurityError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "HMAC-SHA256" => Ok(Self::HmacSha256),
            "HMAC-SHA512" => Ok(Self::HmacSha512),
            _ => Err(SecurityError::validation(format!("Unknown algorithm: {}", s))),
        }
    }
}

/// Request signer for creating signatures.
#[derive(Clone)]
pub struct RequestSigner {
    secret: SecretString,
    algorithm: SigningAlgorithm,
}

impl RequestSigner {
    /// Create a new request signer.
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: SecretString::new(secret.into()),
            algorithm: SigningAlgorithm::default(),
        }
    }

    /// Set the signing algorithm.
    #[must_use]
    pub fn with_algorithm(mut self, algorithm: SigningAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Sign a request.
    ///
    /// # Errors
    /// Returns error if signing fails.
    pub fn sign(&self, request: &SignableRequest) -> Result<Signature> {
        let canonical = request.canonical_string();
        let timestamp = Utc::now();

        let string_to_sign = format!("{}\n{}", timestamp.timestamp(), canonical);

        let signature = match self.algorithm {
            SigningAlgorithm::HmacSha256 => {
                HashingService::hmac_sha256_hex(
                    self.secret.expose_secret().as_bytes(),
                    string_to_sign.as_bytes(),
                )?
            }
            SigningAlgorithm::HmacSha512 => {
                let hmac = hmac_sha512(
                    self.secret.expose_secret().as_bytes(),
                    string_to_sign.as_bytes(),
                )?;
                hex::encode(hmac)
            }
        };

        Ok(Signature {
            algorithm: self.algorithm,
            timestamp,
            value: signature,
        })
    }

    /// Sign a simple string message.
    ///
    /// # Errors
    /// Returns error if signing fails.
    pub fn sign_message(&self, message: &str) -> Result<String> {
        HashingService::hmac_sha256_hex(
            self.secret.expose_secret().as_bytes(),
            message.as_bytes(),
        )
    }

    /// Create a signature header value.
    ///
    /// # Errors
    /// Returns error if signing fails.
    pub fn create_header(&self, request: &SignableRequest) -> Result<String> {
        let signature = self.sign(request)?;
        Ok(signature.to_header_value())
    }
}

impl std::fmt::Debug for RequestSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestSigner")
            .field("secret", &"[REDACTED]")
            .field("algorithm", &self.algorithm)
            .finish()
    }
}

/// Signature verifier for validating signatures.
#[derive(Clone)]
pub struct SignatureVerifier {
    secret: SecretString,
    algorithm: SigningAlgorithm,
    validity_duration: Duration,
    clock_skew: Duration,
}

impl SignatureVerifier {
    /// Create a new signature verifier.
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: SecretString::new(secret.into()),
            algorithm: SigningAlgorithm::default(),
            validity_duration: Duration::from_secs(300),
            clock_skew: Duration::from_secs(60),
        }
    }

    /// Create from signing config.
    pub fn from_config(config: &SigningConfig, secret: impl Into<String>) -> Self {
        let algorithm = config
            .algorithm
            .parse()
            .unwrap_or(SigningAlgorithm::default());

        Self {
            secret: SecretString::new(secret.into()),
            algorithm,
            validity_duration: config.validity_duration,
            clock_skew: config.clock_skew,
        }
    }

    /// Set the signing algorithm.
    #[must_use]
    pub fn with_algorithm(mut self, algorithm: SigningAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Set validity duration.
    #[must_use]
    pub fn with_validity(mut self, duration: Duration) -> Self {
        self.validity_duration = duration;
        self
    }

    /// Set clock skew tolerance.
    #[must_use]
    pub fn with_clock_skew(mut self, duration: Duration) -> Self {
        self.clock_skew = duration;
        self
    }

    /// Verify a signature.
    ///
    /// # Errors
    /// Returns error if signature is invalid or expired.
    pub fn verify(&self, request: &SignableRequest, signature: &Signature) -> Result<()> {
        // Check timestamp
        let now = Utc::now();
        let timestamp = signature.timestamp;

        // Check if signature is too old
        let age = now
            .signed_duration_since(timestamp)
            .to_std()
            .unwrap_or(Duration::ZERO);

        if age > self.validity_duration + self.clock_skew {
            return Err(SecurityError::SignatureExpired);
        }

        // Check if signature is in the future (within clock skew)
        if timestamp > now {
            let future = timestamp
                .signed_duration_since(now)
                .to_std()
                .unwrap_or(Duration::ZERO);

            if future > self.clock_skew {
                return Err(SecurityError::InvalidSignature);
            }
        }

        // Verify signature
        let canonical = request.canonical_string();
        let string_to_sign = format!("{}\n{}", timestamp.timestamp(), canonical);

        let expected = match self.algorithm {
            SigningAlgorithm::HmacSha256 => {
                HashingService::hmac_sha256_hex(
                    self.secret.expose_secret().as_bytes(),
                    string_to_sign.as_bytes(),
                )?
            }
            SigningAlgorithm::HmacSha512 => {
                let hmac = hmac_sha512(
                    self.secret.expose_secret().as_bytes(),
                    string_to_sign.as_bytes(),
                )?;
                hex::encode(hmac)
            }
        };

        // Constant-time comparison
        if !HashingService::constant_time_eq(expected.as_bytes(), signature.value.as_bytes()) {
            return Err(SecurityError::InvalidSignature);
        }

        Ok(())
    }

    /// Verify a signature from header value.
    ///
    /// # Errors
    /// Returns error if signature is invalid or expired.
    pub fn verify_header(&self, request: &SignableRequest, header_value: &str) -> Result<()> {
        let signature = Signature::from_header_value(header_value)?;
        self.verify(request, &signature)
    }

    /// Verify a simple message signature.
    ///
    /// # Errors
    /// Returns error if signature is invalid.
    pub fn verify_message(&self, message: &str, signature: &str) -> Result<()> {
        let expected = HashingService::hmac_sha256_hex(
            self.secret.expose_secret().as_bytes(),
            message.as_bytes(),
        )?;

        if !HashingService::constant_time_eq(expected.as_bytes(), signature.as_bytes()) {
            return Err(SecurityError::InvalidSignature);
        }

        Ok(())
    }
}

impl std::fmt::Debug for SignatureVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignatureVerifier")
            .field("secret", &"[REDACTED]")
            .field("algorithm", &self.algorithm)
            .field("validity_duration", &self.validity_duration)
            .field("clock_skew", &self.clock_skew)
            .finish()
    }
}

/// A request that can be signed.
#[derive(Debug, Clone)]
pub struct SignableRequest {
    /// HTTP method.
    pub method: String,
    /// Request path.
    pub path: String,
    /// Query parameters (sorted).
    pub query: BTreeMap<String, String>,
    /// Headers to include in signature.
    pub headers: BTreeMap<String, String>,
    /// Request body hash (SHA-256).
    pub body_hash: Option<String>,
}

impl SignableRequest {
    /// Create a new signable request.
    #[must_use]
    pub fn new(method: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            method: method.into().to_uppercase(),
            path: path.into(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body_hash: None,
        }
    }

    /// Add a query parameter.
    #[must_use]
    pub fn query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.insert(key.into(), value.into());
        self
    }

    /// Add a header.
    #[must_use]
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into().to_lowercase(), value.into());
        self
    }

    /// Set body hash.
    #[must_use]
    pub fn body_hash(mut self, hash: impl Into<String>) -> Self {
        self.body_hash = Some(hash.into());
        self
    }

    /// Compute and set body hash from body bytes.
    #[must_use]
    pub fn with_body(mut self, body: &[u8]) -> Self {
        self.body_hash = Some(HashingService::sha256_hex(body));
        self
    }

    /// Create canonical string for signing.
    #[must_use]
    pub fn canonical_string(&self) -> String {
        let mut parts = Vec::new();

        // Method
        parts.push(self.method.clone());

        // Path
        parts.push(self.path.clone());

        // Query string (sorted)
        if !self.query.is_empty() {
            let query_string: String = self
                .query
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            parts.push(query_string);
        } else {
            parts.push(String::new());
        }

        // Headers (sorted by key)
        if !self.headers.is_empty() {
            let headers_string: String = self
                .headers
                .iter()
                .map(|(k, v)| format!("{}:{}", k, v))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(headers_string);
        } else {
            parts.push(String::new());
        }

        // Body hash
        parts.push(self.body_hash.clone().unwrap_or_default());

        parts.join("\n")
    }
}

/// A signature.
#[derive(Debug, Clone)]
pub struct Signature {
    /// Algorithm used.
    pub algorithm: SigningAlgorithm,
    /// Timestamp when signed.
    pub timestamp: DateTime<Utc>,
    /// Signature value (hex encoded).
    pub value: String,
}

impl Signature {
    /// Create a signature header value.
    #[must_use]
    pub fn to_header_value(&self) -> String {
        format!(
            "{};{};{}",
            self.algorithm,
            self.timestamp.timestamp(),
            self.value
        )
    }

    /// Parse from header value.
    ///
    /// # Errors
    /// Returns error if header format is invalid.
    pub fn from_header_value(value: &str) -> Result<Self> {
        let parts: Vec<&str> = value.split(';').collect();

        if parts.len() != 3 {
            return Err(SecurityError::InvalidSignature);
        }

        let algorithm = parts[0].parse()?;

        let timestamp = parts[1]
            .parse::<i64>()
            .map_err(|_| SecurityError::InvalidSignature)?;

        let timestamp = DateTime::from_timestamp(timestamp, 0)
            .ok_or(SecurityError::InvalidSignature)?;

        Ok(Self {
            algorithm,
            timestamp,
            value: parts[2].to_string(),
        })
    }
}

/// HMAC-SHA512 implementation.
fn hmac_sha512(key: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    use hmac::{Hmac, Mac};
    use sha2::Sha512;

    type HmacSha512 = Hmac<Sha512>;

    let mut mac = HmacSha512::new_from_slice(key)
        .map_err(|e| SecurityError::Internal(format!("Invalid HMAC key: {}", e)))?;

    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

/// Webhook signature utilities.
pub mod webhook {
    use super::*;

    /// Sign a webhook payload.
    ///
    /// # Errors
    /// Returns error if signing fails.
    pub fn sign(secret: &str, payload: &[u8]) -> Result<String> {
        let timestamp = Utc::now().timestamp();
        let signed_payload = format!("{}:{}", timestamp, hex::encode(payload));

        let signature = HashingService::hmac_sha256_hex(
            secret.as_bytes(),
            signed_payload.as_bytes(),
        )?;

        Ok(format!("t={},v1={}", timestamp, signature))
    }

    /// Verify a webhook signature.
    ///
    /// # Errors
    /// Returns error if signature is invalid.
    pub fn verify(
        secret: &str,
        payload: &[u8],
        signature: &str,
        tolerance: Duration,
    ) -> Result<()> {
        // Parse signature
        let parts: Vec<&str> = signature.split(',').collect();
        if parts.len() < 2 {
            return Err(SecurityError::InvalidSignature);
        }

        let timestamp = parts[0]
            .strip_prefix("t=")
            .and_then(|t| t.parse::<i64>().ok())
            .ok_or(SecurityError::InvalidSignature)?;

        let sig_value = parts
            .iter()
            .find(|p| p.starts_with("v1="))
            .and_then(|p| p.strip_prefix("v1="))
            .ok_or(SecurityError::InvalidSignature)?;

        // Check timestamp
        let now = Utc::now().timestamp();
        let age = (now - timestamp).unsigned_abs();

        if age > tolerance.as_secs() {
            return Err(SecurityError::SignatureExpired);
        }

        // Verify signature
        let signed_payload = format!("{}:{}", timestamp, hex::encode(payload));
        let expected = HashingService::hmac_sha256_hex(
            secret.as_bytes(),
            signed_payload.as_bytes(),
        )?;

        if !HashingService::constant_time_eq(expected.as_bytes(), sig_value.as_bytes()) {
            return Err(SecurityError::InvalidSignature);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_signer() {
        let signer = RequestSigner::new("secret_key");
        let request = SignableRequest::new("POST", "/api/v1/chat")
            .header("content-type", "application/json")
            .with_body(b"{}");

        let signature = signer.sign(&request).unwrap();
        assert!(!signature.value.is_empty());
        assert_eq!(signature.algorithm, SigningAlgorithm::HmacSha256);
    }

    #[test]
    fn test_signature_verification() {
        let secret = "shared_secret";
        let signer = RequestSigner::new(secret);
        let verifier = SignatureVerifier::new(secret);

        let request = SignableRequest::new("GET", "/api/users")
            .query("page", "1")
            .query("limit", "10");

        let signature = signer.sign(&request).unwrap();
        let result = verifier.verify(&request, &signature);

        assert!(result.is_ok());
    }

    #[test]
    fn test_signature_verification_wrong_secret() {
        let signer = RequestSigner::new("secret1");
        let verifier = SignatureVerifier::new("secret2");

        let request = SignableRequest::new("GET", "/api/users");

        let signature = signer.sign(&request).unwrap();
        let result = verifier.verify(&request, &signature);

        assert!(matches!(result, Err(SecurityError::InvalidSignature)));
    }

    #[test]
    fn test_signature_verification_tampered_request() {
        let secret = "shared_secret";
        let signer = RequestSigner::new(secret);
        let verifier = SignatureVerifier::new(secret);

        let original = SignableRequest::new("GET", "/api/users");
        let tampered = SignableRequest::new("GET", "/api/admin");

        let signature = signer.sign(&original).unwrap();
        let result = verifier.verify(&tampered, &signature);

        assert!(matches!(result, Err(SecurityError::InvalidSignature)));
    }

    #[test]
    fn test_signature_expiration() {
        let secret = "shared_secret";
        let signer = RequestSigner::new(secret);
        let verifier = SignatureVerifier::new(secret)
            .with_validity(Duration::from_secs(1));

        let request = SignableRequest::new("GET", "/api/users");
        let mut signature = signer.sign(&request).unwrap();

        // Make signature old
        signature.timestamp = Utc::now() - chrono::Duration::seconds(3600);

        let result = verifier.verify(&request, &signature);
        assert!(matches!(result, Err(SecurityError::SignatureExpired)));
    }

    #[test]
    fn test_signature_header_roundtrip() {
        let secret = "shared_secret";
        let signer = RequestSigner::new(secret);

        let request = SignableRequest::new("POST", "/api/chat");
        let signature = signer.sign(&request).unwrap();

        let header = signature.to_header_value();
        let parsed = Signature::from_header_value(&header).unwrap();

        assert_eq!(signature.algorithm, parsed.algorithm);
        assert_eq!(signature.value, parsed.value);
    }

    #[test]
    fn test_message_signing() {
        let secret = "shared_secret";
        let signer = RequestSigner::new(secret);
        let verifier = SignatureVerifier::new(secret);

        let message = "Hello, World!";
        let signature = signer.sign_message(message).unwrap();

        let result = verifier.verify_message(message, &signature);
        assert!(result.is_ok());

        let wrong_result = verifier.verify_message("Wrong message", &signature);
        assert!(wrong_result.is_err());
    }

    #[test]
    fn test_canonical_string() {
        let request = SignableRequest::new("POST", "/api/v1/chat")
            .query("key", "value")
            .header("content-type", "application/json")
            .with_body(b"test");

        let canonical = request.canonical_string();
        assert!(canonical.contains("POST"));
        assert!(canonical.contains("/api/v1/chat"));
        assert!(canonical.contains("key=value"));
        assert!(canonical.contains("content-type:application/json"));
    }

    #[test]
    fn test_signing_algorithm_parse() {
        assert_eq!(
            "HMAC-SHA256".parse::<SigningAlgorithm>().unwrap(),
            SigningAlgorithm::HmacSha256
        );
        assert_eq!(
            "hmac-sha512".parse::<SigningAlgorithm>().unwrap(),
            SigningAlgorithm::HmacSha512
        );
        assert!("unknown".parse::<SigningAlgorithm>().is_err());
    }

    #[test]
    fn test_signing_with_sha512() {
        let signer = RequestSigner::new("secret")
            .with_algorithm(SigningAlgorithm::HmacSha512);
        let verifier = SignatureVerifier::new("secret")
            .with_algorithm(SigningAlgorithm::HmacSha512);

        let request = SignableRequest::new("GET", "/test");
        let signature = signer.sign(&request).unwrap();

        assert_eq!(signature.algorithm, SigningAlgorithm::HmacSha512);
        assert!(verifier.verify(&request, &signature).is_ok());
    }

    #[test]
    fn test_webhook_signing() {
        let secret = "whsec_test123";
        let payload = b"test payload";

        let signature = webhook::sign(secret, payload).unwrap();
        assert!(signature.starts_with("t="));
        assert!(signature.contains(",v1="));

        let result = webhook::verify(secret, payload, &signature, Duration::from_secs(300));
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_verification_invalid() {
        let result = webhook::verify(
            "secret",
            b"payload",
            "t=12345,v1=invalid",
            Duration::from_secs(300),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_debug_redaction() {
        let signer = RequestSigner::new("secret_key");
        let debug = format!("{:?}", signer);
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("secret_key"));

        let verifier = SignatureVerifier::new("secret_key");
        let debug = format!("{:?}", verifier);
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("secret_key"));
    }
}
