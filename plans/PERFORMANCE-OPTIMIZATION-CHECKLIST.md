# Performance Optimization Checklist

Comprehensive performance refinement guide for LLM Inference Gateway optimization.

**Target:** 12,500+ RPS, P95 < 120ms, < 0.01% error rate

---

## 1. HTTP Server Optimizations

### TCP/IP Stack
- [ ] **TCP_NODELAY enabled** - Disable Nagle's algorithm for low-latency responses
  - Implementation: `HttpConnector::set_nodelay(true)`
  - Impact: -5-10ms per request
  - Measurement: Track P50/P95 latency before/after

- [ ] **SO_REUSEPORT configured** - Enable multi-listener socket binding
  - Implementation: Set socket option at listener creation
  - Impact: Better load distribution across threads
  - Measurement: CPU utilization per core

- [ ] **Optimal worker thread count** - Match CPU core count (8-16 workers)
  - Implementation: `#[tokio::main(flavor = "multi_thread", worker_threads = 8)]`
  - Impact: Maximum parallelism without context switching overhead
  - Measurement: Thread contention metrics

- [ ] **Connection keep-alive tuned** - Set to 60s for connection reuse
  - Implementation: `pool_idle_timeout(Duration::from_secs(60))`
  - Impact: Reduce connection establishment overhead
  - Measurement: Active vs idle connections

- [ ] **Request body size limits** - Enforce max 10MB per request
  - Implementation: Body::limited(10 * 1024 * 1024)
  - Impact: Prevent memory exhaustion attacks
  - Measurement: Memory spikes during load tests

- [ ] **Response compression (conditional)** - Enable for >1KB responses
  - Implementation: Tower compression middleware with size threshold
  - Impact: -60% bandwidth, +2-3ms CPU overhead
  - Measurement: Response size vs latency tradeoff

### Axum Framework Configuration
- [ ] **Router optimization** - Use static routes over regex patterns
  - Implementation: Prefer `/v1/chat` over `/v1/:version/chat`
  - Impact: -50μs routing decision time
  - Measurement: Flamegraph analysis of route matching

- [ ] **Middleware stack ordering** - Place auth before heavy middleware
  - Implementation: Order: auth → rate limit → logging → compression
  - Impact: Fail fast on unauthorized requests
  - Measurement: P99 latency for rejected requests

- [ ] **Response pooling** - Reuse Response objects
  - Implementation: ObjectPool<Response> with pre-allocated capacity
  - Impact: -100μs allocation time per response
  - Measurement: Allocations/sec metrics

---

## 2. Memory Optimizations

### Allocation Strategy
- [ ] **Pre-allocated buffers** - Pool 64KB buffers for request/response
  - Implementation: `BytesMut::with_capacity(65536)` reuse pool
  - Impact: -200μs per request
  - Measurement: Heap allocations via `heaptrack`

- [ ] **Object pooling for requests** - Pool GatewayRequest/Response structs
  - Implementation: `deadpool` with 100-500 objects per pool
  - Impact: -150μs allocation overhead
  - Measurement: Pool hit rate >95%

- [ ] **Zero-copy where possible** - Use `Bytes` instead of `Vec<u8>`
  - Implementation: Replace all `Vec<u8>` with `Bytes` in hot paths
  - Impact: Eliminate memcpy for HTTP bodies
  - Measurement: CPU cache misses reduction

- [ ] **Avoid String allocations in hot path** - Use `&str` references
  - Implementation: `request.model.as_str()` instead of `.clone()`
  - Impact: -50μs per request
  - Measurement: Flamegraph string allocation hotspots

- [ ] **SmallVec for small collections** - Use for messages array (<8 items)
  - Implementation: `SmallVec<[Message; 8]>` instead of `Vec<Message>`
  - Impact: Stack allocation for common case
  - Measurement: Heap allocations count

- [ ] **Arena allocation for request scope** - Single allocator per request
  - Implementation: `bumpalo::Bump` arena reset after request completion
  - Impact: -100μs allocation overhead
  - Measurement: Allocator fragmentation metrics

### Memory Budget Enforcement
- [ ] **Per-request limit: <10KB** - Track with custom allocator wrapper
  - Measurement: `jemalloc` stats per request context

- [ ] **Per-connection limit: <50KB** - Monitor with connection metadata
  - Measurement: Connection count × average memory

- [ ] **Provider pool limit: <1MB** - Size connection pools appropriately
  - Current: 100 connections × ~10KB each
  - Measurement: Pool statistics via telemetry

- [ ] **Cache budget: <100MB** - Redis memory limit with eviction policy
  - Implementation: `maxmemory 100mb` with `allkeys-lru`
  - Measurement: Redis INFO memory stats

- [ ] **Total RSS baseline: <50MB** - Container memory reservation
  - Implementation: Kubernetes resource limits
  - Measurement: `ps aux | grep gateway` RSS column

---

## 3. CPU Optimizations

### Code-Level Optimizations
- [ ] **Avoid unnecessary clones** - Use references in function signatures
  - Implementation: `fn process(request: &GatewayRequest)` not `request: GatewayRequest`
  - Impact: -100μs per request
  - Measurement: Flamegraph clone() calls

- [ ] **Use references over ownership** - Pass `&str`, `&[u8]` where possible
  - Implementation: Review all function signatures for owned types
  - Impact: Reduce move semantics overhead
  - Measurement: Compiler optimization report

- [ ] **Inline hot functions** - Add `#[inline]` to <10 line functions
  - Implementation: `#[inline]` on `validate_request()`, `check_rate_limit()`
  - Impact: -20μs per call (eliminate function call overhead)
  - Measurement: Compare assembly output with/without inline

- [ ] **SIMD for JSON parsing** - Use `simd-json` crate
  - Implementation: Replace `serde_json` with `simd_json` for parsing
  - Impact: 2-3x faster JSON deserialization
  - Measurement: Benchmark with Criterion

- [ ] **Profile-guided optimization (PGO)** - Enable PGO compilation
  - Implementation: `RUSTFLAGS="-C profile-generate"` → collect → `profile-use`
  - Impact: 10-15% overall performance gain
  - Measurement: Before/after load test comparison

### Async Runtime Tuning
- [ ] **Tokio thread pool sizing** - 1 thread per CPU core
  - Implementation: `worker_threads = num_cpus::get()`
  - Measurement: CPU utilization balance across cores

- [ ] **Blocking task offload** - Move blocking I/O to blocking pool
  - Implementation: `tokio::task::spawn_blocking(|| {...})`
  - Impact: Prevent async worker thread starvation
  - Measurement: Blocked time metrics

- [ ] **Task spawn overhead reduction** - Batch operations where possible
  - Implementation: Use `futures::stream::iter().buffer_unordered(100)`
  - Impact: -50μs per spawn avoided
  - Measurement: Task spawn rate

---

## 4. I/O Optimizations

### HTTP Client Configuration
- [ ] **Connection pooling sized correctly** - 100 per provider
  - Current: `max_connections_per_provider: 100`
  - Impact: Reuse TCP connections, avoid handshake overhead
  - Measurement: Active connections via pool stats

- [ ] **HTTP/2 multiplexing enabled** - Single connection, multiple streams
  - Implementation: `hyper::Client::builder().http2_only(true)`
  - Impact: -50ms connection establishment per request
  - Measurement: Connections created/sec

- [ ] **TLS session resumption** - Cache TLS sessions for 1 hour
  - Implementation: Configure TLS connector with session cache
  - Impact: -30ms TLS handshake on resumed sessions
  - Measurement: Full vs resumed handshake ratio

- [ ] **DNS caching** - Cache DNS lookups for 5 minutes
  - Implementation: Custom DNS resolver with TTL cache
  - Impact: -10ms DNS resolution per request
  - Measurement: DNS query rate

- [ ] **Async file I/O for config** - Use `tokio::fs` for config loading
  - Implementation: Replace `std::fs` with `tokio::fs`
  - Impact: Non-blocking config reloads
  - Measurement: Blocking task count

### Redis Optimizations
- [ ] **Pipeline commands** - Batch GET/SET operations
  - Implementation: `redis::pipe().get().set().query_async()`
  - Impact: -2ms RTT per additional command
  - Measurement: Redis commands/sec

- [ ] **Connection pooling** - 20 connections minimum
  - Implementation: `deadpool-redis` with min_size: 20
  - Impact: Reduce connection acquisition time
  - Measurement: Connection wait time P95

- [ ] **Key compression** - Compress cached responses >1KB
  - Implementation: `zstd` compression before SET
  - Impact: 60% memory reduction, +1ms CPU overhead
  - Measurement: Cache memory usage

---

## 5. Hot Path Analysis

Request lifecycle with latency budget:

```
Client → Gateway → Provider → Response
         ↓         ↓          ↓
         Parse     Route      Transform
         [200μs]   [50μs]     [300μs]
                   ↓
                   Cache Check
                   [2ms]
                   ↓
                   Provider API
                   [50-200ms]
```

### Critical Path Optimization Targets

**Request Parsing (Budget: <200μs)**
- [ ] Zero-copy JSON parsing with `simd-json`
- [ ] Pre-compiled regex for validation
- [ ] Lazy deserialization of optional fields
- Measurement: `criterion` micro-benchmark

**Routing Decision (Budget: <50μs)**
- [ ] Static route table lookup (no regex)
- [ ] Inline model name mapping
- [ ] Pre-computed provider capabilities
- Measurement: Flamegraph routing overhead

**Request Validation (Budget: <300μs)**
- [ ] Early return on invalid requests
- [ ] Parallel validation of independent fields
- [ ] Cached validation results for repeated patterns
- Measurement: Validation time histogram

**Cache Lookup (Budget: <2ms)**
- [ ] Single Redis GET operation
- [ ] Compressed key names (<50 bytes)
- [ ] TTL check in Redis (not application)
- Measurement: Redis latency P95

**Provider Transform (Budget: <300μs)**
- [ ] Pre-allocated transformation buffers
- [ ] Avoid intermediate JSON representations
- [ ] Direct serialization to `Bytes`
- Measurement: Transformation time per provider

---

## 6. Benchmark Suite

### Micro-Benchmarks (Criterion)

```rust
// File: benches/request_parsing.rs
#[bench]
fn parse_gateway_request(b: &mut Bencher) {
    let json = include_bytes!("fixtures/request.json");
    b.iter(|| {
        simd_json::from_slice::<GatewayRequest>(json)
    });
}
// Target: <100μs (P50)
```

| Benchmark | Current | Target | Tool |
|-----------|---------|--------|------|
| Request parsing | 180μs | <100μs | Criterion |
| Routing decision | 75μs | <50μs | Criterion |
| JSON serialization | 250μs | <200μs | Criterion |
| Provider transform | 320μs | <250μs | Criterion |
| Rate limit check | 150μs | <100μs | Criterion |

### Load Testing (k6)

```javascript
// File: tests/load/scenario.js
import http from 'k6/http';
import { check } from 'k6';

export const options = {
  stages: [
    { duration: '2m', target: 1000 },   // Ramp up
    { duration: '5m', target: 10000 },  // Sustain
    { duration: '2m', target: 0 },      // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<120'], // 95% under 120ms
    http_req_failed: ['rate<0.01'],   // Error rate < 1%
  },
};
```

| Scenario | RPS | Duration | Pass Criteria |
|----------|-----|----------|---------------|
| Smoke test | 100 | 1 min | 0% errors |
| Load test | 10K | 5 min | P95 <120ms |
| Stress test | 15K | 3 min | Graceful degradation |
| Spike test | 0→20K→0 | 2 min | Recovery <30s |
| Endurance | 5K | 1 hour | No memory leak |

### Continuous Benchmarking
- [ ] Run benchmarks in CI on every PR
- [ ] Store results in time-series database
- [ ] Alert on >5% regression
- [ ] Generate flamegraphs for significant changes
- Tooling: GitHub Actions + Bencher.dev

---

## 7. Profiling Methodology

### CPU Profiling
```bash
# Production profiling with perf
perf record -F 99 -p $(pgrep gateway) -g -- sleep 60
perf script | stackcollapse-perf.pl | flamegraph.pl > flame.svg

# Development profiling
cargo flamegraph --bin gateway -- --config config.yaml
```
- [ ] Profile under realistic load (10K RPS)
- [ ] Identify functions taking >1% CPU time
- [ ] Target top 5 hotspots for optimization
- Schedule: Weekly during development

### Memory Profiling
```bash
# Heap profiling
heaptrack ./target/release/gateway
heaptrack_gui heaptrack.gateway.*

# Allocation tracking
valgrind --tool=massif --time-unit=B ./gateway
massif-visualizer massif.out.*
```
- [ ] Identify allocation hotspots
- [ ] Track memory growth over time
- [ ] Verify no memory leaks in 1-hour run
- Schedule: Before each release

### Async Profiling
```bash
# Tokio console
tokio-console

# Custom instrumentation
RUSTFLAGS="--cfg tokio_unstable" cargo run
```
- [ ] Monitor task spawn rate
- [ ] Identify long-blocking tasks (>100ms)
- [ ] Track async runtime overhead
- Schedule: During load testing

### Continuous Profiling
```bash
# Pyroscope agent
./pyroscope agent -application-name=gateway \
  -server-address=http://pyroscope:4040
```
- [ ] Deploy Pyroscope in production
- [ ] Set up dashboards for CPU/memory profiles
- [ ] Alert on profile anomalies
- Schedule: Always-on in production

---

## 8. Regression Prevention

### CI Pipeline Integration
```yaml
# .github/workflows/benchmark.yml
name: Performance Benchmarks
on: [pull_request]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run benchmarks
        run: |
          cargo bench --bench request_parsing -- --save-baseline pr-${{ github.event.number }}
      - name: Compare with main
        run: |
          cargo bench --bench request_parsing -- --baseline main
      - name: Fail on >5% regression
        run: |
          ./scripts/check_regression.sh 5
```

### Performance Budget Enforcement
```toml
# Cargo.toml
[profile.release]
lto = true
codegen-units = 1
panic = 'abort'

[profile.bench]
inherits = "release"
```

### Automated Alerts
- [ ] Slack notification on benchmark failure
- [ ] Block merge on >5% regression
- [ ] Generate performance comparison report
- [ ] Track historical trend in dashboard

### Performance SLOs
| Metric | SLO | Measurement |
|--------|-----|-------------|
| P50 latency | <45ms | Prometheus histogram |
| P95 latency | <120ms | Prometheus histogram |
| P99 latency | <350ms | Prometheus histogram |
| Error rate | <0.01% | Error count / total requests |
| Throughput | >10K RPS | Requests/sec gauge |
| Memory | <2.5GB | Container RSS |

---

## 9. Production Monitoring

### Key Performance Indicators

**Latency Metrics**
```promql
# P95 latency by endpoint
histogram_quantile(0.95,
  rate(http_request_duration_seconds_bucket[5m])
) by (endpoint)

# Provider API latency
histogram_quantile(0.95,
  rate(llm_provider_latency_seconds_bucket[5m])
) by (provider)
```

**Throughput Metrics**
```promql
# Requests per second
rate(http_requests_total[1m])

# Successful vs failed requests
sum(rate(http_requests_total{status=~"2.."}[5m])) /
sum(rate(http_requests_total[5m]))
```

**Resource Utilization**
```promql
# CPU usage
rate(process_cpu_seconds_total[5m]) * 100

# Memory usage
process_resident_memory_bytes / 1024 / 1024

# Connection pool utilization
llm_connection_pool_active / llm_connection_pool_max
```

### Alerting Rules
```yaml
# prometheus/alerts.yml
groups:
  - name: performance
    rules:
      - alert: HighLatency
        expr: |
          histogram_quantile(0.95,
            rate(http_request_duration_seconds_bucket[5m])
          ) > 0.120
        for: 5m
        annotations:
          summary: "P95 latency above 120ms"

      - alert: HighErrorRate
        expr: |
          rate(http_requests_total{status=~"5.."}[5m]) > 0.01
        for: 2m
        annotations:
          summary: "Error rate above 1%"

      - alert: MemoryLeak
        expr: |
          delta(process_resident_memory_bytes[1h]) > 100000000
        for: 1h
        annotations:
          summary: "Memory growing by >100MB/hour"
```

---

## 10. Optimization Workflow

### Phase 1: Measure (Week 1)
1. [ ] Set up profiling infrastructure
2. [ ] Run baseline benchmarks (all scenarios)
3. [ ] Collect production metrics for 7 days
4. [ ] Identify top 10 bottlenecks via flamegraph
5. [ ] Document current performance baseline

### Phase 2: Optimize (Weeks 2-4)
1. [ ] Prioritize optimizations by impact/effort
2. [ ] Implement top 5 high-impact optimizations
3. [ ] Benchmark each change individually
4. [ ] Profile to verify improvement
5. [ ] Document performance gains

### Phase 3: Validate (Week 5)
1. [ ] Run full benchmark suite
2. [ ] Stress test at 2x target load
3. [ ] Endurance test for 24 hours
4. [ ] Verify no regressions in functionality
5. [ ] Update performance documentation

### Phase 4: Deploy (Week 6)
1. [ ] Canary deployment to 5% traffic
2. [ ] Monitor metrics for 24 hours
3. [ ] Gradual rollout to 100%
4. [ ] Verify SLO compliance
5. [ ] Document lessons learned

### Optimization Priority Matrix

| Optimization | Impact | Effort | Priority |
|--------------|--------|--------|----------|
| SIMD JSON parsing | High | Low | P0 |
| Connection pooling | High | Medium | P0 |
| Zero-copy buffers | High | Medium | P1 |
| Inline hot functions | Medium | Low | P1 |
| PGO compilation | Medium | Low | P1 |
| Arena allocators | Medium | High | P2 |
| Custom allocator | Low | High | P3 |

---

## 11. Performance Testing Checklist

### Pre-Release Validation
- [ ] All micro-benchmarks pass (<5% regression)
- [ ] Load test at 10K RPS for 5 minutes (P95 <120ms)
- [ ] Stress test at 15K RPS (graceful degradation)
- [ ] Endurance test at 5K RPS for 1 hour (no memory leak)
- [ ] Spike test 0→20K→0 RPS (recovery <30s)
- [ ] Multi-region latency test (<200ms cross-region)
- [ ] Cache hit ratio >70% under load
- [ ] Error rate <0.01% across all scenarios

### Production Readiness
- [ ] Monitoring dashboards configured
- [ ] Alerting rules tested and documented
- [ ] Runbook for performance incidents
- [ ] Auto-scaling policies validated
- [ ] Circuit breakers tested under failure
- [ ] Rate limits enforced correctly
- [ ] Chaos engineering validation passed

---

## 12. Tools & Resources

### Profiling Tools
- **CPU:** `perf`, `flamegraph`, `cargo-flamegraph`
- **Memory:** `heaptrack`, `valgrind`, `massif`
- **Async:** `tokio-console`, custom instrumentation
- **Continuous:** `pyroscope`, `pprof`

### Benchmarking Tools
- **Micro:** `criterion` (Rust benchmarks)
- **Load:** `k6`, `wrk`, `drill`
- **APM:** Prometheus + Grafana
- **Tracing:** Jaeger, OpenTelemetry

### Analysis Tools
- **Flamegraph:** Visual CPU profiling
- **perf-top:** Real-time CPU hotspots
- **cargo-bloat:** Binary size analysis
- **cargo-expand:** Macro expansion inspection

### Reference Architectures
- Cloudflare Workers architecture
- AWS Lambda optimizations
- Fastly Compute@Edge patterns

---

## Summary

**Critical Path Focus Areas:**
1. HTTP/2 connection pooling (50ms savings)
2. SIMD JSON parsing (100μs savings)
3. Zero-copy buffer management (200μs savings)
4. Inline hot functions (50μs savings)
5. Profile-guided optimization (15% overall gain)

**Expected Performance Gains:**
- Throughput: 10K → 15K RPS (+50%)
- P95 Latency: 120ms → 80ms (-33%)
- Memory: 2.5GB → 1.8GB (-28%)
- Error Rate: <0.01% (maintained)

**Total Optimization Effort:** 6 weeks
**Monitoring Overhead:** <2% CPU/memory

---

**Document Version:** 1.0
**Last Updated:** November 2024
**Owner:** Performance Engineering Team
**Review Cadence:** Quarterly
