//! PII (Personally Identifiable Information) redaction for logs.
//!
//! This module provides enterprise-grade PII detection and redaction capabilities:
//! - Email addresses
//! - Phone numbers (various formats)
//! - Social Security Numbers (SSN)
//! - Credit card numbers
//! - IP addresses
//! - API keys and tokens
//! - Custom patterns
//!
//! # Example
//!
//! ```rust
//! use gateway_telemetry::pii::{PiiRedactor, PiiConfig, PiiPattern};
//!
//! let config = PiiConfig::default();
//! let redactor = PiiRedactor::new(config);
//!
//! let text = "Contact john@example.com or call 555-123-4567";
//! let redacted = redactor.redact(text);
//! assert!(!redacted.contains("john@example.com"));
//! assert!(!redacted.contains("555-123-4567"));
//! ```

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashSet;

/// Configuration for PII redaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiConfig {
    /// Enable PII redaction
    pub enabled: bool,

    /// Redaction replacement text
    pub replacement: RedactionStyle,

    /// Built-in patterns to enable
    pub patterns: PiiPatternConfig,

    /// Custom regex patterns to redact
    pub custom_patterns: Vec<CustomPattern>,

    /// Allowlist of specific values that should not be redacted
    pub allowlist: HashSet<String>,

    /// Hash sensitive values instead of replacing
    pub hash_values: bool,

    /// Include original character count in replacement
    pub include_length: bool,
}

impl Default for PiiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            replacement: RedactionStyle::default(),
            patterns: PiiPatternConfig::default(),
            custom_patterns: Vec::new(),
            allowlist: HashSet::new(),
            hash_values: false,
            include_length: false,
        }
    }
}

impl PiiConfig {
    /// Create a new configuration with all patterns enabled
    #[must_use]
    pub fn all_patterns() -> Self {
        Self {
            patterns: PiiPatternConfig::all(),
            ..Default::default()
        }
    }

    /// Create a minimal configuration with only basic patterns
    #[must_use]
    pub fn minimal() -> Self {
        Self {
            patterns: PiiPatternConfig::minimal(),
            ..Default::default()
        }
    }

    /// Builder: set enabled state
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Builder: set replacement style
    #[must_use]
    pub fn with_replacement(mut self, replacement: RedactionStyle) -> Self {
        self.replacement = replacement;
        self
    }

    /// Builder: add custom pattern
    #[must_use]
    pub fn with_custom_pattern(mut self, pattern: CustomPattern) -> Self {
        self.custom_patterns.push(pattern);
        self
    }

    /// Builder: add to allowlist
    #[must_use]
    pub fn with_allowed(mut self, value: impl Into<String>) -> Self {
        self.allowlist.insert(value.into());
        self
    }

    /// Builder: enable value hashing
    #[must_use]
    pub fn with_hashing(mut self, enabled: bool) -> Self {
        self.hash_values = enabled;
        self
    }
}

/// Style of redaction replacement
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RedactionStyle {
    /// Replace with a fixed string like "[REDACTED]"
    Fixed {
        /// The replacement text
        text: String,
    },

    /// Replace with category-specific placeholder like "[EMAIL]", "[PHONE]"
    Categorized,

    /// Replace with asterisks
    Masked {
        /// The character to use for masking
        char: char,
    },

    /// Replace with a hash of the original value
    Hashed {
        /// Prefix for the hash (e.g., "HASH")
        prefix: String,
    },
}

impl Default for RedactionStyle {
    fn default() -> Self {
        Self::Categorized
    }
}

/// Configuration for built-in PII patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiPatternConfig {
    /// Detect email addresses
    pub email: bool,
    /// Detect phone numbers
    pub phone: bool,
    /// Detect SSN (Social Security Numbers)
    pub ssn: bool,
    /// Detect credit card numbers
    pub credit_card: bool,
    /// Detect IPv4 addresses
    pub ipv4: bool,
    /// Detect IPv6 addresses
    pub ipv6: bool,
    /// Detect API keys (common patterns)
    pub api_keys: bool,
    /// Detect JWT tokens
    pub jwt: bool,
    /// Detect bearer tokens
    pub bearer_token: bool,
    /// Detect AWS access keys
    pub aws_keys: bool,
    /// Detect passport numbers
    pub passport: bool,
    /// Detect dates of birth
    pub date_of_birth: bool,
    /// Detect URLs with credentials
    pub url_credentials: bool,
    /// Detect base64 encoded secrets
    pub base64_secrets: bool,
}

impl Default for PiiPatternConfig {
    fn default() -> Self {
        Self {
            email: true,
            phone: true,
            ssn: true,
            credit_card: true,
            ipv4: true,
            ipv6: false, // Disabled by default (many false positives)
            api_keys: true,
            jwt: true,
            bearer_token: true,
            aws_keys: true,
            passport: false, // Country-specific, many false positives
            date_of_birth: false, // Many false positives
            url_credentials: true,
            base64_secrets: false, // Performance impact, many false positives
        }
    }
}

impl PiiPatternConfig {
    /// Enable all patterns
    #[must_use]
    pub fn all() -> Self {
        Self {
            email: true,
            phone: true,
            ssn: true,
            credit_card: true,
            ipv4: true,
            ipv6: true,
            api_keys: true,
            jwt: true,
            bearer_token: true,
            aws_keys: true,
            passport: true,
            date_of_birth: true,
            url_credentials: true,
            base64_secrets: true,
        }
    }

    /// Minimal set of patterns (fastest performance)
    #[must_use]
    pub fn minimal() -> Self {
        Self {
            email: true,
            phone: true,
            ssn: true,
            credit_card: true,
            ipv4: false,
            ipv6: false,
            api_keys: true,
            jwt: false,
            bearer_token: true,
            aws_keys: true,
            passport: false,
            date_of_birth: false,
            url_credentials: false,
            base64_secrets: false,
        }
    }
}

/// Custom pattern for PII detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPattern {
    /// Pattern name for replacement
    pub name: String,
    /// Regex pattern
    pub pattern: String,
    /// Optional replacement text (uses name if not specified)
    pub replacement: Option<String>,
}

impl CustomPattern {
    /// Create a new custom pattern
    pub fn new(name: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pattern: pattern.into(),
            replacement: None,
        }
    }

    /// Set custom replacement text
    #[must_use]
    pub fn with_replacement(mut self, replacement: impl Into<String>) -> Self {
        self.replacement = Some(replacement.into());
        self
    }
}

/// PII pattern types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PiiPattern {
    /// Email address
    Email,
    /// Phone number
    Phone,
    /// Social Security Number
    Ssn,
    /// Credit card number
    CreditCard,
    /// IPv4 address
    Ipv4,
    /// IPv6 address
    Ipv6,
    /// API key
    ApiKey,
    /// JWT token
    Jwt,
    /// Bearer token
    BearerToken,
    /// AWS access key
    AwsKey,
    /// Passport number
    Passport,
    /// Date of birth
    DateOfBirth,
    /// URL with credentials
    UrlCredentials,
    /// Base64 secret
    Base64Secret,
    /// Custom pattern
    Custom,
}

impl PiiPattern {
    /// Get the replacement placeholder for this pattern type
    #[must_use]
    pub fn placeholder(&self) -> &'static str {
        match self {
            Self::Email => "[EMAIL]",
            Self::Phone => "[PHONE]",
            Self::Ssn => "[SSN]",
            Self::CreditCard => "[CREDIT_CARD]",
            Self::Ipv4 => "[IP_ADDRESS]",
            Self::Ipv6 => "[IP_ADDRESS]",
            Self::ApiKey => "[API_KEY]",
            Self::Jwt => "[JWT_TOKEN]",
            Self::BearerToken => "[BEARER_TOKEN]",
            Self::AwsKey => "[AWS_KEY]",
            Self::Passport => "[PASSPORT]",
            Self::DateOfBirth => "[DOB]",
            Self::UrlCredentials => "[URL_CREDENTIALS]",
            Self::Base64Secret => "[SECRET]",
            Self::Custom => "[REDACTED]",
        }
    }
}

// Pre-compiled regex patterns for performance
static EMAIL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap()
});

static PHONE_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Matches various phone formats:
    // - +1-555-123-4567
    // - (555) 123-4567
    // - 555.123.4567
    // - 5551234567
    Regex::new(r"(?:\+?1[-.\s]?)?(?:\([0-9]{3}\)|[0-9]{3})[-.\s]?[0-9]{3}[-.\s]?[0-9]{4}").unwrap()
});

static SSN_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Matches XXX-XX-XXXX format and variations
    Regex::new(r"\b[0-9]{3}[-\s]?[0-9]{2}[-\s]?[0-9]{4}\b").unwrap()
});

static CREDIT_CARD_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Matches major credit card formats with optional spaces/dashes
    Regex::new(r"\b(?:4[0-9]{3}|5[1-5][0-9]{2}|6(?:011|5[0-9]{2})|3[47][0-9]{2}|3(?:0[0-5]|[68][0-9])[0-9])[-\s]?[0-9]{4}[-\s]?[0-9]{4}[-\s]?[0-9]{4}\b").unwrap()
});

static IPV4_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b").unwrap()
});

static IPV6_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:[0-9a-f]{1,4}:){7}[0-9a-f]{1,4}\b|(?:[0-9a-f]{1,4}:){1,7}:|(?:[0-9a-f]{1,4}:){1,6}:[0-9a-f]{1,4}").unwrap()
});

static JWT_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Matches JWT format: xxxxx.yyyyy.zzzzz (base64url encoded)
    Regex::new(r"\beyJ[A-Za-z0-9_-]*\.eyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]*\b").unwrap()
});

static BEARER_TOKEN_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)Bearer\s+[A-Za-z0-9_-]+").unwrap()
});

static API_KEY_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Common API key patterns
    Regex::new(r#"(?i)(?:api[_-]?key|apikey|api_secret|secret_key|access_token)[=:\s]+['"]?([A-Za-z0-9_-]{20,})['"]?"#).unwrap()
});

static AWS_ACCESS_KEY_REGEX: Lazy<Regex> = Lazy::new(|| {
    // AWS Access Key ID: AKIA followed by 16 characters
    Regex::new(r"\b(AKIA|ABIA|ACCA|ASIA)[A-Z0-9]{16}\b").unwrap()
});

static AWS_SECRET_KEY_REGEX: Lazy<Regex> = Lazy::new(|| {
    // AWS Secret Access Key: 40 character base64
    Regex::new(r#"(?i)(?:aws[_-]?secret|secret[_-]?key)[=:\s]+['"]?([A-Za-z0-9/+=]{40})['"]?"#).unwrap()
});

static URL_CREDENTIALS_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Matches URLs with embedded credentials: https://user:pass@host.com
    // Pattern: protocol :// username : password @ host
    Regex::new(r"(?i)(?:https?|ftp)://[^/\s@:]+:[^/\s@]+@[^\s]+").unwrap()
});

static BASE64_SECRET_REGEX: Lazy<Regex> = Lazy::new(|| {
    // Long base64 strings that might be secrets (min 32 chars)
    Regex::new(r#"(?i)(?:secret|password|token|key|credential)[=:\s]+['"]?([A-Za-z0-9+/]{32,}={0,2})['"]?"#).unwrap()
});

static OPENAI_KEY_REGEX: Lazy<Regex> = Lazy::new(|| {
    // OpenAI keys: sk- followed by alphanumeric chars and dashes (new format includes proj-, live-, etc.)
    Regex::new(r"\bsk-[A-Za-z0-9_-]{20,}\b").unwrap()
});

static ANTHROPIC_KEY_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\bsk-ant-[A-Za-z0-9_-]{20,}\b").unwrap()
});

/// PII Redactor - detects and redacts PII from text
#[derive(Debug, Clone)]
pub struct PiiRedactor {
    config: PiiConfig,
    custom_regexes: Vec<(String, Regex)>,
}

impl PiiRedactor {
    /// Create a new PII redactor with the given configuration
    pub fn new(config: PiiConfig) -> Self {
        let custom_regexes = config
            .custom_patterns
            .iter()
            .filter_map(|p| {
                Regex::new(&p.pattern).ok().map(|r| {
                    (
                        p.replacement
                            .clone()
                            .unwrap_or_else(|| format!("[{}]", p.name.to_uppercase())),
                        r,
                    )
                })
            })
            .collect();

        Self {
            config,
            custom_regexes,
        }
    }

    /// Create a redactor with default configuration
    #[must_use]
    pub fn default_config() -> Self {
        Self::new(PiiConfig::default())
    }

    /// Redact PII from the given text
    #[must_use]
    pub fn redact<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if !self.config.enabled || text.is_empty() {
            return Cow::Borrowed(text);
        }

        let mut result = text.to_string();
        let mut redacted = false;

        // Apply built-in patterns
        // NOTE: URL credentials MUST be processed before email to avoid partial matches
        // (e.g., "user:pass@host.com" could match email pattern on "pass@host.com")
        if self.config.patterns.url_credentials {
            if let Some(new_result) =
                self.apply_pattern(&result, &URL_CREDENTIALS_REGEX, PiiPattern::UrlCredentials)
            {
                result = new_result;
                redacted = true;
            }
        }

        if self.config.patterns.email {
            if let Some(new_result) = self.apply_pattern(&result, &EMAIL_REGEX, PiiPattern::Email) {
                result = new_result;
                redacted = true;
            }
        }

        if self.config.patterns.phone {
            if let Some(new_result) = self.apply_pattern(&result, &PHONE_REGEX, PiiPattern::Phone) {
                result = new_result;
                redacted = true;
            }
        }

        if self.config.patterns.ssn {
            if let Some(new_result) = self.apply_pattern(&result, &SSN_REGEX, PiiPattern::Ssn) {
                result = new_result;
                redacted = true;
            }
        }

        if self.config.patterns.credit_card {
            if let Some(new_result) =
                self.apply_pattern(&result, &CREDIT_CARD_REGEX, PiiPattern::CreditCard)
            {
                result = new_result;
                redacted = true;
            }
        }

        if self.config.patterns.ipv4 {
            if let Some(new_result) = self.apply_pattern(&result, &IPV4_REGEX, PiiPattern::Ipv4) {
                result = new_result;
                redacted = true;
            }
        }

        if self.config.patterns.ipv6 {
            if let Some(new_result) = self.apply_pattern(&result, &IPV6_REGEX, PiiPattern::Ipv6) {
                result = new_result;
                redacted = true;
            }
        }

        if self.config.patterns.jwt {
            if let Some(new_result) = self.apply_pattern(&result, &JWT_REGEX, PiiPattern::Jwt) {
                result = new_result;
                redacted = true;
            }
        }

        if self.config.patterns.bearer_token {
            if let Some(new_result) =
                self.apply_pattern(&result, &BEARER_TOKEN_REGEX, PiiPattern::BearerToken)
            {
                result = new_result;
                redacted = true;
            }
        }

        if self.config.patterns.api_keys {
            // OpenAI keys
            if let Some(new_result) =
                self.apply_pattern(&result, &OPENAI_KEY_REGEX, PiiPattern::ApiKey)
            {
                result = new_result;
                redacted = true;
            }
            // Anthropic keys
            if let Some(new_result) =
                self.apply_pattern(&result, &ANTHROPIC_KEY_REGEX, PiiPattern::ApiKey)
            {
                result = new_result;
                redacted = true;
            }
            // Generic API keys
            if let Some(new_result) =
                self.apply_pattern(&result, &API_KEY_REGEX, PiiPattern::ApiKey)
            {
                result = new_result;
                redacted = true;
            }
        }

        if self.config.patterns.aws_keys {
            if let Some(new_result) =
                self.apply_pattern(&result, &AWS_ACCESS_KEY_REGEX, PiiPattern::AwsKey)
            {
                result = new_result;
                redacted = true;
            }
            if let Some(new_result) =
                self.apply_pattern(&result, &AWS_SECRET_KEY_REGEX, PiiPattern::AwsKey)
            {
                result = new_result;
                redacted = true;
            }
        }

        // URL credentials is processed at the start of the method

        if self.config.patterns.base64_secrets {
            if let Some(new_result) =
                self.apply_pattern(&result, &BASE64_SECRET_REGEX, PiiPattern::Base64Secret)
            {
                result = new_result;
                redacted = true;
            }
        }

        // Apply custom patterns
        for (replacement, regex) in &self.custom_regexes {
            if regex.is_match(&result) {
                // Collect match info (range and text) before modifying result
                let matches: Vec<(std::ops::Range<usize>, String)> = regex
                    .find_iter(&result)
                    .map(|m| (m.range(), m.as_str().to_string()))
                    .collect();
                for (range, matched_text) in matches.into_iter().rev() {
                    if !self.config.allowlist.contains(&matched_text) {
                        result.replace_range(range, replacement);
                        redacted = true;
                    }
                }
            }
        }

        if redacted {
            Cow::Owned(result)
        } else {
            Cow::Borrowed(text)
        }
    }

    /// Apply a pattern and return the result if any matches were found
    fn apply_pattern(&self, text: &str, regex: &Regex, pattern: PiiPattern) -> Option<String> {
        if !regex.is_match(text) {
            return None;
        }

        let matches: Vec<_> = regex.find_iter(text).collect();
        let mut result = text.to_string();

        // Process matches in reverse order to preserve indices
        for m in matches.into_iter().rev() {
            let matched_text = m.as_str();

            // Skip allowlisted values
            if self.config.allowlist.contains(matched_text) {
                continue;
            }

            let replacement = self.get_replacement(matched_text, pattern);
            result.replace_range(m.range(), &replacement);
        }

        Some(result)
    }

    /// Get the replacement string for a matched value
    fn get_replacement(&self, original: &str, pattern: PiiPattern) -> String {
        match &self.config.replacement {
            RedactionStyle::Fixed { text } => {
                if self.config.include_length {
                    format!("{}({})", text, original.len())
                } else {
                    text.clone()
                }
            }
            RedactionStyle::Categorized => {
                if self.config.include_length {
                    format!("{}({})", pattern.placeholder(), original.len())
                } else {
                    pattern.placeholder().to_string()
                }
            }
            RedactionStyle::Masked { char } => char.to_string().repeat(original.len()),
            RedactionStyle::Hashed { prefix } => {
                let hash = self.hash_value(original);
                format!("{}:{}", prefix, &hash[..8])
            }
        }
    }

    /// Hash a value for replacement
    fn hash_value(&self, value: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Check if text contains any PII
    #[must_use]
    pub fn contains_pii(&self, text: &str) -> bool {
        if !self.config.enabled || text.is_empty() {
            return false;
        }

        if self.config.patterns.email && EMAIL_REGEX.is_match(text) {
            return true;
        }
        if self.config.patterns.phone && PHONE_REGEX.is_match(text) {
            return true;
        }
        if self.config.patterns.ssn && SSN_REGEX.is_match(text) {
            return true;
        }
        if self.config.patterns.credit_card && CREDIT_CARD_REGEX.is_match(text) {
            return true;
        }
        if self.config.patterns.ipv4 && IPV4_REGEX.is_match(text) {
            return true;
        }
        if self.config.patterns.ipv6 && IPV6_REGEX.is_match(text) {
            return true;
        }
        if self.config.patterns.jwt && JWT_REGEX.is_match(text) {
            return true;
        }
        if self.config.patterns.bearer_token && BEARER_TOKEN_REGEX.is_match(text) {
            return true;
        }
        if self.config.patterns.api_keys {
            if OPENAI_KEY_REGEX.is_match(text)
                || ANTHROPIC_KEY_REGEX.is_match(text)
                || API_KEY_REGEX.is_match(text)
            {
                return true;
            }
        }
        if self.config.patterns.aws_keys
            && (AWS_ACCESS_KEY_REGEX.is_match(text) || AWS_SECRET_KEY_REGEX.is_match(text))
        {
            return true;
        }
        if self.config.patterns.url_credentials && URL_CREDENTIALS_REGEX.is_match(text) {
            return true;
        }

        // Check custom patterns
        for (_, regex) in &self.custom_regexes {
            if regex.is_match(text) {
                return true;
            }
        }

        false
    }

    /// Get statistics about PII found in text
    #[must_use]
    pub fn analyze(&self, text: &str) -> PiiAnalysis {
        let mut analysis = PiiAnalysis::default();

        if !self.config.enabled || text.is_empty() {
            return analysis;
        }

        if self.config.patterns.email {
            analysis.email_count = EMAIL_REGEX.find_iter(text).count();
        }
        if self.config.patterns.phone {
            analysis.phone_count = PHONE_REGEX.find_iter(text).count();
        }
        if self.config.patterns.ssn {
            analysis.ssn_count = SSN_REGEX.find_iter(text).count();
        }
        if self.config.patterns.credit_card {
            analysis.credit_card_count = CREDIT_CARD_REGEX.find_iter(text).count();
        }
        if self.config.patterns.ipv4 {
            analysis.ip_count += IPV4_REGEX.find_iter(text).count();
        }
        if self.config.patterns.ipv6 {
            analysis.ip_count += IPV6_REGEX.find_iter(text).count();
        }
        if self.config.patterns.api_keys {
            analysis.api_key_count += OPENAI_KEY_REGEX.find_iter(text).count();
            analysis.api_key_count += ANTHROPIC_KEY_REGEX.find_iter(text).count();
            analysis.api_key_count += API_KEY_REGEX.find_iter(text).count();
        }
        if self.config.patterns.jwt {
            analysis.token_count += JWT_REGEX.find_iter(text).count();
        }
        if self.config.patterns.bearer_token {
            analysis.token_count += BEARER_TOKEN_REGEX.find_iter(text).count();
        }
        if self.config.patterns.aws_keys {
            analysis.api_key_count += AWS_ACCESS_KEY_REGEX.find_iter(text).count();
            analysis.api_key_count += AWS_SECRET_KEY_REGEX.find_iter(text).count();
        }

        analysis.total_pii_found = analysis.email_count
            + analysis.phone_count
            + analysis.ssn_count
            + analysis.credit_card_count
            + analysis.ip_count
            + analysis.api_key_count
            + analysis.token_count;

        analysis
    }
}

/// Analysis results from PII scanning
#[derive(Debug, Clone, Default)]
pub struct PiiAnalysis {
    /// Number of email addresses found
    pub email_count: usize,
    /// Number of phone numbers found
    pub phone_count: usize,
    /// Number of SSNs found
    pub ssn_count: usize,
    /// Number of credit card numbers found
    pub credit_card_count: usize,
    /// Number of IP addresses found
    pub ip_count: usize,
    /// Number of API keys found
    pub api_key_count: usize,
    /// Number of tokens found
    pub token_count: usize,
    /// Total PII items found
    pub total_pii_found: usize,
}

impl PiiAnalysis {
    /// Check if any PII was found
    #[must_use]
    pub fn has_pii(&self) -> bool {
        self.total_pii_found > 0
    }
}

/// Extension trait for redacting PII from strings
pub trait RedactPii {
    /// Redact PII using the default redactor
    fn redact_pii(&self) -> String;

    /// Redact PII using a custom redactor
    fn redact_pii_with(&self, redactor: &PiiRedactor) -> String;
}

impl RedactPii for str {
    fn redact_pii(&self) -> String {
        static DEFAULT_REDACTOR: Lazy<PiiRedactor> =
            Lazy::new(|| PiiRedactor::new(PiiConfig::default()));
        DEFAULT_REDACTOR.redact(self).into_owned()
    }

    fn redact_pii_with(&self, redactor: &PiiRedactor) -> String {
        redactor.redact(self).into_owned()
    }
}

impl RedactPii for String {
    fn redact_pii(&self) -> String {
        self.as_str().redact_pii()
    }

    fn redact_pii_with(&self, redactor: &PiiRedactor) -> String {
        self.as_str().redact_pii_with(redactor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_redaction() {
        let redactor = PiiRedactor::default_config();

        let text = "Contact john.doe@example.com for help";
        let redacted = redactor.redact(text);
        assert!(!redacted.contains("john.doe@example.com"));
        assert!(redacted.contains("[EMAIL]"));
    }

    #[test]
    fn test_phone_redaction() {
        let redactor = PiiRedactor::default_config();

        // Various phone formats
        assert!(redactor
            .redact("Call 555-123-4567")
            .contains("[PHONE]"));
        assert!(redactor
            .redact("Call (555) 123-4567")
            .contains("[PHONE]"));
        assert!(redactor
            .redact("Call +1-555-123-4567")
            .contains("[PHONE]"));
        assert!(redactor
            .redact("Call 555.123.4567")
            .contains("[PHONE]"));
    }

    #[test]
    fn test_ssn_redaction() {
        let redactor = PiiRedactor::default_config();

        let text = "SSN: 123-45-6789";
        let redacted = redactor.redact(text);
        assert!(!redacted.contains("123-45-6789"));
        assert!(redacted.contains("[SSN]"));
    }

    #[test]
    fn test_credit_card_redaction() {
        let redactor = PiiRedactor::default_config();

        // Visa
        assert!(redactor
            .redact("Card: 4111-1111-1111-1111")
            .contains("[CREDIT_CARD]"));
        // Mastercard
        assert!(redactor
            .redact("Card: 5500 0000 0000 0004")
            .contains("[CREDIT_CARD]"));
    }

    #[test]
    fn test_ip_address_redaction() {
        let redactor = PiiRedactor::default_config();

        let text = "Client IP: 192.168.1.100";
        let redacted = redactor.redact(text);
        assert!(!redacted.contains("192.168.1.100"));
        assert!(redacted.contains("[IP_ADDRESS]"));
    }

    #[test]
    fn test_api_key_redaction() {
        let redactor = PiiRedactor::default_config();

        // OpenAI key - use realistic format (no SSN-like patterns)
        let text = "Key: sk-proj-abcdefghijklmnopqrstuvwxyz";
        let redacted = redactor.redact(text);
        assert!(!redacted.contains("sk-proj-"));
        assert!(redacted.contains("[API_KEY]"));

        // Anthropic key
        let text2 = "Key: sk-ant-api-abcdefghijklmnopqrstuvwxyz";
        let redacted2 = redactor.redact(text2);
        assert!(!redacted2.contains("sk-ant-"));
        assert!(redacted2.contains("[API_KEY]"));
    }

    #[test]
    fn test_jwt_redaction() {
        let redactor = PiiRedactor::default_config();

        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let text = format!("Token: {}", jwt);
        let redacted = redactor.redact(&text);
        assert!(!redacted.contains("eyJ"));
        assert!(redacted.contains("[JWT_TOKEN]"));
    }

    #[test]
    fn test_bearer_token_redaction() {
        let redactor = PiiRedactor::default_config();

        let text = "Authorization: Bearer abc123xyz789token";
        let redacted = redactor.redact(text);
        assert!(!redacted.contains("abc123xyz789token"));
        assert!(redacted.contains("[BEARER_TOKEN]"));
    }

    #[test]
    fn test_aws_key_redaction() {
        let redactor = PiiRedactor::default_config();

        let text = "AWS Key: AKIAIOSFODNN7EXAMPLE";
        let redacted = redactor.redact(text);
        assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(redacted.contains("[AWS_KEY]"));
    }

    #[test]
    fn test_url_credentials_redaction() {
        let redactor = PiiRedactor::default_config();

        // Test the regex directly
        assert!(
            URL_CREDENTIALS_REGEX.is_match("https://admin:secretpass@db.example.com"),
            "URL regex should match"
        );

        let text = "Connect to https://admin:secretpass@db.example.com/mydb";
        let redacted = redactor.redact(text);
        assert!(!redacted.contains("secretpass"), "Password should be redacted. Got: {}", redacted);
        assert!(redacted.contains("[URL_CREDENTIALS]"), "Should contain placeholder. Got: {}", redacted);
    }

    #[test]
    fn test_multiple_pii_types() {
        let redactor = PiiRedactor::default_config();

        let text = "User john@example.com called from 555-123-4567, SSN 123-45-6789";
        let redacted = redactor.redact(text);

        assert!(!redacted.contains("john@example.com"));
        assert!(!redacted.contains("555-123-4567"));
        assert!(!redacted.contains("123-45-6789"));
        assert!(redacted.contains("[EMAIL]"));
        assert!(redacted.contains("[PHONE]"));
        assert!(redacted.contains("[SSN]"));
    }

    #[test]
    fn test_allowlist() {
        let config = PiiConfig::default().with_allowed("allowed@example.com");
        let redactor = PiiRedactor::new(config);

        let text = "Contact allowed@example.com or blocked@example.com";
        let redacted = redactor.redact(text);

        assert!(redacted.contains("allowed@example.com"));
        assert!(!redacted.contains("blocked@example.com"));
    }

    #[test]
    fn test_fixed_replacement() {
        let config = PiiConfig {
            replacement: RedactionStyle::Fixed {
                text: "***REDACTED***".to_string(),
            },
            ..Default::default()
        };
        let redactor = PiiRedactor::new(config);

        let text = "Email: test@example.com";
        let redacted = redactor.redact(text);
        assert!(redacted.contains("***REDACTED***"));
    }

    #[test]
    fn test_masked_replacement() {
        let config = PiiConfig {
            replacement: RedactionStyle::Masked { char: '*' },
            ..Default::default()
        };
        let redactor = PiiRedactor::new(config);

        let text = "SSN: 123-45-6789";
        let redacted = redactor.redact(text);
        // 11 characters in SSN with dashes
        assert!(redacted.contains("***********"));
    }

    #[test]
    fn test_hashed_replacement() {
        let config = PiiConfig {
            replacement: RedactionStyle::Hashed {
                prefix: "HASH".to_string(),
            },
            ..Default::default()
        };
        let redactor = PiiRedactor::new(config);

        let text = "Email: test@example.com";
        let redacted = redactor.redact(text);
        assert!(redacted.contains("HASH:"));
    }

    #[test]
    fn test_include_length() {
        let config = PiiConfig {
            include_length: true,
            ..Default::default()
        };
        let redactor = PiiRedactor::new(config);

        let text = "Email: test@example.com"; // 16 chars
        let redacted = redactor.redact(text);
        assert!(redacted.contains("[EMAIL](16)"));
    }

    #[test]
    fn test_custom_pattern() {
        let config = PiiConfig::default()
            .with_custom_pattern(CustomPattern::new("employee_id", r"EMP-[0-9]{6}"));
        let redactor = PiiRedactor::new(config);

        let text = "Employee EMP-123456 requested access";
        let redacted = redactor.redact(text);
        assert!(!redacted.contains("EMP-123456"));
        assert!(redacted.contains("[EMPLOYEE_ID]"));
    }

    #[test]
    fn test_contains_pii() {
        let redactor = PiiRedactor::default_config();

        assert!(redactor.contains_pii("Contact test@example.com"));
        assert!(redactor.contains_pii("Call 555-123-4567"));
        assert!(redactor.contains_pii("SSN: 123-45-6789"));
        assert!(!redactor.contains_pii("Hello, World!"));
    }

    #[test]
    fn test_analyze() {
        let redactor = PiiRedactor::default_config();

        let text =
            "Contact john@example.com and jane@example.com, call 555-123-4567, SSN: 123-45-6789";
        let analysis = redactor.analyze(text);

        assert_eq!(analysis.email_count, 2);
        assert_eq!(analysis.phone_count, 1);
        assert_eq!(analysis.ssn_count, 1);
        assert_eq!(analysis.total_pii_found, 4);
        assert!(analysis.has_pii());
    }

    #[test]
    fn test_disabled_redaction() {
        let config = PiiConfig {
            enabled: false,
            ..Default::default()
        };
        let redactor = PiiRedactor::new(config);

        let text = "Contact test@example.com";
        let redacted = redactor.redact(text);
        assert_eq!(redacted, text);
    }

    #[test]
    fn test_empty_text() {
        let redactor = PiiRedactor::default_config();
        let redacted = redactor.redact("");
        assert_eq!(redacted, "");
    }

    #[test]
    fn test_no_pii() {
        let redactor = PiiRedactor::default_config();
        let text = "Hello, this is a regular message with no sensitive data.";
        let redacted = redactor.redact(text);
        assert_eq!(redacted, text);
    }

    #[test]
    fn test_trait_extension() {
        let text = "Contact test@example.com";
        let redacted = text.redact_pii();
        assert!(redacted.contains("[EMAIL]"));
    }

    #[test]
    fn test_minimal_config() {
        let config = PiiConfig::minimal();
        let redactor = PiiRedactor::new(config);

        // Should still detect basics
        assert!(redactor.contains_pii("test@example.com"));
        assert!(redactor.contains_pii("555-123-4567"));

        // But not IPv4 (disabled in minimal)
        let config2 = PiiConfig::minimal();
        assert!(!config2.patterns.ipv4);
    }

    #[test]
    fn test_all_patterns_config() {
        let config = PiiConfig::all_patterns();
        assert!(config.patterns.email);
        assert!(config.patterns.ipv6);
        assert!(config.patterns.passport);
        assert!(config.patterns.base64_secrets);
    }
}
