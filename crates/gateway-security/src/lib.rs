//! # Gateway Security
//!
//! Comprehensive security hardening for the LLM Inference Gateway.
//!
//! ## Features
//!
//! - **Input Validation**: Request sanitization and validation
//! - **Security Headers**: HSTS, CSP, X-Frame-Options, etc.
//! - **Secrets Management**: Secure storage and handling of secrets
//! - **Request Signing**: HMAC-based request authentication
//! - **IP Filtering**: Allow/deny lists and rate limiting by IP
//! - **Content Security**: XSS and injection prevention
//!
//! ## Example
//!
//! ```rust,no_run
//! use gateway_security::{SecurityConfig, SecurityLayer};
//!
//! let config = SecurityConfig::default();
//! let layer = SecurityLayer::new(config);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
pub mod crypto;
pub mod error;
pub mod headers;
pub mod ip_filter;
pub mod middleware;
pub mod sanitize;
pub mod secrets;
pub mod signing;
pub mod validation;

pub use config::{SecurityConfig, SecurityConfigBuilder};
pub use crypto::{Encryption, HashingService, KeyDerivation};
pub use error::{SecurityError, Result};
pub use headers::{SecurityHeaders, SecurityHeadersLayer};
pub use ip_filter::{IpFilter, IpFilterConfig};
pub use middleware::SecurityLayer;
pub use sanitize::{Sanitizer, SanitizeConfig};
pub use secrets::{SecretStore, SecretValue};
pub use signing::{RequestSigner, SignatureVerifier};
pub use validation::{InputValidator, ValidationResult};
