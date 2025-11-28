//! Input validation utilities.

use crate::config::ValidationConfig;
use crate::error::{Result, SecurityError};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use std::collections::HashSet;
use validator::Validate;

/// Common validation patterns.
pub mod patterns {
    use super::*;

    /// Email pattern.
    pub static EMAIL: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap()
    });

    /// UUID pattern.
    pub static UUID: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
            .unwrap()
    });

    /// Alphanumeric pattern.
    pub static ALPHANUMERIC: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-zA-Z0-9]+$").unwrap());

    /// Slug pattern (lowercase alphanumeric with hyphens).
    pub static SLUG: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-z0-9]+(?:-[a-z0-9]+)*$").unwrap());

    /// API key pattern.
    pub static API_KEY: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^[a-zA-Z]+_[a-zA-Z0-9]{32,}$").unwrap());

    /// URL pattern (basic).
    pub static URL: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^https?://[^\s/$.?#].[^\s]*$").unwrap());

    /// IP address pattern (IPv4).
    pub static IPV4: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$").unwrap()
    });

    /// Safe string pattern (no special chars).
    pub static SAFE_STRING: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^[a-zA-Z0-9\s\-_.,!?]+$").unwrap());
}

/// Input validator.
#[derive(Debug, Clone)]
pub struct InputValidator {
    config: ValidationConfig,
}

impl InputValidator {
    /// Create a new input validator.
    #[must_use]
    pub fn new(config: ValidationConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    #[must_use]
    pub fn default_validator() -> Self {
        Self::new(ValidationConfig::default())
    }

    /// Create with strict configuration.
    #[must_use]
    pub fn strict() -> Self {
        Self::new(ValidationConfig::strict())
    }

    /// Validate a string length.
    ///
    /// # Errors
    /// Returns error if string exceeds maximum length.
    pub fn validate_string_length(&self, value: &str) -> Result<()> {
        if value.len() > self.config.max_string_length {
            return Err(SecurityError::validation(format!(
                "String exceeds maximum length of {}",
                self.config.max_string_length
            )));
        }
        Ok(())
    }

    /// Validate array length.
    ///
    /// # Errors
    /// Returns error if array exceeds maximum length.
    pub fn validate_array_length<T>(&self, value: &[T]) -> Result<()> {
        if value.len() > self.config.max_array_length {
            return Err(SecurityError::validation(format!(
                "Array exceeds maximum length of {}",
                self.config.max_array_length
            )));
        }
        Ok(())
    }

    /// Validate request body size.
    ///
    /// # Errors
    /// Returns error if body exceeds maximum size.
    pub fn validate_body_size(&self, size: usize) -> Result<()> {
        if size > self.config.max_body_size {
            return Err(SecurityError::validation(format!(
                "Request body exceeds maximum size of {} bytes",
                self.config.max_body_size
            )));
        }
        Ok(())
    }

    /// Validate content type.
    ///
    /// # Errors
    /// Returns error if content type is not allowed.
    pub fn validate_content_type(&self, content_type: &str) -> Result<()> {
        let base_type = content_type.split(';').next().unwrap_or(content_type).trim();

        if !self.config.allowed_content_types.contains(base_type) {
            return Err(SecurityError::validation(format!(
                "Content type '{}' is not allowed",
                content_type
            )));
        }
        Ok(())
    }

    /// Validate JSON depth.
    ///
    /// # Errors
    /// Returns error if JSON exceeds maximum nesting depth.
    pub fn validate_json_depth(&self, value: &Value) -> Result<()> {
        let depth = json_depth(value);
        if depth > self.config.max_depth {
            return Err(SecurityError::validation(format!(
                "JSON exceeds maximum nesting depth of {}",
                self.config.max_depth
            )));
        }
        Ok(())
    }

    /// Validate JSON structure.
    ///
    /// # Errors
    /// Returns error if JSON structure is invalid.
    pub fn validate_json(&self, value: &Value) -> Result<()> {
        self.validate_json_depth(value)?;
        self.validate_json_sizes(value)?;
        Ok(())
    }

    /// Validate JSON string and array sizes.
    fn validate_json_sizes(&self, value: &Value) -> Result<()> {
        match value {
            Value::String(s) => self.validate_string_length(s),
            Value::Array(arr) => {
                self.validate_array_length(arr)?;
                for item in arr {
                    self.validate_json_sizes(item)?;
                }
                Ok(())
            }
            Value::Object(obj) => {
                for (key, val) in obj {
                    self.validate_string_length(key)?;
                    self.validate_json_sizes(val)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Validate UTF-8 encoding.
    ///
    /// # Errors
    /// Returns error if bytes are not valid UTF-8.
    pub fn validate_utf8(&self, bytes: &[u8]) -> Result<String> {
        if !self.config.validate_utf8 {
            return String::from_utf8(bytes.to_vec())
                .map_err(|_| SecurityError::validation("Invalid UTF-8 encoding"));
        }

        let mut result = String::from_utf8(bytes.to_vec())
            .map_err(|_| SecurityError::validation("Invalid UTF-8 encoding"))?;

        if self.config.strip_null_bytes {
            result = result.replace('\0', "");
        }

        Ok(result)
    }

    /// Validate a struct using the validator crate.
    ///
    /// # Errors
    /// Returns error if validation fails.
    pub fn validate_struct<T: Validate>(&self, value: &T) -> Result<()> {
        value.validate().map_err(|e| {
            let errors: Vec<String> = e
                .field_errors()
                .into_iter()
                .map(|(field, errors)| {
                    let messages: Vec<String> = errors
                        .iter()
                        .filter_map(|e| e.message.as_ref().map(|m| m.to_string()))
                        .collect();
                    format!("{}: {}", field, messages.join(", "))
                })
                .collect();
            SecurityError::validation(errors.join("; "))
        })
    }
}

/// Calculate JSON nesting depth.
fn json_depth(value: &Value) -> usize {
    match value {
        Value::Array(arr) => 1 + arr.iter().map(json_depth).max().unwrap_or(0),
        Value::Object(obj) => 1 + obj.values().map(json_depth).max().unwrap_or(0),
        _ => 0,
    }
}

/// Validation result for detailed error information.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether validation passed.
    pub valid: bool,
    /// Validation errors.
    pub errors: Vec<ValidationError>,
}

impl ValidationResult {
    /// Create a successful result.
    #[must_use]
    pub fn ok() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
        }
    }

    /// Create a failed result.
    #[must_use]
    pub fn failed(errors: Vec<ValidationError>) -> Self {
        Self {
            valid: false,
            errors,
        }
    }

    /// Add an error.
    pub fn add_error(&mut self, error: ValidationError) {
        self.valid = false;
        self.errors.push(error);
    }

    /// Check if valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Convert to Result.
    ///
    /// # Errors
    /// Returns error if validation failed.
    pub fn into_result(self) -> Result<()> {
        if self.valid {
            Ok(())
        } else {
            let messages: Vec<String> = self.errors.iter().map(|e| e.to_string()).collect();
            Err(SecurityError::validation(messages.join("; ")))
        }
    }
}

/// A validation error.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Field that failed validation.
    pub field: String,
    /// Error message.
    pub message: String,
    /// Error code.
    pub code: String,
}

impl ValidationError {
    /// Create a new validation error.
    #[must_use]
    pub fn new(field: impl Into<String>, message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            code: code.into(),
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {} ({})", self.field, self.message, self.code)
    }
}

/// Validator builder for fluent validation.
#[derive(Debug)]
pub struct ValidatorBuilder {
    result: ValidationResult,
}

impl Default for ValidatorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidatorBuilder {
    /// Create a new validator builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            result: ValidationResult::ok(),
        }
    }

    /// Validate that a value is not empty.
    #[must_use]
    pub fn not_empty(mut self, field: &str, value: &str) -> Self {
        if value.is_empty() {
            self.result.add_error(ValidationError::new(
                field,
                "must not be empty",
                "required",
            ));
        }
        self
    }

    /// Validate minimum length.
    #[must_use]
    pub fn min_length(mut self, field: &str, value: &str, min: usize) -> Self {
        if value.len() < min {
            self.result.add_error(ValidationError::new(
                field,
                format!("must be at least {} characters", min),
                "min_length",
            ));
        }
        self
    }

    /// Validate maximum length.
    #[must_use]
    pub fn max_length(mut self, field: &str, value: &str, max: usize) -> Self {
        if value.len() > max {
            self.result.add_error(ValidationError::new(
                field,
                format!("must be at most {} characters", max),
                "max_length",
            ));
        }
        self
    }

    /// Validate against a regex pattern.
    #[must_use]
    pub fn matches(mut self, field: &str, value: &str, pattern: &Regex, message: &str) -> Self {
        if !pattern.is_match(value) {
            self.result.add_error(ValidationError::new(
                field,
                message,
                "pattern",
            ));
        }
        self
    }

    /// Validate email format.
    #[must_use]
    pub fn email(self, field: &str, value: &str) -> Self {
        self.matches(field, value, &patterns::EMAIL, "must be a valid email")
    }

    /// Validate UUID format.
    #[must_use]
    pub fn uuid(self, field: &str, value: &str) -> Self {
        self.matches(field, value, &patterns::UUID, "must be a valid UUID")
    }

    /// Validate URL format.
    #[must_use]
    pub fn url(self, field: &str, value: &str) -> Self {
        self.matches(field, value, &patterns::URL, "must be a valid URL")
    }

    /// Validate that value is in a set.
    #[must_use]
    pub fn one_of(mut self, field: &str, value: &str, allowed: &HashSet<&str>) -> Self {
        if !allowed.contains(value) {
            self.result.add_error(ValidationError::new(
                field,
                format!("must be one of: {:?}", allowed),
                "one_of",
            ));
        }
        self
    }

    /// Validate numeric range.
    #[must_use]
    pub fn range<T: PartialOrd + std::fmt::Display>(
        mut self,
        field: &str,
        value: T,
        min: T,
        max: T,
    ) -> Self {
        if value < min || value > max {
            self.result.add_error(ValidationError::new(
                field,
                format!("must be between {} and {}", min, max),
                "range",
            ));
        }
        self
    }

    /// Apply custom validation.
    #[must_use]
    pub fn custom<F>(mut self, field: &str, check: F, message: &str) -> Self
    where
        F: FnOnce() -> bool,
    {
        if !check() {
            self.result.add_error(ValidationError::new(
                field,
                message,
                "custom",
            ));
        }
        self
    }

    /// Validate conditionally.
    #[must_use]
    pub fn when<F>(self, condition: bool, validator: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        if condition {
            validator(self)
        } else {
            self
        }
    }

    /// Build the validation result.
    #[must_use]
    pub fn build(self) -> ValidationResult {
        self.result
    }

    /// Build and convert to Result.
    ///
    /// # Errors
    /// Returns error if validation failed.
    pub fn validate(self) -> Result<()> {
        self.build().into_result()
    }
}

/// Validate an email address.
#[must_use]
pub fn is_valid_email(email: &str) -> bool {
    patterns::EMAIL.is_match(email)
}

/// Validate a UUID.
#[must_use]
pub fn is_valid_uuid(uuid: &str) -> bool {
    patterns::UUID.is_match(uuid)
}

/// Validate an API key format.
#[must_use]
pub fn is_valid_api_key(key: &str) -> bool {
    patterns::API_KEY.is_match(key)
}

/// Validate a URL.
#[must_use]
pub fn is_valid_url(url: &str) -> bool {
    patterns::URL.is_match(url)
}

/// Validate an IPv4 address.
#[must_use]
pub fn is_valid_ipv4(ip: &str) -> bool {
    patterns::IPV4.is_match(ip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_validation() {
        assert!(is_valid_email("user@example.com"));
        assert!(is_valid_email("user.name+tag@example.co.uk"));
        assert!(!is_valid_email("invalid"));
        assert!(!is_valid_email("@example.com"));
        assert!(!is_valid_email("user@"));
    }

    #[test]
    fn test_uuid_validation() {
        assert!(is_valid_uuid("123e4567-e89b-12d3-a456-426614174000"));
        assert!(is_valid_uuid("00000000-0000-0000-0000-000000000000"));
        assert!(!is_valid_uuid("invalid"));
        assert!(!is_valid_uuid("123e4567-e89b-12d3-a456"));
    }

    #[test]
    fn test_api_key_validation() {
        assert!(is_valid_api_key("llm_1234567890abcdef1234567890abcdef"));
        assert!(is_valid_api_key("sk_abcdefghijklmnopqrstuvwxyz123456"));
        assert!(!is_valid_api_key("invalid"));
        assert!(!is_valid_api_key("1234567890"));
    }

    #[test]
    fn test_url_validation() {
        assert!(is_valid_url("https://example.com"));
        assert!(is_valid_url("http://localhost:8080/path"));
        assert!(!is_valid_url("invalid"));
        assert!(!is_valid_url("ftp://example.com"));
    }

    #[test]
    fn test_ipv4_validation() {
        assert!(is_valid_ipv4("192.168.1.1"));
        assert!(is_valid_ipv4("0.0.0.0"));
        assert!(is_valid_ipv4("255.255.255.255"));
        assert!(!is_valid_ipv4("256.0.0.0"));
        assert!(!is_valid_ipv4("invalid"));
    }

    #[test]
    fn test_input_validator_string_length() {
        let validator = InputValidator::new(ValidationConfig {
            max_string_length: 10,
            ..Default::default()
        });

        assert!(validator.validate_string_length("short").is_ok());
        assert!(validator.validate_string_length("this is too long").is_err());
    }

    #[test]
    fn test_input_validator_array_length() {
        let validator = InputValidator::new(ValidationConfig {
            max_array_length: 3,
            ..Default::default()
        });

        assert!(validator.validate_array_length(&[1, 2]).is_ok());
        assert!(validator.validate_array_length(&[1, 2, 3, 4, 5]).is_err());
    }

    #[test]
    fn test_input_validator_json_depth() {
        let validator = InputValidator::new(ValidationConfig {
            max_depth: 2,
            ..Default::default()
        });

        let shallow: Value = serde_json::json!({"a": {"b": 1}});
        assert!(validator.validate_json_depth(&shallow).is_ok());

        let deep: Value = serde_json::json!({"a": {"b": {"c": {"d": 1}}}});
        assert!(validator.validate_json_depth(&deep).is_err());
    }

    #[test]
    fn test_input_validator_content_type() {
        let validator = InputValidator::default_validator();

        assert!(validator.validate_content_type("application/json").is_ok());
        assert!(validator
            .validate_content_type("application/json; charset=utf-8")
            .is_ok());
        assert!(validator.validate_content_type("text/plain").is_ok());
        assert!(validator.validate_content_type("text/html").is_err());
    }

    #[test]
    fn test_validator_builder() {
        let result = ValidatorBuilder::new()
            .not_empty("name", "John")
            .min_length("name", "John", 2)
            .max_length("name", "John", 10)
            .build();

        assert!(result.is_valid());
    }

    #[test]
    fn test_validator_builder_failures() {
        let result = ValidatorBuilder::new()
            .not_empty("name", "")
            .min_length("password", "ab", 8)
            .build();

        assert!(!result.is_valid());
        assert_eq!(result.errors.len(), 2);
    }

    #[test]
    fn test_validator_builder_email() {
        assert!(ValidatorBuilder::new()
            .email("email", "user@example.com")
            .validate()
            .is_ok());

        assert!(ValidatorBuilder::new()
            .email("email", "invalid")
            .validate()
            .is_err());
    }

    #[test]
    fn test_validator_builder_uuid() {
        assert!(ValidatorBuilder::new()
            .uuid("id", "123e4567-e89b-12d3-a456-426614174000")
            .validate()
            .is_ok());

        assert!(ValidatorBuilder::new()
            .uuid("id", "invalid")
            .validate()
            .is_err());
    }

    #[test]
    fn test_validator_builder_range() {
        assert!(ValidatorBuilder::new()
            .range("age", 25, 0, 100)
            .validate()
            .is_ok());

        assert!(ValidatorBuilder::new()
            .range("age", 150, 0, 100)
            .validate()
            .is_err());
    }

    #[test]
    fn test_validator_builder_custom() {
        let value = 42;
        assert!(ValidatorBuilder::new()
            .custom("value", || value % 2 == 0, "must be even")
            .validate()
            .is_ok());

        let value = 43;
        assert!(ValidatorBuilder::new()
            .custom("value", || value % 2 == 0, "must be even")
            .validate()
            .is_err());
    }

    #[test]
    fn test_validator_builder_conditional() {
        let include_email = true;
        let result = ValidatorBuilder::new()
            .not_empty("name", "John")
            .when(include_email, |v| v.email("email", "user@example.com"))
            .validate();

        assert!(result.is_ok());
    }
}
