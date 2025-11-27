# LLM Inference Gateway - SPARC Completion Document

**Version:** 1.0.0
**Phase:** SPARC Phase 5 - Completion
**Last Updated:** 2025-11-27
**Status:** Implementation Ready

---

## Executive Summary

This document represents the Completion phase of the SPARC methodology for the LLM Inference Gateway project. It provides a comprehensive implementation blueprint that transforms the specification, pseudocode, architecture, and refinement documents into actionable development guidance.

### Completion Phase Objectives

| Objective | Description | Success Criteria |
|-----------|-------------|------------------|
| **Actionable Roadmap** | Step-by-step implementation guide | Clear task sequencing |
| **Zero Ambiguity** | Complete file/function specifications | No implementation guesswork |
| **Quality Assurance** | Integrated testing at every step | 85%+ coverage maintained |
| **Production Path** | Clear route from code to deployment | Automated CI/CD pipeline |

### Quality Commitment

```
Target: Enterprise-grade, commercially viable, production-ready
        Bug-free implementation with ZERO compilation errors

Enforcement:
  - Every module compiles independently before integration
  - Every function has unit tests before PR merge
  - Every integration point has contract tests
  - CI/CD gates block non-compliant code
```

---

## Table of Contents

1. [Implementation Roadmap](#1-implementation-roadmap)
2. [Project Scaffolding](#2-project-scaffolding)
3. [Module Implementation Guide](#3-module-implementation-guide)
4. [Implementation Order & Dependencies](#4-implementation-order--dependencies)
5. [Code Templates](#5-code-templates)
6. [Testing Strategy](#6-testing-strategy)
7. [CI/CD Pipeline](#7-cicd-pipeline)
8. [Quality Gates](#8-quality-gates)
9. [Deployment Guide](#9-deployment-guide)
10. [Post-Implementation Checklist](#10-post-implementation-checklist)

---

## 1. Implementation Roadmap

### 1.1 Phase Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     IMPLEMENTATION PHASES                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  PHASE 1: FOUNDATION                                                         │
│  ══════════════════                                                          │
│  • Project scaffolding & Cargo workspace                                    │
│  • Core types (GatewayRequest, GatewayResponse, GatewayError)              │
│  • Configuration schema & loader                                            │
│  • Basic HTTP server skeleton                                               │
│  Deliverable: Compiling project with health endpoint                        │
│                                                                              │
│  PHASE 2: PROVIDER LAYER                                                     │
│  ═════════════════════                                                       │
│  • Provider trait definition                                                │
│  • Provider registry                                                        │
│  • OpenAI provider implementation                                           │
│  • Request/response transformation                                          │
│  Deliverable: Working OpenAI proxy                                          │
│                                                                              │
│  PHASE 3: RESILIENCE                                                         │
│  ═════════════════                                                           │
│  • Circuit breaker implementation                                           │
│  • Retry policy with exponential backoff                                    │
│  • Timeout management                                                        │
│  • Bulkhead pattern                                                          │
│  Deliverable: Fault-tolerant provider calls                                 │
│                                                                              │
│  PHASE 4: ROUTING                                                            │
│  ═══════════════                                                             │
│  • Router implementation                                                     │
│  • Load balancing strategies                                                │
│  • Health-aware routing                                                      │
│  • Rules engine                                                              │
│  Deliverable: Multi-provider routing                                        │
│                                                                              │
│  PHASE 5: MIDDLEWARE                                                         │
│  ═════════════════                                                           │
│  • Middleware trait & pipeline                                              │
│  • Authentication middleware                                                │
│  • Rate limiting middleware                                                 │
│  • Logging & tracing middleware                                             │
│  Deliverable: Complete request processing pipeline                          │
│                                                                              │
│  PHASE 6: OBSERVABILITY                                                      │
│  ═══════════════════                                                         │
│  • Prometheus metrics                                                        │
│  • OpenTelemetry tracing                                                    │
│  • Structured logging                                                        │
│  • Audit logging                                                             │
│  Deliverable: Full observability stack                                      │
│                                                                              │
│  PHASE 7: ADDITIONAL PROVIDERS                                               │
│  ═════════════════════════════                                               │
│  • Anthropic provider                                                        │
│  • Google AI provider                                                        │
│  • vLLM provider                                                             │
│  • Ollama provider                                                           │
│  • Azure OpenAI provider                                                     │
│  • AWS Bedrock provider                                                      │
│  Deliverable: 8-provider support                                            │
│                                                                              │
│  PHASE 8: PRODUCTION HARDENING                                               │
│  ══════════════════════════════                                              │
│  • Performance optimization                                                  │
│  • Security audit                                                            │
│  • Load testing                                                              │
│  • Documentation                                                             │
│  Deliverable: Production-ready release                                      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Milestone Definitions

| Milestone | Description | Acceptance Criteria | Dependencies |
|-----------|-------------|---------------------|--------------|
| **M1: Scaffold** | Project compiles | `cargo build` succeeds | None |
| **M2: Health** | Health endpoint works | `/health/live` returns 200 | M1 |
| **M3: OpenAI** | OpenAI proxy works | End-to-end request succeeds | M2 |
| **M4: Resilient** | Circuit breaker active | Failure triggers open state | M3 |
| **M5: Routed** | Multi-provider routing | Load balancer distributes | M4 |
| **M6: Secured** | Auth & rate limit | Unauthorized returns 401 | M5 |
| **M7: Observable** | Metrics & traces | Prometheus scrape works | M6 |
| **M8: Multi-Provider** | All 8 providers | Each provider tested | M7 |
| **M9: Optimized** | Performance targets met | P95 <5ms, 10K RPS | M8 |
| **M10: Released** | Production deployment | Canary successful | M9 |

### 1.3 Task Breakdown

#### Phase 1: Foundation (Tasks 1-15)

| # | Task | File(s) | Est. LoC | Test Coverage |
|---|------|---------|----------|---------------|
| 1 | Create Cargo workspace | `Cargo.toml` | 50 | N/A |
| 2 | Create gateway-core crate | `crates/gateway-core/` | 100 | N/A |
| 3 | Implement GatewayRequest | `core/src/request.rs` | 150 | 95% |
| 4 | Implement GatewayResponse | `core/src/response.rs` | 120 | 95% |
| 5 | Implement GatewayError | `core/src/error.rs` | 200 | 90% |
| 6 | Implement validated types | `core/src/types/` | 300 | 95% |
| 7 | Create gateway-config crate | `crates/gateway-config/` | 100 | N/A |
| 8 | Implement config schema | `config/src/schema.rs` | 250 | 90% |
| 9 | Implement config loader | `config/src/loader.rs` | 150 | 85% |
| 10 | Implement config validator | `config/src/validator.rs` | 100 | 90% |
| 11 | Create gateway-server crate | `crates/gateway-server/` | 100 | N/A |
| 12 | Implement Axum server | `server/src/server.rs` | 150 | 80% |
| 13 | Implement health handlers | `server/src/handlers/health.rs` | 80 | 90% |
| 14 | Create main.rs entry | `src/main.rs` | 100 | 80% |
| 15 | Integration test: health | `tests/integration/health.rs` | 50 | N/A |

#### Phase 2: Provider Layer (Tasks 16-28)

| # | Task | File(s) | Est. LoC | Test Coverage |
|---|------|---------|----------|---------------|
| 16 | Create gateway-providers crate | `crates/gateway-providers/` | 100 | N/A |
| 17 | Define LLMProvider trait | `providers/src/traits.rs` | 100 | N/A |
| 18 | Implement ProviderRegistry | `providers/src/registry.rs` | 200 | 90% |
| 19 | Implement provider factory | `providers/src/factory.rs` | 150 | 85% |
| 20 | OpenAI: request transform | `providers/src/openai/transform.rs` | 200 | 90% |
| 21 | OpenAI: response transform | `providers/src/openai/response.rs` | 150 | 90% |
| 22 | OpenAI: client implementation | `providers/src/openai/client.rs` | 250 | 85% |
| 23 | OpenAI: streaming support | `providers/src/openai/stream.rs` | 200 | 85% |
| 24 | OpenAI: health check | `providers/src/openai/health.rs` | 80 | 90% |
| 25 | Implement chat handler | `server/src/handlers/chat.rs` | 200 | 85% |
| 26 | Implement models handler | `server/src/handlers/models.rs` | 100 | 90% |
| 27 | Wire up provider to server | `server/src/router.rs` | 100 | 80% |
| 28 | Integration test: OpenAI | `tests/integration/openai.rs` | 150 | N/A |

#### Phase 3: Resilience (Tasks 29-40)

| # | Task | File(s) | Est. LoC | Test Coverage |
|---|------|---------|----------|---------------|
| 29 | Create gateway-resilience crate | `crates/gateway-resilience/` | 100 | N/A |
| 30 | Implement CircuitBreaker | `resilience/src/circuit_breaker.rs` | 300 | 95% |
| 31 | Implement CircuitBreakerMetrics | `resilience/src/cb_metrics.rs` | 100 | 90% |
| 32 | Implement RetryPolicy | `resilience/src/retry.rs` | 200 | 90% |
| 33 | Implement exponential backoff | `resilience/src/backoff.rs` | 100 | 95% |
| 34 | Implement Bulkhead | `resilience/src/bulkhead.rs` | 150 | 90% |
| 35 | Implement TimeoutManager | `resilience/src/timeout.rs` | 150 | 90% |
| 36 | Implement ResilienceCoordinator | `resilience/src/coordinator.rs` | 200 | 85% |
| 37 | Integrate with provider calls | `providers/src/resilient.rs` | 150 | 85% |
| 38 | Unit tests: circuit breaker | `resilience/src/tests/` | 300 | N/A |
| 39 | Unit tests: retry policy | `resilience/src/tests/` | 200 | N/A |
| 40 | Integration test: failover | `tests/integration/failover.rs` | 150 | N/A |

#### Phase 4: Routing (Tasks 41-52)

| # | Task | File(s) | Est. LoC | Test Coverage |
|---|------|---------|----------|---------------|
| 41 | Create gateway-routing crate | `crates/gateway-routing/` | 100 | N/A |
| 42 | Implement Router core | `routing/src/router.rs` | 250 | 90% |
| 43 | Implement RulesEngine | `routing/src/rules.rs` | 200 | 90% |
| 44 | Implement RuleMatcher | `routing/src/matcher.rs` | 150 | 95% |
| 45 | Implement LoadBalancer trait | `routing/src/balancer/mod.rs` | 80 | N/A |
| 46 | Implement RoundRobinBalancer | `routing/src/balancer/round_robin.rs` | 100 | 95% |
| 47 | Implement LeastLatencyBalancer | `routing/src/balancer/least_latency.rs` | 150 | 90% |
| 48 | Implement CostOptimizedBalancer | `routing/src/balancer/cost.rs` | 150 | 90% |
| 49 | Implement HealthAwareRouter | `routing/src/health.rs` | 150 | 90% |
| 50 | Integrate router with server | `server/src/router.rs` | 100 | 80% |
| 51 | Unit tests: routing | `routing/src/tests/` | 250 | N/A |
| 52 | Integration test: load balancing | `tests/integration/routing.rs` | 150 | N/A |

#### Phase 5: Middleware (Tasks 53-67)

| # | Task | File(s) | Est. LoC | Test Coverage |
|---|------|---------|----------|---------------|
| 53 | Implement Middleware trait | `server/src/middleware/mod.rs` | 100 | N/A |
| 54 | Implement MiddlewareStack | `server/src/middleware/stack.rs` | 150 | 90% |
| 55 | Implement RequestIdMiddleware | `server/src/middleware/request_id.rs` | 80 | 90% |
| 56 | Implement AuthMiddleware | `server/src/middleware/auth.rs` | 200 | 90% |
| 57 | Implement ApiKeyValidator | `server/src/middleware/auth/api_key.rs` | 150 | 95% |
| 58 | Implement JwtValidator | `server/src/middleware/auth/jwt.rs` | 200 | 90% |
| 59 | Implement RateLimitMiddleware | `server/src/middleware/rate_limit.rs` | 200 | 90% |
| 60 | Implement TokenBucket | `server/src/middleware/rate_limit/bucket.rs` | 150 | 95% |
| 61 | Implement ValidationMiddleware | `server/src/middleware/validation.rs` | 150 | 90% |
| 62 | Implement LoggingMiddleware | `server/src/middleware/logging.rs` | 150 | 85% |
| 63 | Implement TracingMiddleware | `server/src/middleware/tracing.rs` | 150 | 85% |
| 64 | Implement CacheMiddleware | `server/src/middleware/cache.rs` | 200 | 85% |
| 65 | Wire up middleware pipeline | `server/src/server.rs` | 100 | 80% |
| 66 | Unit tests: middleware | `server/src/middleware/tests/` | 300 | N/A |
| 67 | Integration test: auth | `tests/integration/auth.rs` | 150 | N/A |

#### Phase 6: Observability (Tasks 68-80)

| # | Task | File(s) | Est. LoC | Test Coverage |
|---|------|---------|----------|---------------|
| 68 | Create gateway-telemetry crate | `crates/gateway-telemetry/` | 100 | N/A |
| 69 | Implement MetricsRegistry | `telemetry/src/metrics.rs` | 250 | 85% |
| 70 | Implement request metrics | `telemetry/src/metrics/request.rs` | 150 | 90% |
| 71 | Implement provider metrics | `telemetry/src/metrics/provider.rs` | 150 | 90% |
| 72 | Implement Prometheus exporter | `telemetry/src/prometheus.rs` | 150 | 85% |
| 73 | Implement TracingSystem | `telemetry/src/tracing.rs` | 200 | 85% |
| 74 | Implement span management | `telemetry/src/tracing/span.rs` | 150 | 85% |
| 75 | Implement context propagation | `telemetry/src/tracing/context.rs` | 100 | 90% |
| 76 | Implement StructuredLogger | `telemetry/src/logging.rs` | 150 | 85% |
| 77 | Implement AuditLogger | `telemetry/src/audit.rs` | 150 | 90% |
| 78 | Implement metrics handler | `server/src/handlers/metrics.rs` | 80 | 90% |
| 79 | Unit tests: telemetry | `telemetry/src/tests/` | 200 | N/A |
| 80 | Integration test: metrics | `tests/integration/metrics.rs` | 100 | N/A |

#### Phase 7: Additional Providers (Tasks 81-98)

| # | Task | File(s) | Est. LoC | Test Coverage |
|---|------|---------|----------|---------------|
| 81 | Anthropic: transform | `providers/src/anthropic/transform.rs` | 200 | 90% |
| 82 | Anthropic: client | `providers/src/anthropic/client.rs` | 250 | 85% |
| 83 | Anthropic: streaming | `providers/src/anthropic/stream.rs` | 200 | 85% |
| 84 | Google AI: transform | `providers/src/google/transform.rs` | 200 | 90% |
| 85 | Google AI: client | `providers/src/google/client.rs` | 250 | 85% |
| 86 | Google AI: streaming | `providers/src/google/stream.rs` | 200 | 85% |
| 87 | vLLM: transform | `providers/src/vllm/transform.rs` | 150 | 90% |
| 88 | vLLM: client | `providers/src/vllm/client.rs` | 200 | 85% |
| 89 | Ollama: transform | `providers/src/ollama/transform.rs` | 150 | 90% |
| 90 | Ollama: client | `providers/src/ollama/client.rs` | 200 | 85% |
| 91 | Azure OpenAI: transform | `providers/src/azure/transform.rs` | 200 | 90% |
| 92 | Azure OpenAI: client | `providers/src/azure/client.rs` | 250 | 85% |
| 93 | AWS Bedrock: transform | `providers/src/bedrock/transform.rs` | 250 | 90% |
| 94 | AWS Bedrock: client | `providers/src/bedrock/client.rs` | 300 | 85% |
| 95 | Together AI: client | `providers/src/together/client.rs` | 200 | 85% |
| 96 | Integration test: Anthropic | `tests/integration/anthropic.rs` | 150 | N/A |
| 97 | Integration test: Google | `tests/integration/google.rs` | 150 | N/A |
| 98 | Integration test: vLLM | `tests/integration/vllm.rs` | 150 | N/A |

#### Phase 8: Production Hardening (Tasks 99-115)

| # | Task | File(s) | Est. LoC | Test Coverage |
|---|------|---------|----------|---------------|
| 99 | Hot reload implementation | `config/src/hot_reload.rs` | 200 | 85% |
| 100 | Connection pool tuning | `providers/src/pool.rs` | 150 | 85% |
| 101 | JSON optimization (simd-json) | `core/src/json.rs` | 100 | 90% |
| 102 | Buffer pooling | `core/src/buffers.rs` | 150 | 90% |
| 103 | Graceful shutdown | `server/src/shutdown.rs` | 150 | 85% |
| 104 | Benchmark suite | `benches/` | 300 | N/A |
| 105 | Load test harness | `tests/load/` | 200 | N/A |
| 106 | Security hardening | Multiple | 200 | 90% |
| 107 | PII redaction | `telemetry/src/redaction.rs` | 150 | 95% |
| 108 | Dockerfile | `Dockerfile` | 50 | N/A |
| 109 | Docker Compose | `docker-compose.yml` | 100 | N/A |
| 110 | Kubernetes manifests | `k8s/` | 300 | N/A |
| 111 | Helm chart | `helm/` | 400 | N/A |
| 112 | API documentation | `docs/api/` | 500 | N/A |
| 113 | Operations runbook | `docs/ops/` | 300 | N/A |
| 114 | CI/CD pipeline | `.github/workflows/` | 200 | N/A |
| 115 | Release automation | `.github/workflows/release.yml` | 100 | N/A |

---

## 2. Project Scaffolding

### 2.1 Directory Structure

```
llm-inference-gateway/
├── Cargo.toml                          # Workspace root
├── Cargo.lock                          # Locked dependencies
├── rust-toolchain.toml                 # Rust version pinning
├── .cargo/
│   └── config.toml                     # Cargo configuration
├── .github/
│   └── workflows/
│       ├── ci.yml                      # CI pipeline
│       ├── release.yml                 # Release automation
│       └── security.yml                # Security scanning
├── crates/
│   ├── gateway-core/                   # Core types & traits
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── request.rs              # GatewayRequest
│   │       ├── response.rs             # GatewayResponse
│   │       ├── error.rs                # GatewayError
│   │       ├── types/
│   │       │   ├── mod.rs
│   │       │   ├── validated.rs        # Newtype wrappers
│   │       │   ├── message.rs          # ChatMessage, Role
│   │       │   ├── tool.rs             # Tool definitions
│   │       │   └── usage.rs            # Token usage
│   │       └── provider.rs             # Provider trait
│   │
│   ├── gateway-config/                 # Configuration
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── schema.rs               # Config types
│   │       ├── loader.rs               # Multi-source loading
│   │       ├── validator.rs            # Validation logic
│   │       ├── hot_reload.rs           # Hot reload manager
│   │       └── secrets.rs              # Secret resolution
│   │
│   ├── gateway-providers/              # Provider implementations
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs               # LLMProvider trait
│   │       ├── registry.rs             # Provider registry
│   │       ├── factory.rs              # Provider factory
│   │       ├── resilient.rs            # Resilient wrapper
│   │       ├── openai/
│   │       │   ├── mod.rs
│   │       │   ├── client.rs
│   │       │   ├── transform.rs
│   │       │   ├── stream.rs
│   │       │   └── health.rs
│   │       ├── anthropic/
│   │       │   ├── mod.rs
│   │       │   ├── client.rs
│   │       │   ├── transform.rs
│   │       │   └── stream.rs
│   │       ├── google/
│   │       │   └── ... (similar structure)
│   │       ├── vllm/
│   │       │   └── ...
│   │       ├── ollama/
│   │       │   └── ...
│   │       ├── azure/
│   │       │   └── ...
│   │       ├── bedrock/
│   │       │   └── ...
│   │       └── together/
│   │           └── ...
│   │
│   ├── gateway-routing/                # Routing & load balancing
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── router.rs               # Main router
│   │       ├── rules.rs                # Rules engine
│   │       ├── matcher.rs              # Rule matching
│   │       ├── health.rs               # Health-aware routing
│   │       └── balancer/
│   │           ├── mod.rs
│   │           ├── round_robin.rs
│   │           ├── least_latency.rs
│   │           ├── cost.rs
│   │           └── weighted.rs
│   │
│   ├── gateway-resilience/             # Fault tolerance
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── circuit_breaker.rs
│   │       ├── retry.rs
│   │       ├── backoff.rs
│   │       ├── bulkhead.rs
│   │       ├── timeout.rs
│   │       └── coordinator.rs
│   │
│   ├── gateway-telemetry/              # Observability
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── metrics.rs
│   │       ├── prometheus.rs
│   │       ├── tracing.rs
│   │       ├── logging.rs
│   │       ├── audit.rs
│   │       └── redaction.rs
│   │
│   └── gateway-server/                 # HTTP server
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── server.rs               # Server setup
│           ├── router.rs               # Axum router
│           ├── shutdown.rs             # Graceful shutdown
│           ├── handlers/
│           │   ├── mod.rs
│           │   ├── chat.rs             # /v1/chat/completions
│           │   ├── completions.rs      # /v1/completions
│           │   ├── embeddings.rs       # /v1/embeddings
│           │   ├── models.rs           # /v1/models
│           │   ├── health.rs           # Health endpoints
│           │   └── metrics.rs          # Prometheus metrics
│           ├── middleware/
│           │   ├── mod.rs
│           │   ├── stack.rs
│           │   ├── request_id.rs
│           │   ├── auth.rs
│           │   ├── rate_limit.rs
│           │   ├── validation.rs
│           │   ├── logging.rs
│           │   ├── tracing.rs
│           │   └── cache.rs
│           └── error.rs                # API error types
│
├── src/
│   └── main.rs                         # Binary entry point
│
├── config/
│   ├── default.yaml                    # Default config
│   ├── development.yaml                # Dev overrides
│   ├── staging.yaml                    # Staging config
│   └── production.yaml                 # Production config
│
├── tests/
│   ├── common/
│   │   └── mod.rs                      # Test utilities
│   ├── integration/
│   │   ├── health.rs
│   │   ├── openai.rs
│   │   ├── anthropic.rs
│   │   ├── routing.rs
│   │   ├── auth.rs
│   │   ├── rate_limit.rs
│   │   └── metrics.rs
│   └── e2e/
│       ├── full_flow.rs
│       └── streaming.rs
│
├── benches/
│   ├── routing.rs
│   ├── serialization.rs
│   └── middleware.rs
│
├── docs/
│   ├── api/
│   │   └── openapi.yaml
│   ├── ops/
│   │   └── runbook.md
│   └── architecture/
│       └── decisions/
│
├── k8s/
│   ├── namespace.yaml
│   ├── deployment.yaml
│   ├── service.yaml
│   ├── configmap.yaml
│   ├── secret.yaml
│   ├── hpa.yaml
│   └── pdb.yaml
│
├── helm/
│   └── llm-gateway/
│       ├── Chart.yaml
│       ├── values.yaml
│       └── templates/
│
├── Dockerfile
├── docker-compose.yml
├── .dockerignore
├── .gitignore
├── README.md
└── LICENSE
```

### 2.2 Workspace Cargo.toml

```toml
[workspace]
resolver = "2"
members = [
    "crates/gateway-core",
    "crates/gateway-config",
    "crates/gateway-providers",
    "crates/gateway-routing",
    "crates/gateway-resilience",
    "crates/gateway-telemetry",
    "crates/gateway-server",
]

[workspace.package]
version = "1.0.0"
edition = "2021"
rust-version = "1.75.0"
license = "Apache-2.0"
repository = "https://github.com/llm-devops/llm-inference-gateway"
authors = ["LLM DevOps Team"]

[workspace.dependencies]
# Async runtime
tokio = { version = "1.35", features = ["full"] }
async-trait = "0.1"
futures = "0.3"
async-stream = "0.3"

# HTTP
axum = { version = "0.7", features = ["macros", "ws"] }
axum-extra = { version = "0.9", features = ["typed-header"] }
tower = { version = "0.4", features = ["full"] }
tower-http = { version = "0.5", features = ["full"] }
hyper = { version = "1.1", features = ["full"] }
reqwest = { version = "0.11", default-features = false, features = ["json", "stream", "rustls-tls"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
toml = "0.8"

# Validation
validator = { version = "0.16", features = ["derive"] }

# Observability
opentelemetry = { version = "0.21", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.14", features = ["tonic"] }
opentelemetry_sdk = { version = "0.21", features = ["rt-tokio"] }
prometheus = "0.13"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-opentelemetry = "0.22"

# Concurrency
dashmap = "5.5"
arc-swap = "1.6"
parking_lot = "0.12"

# Utilities
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
bytes = "1.5"
thiserror = "1.0"
anyhow = "1.0"
rand = "0.8"
notify = "6.1"
secrecy = "0.8"

# Internal crates
gateway-core = { path = "crates/gateway-core" }
gateway-config = { path = "crates/gateway-config" }
gateway-providers = { path = "crates/gateway-providers" }
gateway-routing = { path = "crates/gateway-routing" }
gateway-resilience = { path = "crates/gateway-resilience" }
gateway-telemetry = { path = "crates/gateway-telemetry" }
gateway-server = { path = "crates/gateway-server" }

[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
nursery = "warn"
unwrap_used = "deny"
expect_used = "warn"
panic = "deny"
```

### 2.3 Rust Toolchain

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.75.0"
components = ["rustfmt", "clippy", "rust-src"]
targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]
```

### 2.4 Cargo Config

```toml
# .cargo/config.toml
[build]
rustflags = ["-C", "target-cpu=native"]

[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[alias]
xtask = "run --package xtask --"

[env]
CARGO_INCREMENTAL = "1"
```

---

## 3. Module Implementation Guide

### 3.1 gateway-core Module

#### 3.1.1 GatewayRequest Implementation

```rust
// crates/gateway-core/src/request.rs

use crate::types::{
    MaxTokens, ModelId, RequestId, Temperature, TopP,
    ChatMessage, ToolDefinition, RequestMetadata,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unified gateway request that abstracts all provider formats.
///
/// This is the canonical request type used throughout the gateway.
/// All incoming requests are normalized to this format before processing.
///
/// # Example
///
/// ```rust
/// use gateway_core::GatewayRequest;
///
/// let request = GatewayRequest::builder()
///     .model("gpt-4")
///     .messages(vec![ChatMessage::user("Hello")])
///     .temperature(0.7)
///     .build()?;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayRequest {
    /// Unique request identifier (UUID v7 for time-ordering)
    #[serde(default = "generate_request_id")]
    pub id: RequestId,

    /// Target model identifier
    pub model: ModelId,

    /// Chat messages for conversation
    #[serde(default)]
    pub messages: Vec<ChatMessage>,

    /// Sampling temperature (0.0 - 2.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<Temperature>,

    /// Maximum tokens to generate
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<MaxTokens>,

    /// Top-p (nucleus) sampling
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<TopP>,

    /// Enable streaming response
    #[serde(default)]
    pub stream: bool,

    /// Tool/function definitions for function calling
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,

    /// Request metadata for routing, billing, and audit
    #[serde(default)]
    pub metadata: RequestMetadata,

    /// Request creation timestamp
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

impl GatewayRequest {
    /// Create a new request builder
    pub fn builder() -> GatewayRequestBuilder {
        GatewayRequestBuilder::new()
    }

    /// Validate the request and return a validated wrapper
    pub fn validate(self) -> Result<ValidatedRequest, ValidationError> {
        // Validation logic
        if self.messages.is_empty() {
            return Err(ValidationError::EmptyMessages);
        }
        Ok(ValidatedRequest(self))
    }
}

/// Builder with typestate for compile-time validation
pub struct GatewayRequestBuilder<Model = (), Messages = ()> {
    inner: GatewayRequest,
    _model: std::marker::PhantomData<Model>,
    _messages: std::marker::PhantomData<Messages>,
}

pub struct ModelSet;
pub struct MessagesSet;

impl GatewayRequestBuilder<(), ()> {
    pub fn new() -> Self {
        Self {
            inner: GatewayRequest {
                id: generate_request_id(),
                model: ModelId::default(),
                messages: Vec::new(),
                temperature: None,
                max_tokens: None,
                top_p: None,
                stream: false,
                tools: None,
                metadata: RequestMetadata::default(),
                created_at: Utc::now(),
            },
            _model: std::marker::PhantomData,
            _messages: std::marker::PhantomData,
        }
    }
}

impl<Messages> GatewayRequestBuilder<(), Messages> {
    pub fn model(self, model: impl Into<String>) -> Result<GatewayRequestBuilder<ModelSet, Messages>, ValidationError> {
        let model_id = ModelId::new(model.into())?;
        Ok(GatewayRequestBuilder {
            inner: GatewayRequest {
                model: model_id,
                ..self.inner
            },
            _model: std::marker::PhantomData,
            _messages: std::marker::PhantomData,
        })
    }
}

impl<Model> GatewayRequestBuilder<Model, ()> {
    pub fn messages(self, messages: Vec<ChatMessage>) -> Result<GatewayRequestBuilder<Model, MessagesSet>, ValidationError> {
        if messages.is_empty() {
            return Err(ValidationError::EmptyMessages);
        }
        Ok(GatewayRequestBuilder {
            inner: GatewayRequest {
                messages,
                ..self.inner
            },
            _model: std::marker::PhantomData,
            _messages: std::marker::PhantomData,
        })
    }
}

impl GatewayRequestBuilder<ModelSet, MessagesSet> {
    /// Build the request - only available when model AND messages are set
    pub fn build(self) -> GatewayRequest {
        self.inner
    }
}

// Optional setters available at any state
impl<Model, Messages> GatewayRequestBuilder<Model, Messages> {
    pub fn temperature(mut self, temp: f32) -> Result<Self, ValidationError> {
        self.inner.temperature = Some(Temperature::new(temp)?);
        Ok(self)
    }

    pub fn max_tokens(mut self, tokens: u32) -> Result<Self, ValidationError> {
        self.inner.max_tokens = Some(MaxTokens::new(tokens)?);
        Ok(self)
    }

    pub fn stream(mut self, stream: bool) -> Self {
        self.inner.stream = stream;
        self
    }
}

fn generate_request_id() -> RequestId {
    RequestId::new(Uuid::now_v7().to_string())
        .expect("UUID is always valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_requires_model_and_messages() {
        // This should compile
        let request = GatewayRequest::builder()
            .model("gpt-4").unwrap()
            .messages(vec![ChatMessage::user("Hello")]).unwrap()
            .build();

        assert_eq!(request.model.as_str(), "gpt-4");
    }

    #[test]
    fn test_builder_rejects_empty_messages() {
        let result = GatewayRequest::builder()
            .model("gpt-4").unwrap()
            .messages(vec![]);

        assert!(result.is_err());
    }
}
```

#### 3.1.2 GatewayError Implementation

```rust
// crates/gateway-core/src/error.rs

use axum::http::StatusCode;
use std::time::Duration;
use thiserror::Error;

/// Comprehensive gateway error type with categorization.
///
/// Each variant maps to a specific HTTP status code and includes
/// all context needed for debugging and client communication.
#[derive(Debug, Error)]
pub enum GatewayError {
    // ─── Client Errors (4xx) ───────────────────────────────────────────

    #[error("Validation error: {message}")]
    Validation {
        message: String,
        field: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Authentication failed: {message}")]
    Authentication {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Authorization denied: {message}")]
    Authorization {
        message: String,
        required_permission: Option<String>,
    },

    #[error("Rate limit exceeded")]
    RateLimit {
        retry_after: Option<Duration>,
        limit: u64,
        window: Duration,
    },

    #[error("Model not found: {model}")]
    ModelNotFound { model: String },

    #[error("Request payload too large: {size} bytes (max: {max_size})")]
    PayloadTooLarge { size: usize, max_size: usize },

    // ─── Server Errors (5xx) ───────────────────────────────────────────

    #[error("Provider error: {provider} - {message}")]
    Provider {
        provider: String,
        message: String,
        status_code: Option<u16>,
        retryable: bool,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Circuit breaker open for provider: {provider}")]
    CircuitBreakerOpen {
        provider: String,
        reset_at: Option<std::time::Instant>,
    },

    #[error("No healthy providers available for model: {model}")]
    NoHealthyProviders { model: String },

    #[error("Request timeout after {duration:?}")]
    Timeout {
        duration: Duration,
        stage: String,
    },

    #[error("Internal error: {message}")]
    Internal {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl GatewayError {
    /// HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            // 4xx Client Errors
            Self::Validation { .. } => StatusCode::BAD_REQUEST,
            Self::Authentication { .. } => StatusCode::UNAUTHORIZED,
            Self::Authorization { .. } => StatusCode::FORBIDDEN,
            Self::RateLimit { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::ModelNotFound { .. } => StatusCode::NOT_FOUND,
            Self::PayloadTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,

            // 5xx Server Errors
            Self::Provider { status_code: Some(code), .. } if *code >= 400 && *code < 500 => {
                StatusCode::BAD_GATEWAY
            }
            Self::Provider { .. } => StatusCode::BAD_GATEWAY,
            Self::CircuitBreakerOpen { .. } => StatusCode::SERVICE_UNAVAILABLE,
            Self::NoHealthyProviders { .. } => StatusCode::SERVICE_UNAVAILABLE,
            Self::Timeout { .. } => StatusCode::GATEWAY_TIMEOUT,
            Self::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Error type for API response
    pub fn error_type(&self) -> &'static str {
        match self {
            Self::Validation { .. } => "invalid_request_error",
            Self::Authentication { .. } => "authentication_error",
            Self::Authorization { .. } => "permission_error",
            Self::RateLimit { .. } => "rate_limit_error",
            Self::ModelNotFound { .. } => "invalid_request_error",
            Self::PayloadTooLarge { .. } => "invalid_request_error",
            Self::Provider { .. } => "api_error",
            Self::CircuitBreakerOpen { .. } => "service_unavailable_error",
            Self::NoHealthyProviders { .. } => "service_unavailable_error",
            Self::Timeout { .. } => "timeout_error",
            Self::Internal { .. } => "internal_error",
        }
    }

    /// Error code for programmatic handling
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Validation { .. } => "validation_error",
            Self::Authentication { .. } => "invalid_api_key",
            Self::Authorization { .. } => "insufficient_permissions",
            Self::RateLimit { .. } => "rate_limit_exceeded",
            Self::ModelNotFound { .. } => "model_not_found",
            Self::PayloadTooLarge { .. } => "payload_too_large",
            Self::Provider { .. } => "provider_error",
            Self::CircuitBreakerOpen { .. } => "circuit_breaker_open",
            Self::NoHealthyProviders { .. } => "no_healthy_providers",
            Self::Timeout { .. } => "timeout",
            Self::Internal { .. } => "internal_error",
        }
    }

    /// Whether this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Provider { retryable: true, .. }
                | Self::Timeout { .. }
                | Self::RateLimit { .. }
        )
    }

    /// Create a validation error
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
            field: None,
            source: None,
        }
    }

    /// Create a validation error with field context
    pub fn validation_field(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
            field: Some(field.into()),
            source: None,
        }
    }

    /// Create an internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            source: None,
        }
    }

    /// Add source error context
    pub fn with_source(self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        match self {
            Self::Validation { message, field, .. } => Self::Validation {
                message,
                field,
                source: Some(Box::new(source)),
            },
            Self::Internal { message, .. } => Self::Internal {
                message,
                source: Some(Box::new(source)),
            },
            Self::Provider { provider, message, status_code, retryable, .. } => Self::Provider {
                provider,
                message,
                status_code,
                retryable,
                source: Some(Box::new(source)),
            },
            other => other,
        }
    }
}

/// API error response format (OpenAI-compatible)
#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub error: ApiErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
}

impl From<&GatewayError> for ApiErrorResponse {
    fn from(err: &GatewayError) -> Self {
        Self {
            error: ApiErrorDetail {
                error_type: err.error_type().to_string(),
                message: err.to_string(),
                code: err.error_code().to_string(),
                param: match err {
                    GatewayError::Validation { field, .. } => field.clone(),
                    _ => None,
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_codes() {
        assert_eq!(
            GatewayError::validation("test").status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            GatewayError::Authentication {
                message: "test".into(),
                source: None
            }.status_code(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn test_retryable() {
        assert!(GatewayError::RateLimit {
            retry_after: Some(Duration::from_secs(1)),
            limit: 100,
            window: Duration::from_secs(60),
        }.is_retryable());

        assert!(!GatewayError::validation("test").is_retryable());
    }
}
```

### 3.2 Additional Module Specifications

Due to the comprehensive nature of the implementation guide, detailed specifications for each module are provided in the following format:

| Module | Key Files | Critical Implementations |
|--------|-----------|-------------------------|
| **gateway-providers** | `traits.rs`, `registry.rs`, `openai/client.rs` | LLMProvider trait, ProviderRegistry, HTTP client with streaming |
| **gateway-routing** | `router.rs`, `rules.rs`, `balancer/*.rs` | Rules engine, load balancing strategies, health-aware routing |
| **gateway-resilience** | `circuit_breaker.rs`, `retry.rs`, `bulkhead.rs` | State machine, exponential backoff, semaphore-based isolation |
| **gateway-telemetry** | `metrics.rs`, `tracing.rs`, `audit.rs` | Prometheus registry, OpenTelemetry spans, structured audit logs |
| **gateway-server** | `server.rs`, `handlers/*.rs`, `middleware/*.rs` | Axum setup, request handlers, middleware pipeline |

---

## 4. Implementation Order & Dependencies

### 4.1 Dependency Graph

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      IMPLEMENTATION DEPENDENCY GRAPH                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Level 0 (No Dependencies)                                                   │
│  ┌─────────────────┐                                                        │
│  │  gateway-core   │  ← Implement FIRST                                     │
│  │  (types, error) │                                                        │
│  └────────┬────────┘                                                        │
│           │                                                                  │
│  Level 1 (Depends on core)                                                   │
│  ┌────────┴────────┐  ┌─────────────────┐  ┌─────────────────┐             │
│  │ gateway-config  │  │gateway-resilience│ │gateway-telemetry│             │
│  │    (config)     │  │  (circuit, retry)│ │(metrics, traces)│             │
│  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘             │
│           │                    │                    │                       │
│  Level 2 (Depends on Level 1)                                               │
│  ┌────────┴─────────────────────┴────────────────────┴───────┐              │
│  │                     gateway-providers                      │              │
│  │         (registry, openai, anthropic, etc.)               │              │
│  └──────────────────────────┬────────────────────────────────┘              │
│                             │                                                │
│  Level 3 (Depends on providers)                                             │
│  ┌──────────────────────────┴────────────────────────────────┐              │
│  │                      gateway-routing                       │              │
│  │              (router, balancer, rules)                    │              │
│  └──────────────────────────┬────────────────────────────────┘              │
│                             │                                                │
│  Level 4 (Depends on all)                                                   │
│  ┌──────────────────────────┴────────────────────────────────┐              │
│  │                      gateway-server                        │              │
│  │            (http, handlers, middleware)                   │              │
│  └──────────────────────────┬────────────────────────────────┘              │
│                             │                                                │
│  Level 5 (Binary)                                                           │
│  ┌──────────────────────────┴────────────────────────────────┐              │
│  │                         main.rs                            │              │
│  │                    (entry point)                          │              │
│  └───────────────────────────────────────────────────────────┘              │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Implementation Sequence

```
WEEK 1: Foundation
├── Day 1-2: gateway-core types
│   ├── GatewayRequest
│   ├── GatewayResponse
│   ├── GatewayError
│   └── Validated types (Temperature, MaxTokens, etc.)
├── Day 3-4: gateway-config
│   ├── Config schema
│   ├── YAML/TOML loader
│   └── Validation
└── Day 5: gateway-server skeleton
    ├── Basic Axum server
    └── Health endpoints

WEEK 2: Provider Layer
├── Day 1-2: Provider trait & registry
├── Day 3-4: OpenAI provider
│   ├── Request/response transform
│   ├── Streaming support
│   └── Health check
└── Day 5: Integration test

WEEK 3: Resilience
├── Day 1-2: Circuit breaker
├── Day 3: Retry policy
├── Day 4: Bulkhead & timeout
└── Day 5: Integration with providers

WEEK 4: Routing
├── Day 1-2: Router & rules engine
├── Day 3: Load balancing strategies
├── Day 4: Health-aware routing
└── Day 5: Integration test

WEEK 5: Middleware
├── Day 1-2: Auth middleware (API key, JWT)
├── Day 3: Rate limiting
├── Day 4: Logging & tracing
└── Day 5: Complete pipeline integration

WEEK 6: Observability
├── Day 1-2: Prometheus metrics
├── Day 3: OpenTelemetry tracing
├── Day 4: Structured logging & audit
└── Day 5: Dashboard setup

WEEK 7-8: Additional Providers
├── Anthropic
├── Google AI
├── vLLM
├── Ollama
├── Azure OpenAI
└── AWS Bedrock

WEEK 9-10: Production Hardening
├── Performance optimization
├── Security audit
├── Load testing
├── Documentation
└── CI/CD finalization
```

### 4.3 Parallel Development Tracks

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PARALLEL DEVELOPMENT TRACKS                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Track A: Core Development (Critical Path)                                   │
│  ─────────────────────────────────────────                                   │
│  Developer 1: gateway-core → gateway-providers → gateway-server             │
│                                                                              │
│  Track B: Infrastructure (Can Start Week 2)                                  │
│  ───────────────────────────────────────────                                 │
│  Developer 2: gateway-config → gateway-resilience → gateway-routing         │
│                                                                              │
│  Track C: Observability (Can Start Week 3)                                   │
│  ──────────────────────────────────────────                                  │
│  Developer 3: gateway-telemetry → Dashboards → Alerting                     │
│                                                                              │
│  Track D: DevOps (Can Start Week 1)                                         │
│  ──────────────────────────────────                                          │
│  DevOps: CI/CD → Docker → Kubernetes → Helm                                 │
│                                                                              │
│  Sync Points:                                                                │
│  • End of Week 2: Core + Config integrated                                   │
│  • End of Week 4: Resilience + Routing integrated                           │
│  • End of Week 6: Full observability integrated                             │
│  • End of Week 8: All providers complete                                    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Code Templates

### 5.1 Provider Template

```rust
// Template for implementing new providers
// crates/gateway-providers/src/{provider}/mod.rs

use crate::traits::{LLMProvider, ProviderCapabilities, HealthStatus};
use gateway_core::{GatewayRequest, GatewayResponse, GatewayError, ChatChunk};
use async_trait::async_trait;
use futures::stream::BoxStream;
use std::sync::Arc;

mod transform;
mod stream;

pub use transform::*;

/// {Provider} LLM Provider
///
/// Implements the LLMProvider trait for {Provider} API.
pub struct {Provider}Provider {
    client: reqwest::Client,
    config: {Provider}Config,
    metrics: Arc<ProviderMetrics>,
}

impl {Provider}Provider {
    /// Create a new {Provider} provider
    pub fn new(config: {Provider}Config) -> Result<Self, GatewayError> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .pool_max_idle_per_host(config.pool_size)
            .build()
            .map_err(|e| GatewayError::internal(e.to_string()))?;

        Ok(Self {
            client,
            config,
            metrics: Arc::new(ProviderMetrics::new("{provider}")),
        })
    }
}

#[async_trait]
impl LLMProvider for {Provider}Provider {
    fn id(&self) -> &str {
        &self.config.id
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::{Provider}
    }

    async fn chat_completion(
        &self,
        request: &GatewayRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        let timer = self.metrics.start_request();

        // Transform request to provider format
        let provider_request = transform::to_{provider}_request(request)?;

        // Execute request
        let response = self.client
            .post(&format!("{}/api/endpoint", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key.expose_secret()))
            .json(&provider_request)
            .send()
            .await
            .map_err(|e| GatewayError::Provider {
                provider: self.id().to_string(),
                message: e.to_string(),
                status_code: None,
                retryable: e.is_timeout() || e.is_connect(),
                source: Some(Box::new(e)),
            })?;

        // Handle response
        if response.status().is_success() {
            let provider_response: {Provider}Response = response
                .json()
                .await
                .map_err(|e| GatewayError::Provider {
                    provider: self.id().to_string(),
                    message: "Failed to parse response".to_string(),
                    status_code: None,
                    retryable: false,
                    source: Some(Box::new(e)),
                })?;

            let gateway_response = transform::from_{provider}_response(provider_response)?;
            self.metrics.record_success(timer);
            Ok(gateway_response)
        } else {
            let status = response.status().as_u16();
            let error_body = response.text().await.unwrap_or_default();
            self.metrics.record_failure(timer);

            Err(GatewayError::Provider {
                provider: self.id().to_string(),
                message: error_body,
                status_code: Some(status),
                retryable: status >= 500 || status == 429,
                source: None,
            })
        }
    }

    async fn chat_completion_stream(
        &self,
        request: &GatewayRequest,
    ) -> Result<BoxStream<'static, Result<ChatChunk, GatewayError>>, GatewayError> {
        stream::create_stream(self, request).await
    }

    async fn health_check(&self) -> HealthStatus {
        match self.client
            .get(&format!("{}/health", self.config.base_url))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => HealthStatus::Healthy,
            Ok(_) => HealthStatus::Degraded,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.config.capabilities
    }

    fn models(&self) -> &[ModelInfo] {
        &self.config.models
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};

    #[tokio::test]
    async fn test_chat_completion() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/endpoint"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    // Provider-specific response
                })))
            .mount(&mock_server)
            .await;

        let config = {Provider}Config {
            id: "test".to_string(),
            base_url: mock_server.uri(),
            // ...
        };

        let provider = {Provider}Provider::new(config).unwrap();
        let request = GatewayRequest::builder()
            .model("model-name")
            .messages(vec![ChatMessage::user("Hello")])
            .build();

        let response = provider.chat_completion(&request).await;
        assert!(response.is_ok());
    }
}
```

### 5.2 Middleware Template

```rust
// Template for implementing middleware
// crates/gateway-server/src/middleware/{name}.rs

use crate::middleware::{Middleware, Next};
use gateway_core::{GatewayRequest, GatewayResponse, GatewayError};
use async_trait::async_trait;
use std::sync::Arc;

/// {Name} Middleware
///
/// {Description of what this middleware does}
pub struct {Name}Middleware {
    config: {Name}Config,
    // Additional dependencies
}

impl {Name}Middleware {
    /// Create a new {Name} middleware
    pub fn new(config: {Name}Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Middleware for {Name}Middleware {
    async fn handle(
        &self,
        mut request: GatewayRequest,
        next: Next<'_>,
    ) -> Result<GatewayResponse, GatewayError> {
        // ─── Pre-processing ───────────────────────────────────────────
        // Modify request or perform checks before continuing

        // Example: Check some condition
        // if !self.check_condition(&request) {
        //     return Err(GatewayError::...);
        // }

        // Example: Modify request
        // request.metadata.some_field = Some(value);

        // ─── Call next middleware ─────────────────────────────────────
        let response = next.run(request).await?;

        // ─── Post-processing ──────────────────────────────────────────
        // Modify response or perform cleanup after response

        Ok(response)
    }

    fn name(&self) -> &'static str {
        "{name}"
    }

    fn priority(&self) -> u32 {
        // Lower = earlier in chain
        // 100: Auth
        // 200: Rate limit
        // 300: Validation
        // 400: Logging
        // 500: Default
        // 600: Caching
        // 900: Routing
        500
    }
}

#[derive(Debug, Clone)]
pub struct {Name}Config {
    // Configuration fields
}

impl Default for {Name}Config {
    fn default() -> Self {
        Self {
            // Default values
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_{name}_middleware() {
        let middleware = {Name}Middleware::new({Name}Config::default());

        let request = GatewayRequest::builder()
            .model("gpt-4")
            .messages(vec![ChatMessage::user("Hello")])
            .build();

        // Create mock next middleware
        let next = Next::mock(|_req| async {
            Ok(GatewayResponse::default())
        });

        let result = middleware.handle(request, next).await;
        assert!(result.is_ok());
    }
}
```

---

## 6. Testing Strategy

### 6.1 Test Categories

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          TEST ARCHITECTURE                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                      UNIT TESTS (70%)                               │    │
│  │  Location: src/**/*.rs (inline) + src/**/tests/*.rs                │    │
│  │  Framework: #[test], #[tokio::test]                                 │    │
│  │  Coverage target: 90%+ for core, 85%+ for providers                │    │
│  │                                                                     │    │
│  │  Categories:                                                        │    │
│  │  • Type validation (newtypes, builders)                            │    │
│  │  • Transformation logic (request/response)                         │    │
│  │  • State machines (circuit breaker, rate limiter)                  │    │
│  │  • Error handling paths                                            │    │
│  │  • Serialization/deserialization                                   │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                  INTEGRATION TESTS (20%)                            │    │
│  │  Location: tests/integration/*.rs                                   │    │
│  │  Framework: tokio::test + wiremock                                 │    │
│  │  Coverage target: All provider endpoints, all middleware           │    │
│  │                                                                     │    │
│  │  Categories:                                                        │    │
│  │  • Provider adapters (mocked responses)                            │    │
│  │  • Middleware pipeline (auth, rate limit)                          │    │
│  │  • Router + load balancer                                          │    │
│  │  • Circuit breaker behavior                                        │    │
│  │  • Configuration loading                                           │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                      E2E TESTS (10%)                                │    │
│  │  Location: tests/e2e/*.rs                                          │    │
│  │  Framework: Docker Compose + real providers (test keys)            │    │
│  │  Coverage target: Critical paths only                              │    │
│  │                                                                     │    │
│  │  Categories:                                                        │    │
│  │  • Full request lifecycle (non-streaming)                          │    │
│  │  • Full request lifecycle (streaming)                              │    │
│  │  • Multi-provider failover                                         │    │
│  │  • Hot configuration reload                                        │    │
│  │  • Graceful shutdown                                               │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                   PROPERTY-BASED TESTS                              │    │
│  │  Framework: proptest                                                │    │
│  │                                                                     │    │
│  │  Categories:                                                        │    │
│  │  • Input validation (fuzz all parameters)                          │    │
│  │  • Serialization roundtrip                                         │    │
│  │  • Rate limiter correctness                                        │    │
│  │  • Circuit breaker state transitions                               │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                    BENCHMARK TESTS                                  │    │
│  │  Location: benches/*.rs                                            │    │
│  │  Framework: criterion                                              │    │
│  │                                                                     │    │
│  │  Categories:                                                        │    │
│  │  • Request parsing                                                 │    │
│  │  • Routing decision                                                │    │
│  │  • Middleware pipeline                                             │    │
│  │  • JSON serialization                                              │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Test Coverage Requirements

| Module | Unit | Integration | E2E | Benchmark |
|--------|------|-------------|-----|-----------|
| gateway-core | 90% | - | - | Yes |
| gateway-config | 85% | 80% | - | - |
| gateway-providers | 85% | 90% | 50% | Yes |
| gateway-routing | 90% | 85% | - | Yes |
| gateway-resilience | 95% | 80% | - | - |
| gateway-telemetry | 80% | 70% | - | Yes |
| gateway-server | 80% | 85% | 80% | Yes |

### 6.3 Test Utilities

```rust
// tests/common/mod.rs

use gateway_core::{GatewayRequest, GatewayResponse, ChatMessage};
use wiremock::{MockServer, Mock, ResponseTemplate};
use std::sync::Arc;

/// Create a test request with minimal required fields
pub fn test_request() -> GatewayRequest {
    GatewayRequest::builder()
        .model("gpt-4").unwrap()
        .messages(vec![ChatMessage::user("Hello")]).unwrap()
        .build()
}

/// Create a mock OpenAI server
pub async fn mock_openai_server() -> MockServer {
    let server = MockServer::start().await;

    // Success response
    Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(openai_success_response()))
        .mount(&server)
        .await;

    server
}

/// Standard OpenAI success response
pub fn openai_success_response() -> serde_json::Value {
    serde_json::json!({
        "id": "chatcmpl-123",
        "object": "chat.completion",
        "created": 1677652288,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Hello! How can I help you today?"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 9,
            "completion_tokens": 12,
            "total_tokens": 21
        }
    })
}

/// Test configuration
pub fn test_config() -> GatewayConfig {
    GatewayConfig {
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 0, // Random available port
            ..Default::default()
        },
        ..Default::default()
    }
}
```

---

## 7. CI/CD Pipeline

### 7.1 GitHub Actions Workflow

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1
  RUSTFLAGS: "-D warnings"

jobs:
  # ─── Code Quality ────────────────────────────────────────────────────────
  lint:
    name: Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@1.75.0
        with:
          components: rustfmt, clippy

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

  # ─── Unit Tests ──────────────────────────────────────────────────────────
  test:
    name: Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@1.75.0

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Run tests
        run: cargo test --all-features --workspace

      - name: Run doc tests
        run: cargo test --doc --all-features

  # ─── Code Coverage ───────────────────────────────────────────────────────
  coverage:
    name: Coverage
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@1.75.0
        with:
          components: llvm-tools-preview

      - name: Install cargo-llvm-cov
        uses: taiki-e/install-action@cargo-llvm-cov

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Generate coverage report
        run: cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

      - name: Upload coverage to Codecov
        uses: codecov/codecov-action@v3
        with:
          files: lcov.info
          fail_ci_if_error: true

      - name: Check coverage threshold
        run: |
          COVERAGE=$(cargo llvm-cov --all-features --workspace --json | jq '.data[0].totals.lines.percent')
          if (( $(echo "$COVERAGE < 85" | bc -l) )); then
            echo "Coverage $COVERAGE% is below threshold 85%"
            exit 1
          fi

  # ─── Security Audit ──────────────────────────────────────────────────────
  security:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@1.75.0

      - name: Install cargo-audit
        run: cargo install cargo-audit

      - name: Run security audit
        run: cargo audit

      - name: Run cargo deny
        uses: EmbarkStudios/cargo-deny-action@v1

  # ─── Build ───────────────────────────────────────────────────────────────
  build:
    name: Build
    runs-on: ubuntu-latest
    needs: [lint, test]
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@1.75.0

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Build release
        run: cargo build --release --all-features

      - name: Upload binary
        uses: actions/upload-artifact@v3
        with:
          name: llm-inference-gateway
          path: target/release/llm-inference-gateway

  # ─── Integration Tests ───────────────────────────────────────────────────
  integration:
    name: Integration Tests
    runs-on: ubuntu-latest
    needs: [build]
    services:
      redis:
        image: redis:7
        ports:
          - 6379:6379
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@1.75.0

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Run integration tests
        run: cargo test --test '*' --all-features
        env:
          REDIS_URL: redis://localhost:6379

  # ─── Docker Build ────────────────────────────────────────────────────────
  docker:
    name: Docker Build
    runs-on: ubuntu-latest
    needs: [integration]
    steps:
      - uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Build Docker image
        uses: docker/build-push-action@v5
        with:
          context: .
          push: false
          tags: llm-inference-gateway:${{ github.sha }}
          cache-from: type=gha
          cache-to: type=gha,mode=max

      - name: Scan image for vulnerabilities
        uses: aquasecurity/trivy-action@master
        with:
          image-ref: llm-inference-gateway:${{ github.sha }}
          format: 'sarif'
          output: 'trivy-results.sarif'

  # ─── Load Tests (on main only) ───────────────────────────────────────────
  load-test:
    name: Load Test
    runs-on: ubuntu-latest
    needs: [docker]
    if: github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v4

      - name: Run load tests
        run: |
          docker compose -f docker-compose.test.yml up -d
          sleep 10
          # Run k6 or similar load testing tool
          docker compose -f docker-compose.test.yml down
```

### 7.2 Release Workflow

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write
  packages: write

jobs:
  release:
    name: Release
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Build binaries
        run: |
          cargo build --release --target x86_64-unknown-linux-gnu
          cargo build --release --target aarch64-unknown-linux-gnu

      - name: Build and push Docker image
        uses: docker/build-push-action@v5
        with:
          context: .
          push: true
          tags: |
            ghcr.io/${{ github.repository }}:${{ github.ref_name }}
            ghcr.io/${{ github.repository }}:latest

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          files: |
            target/x86_64-unknown-linux-gnu/release/llm-inference-gateway
            target/aarch64-unknown-linux-gnu/release/llm-inference-gateway
          generate_release_notes: true
```

---

## 8. Quality Gates

### 8.1 Gate Definitions

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           QUALITY GATES                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  GATE 1: PRE-COMMIT (Local)                                                 │
│  ──────────────────────────                                                  │
│  Trigger: git commit                                                        │
│  Checks:                                                                    │
│    ✓ cargo fmt --check                                                      │
│    ✓ cargo clippy -- -D warnings                                            │
│    ✓ cargo test (unit only)                                                 │
│  Blocking: Yes (commit rejected if fails)                                   │
│                                                                              │
│  ─────────────────────────────────────────────────────────────────────────  │
│                                                                              │
│  GATE 2: PR CHECKS (CI)                                                     │
│  ──────────────────────                                                      │
│  Trigger: Pull request                                                      │
│  Checks:                                                                    │
│    ✓ All Gate 1 checks                                                      │
│    ✓ cargo test --all-features (full suite)                                 │
│    ✓ Code coverage ≥ 85%                                                    │
│    ✓ cargo audit (no vulnerabilities)                                       │
│    ✓ cargo deny (license compliance)                                        │
│    ✓ Documentation compiles                                                 │
│  Blocking: Yes (PR cannot merge if fails)                                   │
│                                                                              │
│  ─────────────────────────────────────────────────────────────────────────  │
│                                                                              │
│  GATE 3: MERGE TO MAIN (CI)                                                 │
│  ──────────────────────────                                                  │
│  Trigger: Merge to main branch                                              │
│  Checks:                                                                    │
│    ✓ All Gate 2 checks                                                      │
│    ✓ Integration tests pass                                                 │
│    ✓ E2E tests pass                                                         │
│    ✓ Docker image builds                                                    │
│    ✓ Image security scan (Trivy)                                            │
│  Blocking: Yes (merge blocked if fails)                                     │
│                                                                              │
│  ─────────────────────────────────────────────────────────────────────────  │
│                                                                              │
│  GATE 4: RELEASE (CD)                                                       │
│  ────────────────────                                                        │
│  Trigger: Tag push (v*)                                                     │
│  Checks:                                                                    │
│    ✓ All Gate 3 checks                                                      │
│    ✓ Load test: P95 < 5ms at 10K RPS                                        │
│    ✓ Soak test: 1 hour at 5K RPS (no leak)                                  │
│    ✓ Canary deployment successful                                           │
│  Blocking: Yes (release blocked if fails)                                   │
│                                                                              │
│  ─────────────────────────────────────────────────────────────────────────  │
│                                                                              │
│  GATE 5: PRODUCTION (Monitoring)                                            │
│  ───────────────────────────────                                             │
│  Trigger: Production deployment                                             │
│  Checks:                                                                    │
│    ✓ Health endpoints return 200                                            │
│    ✓ Error rate < 1% for 15 minutes                                         │
│    ✓ P95 latency < 5ms                                                      │
│    ✓ No increase in 5xx errors                                              │
│  Action: Auto-rollback if fails                                             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 8.2 Pre-commit Hook

```bash
#!/bin/bash
# .git/hooks/pre-commit

set -e

echo "Running pre-commit checks..."

# Format check
echo "  Checking formatting..."
cargo fmt --all -- --check

# Clippy
echo "  Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings

# Quick tests
echo "  Running quick tests..."
cargo test --lib --quiet

echo "All checks passed!"
```

---

## 9. Deployment Guide

### 9.1 Dockerfile

```dockerfile
# Dockerfile
# Multi-stage build for minimal image size

# ─── Build Stage ───────────────────────────────────────────────────────────
FROM rust:1.75-bookworm AS builder

WORKDIR /app

# Copy manifests first for caching
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/gateway-core/Cargo.toml crates/gateway-core/
COPY crates/gateway-config/Cargo.toml crates/gateway-config/
COPY crates/gateway-providers/Cargo.toml crates/gateway-providers/
COPY crates/gateway-routing/Cargo.toml crates/gateway-routing/
COPY crates/gateway-resilience/Cargo.toml crates/gateway-resilience/
COPY crates/gateway-telemetry/Cargo.toml crates/gateway-telemetry/
COPY crates/gateway-server/Cargo.toml crates/gateway-server/

# Create dummy source files for dependency caching
RUN mkdir -p crates/gateway-core/src && echo "fn main() {}" > crates/gateway-core/src/lib.rs && \
    mkdir -p crates/gateway-config/src && echo "fn main() {}" > crates/gateway-config/src/lib.rs && \
    mkdir -p crates/gateway-providers/src && echo "fn main() {}" > crates/gateway-providers/src/lib.rs && \
    mkdir -p crates/gateway-routing/src && echo "fn main() {}" > crates/gateway-routing/src/lib.rs && \
    mkdir -p crates/gateway-resilience/src && echo "fn main() {}" > crates/gateway-resilience/src/lib.rs && \
    mkdir -p crates/gateway-telemetry/src && echo "fn main() {}" > crates/gateway-telemetry/src/lib.rs && \
    mkdir -p crates/gateway-server/src && echo "fn main() {}" > crates/gateway-server/src/lib.rs && \
    mkdir -p src && echo "fn main() {}" > src/main.rs

# Build dependencies only
RUN cargo build --release && rm -rf src crates

# Copy actual source code
COPY crates crates
COPY src src

# Build the application
RUN cargo build --release --all-features

# ─── Runtime Stage ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/llm-inference-gateway /app/llm-inference-gateway

# Copy default config
COPY config/default.yaml /app/config/default.yaml

# Create non-root user
RUN useradd -r -s /bin/false gateway && \
    chown -R gateway:gateway /app

USER gateway

EXPOSE 8080

ENV RUST_LOG=info
ENV CONFIG_PATH=/app/config/default.yaml

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health/live || exit 1

ENTRYPOINT ["/app/llm-inference-gateway"]
CMD ["--config", "/app/config/default.yaml"]
```

### 9.2 Kubernetes Deployment

```yaml
# k8s/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: llm-inference-gateway
  labels:
    app: llm-inference-gateway
spec:
  replicas: 3
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  selector:
    matchLabels:
      app: llm-inference-gateway
  template:
    metadata:
      labels:
        app: llm-inference-gateway
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "8080"
        prometheus.io/path: "/metrics"
    spec:
      serviceAccountName: llm-inference-gateway
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
      containers:
      - name: gateway
        image: ghcr.io/llm-devops/llm-inference-gateway:latest
        imagePullPolicy: Always
        ports:
        - name: http
          containerPort: 8080
          protocol: TCP
        env:
        - name: RUST_LOG
          value: "info"
        - name: CONFIG_PATH
          value: "/etc/gateway/config.yaml"
        envFrom:
        - secretRef:
            name: llm-gateway-secrets
        resources:
          requests:
            cpu: "1"
            memory: "1Gi"
          limits:
            cpu: "2"
            memory: "2Gi"
        livenessProbe:
          httpGet:
            path: /health/live
            port: http
          initialDelaySeconds: 5
          periodSeconds: 10
          timeoutSeconds: 3
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /health/ready
            port: http
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 3
        volumeMounts:
        - name: config
          mountPath: /etc/gateway
          readOnly: true
      volumes:
      - name: config
        configMap:
          name: llm-gateway-config
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            podAffinityTerm:
              labelSelector:
                matchLabels:
                  app: llm-inference-gateway
              topologyKey: kubernetes.io/hostname
      topologySpreadConstraints:
      - maxSkew: 1
        topologyKey: topology.kubernetes.io/zone
        whenUnsatisfiable: ScheduleAnyway
        labelSelector:
          matchLabels:
            app: llm-inference-gateway
```

---

## 10. Post-Implementation Checklist

### 10.1 Final Verification

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    POST-IMPLEMENTATION VERIFICATION                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  COMPILATION & BUILD                                                         │
│  ───────────────────                                                         │
│  [ ] cargo build --release compiles with ZERO warnings                      │
│  [ ] cargo build --release --all-features compiles                          │
│  [ ] Docker image builds successfully                                        │
│  [ ] All feature flags combinations compile                                  │
│                                                                              │
│  TESTING                                                                     │
│  ───────                                                                     │
│  [ ] cargo test --all-features passes 100%                                   │
│  [ ] Integration tests pass                                                  │
│  [ ] E2E tests pass                                                          │
│  [ ] Load test meets targets: P95 <5ms, 10K RPS                             │
│  [ ] Soak test passes: 1 hour, no memory leak                               │
│  [ ] Code coverage ≥ 85%                                                     │
│                                                                              │
│  SECURITY                                                                    │
│  ────────                                                                    │
│  [ ] cargo audit: zero vulnerabilities                                       │
│  [ ] cargo deny: all licenses approved                                       │
│  [ ] Trivy scan: no critical vulnerabilities                                │
│  [ ] Secrets not logged (verify with grep)                                  │
│  [ ] PII redaction working (verify in logs)                                 │
│  [ ] TLS 1.3 enforced (verify with openssl)                                 │
│                                                                              │
│  OBSERVABILITY                                                               │
│  ─────────────                                                               │
│  [ ] Prometheus metrics exposed at /metrics                                  │
│  [ ] All request metrics recording                                          │
│  [ ] Traces propagating to collector                                        │
│  [ ] Structured logs in JSON format                                         │
│  [ ] Audit logs capturing all requests                                      │
│  [ ] Grafana dashboards configured                                          │
│  [ ] Alerting rules active                                                  │
│                                                                              │
│  DOCUMENTATION                                                               │
│  ─────────────                                                               │
│  [ ] README complete with quick start                                        │
│  [ ] API documentation generated (OpenAPI)                                  │
│  [ ] Architecture decision records complete                                 │
│  [ ] Operations runbook written                                             │
│  [ ] Configuration reference documented                                     │
│                                                                              │
│  DEPLOYMENT                                                                  │
│  ──────────                                                                  │
│  [ ] Kubernetes manifests validated                                         │
│  [ ] Helm chart tested                                                      │
│  [ ] Health endpoints working                                               │
│  [ ] Graceful shutdown verified                                             │
│  [ ] HPA configured and tested                                              │
│  [ ] PDB configured                                                         │
│                                                                              │
│  PERFORMANCE                                                                 │
│  ───────────                                                                 │
│  [ ] P50 latency < 2ms (gateway overhead)                                    │
│  [ ] P95 latency < 5ms (gateway overhead)                                    │
│  [ ] Throughput > 10K RPS per instance                                      │
│  [ ] Memory < 256MB at baseline                                              │
│  [ ] No memory leaks under load                                             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 10.2 Release Criteria

| Criteria | Requirement | Verification Method |
|----------|-------------|---------------------|
| **Compilation** | Zero warnings in release mode | CI build log |
| **Tests** | 100% pass rate | CI test results |
| **Coverage** | ≥85% line coverage | Coverage report |
| **Security** | Zero high/critical vulnerabilities | Audit + scan reports |
| **Performance** | P95 <5ms at 10K RPS | Load test results |
| **Documentation** | All public APIs documented | Doc generation |
| **Deployment** | Canary successful for 24 hours | Monitoring dashboards |

---

## Document References

| Document | Location | Purpose |
|----------|----------|---------|
| Specification | `plans/LLM-Inference-Gateway-Specification.md` | Requirements |
| Pseudocode | `plans/LLM-Inference-Gateway-Pseudocode.md` | Detailed pseudocode |
| Architecture | `plans/LLM-Inference-Gateway-Architecture.md` | System design |
| Refinement | `plans/LLM-Inference-Gateway-Refinement.md` | Quality guidelines |
| Edge Cases | `EDGE_CASES_AND_ERROR_HANDLING.md` | Error scenarios |
| Type Safety | `TYPE_SAFETY_RULES.md` | Type system |
| Concurrency | `docs/CONCURRENCY_PATTERNS.md` | Thread safety |
| Performance | `PERFORMANCE-OPTIMIZATION-CHECKLIST.md` | Optimization |
| Dependencies | `plans/DEPENDENCY-MATRIX.md` | Version matrix |
| Coding Standards | `RUST-CODING-STANDARDS.md` | Code quality |

---

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-11-27 | SPARC Swarm | Initial completion document |

---

## Conclusion

This Completion document provides everything needed to implement the LLM Inference Gateway from zero to production:

1. **115 discrete tasks** with file locations, line estimates, and coverage targets
2. **Dependency-ordered implementation** with parallel development tracks
3. **Code templates** for providers and middleware
4. **Complete test strategy** with quality gates
5. **CI/CD pipeline** with automated checks at every stage
6. **Kubernetes deployment** with production-ready manifests
7. **Post-implementation checklist** for final verification

**Implementation can begin immediately.**

The SPARC methodology phases are now complete:
- **S**pecification: Requirements and scope defined
- **P**seudocode: Detailed algorithms and data structures
- **A**rchitecture: System design and component interactions
- **R**efinement: Quality standards and edge cases
- **C**ompletion: Implementation roadmap (this document)

**Status: Ready for Development**
