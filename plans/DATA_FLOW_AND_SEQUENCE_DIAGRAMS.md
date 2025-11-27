# LLM-Inference-Gateway: Data Flow & Sequence Documentation

> **Document Type**: Architecture Documentation
> **Version**: 1.0.0
> **Last Updated**: 2025-11-27
> **Status**: Complete

---

## Table of Contents

1. [Request Lifecycle Overview](#1-request-lifecycle-overview)
2. [Sequence Diagrams](#2-sequence-diagrams)
3. [Data Transformation Pipeline](#3-data-transformation-pipeline)
4. [State Management](#4-state-management)
5. [Error Propagation](#5-error-propagation)
6. [Performance Characteristics](#6-performance-characteristics)

---

## 1. Request Lifecycle Overview

### Complete Flow: Client to Response

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     REQUEST LIFECYCLE (COMPLETE FLOW)                        │
└─────────────────────────────────────────────────────────────────────────────┘

Client Request
    │
    ├──> [1] TLS Termination (0-2ms)
    │    └──> Certificate validation
    │    └──> TLS 1.3 handshake (if new connection)
    │
    ├──> [2] HTTP Parser (Axum) (<1ms)
    │    └──> Parse HTTP/1.1 or HTTP/2
    │    └──> Extract headers, body
    │    └──> Connection pooling check
    │
    ├──> [3] Middleware Pipeline (1-3ms)
    │    │
    │    ├──> [3a] Request ID Generation
    │    │    └──> Generate unique trace ID
    │    │    └──> Propagate W3C Trace Context
    │    │
    │    ├──> [3b] Authentication (0.5-1ms)
    │    │    └──> Extract API key from header
    │    │    └──> Hash and lookup in cache (DashMap)
    │    │    └──> Validate tenant permissions
    │    │
    │    ├──> [3c] Rate Limiting (0.1-0.5ms)
    │    │    └──> Token bucket check (atomic operations)
    │    │    └──> Per-tenant/per-key rate limits
    │    │    └──> Return 429 if exceeded
    │    │
    │    ├──> [3d] Request Validation (0.5-1ms)
    │    │    └──> JSON schema validation
    │    │    └──> Parameter bounds checking
    │    │    └──> Model name normalization
    │    │
    │    └──> [3e] Cost Budget Check (0.2ms)
    │         └──> Estimate request cost
    │         └──> Check monthly budget (atomic counter)
    │
    ├──> [4] Router Selection (<1ms)
    │    │
    │    ├──> [4a] Candidate Provider Lookup (O(1))
    │    │    └──> DashMap lookup by model name
    │    │    └──> Filter unavailable providers
    │    │
    │    ├──> [4b] Routing Rules Engine (0.2-0.5ms)
    │    │    └──> Match request attributes to rules
    │    │    └──> Apply tenant-specific overrides
    │    │    └──> Determine load balancing strategy
    │    │
    │    ├──> [4c] Health-Aware Filtering (0.1-0.3ms)
    │    │    └──> Check health scores (atomic reads)
    │    │    └──> Filter out degraded providers
    │    │    └──> Circuit breaker state check
    │    │
    │    └──> [4d] Load Balancing Selection (0.1-0.3ms)
    │         ├──> Round Robin: Atomic counter increment
    │         ├──> Least Connections: Compare atomic counters
    │         ├──> Least Latency: Read latency histograms
    │         └──> Cost Optimized: Compare cost metrics
    │
    ├──> [5] Circuit Breaker Check (0.1ms)
    │    └──> Check circuit state (atomic read)
    │    └──> If OPEN: Return 503 with Retry-After
    │    └──> If HALF_OPEN: Rate limit test requests
    │    └──> Acquire circuit breaker guard
    │
    ├──> [6] Bulkhead/Concurrency Limit (0.1-5ms)
    │    └──> Acquire semaphore permit
    │    └──> Wait if at max concurrency
    │    └──> Timeout if wait exceeds threshold
    │
    ├──> [7] Provider Request Transform (0.5-1ms)
    │    └──> Convert gateway format → provider format
    │    └──> Map model names (aliases)
    │    └──> Adjust parameters for provider
    │
    ├──> [8] Provider API Call (50-5000ms)
    │    │
    │    ├──> HTTP client request (connection pooled)
    │    ├──> Request timeout enforcement
    │    └──> Streaming or non-streaming path
    │
    ├──> [9] Response Transform (0.5-2ms)
    │    └──> Provider format → gateway format
    │    └──> Normalize token counts
    │    └──> Extract usage metadata
    │
    ├──> [10] Post-Processing Middleware (1-2ms)
    │    │
    │    ├──> [10a] Circuit Breaker Update
    │    │    └──> Record success/failure
    │    │    └──> Update sliding window metrics
    │    │
    │    ├──> [10b] Metrics Collection
    │    │    └──> Latency histogram update
    │    │    └──> Token usage tracking
    │    │    └──> Cost calculation
    │    │
    │    ├──> [10c] Telemetry Export
    │    │    └──> Emit OpenTelemetry span
    │    │    └──> Prometheus metrics increment
    │    │    └──> Structured log output
    │    │
    │    └──> [10d] Response Caching (optional)
    │         └──> Store in semantic cache
    │         └──> TTL-based eviction
    │
    └──> [11] HTTP Response (0.5-1ms)
         └──> JSON serialization
         └──> Compression (if enabled)
         └──> Send to client

Total P50 Latency: Request overhead = 5-15ms + Provider latency
Total P95 Latency: Request overhead = 8-25ms + Provider latency
Total P99 Latency: Request overhead = 15-50ms + Provider latency
```

---

## 2. Sequence Diagrams

### 2.1 Successful Chat Completion Request (Non-Streaming)

```
┌────────┐     ┌─────────┐     ┌──────────┐     ┌────────┐     ┌──────────┐     ┌──────────┐
│ Client │     │ Gateway │     │ Middleware│    │ Router │     │Circuit   │     │ Provider │
│        │     │ Server  │     │ Pipeline  │    │        │     │Breaker   │     │          │
└───┬────┘     └────┬────┘     └────┬─────┘     └───┬────┘     └────┬─────┘     └────┬─────┘
    │               │               │               │               │               │
    │ POST /v1/chat/│               │               │               │               │
    │  completions  │               │               │               │               │
    │──────────────>│               │               │               │               │
    │               │               │               │               │               │
    │ [t=0ms]       │ Generate      │               │               │               │
    │               │ Request ID    │               │               │               │
    │               │ Trace ID      │               │               │               │
    │               │───────────────┤               │               │               │
    │               │               │               │               │               │
    │ [t=0.5ms]     │               │ Authenticate  │               │               │
    │               │               │ API Key       │               │               │
    │               │               │<─ - - - - - - │               │               │
    │               │               │ ✓ Valid       │               │               │
    │               │               │ - - - - - - ->│               │               │
    │               │               │               │               │               │
    │ [t=1ms]       │               │ Rate Limit    │               │               │
    │               │               │ Check         │               │               │
    │               │               │<─ - - - - - - │               │               │
    │               │               │ ✓ Allowed     │               │               │
    │               │               │ - - - - - - ->│               │               │
    │               │               │               │               │               │
    │ [t=1.5ms]     │               │ Validate JSON │               │               │
    │               │               │ Schema        │               │               │
    │               │               │<─ - - - - - - │               │               │
    │               │               │ ✓ Valid       │               │               │
    │               │               │ - - - - - - ->│               │               │
    │               │               │               │               │               │
    │ [t=2ms]       │               │               │ Select        │               │
    │               │               │               │ Provider      │               │
    │               │               │               │<─ - - - - - - ┤               │
    │               │               │               │               │               │
    │ [t=2.5ms]     │               │               │ Lookup        │               │
    │               │               │               │ Candidates    │               │
    │               │               │               │ (DashMap O(1))│               │
    │               │               │               │               │               │
    │               │               │               │ Apply Rules   │               │
    │               │               │               │ Filter Health │               │
    │               │               │               │ Load Balance  │               │
    │               │               │               │               │               │
    │ [t=3ms]       │               │               │ Provider:     │               │
    │               │               │               │ openai-gpt4   │               │
    │               │               │               │ - - - - - - ->│               │
    │               │               │               │               │               │
    │ [t=3.2ms]     │               │               │               │ Check State   │
    │               │               │               │               │ (CLOSED)      │
    │               │               │               │               │<─ - - - - - - │
    │               │               │               │               │ ✓ Allow       │
    │               │               │               │               │ - - - - - - ->│
    │               │               │               │               │               │
    │ [t=3.5ms]     │               │ Transform     │               │               │
    │               │               │ Request →     │               │               │
    │               │               │ OpenAI Format │               │               │
    │               │               │───────────────┤               │               │
    │               │               │               │               │               │
    │ [t=4ms]       │               │               │               │ HTTP POST     │
    │               │               │               │               │ /v1/chat/     │
    │               │               │               │               │ completions   │
    │               │               │               │               │──────────────>│
    │               │               │               │               │               │
    │               │               │               │               │               │ ┌──────────┐
    │               │               │               │               │               │ │ Process  │
    │               │               │               │               │               │ │ Request  │
    │               │               │               │               │               │ │ Generate │
    │               │               │               │               │               │ │ Response │
    │               │               │               │               │               │ └──────────┘
    │               │               │               │               │               │
    │ [t=1504ms]    │               │               │               │               │
    │               │               │               │               │               │ 200 OK     │
    │               │               │               │               │               │ {response} │
    │               │               │               │               │<──────────────│
    │               │               │               │               │               │
    │ [t=1505ms]    │               │               │               │ Record Success│
    │               │               │               │               │ Update Metrics│
    │               │               │               │               │───────────────┤
    │               │               │               │               │               │
    │               │               │ Transform     │               │               │
    │               │               │ OpenAI → GW   │               │               │
    │               │               │<──────────────┤               │               │
    │               │               │               │               │               │
    │               │               │ Emit Metrics: │               │               │
    │               │               │ - Latency: 1501ms             │               │
    │               │               │ - Tokens: 250 │               │               │
    │               │               │ - Cost: $0.015│               │               │
    │               │               │───────────────┤               │               │
    │               │               │               │               │               │
    │ [t=1507ms]    │ 200 OK        │               │               │               │
    │<──────────────│ {response}    │               │               │               │
    │               │               │               │               │               │
    │               │               │               │               │               │
    └───┬────┘     └────┬────┘     └────┬─────┘     └───┬────┘     └────┬─────┘     └────┬─────┘

Total Request Time: 1507ms
  - Gateway Overhead: 7ms (0.46%)
  - Provider Latency: 1500ms (99.54%)
```

---

### 2.2 Streaming Response with SSE

```
┌────────┐     ┌─────────┐     ┌──────────┐     ┌────────┐     ┌──────────┐
│ Client │     │ Gateway │     │ Middleware│    │ Router │     │ Provider │
│        │     │ Server  │     │ Pipeline  │    │        │     │          │
└───┬────┘     └────┬────┘     └────┬─────┘     └───┬────┘     └────┬─────┘
    │               │               │               │               │
    │ POST /v1/chat/│               │               │               │
    │  completions  │               │               │               │
    │ {stream:true} │               │               │               │
    │──────────────>│               │               │               │
    │               │               │               │               │
    │ [t=0-4ms]     │ Middleware    │               │               │
    │               │ Pipeline      │               │               │
    │               │ (Auth, Rate   │               │               │
    │               │  Limit, etc)  │               │               │
    │               │──────────────>│               │               │
    │               │               │               │               │
    │               │               │ Route Request │               │
    │               │               │──────────────>│               │
    │               │               │               │               │
    │ [t=5ms]       │               │               │ Select        │
    │               │               │               │ Provider      │
    │               │               │               │───────────────┤
    │               │               │               │ openai-gpt4   │
    │               │               │<──────────────│               │
    │               │               │               │               │
    │               │               │               │ HTTP POST     │
    │               │               │               │ (stream=true) │
    │               │               │               │──────────────>│
    │               │               │               │               │
    │ [t=8ms]       │ 200 OK        │               │               │
    │               │ Content-Type: │               │               │
    │               │ text/event-   │               │               │
    │               │ stream        │               │               │
    │<──────────────│               │               │               │
    │               │               │               │               │
    │               │               │               │               │ Stream Starts │
    │               │               │               │               │ ┌───────────┐ │
    │               │               │               │               │ │ Generate  │ │
    │               │               │               │               │ │ Tokens    │ │
    │               │               │               │               │ └───────────┘ │
    │               │               │               │               │               │
    │ [t=50ms]      │               │               │               │ data: {chunk1}│
    │               │               │               │               │<──────────────│
    │               │               │ Transform     │               │               │
    │               │               │ Chunk         │               │               │
    │               │               │<──────────────┤               │               │
    │               │ data: {chunk1}│               │               │               │
    │<──────────────│               │               │               │               │
    │               │               │               │               │               │
    │ [t=120ms]     │               │               │               │ data: {chunk2}│
    │               │               │               │               │<──────────────│
    │               │               │ Transform     │               │               │
    │               │               │<──────────────┤               │               │
    │               │ data: {chunk2}│               │               │               │
    │<──────────────│               │               │               │               │
    │               │               │               │               │               │
    │     ...       │     ...       │     ...       │     ...       │      ...      │
    │               │               │               │               │               │
    │ [t=2100ms]    │               │               │               │ data: {chunkN}│
    │               │               │               │               │<──────────────│
    │               │               │ Transform     │               │               │
    │               │               │<──────────────┤               │               │
    │               │ data: {chunkN}│               │               │               │
    │<──────────────│               │               │               │               │
    │               │               │               │               │               │
    │ [t=2105ms]    │               │               │               │ data: [DONE]  │
    │               │               │               │               │<──────────────│
    │               │ data: [DONE]  │               │               │               │
    │<──────────────│               │               │               │               │
    │               │               │               │               │               │
    │ [t=2110ms]    │ Stream Closed │               │               │               │
    │<──────────────│               │               │               │               │
    │               │               │               │               │               │
    │               │               │ Collect Metrics:              │               │
    │               │               │ - Total Tokens: 350           │               │
    │               │               │ - Stream Duration: 2.1s       │               │
    │               │               │ - Chunks Sent: 45             │               │
    │               │               │───────────────┤               │               │
    │               │               │               │               │               │
    └───┬────┘     └────┬────┘     └────┬─────┘     └───┬────┘     └────┬─────┘

Key Metrics:
  - Time to First Byte (TTFB): 50ms
  - Time Between Chunks: 50-100ms
  - Total Stream Duration: 2100ms
  - Backpressure Handling: Async channel buffering
```

---

### 2.3 Provider Failover Scenario

```
┌────────┐   ┌─────────┐   ┌────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐
│ Client │   │ Gateway │   │ Router │   │Circuit   │   │Provider-A│   │Provider-B│
│        │   │ Server  │   │        │   │Breaker   │   │ (Primary)│   │(Fallback)│
└───┬────┘   └────┬────┘   └───┬────┘   └────┬─────┘   └────┬─────┘   └────┬─────┘
    │             │             │             │             │             │
    │ POST /v1/   │             │             │             │             │
    │ chat/       │             │             │             │             │
    │ completions │             │             │             │             │
    │────────────>│             │             │             │             │
    │             │             │             │             │             │
    │[t=0-5ms]    │ Middleware  │             │             │             │
    │             │ Processing  │             │             │             │
    │             │─────────────┤             │             │             │
    │             │             │             │             │             │
    │[t=5ms]      │             │ Select      │             │             │
    │             │             │ Provider-A  │             │             │
    │             │             │ (Primary)   │             │             │
    │             │             │─────────────┤             │             │
    │             │             │             │             │             │
    │[t=6ms]      │             │             │ Check CB    │             │
    │             │             │             │ State:      │             │
    │             │             │             │ CLOSED      │             │
    │             │             │             │<─ - - - - - │             │
    │             │             │             │ ✓ Allow     │             │
    │             │             │             │ - - - - - ->│             │
    │             │             │             │             │             │
    │[t=7ms]      │             │             │             │ HTTP POST   │
    │             │             │             │             │ Request     │
    │             │             │             │             │────────────>│
    │             │             │             │             │             │
    │             │             │             │             │             │
    │             │             │             │             │             X
    │             │             │             │             │             │ Connection
    │             │             │             │             │             │   Failed
    │[t=5007ms]   │             │             │             │ Timeout!    │
    │             │             │             │             │<────────────│
    │             │             │             │             │ 503 Service │
    │             │             │             │             │ Unavailable │
    │             │             │             │             │             │
    │[t=5008ms]   │             │             │ Record      │             │
    │             │             │             │ Failure     │             │
    │             │             │             │ (attempt 1) │             │
    │             │             │             │<────────────│             │
    │             │             │             │             │             │
    │             │             │             │ Failures: 5 │             │
    │             │             │             │ Threshold   │             │
    │             │             │             │ Exceeded!   │             │
    │             │             │             │             │             │
    │             │             │             │ CB State:   │             │
    │             │             │             │ CLOSED →    │             │
    │             │             │             │ OPEN        │             │
    │             │             │             │─────────────┤             │
    │             │             │             │             │             │
    │[t=5009ms]   │             │ Provider-A  │             │             │
    │             │             │ Failed!     │             │             │
    │             │             │<────────────┤             │             │
    │             │             │             │             │             │
    │             │             │ Execute     │             │             │
    │             │             │ Failover    │             │             │
    │             │             │ Logic       │             │             │
    │             │             │─────────────┤             │             │
    │             │             │             │             │             │
    │             │             │ Select      │             │             │
    │             │             │ Provider-B  │             │             │
    │             │             │ (Fallback)  │             │             │
    │             │             │─────────────┤             │             │
    │             │             │             │             │             │
    │[t=5010ms]   │             │             │             │             │ HTTP POST
    │             │             │             │             │             │ Request
    │             │             │             │             │             │────────────>
    │             │             │             │             │             │
    │             │             │             │             │             │ ✓ Success
    │[t=6510ms]   │             │             │             │             │ 200 OK
    │             │             │             │             │             │<────────────
    │             │             │             │             │             │
    │             │ Response    │             │             │             │
    │             │ from        │             │             │             │
    │             │ Provider-B  │             │             │             │
    │<────────────│             │             │             │             │
    │ 200 OK      │             │             │             │             │
    │             │             │             │             │             │
    │             │             │ Emit Event: │             │             │
    │             │             │ Failover    │             │             │
    │             │             │ Occurred    │             │             │
    │             │             │ A → B       │             │             │
    │             │             │─────────────┤             │             │
    │             │             │             │             │             │
    └───┬────┘   └────┬────┘   └───┬────┘   └────┬─────┘   └────┬─────┘   └────┬─────┘

Metrics:
  - Primary Provider Attempt: 5000ms (timeout)
  - Failover Decision: 2ms
  - Fallback Provider Success: 1500ms
  - Total Request Time: 6510ms
  - Circuit Breaker: Provider-A marked OPEN for 60s
```

---

### 2.4 Rate Limit Exceeded (429 Response)

```
┌────────┐     ┌─────────┐     ┌──────────┐     ┌────────┐
│ Client │     │ Gateway │     │ Middleware│    │ Rate   │
│        │     │ Server  │     │ Pipeline  │    │ Limiter│
└───┬────┘     └────┬────┘     └────┬─────┘     └───┬────┘
    │               │               │               │
    │ POST /v1/chat/│               │               │
    │  completions  │               │               │
    │──────────────>│               │               │
    │               │               │               │
    │ [t=0ms]       │ Generate ID   │               │
    │               │ Start Trace   │               │
    │               │───────────────┤               │
    │               │               │               │
    │ [t=0.5ms]     │               │ Authenticate  │
    │               │               │ ✓ Valid       │
    │               │               │───────────────┤
    │               │               │               │
    │ [t=1ms]       │               │ Rate Limit    │
    │               │               │ Check         │
    │               │               │──────────────>│
    │               │               │               │
    │               │               │               │ ┌──────────────┐
    │               │               │               │ │ Token Bucket │
    │               │               │               │ │ Algorithm    │
    │               │               │               │ │              │
    │               │               │               │ │ tenant_123:  │
    │               │               │               │ │ tokens=0/100 │
    │               │               │               │ │ refill_rate= │
    │               │               │               │ │ 10/min       │
    │               │               │               │ │              │
    │               │               │               │ │ ✗ DENIED     │
    │               │               │               │ └──────────────┘
    │               │               │               │
    │ [t=1.2ms]     │               │               │ Rate Limit
    │               │               │               │ Exceeded!
    │               │               │               │<──────────────│
    │               │               │               │
    │               │               │ Calculate     │
    │               │               │ Retry-After   │
    │               │               │ = 6 seconds   │
    │               │               │───────────────┤
    │               │               │               │
    │ [t=1.5ms]     │ 429 Too Many  │               │
    │               │ Requests      │               │
    │<──────────────│               │               │
    │               │ Headers:      │               │
    │               │ Retry-After: 6│               │
    │               │ X-RateLimit-  │               │
    │               │ Limit: 100    │               │
    │               │ X-RateLimit-  │               │
    │               │ Remaining: 0  │               │
    │               │ X-RateLimit-  │               │
    │               │ Reset: 1701... │               │
    │               │               │               │
    │               │               │ Emit Metrics: │
    │               │               │ - Event: rate_│
    │               │               │   limit_exceed│
    │               │               │ - Tenant: 123 │
    │               │               │ - Path: /chat │
    │               │               │───────────────┤
    │               │               │               │
    │ [Wait 6s]     │               │               │
    │               │               │               │
    │ POST /v1/chat/│               │               │
    │  completions  │               │               │
    │ (retry)       │               │               │
    │──────────────>│               │               │
    │               │               │               │
    │ [t=6001ms]    │               │ Rate Limit    │
    │               │               │ Check         │
    │               │               │──────────────>│
    │               │               │               │ ✓ Allowed
    │               │               │               │ tokens=10/100
    │               │               │<──────────────│
    │               │               │               │
    │ [continues normally...]       │               │
    │               │               │               │
    └───┬────┘     └────┬────┘     └────┬─────┘     └───┬────┘

Response Headers:
  HTTP/1.1 429 Too Many Requests
  Retry-After: 6
  X-RateLimit-Limit: 100
  X-RateLimit-Remaining: 0
  X-RateLimit-Reset: 1701234567
  Content-Type: application/json

Response Body:
  {
    "error": {
      "message": "Rate limit exceeded for tenant",
      "type": "rate_limit_error",
      "param": null,
      "code": "rate_limit_exceeded"
    }
  }
```

---

### 2.5 Circuit Breaker State Transitions

```
┌────────────────────────────────────────────────────────────────────────────┐
│                    CIRCUIT BREAKER STATE TRANSITIONS                       │
└────────────────────────────────────────────────────────────────────────────┘

STATE 1: CLOSED (Normal Operation)
───────────────────────────────────

Provider: openai-gpt4
Circuit State: CLOSED
Failure Count: 0/5
Success Count: 1000

    │
    │ Request 1 ──> SUCCESS (200 OK, 1200ms)
    │ Failure Count: 0/5
    │
    │ Request 2 ──> SUCCESS (200 OK, 1150ms)
    │ Failure Count: 0/5
    │
    │ Request 3 ──> FAILURE (503 Service Unavailable)
    │ Failure Count: 1/5 ←─────────────┐
    │                                   │ Record Failure
    │                                   │ Increment Counter
    │ Request 4 ──> FAILURE (Timeout)   │
    │ Failure Count: 2/5 ←──────────────┘
    │
    │ Request 5 ──> FAILURE (502 Bad Gateway)
    │ Failure Count: 3/5
    │
    │ Request 6 ──> FAILURE (Connection Refused)
    │ Failure Count: 4/5
    │
    │ Request 7 ──> FAILURE (Timeout)
    │ Failure Count: 5/5 ←─────────────┐
    │                                   │ THRESHOLD EXCEEDED!
    │                                   │
    ▼                                   │ Transition:
STATE 2: OPEN (Circuit Broken)         │ CLOSED → OPEN
───────────────────────────────         │ Record timestamp
                                        │ Emit alert
Provider: openai-gpt4 ◄─────────────────┘
Circuit State: OPEN
Opened At: t=1000ms
Timeout: 60 seconds

    │
    │ Request 8 ──> REJECTED (Circuit OPEN)
    │               Return immediately: 503
    │               Error: "Circuit breaker open for provider openai-gpt4"
    │               Retry-After: 55 seconds
    │
    │ Request 9 ──> REJECTED (Circuit OPEN)
    │               Retry-After: 50 seconds
    │
    │ ... (55 seconds elapsed) ...
    │
    │ [t=60000ms] Circuit Timeout Elapsed
    │              Transition: OPEN → HALF_OPEN ←─────┐
    │              Allow limited test requests         │ Automatic
    ▼                                                   │ Recovery
STATE 3: HALF_OPEN (Testing Recovery)                  │
─────────────────────────────────────                  │
                                                        │
Provider: openai-gpt4 ◄─────────────────────────────────┘
Circuit State: HALF_OPEN
Test Requests Allowed: 3
Success Count: 0/3

    │
    │ Request 10 ──> ALLOWED (Test #1)
    │                Send to provider
    │                Result: SUCCESS (200 OK, 1100ms)
    │                Success Count: 1/3 ←──────────────┐
    │                                                   │ Record Success
    │ Request 11 ──> REJECTED (Already 3 concurrent)   │ Check threshold
    │                Wait for test requests to complete │
    │                                                   │
    │ Request 12 ──> ALLOWED (Test #2)                 │
    │                Result: SUCCESS (200 OK, 1050ms)  │
    │                Success Count: 2/3 ←───────────────┘
    │
    │ Request 13 ──> ALLOWED (Test #3)
    │                Result: SUCCESS (200 OK, 1200ms)
    │                Success Count: 3/3 ←─────────────┐
    │                                                  │ SUCCESS THRESHOLD
    │                Transition: HALF_OPEN → CLOSED ←─┘   REACHED!
    ▼                Reset failure count                  Close circuit
BACK TO STATE 1: CLOSED                                   Emit recovery event
────────────────────────────

Provider: openai-gpt4
Circuit State: CLOSED
Failure Count: 0/5
Success Count: 3

    │ [Normal operation resumed]
    │
    │ Request 14 ──> SUCCESS (200 OK)
    │ Request 15 ──> SUCCESS (200 OK)
    │ ...
    ▼

───────────────────────────────────────────────────────────────────────────

ALTERNATIVE PATH: HALF_OPEN → OPEN (Re-opening on Failure)
──────────────────────────────────────────────────────────

STATE 3: HALF_OPEN
Provider: openai-gpt4
Success Count: 1/3

    │
    │ Request 16 ──> ALLOWED (Test #2)
    │                Result: FAILURE (Timeout) ←──────┐
    │                                                  │ Single failure in
    │                Transition: HALF_OPEN → OPEN ←───┘ HALF_OPEN = reopen
    ▼                Reset timeout to 60s
STATE 2: OPEN (Re-opened)

Provider: openai-gpt4
Circuit State: OPEN
Opened At: t=65000ms
Timeout: 60 seconds (exponential backoff)

    │ Circuit will retry again in 60s...
    ▼

───────────────────────────────────────────────────────────────────────────

Key Metrics Tracked:
  - Failure threshold: 5 consecutive failures
  - Success threshold: 3 consecutive successes in HALF_OPEN
  - Open timeout: 60 seconds
  - Half-open max concurrent: 3 requests
  - Sliding window: 10 seconds
  - Failure rate threshold: 50% within window
```

---

## 3. Data Transformation Pipeline

### 3.1 Request Normalization (Client → Gateway Format)

```rust
// ============================================================================
// REQUEST TRANSFORMATION: Client API → Gateway Internal Format
// ============================================================================

INPUT: Client Request (OpenAI Format)
─────────────────────────────────────
POST /v1/chat/completions
Content-Type: application/json

{
  "model": "gpt-4",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "What is the capital of France?"}
  ],
  "temperature": 0.7,
  "max_tokens": 150,
  "stream": false
}

STEP 1: Parse and Validate
───────────────────────────
┌────────────────────────────────────────────┐
│ JSON Deserialization                       │
│ - Parse JSON body                          │
│ - Validate schema against OpenAPI spec     │
│ - Check required fields: model, messages   │
│ - Validate types and bounds                │
└────────────────────────────────────────────┘
        ↓
struct ChatCompletionRequest {
    model: String,            // "gpt-4"
    messages: Vec<Message>,   // 2 messages
    temperature: Option<f32>, // Some(0.7)
    max_tokens: Option<u32>,  // Some(150)
    stream: bool,             // false
    // ... other optional fields
}

STEP 2: Normalize Model Name
─────────────────────────────
┌────────────────────────────────────────────┐
│ Model Alias Resolution                     │
│ Input: "gpt-4"                             │
│ ├─> Check alias table                      │
│ ├─> "gpt-4" → "gpt-4-0613" (latest)        │
│ └─> Canonical: "gpt-4-0613"                │
└────────────────────────────────────────────┘

STEP 3: Extract Metadata
─────────────────────────
┌────────────────────────────────────────────┐
│ Request Context Extraction                 │
│ - API Key: Extract from Authorization      │
│   header "Bearer sk-..."                   │
│ - Tenant ID: Lookup from API key hash      │
│ - User ID: Optional from headers           │
│ - Trace Context: W3C traceparent header    │
└────────────────────────────────────────────┘

STEP 4: Create Gateway Request
───────────────────────────────
┌────────────────────────────────────────────┐
│ Internal Request Construction              │
└────────────────────────────────────────────┘
        ↓
struct GatewayRequest {
    // Identity
    request_id: "req_abc123",
    trace_id: "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",

    // Model
    model: "gpt-4-0613",
    model_family: ModelFamily::GPT4,

    // Payload
    messages: vec![
        Message { role: "system", content: "..." },
        Message { role: "user", content: "..." },
    ],

    // Parameters
    temperature: Some(0.7),
    max_tokens: Some(150),
    top_p: None,
    frequency_penalty: None,
    presence_penalty: None,

    // Streaming
    stream: false,

    // Authentication
    api_key_hash: "hash_of_sk...",
    tenant_id: Some("tenant_123"),
    user_id: None,

    // Routing
    preferred_provider: None,
    excluded_providers: HashSet::new(),

    // Budget
    max_cost: None,
    max_latency: None,

    // Metadata
    timestamp: Instant::now(),
    priority: RequestPriority::Normal,
    retry_count: 0,
}

OUTPUT: Gateway Internal Request
─────────────────────────────────
Ready for middleware pipeline and routing
```

### 3.2 Provider Transformation (Gateway → Provider Format)

```rust
// ============================================================================
// PROVIDER TRANSFORMATION: Gateway Format → Provider-Specific Format
// ============================================================================

INPUT: Gateway Request (Normalized)
────────────────────────────────────
GatewayRequest {
    model: "gpt-4-0613",
    messages: [...],
    temperature: Some(0.7),
    max_tokens: Some(150),
    stream: false,
    ...
}

TRANSFORMATION PATH 1: OpenAI Provider
───────────────────────────────────────
┌────────────────────────────────────────────┐
│ Transform: Gateway → OpenAI Format         │
│                                            │
│ 1. Model Mapping:                          │
│    "gpt-4-0613" → "gpt-4-0613" (identity)  │
│                                            │
│ 2. Message Formatting:                     │
│    Gateway Message → OpenAI Message        │
│    (No transformation needed, compatible)  │
│                                            │
│ 3. Parameter Mapping:                      │
│    temperature: 0.7 → temperature: 0.7     │
│    max_tokens: 150 → max_tokens: 150       │
│                                            │
│ 4. Add Provider-Specific Fields:           │
│    - user: "tenant_123" (for tracking)     │
│    - n: 1 (default)                        │
│                                            │
└────────────────────────────────────────────┘
        ↓
OpenAI Request Body:
{
  "model": "gpt-4-0613",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "What is the capital of France?"}
  ],
  "temperature": 0.7,
  "max_tokens": 150,
  "stream": false,
  "user": "tenant_123"
}

HTTP Request:
POST https://api.openai.com/v1/chat/completions
Headers:
  Authorization: Bearer sk-openai-key-xyz
  Content-Type: application/json

TRANSFORMATION PATH 2: Anthropic Provider
──────────────────────────────────────────
┌────────────────────────────────────────────┐
│ Transform: Gateway → Anthropic Format      │
│                                            │
│ 1. Model Mapping:                          │
│    "gpt-4-0613" → "claude-3-opus-20240229" │
│    (Map to equivalent Anthropic model)     │
│                                            │
│ 2. Message Transformation:                 │
│    Gateway format → Anthropic format       │
│    - Extract system message separately     │
│    - Reformat user/assistant messages      │
│                                            │
│ 3. Parameter Mapping:                      │
│    temperature: 0.7 → temperature: 0.7     │
│    max_tokens: 150 → max_tokens: 150       │
│                                            │
│ 4. Protocol Differences:                   │
│    - system: "You are..." (separate field) │
│    - messages: [{role, content}]           │
│                                            │
└────────────────────────────────────────────┘
        ↓
Anthropic Request Body:
{
  "model": "claude-3-opus-20240229",
  "system": "You are a helpful assistant.",
  "messages": [
    {"role": "user", "content": "What is the capital of France?"}
  ],
  "temperature": 0.7,
  "max_tokens": 150,
  "stream": false
}

HTTP Request:
POST https://api.anthropic.com/v1/messages
Headers:
  x-api-key: sk-ant-key-xyz
  anthropic-version: 2023-06-01
  Content-Type: application/json

TRANSFORMATION PATH 3: Azure OpenAI Provider
─────────────────────────────────────────────
┌────────────────────────────────────────────┐
│ Transform: Gateway → Azure OpenAI Format   │
│                                            │
│ 1. Endpoint Construction:                  │
│    Base: https://{resource}.openai.azure.com│
│    Path: /openai/deployments/{deployment}/ │
│          chat/completions?api-version=2023-05-15│
│                                            │
│ 2. Model → Deployment Mapping:             │
│    "gpt-4-0613" → "my-gpt4-deployment"     │
│    (Lookup from config)                    │
│                                            │
│ 3. Request Body:                           │
│    (Same as OpenAI, mostly compatible)     │
│                                            │
│ 4. Authentication:                         │
│    api-key: {azure-api-key} (header)       │
│                                            │
└────────────────────────────────────────────┘
        ↓
Azure OpenAI Request:
POST https://my-resource.openai.azure.com/openai/deployments/my-gpt4-deployment/chat/completions?api-version=2023-05-15
Headers:
  api-key: azure-key-xyz
  Content-Type: application/json

Body: (Same as OpenAI format)
```

### 3.3 Response Normalization (Provider → Gateway Format)

```rust
// ============================================================================
// RESPONSE TRANSFORMATION: Provider-Specific → Gateway Format
// ============================================================================

RESPONSE PATH 1: OpenAI Provider Response
──────────────────────────────────────────
INPUT: OpenAI Response
{
  "id": "chatcmpl-8abc123",
  "object": "chat.completion",
  "created": 1701234567,
  "model": "gpt-4-0613",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "The capital of France is Paris."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 25,
    "completion_tokens": 8,
    "total_tokens": 33
  }
}

STEP 1: Parse Provider Response
────────────────────────────────
┌────────────────────────────────────────────┐
│ Deserialize JSON                           │
│ Validate response structure                │
│ Extract key fields                         │
└────────────────────────────────────────────┘

STEP 2: Normalize to Gateway Format
────────────────────────────────────
┌────────────────────────────────────────────┐
│ Gateway Response Construction              │
│                                            │
│ 1. Extract completion                      │
│ 2. Normalize token usage                  │
│ 3. Calculate cost                          │
│ 4. Add metadata                            │
└────────────────────────────────────────────┘
        ↓
struct GatewayResponse {
    // Response ID
    id: "chatcmpl-8abc123",

    // Content
    content: "The capital of France is Paris.",
    role: "assistant",
    finish_reason: FinishReason::Stop,

    // Token Usage
    usage: TokenUsage {
        prompt_tokens: 25,
        completion_tokens: 8,
        total_tokens: 33,
    },

    // Cost (calculated from provider pricing)
    cost: Cost {
        input_cost: 0.00075,  // $0.03/1K * 25 tokens
        output_cost: 0.00024, // $0.06/1K * 8 tokens
        total_cost: 0.00099,  // $0.00099
        currency: "USD",
    },

    // Provider metadata
    provider_id: "openai-gpt4-primary",
    provider_type: ProviderType::OpenAI,
    model_used: "gpt-4-0613",

    // Timing
    latency: Duration::from_millis(1234),
    created_at: SystemTime::from_unix(1701234567),

    // Tracing
    request_id: "req_abc123",
    trace_id: "00-4bf92f3577b34da6a3ce929d0e0e4736-...",
}

RESPONSE PATH 2: Anthropic Provider Response
─────────────────────────────────────────────
INPUT: Anthropic Response
{
  "id": "msg_01ABC123",
  "type": "message",
  "role": "assistant",
  "content": [
    {
      "type": "text",
      "text": "The capital of France is Paris."
    }
  ],
  "model": "claude-3-opus-20240229",
  "stop_reason": "end_turn",
  "usage": {
    "input_tokens": 23,
    "output_tokens": 9
  }
}

STEP 1: Transform Anthropic → Gateway
──────────────────────────────────────
┌────────────────────────────────────────────┐
│ Anthropic-Specific Transformations         │
│                                            │
│ 1. Extract text from content array         │
│    content[0].text → content string        │
│                                            │
│ 2. Map stop_reason:                        │
│    "end_turn" → FinishReason::Stop         │
│    "max_tokens" → FinishReason::Length     │
│    "stop_sequence" → FinishReason::Stop    │
│                                            │
│ 3. Token mapping:                          │
│    input_tokens → prompt_tokens            │
│    output_tokens → completion_tokens       │
│                                            │
│ 4. Calculate cost (Anthropic pricing):     │
│    Claude Opus: $15/$75 per 1M tokens      │
└────────────────────────────────────────────┘
        ↓
GatewayResponse {
    id: "msg_01ABC123",
    content: "The capital of France is Paris.",
    role: "assistant",
    finish_reason: FinishReason::Stop,

    usage: TokenUsage {
        prompt_tokens: 23,
        completion_tokens: 9,
        total_tokens: 32,
    },

    cost: Cost {
        input_cost: 0.000345,  // $15/1M * 23 tokens
        output_cost: 0.000675, // $75/1M * 9 tokens
        total_cost: 0.001020,  // $0.00102
        currency: "USD",
    },

    provider_id: "anthropic-opus-fallback",
    provider_type: ProviderType::Anthropic,
    model_used: "claude-3-opus-20240229",
    ...
}

STEP 2: Convert to Client API Format
─────────────────────────────────────
┌────────────────────────────────────────────┐
│ Gateway → OpenAI-Compatible Response       │
│ (For client consumption)                   │
└────────────────────────────────────────────┘
        ↓
OUTPUT: Client Response (OpenAI Format)
{
  "id": "msg_01ABC123",
  "object": "chat.completion",
  "created": 1701234567,
  "model": "gpt-4",  // Original requested model
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "The capital of France is Paris."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 23,
    "completion_tokens": 9,
    "total_tokens": 32
  },
  // Custom headers for transparency
  "x-gateway-provider": "anthropic-opus-fallback",
  "x-gateway-latency-ms": "1456",
  "x-gateway-cost": "0.001020"
}
```

### 3.4 Streaming Chunk Transformation

```rust
// ============================================================================
// STREAM TRANSFORMATION: Provider SSE → Client SSE
// ============================================================================

PROVIDER STREAM: OpenAI SSE Format
───────────────────────────────────
data: {"id":"chatcmpl-8abc","object":"chat.completion.chunk","created":1701234567,"model":"gpt-4-0613","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}

data: {"id":"chatcmpl-8abc","object":"chat.completion.chunk","created":1701234567,"model":"gpt-4-0613","choices":[{"index":0,"delta":{"content":"The"},"finish_reason":null}]}

data: {"id":"chatcmpl-8abc","object":"chat.completion.chunk","created":1701234567,"model":"gpt-4-0613","choices":[{"index":0,"delta":{"content":" capital"},"finish_reason":null}]}

data: {"id":"chatcmpl-8abc","object":"chat.completion.chunk","created":1701234567,"model":"gpt-4-0613","choices":[{"index":0,"delta":{"content":" of"},"finish_reason":null}]}

data: {"id":"chatcmpl-8abc","object":"chat.completion.chunk","created":1701234567,"model":"gpt-4-0613","choices":[{"index":0,"delta":{"content":" France"},"finish_reason":null}]}

data: {"id":"chatcmpl-8abc","object":"chat.completion.chunk","created":1701234567,"model":"gpt-4-0613","choices":[{"index":0,"delta":{"content":" is"},"finish_reason":null}]}

data: {"id":"chatcmpl-8abc","object":"chat.completion.chunk","created":1701234567,"model":"gpt-4-0613","choices":[{"index":0,"delta":{"content":" Paris"},"finish_reason":null}]}

data: {"id":"chatcmpl-8abc","object":"chat.completion.chunk","created":1701234567,"model":"gpt-4-0613","choices":[{"index":0,"delta":{"content":"."},"finish_reason":null}]}

data: {"id":"chatcmpl-8abc","object":"chat.completion.chunk","created":1701234567,"model":"gpt-4-0613","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]

GATEWAY TRANSFORMATION PIPELINE
────────────────────────────────
┌────────────────────────────────────────────┐
│ Stream Processing Pipeline                 │
│                                            │
│ 1. Provider Stream → Async Stream          │
│    - Parse SSE events                      │
│    - Deserialize JSON chunks               │
│    - Validate chunk structure              │
│                                            │
│ 2. Transform Each Chunk                    │
│    - Add gateway metadata                  │
│    - Track cumulative tokens               │
│    - Calculate incremental cost            │
│                                            │
│ 3. Backpressure Handling                   │
│    - Buffer chunks (bounded channel)       │
│    - Apply flow control                    │
│    - Handle slow clients                   │
│                                            │
│ 4. Error Recovery                          │
│    - Detect stream interruptions           │
│    - Emit error events                     │
│    - Close stream gracefully               │
└────────────────────────────────────────────┘

CLIENT STREAM: Gateway SSE Format
──────────────────────────────────
data: {"id":"chatcmpl-8abc","object":"chat.completion.chunk","created":1701234567,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}],"x_gateway":{"provider":"openai-gpt4","latency_ms":45}}

data: {"id":"chatcmpl-8abc","object":"chat.completion.chunk","created":1701234567,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"The"},"finish_reason":null}],"x_gateway":{"tokens":1}}

data: {"id":"chatcmpl-8abc","object":"chat.completion.chunk","created":1701234567,"model":"gpt-4","choices":[{"index":0,"delta":{"content":" capital"},"finish_reason":null}],"x_gateway":{"tokens":2}}

...

data: {"id":"chatcmpl-8abc","object":"chat.completion.chunk","created":1701234567,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":25,"completion_tokens":8,"total_tokens":33},"x_gateway":{"provider":"openai-gpt4","total_latency_ms":1234,"total_cost":"0.00099"}}

data: [DONE]

Backpressure Flow Control:
───────────────────────────
Provider → [Bounded Channel (1000 chunks)] → Gateway → Client
                    ↓
            If channel full:
            - Pause provider reads
            - Wait for consumer drain
            - Resume when space available
```

---

## 4. State Management

### 4.1 Stateless vs Stateful Components

```
┌────────────────────────────────────────────────────────────────────────────┐
│                    STATE MANAGEMENT ARCHITECTURE                           │
└────────────────────────────────────────────────────────────────────────────┘

STATELESS COMPONENTS (Request-scoped only)
──────────────────────────────────────────
┌─────────────────────────────────────┐
│ HTTP Request Handlers               │
│ - No instance state                 │
│ - Pure functions of request         │
│ - All state from GatewayState       │
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│ Request/Response Transformers       │
│ - Stateless conversion logic        │
│ - No side effects                   │
│ - Deterministic transformations     │
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│ JSON Validators                     │
│ - Schema-based validation           │
│ - No persistent state               │
└─────────────────────────────────────┘

STATEFUL COMPONENTS (Shared across requests)
────────────────────────────────────────────
┌─────────────────────────────────────┐
│ Provider Registry                   │
│ State:                              │
│ - DashMap<String, ProviderCandidate>│
│ - Atomic health scores              │
│ - Real-time metrics counters        │
│                                     │
│ Concurrency:                        │
│ - Lock-free reads (Arc)             │
│ - DashMap for concurrent updates    │
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│ Circuit Breakers (per provider)    │
│ State:                              │
│ - AtomicU8: circuit state           │
│ - AtomicU32: failure count          │
│ - AtomicU64: last failure timestamp │
│ - RwLock<SlidingWindow>: metrics    │
│                                     │
│ Concurrency:                        │
│ - Atomic CAS operations             │
│ - Lock for histogram updates        │
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│ Rate Limiters (per tenant/key)     │
│ State:                              │
│ - DashMap<TenantId, TokenBucket>    │
│ - AtomicU64: token count            │
│ - AtomicU64: last refill timestamp  │
│                                     │
│ Concurrency:                        │
│ - Atomic fetch_sub for tokens       │
│ - Lock-free token bucket algorithm  │
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│ Routing Table                       │
│ State:                              │
│ - DashMap<Model, Vec<Provider>>     │
│ - DashMap<ProviderId, Provider>     │
│ - AtomicU64: generation counter     │
│                                     │
│ Concurrency:                        │
│ - Immutable provider candidates     │
│ - Arc for shared references         │
│ - Copy-on-write for updates         │
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│ Metrics Aggregator                  │
│ State:                              │
│ - AtomicU64: request counters       │
│ - RwLock<Histogram>: latency        │
│ - DashMap: per-provider metrics     │
│                                     │
│ Concurrency:                        │
│ - Atomic increments                 │
│ - Read locks for histogram reads    │
│ - Write locks for updates           │
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│ Response Cache (optional)           │
│ State:                              │
│ - DashMap<CacheKey, CachedResponse> │
│ - LRU eviction metadata             │
│ - TTL expiration tracking           │
│                                     │
│ Concurrency:                        │
│ - DashMap for concurrent access     │
│ - Atomic TTL checks                 │
└─────────────────────────────────────┘
```

### 4.2 Shared State Patterns

```rust
// ============================================================================
// SHARED STATE PATTERNS: Arc + DashMap + Atomics
// ============================================================================

// Pattern 1: Arc for Immutable Shared State
// ──────────────────────────────────────────
struct ProviderCandidate {
    provider_id: String,          // Immutable
    endpoint_url: String,         // Immutable
    capabilities: Capabilities,   // Immutable

    // Mutable state via interior mutability
    health_score: Arc<AtomicCell<f64>>,
    active_connections: AtomicU32,
}

let candidate = Arc::new(ProviderCandidate { ... });

// Multiple threads can read immutable fields
let clone1 = Arc::clone(&candidate);
let clone2 = Arc::clone(&candidate);

tokio::spawn(async move {
    println!("Provider: {}", clone1.provider_id); // Immutable read
    clone1.active_connections.fetch_add(1, Ordering::Relaxed); // Atomic update
});

tokio::spawn(async move {
    let score = clone2.health_score.load(); // Lock-free read
});

// Pattern 2: DashMap for Concurrent HashMap
// ──────────────────────────────────────────
use dashmap::DashMap;

struct ProviderRegistry {
    providers: DashMap<String, Arc<ProviderCandidate>>,
}

impl ProviderRegistry {
    fn get(&self, id: &str) -> Option<Arc<ProviderCandidate>> {
        self.providers.get(id).map(|entry| Arc::clone(entry.value()))
        // DashMap handles locking internally, appears lock-free to caller
    }

    fn insert(&self, id: String, provider: Arc<ProviderCandidate>) {
        self.providers.insert(id, provider);
        // Concurrent inserts are safe
    }

    fn update_health(&self, id: &str, score: f64) {
        if let Some(provider) = self.providers.get(id) {
            provider.health_score.store(score);
            // No need to re-insert, interior mutability
        }
    }
}

// Pattern 3: Atomic Operations for Counters
// ──────────────────────────────────────────
struct RequestMetrics {
    total_requests: AtomicU64,
    successful_requests: AtomicU64,
    failed_requests: AtomicU64,
}

impl RequestMetrics {
    fn record_success(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.successful_requests.fetch_add(1, Ordering::Relaxed);
        // Lock-free, wait-free increments
    }

    fn success_rate(&self) -> f64 {
        let total = self.total_requests.load(Ordering::Relaxed);
        let successful = self.successful_requests.load(Ordering::Relaxed);

        if total == 0 {
            return 0.0;
        }
        successful as f64 / total as f64
        // Eventual consistency, no locks
    }
}

// Pattern 4: RwLock for Histograms (Read-heavy)
// ──────────────────────────────────────────────
use parking_lot::RwLock;
use hdrhistogram::Histogram;

struct LatencyTracker {
    histogram: RwLock<Histogram<u64>>,
}

impl LatencyTracker {
    fn record(&self, latency_micros: u64) {
        let mut hist = self.histogram.write();
        hist.record(latency_micros).ok();
        // Write lock required for updates
    }

    fn get_percentile(&self, percentile: f64) -> u64 {
        let hist = self.histogram.read();
        hist.value_at_percentile(percentile)
        // Read lock allows concurrent readers
    }
}

// Pattern 5: ArcSwap for Hot Configuration Reload
// ────────────────────────────────────────────────
use arc_swap::ArcSwap;

struct GatewayState {
    config: Arc<ArcSwap<GatewayConfig>>,
}

impl GatewayState {
    fn reload_config(&self, new_config: GatewayConfig) {
        self.config.store(Arc::new(new_config));
        // Atomic swap, readers see old or new (never partial)
    }

    fn get_config(&self) -> Arc<GatewayConfig> {
        self.config.load_full()
        // Wait-free read
    }
}

// Pattern 6: Channel for Async Communication
// ───────────────────────────────────────────
use tokio::sync::mpsc;

struct StreamProcessor {
    chunk_sender: mpsc::Sender<StreamChunk>,
    chunk_receiver: mpsc::Receiver<StreamChunk>,
}

impl StreamProcessor {
    async fn send_chunk(&self, chunk: StreamChunk) -> Result<()> {
        self.chunk_sender.send(chunk).await?;
        // Backpressure if receiver is slow
        Ok(())
    }

    async fn receive_chunk(&mut self) -> Option<StreamChunk> {
        self.chunk_receiver.recv().await
        // Async wait for next chunk
    }
}
```

### 4.3 State Synchronization Across Instances

```
┌────────────────────────────────────────────────────────────────────────────┐
│           STATE SYNCHRONIZATION (MULTI-INSTANCE DEPLOYMENT)                │
└────────────────────────────────────────────────────────────────────────────┘

Instance 1          Instance 2          Instance 3
┌────────┐         ┌────────┐         ┌────────┐
│Gateway │         │Gateway │         │Gateway │
│  Pod   │         │  Pod   │         │  Pod   │
└────┬───┘         └────┬───┘         └────┬───┘
     │                  │                  │
     │                  │                  │
     ├──────────────────┴──────────────────┤
     │    Shared State Synchronization     │
     └──────────────────┬──────────────────┘
                        │
            ┌───────────┴───────────┐
            │                       │
    ┌───────▼──────┐       ┌────────▼───────┐
    │ Redis Cluster│       │ etcd / Consul  │
    │              │       │                │
    │ - Rate Limit │       │ - Configuration│
    │   State      │       │ - Provider     │
    │ - Circuit    │       │   Registry     │
    │   Breaker    │       │ - Routing      │
    │   State      │       │   Rules        │
    │ - Metrics    │       │                │
    └──────────────┘       └────────────────┘

LOCAL vs DISTRIBUTED STATE
──────────────────────────

LOCAL STATE (Per-Instance)
├─> Request Context (thread-local)
├─> Connection Pools (HTTP clients)
├─> In-memory Caches (short TTL)
├─> Latency Histograms (aggregated to central)
└─> Active Request Tracking

DISTRIBUTED STATE (Cross-Instance)
├─> Rate Limit Buckets
│   └─> Redis: INCR/DECR operations
│       Script: Token bucket with atomic refill
│
├─> Circuit Breaker State
│   └─> Redis: Circuit state + failure counters
│       Pub/Sub: Notify other instances of state changes
│
├─> Provider Health Scores
│   └─> etcd: Health check results
│       Watch: Update local cache on changes
│
├─> Configuration
│   └─> etcd/Consul: Routing rules, middleware config
│       Watch: Hot reload on updates
│
└─> Metrics Aggregation
    └─> Prometheus: Time-series metrics
        Push Gateway: Instance-level metrics

SYNCHRONIZATION STRATEGIES
───────────────────────────

1. Rate Limiting (Redis Script)
   ──────────────────────────────
   -- Atomic token bucket implementation
   local key = KEYS[1]
   local max_tokens = tonumber(ARGV[1])
   local refill_rate = tonumber(ARGV[2])
   local requested = tonumber(ARGV[3])
   local now = tonumber(ARGV[4])

   local tokens = redis.call('HGET', key, 'tokens')
   local last_refill = redis.call('HGET', key, 'last_refill')

   if not tokens then
       tokens = max_tokens
       last_refill = now
   else
       tokens = tonumber(tokens)
       last_refill = tonumber(last_refill)
   end

   -- Refill tokens
   local elapsed = now - last_refill
   local new_tokens = math.min(max_tokens, tokens + (elapsed * refill_rate))

   if new_tokens >= requested then
       new_tokens = new_tokens - requested
       redis.call('HSET', key, 'tokens', new_tokens)
       redis.call('HSET', key, 'last_refill', now)
       redis.call('EXPIRE', key, 3600)
       return 1  -- Allowed
   else
       return 0  -- Denied
   end

2. Circuit Breaker (Redis + Pub/Sub)
   ──────────────────────────────────
   Instance 1:
   ──────────
   1. Detect failures locally
   2. Update Redis:
      SET circuit:provider-a:state "OPEN"
      SET circuit:provider-a:opened_at <timestamp>
   3. PUBLISH circuit:updates "provider-a:OPEN"

   Instance 2 & 3:
   ───────────────
   1. SUBSCRIBE circuit:updates
   2. Receive "provider-a:OPEN"
   3. Update local circuit breaker cache
   4. Reject requests without hitting provider

3. Configuration Sync (etcd Watch)
   ────────────────────────────────
   etcd:
   ────
   /gateway/config/routing_rules = {...}
   /gateway/config/providers = [...]

   Each Instance:
   ──────────────
   1. Watch /gateway/config/ prefix
   2. On change event:
      - Download new configuration
      - Validate configuration
      - Atomically swap (ArcSwap)
      - Apply to routing engine
   3. All instances converge within ~100ms

4. Metrics Aggregation (Prometheus)
   ─────────────────────────────────
   Each Instance:
   ──────────────
   - Maintain local counters/histograms
   - Expose /metrics endpoint

   Prometheus:
   ───────────
   - Scrape all instances
   - Aggregate with PromQL:
     sum(rate(gateway_requests_total[5m]))
     histogram_quantile(0.95, sum(rate(gateway_latency_bucket[5m])) by (le))

CONSISTENCY GUARANTEES
──────────────────────
┌────────────────────────────────────────┐
│ Component    │ Consistency Model       │
├──────────────┼─────────────────────────┤
│ Rate Limit   │ Strong Consistency      │
│              │ (Redis atomic ops)      │
├──────────────┼─────────────────────────┤
│ Circuit      │ Eventual Consistency    │
│ Breaker      │ (Pub/Sub propagation)   │
├──────────────┼─────────────────────────┤
│ Config       │ Eventual Consistency    │
│              │ (etcd watch + local)    │
├──────────────┼─────────────────────────┤
│ Health       │ Eventual Consistency    │
│ Scores       │ (periodic sync)         │
├──────────────┼─────────────────────────┤
│ Metrics      │ Eventual Consistency    │
│              │ (scrape interval)       │
└────────────────────────────────────────┘
```

---

## 5. Error Propagation

### 5.1 Error Flow Through Middleware

```
┌────────────────────────────────────────────────────────────────────────────┐
│                   ERROR PROPAGATION THROUGH LAYERS                         │
└────────────────────────────────────────────────────────────────────────────┘

ERROR SOURCE 1: Authentication Failure
───────────────────────────────────────
Client Request
    │
    ▼
┌─────────────────────────┐
│ Authentication Middleware│
│                         │
│ Check: API Key Invalid  │
│ Error: AuthError::      │
│   InvalidApiKey         │
└────────┬────────────────┘
         │
         │ ✗ SHORT-CIRCUIT (Early Return)
         │
         ▼
┌─────────────────────────┐
│ Error Handler           │
│                         │
│ Convert:                │
│ AuthError → GatewayError│
│ → HTTP Response         │
└────────┬────────────────┘
         │
         ▼
HTTP 401 Unauthorized
{
  "error": {
    "message": "Invalid API key",
    "type": "invalid_request_error",
    "param": null,
    "code": "invalid_api_key"
  }
}

ERROR SOURCE 2: Rate Limit Exceeded
────────────────────────────────────
Client Request
    │
    ▼
Auth ✓ → Rate Limit Check
            │
            │ Token Bucket: 0 tokens
            │
            ▼
         ┌─────────────────────────┐
         │ Rate Limit Middleware   │
         │                         │
         │ Error: RateLimitError:: │
         │   QuotaExceeded {       │
         │     retry_after: 6s     │
         │   }                     │
         └────────┬────────────────┘
                  │
                  │ ✗ SHORT-CIRCUIT
                  │
                  ▼
         ┌─────────────────────────┐
         │ Error Handler           │
         │                         │
         │ Convert to HTTP 429     │
         │ Add Retry-After header  │
         └────────┬────────────────┘
                  │
                  ▼
HTTP 429 Too Many Requests
Retry-After: 6
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 0
{
  "error": {
    "message": "Rate limit exceeded",
    "type": "rate_limit_error",
    "param": null,
    "code": "rate_limit_exceeded"
  }
}

ERROR SOURCE 3: Validation Error
─────────────────────────────────
Client Request
    │
    ▼
Auth ✓ → Rate Limit ✓ → Validation
                            │
                            │ Invalid: max_tokens = -1
                            │
                            ▼
                  ┌─────────────────────────┐
                  │ Validation Middleware   │
                  │                         │
                  │ Error: ValidationError::│
                  │   InvalidParameter {    │
                  │     param: "max_tokens",│
                  │     reason: "negative"  │
                  │   }                     │
                  └────────┬────────────────┘
                           │
                           │ ✗ SHORT-CIRCUIT
                           │
                           ▼
                  ┌─────────────────────────┐
                  │ Error Handler           │
                  │                         │
                  │ Convert to HTTP 400     │
                  └────────┬────────────────┘
                           │
                           ▼
HTTP 400 Bad Request
{
  "error": {
    "message": "Invalid parameter: max_tokens must be positive",
    "type": "invalid_request_error",
    "param": "max_tokens",
    "code": "invalid_parameter"
  }
}

ERROR SOURCE 4: Circuit Breaker Open
─────────────────────────────────────
Client Request
    │
    ▼
Auth ✓ → Rate Limit ✓ → Validation ✓ → Router ✓
                                            │
                                            ▼
                                  ┌─────────────────────────┐
                                  │ Circuit Breaker Check   │
                                  │                         │
                                  │ State: OPEN             │
                                  │ Provider: openai-gpt4   │
                                  │                         │
                                  │ Error: CircuitBreaker:: │
                                  │   Open {                │
                                  │     provider: "...",    │
                                  │     retry_after: 45s    │
                                  │   }                     │
                                  └────────┬────────────────┘
                                           │
                                           │ ✗ REJECT (no provider call)
                                           │
                                           ▼
                                  ┌─────────────────────────┐
                                  │ Failover Logic          │
                                  │                         │
                                  │ Try fallback provider?  │
                                  │ Yes → anthropic-opus    │
                                  └────────┬────────────────┘
                                           │
                                           │ SUCCESS on fallback
                                           ▼
                                  HTTP 200 OK (from fallback)
                                  X-Gateway-Provider: anthropic-opus
                                  X-Gateway-Failover: true

ERROR SOURCE 5: Provider Timeout
─────────────────────────────────
Client Request
    │
    ▼
Auth ✓ → ... → Router ✓ → Circuit Breaker ✓
                                    │
                                    ▼
                          ┌─────────────────────────┐
                          │ Provider API Call       │
                          │                         │
                          │ HTTP POST to OpenAI     │
                          │ Timeout: 30s            │
                          │                         │
                          │ ... [30 seconds] ...    │
                          │                         │
                          │ ✗ TIMEOUT!              │
                          │                         │
                          │ Error: ProviderError::  │
                          │   Timeout {             │
                          │     provider: "...",    │
                          │     duration: 30s       │
                          │   }                     │
                          └────────┬────────────────┘
                                   │
                                   │ Update Circuit Breaker
                                   │ Record Failure
                                   │
                                   ▼
                          ┌─────────────────────────┐
                          │ Retry Logic             │
                          │                         │
                          │ Attempt 1/3 failed      │
                          │ Backoff: 100ms          │
                          │                         │
                          │ Wait... Retry           │
                          └────────┬────────────────┘
                                   │
                                   │ SUCCESS on retry
                                   ▼
                          HTTP 200 OK
                          X-Gateway-Retries: 1

ERROR SOURCE 6: Provider Error (500)
─────────────────────────────────────
Provider API Call
    │
    ▼
┌─────────────────────────┐
│ Provider Response       │
│                         │
│ HTTP 500 Internal Error │
│ Body: {                 │
│   "error": {            │
│     "message": "..."    │
│   }                     │
│ }                       │
└────────┬────────────────┘
         │
         │ Parse provider error
         │
         ▼
┌─────────────────────────┐
│ Error Transformer       │
│                         │
│ ProviderError::         │
│   ServerError {         │
│     status: 500,        │
│     provider: "...",    │
│     body: "..."         │
│   }                     │
└────────┬────────────────┘
         │
         │ Circuit Breaker: Record Failure
         │ Retry Logic: Attempt retry
         │
         ▼
┌─────────────────────────┐
│ Client Error Response   │
│                         │
│ HTTP 502 Bad Gateway    │
│ {                       │
│   "error": {            │
│     "message":          │
│       "Upstream error", │
│     "type":             │
│       "api_error",      │
│     "code":             │
│       "provider_error"  │
│   }                     │
│ }                       │
└─────────────────────────┘

X-Gateway-Provider-Error: true
X-Gateway-Provider-Status: 500
```

### 5.2 Error Transformation at Boundaries

```rust
// ============================================================================
// ERROR TRANSFORMATION: Provider-Specific → Gateway → Client
// ============================================================================

// Boundary 1: Provider → Gateway Internal
// ────────────────────────────────────────

// OpenAI Error Response
{
  "error": {
    "message": "Incorrect API key provided",
    "type": "invalid_request_error",
    "param": null,
    "code": "invalid_api_key"
  }
}

↓ Transform ↓

enum ProviderError {
    Authentication {
        message: String,
        provider: String,
    },
    RateLimit {
        retry_after: Option<Duration>,
    },
    Timeout {
        duration: Duration,
    },
    ServerError {
        status: u16,
        body: String,
    },
    ...
}

↓ Convert ↓

enum GatewayError {
    ProviderAuthentication {
        provider_id: String,
        original_message: String,
    },
    ProviderRateLimit {
        provider_id: String,
        retry_after: Option<Duration>,
    },
    ProviderTimeout {
        provider_id: String,
        duration: Duration,
    },
    ...
}

// Boundary 2: Gateway Internal → Client Response
// ───────────────────────────────────────────────

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        match self {
            GatewayError::AuthenticationFailed { .. } => {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({
                        "error": {
                            "message": "Invalid API key",
                            "type": "invalid_request_error",
                            "param": null,
                            "code": "invalid_api_key"
                        }
                    }))
                ).into_response()
            }

            GatewayError::RateLimitExceeded { retry_after, .. } => {
                let mut response = (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({
                        "error": {
                            "message": "Rate limit exceeded",
                            "type": "rate_limit_error",
                            "param": null,
                            "code": "rate_limit_exceeded"
                        }
                    }))
                ).into_response();

                if let Some(retry) = retry_after {
                    response.headers_mut().insert(
                        "Retry-After",
                        retry.as_secs().to_string().parse().unwrap()
                    );
                }

                response
            }

            GatewayError::ProviderTimeout { provider_id, .. } => {
                (
                    StatusCode::GATEWAY_TIMEOUT,
                    Json(json!({
                        "error": {
                            "message": "Request timeout",
                            "type": "timeout_error",
                            "param": null,
                            "code": "timeout"
                        }
                    }))
                ).into_response()
            }

            GatewayError::CircuitBreakerOpen { provider_id, retry_after } => {
                let mut response = (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({
                        "error": {
                            "message": "Service temporarily unavailable",
                            "type": "service_unavailable",
                            "param": null,
                            "code": "circuit_breaker_open"
                        }
                    }))
                ).into_response();

                response.headers_mut().insert(
                    "Retry-After",
                    retry_after.as_secs().to_string().parse().unwrap()
                );

                response
            }

            _ => {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": {
                            "message": "Internal server error",
                            "type": "server_error",
                            "param": null,
                            "code": "internal_error"
                        }
                    }))
                ).into_response()
            }
        }
    }
}
```

### 5.3 Client-Facing Error Responses

```
┌────────────────────────────────────────────────────────────────────────────┐
│                     CLIENT ERROR RESPONSE CATALOG                          │
└────────────────────────────────────────────────────────────────────────────┘

HTTP 400 Bad Request
────────────────────
Causes:
  - Invalid JSON syntax
  - Missing required fields
  - Invalid parameter values
  - Unsupported model name

Example:
{
  "error": {
    "message": "Invalid parameter: temperature must be between 0 and 2",
    "type": "invalid_request_error",
    "param": "temperature",
    "code": "invalid_parameter"
  }
}

HTTP 401 Unauthorized
─────────────────────
Causes:
  - Missing API key
  - Invalid API key
  - Expired API key

Example:
{
  "error": {
    "message": "Invalid API key provided",
    "type": "invalid_request_error",
    "param": null,
    "code": "invalid_api_key"
  }
}

Headers:
  WWW-Authenticate: Bearer realm="Gateway API"

HTTP 403 Forbidden
──────────────────
Causes:
  - Tenant not authorized for model
  - Quota exceeded
  - IP whitelist violation

Example:
{
  "error": {
    "message": "You do not have access to model gpt-4",
    "type": "permission_error",
    "param": "model",
    "code": "model_not_accessible"
  }
}

HTTP 404 Not Found
──────────────────
Causes:
  - Invalid endpoint path
  - Non-existent model

Example:
{
  "error": {
    "message": "The model 'gpt-5' does not exist",
    "type": "invalid_request_error",
    "param": "model",
    "code": "model_not_found"
  }
}

HTTP 429 Too Many Requests
───────────────────────────
Causes:
  - Rate limit exceeded (requests/min)
  - Rate limit exceeded (tokens/min)

Example:
{
  "error": {
    "message": "Rate limit exceeded. Please try again in 6 seconds.",
    "type": "rate_limit_error",
    "param": null,
    "code": "rate_limit_exceeded"
  }
}

Headers:
  Retry-After: 6
  X-RateLimit-Limit: 100
  X-RateLimit-Remaining: 0
  X-RateLimit-Reset: 1701234573

HTTP 500 Internal Server Error
───────────────────────────────
Causes:
  - Unhandled exceptions
  - Database connection failures
  - Configuration errors

Example:
{
  "error": {
    "message": "An internal error occurred",
    "type": "server_error",
    "param": null,
    "code": "internal_error"
  }
}

Headers:
  X-Request-ID: req_abc123def456

HTTP 502 Bad Gateway
────────────────────
Causes:
  - Provider returned invalid response
  - Provider connection failed
  - Provider returned 5xx error

Example:
{
  "error": {
    "message": "The upstream provider returned an error",
    "type": "api_error",
    "param": null,
    "code": "provider_error"
  }
}

Headers:
  X-Gateway-Provider: openai-gpt4
  X-Gateway-Provider-Status: 500

HTTP 503 Service Unavailable
─────────────────────────────
Causes:
  - Circuit breaker open
  - All providers unavailable
  - Graceful shutdown in progress

Example:
{
  "error": {
    "message": "The service is temporarily unavailable. Please try again later.",
    "type": "service_unavailable",
    "param": null,
    "code": "service_unavailable"
  }
}

Headers:
  Retry-After: 60
  X-Gateway-Circuit-Breaker: open

HTTP 504 Gateway Timeout
────────────────────────
Causes:
  - Provider timeout
  - Request processing timeout
  - Streaming timeout

Example:
{
  "error": {
    "message": "The request timed out after 30 seconds",
    "type": "timeout_error",
    "param": null,
    "code": "timeout"
  }
}

Headers:
  X-Gateway-Timeout-Type: provider
  X-Gateway-Timeout-Duration: 30000
```

---

## 6. Performance Characteristics

### 6.1 Latency Breakdown

```
┌────────────────────────────────────────────────────────────────────────────┐
│                        LATENCY BREAKDOWN BY COMPONENT                      │
└────────────────────────────────────────────────────────────────────────────┘

P50 Latency (Median)
────────────────────
TLS Handshake (new connection):      0-2ms    ━━░░░░░░░░░░░░░░░░░░░░
HTTP Parsing:                        0.1ms    ░░░░░░░░░░░░░░░░░░░░░░
Request ID Generation:               0.05ms   ░░░░░░░░░░░░░░░░░░░░░░
Authentication (cache hit):          0.2ms    ░░░░░░░░░░░░░░░░░░░░░░
Rate Limiting:                       0.1ms    ░░░░░░░░░░░░░░░░░░░░░░
Request Validation:                  0.3ms    ░░░░░░░░░░░░░░░░░░░░░░
Router Selection:                    0.5ms    ━░░░░░░░░░░░░░░░░░░░░░
Circuit Breaker Check:               0.05ms   ░░░░░░░░░░░░░░░░░░░░░░
Request Transform:                   0.4ms    ░░░░░░░░░░░░░░░░░░░░░░
Provider API Call:                   1200ms   ━━━━━━━━━━━━━━━━━━━━━━
Response Transform:                  0.6ms    ━░░░░░░░░░░░░░░░░░░░░░
Metrics Collection:                  0.2ms    ░░░░░░░░░░░░░░░░░░░░░░
Response Serialization:              0.3ms    ░░░░░░░░░░░░░░░░░░░░░░
────────────────────────────────────────────────────────────────────
Total Gateway Overhead (P50):        3.1ms    ██░░░░░░░░░░░░░░░░░░░░
Provider Latency (P50):              1200ms   ████████████████████░░
Total Request Latency (P50):         1203ms   ████████████████████░░

Gateway Overhead: 0.26% of total

P95 Latency
───────────
TLS Handshake:                       0-2ms
HTTP Parsing:                        0.2ms
Request ID Generation:               0.1ms
Authentication (cache miss + DB):    2.5ms    ████░░░░░░░░░░░░░░░░░░
Rate Limiting:                       0.3ms
Request Validation:                  0.8ms    ██░░░░░░░░░░░░░░░░░░░░
Router Selection:                    1.2ms    ███░░░░░░░░░░░░░░░░░░░
Circuit Breaker Check:               0.1ms
Request Transform:                   1.0ms    ██░░░░░░░░░░░░░░░░░░░░
Provider API Call:                   2500ms   ██████████████████████
Response Transform:                  1.5ms    ███░░░░░░░░░░░░░░░░░░░
Metrics Collection:                  0.5ms    █░░░░░░░░░░░░░░░░░░░░░
Response Serialization:              0.8ms    ██░░░░░░░░░░░░░░░░░░░░
────────────────────────────────────────────────────────────────────
Total Gateway Overhead (P95):        9.0ms    ██████░░░░░░░░░░░░░░░░
Provider Latency (P95):              2500ms   ████████████████████░░
Total Request Latency (P95):         2509ms   ████████████████████░░

Gateway Overhead: 0.36% of total

P99 Latency
───────────
Total Gateway Overhead (P99):        25ms     (includes GC pauses, context switches)
Provider Latency (P99):              5000ms
Total Request Latency (P99):         5025ms

Gateway Overhead: 0.50% of total

Streaming Latency
─────────────────
Time to First Byte (TTFB):          50ms     ██████████░░░░░░░░░░░░
  Gateway Overhead:                  5ms      █░░░░░░░░░░░░░░░░░░░░░
  Provider TTFB:                     45ms     █████████░░░░░░░░░░░░░

Time Between Chunks:                70ms     ██████████████░░░░░░░░
  Gateway Overhead per chunk:        <1ms     ░░░░░░░░░░░░░░░░░░░░░░
  Provider chunk generation:         ~70ms    ██████████████░░░░░░░░
```

### 6.2 Throughput Metrics

```
┌────────────────────────────────────────────────────────────────────────────┐
│                          THROUGHPUT CHARACTERISTICS                        │
└────────────────────────────────────────────────────────────────────────────┘

Single Instance Capacity
────────────────────────
CPU: 4 cores
Memory: 8 GB
Network: 10 Gbps

┌────────────────────────────────────┐
│ Concurrent Connections             │
├────────────────────────────────────┤
│ Maximum:          50,000           │
│ Typical (prod):   10,000           │
│ Per core:         2,500            │
└────────────────────────────────────┘

┌────────────────────────────────────┐
│ Requests per Second (Non-Streaming)│
├────────────────────────────────────┤
│ Theoretical Max:  20,000 RPS       │
│ Sustained (avg):  12,000 RPS       │
│ With retries:     10,000 RPS       │
│ Per core:         3,000 RPS        │
└────────────────────────────────────┘

┌────────────────────────────────────┐
│ Streaming Connections              │
├────────────────────────────────────┤
│ Maximum:          5,000            │
│ Typical:          2,000            │
│ Chunk rate:       ~14 chunks/sec   │
│                   per stream       │
└────────────────────────────────────┘

Resource Utilization at Load
─────────────────────────────
At 10,000 RPS (non-streaming):
  CPU: 70-80% (3.2 cores)
  Memory: 4 GB (50% of available)
  Network: 2 Gbps ingress, 3 Gbps egress
  File Descriptors: 12,000 / 65,536

At 2,000 streaming connections:
  CPU: 40-50% (2 cores)
  Memory: 6 GB (75% - buffer overhead)
  Network: 1 Gbps ingress, 4 Gbps egress
  File Descriptors: 5,000 / 65,536

Horizontal Scaling
──────────────────
┌────────────┬──────────┬──────────┬──────────┐
│ Instances  │ Total RPS│ Conn     │ Efficiency│
├────────────┼──────────┼──────────┼──────────┤
│ 1          │ 10,000   │ 10,000   │ 100%     │
│ 2          │ 19,500   │ 20,000   │ 97.5%    │
│ 4          │ 38,000   │ 40,000   │ 95%      │
│ 8          │ 74,000   │ 80,000   │ 92.5%    │
│ 16         │ 144,000  │ 160,000  │ 90%      │
└────────────┴──────────┴──────────┴──────────┘

Scaling efficiency loss due to:
  - Distributed state synchronization (Redis)
  - Circuit breaker state propagation
  - Configuration sync overhead

Memory Usage Patterns
──────────────────────
Base Memory (idle):                  500 MB
Per request (non-streaming):         ~10 KB
Per streaming connection:            ~500 KB (buffers)
Provider candidate metadata:         ~1 KB per provider
Metrics histograms:                  ~50 MB total
Connection pool overhead:            ~100 MB

Total at 10,000 concurrent:
  Non-streaming: 500 MB + 100 MB = 600 MB
  Streaming (2,000): 500 MB + 1 GB = 1.5 GB

Network Bandwidth Requirements
───────────────────────────────
Average Request Size:                2 KB
Average Response Size:               5 KB

At 10,000 RPS:
  Ingress: 10,000 * 2 KB = 20 MB/s = 160 Mbps
  Egress:  10,000 * 5 KB = 50 MB/s = 400 Mbps
  Total:                              560 Mbps

At 2,000 streaming:
  Ingress: 2,000 * 2 KB = 4 MB/s =   32 Mbps
  Egress:  2,000 * ~500 KB/s =      1000 MB/s = 8 Gbps
  Total:                              8.03 Gbps

Bottleneck Analysis
───────────────────
At low load (<1,000 RPS):
  Bottleneck: None (CPU < 20%)

At medium load (1,000-5,000 RPS):
  Bottleneck: Provider API latency

At high load (5,000-10,000 RPS):
  Bottleneck: CPU (routing + serialization)

At very high load (>10,000 RPS):
  Bottleneck: Network bandwidth (especially streaming)

Optimization Opportunities:
  - Use faster JSON serialization (simd-json)
  - Optimize routing table lookups (cached Arc)
  - Reduce memory allocations (object pooling)
  - Compress responses (gzip/brotli)
```

---

## Conclusion

This comprehensive data flow and sequence documentation provides:

1. **Complete Request Lifecycle**: Detailed breakdown of all processing stages from client request to response, with timing annotations and component interactions.

2. **Sequence Diagrams**: ASCII-based sequence diagrams for key scenarios including successful requests, streaming, failover, rate limiting, and circuit breaker state transitions.

3. **Data Transformation Pipeline**: Detailed transformations at each boundary (Client → Gateway → Provider → Client) with concrete examples for OpenAI, Anthropic, and Azure OpenAI providers.

4. **State Management**: Comprehensive coverage of stateless vs stateful components, shared state patterns using Arc/DashMap/Atomics, and multi-instance synchronization strategies.

5. **Error Propagation**: Complete error flow through middleware layers, error transformation at boundaries, and client-facing error response catalog.

6. **Performance Characteristics**: Detailed latency breakdown by component, throughput metrics, resource utilization patterns, and scaling characteristics.

This documentation serves as the authoritative reference for understanding the LLM-Inference-Gateway's runtime behavior, data flows, and operational characteristics.
