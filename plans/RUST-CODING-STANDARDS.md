# Rust Coding Standards - LLM Inference Gateway

**Version:** 1.0.0
**Last Updated:** November 2024
**Applies To:** All Rust code in the LLM Inference Gateway project

## Table of Contents

1. [Rust Style Guide](#1-rust-style-guide)
2. [Clippy Configuration](#2-clippy-configuration)
3. [Documentation Standards](#3-documentation-standards)
4. [Error Handling Rules](#4-error-handling-rules)
5. [Testing Standards](#5-testing-standards)
6. [Performance Guidelines](#6-performance-guidelines)
7. [Security Requirements](#7-security-requirements)
8. [Pre-commit Hooks](#8-pre-commit-hooks)
9. [Code Review Checklist](#9-code-review-checklist)

---

## 1. Rust Style Guide

### 1.1 Naming Conventions

| Item | Convention | Example | Notes |
|------|------------|---------|-------|
| **Types** | PascalCase | `GatewayRequest`, `ProviderConfig` | Struct, enum, trait |
| **Functions** | snake_case | `handle_request`, `parse_response` | Methods, functions |
| **Constants** | SCREAMING_SNAKE | `MAX_RETRIES`, `DEFAULT_TIMEOUT` | Static values |
| **Modules** | snake_case | `circuit_breaker`, `rate_limiter` | File/directory names |
| **Traits** | PascalCase + suffix | `Serializable`, `Provider`, `Handler` | Prefer -able/-er |
| **Type Parameters** | Single uppercase | `T`, `E`, `K`, `V` | Generic parameters |
| **Lifetimes** | Short lowercase | `'a`, `'de`, `'static` | Lifetime annotations |
| **Macros** | snake_case | `retry!`, `measure_time!` | Macro definitions |

### 1.2 Code Organization

#### Module Structure

```
src/
├── lib.rs              # Public API surface
├── main.rs             # Binary entry point
├── core/               # Core abstractions
│   ├── mod.rs
│   ├── models.rs       # Data models
│   └── traits.rs       # Core traits
├── providers/          # Provider implementations
│   ├── mod.rs
│   ├── openai.rs
│   ├── anthropic.rs
│   └── common/
│       ├── mod.rs
│       ├── client.rs
│       └── errors.rs
├── middleware/         # Middleware pipeline
│   ├── mod.rs
│   ├── auth.rs
│   └── rate_limit.rs
├── infrastructure/     # Infrastructure concerns
│   ├── mod.rs
│   ├── cache.rs
│   └── metrics.rs
└── utils/              # Shared utilities
    ├── mod.rs
    └── retry.rs
```

#### Import Ordering

```rust
// 1. Standard library
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// 2. External crates (alphabetical)
use anyhow::{Context, Result};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tracing::{debug, info, warn};

// 3. Internal modules (alphabetical, grouped by depth)
use crate::core::models::{ChatRequest, ChatResponse};
use crate::core::traits::Provider;
use crate::providers::openai::OpenAIProvider;
```

#### Visibility Rules

```rust
// PREFER: Explicit crate-local visibility
pub(crate) struct InternalConfig {
    pub(crate) max_retries: u32,
}

// AVOID: Unnecessary public exposure
pub struct InternalConfig { // Don't expose if not needed
    pub max_retries: u32,    // Don't expose if not needed
}

// OK: Clear public API
pub struct GatewayRequest {
    pub model: String,
    pub messages: Vec<Message>,
}
```

### 1.3 Code Formatting

```rust
// Line length: 100 characters (enforced by rustfmt)
// Indentation: 4 spaces (no tabs)
// Trailing commas: Required in multi-line
// Brace style: Same line for structs/enums, next line for impl/fn blocks

// CORRECT
pub struct Config {
    pub timeout: Duration,
    pub max_retries: u32,
    pub base_url: String,  // Trailing comma
}

impl Config {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            max_retries: 3,
            base_url: String::from("https://api.example.com"),
        }
    }
}

// Function arguments: One per line if exceeds 100 chars
pub async fn process_request(
    request: ChatRequest,
    provider: Arc<dyn Provider>,
    cache: Arc<Cache>,
) -> Result<ChatResponse> {
    // Implementation
}
```

### 1.4 Type Design

```rust
// PREFER: NewType pattern for domain concepts
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn new(key: impl Into<String>) -> Result<Self> {
        let key = key.into();
        if key.is_empty() || !key.starts_with("sk-") {
            anyhow::bail!("Invalid API key format");
        }
        Ok(Self(key))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// PREFER: Builder pattern for complex types
#[derive(Debug)]
pub struct ProviderConfig {
    base_url: String,
    timeout: Duration,
    max_retries: u32,
    rate_limit: Option<RateLimitConfig>,
}

impl ProviderConfig {
    pub fn builder() -> ProviderConfigBuilder {
        ProviderConfigBuilder::default()
    }
}
```

---

## 2. Clippy Configuration

### 2.1 Complete clippy.toml

```toml
# /clippy.toml
# Clippy configuration for LLM Inference Gateway

# Line length matching rustfmt
max-suggested-length = 100

# Cognitive complexity threshold
cognitive-complexity-threshold = 25

# Type complexity threshold
type-complexity-threshold = 250

# Too many arguments warning threshold
too-many-arguments-threshold = 7

# Single character binding names allowed
single-char-binding-names-threshold = 4

# Documentation configuration
missing-docs-in-crate-items = false
```

### 2.2 Enabled Lints (Cargo.toml)

```toml
[lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"
unused_results = "warn"

[lints.clippy]
# Correctness (deny - bugs that must be fixed)
correctness = "deny"

# Pedantic (warn - best practices)
pedantic = "warn"
nursery = "warn"

# Performance critical
perf = "deny"

# Specific denials
unwrap_used = "deny"
expect_used = "warn"
panic = "deny"
todo = "warn"
unimplemented = "deny"

# Async patterns
future_not_send = "warn"

# Documentation
missing_errors_doc = "warn"
missing_panics_doc = "warn"

# Allowed exceptions (with rationale)
[lints.clippy.allow]
# Allow module name repetition for clarity
module_name_repetitions = "allow"
# Allow similar names (e.g., req/res, tx/rx)
similar_names = "allow"
# Allow inline format args (readability preference)
uninlined_format_args = "allow"
```

### 2.3 Project-Specific Lints

```rust
// In lib.rs or main.rs
#![deny(
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications,
)]

#![warn(
    missing_docs,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
)]

#![allow(
    clippy::module_name_repetitions,
    clippy::similar_names,
)]
```

---

## 3. Documentation Standards

### 3.1 Required Documentation

| Item | Requirement | Enforcement |
|------|-------------|-------------|
| Public modules | Module-level docs with examples | CI check |
| Public structs/enums | Type documentation | CI check |
| Public functions | Full documentation | CI check |
| Error types | Error scenarios with examples | Code review |
| Unsafe blocks | SAFETY comment explaining invariants | Deny compilation |
| Complex algorithms | Inline explanation comments | Code review |

### 3.2 Documentation Template

```rust
/// Processes a chat completion request through the provider abstraction layer.
///
/// This function handles the full lifecycle of a chat request:
/// 1. Validates the request against provider capabilities
/// 2. Applies rate limiting and authentication
/// 3. Routes to the appropriate provider implementation
/// 4. Handles retries and circuit breaking
/// 5. Caches successful responses
///
/// # Arguments
///
/// * `request` - The chat completion request containing messages and parameters
/// * `provider` - The LLM provider to use (OpenAI, Anthropic, etc.)
/// * `cache` - Optional cache for response memoization
///
/// # Returns
///
/// Returns `Ok(ChatResponse)` on success containing:
/// - Generated completion text
/// - Token usage statistics
/// - Provider metadata
///
/// # Errors
///
/// This function will return an error if:
/// * `ProviderError::RateLimitExceeded` - Provider rate limit hit
/// * `ProviderError::AuthenticationFailed` - Invalid API key
/// * `ProviderError::ServiceUnavailable` - Provider API is down
/// * `ProviderError::InvalidRequest` - Request validation failed
///
/// # Examples
///
/// ```rust
/// use llm_gateway::core::models::{ChatRequest, Message};
/// use llm_gateway::providers::openai::OpenAIProvider;
///
/// # async fn example() -> anyhow::Result<()> {
/// let provider = OpenAIProvider::new("sk-...")?;
/// let request = ChatRequest {
///     model: "gpt-4".to_string(),
///     messages: vec![Message::user("Hello, world!")],
///     temperature: Some(0.7),
///     ..Default::default()
/// };
///
/// let response = process_chat_request(request, provider, None).await?;
/// println!("Response: {}", response.text);
/// # Ok(())
/// # }
/// ```
///
/// # Panics
///
/// Never panics. All error conditions are returned as `Result::Err`.
///
/// # Performance
///
/// - Cold path (cache miss): ~150-500ms depending on provider
/// - Hot path (cache hit): ~5-10ms
/// - Memory: ~2KB per request (excluding response body)
pub async fn process_chat_request(
    request: ChatRequest,
    provider: Arc<dyn Provider>,
    cache: Option<Arc<Cache>>,
) -> Result<ChatResponse, ProviderError> {
    // Implementation
}
```

### 3.3 Module Documentation

```rust
//! Circuit breaker implementation for resilient provider communication.
//!
//! This module implements the circuit breaker pattern to prevent cascading failures
//! when downstream LLM providers become unavailable or degraded.
//!
//! # Architecture
//!
//! The circuit breaker has three states:
//! - **Closed**: Normal operation, requests flow through
//! - **Open**: Provider is failing, requests fail fast
//! - **Half-Open**: Testing if provider has recovered
//!
//! # State Transitions
//!
//! ```text
//! Closed --> Open: After threshold failures
//! Open --> Half-Open: After timeout period
//! Half-Open --> Closed: If test requests succeed
//! Half-Open --> Open: If test requests fail
//! ```
//!
//! # Examples
//!
//! ```rust
//! use llm_gateway::resilience::CircuitBreaker;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let breaker = CircuitBreaker::new(5, Duration::from_secs(30));
//!
//! match breaker.call(|| async { provider.chat(request).await }).await {
//!     Ok(response) => println!("Success: {response:?}"),
//!     Err(e) => println!("Circuit open or request failed: {e}"),
//! }
//! # Ok(())
//! # }
//! ```

pub mod circuit_breaker;
pub mod retry;
pub mod timeout;
```

---

## 4. Error Handling Rules

### 4.1 Error Handling Principles

```rust
// RULE 1: Never use unwrap() or panic!() in production code paths
// FORBIDDEN
let value = some_option.unwrap();
let result = risky_operation().unwrap();

// CORRECT: Use ? operator or explicit matching
let value = some_option.ok_or_else(|| anyhow::anyhow!("Value missing"))?;
let result = risky_operation()?;

// RULE 2: Use expect() ONLY in initialization or tests with descriptive messages
// ACCEPTABLE (initialization only)
let config = load_config().expect("Failed to load config.yaml - required for startup");

// CORRECT (tests)
#[cfg(test)]
fn test_something() {
    let value = parse("valid").expect("test data should be valid");
}

// RULE 3: Always propagate errors with context
// INSUFFICIENT
let data = read_file(path)?;

// CORRECT
let data = read_file(path)
    .with_context(|| format!("Failed to read provider config from {path}"))?;
```

### 4.2 Custom Error Types

```rust
use thiserror::Error;

/// Errors that can occur during provider operations.
#[derive(Error, Debug)]
pub enum ProviderError {
    /// The provider's API rate limit was exceeded.
    ///
    /// This typically resolves after waiting for the rate limit window to reset.
    /// Consider implementing exponential backoff or switching providers.
    #[error("Rate limit exceeded for provider {provider}: {message}")]
    RateLimitExceeded {
        provider: String,
        message: String,
        retry_after: Option<Duration>,
    },

    /// Authentication with the provider failed.
    ///
    /// Check that your API key is valid and has not expired.
    #[error("Authentication failed for {provider}: {message}")]
    AuthenticationFailed {
        provider: String,
        message: String,
    },

    /// The provider service is temporarily unavailable.
    ///
    /// This may be due to provider downtime or network issues.
    #[error("Service unavailable: {provider}")]
    ServiceUnavailable {
        provider: String,
        status_code: Option<u16>,
    },

    /// The request was invalid and rejected by the provider.
    #[error("Invalid request: {message}")]
    InvalidRequest {
        message: String,
        field: Option<String>,
    },

    /// Network communication error.
    #[error("Network error communicating with {provider}: {source}")]
    NetworkError {
        provider: String,
        #[source]
        source: reqwest::Error,
    },

    /// Response parsing error.
    #[error("Failed to parse response from {provider}: {source}")]
    ParseError {
        provider: String,
        #[source]
        source: serde_json::Error,
    },
}

// Implement conversions where appropriate
impl From<reqwest::Error> for ProviderError {
    fn from(err: reqwest::Error) -> Self {
        ProviderError::NetworkError {
            provider: "unknown".to_string(),
            source: err,
        }
    }
}
```

### 4.3 Result Type Aliases

```rust
// Define domain-specific Result types
pub type ProviderResult<T> = std::result::Result<T, ProviderError>;
pub type GatewayResult<T> = std::result::Result<T, GatewayError>;

// Use in signatures
pub async fn chat_completion(
    request: ChatRequest,
) -> ProviderResult<ChatResponse> {
    // Implementation
}
```

---

## 5. Testing Standards

### 5.1 Test Organization

```rust
// File structure:
// src/providers/openai.rs
// tests/integration/openai_tests.rs
// tests/common/mod.rs (shared test utilities)

// Unit tests: In same file
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_chat_request_valid_input() {
        let json = r#"{"model":"gpt-4","messages":[]}"#;
        let result = parse_chat_request(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_chat_request_missing_model() {
        let json = r#"{"messages":[]}"#;
        let result = parse_chat_request(json);
        assert!(matches!(result, Err(ParseError::MissingField("model"))));
    }

    #[tokio::test]
    async fn test_openai_chat_completion_success() {
        let provider = create_test_provider();
        let request = create_test_request();

        let response = provider.chat(request).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert!(!response.text.is_empty());
        assert!(response.tokens_used > 0);
    }
}
```

### 5.2 Test Naming Convention

```
test_<function>_<scenario>_<expected_outcome>
```

Examples:
- `test_circuit_breaker_open_state_fails_fast`
- `test_retry_with_exponential_backoff_succeeds_on_third_attempt`
- `test_rate_limiter_exceeds_quota_returns_error`
- `test_cache_hit_returns_cached_value`

### 5.3 Test Coverage Requirements

| Module Type | Coverage Target | Enforcement |
|-------------|----------------|-------------|
| Core models | 90%+ | CI warning at 80% |
| Provider implementations | 85%+ | CI warning at 75% |
| Middleware | 85%+ | CI warning at 75% |
| Utilities | 90%+ | CI warning at 80% |
| Error handling paths | 80%+ | Code review |

### 5.4 Property-Based Testing

```rust
// Use proptest for complex validation logic
#[cfg(test)]
mod proptests {
    use proptest::prelude::*;
    use super::*;

    proptest! {
        #[test]
        fn test_rate_limiter_never_exceeds_max(
            requests in 1..1000usize,
            window_secs in 1..60u64,
        ) {
            let limiter = RateLimiter::new(100, Duration::from_secs(window_secs));

            let mut allowed = 0;
            for _ in 0..requests {
                if limiter.try_acquire() {
                    allowed += 1;
                }
            }

            prop_assert!(allowed <= 100);
        }
    }
}
```

### 5.5 Integration Test Patterns

```rust
// tests/integration/openai_tests.rs
use llm_gateway::providers::openai::OpenAIProvider;
use std::sync::Arc;

#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored
async fn integration_test_openai_real_api() {
    let api_key = std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY must be set for integration tests");

    let provider = OpenAIProvider::new(api_key)
        .expect("Failed to create provider");

    let request = ChatRequest {
        model: "gpt-3.5-turbo".to_string(),
        messages: vec![Message::user("Say 'test'")],
        max_tokens: Some(10),
        ..Default::default()
    };

    let response = provider.chat(request).await
        .expect("API call should succeed");

    assert!(!response.text.is_empty());
}
```

---

## 6. Performance Guidelines

### 6.1 Async/Await Best Practices

```rust
// PREFER: Spawn tasks for independent operations
let (result1, result2) = tokio::join!(
    async { provider1.chat(request.clone()).await },
    async { provider2.chat(request.clone()).await },
);

// AVOID: Sequential awaits for independent work
let result1 = provider1.chat(request.clone()).await;
let result2 = provider2.chat(request.clone()).await; // Waits for first to complete
```

### 6.2 Allocation Minimization

```rust
// PREFER: Reuse allocations
let mut buffer = Vec::with_capacity(expected_size);

// PREFER: String building with known capacity
let mut result = String::with_capacity(estimated_length);

// AVOID: Repeated allocations in loops
for item in items {
    let s = format!("Item: {}", item); // New allocation each iteration
}
```

### 6.3 Clone Avoidance

```rust
// PREFER: Borrow when possible
fn process_request(request: &ChatRequest) -> Result<()> {
    // Read-only access
}

// PREFER: Arc for shared ownership
let provider = Arc::new(OpenAIProvider::new(api_key)?);
let provider_clone = Arc::clone(&provider); // Cheap pointer clone

// AVOID: Unnecessary clones
fn process_request(request: ChatRequest) -> Result<()> {
    // Takes ownership, forces caller to clone
}
```

---

## 7. Security Requirements

### 7.1 Secret Handling

```rust
// RULE: Never log secrets
// FORBIDDEN
tracing::info!("Using API key: {}", api_key);

// CORRECT: Redact secrets in logs
tracing::info!("Using API key: {}***", &api_key[..8]);

// RULE: Use secure types for secrets
use secrecy::{Secret, ExposeSecret};

pub struct ProviderConfig {
    pub base_url: String,
    pub api_key: Secret<String>, // Won't appear in Debug output
}

// Access only when needed
let key = config.api_key.expose_secret();
```

### 7.2 Input Validation

```rust
// RULE: Validate all external input
pub fn create_request(model: String, messages: Vec<Message>) -> Result<ChatRequest> {
    // Validate model name
    if model.is_empty() || model.len() > 100 {
        anyhow::bail!("Invalid model name length");
    }

    // Validate message count
    if messages.is_empty() {
        anyhow::bail!("At least one message required");
    }

    if messages.len() > 1000 {
        anyhow::bail!("Too many messages (max 1000)");
    }

    // Validate message content
    for msg in &messages {
        if msg.content.len() > 1_000_000 {
            anyhow::bail!("Message content too large (max 1MB)");
        }
    }

    Ok(ChatRequest { model, messages, ..Default::default() })
}
```

### 7.3 Dependency Auditing

```bash
# Run before every release
cargo audit

# Fail CI on high/critical vulnerabilities
cargo audit --deny warnings
```

---

## 8. Pre-commit Hooks

### 8.1 .pre-commit-config.yaml

```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: cargo-fmt
        name: cargo fmt
        entry: cargo fmt --all --
        language: system
        types: [rust]
        pass_filenames: false

      - id: cargo-clippy
        name: cargo clippy
        entry: cargo clippy --all-targets --all-features -- -D warnings
        language: system
        types: [rust]
        pass_filenames: false

      - id: cargo-test
        name: cargo test
        entry: cargo test --all-features
        language: system
        types: [rust]
        pass_filenames: false

      - id: cargo-audit
        name: cargo audit
        entry: cargo audit
        language: system
        pass_filenames: false

  - repo: https://github.com/pre-commit/pre-commit-hooks
    rev: v4.5.0
    hooks:
      - id: check-yaml
      - id: check-toml
      - id: check-merge-conflict
      - id: trailing-whitespace
      - id: end-of-file-fixer
```

### 8.2 Installation

```bash
# Install pre-commit
pip install pre-commit

# Install hooks
pre-commit install

# Run manually
pre-commit run --all-files
```

---

## 9. Code Review Checklist

### 9.1 Pull Request Review Checklist

#### Correctness
- [ ] No `unwrap()` or `panic!()` in production code paths
- [ ] All `expect()` calls have descriptive messages and valid justification
- [ ] All errors are properly handled with `?` or explicit matching
- [ ] Error context provided with `.context()` or `.with_context()`
- [ ] No `TODO` comments without linked GitHub issue

#### Testing
- [ ] Unit tests added for new functionality
- [ ] Integration tests added if external dependencies involved
- [ ] Edge cases covered (empty inputs, max limits, errors)
- [ ] Test names follow `test_<function>_<scenario>_<outcome>` convention
- [ ] Property-based tests for complex validation logic

#### Documentation
- [ ] Public items have rustdoc comments
- [ ] Module-level documentation updated if structure changed
- [ ] Error types documented with examples
- [ ] Complex algorithms have inline explanation comments
- [ ] README/guides updated for new features

#### Performance
- [ ] No unnecessary clones in hot paths
- [ ] Async operations run concurrently where possible
- [ ] Allocations minimized (use `with_capacity` where size known)
- [ ] Benchmarks run for performance-critical changes

#### Security
- [ ] No secrets in logs or error messages
- [ ] Input validation for all external data
- [ ] No SQL injection vectors (use parameterized queries)
- [ ] Dependencies audited (`cargo audit` passes)
- [ ] No unsafe code (or justified with SAFETY comment)

#### Code Quality
- [ ] `cargo fmt` passes
- [ ] `cargo clippy` passes with no warnings
- [ ] No commented-out code blocks
- [ ] Imports organized (std, external, internal)
- [ ] Visibility minimized (`pub(crate)` preferred over `pub`)

#### Architecture
- [ ] Follows existing module structure
- [ ] Dependencies added to appropriate Cargo.toml section
- [ ] Trait implementations follow project patterns
- [ ] Error types follow thiserror pattern

### 9.2 Review Comments Template

```markdown
## Performance Concern
**Location:** `src/providers/openai.rs:142`
**Issue:** Unnecessary clone in hot path
**Suggestion:**
\`\`\`rust
// Instead of:
let messages = request.messages.clone();

// Use:
let messages = &request.messages;
\`\`\`
**Priority:** High

## Security Issue
**Location:** `src/middleware/auth.rs:56`
**Issue:** API key logged in error message
**Suggestion:** Redact secret before logging
**Priority:** Critical

## Documentation Missing
**Location:** `src/core/models.rs:234`
**Issue:** Public function lacks documentation
**Suggestion:** Add rustdoc with examples
**Priority:** Medium
```

---

## 10. Continuous Integration

### 10.1 CI Pipeline (GitHub Actions)

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

env:
  RUST_BACKTRACE: 1
  CARGO_TERM_COLOR: always

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Run tests
        run: cargo test --all-features

      - name: Security audit
        run: |
          cargo install cargo-audit
          cargo audit

      - name: Check documentation
        run: cargo doc --all-features --no-deps
```

---

## Summary

These coding standards ensure:
- **Correctness**: Comprehensive error handling and testing
- **Performance**: Optimized async patterns and minimal allocations
- **Security**: Secret protection and input validation
- **Maintainability**: Clear documentation and consistent style
- **Quality**: Automated checks via clippy and pre-commit hooks

All contributors must follow these standards for code to be merged into the main branch.

---

**Questions?** Open an issue or contact the maintainers.
