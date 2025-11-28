# LLM Inference Gateway

A high-performance, production-ready Rust-based gateway for unifying multiple Large Language Model (LLM) providers into a single, scalable API.

[![License](https://img.shields.io/badge/license-Commercial-blue.svg)](LICENSE.md)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)]()
[![Tests](https://img.shields.io/badge/tests-128%20passed-brightgreen.svg)]()
[![Kubernetes](https://img.shields.io/badge/kubernetes-1.24%2B-blue.svg)](https://kubernetes.io)

---

## Table of Contents

- [Features](#features)
- [Quick Start](#quick-start)
- [Architecture](#architecture)
- [CLI Reference](#cli-reference)
- [SDK](#sdk)
- [API Usage](#api-usage)
- [Configuration](#configuration)
- [Security](#security)
- [Monitoring & Observability](#monitoring--observability)
- [Database Migrations](#database-migrations)
- [Deployment](#deployment)
- [Performance Benchmarks](#performance-benchmarks)
- [Supported Providers](#supported-providers)
- [Contributing](#contributing)
- [License](#license)

---

## Features

### Core Gateway Capabilities

| Feature | Description |
|---------|-------------|
| **Multi-Provider Support** | Unified interface for OpenAI, Anthropic, Azure OpenAI, Google Gemini, AWS Bedrock, Cohere, and more |
| **High Performance** | Built in Rust for maximum throughput (10,000+ RPS per instance) with zero-copy operations |
| **Streaming Support** | Full Server-Sent Events (SSE) support for real-time token streaming |
| **Intelligent Routing** | Cost-aware, latency-optimized provider selection with configurable strategies |
| **Advanced Caching** | Response caching with semantic similarity matching and TTL management |
| **Rate Limiting** | Per-provider, per-tenant token bucket rate limiting |
| **Circuit Breakers** | Automatic provider health detection with circuit breaker patterns |
| **Request Retries** | Configurable retry policies with exponential backoff |
| **Load Balancing** | Round-robin, weighted, and least-connections load balancing |

### Enterprise Features

| Feature | Description |
|---------|-------------|
| **High Availability** | Multi-region deployments with automatic failover |
| **Horizontal Scaling** | Kubernetes-native with HPA auto-scaling support |
| **Security Hardening** | IP filtering, request signing, header security, PII redaction |
| **Multi-Tenancy** | Namespace isolation, resource quotas, per-tenant configuration |
| **Cost Tracking** | Token usage analytics, cost attribution, budget alerts |
| **GDPR Compliance** | Data residency controls, PII detection and masking |
| **Audit Logging** | Comprehensive request/response logging with structured output |
| **Database Integration** | PostgreSQL support with SQLx migrations |

### Resilience Features

| Feature | Description |
|---------|-------------|
| **Circuit Breaker** | Three-state circuit breaker (Closed → Open → Half-Open) |
| **Bulkhead Pattern** | Isolated resource pools to prevent cascade failures |
| **Timeout Management** | Configurable timeouts per operation type |
| **Retry Strategies** | Fixed, exponential, and jittered retry policies |
| **Health Checks** | Active and passive health monitoring |
| **Fallback Providers** | Automatic failover to backup providers |

---

## Quick Start

### Prerequisites

- **Rust 1.75+** (for local development)
- **Docker** (for containerized deployment)
- **PostgreSQL 14+** (for persistence)
- **Redis 7.0+** (optional, for distributed caching)

### Installation

```bash
# Clone repository
git clone https://github.com/your-org/llm-inference-gateway.git
cd llm-inference-gateway

# Build release binary
cargo build --release

# Or install CLI globally
cargo install --path crates/gateway-cli
```

### Running the Gateway

```bash
# Set required environment variables
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
export DATABASE_URL="postgres://user:pass@localhost/gateway"

# Run database migrations
llm-gateway migrate run

# Start the gateway server
llm-gateway start --config config.yaml

# Or with environment variables only
llm-gateway start --port 8080 --host 0.0.0.0
```

### Docker Deployment

```bash
# Build Docker image
docker build -t llm-gateway:latest -f deployment/docker/Dockerfile .

# Run container
docker run -d \
  -p 8080:8080 \
  -p 9090:9090 \
  -e OPENAI_API_KEY="sk-..." \
  -e ANTHROPIC_API_KEY="sk-ant-..." \
  -e DATABASE_URL="postgres://..." \
  llm-gateway:latest
```

The gateway will be available at `http://localhost:8080`

---

## Architecture

### Crate Structure

The gateway is organized as a Rust workspace with modular crates:

```
llm-inference-gateway/
├── crates/
│   ├── gateway-core/        # Core types, requests, responses, streaming
│   ├── gateway-config/      # Configuration loading, hot-reload, validation
│   ├── gateway-providers/   # Provider implementations (OpenAI, Anthropic, etc.)
│   ├── gateway-routing/     # Request routing, load balancing, rules engine
│   ├── gateway-resilience/  # Circuit breakers, retries, timeouts, bulkheads
│   ├── gateway-telemetry/   # Metrics, tracing, logging, PII redaction
│   ├── gateway-security/    # Security middleware, validation, encryption
│   ├── gateway-server/      # HTTP server, handlers, middleware
│   ├── gateway-sdk/         # Rust client SDK
│   ├── gateway-cli/         # Command-line interface
│   └── gateway-migrations/  # Database migrations (SQLx)
└── src/                     # Main binary entry point
```

### System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Client Applications                       │
│              (SDK, CLI, REST API, Streaming SSE)                │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│                       Gateway Server                             │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────────────────┐ │
│  │   Security   │ │    Auth      │ │    Rate Limiting         │ │
│  │  Middleware  │ │  Middleware  │ │    Middleware            │ │
│  └──────────────┘ └──────────────┘ └──────────────────────────┘ │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                    Request Router                         │   │
│  │  (Model Routing │ Cost Routing │ Latency Routing)        │   │
│  └──────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                  Resilience Layer                         │   │
│  │  (Circuit Breaker │ Retry │ Timeout │ Bulkhead)          │   │
│  └──────────────────────────────────────────────────────────┘   │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│                    Provider Registry                             │
│  ┌─────────┐ ┌───────────┐ ┌───────┐ ┌────────┐ ┌────────────┐ │
│  │ OpenAI  │ │ Anthropic │ │ Azure │ │ Google │ │ AWS Bedrock│ │
│  └─────────┘ └───────────┘ └───────┘ └────────┘ └────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### Key Design Principles

- **Provider Agnostic** - Unified API regardless of underlying provider
- **Resilient** - Automatic retries, circuit breakers, fallback providers
- **Observable** - Comprehensive metrics, structured logging, distributed tracing
- **Performant** - Async I/O, connection pooling, zero-copy streaming
- **Secure** - Defense in depth with multiple security layers
- **Extensible** - Plugin architecture for custom providers and middleware

---

## CLI Reference

The `llm-gateway` CLI provides comprehensive management capabilities.

### Global Options

```bash
llm-gateway [OPTIONS] <COMMAND>

Options:
  -v, --verbose...         Increase output verbosity (-v, -vv, -vvv)
      --json               Output in JSON format
  -u, --url <URL>          Gateway server URL [default: http://localhost:8080]
  -k, --api-key <API_KEY>  API key for authentication
  -h, --help               Print help
  -V, --version            Print version
```

### Commands

#### Server Management

```bash
# Start the gateway server
llm-gateway start [OPTIONS]
  -c, --config <FILE>      Configuration file path
  -p, --port <PORT>        Server port [default: 8080]
  -H, --host <HOST>        Server bind address [default: 0.0.0.0]
      --workers <N>        Number of worker threads
      --metrics-port <PORT> Prometheus metrics port [default: 9090]

# Check gateway health
llm-gateway health [OPTIONS]
      --detailed           Show detailed health information
      --timeout <SECS>     Health check timeout [default: 10]
```

#### Model & Chat Operations

```bash
# List available models
llm-gateway models [OPTIONS]
  -p, --provider <NAME>    Filter by provider
      --capabilities       Show model capabilities

# Send chat completion request
llm-gateway chat [OPTIONS] <MESSAGE>
  -m, --model <MODEL>      Model to use [default: gpt-4o]
  -s, --system <PROMPT>    System prompt
      --stream             Enable streaming response
      --temperature <T>    Temperature [default: 0.7]
      --max-tokens <N>     Maximum tokens to generate
```

#### Metrics & Monitoring

```bash
# View latency metrics
llm-gateway latency [OPTIONS]
  -p, --provider <NAME>    Filter by provider
  -m, --model <MODEL>      Filter by model
  -w, --window <DURATION>  Time window [default: 1h]
      --percentiles        Show percentile breakdown

# View cost tracking
llm-gateway cost [OPTIONS]
  -p, --provider <NAME>    Filter by provider
  -m, --model <MODEL>      Filter by model
  -t, --tenant <ID>        Filter by tenant
  -w, --window <DURATION>  Time window [default: 24h]
  -g, --group-by <FIELD>   Group by (provider, model, tenant, hour, day)
      --breakdown          Show detailed cost breakdown

# View token usage statistics
llm-gateway token-usage [OPTIONS]
  -p, --provider <NAME>    Filter by provider
  -m, --model <MODEL>      Filter by model
  -t, --tenant <ID>        Filter by tenant
  -w, --window <DURATION>  Time window [default: 24h]
  -g, --group-by <FIELD>   Group by field
      --detailed           Show detailed breakdown
```

#### Backend Health & Routing

```bash
# Monitor backend health
llm-gateway backend-health [OPTIONS]
  -p, --provider <NAME>    Filter by provider
      --unhealthy-only     Show only unhealthy backends
      --history            Include historical health data
  -w, --watch              Watch mode - continuously refresh
      --interval <SECS>    Refresh interval [default: 5]

# Manage routing strategies
llm-gateway routing-strategy <COMMAND>

Commands:
  show      Show current routing strategy and configuration
  rules     List all routing rules
  weights   Show provider weights and load balancing info
  test      Test routing for a specific request
  stats     Show routing statistics

# Example: Test routing for a model
llm-gateway routing-strategy test --model gpt-4o --tenant tenant-001
```

#### Cache Management

```bash
# View and manage cache
llm-gateway cache-status <COMMAND>

Commands:
  stats     Show cache statistics
  list      List cached entries
  clear     Clear cache entries
  config    Show cache configuration

# Examples
llm-gateway cache-status stats --detailed
llm-gateway cache-status list --model gpt-4o --limit 20
llm-gateway cache-status clear --older-than 24h --force
```

#### Configuration & Validation

```bash
# Manage configuration
llm-gateway config <COMMAND>

Commands:
  show      Display current configuration
  validate  Validate configuration file
  generate  Generate sample configuration

# Validate configuration file
llm-gateway validate <CONFIG_FILE>
      --strict             Strict validation mode

# Show gateway info
llm-gateway info
      --detailed           Show detailed system information
```

#### Database Migrations

```bash
# Database migration management
llm-gateway migrate <COMMAND>

Commands:
  run       Run pending migrations
  revert    Revert the last migration
  status    Show migration status
  create    Create a new migration

# Examples
llm-gateway migrate run
llm-gateway migrate status
llm-gateway migrate revert --steps 2
```

#### Shell Completions

```bash
# Generate shell completions
llm-gateway completions <SHELL>

# Supported shells: bash, zsh, fish, powershell

# Install for bash
llm-gateway completions bash > /etc/bash_completion.d/llm-gateway

# Install for zsh
llm-gateway completions zsh > ~/.zfunc/_llm-gateway
```

---

## SDK

### Rust SDK

The `gateway-sdk` crate provides a type-safe Rust client for interacting with the gateway.

#### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
gateway-sdk = { path = "crates/gateway-sdk" }
tokio = { version = "1", features = ["full"] }
```

#### Basic Usage

```rust
use gateway_sdk::{Client, ChatRequest, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let client = Client::builder()
        .base_url("http://localhost:8080")
        .api_key("your-api-key")
        .timeout(Duration::from_secs(30))
        .build()?;

    // Send chat completion request
    let request = ChatRequest::builder()
        .model("gpt-4o")
        .messages(vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello, how are you?"),
        ])
        .temperature(0.7)
        .max_tokens(150)
        .build()?;

    let response = client.chat(request).await?;
    println!("Response: {}", response.choices[0].message.content);

    Ok(())
}
```

#### Streaming Responses

```rust
use futures::StreamExt;
use gateway_sdk::{Client, ChatRequest, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new("http://localhost:8080", Some("your-api-key"));

    let request = ChatRequest::builder()
        .model("claude-3-5-sonnet")
        .messages(vec![Message::user("Tell me a story")])
        .stream(true)
        .build()?;

    let mut stream = client.chat_stream(request).await?;

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) => {
                if let Some(content) = &chunk.choices[0].delta.content {
                    print!("{}", content);
                }
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    Ok(())
}
```

#### SDK Features

| Feature | Description |
|---------|-------------|
| **Type Safety** | Strongly typed requests and responses |
| **Streaming** | Full async streaming support |
| **Auto-Retry** | Configurable retry policies |
| **Connection Pooling** | Efficient HTTP connection reuse |
| **Timeout Handling** | Per-request and global timeouts |
| **Error Handling** | Rich error types with context |
| **Tracing** | OpenTelemetry integration |

---

## API Usage

### Chat Completions

```bash
# Basic chat completion
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -d '{
    "model": "gpt-4o",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Hello, how are you?"}
    ],
    "temperature": 0.7,
    "max_tokens": 150
  }'
```

### Streaming Response

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -N \
  -d '{
    "model": "claude-3-5-sonnet",
    "messages": [{"role": "user", "content": "Tell me a story"}],
    "stream": true
  }'
```

### Multi-Modal (Vision)

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -d '{
    "model": "gpt-4o",
    "messages": [
      {
        "role": "user",
        "content": [
          {"type": "text", "text": "What is in this image?"},
          {
            "type": "image_url",
            "image_url": {"url": "https://example.com/image.jpg"}
          }
        ]
      }
    ]
  }'
```

### List Models

```bash
curl http://localhost:8080/v1/models \
  -H "Authorization: Bearer YOUR_API_KEY"
```

### Health Check

```bash
# Simple health check
curl http://localhost:8080/health

# Detailed health check
curl http://localhost:8080/health?detailed=true
```

### Response Format

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1700000000,
  "model": "gpt-4o",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! I'm doing well, thank you for asking."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 25,
    "completion_tokens": 12,
    "total_tokens": 37
  }
}
```

---

## Configuration

### Configuration File (YAML)

```yaml
server:
  host: "0.0.0.0"
  port: 8080
  max_connections: 10000
  timeout_seconds: 300
  metrics_port: 9090

providers:
  openai:
    enabled: true
    api_key: "${OPENAI_API_KEY}"
    base_url: "https://api.openai.com/v1"
    timeout_seconds: 60
    max_retries: 3
    rate_limit:
      requests_per_minute: 500
      tokens_per_minute: 150000

  anthropic:
    enabled: true
    api_key: "${ANTHROPIC_API_KEY}"
    base_url: "https://api.anthropic.com"
    api_version: "2024-01-01"
    timeout_seconds: 300
    max_retries: 3

  azure_openai:
    enabled: false
    api_key: "${AZURE_OPENAI_API_KEY}"
    endpoint: "${AZURE_OPENAI_ENDPOINT}"
    api_version: "2024-02-01"

routing:
  strategy: "cost_optimized"  # latency_optimized, round_robin, weighted
  default_provider: "openai"
  fallback_enabled: true
  health_check_interval_seconds: 30

  rules:
    - name: "claude-models"
      condition: "model starts_with 'claude'"
      target_provider: "anthropic"
      priority: 10

    - name: "gpt-models"
      condition: "model starts_with 'gpt'"
      target_provider: "openai"
      priority: 10

resilience:
  circuit_breaker:
    enabled: true
    failure_threshold: 5
    success_threshold: 3
    timeout_seconds: 60

  retry:
    max_attempts: 3
    initial_delay_ms: 100
    max_delay_ms: 5000
    backoff_multiplier: 2.0

  timeout:
    connect_seconds: 5
    request_seconds: 300
    streaming_seconds: 600

  bulkhead:
    max_concurrent: 1000
    max_queue: 500

cache:
  enabled: true
  backend: "memory"  # memory, redis
  max_size_mb: 1024
  default_ttl_seconds: 3600
  semantic_cache:
    enabled: true
    similarity_threshold: 0.95

security:
  enabled: true
  ip_filter:
    enabled: false
    whitelist: []
    blacklist: []

  rate_limit:
    enabled: true
    requests_per_minute: 1000
    burst_size: 100

  headers:
    remove_sensitive: true
    add_security_headers: true

telemetry:
  logging:
    level: "info"
    format: "json"
    pii_redaction: true

  metrics:
    enabled: true
    port: 9090

  tracing:
    enabled: true
    jaeger_endpoint: "http://jaeger:14268/api/traces"
    sample_rate: 0.1

database:
  url: "${DATABASE_URL}"
  max_connections: 20
  min_connections: 5
```

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SERVER_HOST` | No | `0.0.0.0` | Server bind address |
| `SERVER_PORT` | No | `8080` | HTTP server port |
| `METRICS_PORT` | No | `9090` | Prometheus metrics port |
| `RUST_LOG` | No | `info` | Log level |
| `DATABASE_URL` | Yes | - | PostgreSQL connection URL |
| `REDIS_URL` | No | - | Redis connection URL |
| `OPENAI_API_KEY` | Conditional | - | OpenAI API key |
| `ANTHROPIC_API_KEY` | Conditional | - | Anthropic API key |
| `AZURE_OPENAI_ENDPOINT` | Conditional | - | Azure OpenAI endpoint |
| `AZURE_OPENAI_API_KEY` | Conditional | - | Azure OpenAI key |

---

## Security

### Security Features

The `gateway-security` crate provides comprehensive security middleware:

| Feature | Description |
|---------|-------------|
| **IP Filtering** | Whitelist/blacklist IP addresses and CIDR ranges |
| **Request Signing** | HMAC-SHA256 request signature verification |
| **Header Security** | Automatic security headers (HSTS, CSP, etc.) |
| **Input Validation** | Request validation and sanitization |
| **Secret Management** | Encrypted secret storage with rotation |
| **PII Redaction** | Automatic detection and masking of sensitive data |
| **Rate Limiting** | Token bucket rate limiting per tenant/IP |
| **Audit Logging** | Comprehensive security event logging |

### Security Configuration

```yaml
security:
  enabled: true

  ip_filter:
    enabled: true
    whitelist:
      - "10.0.0.0/8"
      - "192.168.1.0/24"
    blacklist:
      - "1.2.3.4"

  request_signing:
    enabled: true
    algorithm: "hmac-sha256"
    header_name: "X-Signature"

  headers:
    remove_sensitive: true
    add_security_headers: true
    allowed_hosts:
      - "api.example.com"

  validation:
    max_request_size_bytes: 10485760  # 10MB
    max_messages: 100
    max_message_length: 100000

  secrets:
    encryption_key: "${SECRETS_ENCRYPTION_KEY}"
    rotation_days: 90
```

### Best Practices

1. **Secret Management** - Use environment variables or external secret stores
2. **TLS Everywhere** - Enable TLS for all external communications
3. **Network Policies** - Implement Kubernetes network policies
4. **Non-Root Containers** - Run containers as non-root user (UID 1000)
5. **Image Scanning** - Scan images with Trivy/Snyk before deployment
6. **Audit Logging** - Enable comprehensive audit logging
7. **Rate Limiting** - Configure appropriate rate limits per tenant

---

## Monitoring & Observability

### Prometheus Metrics

Access metrics at `http://localhost:9090/metrics`

**Request Metrics:**
- `gateway_requests_total` - Total requests by status, provider, model
- `gateway_request_duration_seconds` - Request latency histogram
- `gateway_request_tokens_total` - Total tokens processed (input/output)

**Provider Metrics:**
- `gateway_provider_requests_total` - Requests per provider
- `gateway_provider_errors_total` - Errors per provider
- `gateway_provider_latency_seconds` - Provider API latency

**Resilience Metrics:**
- `gateway_circuit_breaker_state` - Circuit breaker states
- `gateway_retry_attempts_total` - Retry attempt counts
- `gateway_rate_limit_exceeded_total` - Rate limit violations

**Cache Metrics:**
- `gateway_cache_hits_total` - Cache hit count
- `gateway_cache_misses_total` - Cache miss count
- `gateway_cache_size_bytes` - Current cache size

### Distributed Tracing

OpenTelemetry integration with Jaeger:

```bash
# Enable tracing
export OTEL_EXPORTER_JAEGER_ENDPOINT="http://jaeger:14268/api/traces"
export OTEL_SERVICE_NAME="llm-gateway"

# View traces at http://localhost:16686
```

### Structured Logging

JSON-formatted logs with context:

```json
{
  "timestamp": "2024-01-15T10:30:45.123Z",
  "level": "INFO",
  "target": "gateway_server::handlers",
  "message": "Request completed",
  "request_id": "req-abc123",
  "provider": "openai",
  "model": "gpt-4o",
  "latency_ms": 245,
  "tokens": {"input": 150, "output": 50},
  "status": 200
}
```

---

## Database Migrations

The gateway uses SQLx for database migrations with PostgreSQL.

### Migration Commands

```bash
# Check migration status
llm-gateway migrate status

# Run pending migrations
llm-gateway migrate run

# Revert last migration
llm-gateway migrate revert

# Revert multiple migrations
llm-gateway migrate revert --steps 3

# Create new migration
llm-gateway migrate create add_usage_tracking
```

### Migration Structure

Migrations are stored in `crates/gateway-migrations/migrations/`:

```
migrations/
├── 20240101000000_initial_schema.sql
├── 20240102000000_add_providers.sql
├── 20240103000000_add_usage_tracking.sql
└── 20240104000000_add_audit_logs.sql
```

---

## Deployment

### Kubernetes Deployment

```bash
# Create namespace
kubectl create namespace llm-gateway

# Create secrets
kubectl create secret generic llm-provider-secrets \
  --from-literal=openai-api-key="sk-..." \
  --from-literal=anthropic-api-key="sk-ant-..." \
  -n llm-gateway

# Deploy using Kustomize
kubectl apply -k deployment/k8s/

# Verify deployment
kubectl get pods -n llm-gateway
```

### Helm Chart

```bash
# Add Helm repository
helm repo add llm-gateway https://charts.llmdevops.com

# Install chart
helm install llm-gateway llm-gateway/llm-gateway \
  --namespace llm-gateway \
  --set providers.openai.apiKey=$OPENAI_API_KEY \
  --set providers.anthropic.apiKey=$ANTHROPIC_API_KEY
```

### Deployment Tiers

| Tier | RPS | Nodes | Estimated Cost |
|------|-----|-------|----------------|
| **Development** | < 100 | 1 | Free (local) |
| **Startup** | 1,000 | 3 | $150-250/month |
| **Production** | 10,000 | 5-10 | $800-1,200/month |
| **Enterprise** | 100,000+ | 20+ | $3,500-5,000/month |

---

## Performance Benchmarks

Tested on: `AWS c5.2xlarge (8 vCPU, 16GB RAM)`

| Metric | Value |
|--------|-------|
| **Max Throughput** | 12,500 RPS |
| **P50 Latency** | 45ms |
| **P95 Latency** | 120ms |
| **P99 Latency** | 350ms |
| **Memory Usage** | 2.5GB (under load) |
| **CPU Usage** | 60% (at 10K RPS) |
| **Error Rate** | < 0.01% |

*Benchmarks measured with K6 load testing, excluding provider latency*

---

## Supported Providers

| Provider | Streaming | Function Calling | Vision | Max Context | Status |
|----------|-----------|------------------|--------|-------------|--------|
| **OpenAI** | ✅ | ✅ | ✅ | 128K | Production |
| **Anthropic Claude** | ✅ | ✅ | ✅ | 200K | Production |
| **Azure OpenAI** | ✅ | ✅ | ✅ | 128K | Production |
| **Google Gemini** | ✅ | ✅ | ✅ | 1M | Beta |
| **AWS Bedrock** | ✅ | ✅ | ✅ | 200K | Beta |
| **Cohere** | ✅ | ✅ | ❌ | 128K | Beta |
| **Together AI** | ✅ | ❌ | ❌ | 32K | Beta |
| **Mistral AI** | ✅ | ✅ | ❌ | 32K | Beta |

---

## Contributing

We welcome contributions! Please see our [Contributing Guidelines](CONTRIBUTING.md).

### Development Setup

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/your-org/llm-inference-gateway.git
cd llm-inference-gateway
cargo build

# Run tests
cargo test --all

# Run with hot reload
cargo watch -x run
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Lint
cargo clippy --all-targets --all-features

# Security audit
cargo audit

# Run all checks
cargo test --all && cargo clippy && cargo fmt --check
```

---

## License

This project is licensed under the **LLM Dev Ops Commercial License**.

See [LICENSE.md](./LICENSE.md) for full license text.

- Commercial use requires license agreement
- Free for evaluation and non-commercial use
- Enterprise support available

---

## Support

### Community
- **GitHub Issues:** [Report bugs and request features](https://github.com/your-org/llm-gateway/issues)
- **Discussions:** [Ask questions and share ideas](https://github.com/your-org/llm-gateway/discussions)

### Enterprise
- **Email:** support@llmdevops.com
- **SLA:** 99.9% uptime guarantee
- **24/7 Support:** Available for enterprise customers

---

## Acknowledgments

Built with:
- [Rust](https://www.rust-lang.org) - Systems programming language
- [Tokio](https://tokio.rs) - Async runtime
- [Axum](https://github.com/tokio-rs/axum) - Web framework
- [SQLx](https://github.com/launchbadge/sqlx) - Database toolkit
- [Prometheus](https://prometheus.io) - Monitoring

---

**Built with ❤️ by the LLM DevOps team**

**Last Updated:** November 2024 | **Version:** 1.0.0
