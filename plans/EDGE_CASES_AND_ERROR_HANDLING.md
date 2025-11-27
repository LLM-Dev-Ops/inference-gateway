# Edge Cases and Error Handling Documentation
## LLM Inference Gateway - QA Architecture Reference

**Version:** 1.0
**Last Updated:** 2025-11-27
**Status:** Production Reference

---

## 1. Edge Cases Catalog

### 1.1 Request Handling Edge Cases

#### Empty and Minimal Input Cases
| Edge Case | Detection | Handling Strategy | Test Coverage |
|-----------|-----------|-------------------|---------------|
| **Empty messages array** | `messages.is_empty()` | Return `InvalidRequest` error immediately | REQUIRED |
| **Single character prompt** | `text.len() == 1` | Process normally, warn if unusual token count | OPTIONAL |
| **Whitespace-only messages** | `text.trim().is_empty()` | Reject or trim based on provider capability | REQUIRED |
| **Null vs missing fields** | JSON deserialization | Distinguish via `Option<T>` - null=Some(null), missing=None | CRITICAL |
| **Empty system prompt** | `system.as_ref().map(String::is_empty)` | Skip system message injection for providers | REQUIRED |

#### Token Limit Cases
| Edge Case | Detection | Handling Strategy | Test Coverage |
|-----------|-----------|-------------------|---------------|
| **Maximum tokens exceeded** | Compare against `ProviderCapabilities.max_context_tokens` | Return early `InvalidRequest` with token count | CRITICAL |
| **Extremely long prompts (>100K)** | Token estimation pre-flight | Chunk or reject based on `max_context_tokens` | REQUIRED |
| **max_tokens=0 or negative** | Validation in `validate_request()` | Reject with clear error message | REQUIRED |
| **Token overflow (u32::MAX)** | Check sum of prompt + completion tokens | Return error before sending to provider | REQUIRED |
| **Missing max_tokens for required providers** | Provider-specific validation | Use provider default or return error | OPTIONAL |

#### Character Encoding Edge Cases
| Edge Case | Detection | Handling Strategy | Test Coverage |
|-----------|-----------|-------------------|---------------|
| **Invalid UTF-8 sequences** | `String::from_utf8()` returns `Err` | Return `SerializationError`, log binary data | CRITICAL |
| **Emoji in prompts** | Unicode category detection | Pass through, providers should handle | REQUIRED |
| **RTL (Right-to-Left) text** | Unicode bidirectional markers | Preserve markers, log if issues reported | OPTIONAL |
| **Zero-width characters** | `\u{200B}`, `\u{FEFF}` detection | Strip or preserve based on config flag | OPTIONAL |
| **Surrogate pairs (UTF-16)** | Rust String handles natively | Trust Rust's UTF-8 validation | AUTOMATIC |
| **Mixed encoding in JSON** | JSON parser enforces UTF-8 | Reject with `SerializationError` | AUTOMATIC |

#### Malformed Request Cases
| Edge Case | Detection | Handling Strategy | Test Coverage |
|-----------|-----------|-------------------|---------------|
| **Malformed JSON** | `serde_json::from_slice()` error | Return 400 with parse error details | CRITICAL |
| **Extra unknown fields** | `#[serde(deny_unknown_fields)]` or allow | Config-driven: strict vs permissive mode | REQUIRED |
| **Type mismatches** | Serde deserialization error | Return 400 with field path and expected type | CRITICAL |
| **Nested JSON depth >100** | Serde recursion limit | Return error, prevent stack overflow | REQUIRED |
| **Array length violations** | Validate `messages.len()` bounds | Enforce max messages per request (default: 1000) | REQUIRED |

#### Temperature and Sampling Edge Cases
| Edge Case | Detection | Handling Strategy | Test Coverage |
|-----------|-----------|-----------|---------------|
| **temperature < 0.0** | Range check in validation | Reject with `InvalidRequest` | REQUIRED |
| **temperature > 2.0** | Range check (provider-dependent) | Warn or reject based on provider max | REQUIRED |
| **top_p outside [0,1]** | Range validation | Reject if outside valid range | REQUIRED |
| **top_k = 0** | Zero check | Reject or treat as disabled | REQUIRED |
| **Conflicting sampling params** | Both top_k and top_p set | Allow (provider handles priority) | OPTIONAL |

---

### 1.2 Provider Edge Cases

#### Response Integrity Cases
| Edge Case | Detection | Handling Strategy | Test Coverage |
|-----------|-----------|-------------------|---------------|
| **Empty response body** | `body.is_empty()` | Return `ProviderInternalError` with context | CRITICAL |
| **Partial response (truncated JSON)** | JSON parse error mid-stream | Retry if retryable, else return error | CRITICAL |
| **Missing required fields** | Serde deserialization error | Return `SerializationError` with missing field | CRITICAL |
| **Invalid finish_reason** | Enum parse failure | Default to `FinishReason::Error` and log | REQUIRED |
| **Negative token counts** | Check `usage.tokens < 0` | Reject response, log provider issue | REQUIRED |
| **Response larger than expected** | Content-Length or chunk size check | Limit to 10MB, reject if exceeded | REQUIRED |

#### Streaming Edge Cases
| Edge Case | Detection | Handling Strategy | Test Coverage |
|-----------|-----------|-------------------|---------------|
| **Stream interruption mid-token** | SSE connection drop | Return `StreamError`, client handles reconnect | CRITICAL |
| **Duplicate SSE events** | Track event IDs if provided | Deduplicate or pass through (client choice) | OPTIONAL |
| **Malformed SSE chunk** | Parse error in SSE line | Log, skip chunk, continue stream | REQUIRED |
| **Stream never sends `[DONE]`** | Timeout on stream completion | Close stream after inactivity timeout | REQUIRED |
| **Empty delta content** | `delta.content.is_none()` | Pass through, may indicate start/end event | NORMAL |

#### Network and Connectivity Cases
| Edge Case | Detection | Handling Strategy | Test Coverage |
|-----------|-----------|-------------------|---------------|
| **DNS resolution failure** | Hyper connection error | Retry with exponential backoff, then fallback | CRITICAL |
| **TLS certificate expiry** | TLS handshake error | Alert immediately, fail request (no retry) | CRITICAL |
| **Connection reset by peer** | TCP RST packet | Retry up to `max_retries` with backoff | REQUIRED |
| **Slow loris attacks** | Request timeout with partial data | Abort connection, ban IP via WAF integration | SECURITY |
| **Provider returns 5xx** | HTTP status check | Retry if `is_retryable_error()` returns true | CRITICAL |

#### Authentication and Rate Limiting Cases
| Edge Case | Detection | Handling Strategy | Test Coverage |
|-----------|-----------|-------------------|---------------|
| **Provider auth expiry mid-request** | 401/403 during request | Mark credential invalid, trigger refresh flow | CRITICAL |
| **Rate limit hit mid-request** | 429 status code | Extract `Retry-After` header, wait or queue | CRITICAL |
| **Rate limit boundary (token bucket)** | `try_consume()` returns wait time | Sleep for calculated duration before retry | REQUIRED |
| **Concurrent rate limit exhaustion** | Semaphore permits exhausted | Queue request or return 503 with retry estimate | REQUIRED |

---

### 1.3 Concurrency Edge Cases

#### State Transition Cases
| Edge Case | Detection | Handling Strategy | Test Coverage |
|-----------|-----------|-------------------|---------------|
| **Simultaneous circuit breaker trips** | Multiple failures within window | Atomic state transition to Open | CRITICAL |
| **Config reload during request** | Version mismatch detection | Complete in-flight requests with old config | REQUIRED |
| **Provider registration during routing** | Lock contention on `RwLock<HashMap>` | Use read lock for routing, write for registration | REQUIRED |
| **Health check during provider removal** | Check provider existence before health check | Skip health check if provider deregistered | REQUIRED |

#### Resource Contention Cases
| Edge Case | Detection | Handling Strategy | Test Coverage |
|-----------|-----------|-------------------|---------------|
| **Connection pool exhaustion** | Semaphore timeout on `acquire_permit()` | Return 503, increment metric, consider scaling | CRITICAL |
| **Cache stampede** | Multiple misses for same key | Lock per cache key during fetch (write-through) | REQUIRED |
| **RwLock writer starvation** | Monitoring lock wait times | Prefer `tokio::sync::RwLock` with fair scheduling | OPTIONAL |
| **Memory exhaustion (large cache)** | Monitor cache size vs limit | Evict oldest entries via LRU, enforce `max_size` | REQUIRED |

#### Race Condition Cases
| Edge Case | Detection | Handling Strategy | Test Coverage |
|-----------|-----------|-------------------|---------------|
| **Provider health flip during request** | Circuit breaker state change | Complete request, next request uses new state | ACCEPTABLE |
| **Metrics update race** | Concurrent counter increments | Use atomic operations or mutex-guarded metrics | REQUIRED |
| **Duplicate request deduplication** | Track request IDs in-flight | Optional: implement request ID deduplication cache | OPTIONAL |

---

## 2. Error Handling Matrix

### 2.1 Validation Errors (4xx Client Errors)

| Error Type | HTTP Status | Retryable | Client Message | Internal Action | Logging Level |
|------------|-------------|-----------|----------------|-----------------|---------------|
| **Empty messages array** | 400 | No | "Request must contain at least one message" | None | WARN |
| **Invalid temperature** | 400 | No | "Temperature must be between 0.0 and 2.0" | None | WARN |
| **Invalid top_p** | 400 | No | "top_p must be between 0.0 and 1.0" | None | WARN |
| **Token limit exceeded** | 400 | No | "Prompt exceeds max tokens: {actual} > {limit}" | Increment `validation_errors` metric | WARN |
| **Invalid UTF-8** | 400 | No | "Request contains invalid UTF-8 sequences" | Log hex dump | ERROR |
| **Malformed JSON** | 400 | No | "Invalid JSON: {parse_error}" | None | WARN |
| **Unknown provider** | 404 | No | "Provider '{id}' not found" | None | WARN |
| **Unsupported capability** | 400 | No | "Provider does not support {feature}" | None | INFO |
| **Missing required field** | 400 | No | "Missing required field: {field_name}" | None | WARN |
| **Type mismatch** | 400 | No | "Field '{field}' expects {expected}, got {actual}" | None | WARN |
| **Request too large** | 413 | No | "Request body exceeds 10MB limit" | Increment `oversized_requests` metric | WARN |

### 2.2 Authentication Errors (4xx Client Errors)

| Error Type | HTTP Status | Retryable | Client Message | Internal Action | Logging Level |
|------------|-------------|-----------|----------------|-----------------|---------------|
| **Missing API key** | 401 | No | "Authentication required: missing API key" | None | WARN |
| **Invalid API key** | 401 | No | "Invalid API key" | Mark credential for review | ERROR |
| **Expired API key** | 401 | Yes (after refresh) | "API key expired, please refresh" | Trigger credential refresh | ERROR |
| **Insufficient permissions** | 403 | No | "API key lacks permission for operation" | None | WARN |
| **Provider auth failure** | 502 | Yes | "Upstream provider authentication failed" | Alert on-call if persistent | ERROR |

### 2.3 Provider Errors (5xx Server Errors)

| Error Type | HTTP Status | Retryable | Client Message | Internal Action | Logging Level |
|------------|-------------|-----------|----------------|-----------------|---------------|
| **Provider rate limit** | 429 | Yes | "Rate limit exceeded, retry after {seconds}s" | Respect `Retry-After`, update rate limiter | WARN |
| **Provider timeout** | 504 | Yes | "Request timed out after {timeout}s" | Retry with backoff, check health | ERROR |
| **Provider 500** | 502 | Yes | "Provider internal error" | Retry, trigger circuit breaker if repeated | ERROR |
| **Provider 503** | 503 | Yes | "Provider temporarily unavailable" | Retry, failover to backup provider | WARN |
| **Connection refused** | 503 | Yes | "Cannot connect to provider" | Retry, mark unhealthy, alert | ERROR |
| **TLS error** | 502 | No | "TLS/SSL error connecting to provider" | Alert immediately, do not retry | CRITICAL |
| **DNS resolution failure** | 502 | Yes | "Cannot resolve provider hostname" | Retry with alternative DNS, alert | ERROR |
| **Empty response** | 502 | Yes | "Provider returned empty response" | Retry, log provider ID | ERROR |
| **Invalid response format** | 502 | No | "Provider response format invalid" | Log response, do not retry | ERROR |
| **Stream error** | 500 | No | "Stream interrupted: {reason}" | Client may retry from beginning | ERROR |
| **Circuit breaker open** | 503 | Yes (after timeout) | "Service temporarily unavailable" | Wait for circuit recovery | WARN |
| **Connection pool exhausted** | 503 | Yes | "Service at capacity, retry shortly" | Increment pool exhaustion metric, consider scaling | WARN |
| **Response too large** | 502 | No | "Provider response exceeds size limit" | Log provider + model, do not cache | ERROR |
| **Partial response** | 502 | Yes | "Provider returned incomplete response" | Retry, check network stability | ERROR |
| **Serialization error** | 500 | No | "Failed to parse provider response" | Log raw response for debugging | ERROR |

### 2.4 System Errors (5xx Server Errors)

| Error Type | HTTP Status | Retryable | Client Message | Internal Action | Logging Level |
|------------|-------------|-----------|----------------|-----------------|---------------|
| **Internal panic** | 500 | No | "Internal server error" | Log backtrace, alert on-call | CRITICAL |
| **Database connection failure** | 503 | Yes | "Service temporarily unavailable" | Retry DB connection, alert if persistent | CRITICAL |
| **Cache failure** | 200 (degrade) | N/A | (none, serve from origin) | Log cache error, continue without cache | WARN |
| **Metrics collection failure** | N/A | N/A | (none) | Log metrics error, continue request | ERROR |
| **Memory allocation failure** | 503 | No | "Service at capacity" | Alert immediately, trigger restart | CRITICAL |
| **Thread pool exhaustion** | 503 | Yes | "Service at capacity" | Queue request, alert if queue grows | ERROR |
| **Lock poisoning** | 500 | No | "Internal synchronization error" | Log stack trace, restart component | CRITICAL |
| **Shutdown in progress** | 503 | No | "Service shutting down" | Graceful shutdown, drain requests | INFO |

### 2.5 Timeout Errors

| Error Type | HTTP Status | Retryable | Client Message | Internal Action | Logging Level |
|------------|-------------|-----------|----------------|-----------------|---------------|
| **Request timeout (client)** | 408 | Yes | "Client request timeout after {timeout}s" | None | WARN |
| **Provider timeout** | 504 | Yes | "Provider response timeout" | Retry with backoff, adjust timeout | ERROR |
| **Stream read timeout** | 504 | No | "Stream inactive for {timeout}s" | Close stream, client may retry | WARN |
| **Connection timeout** | 504 | Yes | "Connection timeout" | Retry, check network/firewall | ERROR |
| **Rate limit wait timeout** | 429 | Yes | "Rate limit wait exceeded max duration" | Return error, client should back off | WARN |

---

## 3. Recovery Procedures

### 3.1 Automatic Recovery

#### Provider Failure Recovery
```
Error Class: Provider 5xx, Timeout, Network Error
├─ Step 1: Immediate Retry (if retryable)
│  ├─ Backoff: 100ms → 200ms → 400ms
│  └─ Max Retries: 3 attempts
├─ Step 2: Circuit Breaker Check
│  ├─ Failure Threshold: 5 consecutive failures
│  ├─ Open State Duration: 30 seconds
│  └─ Half-Open Test: 2 successful requests to close
├─ Step 3: Failover (if configured)
│  ├─ Trigger: Circuit breaker opens
│  ├─ Action: Route to backup provider(s)
│  └─ Monitor: Track failover success rate
└─ Step 4: Auto-Recovery
   ├─ Background health checks: every 60s
   ├─ Circuit breaker transitions to half-open
   └─ Gradual traffic ramp-up on success
```

#### Rate Limit Recovery
```
Error Class: 429 Rate Limit Exceeded
├─ Step 1: Extract Retry-After Header
│  ├─ Parse duration from provider response
│  └─ Fallback: Use token bucket calculation
├─ Step 2: Backpressure Application
│  ├─ Sleep for calculated duration
│  ├─ Queue subsequent requests (optional)
│  └─ Update rate limiter state
└─ Step 3: Automatic Resume
   ├─ Token bucket refills over time
   ├─ Requests dequeued automatically
   └─ Monitor: rate limit hit frequency
```

#### Connection Pool Recovery
```
Error Class: Connection Pool Exhausted
├─ Step 1: Queue Request (if enabled)
│  ├─ Max Queue Depth: 100 requests
│  ├─ Queue Timeout: 10 seconds
│  └─ Eviction: FIFO when full
├─ Step 2: Shed Load (if queue full)
│  ├─ Return 503 to client
│  ├─ Increment load shedding metric
│  └─ Client should retry with backoff
└─ Step 3: Auto-Scale (if configured)
   ├─ Monitor: pool exhaustion rate
   ├─ Trigger: >80% utilization for 5 minutes
   └─ Action: Increase max_connections_per_provider
```

#### Cache Recovery
```
Error Class: Cache Unavailable
├─ Step 1: Graceful Degradation
│  ├─ Skip cache lookup
│  ├─ Serve request from origin (provider)
│  └─ Log cache miss
├─ Step 2: Cache Bypass
│  ├─ Continue all requests without caching
│  ├─ Monitor cache error rate
│  └─ Alert if cache down >5 minutes
└─ Step 3: Cache Rebuild
   ├─ Background task: reconnect to cache
   ├─ Clear potentially corrupt entries
   └─ Resume caching on successful connection
```

---

### 3.2 Manual Intervention Triggers

#### Critical Alerts (Immediate Response Required)
| Condition | Trigger | Action | Owner |
|-----------|---------|--------|-------|
| **Provider API key expired** | 401 error rate >50% | Rotate credentials immediately | DevOps |
| **TLS certificate expiry** | TLS handshake failures | Renew certificate, restart service | Security Team |
| **All providers unhealthy** | No healthy providers for 2 minutes | Investigate network/config, rollback if recent deploy | On-Call SRE |
| **Memory leak detected** | Memory usage growth >20%/hour | Restart pods, investigate leak with profiler | Backend Team |
| **Persistent 5xx errors** | Error rate >10% for 5 minutes | Check provider status pages, engage vendor support | On-Call SRE |

#### Warning Alerts (Response Within 30 Minutes)
| Condition | Trigger | Action | Owner |
|-----------|---------|--------|-------|
| **Circuit breaker opened** | Circuit state = Open for >5 minutes | Review provider health, adjust thresholds | Backend Team |
| **High rate limit hits** | 429 errors >20% of requests | Increase rate limits or shard traffic | DevOps |
| **Connection pool near capacity** | >80% utilization for 10 minutes | Increase pool size or scale horizontally | SRE |
| **Cache hit rate drop** | Cache hit rate <60% (normally >85%) | Check cache size, TTL settings, memory | Backend Team |

#### Info Alerts (Review During Business Hours)
| Condition | Trigger | Action | Owner |
|-----------|---------|--------|-------|
| **Slow provider responses** | P95 latency >10s | Optimize prompts, consider faster models | Product Team |
| **Unusual traffic patterns** | Request rate +200% vs baseline | Verify legitimate traffic, check for abuse | Security Team |
| **Provider deprecation notices** | Provider API version sunset alert | Plan migration to new API version | Backend Team |

---

### 3.3 Escalation Criteria

#### Level 1: Self-Healing (Automatic)
- Single provider failure with healthy fallback
- Transient network errors (retries succeed)
- Rate limit hit (backoff and resume)
- Circuit breaker cycling (opens and closes normally)

#### Level 2: Monitoring Alert (Automated Notification)
- Circuit breaker open >5 minutes
- Error rate >5% for any provider
- Connection pool exhaustion events
- Cache failure lasting >2 minutes

#### Level 3: On-Call Page (Immediate Response)
- All providers unhealthy simultaneously
- Error rate >20% system-wide
- Memory/CPU exhaustion
- Security incident (API key leak, DDoS)

#### Level 4: Incident Commander Activation
- Multi-region outage
- Data integrity issues (corruption detected)
- Vendor-side major incident (provider outage)
- Service degradation >1 hour affecting >50% users

---

### 3.4 Monitoring and Alerting

#### Key Metrics to Monitor
```yaml
Request Metrics:
  - llm_requests_total (by provider, model, status)
  - llm_request_duration_seconds (histogram)
  - llm_requests_failure (by error_type)

Provider Health:
  - llm_provider_healthy (boolean gauge)
  - llm_circuit_breaker_state (0=closed, 1=half-open, 2=open)
  - llm_provider_error_rate (rolling 5m average)

Resource Utilization:
  - llm_active_connections (by provider)
  - llm_connection_pool_utilization_percent
  - llm_cache_size_bytes
  - llm_cache_hit_rate

Token Usage:
  - llm_tokens_prompt (counter)
  - llm_tokens_completion (counter)
  - llm_tokens_cost_usd (calculated)

Rate Limiting:
  - llm_rate_limit_hits (counter)
  - llm_rate_limit_wait_seconds (histogram)
```

#### Alert Thresholds (Recommended)
```yaml
Critical:
  - error_rate > 20% for 2 minutes
  - all_providers_unhealthy for 1 minute
  - p99_latency > 60s for 5 minutes

Warning:
  - error_rate > 5% for 5 minutes
  - circuit_breaker_open for 5 minutes
  - connection_pool_utilization > 90% for 10 minutes
  - cache_hit_rate < 70% for 15 minutes

Info:
  - new_provider_registered
  - config_reloaded
  - cache_eviction_rate > 1000/min
```

---

## 4. Testing Requirements

### 4.1 Edge Case Test Coverage
```
Priority Levels:
  CRITICAL:  Must have automated tests in CI/CD
  REQUIRED:  Should have tests, may be manual initially
  OPTIONAL:  Nice to have, test if time permits
  AUTOMATIC: Handled by language/framework
```

### 4.2 Chaos Engineering Scenarios
1. **Provider flakiness**: Random 5xx injection (10% of requests)
2. **Network partition**: Drop all packets to provider for 30s
3. **Slow provider**: Inject 10-30s artificial latency
4. **Memory pressure**: Limit container memory to trigger OOM
5. **Concurrent load**: 10,000 simultaneous requests
6. **Cache corruption**: Inject invalid cache entries
7. **Time skew**: Simulate clock drift (rate limiter impact)
8. **TLS expiry**: Test with expired certificates

### 4.3 Integration Test Matrix
```
Test each provider against:
  - Empty messages
  - Maximum context length
  - Streaming interruption
  - Invalid credentials
  - Rate limit scenarios
  - Malformed responses
  - Timeout conditions
```

---

## 5. Appendix: Error Code Reference

### 5.1 Custom Error Codes
```rust
// Extend ProviderError enum with error codes
pub enum ProviderError {
    // 1xxx: Validation Errors
    EmptyMessages,          // 1001
    InvalidTemperature,     // 1002
    TokenLimitExceeded,     // 1003

    // 2xxx: Provider Errors
    ProviderTimeout,        // 2001
    ProviderRateLimit,      // 2002
    ProviderAuthFailed,     // 2003

    // 3xxx: System Errors
    CircuitBreakerOpen,     // 3001
    ConnectionPoolExhausted,// 3002
    CacheUnavailable,       // 3003

    // 4xxx: Network Errors
    DnsResolutionFailed,    // 4001
    TlsHandshakeFailed,     // 4002
    ConnectionResetByPeer,  // 4003
}
```

### 5.2 HTTP Status Code Mapping
```
400: Validation errors, malformed requests
401: Authentication failures
403: Authorization/permission errors
404: Provider not found, model not found
408: Client request timeout
413: Request payload too large
429: Rate limit exceeded
500: Internal server errors, panics
502: Provider communication errors
503: Service unavailable, circuit breaker open
504: Gateway timeout (provider timeout)
```

---

**Document Maintenance:**
- Review quarterly or after major incidents
- Update based on production telemetry
- Incorporate lessons learned from postmortems
- Version control all changes

**Related Documentation:**
- Architecture Decision Records (ADRs)
- Runbook for on-call engineers
- Provider-specific integration guides
- Performance tuning guide
