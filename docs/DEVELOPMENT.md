# LLM Inference Gateway - Development Guide

Guide for developing, testing, and contributing to the LLM Inference Gateway.

## Table of Contents

- [Development Environment Setup](#development-environment-setup)
- [Project Structure](#project-structure)
- [Building](#building)
- [Testing](#testing)
- [Code Style](#code-style)
- [Adding New Features](#adding-new-features)
- [Contributing](#contributing)

---

## Development Environment Setup

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 1.75+ | Primary language |
| Docker | 20.10+ | Container builds |
| Docker Compose | 2.0+ | Local development |
| Redis | 7.0+ | Caching (optional) |
| Git | 2.30+ | Version control |

### Installing Rust

```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Verify installation
rustc --version
cargo --version

# Install additional components
rustup component add clippy rustfmt
```

### IDE Setup

#### VS Code (Recommended)

Install extensions:
- **rust-analyzer**: Rust language support
- **CodeLLDB**: Debugger
- **crates**: Dependency management
- **Error Lens**: Inline error display

Settings (`.vscode/settings.json`):
```json
{
  "rust-analyzer.checkOnSave.command": "clippy",
  "rust-analyzer.cargo.features": "all",
  "editor.formatOnSave": true,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  }
}
```

#### IntelliJ IDEA / CLion

Install the **Rust** plugin from JetBrains Marketplace.

### Clone and Setup

```bash
# Clone the repository
git clone https://github.com/your-org/llm-inference-gateway.git
cd llm-inference-gateway

# Install pre-commit hooks (optional but recommended)
cargo install cargo-husky

# Create local environment file
cp .env.example .env
# Edit .env with your API keys

# Build the project
cargo build

# Run tests to verify setup
cargo test
```

### Local Development Services

```bash
# Start Redis for local development
docker-compose up -d redis

# Or use Docker for full local environment
docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d
```

---

## Project Structure

```
llm-inference-gateway/
├── Cargo.toml                    # Workspace manifest
├── Cargo.lock                    # Dependency lock file
├── README.md                     # Project overview
├── LICENSE                       # License file
│
├── crates/                       # Workspace crates
│   ├── gateway-core/             # Core types and traits
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            # Crate entry point
│   │       ├── request.rs        # Request types
│   │       ├── response.rs       # Response types
│   │       ├── error.rs          # Error types
│   │       └── provider.rs       # Provider trait
│   │
│   ├── gateway-api/              # HTTP API layer
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── router.rs         # Axum router setup
│   │       ├── handlers/         # Request handlers
│   │       └── middleware/       # Custom middleware
│   │
│   ├── gateway-providers/        # LLM provider implementations
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── openai.rs         # OpenAI provider
│   │       ├── anthropic.rs      # Anthropic provider
│   │       ├── google.rs         # Google provider
│   │       └── registry.rs       # Provider registry
│   │
│   ├── gateway-router/           # Request routing
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── strategy.rs       # Routing strategies
│   │       └── selector.rs       # Provider selection
│   │
│   ├── gateway-cache/            # Caching layer
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── memory.rs         # In-memory cache
│   │       ├── redis.rs          # Redis cache
│   │       └── key.rs            # Cache key generation
│   │
│   ├── gateway-rate-limit/       # Rate limiting
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── limiter.rs        # Rate limiter
│   │       └── backend.rs        # Storage backends
│   │
│   ├── gateway-auth/             # Authentication
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── api_key.rs        # API key auth
│   │       └── jwt.rs            # JWT auth
│   │
│   ├── gateway-telemetry/        # Observability
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── metrics.rs        # Prometheus metrics
│   │       ├── tracing.rs        # OpenTelemetry tracing
│   │       ├── pii.rs            # PII redaction
│   │       └── cost.rs           # Cost tracking
│   │
│   ├── gateway-config/           # Configuration
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── loader.rs         # Config loading
│   │
│   └── llm-gateway/              # Main binary
│       ├── Cargo.toml
│       └── src/
│           └── main.rs           # Entry point
│
├── tests/                        # Integration tests
│   └── integration/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── api_tests.rs
│           └── e2e_tests.rs
│
├── deploy/                       # Deployment configurations
│   ├── docker/
│   ├── kubernetes/
│   └── monitoring/
│
├── docs/                         # Documentation
│   ├── API.md
│   ├── ARCHITECTURE.md
│   ├── CONFIGURATION.md
│   ├── DEPLOYMENT.md
│   └── DEVELOPMENT.md
│
└── examples/                     # Usage examples
    ├── python/
    ├── nodejs/
    └── curl/
```

---

## Building

### Development Build

```bash
# Debug build (faster compilation, slower runtime)
cargo build

# Build specific crate
cargo build -p gateway-api

# Build with all features
cargo build --all-features
```

### Release Build

```bash
# Optimized release build
cargo build --release

# The binary is at:
./target/release/llm-gateway
```

### Cross-Compilation

```bash
# Install cross
cargo install cross

# Build for Linux
cross build --target x86_64-unknown-linux-gnu --release

# Build for ARM64
cross build --target aarch64-unknown-linux-gnu --release
```

### Docker Build

```bash
# Production image
docker build -t llm-gateway:latest .

# Development image (with dev tools)
docker build --target development -t llm-gateway:dev .

# Multi-arch build
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t llm-gateway:latest .
```

---

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p gateway-core

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run tests matching pattern
cargo test openai

# Run ignored tests (requires API keys)
cargo test -- --ignored
```

### Test Categories

#### Unit Tests

Located in `src/` files next to the code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        // Test code
    }
}
```

Run with:
```bash
cargo test --lib
```

#### Integration Tests

Located in `tests/integration/`:

```bash
# Run integration tests
cargo test -p integration-tests

# Run with live providers (requires API keys)
OPENAI_API_KEY=sk-xxx cargo test -p integration-tests -- --ignored
```

#### End-to-End Tests

```bash
# Start the gateway
docker-compose up -d

# Run E2E tests
cargo test -p integration-tests e2e -- --ignored
```

### Test Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run with coverage
cargo tarpaulin --out Html

# View report
open tarpaulin-report.html
```

### Benchmarks

```bash
# Run benchmarks
cargo bench

# Run specific benchmark
cargo bench cache

# Generate benchmark report
cargo bench -- --save-baseline main
```

---

## Code Style

### Formatting

```bash
# Format all code
cargo fmt

# Check formatting (CI)
cargo fmt -- --check
```

### Linting

```bash
# Run clippy
cargo clippy

# Run with all targets and features
cargo clippy --all-targets --all-features

# Treat warnings as errors (CI)
cargo clippy -- -D warnings
```

### Pre-commit Hooks

The project uses cargo-husky for pre-commit hooks:

```bash
# Hooks run automatically on commit:
# 1. cargo fmt --check
# 2. cargo clippy
# 3. cargo test
```

### Documentation

```bash
# Build documentation
cargo doc

# Build and open
cargo doc --open

# Include private items
cargo doc --document-private-items
```

### Commit Messages

Follow conventional commits:

```
type(scope): description

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting
- `refactor`: Code restructuring
- `perf`: Performance improvement
- `test`: Adding tests
- `chore`: Maintenance

Examples:
```
feat(providers): add Azure OpenAI support
fix(cache): handle Redis connection timeout
docs(api): update rate limit documentation
```

---

## Adding New Features

### Adding a New Provider

1. **Create the provider module** in `crates/gateway-providers/src/`:

```rust
// crates/gateway-providers/src/new_provider.rs

use async_trait::async_trait;
use gateway_core::{ChatRequest, ChatResponse, Provider, ProviderError};

pub struct NewProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl NewProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url: "https://api.newprovider.com/v1".to_string(),
        }
    }
}

#[async_trait]
impl Provider for NewProvider {
    fn name(&self) -> &str {
        "new_provider"
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo::new("model-1", "new_provider"),
            ModelInfo::new("model-2", "new_provider"),
        ]
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        // Transform request to provider format
        let provider_request = self.transform_request(request)?;

        // Make API call
        let response = self.client
            .post(&format!("{}/chat", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&provider_request)
            .send()
            .await?;

        // Transform response to gateway format
        self.transform_response(response).await
    }

    async fn chat_stream(&self, request: ChatRequest)
        -> Result<BoxStream<'static, Result<ChatChunk, ProviderError>>, ProviderError>
    {
        // Implement streaming
    }
}

impl NewProvider {
    fn transform_request(&self, request: ChatRequest) -> Result<ProviderRequest, ProviderError> {
        // Map gateway format to provider format
    }

    async fn transform_response(&self, response: reqwest::Response)
        -> Result<ChatResponse, ProviderError>
    {
        // Map provider format to gateway format
    }
}
```

2. **Export the provider** in `lib.rs`:

```rust
// crates/gateway-providers/src/lib.rs
mod new_provider;
pub use new_provider::NewProvider;
```

3. **Add configuration** in `gateway-config`:

```rust
// crates/gateway-config/src/lib.rs
pub struct NewProviderConfig {
    pub enabled: bool,
    pub api_key: String,
    pub base_url: Option<String>,
}
```

4. **Register in the provider registry**:

```rust
// crates/gateway-providers/src/registry.rs
if config.new_provider.enabled {
    registry.register(NewProvider::new(config.new_provider.api_key));
}
```

5. **Add tests**:

```rust
// crates/gateway-providers/src/new_provider.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_request() {
        // Test request transformation
    }

    #[tokio::test]
    async fn test_chat() {
        // Mock test for chat
    }

    #[tokio::test]
    #[ignore] // Requires API key
    async fn test_chat_live() {
        // Live test with real API
    }
}
```

6. **Update documentation**:
   - Add to `docs/API.md`
   - Add to `docs/CONFIGURATION.md`
   - Update `README.md`

### Adding a New Middleware

1. **Create middleware** in `crates/gateway-api/src/middleware/`:

```rust
// crates/gateway-api/src/middleware/custom.rs

use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};

pub async fn custom_middleware(
    request: Request,
    next: Next,
) -> Response {
    // Pre-processing
    let start = std::time::Instant::now();

    // Call next handler
    let response = next.run(request).await;

    // Post-processing
    let duration = start.elapsed();
    tracing::info!(duration_ms = %duration.as_millis(), "Request completed");

    response
}
```

2. **Add to router**:

```rust
// crates/gateway-api/src/router.rs
use crate::middleware::custom::custom_middleware;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .layer(axum::middleware::from_fn(custom_middleware))
        .with_state(state)
}
```

### Adding New Metrics

1. **Define metric** in `gateway-telemetry`:

```rust
// crates/gateway-telemetry/src/metrics.rs

use prometheus::{IntCounter, IntCounterVec, Histogram, HistogramVec, Opts, Registry};

lazy_static! {
    pub static ref CUSTOM_COUNTER: IntCounter = IntCounter::new(
        "llm_gateway_custom_total",
        "Description of custom metric"
    ).unwrap();

    pub static ref CUSTOM_HISTOGRAM: Histogram = Histogram::with_opts(
        HistogramOpts::new(
            "llm_gateway_custom_duration_seconds",
            "Custom operation duration"
        ).buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0])
    ).unwrap();
}

pub fn register_custom_metrics(registry: &Registry) {
    registry.register(Box::new(CUSTOM_COUNTER.clone())).unwrap();
    registry.register(Box::new(CUSTOM_HISTOGRAM.clone())).unwrap();
}
```

2. **Use in code**:

```rust
use gateway_telemetry::metrics::{CUSTOM_COUNTER, CUSTOM_HISTOGRAM};

CUSTOM_COUNTER.inc();

let timer = CUSTOM_HISTOGRAM.start_timer();
// ... operation ...
timer.observe_duration();
```

---

## Contributing

### Workflow

1. **Fork** the repository
2. **Create a branch**: `git checkout -b feature/my-feature`
3. **Make changes** and commit
4. **Run tests**: `cargo test`
5. **Run lints**: `cargo clippy`
6. **Push**: `git push origin feature/my-feature`
7. **Open a Pull Request**

### Pull Request Guidelines

- [ ] Tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation updated
- [ ] CHANGELOG updated (for user-facing changes)
- [ ] Commits are clean and descriptive

### Code Review

All PRs require review. Reviewers look for:

1. **Correctness**: Does the code work as intended?
2. **Tests**: Are there adequate tests?
3. **Performance**: Any performance implications?
4. **Security**: Any security concerns?
5. **Style**: Consistent with codebase?
6. **Documentation**: Is it documented?

### Release Process

1. Update version in `Cargo.toml` files
2. Update `CHANGELOG.md`
3. Create release PR
4. After merge, tag release: `git tag v0.1.0`
5. Push tag: `git push origin v0.1.0`
6. CI builds and publishes release

---

## Debugging

### Logging

```bash
# Enable debug logging
RUST_LOG=debug cargo run

# Enable trace logging for specific module
RUST_LOG=gateway_providers=trace cargo run

# Log to file
RUST_LOG=info cargo run 2>&1 | tee gateway.log
```

### Debugging with VS Code

`.vscode/launch.json`:
```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug Gateway",
      "program": "${workspaceFolder}/target/debug/llm-gateway",
      "args": ["--config", "gateway.yaml"],
      "cwd": "${workspaceFolder}",
      "env": {
        "RUST_LOG": "debug",
        "OPENAI_API_KEY": "${env:OPENAI_API_KEY}"
      }
    },
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug Tests",
      "cargo": {
        "args": ["test", "--no-run", "--lib"],
        "filter": {
          "kind": "lib"
        }
      },
      "cwd": "${workspaceFolder}"
    }
  ]
}
```

### Profiling

```bash
# Install flamegraph
cargo install flamegraph

# Profile the gateway
cargo flamegraph --bin llm-gateway

# CPU profiling with perf
perf record --call-graph dwarf ./target/release/llm-gateway
perf report

# Memory profiling with heaptrack
heaptrack ./target/release/llm-gateway
heaptrack_gui heaptrack.llm-gateway.*.gz
```

---

## Troubleshooting Development Issues

### Compilation Errors

```bash
# Clean build artifacts
cargo clean

# Update dependencies
cargo update

# Check for outdated dependencies
cargo outdated
```

### Slow Compilation

```bash
# Use sccache for caching
cargo install sccache
export RUSTC_WRAPPER=sccache

# Check what's taking time
cargo build --timings

# Use mold linker (Linux)
sudo apt install mold
RUSTFLAGS="-C link-arg=-fuse-ld=mold" cargo build
```

### Test Failures

```bash
# Run single test with output
cargo test test_name -- --nocapture

# Run tests sequentially (for isolation)
cargo test -- --test-threads=1

# Run with backtrace
RUST_BACKTRACE=1 cargo test
```

---

## Resources

### Documentation

- [Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Axum Documentation](https://docs.rs/axum/latest/axum/)
- [OpenTelemetry Rust](https://github.com/open-telemetry/opentelemetry-rust)

### Tools

- [cargo-watch](https://crates.io/crates/cargo-watch): Auto-rebuild on changes
- [cargo-expand](https://crates.io/crates/cargo-expand): Expand macros
- [cargo-deny](https://crates.io/crates/cargo-deny): License and security checks
- [cargo-audit](https://crates.io/crates/cargo-audit): Security vulnerabilities

### Community

- GitHub Issues: Bug reports and feature requests
- Discussions: Questions and ideas
- Discord: Real-time chat (if available)
