# Provider Abstraction Layer - Architecture Summary

## Overview

This document provides a comprehensive overview of the Provider Abstraction Layer (PAL) design for the LLM Inference Gateway. The architecture supports multiple LLM providers with unified interfaces, advanced features like circuit breakers, load balancing, caching, and comprehensive observability.

## File Structure

```
plans/
├── provider-abstraction-layer.rs      # Core provider trait and implementations
├── provider-advanced-features.rs      # Circuit breakers, load balancing, caching
├── provider-implementations.rs        # Additional provider implementations
└── PROVIDER_ARCHITECTURE_SUMMARY.md   # This file
```

## Core Architecture Components

### 1. Provider Trait System (`provider-abstraction-layer.rs`)

#### Key Traits
- **`LLMProvider`**: Core trait that all providers implement
  - Non-streaming: `chat_completion()`
  - Streaming: `chat_completion_stream()`
  - Health: `health_check()`
  - Capabilities: `capabilities()`
  - Transformations: `transform_request()`, `transform_response()`

#### Unified Request/Response Types
- **`GatewayRequest`**: Provider-agnostic request format
- **`GatewayResponse`**: Unified response format
- **`ChatChunk`**: Streaming chunk format
- Support for:
  - Text and multimodal content
  - Tool/function calling
  - System prompts
  - Streaming control

#### Provider Registry
- Thread-safe provider storage using `Arc<RwLock<HashMap>>`
- Dynamic provider registration/deregistration
- Background health check monitoring
- Capability-based provider lookup

#### Connection Pool Management
- HTTP/2 multiplexing with `hyper`
- TLS session resumption
- Per-provider connection limits using `Semaphore`
- Automatic connection cleanup
- Configurable timeouts and keep-alive

#### Rate Limiting
- Token bucket algorithm
- Separate limits for requests and tokens
- Per-provider rate tracking
- Automatic backoff calculation

### 2. Advanced Features (`provider-advanced-features.rs`)

#### Circuit Breaker Pattern
- States: Closed, Open, HalfOpen
- Configurable failure/success thresholds
- Automatic recovery testing
- Provider-level fault isolation

**Usage:**
```rust
let provider = CircuitBreakerProvider::new(base_provider);
// Automatically handles failures and prevents cascade
```

#### Load Balancing Strategies
Three built-in strategies:

1. **Round Robin**: Simple rotation through providers
2. **Latency-Weighted**: Routes to fastest providers
3. **Least Connections**: Distributes based on active connections

**Usage:**
```rust
let balancer = Arc::new(LatencyWeightedBalancer::new(100));
let pool = LoadBalancedProvider::new(balancer);
pool.add_provider(openai).await;
pool.add_provider(anthropic).await;
```

#### Response Caching
- LRU cache with TTL
- Content-based cache keys
- Automatic eviction
- Streaming requests bypass cache

**Cache Key Factors:**
- Model
- Messages (hashed)
- Temperature
- Max tokens

#### Observability
- Prometheus metrics integration
- Request/response tracing with OpenTelemetry
- Per-provider metrics:
  - Request counts (total, success, failure)
  - Latency histograms
  - Token usage
  - Active connections
  - Circuit breaker state
  - Cache hit/miss ratios

#### Fallback Provider
- Primary provider with ordered fallbacks
- Automatic failover on errors
- Transparent to callers

### 3. Provider Implementations

#### Fully Implemented (with pseudocode)

##### OpenAI Provider
- **Endpoint**: `https://api.openai.com/v1/chat/completions`
- **Auth**: Bearer token
- **Features**:
  - Streaming via SSE
  - Function calling
  - Vision (multimodal)
  - System messages
- **Rate Limits**: 500 RPM, 150K TPM

##### Anthropic Provider
- **Endpoint**: `https://api.anthropic.com/v1/messages`
- **Auth**: x-api-key header
- **Features**:
  - Streaming via SSE
  - Tool use
  - Vision (base64 only)
  - System prompt (separate field)
- **Rate Limits**: 1000 RPM, 400K TPM
- **Unique Aspects**:
  - System messages in separate field
  - Base64-only images
  - Different response format

#### Partially Implemented (stubs with key logic)

##### Google Gemini Provider
- **Endpoint**: `https://generativelanguage.googleapis.com/v1beta/models`
- **Auth**: API key in URL
- **Format**: Gemini-specific JSON
- **Features**: Vision, streaming, safety filters

##### vLLM Provider
- **Endpoint**: Configurable (local deployment)
- **Format**: OpenAI-compatible
- **Features**: No rate limits, model-dependent capabilities

##### Ollama Provider
- **Endpoint**: `http://localhost:11434/api/chat`
- **Auth**: None (local)
- **Features**: Custom format, dynamic model list

##### AWS Bedrock Provider
- **SDK**: AWS SDK for Rust
- **Auth**: AWS credentials
- **Format**: Model-specific (e.g., Claude on Bedrock)

##### Azure OpenAI Provider
- **Endpoint**: `https://{resource}.openai.azure.com`
- **Auth**: api-key header
- **Format**: OpenAI-compatible with deployment names

##### Together AI Provider
- **Endpoint**: `https://api.together.xyz/v1/chat/completions`
- **Auth**: Bearer token
- **Format**: OpenAI-compatible

## Provider Stack Builder

Compose providers with middleware-like pattern:

```rust
let production_provider = ProviderStackBuilder::new(base_provider)
    .with_tracing()          // Add distributed tracing
    .with_circuit_breaker()  // Add fault tolerance
    .with_cache(cache)       // Add response caching
    .with_metrics(metrics)   // Add Prometheus metrics
    .build();
```

**Execution Order** (outer to inner):
1. Tracing (logs request start)
2. Circuit Breaker (checks state)
3. Cache (checks for cached response)
4. Metrics (records request)
5. Base Provider (makes API call)

## Request Flow

### Non-Streaming Request

```
User Request
    ↓
Gateway Request (unified format)
    ↓
Provider Selection (registry or load balancer)
    ↓
Rate Limit Check
    ↓
Circuit Breaker Check
    ↓
Cache Lookup
    ↓ (on miss)
Request Transformation (to provider format)
    ↓
HTTP Request (via connection pool)
    ↓
Response Transformation (to unified format)
    ↓
Cache Store
    ↓
Metrics Recording
    ↓
Gateway Response
```

### Streaming Request

```
User Request
    ↓
Gateway Request (stream=true)
    ↓
Provider Selection
    ↓
Rate Limit Check
    ↓
Circuit Breaker Check
    ↓
Request Transformation
    ↓
HTTP Streaming Request
    ↓
SSE Stream Processing
    ↓
Chunk Transformation (to unified format)
    ↓
Streaming Response (async iterator)
```

## Error Handling

### Error Types
- `NotFound`: Provider not registered
- `RateLimitExceeded`: Rate limit hit
- `AuthenticationFailed`: Invalid credentials
- `InvalidRequest`: Malformed request
- `Timeout`: Request timeout
- `NetworkError`: Connection issues
- `ProviderInternalError`: Provider-side error
- `SerializationError`: JSON parsing error
- `StreamError`: Streaming failure
- `UnsupportedCapability`: Feature not supported

### Retry Strategy
- Exponential backoff
- Configurable max retries (default: 3)
- Retryable errors:
  - Timeout
  - Network errors
  - Rate limit (with backoff)
- Non-retryable:
  - Authentication failures
  - Invalid requests

## Configuration Examples

### OpenAI Configuration
```rust
OpenAIConfig {
    api_key: "sk-...".to_string(),
    base_url: "https://api.openai.com".to_string(),
    organization: Some("org-...".to_string()),
    timeout: Duration::from_secs(60),
    retry_config: RetryConfig {
        max_retries: 3,
        initial_backoff: Duration::from_millis(100),
        max_backoff: Duration::from_secs(10),
        backoff_multiplier: 2.0,
    },
}
```

### Connection Pool Configuration
```rust
ConnectionPoolConfig {
    max_idle_per_host: 32,
    idle_timeout: Duration::from_secs(90),
    connect_timeout: Duration::from_secs(10),
    max_connections_per_provider: 100,
    keep_alive: Duration::from_secs(60),
    http2_only: true,
    tcp_nodelay: true,
}
```

### Circuit Breaker Configuration
```rust
CircuitBreakerProvider::with_config(
    provider,
    5,  // failure_threshold
    2,  // success_threshold
    Duration::from_secs(30), // timeout
)
```

## Performance Considerations

### Zero-Copy Optimizations
- Use `Bytes` for request/response bodies
- Avoid unnecessary allocations
- Stream processing without buffering full responses

### Async/Await Best Practices
- All I/O is non-blocking using Tokio
- Connection pooling prevents connection overhead
- Parallel health checks using `join_all`

### Memory Management
- Arc for shared ownership
- RwLock for concurrent read access
- Semaphore for connection limiting
- Bounded caches with LRU eviction

### Latency Optimizations
- HTTP/2 multiplexing
- TLS session resumption
- Connection keep-alive
- Response caching
- Request pipelining

## Security Considerations

### API Key Management
- Keys stored in config (should use secrets manager in production)
- Keys never logged or exposed in errors
- Per-provider authentication

### Request Validation
- Input validation before transformation
- Model capability checks
- Parameter range validation
- Content filtering hooks

### Rate Limiting
- Prevents API quota exhaustion
- Per-provider limits
- Prevents cost overruns

## Testing Strategy

### Unit Tests
- Request/response transformation
- Rate limiter logic
- Circuit breaker state transitions
- Cache key generation

### Integration Tests
- Mock HTTP servers for each provider
- End-to-end request flow
- Error handling scenarios
- Streaming tests

### Load Tests
- Connection pool under load
- Rate limiter accuracy
- Circuit breaker behavior
- Memory leak detection

## Future Enhancements

### Planned Features
1. **Smart Model Selection**: Automatic model routing based on request complexity
2. **Cost Optimization**: Route to cheapest provider meeting requirements
3. **Adaptive Rate Limiting**: Learn optimal rates from provider headers
4. **Request Batching**: Combine multiple requests for efficiency
5. **Response Validation**: Verify response quality/format
6. **A/B Testing**: Route percentage of traffic to different providers
7. **Provider Plugins**: Dynamic provider loading via WASM or dynamic libraries

### Provider Additions
- Cohere
- AI21 Labs
- Hugging Face Inference API
- Replicate
- RunPod

## Dependencies

### Core Dependencies
```toml
[dependencies]
tokio = { version = "1.35", features = ["full"] }
hyper = { version = "0.14", features = ["full"] }
hyper-tls = "0.5"
async-trait = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bytes = "1.5"
futures = "0.3"
thiserror = "1.0"

# Observability
tracing = "0.1"
opentelemetry = "0.21"
prometheus = "0.13"

# AWS (for Bedrock)
aws-config = "1.0"
aws-sdk-bedrockruntime = "1.0"
```

## Production Deployment Checklist

- [ ] Configure secrets management (vault, AWS Secrets Manager)
- [ ] Set up Prometheus metrics endpoint
- [ ] Configure OpenTelemetry collector
- [ ] Set appropriate rate limits per provider
- [ ] Configure circuit breaker thresholds
- [ ] Set up health check monitoring
- [ ] Configure connection pool sizes based on load
- [ ] Enable request/response logging (with PII filtering)
- [ ] Set up alerting for circuit breaker trips
- [ ] Configure cache TTL and size
- [ ] Test failover scenarios
- [ ] Load test at expected peak traffic
- [ ] Set up cost monitoring per provider
- [ ] Configure timeout values appropriately

## Summary

This Provider Abstraction Layer provides:

✅ **Unified Interface**: Single API for 8+ providers
✅ **Production Ready**: Circuit breakers, retries, timeouts
✅ **Observable**: Metrics, tracing, logging
✅ **Performant**: Connection pooling, caching, zero-copy
✅ **Resilient**: Health checks, fallbacks, rate limiting
✅ **Extensible**: Easy to add new providers
✅ **Type Safe**: Rust's type system prevents runtime errors

The architecture balances flexibility with performance, providing a robust foundation for building a high-performance LLM inference gateway.
