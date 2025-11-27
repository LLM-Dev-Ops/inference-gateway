# LLM Inference Gateway - Architecture Documentation

This directory contains comprehensive architecture documentation and production-ready pseudocode for the LLM Inference Gateway project.

## Documentation Index

### 📋 Specifications & Overview
- **[LLM-Inference-Gateway-Specification.md](./LLM-Inference-Gateway-Specification.md)** - Complete project specification including requirements, architecture, and implementation plan

### 🏗️ Core Architecture

#### Provider Abstraction Layer
- **[PROVIDER_ARCHITECTURE_SUMMARY.md](./PROVIDER_ARCHITECTURE_SUMMARY.md)** - Complete architecture overview of the provider system
- **[PROVIDER_QUICKSTART.md](./PROVIDER_QUICKSTART.md)** - Practical quick start guide with code examples
- **[provider-abstraction-layer.rs](./provider-abstraction-layer.rs)** - Core provider trait, registry, and base implementations (OpenAI, Anthropic)
- **[provider-advanced-features.rs](./provider-advanced-features.rs)** - Advanced features: circuit breakers, load balancing, caching, observability
- **[provider-implementations.rs](./provider-implementations.rs)** - Additional provider implementations (Google, vLLM, Ollama, Bedrock, Azure, Together)

#### Routing & Load Balancing
- **[routing_load_balancing_pseudocode.md](./routing_load_balancing_pseudocode.md)** - Detailed routing engine and load balancing strategies

#### Resilience & Reliability
- **[circuit-breaker-resilience-pseudocode.md](./circuit-breaker-resilience-pseudocode.md)** - Circuit breaker patterns and resilience mechanisms

#### Data Structures
- **[core-data-structures-pseudocode.md](./core-data-structures-pseudocode.md)** - Core data structures, request/response formats, and type definitions

## Quick Navigation

### By Use Case

#### "I want to understand the overall architecture"
→ Start with [PROVIDER_ARCHITECTURE_SUMMARY.md](./PROVIDER_ARCHITECTURE_SUMMARY.md)

#### "I want to implement a provider"
→ Read [PROVIDER_QUICKSTART.md](./PROVIDER_QUICKSTART.md), then see [provider-abstraction-layer.rs](./provider-abstraction-layer.rs)

#### "I need to add resilience features"
→ See [provider-advanced-features.rs](./provider-advanced-features.rs) and [circuit-breaker-resilience-pseudocode.md](./circuit-breaker-resilience-pseudocode.md)

#### "I want to understand routing logic"
→ Read [routing_load_balancing_pseudocode.md](./routing_load_balancing_pseudocode.md)

#### "I need the data structure definitions"
→ Check [core-data-structures-pseudocode.md](./core-data-structures-pseudocode.md)

### By Component

#### Provider System
```
provider-abstraction-layer.rs
├── Core Traits (LLMProvider, LoadBalancer)
├── Provider Registry
├── Connection Pool Management
├── Rate Limiting
└── Base Implementations (OpenAI, Anthropic)

provider-advanced-features.rs
├── Circuit Breaker
├── Load Balancing Strategies
├── Response Caching
├── Observability (Metrics, Tracing)
└── Provider Stack Builder

provider-implementations.rs
├── Google Gemini
├── vLLM
├── Ollama
├── AWS Bedrock
├── Azure OpenAI
└── Together AI
```

#### Routing Engine
```
routing_load_balancing_pseudocode.md
├── Routing Rules Engine
├── Cost-Based Routing
├── Latency-Based Routing
├── Capability-Based Selection
└── Fallback Strategies
```

#### Resilience Layer
```
circuit-breaker-resilience-pseudocode.md
├── Circuit Breaker Pattern
├── Retry Strategies
├── Timeout Management
├── Bulkhead Pattern
└── Health Checking
```

## Architecture Highlights

### Supported Providers (8+)
- ✅ OpenAI (GPT-4, GPT-3.5)
- ✅ Anthropic (Claude 3 Opus/Sonnet/Haiku)
- ✅ Google Gemini
- ✅ vLLM (Self-hosted)
- ✅ Ollama (Local)
- ✅ AWS Bedrock
- ✅ Azure OpenAI
- ✅ Together AI

### Key Features

#### Performance
- **Zero-copy I/O** with Bytes
- **HTTP/2 multiplexing** for connection efficiency
- **Connection pooling** with per-provider limits
- **Response caching** with LRU eviction
- **Async/await** throughout using Tokio

#### Resilience
- **Circuit breakers** prevent cascade failures
- **Automatic retries** with exponential backoff
- **Rate limiting** per provider
- **Health monitoring** with background checks
- **Fallback chains** for high availability

#### Observability
- **Prometheus metrics** for monitoring
- **OpenTelemetry tracing** for distributed traces
- **Structured logging** with tracing crate
- **Request/response logging** (with PII filtering)

#### Load Balancing
- **Round Robin** - Simple rotation
- **Latency-Weighted** - Route to fastest
- **Least Connections** - Distribute evenly
- **Cost-Based** - Optimize for price
- **Custom strategies** - Extensible

### Design Principles

1. **Unified Interface**: Single API for all providers
2. **Type Safety**: Rust's type system prevents errors
3. **Async First**: Non-blocking I/O throughout
4. **Composable**: Stack middleware-like components
5. **Observable**: Metrics and tracing built-in
6. **Resilient**: Failures are expected and handled
7. **Extensible**: Easy to add new providers

## Implementation Checklist

### Phase 1: Core Provider System
- [ ] Implement core traits and error types
- [ ] Build connection pool manager
- [ ] Create provider registry
- [ ] Implement OpenAI provider
- [ ] Implement Anthropic provider
- [ ] Add rate limiting
- [ ] Add health checks

### Phase 2: Advanced Features
- [ ] Circuit breaker implementation
- [ ] Load balancing strategies
- [ ] Response caching
- [ ] Prometheus metrics
- [ ] OpenTelemetry tracing
- [ ] Provider stack builder

### Phase 3: Additional Providers
- [ ] Google Gemini provider
- [ ] vLLM provider
- [ ] Ollama provider
- [ ] AWS Bedrock provider
- [ ] Azure OpenAI provider
- [ ] Together AI provider

### Phase 4: Routing Engine
- [ ] Routing rules engine
- [ ] Cost-based routing
- [ ] Latency-based routing
- [ ] Capability matching
- [ ] Fallback logic

### Phase 5: Testing & Production
- [ ] Unit tests for all components
- [ ] Integration tests with mock servers
- [ ] Load testing
- [ ] Security audit
- [ ] Documentation
- [ ] Production deployment

## Code Examples

### Basic Provider Usage
```rust
// Create and register provider
let provider = factory.create_openai(config);
registry.register(provider).await?;

// Make request
let response = provider.chat_completion(&request).await?;
```

### Production Stack
```rust
// Build production-ready provider
let provider = ProviderStackBuilder::new(base)
    .with_tracing()
    .with_circuit_breaker()
    .with_cache(cache)
    .with_metrics(metrics)
    .build();
```

### Load Balancing
```rust
// Create load-balanced pool
let balancer = Arc::new(LatencyWeightedBalancer::new(100));
let pool = LoadBalancedProvider::new(balancer);
pool.add_provider(openai).await;
pool.add_provider(anthropic).await;

// Automatically routes to best provider
let response = pool.chat_completion(&request).await?;
```

### Streaming
```rust
let mut stream = provider.chat_completion_stream(&request).await?;
while let Some(chunk) = stream.next().await {
    print!("{}", chunk?.delta.content.unwrap_or_default());
}
```

## Performance Benchmarks (Expected)

Based on the architecture design:

| Metric | Target |
|--------|--------|
| Request Latency (overhead) | < 5ms |
| Throughput | > 10,000 req/sec |
| Connection Pool Efficiency | > 95% reuse |
| Cache Hit Rate | > 80% (for repeated queries) |
| Memory Usage | < 100MB baseline |
| Circuit Breaker Response | < 1ms |

## Dependencies Overview

### Core Runtime
- `tokio` - Async runtime
- `hyper` - HTTP client/server
- `hyper-tls` - TLS support

### Data & Serialization
- `serde` - Serialization framework
- `serde_json` - JSON support
- `bytes` - Zero-copy byte buffers

### Observability
- `tracing` - Structured logging
- `opentelemetry` - Distributed tracing
- `prometheus` - Metrics

### Error Handling
- `thiserror` - Error derive macros
- `anyhow` - Error context

### Cloud SDKs
- `aws-sdk-bedrockruntime` - AWS Bedrock

## Contributing

When adding new components:

1. Follow existing patterns in pseudocode files
2. Maintain async/await throughout
3. Add comprehensive error handling
4. Include metrics and tracing
5. Write tests for new features
6. Update documentation

## Architecture Diagrams

### Request Flow
```
Client Request
    ↓
Gateway Entry Point
    ↓
Request Validation
    ↓
Routing Engine ←→ Provider Registry
    ↓              ↓
[Selected Provider with Stack]
    ├── Tracing Layer
    ├── Circuit Breaker
    ├── Cache Layer
    ├── Metrics Layer
    └── Base Provider
        ↓
Provider API (OpenAI/Anthropic/etc)
    ↓
Response Transform
    ↓
Client Response
```

### Provider Stack Composition
```
Request → Tracing → Circuit Breaker → Cache → Metrics → Provider → API
                                                               ↓
Response ← Tracing ← Circuit Breaker ← Cache ← Metrics ← Provider ← API
```

## Additional Resources

### External Documentation
- [OpenAI API Reference](https://platform.openai.com/docs/api-reference)
- [Anthropic API Reference](https://docs.anthropic.com/claude/reference)
- [Google Gemini API](https://ai.google.dev/docs)
- [AWS Bedrock Guide](https://docs.aws.amazon.com/bedrock/)
- [Azure OpenAI Service](https://learn.microsoft.com/en-us/azure/ai-services/openai/)

### Rust Resources
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [async-trait Crate](https://docs.rs/async-trait)
- [Hyper Documentation](https://docs.rs/hyper)

## License

See LICENSE.md in project root.

## Questions?

For questions about the architecture or implementation:
1. Review the relevant documentation file
2. Check the quick start guide
3. Examine the pseudocode examples
4. Open an issue with specific questions

---

**Last Updated**: 2024-11-27

**Status**: Architecture Design Complete - Ready for Implementation

**Next Steps**: Begin Phase 1 implementation starting with core traits and provider registry
