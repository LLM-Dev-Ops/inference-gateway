//! Input sanitization utilities.

use crate::config::ContentSecurityConfig;
use crate::error::{Result, SecurityError};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// XSS patterns to detect.
static XSS_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"(?i)<script[^>]*>").unwrap(),
        Regex::new(r"(?i)</script>").unwrap(),
        Regex::new(r"(?i)javascript:").unwrap(),
        Regex::new(r"(?i)vbscript:").unwrap(),
        Regex::new(r#"(?i)on\w+\s*="#).unwrap(),
        Regex::new(r"(?i)<iframe[^>]*>").unwrap(),
        Regex::new(r"(?i)<object[^>]*>").unwrap(),
        Regex::new(r"(?i)<embed[^>]*>").unwrap(),
        Regex::new(r"(?i)<form[^>]*>").unwrap(),
        Regex::new(r#"(?i)expression\s*\("#).unwrap(),
        Regex::new(r#"(?i)url\s*\(\s*['"]?data:"#).unwrap(),
    ]
});

/// SQL injection patterns to detect.
static SQL_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r#"(?i)\b(SELECT|INSERT|UPDATE|DELETE|DROP|UNION|ALTER|CREATE|TRUNCATE)\b.*\b(FROM|INTO|TABLE|DATABASE)\b"#).unwrap(),
        Regex::new(r"--").unwrap(),
        Regex::new(r#"/\*.*\*/"#).unwrap(),
        Regex::new(r#"(?i);\s*(SELECT|INSERT|UPDATE|DELETE|DROP)"#).unwrap(),
        Regex::new(r#"(?i)\b(OR|AND)\b\s+\d+\s*=\s*\d+"#).unwrap(),
        Regex::new(r#"(?i)\b(OR|AND)\b\s+['"]?\w+['"]?\s*=\s*['"]?\w+['"]?"#).unwrap(),
        Regex::new(r#"(?i)BENCHMARK\s*\("#).unwrap(),
        Regex::new(r#"(?i)SLEEP\s*\("#).unwrap(),
        Regex::new(r#"(?i)LOAD_FILE\s*\("#).unwrap(),
    ]
});

/// Command injection patterns to detect.
static COMMAND_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"[;&|`$]").unwrap(),
        Regex::new(r"\$\(").unwrap(),
        Regex::new(r"`[^`]+`").unwrap(),
        Regex::new(r"(?i)\b(eval|exec|system|shell_exec|passthru|popen)\b").unwrap(),
        Regex::new(r"(?i)\b(cmd|powershell|bash|sh|zsh)\b").unwrap(),
        Regex::new(r"(?i)/etc/passwd").unwrap(),
        Regex::new(r"(?i)/etc/shadow").unwrap(),
    ]
});

/// Path traversal patterns to detect.
static PATH_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"\.\./").unwrap(),
        Regex::new(r"\.\.\\").unwrap(),
        Regex::new(r"%2e%2e[/\\]").unwrap(),
        Regex::new(r"%252e%252e[/\\]").unwrap(),
        Regex::new(r"(?i)file://").unwrap(),
    ]
});

/// Sanitizer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizeConfig {
    /// Strip HTML tags.
    #[serde(default = "default_true")]
    pub strip_html: bool,

    /// Encode HTML entities.
    #[serde(default = "default_true")]
    pub encode_html: bool,

    /// Strip control characters.
    #[serde(default = "default_true")]
    pub strip_control_chars: bool,

    /// Strip null bytes.
    #[serde(default = "default_true")]
    pub strip_null_bytes: bool,

    /// Trim whitespace.
    #[serde(default = "default_true")]
    pub trim_whitespace: bool,

    /// Normalize line endings.
    #[serde(default)]
    pub normalize_line_endings: bool,

    /// Maximum length (0 = no limit).
    #[serde(default)]
    pub max_length: usize,

    /// Allowed HTML tags (for partial sanitization).
    #[serde(default)]
    pub allowed_tags: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl Default for SanitizeConfig {
    fn default() -> Self {
        Self {
            strip_html: true,
            encode_html: true,
            strip_control_chars: true,
            strip_null_bytes: true,
            trim_whitespace: true,
            normalize_line_endings: false,
            max_length: 0,
            allowed_tags: Vec::new(),
        }
    }
}

/// Input sanitizer.
#[derive(Debug, Clone)]
pub struct Sanitizer {
    config: SanitizeConfig,
    security_config: ContentSecurityConfig,
    custom_patterns: Vec<Regex>,
}

impl Sanitizer {
    /// Create a new sanitizer.
    #[must_use]
    pub fn new(config: SanitizeConfig) -> Self {
        Self {
            config,
            security_config: ContentSecurityConfig::default(),
            custom_patterns: Vec::new(),
        }
    }

    /// Create with default configuration.
    #[must_use]
    pub fn default_sanitizer() -> Self {
        Self::new(SanitizeConfig::default())
    }

    /// Set security configuration.
    #[must_use]
    pub fn with_security_config(mut self, config: ContentSecurityConfig) -> Self {
        self.security_config = config;
        self
    }

    /// Add custom forbidden patterns.
    #[must_use]
    pub fn with_patterns(mut self, patterns: Vec<Regex>) -> Self {
        self.custom_patterns = patterns;
        self
    }

    /// Sanitize a string.
    #[must_use]
    pub fn sanitize(&self, input: &str) -> String {
        let mut result = input.to_string();

        // Strip null bytes
        if self.config.strip_null_bytes {
            result = result.replace('\0', "");
        }

        // Strip control characters
        if self.config.strip_control_chars {
            result = strip_control_chars(&result);
        }

        // Strip HTML
        if self.config.strip_html {
            result = strip_html(&result);
        }

        // Encode HTML entities
        if self.config.encode_html && !self.config.strip_html {
            result = encode_html(&result);
        }

        // Normalize line endings
        if self.config.normalize_line_endings {
            result = normalize_line_endings(&result);
        }

        // Trim whitespace
        if self.config.trim_whitespace {
            result = result.trim().to_string();
        }

        // Truncate if needed
        if self.config.max_length > 0 && result.len() > self.config.max_length {
            result = result.chars().take(self.config.max_length).collect();
        }

        result
    }

    /// Check for malicious content without modifying input.
    ///
    /// # Errors
    /// Returns error if malicious content is detected.
    pub fn check(&self, input: &str) -> Result<()> {
        // Check XSS
        if self.security_config.xss_detection {
            if let Some(pattern) = XSS_PATTERNS.iter().find(|p| p.is_match(input)) {
                if self.security_config.log_events {
                    tracing::warn!(pattern = %pattern, "XSS pattern detected");
                }
                if self.security_config.block_on_detection {
                    return Err(SecurityError::forbidden("XSS pattern detected"));
                }
            }
        }

        // Check SQL injection
        if self.security_config.sql_injection_detection {
            if let Some(pattern) = SQL_PATTERNS.iter().find(|p| p.is_match(input)) {
                if self.security_config.log_events {
                    tracing::warn!(pattern = %pattern, "SQL injection pattern detected");
                }
                if self.security_config.block_on_detection {
                    return Err(SecurityError::forbidden("SQL injection pattern detected"));
                }
            }
        }

        // Check command injection
        if self.security_config.command_injection_detection {
            if let Some(pattern) = COMMAND_PATTERNS.iter().find(|p| p.is_match(input)) {
                if self.security_config.log_events {
                    tracing::warn!(pattern = %pattern, "Command injection pattern detected");
                }
                if self.security_config.block_on_detection {
                    return Err(SecurityError::forbidden("Command injection pattern detected"));
                }
            }
        }

        // Check path traversal
        if self.security_config.path_traversal_detection {
            if let Some(pattern) = PATH_PATTERNS.iter().find(|p| p.is_match(input)) {
                if self.security_config.log_events {
                    tracing::warn!(pattern = %pattern, "Path traversal pattern detected");
                }
                if self.security_config.block_on_detection {
                    return Err(SecurityError::forbidden("Path traversal pattern detected"));
                }
            }
        }

        // Check custom patterns
        for pattern in &self.custom_patterns {
            if pattern.is_match(input) {
                if self.security_config.log_events {
                    tracing::warn!(pattern = %pattern, "Custom forbidden pattern detected");
                }
                if self.security_config.block_on_detection {
                    return Err(SecurityError::forbidden("Forbidden content detected"));
                }
            }
        }

        // Check configured forbidden patterns
        for pattern_str in &self.security_config.forbidden_patterns {
            if let Ok(pattern) = Regex::new(pattern_str) {
                if pattern.is_match(input) {
                    if self.security_config.log_events {
                        tracing::warn!(pattern = %pattern_str, "Forbidden pattern detected");
                    }
                    if self.security_config.block_on_detection {
                        return Err(SecurityError::forbidden("Forbidden content detected"));
                    }
                }
            }
        }

        Ok(())
    }

    /// Sanitize and check for malicious content.
    ///
    /// # Errors
    /// Returns error if malicious content is detected.
    pub fn sanitize_and_check(&self, input: &str) -> Result<String> {
        let sanitized = self.sanitize(input);
        self.check(&sanitized)?;
        Ok(sanitized)
    }
}

/// Strip HTML tags from a string.
#[must_use]
pub fn strip_html(input: &str) -> String {
    static HTML_TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]*>").unwrap());
    HTML_TAG.replace_all(input, "").to_string()
}

/// Encode HTML entities.
#[must_use]
pub fn encode_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
        .replace('/', "&#x2F;")
}

/// Decode HTML entities.
#[must_use]
pub fn decode_html(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#x2F;", "/")
        .replace("&#39;", "'")
        .replace("&#47;", "/")
}

/// Strip control characters.
#[must_use]
pub fn strip_control_chars(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
        .collect()
}

/// Normalize line endings to Unix-style (\n).
#[must_use]
pub fn normalize_line_endings(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

/// Sanitize a file path to prevent directory traversal.
#[must_use]
pub fn sanitize_path(input: &str) -> String {
    input
        .replace("..", "")
        .replace("//", "/")
        .replace("\\\\", "\\")
        .replace("\\", "/")
        .trim_start_matches('/')
        .to_string()
}

/// Sanitize a filename.
#[must_use]
pub fn sanitize_filename(input: &str) -> String {
    static UNSAFE_CHARS: Lazy<Regex> = Lazy::new(|| Regex::new(r#"[<>:"/\\|?*\x00-\x1f]"#).unwrap());

    let sanitized = UNSAFE_CHARS.replace_all(input, "_").to_string();

    // Remove leading/trailing dots and spaces
    let sanitized = sanitized.trim_matches(|c| c == '.' || c == ' ');

    // Limit length
    if sanitized.len() > 255 {
        sanitized.chars().take(255).collect()
    } else {
        sanitized.to_string()
    }
}

/// Sanitize a URL parameter.
#[must_use]
pub fn sanitize_url_param(input: &str) -> String {
    url::form_urlencoded::byte_serialize(input.as_bytes()).collect()
}

/// Check if input contains potential XSS.
#[must_use]
pub fn contains_xss(input: &str) -> bool {
    XSS_PATTERNS.iter().any(|p| p.is_match(input))
}

/// Check if input contains potential SQL injection.
#[must_use]
pub fn contains_sql_injection(input: &str) -> bool {
    SQL_PATTERNS.iter().any(|p| p.is_match(input))
}

/// Check if input contains potential command injection.
#[must_use]
pub fn contains_command_injection(input: &str) -> bool {
    COMMAND_PATTERNS.iter().any(|p| p.is_match(input))
}

/// Check if input contains potential path traversal.
#[must_use]
pub fn contains_path_traversal(input: &str) -> bool {
    PATH_PATTERNS.iter().any(|p| p.is_match(input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html() {
        assert_eq!(strip_html("<p>Hello</p>"), "Hello");
        assert_eq!(strip_html("<script>alert('xss')</script>"), "alert('xss')");
        assert_eq!(strip_html("No tags here"), "No tags here");
    }

    #[test]
    fn test_encode_html() {
        assert_eq!(encode_html("<script>"), "&lt;script&gt;");
        assert_eq!(encode_html("a & b"), "a &amp; b");
        assert_eq!(encode_html("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_decode_html() {
        assert_eq!(decode_html("&lt;script&gt;"), "<script>");
        assert_eq!(decode_html("a &amp; b"), "a & b");
    }

    #[test]
    fn test_strip_control_chars() {
        assert_eq!(strip_control_chars("hello\x00world"), "helloworld");
        assert_eq!(strip_control_chars("hello\nworld"), "hello\nworld");
        assert_eq!(strip_control_chars("hello\x01\x02\x03world"), "helloworld");
    }

    #[test]
    fn test_normalize_line_endings() {
        assert_eq!(normalize_line_endings("hello\r\nworld"), "hello\nworld");
        assert_eq!(normalize_line_endings("hello\rworld"), "hello\nworld");
        assert_eq!(normalize_line_endings("hello\nworld"), "hello\nworld");
    }

    #[test]
    fn test_sanitize_path() {
        assert_eq!(sanitize_path("../../../etc/passwd"), "etc/passwd");
        assert_eq!(sanitize_path("/home/user/file"), "home/user/file");
        assert_eq!(sanitize_path("normal/path"), "normal/path");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("normal.txt"), "normal.txt");
        assert_eq!(sanitize_filename("file<name>.txt"), "file_name_.txt");
        assert_eq!(sanitize_filename("..hidden"), "hidden");
        assert_eq!(sanitize_filename("path/to/file"), "path_to_file");
    }

    #[test]
    fn test_sanitize_url_param() {
        assert_eq!(sanitize_url_param("hello world"), "hello+world");
        assert_eq!(sanitize_url_param("a=b&c=d"), "a%3Db%26c%3Dd");
    }

    #[test]
    fn test_contains_xss() {
        assert!(contains_xss("<script>alert('xss')</script>"));
        assert!(contains_xss("javascript:alert(1)"));
        assert!(contains_xss("<img onerror=alert(1)>"));
        assert!(!contains_xss("Normal text"));
    }

    #[test]
    fn test_contains_sql_injection() {
        assert!(contains_sql_injection("SELECT * FROM users"));
        assert!(contains_sql_injection("1; DROP TABLE users--"));
        assert!(contains_sql_injection("' OR '1'='1"));
        assert!(!contains_sql_injection("Normal text"));
    }

    #[test]
    fn test_contains_command_injection() {
        assert!(contains_command_injection("file; rm -rf /"));
        assert!(contains_command_injection("$(cat /etc/passwd)"));
        assert!(contains_command_injection("`whoami`"));
        // Note: this might be a false positive in some contexts
        // but it's safer to detect it
    }

    #[test]
    fn test_contains_path_traversal() {
        assert!(contains_path_traversal("../../../etc/passwd"));
        assert!(contains_path_traversal("..\\..\\..\\windows"));
        assert!(contains_path_traversal("%2e%2e/etc/passwd"));
        assert!(!contains_path_traversal("/normal/path"));
    }

    #[test]
    fn test_sanitizer() {
        let sanitizer = Sanitizer::default_sanitizer();

        let result = sanitizer.sanitize("  <p>Hello</p>\x00  ");
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_sanitizer_check_xss() {
        let sanitizer = Sanitizer::default_sanitizer();

        assert!(sanitizer.check("<script>alert('xss')</script>").is_err());
        assert!(sanitizer.check("Normal text").is_ok());
    }

    #[test]
    fn test_sanitizer_check_sql() {
        let sanitizer = Sanitizer::default_sanitizer();

        assert!(sanitizer.check("SELECT * FROM users").is_err());
        assert!(sanitizer.check("Just some text").is_ok());
    }

    #[test]
    fn test_sanitizer_check_path() {
        let sanitizer = Sanitizer::default_sanitizer();

        assert!(sanitizer.check("../../../etc/passwd").is_err());
        assert!(sanitizer.check("/normal/path/file.txt").is_ok());
    }

    #[test]
    fn test_sanitizer_with_custom_patterns() {
        let sanitizer = Sanitizer::default_sanitizer()
            .with_patterns(vec![Regex::new(r"forbidden").unwrap()]);

        assert!(sanitizer.check("This is forbidden content").is_err());
        assert!(sanitizer.check("This is allowed content").is_ok());
    }

    #[test]
    fn test_sanitizer_sanitize_and_check() {
        let sanitizer = Sanitizer::default_sanitizer();

        // After sanitization, HTML is stripped
        let result = sanitizer.sanitize_and_check("<p>Hello</p>");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello");
    }

    #[test]
    fn test_sanitize_config() {
        let config = SanitizeConfig {
            max_length: 10,
            ..Default::default()
        };
        let sanitizer = Sanitizer::new(config);

        let result = sanitizer.sanitize("This is a very long string");
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn test_sanitizer_permissive() {
        let sanitizer = Sanitizer::default_sanitizer().with_security_config(
            ContentSecurityConfig::permissive(),
        );

        // With permissive config, detection is disabled but doesn't block
        let result = sanitizer.check("<script>alert('xss')</script>");
        assert!(result.is_ok());
    }
}
