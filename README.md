# LLM Inference Gateway

A high-performance, production-ready Rust-based gateway for unifying multiple Large Language Model (LLM) providers into a single, scalable API.

[![License](https://img.shields.io/badge/license-Commercial-blue.svg)](LICENSE.md)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Kubernetes](https://img.shields.io/badge/kubernetes-1.24%2B-blue.svg)](https://kubernetes.io)

---

## Features

### Core Capabilities

- **Multi-Provider Support** - Unified interface for OpenAI, Anthropic, Azure OpenAI, Google Gemini, AWS Bedrock, and more
- **High Performance** - Built in Rust for maximum throughput (10,000+ RPS per instance)
- **Streaming Support** - Server-Sent Events (SSE) for real-time token streaming
- **Intelligent Routing** - Cost-aware, latency-optimized provider selection
- **Advanced Caching** - Redis-backed response caching with TTL management
- **Rate Limiting** - Per-provider token bucket rate limiting
- **Health Monitoring** - Automatic provider health checks with circuit breakers
- **Observability** - Prometheus metrics, distributed tracing, structured logging

### Enterprise Features

- **High Availability** - Multi-region deployments with automatic failover
- **Horizontal Scaling** - Kubernetes-native with auto-scaling support
- **Security Hardening** - Network policies, secret encryption, non-root containers
- **Multi-Tenancy** - Namespace isolation and resource quotas
- **Cost Tracking** - Token usage analytics and cost attribution
- **GDPR Compliance** - Data residency controls and PII redaction

---

## Quick Start

### Prerequisites

- Rust 1.75+ (for local development)
- Docker (for containerized deployment)
- Kubernetes 1.24+ (for production deployment)
- Redis 7.0+ (for caching and rate limiting)

### Local Development

```bash
# Clone repository
git clone https://github.com/your-org/llm-inference-gateway.git
cd llm-inference-gateway

# Set environment variables
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
export REDIS_URL="redis://localhost:6379"

# Run with Docker Compose
docker-compose -f deployment/docker-compose.dev.yml up

# Or build and run locally
cargo build --release
cargo run --release
```

The gateway will be available at `http://localhost:8080`

### Docker Deployment

```bash
# Build image
docker build -t llm-gateway:latest -f deployment/docker/Dockerfile .

# Run container
docker run -d \
  -p 8080:8080 \
  -p 9090:9090 \
  -e OPENAI_API_KEY="sk-..." \
  -e ANTHROPIC_API_KEY="sk-ant-..." \
  -e REDIS_URL="redis://redis:6379" \
  llm-gateway:latest
```

### Kubernetes Deployment

```bash
# Create namespace
kubectl create namespace llm-gateway

# Create secrets
kubectl create secret generic llm-provider-secrets \
  --from-literal=openai-api-key="sk-..." \
  --from-literal=anthropic-api-key="sk-ant-..." \
  -n llm-gateway

# Deploy
kubectl apply -k deployment/k8s/

# Verify
kubectl get pods -n llm-gateway
```

---

## API Usage

### Basic Chat Completion

```bash
curl -X POST https://api.llmgateway.example.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -d '{
    "model": "gpt-4",
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
curl -X POST https://api.llmgateway.example.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -d '{
    "model": "claude-3-sonnet",
    "messages": [{"role": "user", "content": "Tell me a story"}],
    "stream": true
  }'
```

### Multi-Modal (Vision)

```bash
curl -X POST https://api.llmgateway.example.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -d '{
    "model": "gpt-4-vision",
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

### Health Check

```bash
curl https://api.llmgateway.example.com/health
```

Response:
```json
{
  "status": "healthy",
  "version": "1.0.0",
  "uptime_seconds": 86400,
  "providers": [
    {"name": "openai", "healthy": true, "latency_ms": 150},
    {"name": "anthropic", "healthy": true, "latency_ms": 200}
  ],
  "dependencies": {
    "redis": true,
    "disk_space_gb": 45.2
  }
}
```

---

## Architecture

The LLM Inference Gateway is built with a modular, layered architecture optimized for performance and scalability:

```
┌─────────────────────────────────────────────────────────┐
│                     API Layer                           │
│  REST API │ WebSocket │ gRPC (planned)                  │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                  Request Handler                         │
│  Authentication │ Validation │ Rate Limiting             │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                 Provider Abstraction                     │
│  Unified Interface │ Request Transform │ Retry Logic    │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│              Provider Implementations                    │
│  OpenAI │ Anthropic │ Azure │ Google │ AWS Bedrock      │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                Infrastructure Layer                      │
│  Redis │ Connection Pool │ Metrics │ Tracing            │
└─────────────────────────────────────────────────────────┘
```

**Key Design Principles:**
- **Provider Agnostic** - Unified API regardless of underlying provider
- **Resilient** - Automatic retries, circuit breakers, fallback providers
- **Observable** - Comprehensive metrics, logging, and tracing
- **Performant** - Connection pooling, async I/O, zero-copy operations
- **Secure** - TLS everywhere, secret encryption, network isolation

---

## Supported Providers

| Provider | Streaming | Function Calling | Vision | Max Context | Status |
|----------|-----------|------------------|--------|-------------|--------|
| **OpenAI** | ✅ | ✅ | ✅ | 128K | Production |
| **Anthropic Claude** | ✅ | ✅ | ✅ | 200K | Production |
| **Azure OpenAI** | ✅ | ✅ | ✅ | 128K | Production |
| **Google Gemini** | ✅ | ✅ | ✅ | 32K | Production |
| **AWS Bedrock** | ✅ | ✅ | ✅ | 200K | Beta |
| **Together AI** | ✅ | ❌ | ❌ | 8K | Beta |
| **vLLM (Self-hosted)** | ✅ | ❌ | ❌ | Model-dependent | Beta |
| **Ollama (Local)** | ✅ | ❌ | ✅ | Model-dependent | Beta |

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
| **Error Rate** | <0.01% |

*Benchmarks measured with K6 load testing tool, excluding provider latency*

---

## Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SERVER_HOST` | No | `0.0.0.0` | Server bind address |
| `SERVER_PORT` | No | `8080` | HTTP server port |
| `METRICS_PORT` | No | `9090` | Prometheus metrics port |
| `RUST_LOG` | No | `info` | Log level (trace, debug, info, warn, error) |
| `REDIS_URL` | Yes | - | Redis connection URL (redis://host:port) |
| `OPENAI_API_KEY` | Conditional | - | OpenAI API key |
| `ANTHROPIC_API_KEY` | Conditional | - | Anthropic API key |
| `AZURE_OPENAI_ENDPOINT` | Conditional | - | Azure OpenAI endpoint URL |
| `AZURE_OPENAI_API_KEY` | Conditional | - | Azure OpenAI API key |
| `GOOGLE_API_KEY` | Conditional | - | Google Gemini API key |

### Configuration File

Create `config.yaml`:

```yaml
server:
  host: "0.0.0.0"
  port: 8080
  max_connections: 1000
  timeout_seconds: 300

providers:
  openai:
    enabled: true
    base_url: "https://api.openai.com"
    timeout_seconds: 60
    max_retries: 3
    rate_limit:
      requests_per_minute: 500
      tokens_per_minute: 150000

  anthropic:
    enabled: true
    base_url: "https://api.anthropic.com"
    api_version: "2023-06-01"
    timeout_seconds: 300
    max_retries: 3

cache:
  enabled: true
  ttl_seconds: 3600
  max_size_mb: 1024

logging:
  level: "info"
  format: "json"
  output: "stdout"
```

---

## Monitoring & Observability

### Prometheus Metrics

Access metrics at `http://localhost:9090/metrics`

**Key Metrics:**
- `http_requests_total` - Total HTTP requests by status code
- `http_request_duration_seconds` - Request latency histogram
- `llm_provider_requests_total` - Requests per provider
- `llm_provider_errors_total` - Errors per provider
- `llm_provider_latency_seconds` - Provider API latency
- `llm_rate_limit_exceeded_total` - Rate limit violations
- `redis_operations_total` - Redis operation counts
- `cache_hit_ratio` - Cache hit/miss ratio

### Grafana Dashboards

Pre-built dashboards in `/deployment/monitoring/grafana/`:
- Gateway Overview
- Provider Performance
- Infrastructure Metrics
- Cost Analytics

### Distributed Tracing

Jaeger integration for request tracing:
```bash
# Enable tracing
export JAEGER_AGENT_HOST=localhost
export JAEGER_AGENT_PORT=6831
```

View traces at `http://localhost:16686`

---

## Security

### Best Practices

1. **Secret Management**
   - Use external secret stores (AWS Secrets Manager, Vault)
   - Rotate API keys every 90 days
   - Never commit secrets to version control

2. **Network Security**
   - Enable TLS for all external traffic
   - Use network policies in Kubernetes
   - Restrict egress to known LLM provider IPs

3. **Container Security**
   - Run as non-root user (UID 1000)
   - Use distroless base images
   - Scan images with Trivy/Snyk
   - Enable read-only root filesystem

4. **Authentication**
   - Use API keys with rate limits
   - Implement JWT tokens for user authentication
   - Enable audit logging for all requests

### Security Scanning

```bash
# Scan container image
trivy image llm-gateway:latest

# Scan dependencies
cargo audit

# SAST scanning
semgrep --config=auto .
```

---

## Deployment Options

### Development
- Docker Compose
- Local Rust binary
- **Cost:** Free
- **Effort:** 5 minutes

### Small/Startup (1K RPS)
- Kubernetes (3 nodes)
- Managed Redis
- **Cost:** $150-250/month
- **Effort:** 30 minutes

### Production (10K RPS)
- Multi-AZ Kubernetes
- Redis Cluster
- Full monitoring
- **Cost:** $800-1,200/month
- **Effort:** 2-4 hours

### Enterprise (100K+ RPS)
- Multi-region deployment
- Global load balancing
- Advanced monitoring
- **Cost:** $3,500-5,000/month
- **Effort:** 1-2 days

See [DEPLOYMENT.md](./DEPLOYMENT.md) for detailed deployment guides.

---

## Documentation

| Document | Description |
|----------|-------------|
| [DEPLOYMENT.md](./DEPLOYMENT.md) | Complete deployment and infrastructure guide |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | System architecture and design decisions |
| [API-DESIGN-AND-VERSIONING.md](./API-DESIGN-AND-VERSIONING.md) | API specification and versioning strategy |
| [INFRASTRUCTURE-OVERVIEW.md](./INFRASTRUCTURE-OVERVIEW.md) | Infrastructure quick reference |
| [/plans](./plans) | Rust implementation plans and provider integrations |

---

## Troubleshooting

### Common Issues

**Issue: Container fails to start**
```bash
# Check logs
docker logs llm-gateway

# Common causes:
# - Missing environment variables
# - Invalid Redis URL
# - Port already in use
```

**Issue: High latency**
```bash
# Check provider latency
curl localhost:9090/metrics | grep provider_latency

# Solutions:
# - Scale up pods
# - Enable caching
# - Switch to faster provider
```

**Issue: Rate limiting errors**
```bash
# Check rate limit metrics
curl localhost:9090/metrics | grep rate_limit_exceeded

# Solutions:
# - Increase provider rate limits
# - Add more provider accounts
# - Implement request queuing
```

See [DEPLOYMENT.md](./DEPLOYMENT.md#troubleshooting-guide) for complete troubleshooting guide.

---

## Roadmap

### Version 1.1 (Q1 2025)
- [ ] gRPC API support
- [ ] Enhanced prompt caching
- [ ] Cost optimization algorithms
- [ ] Multi-model routing strategies

### Version 1.2 (Q2 2025)
- [ ] WebSocket support for bidirectional streaming
- [ ] Advanced analytics dashboard
- [ ] Plugin system for custom providers
- [ ] A/B testing framework

### Version 2.0 (Q3 2025)
- [ ] Edge deployment support
- [ ] Vector database integration
- [ ] RAG (Retrieval-Augmented Generation)
- [ ] Fine-tuning pipeline integration

---

## Contributing

We welcome contributions! Please see our [Contributing Guidelines](CONTRIBUTING.md) for details.

### Development Setup

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone repository
git clone https://github.com/your-org/llm-inference-gateway.git
cd llm-inference-gateway

# Install dependencies
cargo build

# Run tests
cargo test

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
./scripts/pre-commit.sh
```

---

## License

This project is licensed under the **LLM Dev Ops Commercial License**.

See [LICENSE.md](./LICENSE.md) for full license text.

**Key Points:**
- Commercial use requires license agreement
- Free for evaluation and non-commercial use
- Enterprise support available
- Contact: licensing@llmdevops.com

---

## Support

### Community Support
- **GitHub Issues:** [github.com/your-org/llm-gateway/issues](https://github.com/your-org/llm-gateway/issues)
- **Discussions:** [github.com/your-org/llm-gateway/discussions](https://github.com/your-org/llm-gateway/discussions)
- **Discord:** [discord.gg/llm-gateway](https://discord.gg/llm-gateway)

### Enterprise Support
- **Email:** support@llmdevops.com
- **Slack Connect:** Available for enterprise customers
- **24/7 On-Call:** PagerDuty integration
- **SLA:** 99.9% uptime guarantee

---

## Acknowledgments

Built with:
- [Rust](https://www.rust-lang.org) - Systems programming language
- [Tokio](https://tokio.rs) - Async runtime
- [Axum](https://github.com/tokio-rs/axum) - Web framework
- [Prometheus](https://prometheus.io) - Monitoring
- [Redis](https://redis.io) - Caching and rate limiting

Inspired by API gateway patterns from:
- Kong Gateway
- Envoy Proxy
- OpenAI API

---

## Contact

- **Website:** [llmdevops.com](https://llmdevops.com)
- **Email:** hello@llmdevops.com
- **Twitter:** [@llmdevops](https://twitter.com/llmdevops)
- **LinkedIn:** [company/llm-devops](https://linkedin.com/company/llm-devops)

---

**Built with ❤️ by the LLM DevOps team**

**Last Updated:** November 2024 | **Version:** 1.0.0
