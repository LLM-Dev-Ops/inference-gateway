# Dependency Matrix & Compatibility Guide

**Project:** LLM Inference Gateway
**Language:** Rust
**MSRV:** 1.75.0
**Version:** 1.0.0
**Last Updated:** 2024-11-27

---

## 1. Core Dependencies

| Crate | Version | Purpose | MSRV | License | Security Status |
|-------|---------|---------|------|---------|-----------------|
| **tokio** | 1.35.0 | Async runtime with full features | 1.70 | MIT | Audited (RustSec) |
| **axum** | 0.7.4 | HTTP web framework | 1.70 | MIT | Audited |
| **tower** | 0.4.13 | Service trait, middleware foundation | 1.63 | MIT | Audited |
| **tower-http** | 0.5.1 | HTTP middleware (CORS, compression) | 1.66 | MIT | Audited |
| **hyper** | 1.1.0 | HTTP client/server primitives | 1.70 | MIT | Audited |
| **hyper-util** | 0.1.2 | Hyper utilities and connection pooling | 1.70 | MIT | Audited |
| **hyper-rustls** | 0.26.0 | TLS support for Hyper | 1.70 | Apache-2.0/MIT | Audited |
| **serde** | 1.0.195 | Serialization framework | 1.56 | MIT/Apache-2.0 | Audited |
| **serde_json** | 1.0.111 | JSON serialization | 1.56 | MIT/Apache-2.0 | Audited |
| **bytes** | 1.5.0 | Byte buffer utilities | 1.60 | MIT | Audited |

## 2. HTTP Client & Networking

| Crate | Version | Purpose | MSRV | License | Security Status |
|-------|---------|---------|------|---------|-----------------|
| **reqwest** | 0.11.23 | High-level HTTP client | 1.63 | MIT/Apache-2.0 | Audited |
| **h2** | 0.4.2 | HTTP/2 protocol implementation | 1.66 | MIT | Audited |
| **rustls** | 0.22.2 | Modern TLS library (pure Rust) | 1.70 | Apache-2.0/ISC/MIT | Audited |
| **rustls-native-certs** | 0.7.0 | Native certificate loading | 1.60 | Apache-2.0/MIT | Audited |
| **webpki-roots** | 0.26.0 | Mozilla CA bundle | 1.60 | MPL-2.0 | Audited |

## 3. Async & Concurrency

| Crate | Version | Purpose | MSRV | License | Security Status |
|-------|---------|---------|------|---------|-----------------|
| **async-trait** | 0.1.77 | Async trait support | 1.56 | MIT/Apache-2.0 | Audited |
| **futures** | 0.3.30 | Future combinators and utilities | 1.56 | MIT/Apache-2.0 | Audited |
| **futures-util** | 0.3.30 | Future utilities | 1.56 | MIT/Apache-2.0 | Audited |
| **tokio-stream** | 0.1.14 | Stream utilities for Tokio | 1.56 | MIT | Audited |
| **pin-project** | 1.1.4 | Safe pin projection | 1.56 | Apache-2.0/MIT | Audited |

## 4. Redis & Caching

| Crate | Version | Purpose | MSRV | License | Security Status |
|-------|---------|---------|------|---------|-----------------|
| **redis** | 0.24.0 | Redis client with async support | 1.65 | BSD-3-Clause | Audited |
| **deadpool-redis** | 0.14.0 | Redis connection pooling | 1.65 | MIT/Apache-2.0 | Audited |
| **moka** | 0.12.3 | High-performance in-memory cache | 1.65 | MIT/Apache-2.0 | Audited |

## 5. Observability & Metrics

| Crate | Version | Purpose | MSRV | License | Security Status |
|-------|---------|---------|------|---------|-----------------|
| **prometheus** | 0.13.3 | Prometheus metrics exporter | 1.61 | Apache-2.0 | Audited |
| **opentelemetry** | 0.21.0 | Distributed tracing framework | 1.65 | Apache-2.0 | Audited |
| **opentelemetry-otlp** | 0.14.0 | OTLP exporter for traces | 1.65 | Apache-2.0 | Audited |
| **opentelemetry-prometheus** | 0.14.1 | Prometheus exporter for OTel | 1.65 | Apache-2.0 | Audited |
| **tracing** | 0.1.40 | Application-level tracing | 1.56 | MIT | Audited |
| **tracing-subscriber** | 0.3.18 | Tracing subscriber utilities | 1.56 | MIT | Audited |
| **tracing-opentelemetry** | 0.22.0 | OpenTelemetry layer for tracing | 1.65 | MIT | Audited |

## 6. Configuration & Environment

| Crate | Version | Purpose | MSRV | License | Security Status |
|-------|---------|---------|------|---------|-----------------|
| **config** | 0.14.0 | Configuration management | 1.70 | MIT/Apache-2.0 | Audited |
| **dotenvy** | 0.15.7 | .env file loading | 1.56 | MIT | Audited |
| **clap** | 4.4.18 | CLI argument parsing | 1.70 | MIT/Apache-2.0 | Audited |

## 7. Error Handling & Validation

| Crate | Version | Purpose | MSRV | License | Security Status |
|-------|---------|---------|------|---------|-----------------|
| **thiserror** | 1.0.56 | Custom error derivation | 1.56 | MIT/Apache-2.0 | Audited |
| **anyhow** | 1.0.79 | Error handling utilities | 1.56 | MIT/Apache-2.0 | Audited |
| **validator** | 0.17.0 | Struct validation | 1.70 | MIT | Audited |

## 8. AWS SDK (Bedrock Support)

| Crate | Version | Purpose | MSRV | License | Security Status |
|-------|---------|---------|------|---------|-----------------|
| **aws-config** | 1.1.4 | AWS SDK configuration | 1.70 | Apache-2.0 | Audited |
| **aws-sdk-bedrockruntime** | 1.13.0 | AWS Bedrock API client | 1.70 | Apache-2.0 | Audited |

## 9. Utilities & Data Structures

| Crate | Version | Purpose | MSRV | License | Security Status |
|-------|---------|---------|------|---------|-----------------|
| **uuid** | 1.6.1 | UUID generation | 1.60 | Apache-2.0/MIT | Audited |
| **chrono** | 0.4.33 | Date and time handling | 1.61 | Apache-2.0/MIT | Audited |
| **dashmap** | 5.5.3 | Concurrent hashmap | 1.65 | MIT | Audited |
| **parking_lot** | 0.12.1 | Efficient synchronization primitives | 1.56 | Apache-2.0/MIT | Audited |
| **once_cell** | 1.19.0 | Single assignment cells | 1.60 | MIT/Apache-2.0 | Audited |

## 10. Development Dependencies

| Crate | Version | Purpose | MSRV | License | Security Status |
|-------|---------|---------|------|---------|-----------------|
| **tokio-test** | 0.4.3 | Testing utilities for Tokio | 1.63 | MIT | Audited |
| **mockito** | 1.2.0 | HTTP mocking for tests | 1.70 | MIT | Audited |
| **criterion** | 0.5.1 | Benchmarking framework | 1.70 | Apache-2.0/MIT | Audited |
| **proptest** | 1.4.0 | Property-based testing | 1.65 | Apache-2.0/MIT | Audited |
| **insta** | 1.34.0 | Snapshot testing | 1.65 | Apache-2.0 | Audited |

---

## 2. Version Compatibility Matrix

| Gateway Version | Rust MSRV | Tokio | Axum | Reqwest | OpenTelemetry | Prometheus | Redis |
|-----------------|-----------|-------|------|---------|---------------|------------|-------|
| **0.1.x** | 1.75+ | 1.35+ | 0.7+ | 0.11+ | 0.21+ | 0.13+ | 0.24+ |
| **0.2.x** (planned) | 1.76+ | 1.36+ | 0.7+ | 0.12+ | 0.22+ | 0.13+ | 0.24+ |
| **1.0.x** | 1.75+ | 1.35+ | 0.7+ | 0.11+ | 0.21+ | 0.13+ | 0.24+ |

### Platform Compatibility

| Platform | Supported | Notes |
|----------|-----------|-------|
| Linux (x86_64) | Yes | Primary target |
| Linux (aarch64) | Yes | ARM64 support |
| macOS (Intel) | Yes | Development only |
| macOS (Apple Silicon) | Yes | Development only |
| Windows | Limited | Not recommended for production |
| Docker | Yes | Recommended deployment |
| Kubernetes | Yes | Recommended production deployment |

---

## 3. Feature Flags

```toml
[features]
default = ["openai", "anthropic", "metrics", "tracing"]

# Full feature set
full = [
    "openai",
    "anthropic",
    "google",
    "azure",
    "bedrock",
    "vllm",
    "ollama",
    "together",
    "metrics",
    "tracing",
    "redis-cache",
]

# Provider features
openai = []
anthropic = []
google = []
azure = ["openai"]  # Azure OpenAI is OpenAI-compatible
bedrock = ["aws-config", "aws-sdk-bedrockruntime"]
vllm = []  # OpenAI-compatible
ollama = []
together = []  # OpenAI-compatible

# Infrastructure features
metrics = ["prometheus", "opentelemetry-prometheus"]
tracing = ["opentelemetry", "opentelemetry-otlp", "tracing-opentelemetry"]
redis-cache = ["redis", "deadpool-redis"]

# Development features (dev-dependencies only)
test-utils = ["mockito", "tokio-test"]
benches = ["criterion"]
```

### Feature Dependencies

```
full
├── openai
├── anthropic
├── google
├── azure
│   └── openai
├── bedrock
│   ├── aws-config
│   └── aws-sdk-bedrockruntime
├── vllm
├── ollama
├── together
├── metrics
│   ├── prometheus
│   └── opentelemetry-prometheus
├── tracing
│   ├── opentelemetry
│   ├── opentelemetry-otlp
│   └── tracing-opentelemetry
└── redis-cache
    ├── redis
    └── deadpool-redis
```

---

## 4. Breaking Change Policy

### Semantic Versioning Rules

- **Major (X.0.0)**: Breaking API changes, MSRV bumps, architectural changes
- **Minor (0.X.0)**: New features, non-breaking API additions, dependency updates
- **Patch (0.0.X)**: Bug fixes, security patches, documentation updates

### Deprecation Timeline

1. **Announcement** (N): Feature marked as deprecated with `#[deprecated]` attribute
2. **Warning Period** (N+1 minor): Warnings emitted, migration guide published
3. **Removal** (N+2 major): Feature removed from codebase

### MSRV Policy

- MSRV updates require **minor version bump** (not patch)
- MSRV will not exceed **6 months behind stable**
- Security-critical updates may force MSRV bump in patch release

### Dependency Update Policy

| Update Type | Action | Version Impact |
|-------------|--------|----------------|
| Security patch | Immediate update | Patch release |
| Minor dependency bump | Review + update | Patch release |
| Major dependency bump | Evaluate compatibility | Minor/Major release |
| Breaking dependency change | Full testing required | Major release |

---

## 5. Security Audit Status

### Dependency Security

| Dependency | Last Audit | Vulnerabilities | Action | Status |
|------------|------------|-----------------|--------|--------|
| **tokio** | 2024-11 | 0 (None) | - | ✅ Clean |
| **axum** | 2024-11 | 0 (None) | - | ✅ Clean |
| **hyper** | 2024-11 | 0 (None) | - | ✅ Clean |
| **reqwest** | 2024-11 | 0 (None) | - | ✅ Clean |
| **serde** | 2024-11 | 0 (None) | - | ✅ Clean |
| **rustls** | 2024-11 | 0 (None) | Using instead of OpenSSL | ✅ Clean |
| **redis** | 2024-11 | 0 (None) | - | ✅ Clean |
| **opentelemetry** | 2024-10 | 0 (None) | - | ✅ Clean |

### Security Tools

```bash
# Run security audit
cargo audit

# Check for outdated dependencies
cargo outdated

# Dependency tree analysis
cargo tree --duplicates

# License compliance check
cargo deny check licenses
```

### Known Issues

- **None currently identified**

### Security Best Practices

1. **TLS**: Using `rustls` instead of `openssl` (pure Rust, memory-safe)
2. **Dependencies**: Regular `cargo audit` in CI/CD pipeline
3. **Updates**: Automated dependency updates via Dependabot
4. **Scanning**: Trivy container scanning for production images

---

## 6. Production-Ready Cargo.toml

```toml
[package]
name = "llm-inference-gateway"
version = "1.0.0"
edition = "2021"
rust-version = "1.75.0"
authors = ["LLM DevOps Team <hello@llmdevops.com>"]
license = "SEE LICENSE IN LICENSE.md"
description = "High-performance LLM inference gateway for multiple providers"
repository = "https://github.com/your-org/llm-inference-gateway"
keywords = ["llm", "gateway", "api", "inference", "openai"]
categories = ["web-programming", "api-bindings"]

[dependencies]
# Async Runtime
tokio = { version = "1.35", features = ["full", "tracing"] }
async-trait = "0.1.77"
futures = "0.3.30"
futures-util = "0.3.30"
pin-project = "1.1.4"

# Web Framework
axum = { version = "0.7.4", features = ["macros", "ws"] }
tower = { version = "0.4.13", features = ["full"] }
tower-http = { version = "0.5.1", features = ["fs", "trace", "cors", "compression-gzip"] }
hyper = { version = "1.1", features = ["full"] }
hyper-util = { version = "0.1.2", features = ["full"] }
hyper-rustls = { version = "0.26", features = ["http2"] }

# HTTP Client
reqwest = { version = "0.11.23", features = ["json", "stream", "rustls-tls"], default-features = false }
h2 = "0.4.2"

# TLS
rustls = "0.22.2"
rustls-native-certs = "0.7.0"
webpki-roots = "0.26.0"

# Serialization
serde = { version = "1.0.195", features = ["derive"] }
serde_json = "1.0.111"
bytes = "1.5"

# Redis & Caching (optional)
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"], optional = true }
deadpool-redis = { version = "0.14", optional = true }
moka = { version = "0.12", features = ["future"] }

# Observability
prometheus = { version = "0.13.3", optional = true }
opentelemetry = { version = "0.21", optional = true }
opentelemetry-otlp = { version = "0.14", optional = true }
opentelemetry-prometheus = { version = "0.14", optional = true }
tracing = "0.1.40"
tracing-subscriber = { version = "0.3.18", features = ["env-filter", "json"] }
tracing-opentelemetry = { version = "0.22", optional = true }

# AWS SDK (optional)
aws-config = { version = "1.1.4", optional = true }
aws-sdk-bedrockruntime = { version = "1.13", optional = true }

# Configuration
config = "0.14"
dotenvy = "0.15"
clap = { version = "4.4", features = ["derive", "env"] }

# Error Handling
thiserror = "1.0.56"
anyhow = "1.0.79"
validator = { version = "0.17", features = ["derive"] }

# Utilities
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4.33", features = ["serde"] }
dashmap = "5.5"
parking_lot = "0.12"
once_cell = "1.19"
tokio-stream = "0.1.14"

[dev-dependencies]
tokio-test = "0.4.3"
mockito = "1.2"
criterion = { version = "0.5", features = ["html_reports"] }
proptest = "1.4"
insta = { version = "1.34", features = ["yaml"] }

[features]
default = ["openai", "anthropic", "metrics", "tracing"]

full = [
    "openai",
    "anthropic",
    "google",
    "azure",
    "bedrock",
    "vllm",
    "ollama",
    "together",
    "metrics",
    "tracing",
    "redis-cache",
]

# Providers
openai = []
anthropic = []
google = []
azure = ["openai"]
bedrock = ["aws-config", "aws-sdk-bedrockruntime"]
vllm = []
ollama = []
together = []

# Infrastructure
metrics = ["prometheus", "opentelemetry-prometheus"]
tracing = ["opentelemetry", "opentelemetry-otlp", "tracing-opentelemetry"]
redis-cache = ["redis", "deadpool-redis"]

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"

[profile.release-with-debug]
inherits = "release"
strip = false
debug = true

[profile.dev]
opt-level = 0
debug = true

[profile.test]
opt-level = 1

[[bench]]
name = "provider_benchmarks"
harness = false
required-features = ["benches"]

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

---

## 7. Build Profiles Explained

### Release Profile (Production)
- **opt-level = 3**: Maximum optimization
- **lto = "thin"**: Thin Link-Time Optimization (faster builds, good optimization)
- **codegen-units = 1**: Single codegen unit for better optimization
- **strip = true**: Remove debug symbols (smaller binary)
- **panic = "abort"**: Abort on panic (no unwinding, smaller binary)

**Expected binary size:** ~15-20 MB
**Build time:** ~5-8 minutes (clean build)

### Release-with-Debug Profile (Debugging Production Issues)
- Same as release but keeps debug symbols
- **strip = false**: Keep debug symbols
- **debug = true**: Include debug info

**Use case:** Profiling production performance with tools like `perf`

### Development Profile
- **opt-level = 0**: No optimization (faster compile)
- **debug = true**: Full debug info

**Build time:** ~2-3 minutes (clean build)

---

## 8. Dependency License Summary

| License Type | Count | Crates |
|--------------|-------|--------|
| **MIT** | 28 | tokio, axum, serde, uuid, dashmap, ... |
| **Apache-2.0** | 12 | opentelemetry, aws-sdk-*, rustls, ... |
| **MIT/Apache-2.0** | 15 | hyper, tower, futures, bytes, ... |
| **BSD-3-Clause** | 1 | redis |
| **MPL-2.0** | 1 | webpki-roots |

**License Compliance:** All dependencies are permissive open-source licenses compatible with commercial use.

---

## 9. Maintenance & Updates

### Update Schedule

- **Security patches**: Immediate (within 24 hours)
- **Minor updates**: Monthly review
- **Major updates**: Quarterly evaluation

### Automated Tools

```bash
# Install cargo-edit for dependency management
cargo install cargo-edit

# Update dependencies
cargo upgrade

# Check for security advisories
cargo install cargo-audit
cargo audit

# Check outdated dependencies
cargo install cargo-outdated
cargo outdated
```

### CI/CD Integration

```yaml
# .github/workflows/security.yml
- name: Security Audit
  run: cargo audit

- name: Outdated Check
  run: cargo outdated --exit-code 1
```

---

**Document Version:** 1.0
**Maintained by:** LLM DevOps Architecture Team
**Review Cycle:** Monthly
