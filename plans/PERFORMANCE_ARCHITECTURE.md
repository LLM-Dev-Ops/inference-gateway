# Performance & Scalability Architecture

**Target**: <5ms p95 latency, 10,000+ RPS

## 1. Performance Architecture

### 1.1 Latency Optimization

**Zero-Copy Operations**
- Buffer sharing between request/response pipelines
- Direct memory mapping for large payloads (>1MB)
- Avoid JSON serialization in hot path using protobuf/msgpack
- Minimize data transformations: parse once, validate once, forward

**Connection Pooling**
```
HTTP/2 Connection Pool Configuration:
- Min connections per backend: 10
- Max connections per backend: 100
- Max streams per connection: 100
- Idle timeout: 300s
- Connection reuse: mandatory
```

**Async I/O**
- Fully non-blocking I/O using tokio/async runtime
- Event-driven request handling (no thread-per-request)
- Batched syscalls (io_uring on Linux)
- Zero-allocation futures for common paths
- Request pipelining without head-of-line blocking

### 1.2 Throughput Optimization

**Worker Architecture**
```
┌─────────────────────────────────────┐
│  Load Balancer (HAProxy/Envoy)      │
└──────────────┬──────────────────────┘
               │
       ┌───────┴───────┐
       │               │
   ┌───▼───┐       ┌───▼───┐
   │ Node 1│       │ Node 2│
   │       │       │       │
   │ CPU 0 │       │ CPU 0 │
   │ CPU 1 │       │ CPU 1 │
   │  ...  │       │  ...  │
   └───┬───┘       └───┬───┘
       │               │
   ┌───▼───────────────▼───┐
   │   Backend LLM Pool    │
   └───────────────────────┘
```

**Worker Thread Strategy**
- 1 worker per CPU core (avoid oversubscription)
- Pin workers to cores (CPU affinity)
- Dedicated I/O threads separate from compute
- Lock-free message passing between workers

**Request Pipelining**
- Batch validation across multiple requests
- Parallel backend health checks
- Concurrent prompt template rendering
- Micro-batch inference requests (10-50ms windows)

## 2. Scalability Patterns

### 2.1 Horizontal Scaling (Primary Strategy)

**Stateless Design Requirements**
- No in-memory session storage
- Configuration loaded from distributed store (etcd/Consul)
- Metrics exported to centralized collector (Prometheus)
- Request tracking via correlation IDs (not local state)

**Scaling Triggers**
```
Scale Up:   CPU > 70% OR RPS > 8000 OR p95 latency > 4ms
Scale Down: CPU < 30% AND RPS < 3000 AND p95 latency < 2ms
Cooldown:   5 minutes between scale operations
```

**Load Distribution**
- Consistent hashing for cache affinity
- Least-connections for backend routing
- Weighted round-robin with health checks
- Geographic routing for multi-region deployments

### 2.2 Vertical Scaling Limits

**Single Instance Ceiling**
```
Max RPS per instance:     2,000-3,000 RPS
Recommended instance:     16 vCPU, 32GB RAM
Network bandwidth:        10 Gbps minimum
Disk I/O:                 Not a bottleneck (minimal logging)
```

**When to Stop Vertical Scaling**
- Beyond 32 cores: diminishing returns
- Cost per RPS exceeds horizontal scaling
- Single point of failure risk increases

## 3. Caching Strategy

### 3.1 Multi-Layer Cache Architecture

**L1: Response Cache (In-Memory)**
```
Type:           LRU with TTL
Size:           4GB per instance
TTL:            300s (configurable)
Key:            SHA256(prompt + model + parameters)
Hit Rate Target: >40% for production traffic
Eviction:       LRU + size-based
```

**L2: Connection Cache**
```
Backend Connections:
- HTTP/2 persistent connections
- DNS cache: 300s TTL
- TLS session cache: 3600s TTL
- Keep-alive: enabled (300s timeout)
```

**L3: Configuration Cache**
```
Cache:          Model configs, routing rules, rate limits
Storage:        Local memory + Redis fallback
Refresh:        Every 60s (background)
Invalidation:   Pub/Sub on config changes
```

### 3.2 Cache Invalidation

**Strategies**
- TTL-based expiration (primary method)
- Pub/Sub for immediate invalidation on config changes
- Version tags for model updates
- Graceful degradation on cache miss (fetch + cache)

**Consistency**
```
Pattern: Cache-Aside
1. Check local cache
2. On miss, check Redis (L2)
3. On miss, fetch from source
4. Write to both cache layers
5. Return to client
```

## 4. Resource Management

### 4.1 Connection Pool Sizing

**Backend LLM Connections**
```
Per Backend Pool:
- Min connections:  5
- Max connections:  50
- Per-route limit:  100 concurrent
- Queue size:       1000 requests
- Timeout:          30s request, 5s connect
```

**Calculation Formula**
```
Connections = (Expected RPS × p95 Backend Latency) / 1000
Example: (10,000 × 0.5s) / 1000 = 50 connections
Add 20% buffer = 60 connections
```

### 4.2 Memory Management

**Allocation Strategy**
```
Total Memory (32GB instance):
├─ Process heap:          8GB
├─ Response cache:        4GB
├─ Connection buffers:    2GB
├─ Request buffers:       2GB
├─ OS/Network buffers:    4GB
└─ Reserve:              12GB
```

**GC Tuning (if using Go/Java)**
```
Go:   GOGC=100, frequent small GCs
Java: G1GC, -Xmx8g -Xms8g, pause target 10ms
Rust: No GC, careful with Arc/Box allocations
```

**Memory Limits**
- Max request size: 10MB
- Max response cache entry: 2MB
- Buffer pool: 1000 pre-allocated 64KB buffers

### 4.3 CPU Optimization

**Core Allocation**
```
16-core instance:
├─ Worker threads:     12 cores (request handling)
├─ I/O threads:        2 cores (network, disk)
├─ Background tasks:   1 core (metrics, health)
└─ OS/Reserve:         1 core
```

**Optimization Techniques**
- CPU pinning (taskset/cgroup)
- NUMA-aware memory allocation
- Branch prediction hints in hot paths
- SIMD for JSON parsing (simdjson)
- Profile-guided optimization (PGO)

## 5. Performance SLOs

| Metric | Target | P50 | P95 | P99 | Measurement |
|--------|--------|-----|-----|-----|-------------|
| **Latency (cache hit)** | <2ms | 0.5ms | 1.5ms | 3ms | Histogram, 1min windows |
| **Latency (cache miss)** | <5ms | 2ms | 4ms | 8ms | Histogram, 1min windows |
| **Throughput** | >10,000 RPS | 12,000 | 15,000 | 18,000 | Counter, sustained 5min |
| **Error Rate** | <0.1% | 0.01% | 0.05% | 0.1% | Rate over 5min window |
| **Backend Connection Time** | <100ms | 20ms | 80ms | 150ms | Histogram per backend |
| **Cache Hit Rate** | >40% | 45% | 50% | 55% | Ratio over 5min |
| **CPU Utilization** | <75% | 50% | 70% | 80% | Gauge, per instance |
| **Memory Utilization** | <80% | 60% | 75% | 85% | Gauge, per instance |
| **Concurrent Connections** | <5000 | 2000 | 4000 | 6000 | Gauge, per instance |

**Alerting Thresholds**
- Critical: p95 latency > 8ms for 2 minutes
- Warning: p95 latency > 5ms for 5 minutes
- Critical: Error rate > 1% for 1 minute
- Warning: Cache hit rate < 30% for 10 minutes

## 6. Load Testing Strategy

### 6.1 Benchmark Types

**1. Baseline Performance**
```
Objective: Establish single-instance limits
Duration:  30 minutes
Profile:   Constant 1000 RPS, ramp to failure
Metrics:   Latency percentiles, error rate, resource usage
```

**2. Sustained Load**
```
Objective: Verify stability at target RPS
Duration:  4 hours
Profile:   10,000 RPS constant
Validate:  No memory leaks, stable latency, <0.1% errors
```

**3. Spike Testing**
```
Objective: Test autoscaling and burst capacity
Duration:  1 hour
Profile:   1000 → 20,000 → 1000 RPS (5min cycles)
Validate:  Graceful scaling, no dropped requests
```

**4. Soak Testing**
```
Objective: Long-term stability
Duration:  24-48 hours
Profile:   8,000 RPS with ±20% variance
Validate:  No degradation over time
```

**5. Cache Effectiveness**
```
Objective: Measure cache hit rate impact
Duration:  1 hour
Profile:   Realistic traffic pattern (Zipf distribution)
Variants:  Cache enabled vs disabled
```

### 6.2 Tools

**Criterion (Rust Microbenchmarks)**
```rust
// Benchmark critical path components
criterion_group!(
    benches,
    bench_request_parsing,
    bench_cache_lookup,
    bench_response_serialization,
    bench_routing_decision
);

Target: <100μs per operation
```

**k6 Load Testing**
```javascript
// Realistic load test scenario
export let options = {
  stages: [
    { duration: '2m', target: 2000 },   // Ramp up
    { duration: '5m', target: 10000 },  // Target load
    { duration: '2m', target: 15000 },  // Peak
    { duration: '5m', target: 10000 },  // Sustained
    { duration: '2m', target: 0 }       // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<5'],
    http_req_failed: ['rate<0.001']
  }
};
```

**Additional Tools**
- wrk2: Constant-rate HTTP benchmarking
- hey: Simple load generator with latency histograms
- Apache Bench: Quick baseline tests
- Locust: Python-based distributed load testing

### 6.3 Test Data Generation

```
Prompt Corpus:
- Size: 10,000 unique prompts
- Distribution: Zipf (realistic cache behavior)
- Sizes: 100B to 10KB (realistic range)
- Models: Mix of GPT-4, Claude, Llama weights
- Cache hit target: 40-50%
```

## 7. Bottleneck Analysis

### 7.1 Common Bottlenecks

**Network I/O**
```
Symptoms: High network wait time, low CPU
Solutions:
- Enable HTTP/2 multiplexing
- Increase connection pool size
- Use Unix domain sockets for local backends
- Enable TCP Fast Open
```

**CPU-Bound**
```
Symptoms: >80% CPU, latency increases linearly with load
Solutions:
- Optimize hot paths (profiling)
- Reduce allocations
- Use SIMD for parsing
- Consider horizontal scaling
```

**Memory Pressure**
```
Symptoms: Increased GC pause, swap usage, OOM kills
Solutions:
- Reduce cache size
- Implement backpressure
- Tune GC parameters
- Add more RAM or scale horizontally
```

**Backend Latency**
```
Symptoms: Gateway latency tracks backend, queueing
Solutions:
- Increase backend pool
- Implement circuit breakers
- Add request timeout/retry logic
- Load balance across more backends
```

**Lock Contention**
```
Symptoms: Low CPU with high latency, uneven core usage
Solutions:
- Use lock-free data structures
- Partition shared state
- Per-core caching
- Reduce critical section size
```

### 7.2 Profiling Methodology

**Continuous Profiling**
```
Production:
- CPU profiling: 1% sampling overhead (pprof/perf)
- Memory profiling: On-demand snapshots
- Flame graphs: Real-time in dashboard
```

**Performance Regression Detection**
```
CI Pipeline:
1. Run benchmark suite on every commit
2. Compare to baseline (last release)
3. Alert if >5% regression in critical paths
4. Store historical data for trend analysis
```

**Tools by Language**
```
Rust:  perf, flamegraph, criterion, valgrind/cachegrind
Go:    pprof, trace, benchstat
Node:  --prof, clinic.js, 0x
Java:  JMC, async-profiler, JMH
```

**Profiling Checklist**
```
1. CPU Profile
   - Identify hot functions (>1% CPU)
   - Check for unexpected allocations
   - Verify async overhead is minimal

2. Memory Profile
   - Track allocation rate (target: <100MB/s)
   - Identify leak candidates
   - Check buffer pool effectiveness

3. I/O Profile
   - Network wait time breakdown
   - Connection pool utilization
   - Syscall frequency

4. Off-CPU Analysis
   - Lock wait time
   - I/O wait time
   - Scheduler delays
```

### 7.3 Optimization Priority

**P0: Critical Path (<2ms target)**
```
1. Request parsing and validation
2. Cache lookup
3. Routing decision
4. Response serialization
```

**P1: High Impact (2-5ms target)**
```
1. Backend connection acquisition
2. Authentication/authorization
3. Metrics collection
4. Logging (async only)
```

**P2: Background (>5ms acceptable)**
```
1. Health checks
2. Configuration reload
3. Cache eviction
4. Metrics aggregation
```

## Implementation Priorities

**Phase 1: Foundation (Week 1-2)**
- Async I/O framework
- Connection pooling
- Basic routing

**Phase 2: Performance (Week 3-4)**
- Response caching
- Zero-copy buffers
- Load testing infrastructure

**Phase 3: Scalability (Week 5-6)**
- Horizontal scaling support
- Circuit breakers
- Advanced metrics

**Phase 4: Optimization (Week 7-8)**
- Profile-guided optimization
- SIMD/platform-specific tuning
- Production load testing

---

**Document Version**: 1.0
**Last Updated**: 2025-11-27
**Owner**: Performance Engineering Team
