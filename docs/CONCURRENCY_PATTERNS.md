# Rust Concurrency Patterns - Thread Safety Documentation

## 1. Shared State Inventory

### Complete Shared State Analysis

| State | Type | Access Pattern | Synchronization | Contention Risk | Location |
|-------|------|----------------|-----------------|-----------------|----------|
| Provider Registry | `Arc<RwLock<HashMap>>` | Read-heavy (95%+) | Async RwLock | **Low** | `ProviderRegistry::providers` |
| Health Cache | `Arc<RwLock<HashMap>>` | Read-heavy (90%) | Async RwLock | **Low** | `ProviderRegistry::health_cache` |
| Circuit Breaker State | `Arc<RwLock<CircuitState>>` | Read-heavy (99%) | Async RwLock | **Medium** | `CircuitBreaker::state` |
| Circuit Failure Count | `Arc<RwLock<u32>>` | Write-heavy (50%) | Async RwLock | **Medium** | `CircuitBreaker::failure_count` |
| Rate Limiter Tokens | `Arc<RwLock<TokenBucket>>` | Write-heavy (100%) | Async RwLock | **High** | `RateLimiter::request_tokens` |
| Connection Limits | `Arc<RwLock<HashMap>>` | Read-mostly (80%) | Async RwLock | **Medium** | `ConnectionPool::connection_limits` |
| Semaphore Permits | `Arc<Semaphore>` | Acquire/Release | Lock-free | **Medium** | Per-provider semaphores |
| Provider Metrics | `Arc<RwLock<ProviderMetrics>>` | Write-heavy (60%) | Async RwLock | **High** | Per-provider metrics |
| Response Cache | `Arc<RwLock<HashMap>>` | Read-heavy (70%) | Async RwLock | **Medium** | `ResponseCache::cache` |
| Load Balancer Counter | `Arc<Mutex<usize>>` | Write-only (100%) | Async Mutex | **High** | `RoundRobinBalancer::counter` |
| Latency Windows | `Arc<RwLock<HashMap>>` | Read-write (50/50) | Async RwLock | **High** | `LatencyWeightedBalancer::latencies` |
| Active Connections | `Arc<RwLock<HashMap>>` | Read-write (50/50) | Async RwLock | **Medium** | `LeastConnectionsBalancer::active_connections` |

### Key Observations

1. **No Atomics Used**: Current implementation uses `RwLock<u32>` instead of `AtomicU64` for counters
2. **Potential Performance Issues**: Rate limiters and metrics use write locks on hot paths
3. **Missing Lock-Free Structures**: DashMap not used despite high concurrency needs

---

## 2. Synchronization Patterns

### 2.1 Arc&lt;T&gt; - Immutable Shared Reference Counting

**When to Use**: Sharing immutable data across threads/tasks.

**Performance**: Near-zero overhead, atomic reference counting only.

**Pitfalls**:
- Cannot mutate inner data
- Cloning `Arc` is cheap (atomic increment), but deep cloning data inside is not

**Example**:
```rust
use std::sync::Arc;

// Provider capabilities are immutable after construction
pub struct OpenAIProvider {
    capabilities: ProviderCapabilities, // Not Arc - owned
    client: Arc<ConnectionPool>,        // Arc - shared, immutable
}

impl OpenAIProvider {
    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities // Direct reference, no locking
    }
}
```

**Best Practice**:
```rust
// GOOD: Arc around large, immutable structures
let config = Arc::new(ConnectionPoolConfig::default());
let pool = Arc::new(ConnectionPool::new((*config).clone()));

// BAD: Arc around tiny primitives
let counter = Arc::new(42u32); // Use AtomicU32 instead
```

---

### 2.2 Arc&lt;RwLock&lt;T&gt;&gt; - Read-Heavy Mutable Data

**When to Use**: Many readers, few writers (80/20 or better ratio).

**Performance**:
- Read locks: Fast, multiple concurrent readers
- Write locks: Exclusive, blocks all readers

**Pitfalls**:
- Write lock starvation in read-heavy workloads
- Deadlock if holding lock across `.await` points
- Not `Send` safe across async boundaries

**Current Usage**:
```rust
// Provider Registry - Read-heavy pattern
pub struct ProviderRegistry {
    providers: Arc<RwLock<HashMap<String, Arc<dyn LLMProvider>>>>,
    health_cache: Arc<RwLock<HashMap<String, HealthStatus>>>,
}

impl ProviderRegistry {
    pub async fn get(&self, provider_id: &str) -> Option<Arc<dyn LLMProvider>> {
        let providers = self.providers.read().await; // Multiple readers OK
        providers.get(provider_id).cloned()
    } // Lock dropped immediately

    pub async fn register(&self, provider: Arc<dyn LLMProvider>) -> Result<()> {
        let mut providers = self.providers.write().await; // Exclusive
        providers.insert(provider.provider_id().to_string(), provider);
        Ok(())
    } // Lock dropped immediately
}
```

**Critical Rule**: **Never hold RwLock across `.await`**
```rust
// DANGEROUS - Deadlock risk!
async fn bad_example(&self) {
    let data = self.state.read().await;
    some_async_operation().await; // Lock held across await!
    println!("{:?}", data);
}

// SAFE - Drop lock before await
async fn good_example(&self) {
    let snapshot = {
        let data = self.state.read().await;
        data.clone() // Clone data, drop lock
    };
    some_async_operation().await;
    println!("{:?}", snapshot);
}
```

---

### 2.3 Tokio Semaphore - Connection Pool Limiting

**When to Use**: Limiting concurrent access to resources.

**Performance**: Lock-free implementation, very fast.

**Pitfalls**:
- Permits must be held until operation completes
- Forgetting to acquire permit = unlimited concurrency

**Example**:
```rust
use tokio::sync::Semaphore;

pub struct ConnectionPool {
    connection_limits: Arc<RwLock<HashMap<String, Arc<Semaphore>>>>,
    config: ConnectionPoolConfig,
}

impl ConnectionPool {
    pub async fn acquire_permit(&self, provider_id: &str) -> Result<ConnectionPermit> {
        // Get or create semaphore for provider
        let semaphore = {
            let mut limits = self.connection_limits.write().await;
            limits
                .entry(provider_id.to_string())
                .or_insert_with(|| {
                    Arc::new(Semaphore::new(self.config.max_connections_per_provider))
                })
                .clone()
        }; // Write lock dropped here

        // Acquire permit - blocks if at capacity
        let permit = semaphore.acquire().await
            .map_err(|e| ProviderError::NetworkError(format!("Failed to acquire: {}", e)))?;

        Ok(ConnectionPermit {
            _permit: permit, // Held until ConnectionPermit dropped
            acquired_at: Instant::now(),
        })
    }
}

// Permit automatically released on drop
pub struct ConnectionPermit {
    _permit: tokio::sync::SemaphorePermit<'static>,
    acquired_at: Instant,
}
```

**Pattern: RAII Resource Guard**
```rust
async fn make_request(&self, request: &Request) -> Result<Response> {
    let _permit = self.pool.acquire_permit(&self.provider_id).await?;
    // Permit held for entire request duration
    let response = self.client.request(request).await?;
    Ok(response)
} // Permit released automatically here
```

---

### 2.4 AtomicU64/AtomicBool - Lock-Free Counters (MISSING)

**When to Use**: Simple counters, flags, monotonic sequences.

**Performance**: Fastest synchronization primitive, no locks.

**Current Gap**: Metrics use `RwLock<u64>` instead of `AtomicU64`.

**Recommended Refactor**:
```rust
// CURRENT (slow) - requires write lock
pub struct ProviderMetrics {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
}

// Wrapped in Arc<RwLock<ProviderMetrics>>
let mut metrics = self.metrics.write().await; // Exclusive lock!
metrics.total_requests += 1;

// RECOMMENDED (fast) - lock-free atomic operations
use std::sync::atomic::{AtomicU64, Ordering};

pub struct ProviderMetrics {
    total_requests: AtomicU64,
    successful_requests: AtomicU64,
    failed_requests: AtomicU64,
}

// No locking needed!
self.metrics.total_requests.fetch_add(1, Ordering::Relaxed);
```

**Memory Ordering Guide**:
```rust
// Relaxed: Just atomicity, no ordering guarantees
// Use for: Independent counters, statistics
counter.fetch_add(1, Ordering::Relaxed);

// Acquire/Release: Synchronize with other threads
// Use for: Producer-consumer patterns, state machines
state.store(NEW_STATE, Ordering::Release); // Publish changes
let current = state.load(Ordering::Acquire); // See published changes

// SeqCst: Total ordering across all threads
// Use for: Complex cross-thread visibility (rare, expensive)
health_score.store(score, Ordering::SeqCst);
```

---

### 2.5 DashMap - Concurrent HashMap (RECOMMENDED)

**When to Use**: High-concurrency read/write maps.

**Performance**: Better than `RwLock<HashMap>` under contention.

**Not Currently Used** - Recommended refactor:
```rust
// CURRENT
providers: Arc<RwLock<HashMap<String, Arc<dyn LLMProvider>>>>

// RECOMMENDED
use dashmap::DashMap;

pub struct ProviderRegistry {
    providers: Arc<DashMap<String, Arc<dyn LLMProvider>>>,
}

impl ProviderRegistry {
    pub fn get(&self, provider_id: &str) -> Option<Arc<dyn LLMProvider>> {
        self.providers.get(provider_id).map(|r| r.value().clone())
        // No async, no manual locking!
    }

    pub fn register(&self, provider: Arc<dyn LLMProvider>) {
        self.providers.insert(provider.provider_id().to_string(), provider);
    }
}
```

**Advantages**:
- Lock-free reads under low contention
- Fine-grained locking (per-shard, not entire map)
- No async/await overhead for simple operations

---

### 2.6 ArcSwap - Lock-Free Configuration Updates (RECOMMENDED)

**When to Use**: Infrequent writes, frequent reads of large structures.

**Performance**: Atomic pointer swap, zero-cost reads.

**Use Case**: Hot configuration reloading without disrupting requests.

```rust
use arc_swap::ArcSwap;

pub struct Gateway {
    config: Arc<ArcSwap<GatewayConfig>>, // Lock-free!
}

impl Gateway {
    pub async fn handle_request(&self, req: Request) -> Response {
        let config = self.config.load(); // Atomic load, no locking

        // Use config without holding any locks
        if config.max_tokens > req.tokens {
            // ...
        }

        // Config cannot change during this request
        // (we hold an Arc to the current version)
    }

    pub fn reload_config(&self, new_config: GatewayConfig) {
        self.config.store(Arc::new(new_config)); // Atomic swap!
        // Old config dropped when last request finishes
    }
}
```

---

## 3. Deadlock Prevention

### 3.1 Lock Ordering Rules

**Rule**: Always acquire locks in a consistent global order.

**Example Hierarchy** (lowest to highest):
1. Provider metrics
2. Rate limiter tokens
3. Connection pool limits
4. Circuit breaker state
5. Provider registry
6. Health cache

```rust
// GOOD: Metrics -> Registry (ascending order)
let metrics = self.metrics.write().await;
let registry = self.registry.write().await;

// BAD: Registry -> Metrics (descending order, deadlock risk!)
let registry = self.registry.write().await;
let metrics = self.metrics.write().await;
```

### 3.2 Timeout on Lock Acquisition

**Problem**: Deadlocks cause infinite hangs.

**Solution**: Use `tokio::time::timeout` wrapper.

```rust
use tokio::time::{timeout, Duration};

pub async fn acquire_with_timeout<T>(
    lock: &RwLock<T>,
    duration: Duration,
) -> Result<tokio::sync::RwLockReadGuard<'_, T>> {
    timeout(duration, lock.read())
        .await
        .map_err(|_| ProviderError::Timeout("Lock acquisition timeout".into()))?
        .map_err(|_| ProviderError::ProviderInternalError("Lock poisoned".into()))
}

// Usage
let data = acquire_with_timeout(&self.state, Duration::from_secs(5)).await?;
```

### 3.3 Never Hold Locks Across `.await`

**Critical Rule**: Drop all locks before any `.await` point.

```rust
// WRONG - Potential deadlock
async fn bad_health_check(&self) -> HealthStatus {
    let providers = self.providers.read().await;

    for provider in providers.values() {
        // This holds registry lock while waiting for health check!
        let health = provider.health_check().await; // DEADLOCK RISK
    }
}

// CORRECT - Clone data, release lock, then await
async fn good_health_check(&self) -> Vec<HealthStatus> {
    let provider_list = {
        let providers = self.providers.read().await;
        providers.values().map(Arc::clone).collect::<Vec<_>>()
    }; // Lock dropped here

    let mut results = Vec::new();
    for provider in provider_list {
        results.push(provider.health_check().await); // Safe
    }
    results
}
```

### 3.4 Try-Lock Pattern for Optional Operations

```rust
// Non-blocking attempt to update metrics
pub fn try_record_latency(&self, latency: Duration) {
    if let Ok(mut metrics) = self.metrics.try_write() {
        metrics.total_latency_ms += latency.as_millis() as u64;
    }
    // If lock unavailable, skip update (metrics not critical)
}
```

---

## 4. Race Condition Prevention

### 4.1 Config Reload During Request

**Scenario**: Configuration reloaded while request in-flight.

**Risk**: Request sees partial old/new config state.

**Mitigation**: Use `ArcSwap` for atomic config replacement.

```rust
use arc_swap::ArcSwap;

pub struct GatewayConfig {
    max_tokens: u32,
    timeout: Duration,
    providers: Vec<String>,
}

pub struct Gateway {
    config: Arc<ArcSwap<GatewayConfig>>,
}

impl Gateway {
    // Each request gets consistent snapshot
    pub async fn handle_request(&self) -> Response {
        let config = self.config.load(); // Atomic load

        // All uses of config see same version
        let timeout = config.timeout;
        let max_tokens = config.max_tokens;

        // Even if reload happens here, we still use old config
        tokio::time::sleep(Duration::from_secs(10)).await;

        println!("Timeout: {:?}", config.timeout); // Still old value
    }

    // Atomic swap, no intermediate state
    pub fn reload(&self, new_config: GatewayConfig) {
        self.config.store(Arc::new(new_config));
    }
}
```

### 4.2 Circuit Breaker State Transitions

**Scenario**: Concurrent success/failure updates to circuit state.

**Risk**: Incorrect state transitions (e.g., Open -> Closed without HalfOpen).

**Mitigation**: State machine with compare-and-swap.

```rust
pub async fn on_success(&self) {
    let mut state = self.state.write().await;

    match *state {
        CircuitState::HalfOpen => {
            let mut success_count = self.success_count.write().await;
            *success_count += 1;

            if *success_count >= self.success_threshold {
                // Atomic transition
                *state = CircuitState::Closed;
                *self.failure_count.write().await = 0;
                *success_count = 0;
            }
        }
        CircuitState::Closed => {
            // Reset failures on any success
            *self.failure_count.write().await = 0;
        }
        _ => {}
    }
}
```

**Recommended**: Use atomic state machine:
```rust
use std::sync::atomic::{AtomicU8, Ordering};

const CLOSED: u8 = 0;
const HALF_OPEN: u8 = 1;
const OPEN: u8 = 2;

pub struct CircuitBreaker {
    state: AtomicU8,
    failure_count: AtomicU32,
    success_count: AtomicU32,
}

impl CircuitBreaker {
    pub fn on_success(&self) {
        let current = self.state.load(Ordering::Acquire);

        match current {
            HALF_OPEN => {
                let successes = self.success_count.fetch_add(1, Ordering::Relaxed);
                if successes + 1 >= self.success_threshold {
                    // Try atomic transition
                    let _ = self.state.compare_exchange(
                        HALF_OPEN,
                        CLOSED,
                        Ordering::Release,
                        Ordering::Relaxed,
                    );
                }
            }
            CLOSED => {
                self.failure_count.store(0, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}
```

### 4.3 Rate Limit Check-Then-Decrement

**Scenario**: Check token availability, then decrement in separate step.

**Risk**: Two requests see available tokens, both decrement, go negative.

**Current (UNSAFE)**:
```rust
fn try_consume(&mut self, amount: f64) -> Option<Duration> {
    self.refill();

    if self.tokens >= amount {
        self.tokens -= amount; // RACE: Another thread could have consumed tokens!
        None
    } else {
        Some(calculate_wait_time(amount - self.tokens))
    }
}
```

**Mitigation**: Atomic compare-and-swap loop:
```rust
use std::sync::atomic::{AtomicU64, Ordering};

pub struct TokenBucket {
    tokens: AtomicU64, // Store as fixed-point (tokens * 1000)
    capacity: u64,
    refill_rate: u64,
    last_refill: AtomicU64, // Timestamp in millis
}

impl TokenBucket {
    pub fn try_consume(&self, amount: u64) -> Option<Duration> {
        loop {
            let current = self.tokens.load(Ordering::Acquire);

            if current >= amount {
                // Try atomic decrement
                match self.tokens.compare_exchange(
                    current,
                    current - amount,
                    Ordering::Release,
                    Ordering::Acquire,
                ) {
                    Ok(_) => return None, // Success
                    Err(_) => continue,    // Retry CAS loop
                }
            } else {
                return Some(self.calculate_wait(amount - current));
            }
        }
    }
}
```

---

## 5. Memory Ordering for Atomics

### 5.1 Counter Increment - Relaxed

**Use Case**: Independent statistics, no cross-thread dependencies.

```rust
// Prometheus-style metrics
pub struct Metrics {
    requests_total: AtomicU64,
    requests_success: AtomicU64,
    requests_failure: AtomicU64,
}

impl Metrics {
    pub fn record_request(&self) {
        // Relaxed: Only atomicity matters, order doesn't
        self.requests_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_success(&self) {
        self.requests_success.fetch_add(1, Ordering::Relaxed);
    }
}
```

**Why Relaxed**: Each counter is independent. We don't care if `requests_total` increments before or after `requests_success` from another thread's perspective.

### 5.2 Circuit State Transitions - Acquire/Release

**Use Case**: State machine with visibility requirements.

```rust
pub struct CircuitBreaker {
    state: AtomicU8,
    failure_count: AtomicU32,
    last_failure_time: AtomicU64,
}

impl CircuitBreaker {
    pub fn on_failure(&self) {
        // Increment failure count
        let failures = self.failure_count.fetch_add(1, Ordering::Relaxed);

        // Update timestamp
        self.last_failure_time.store(now_millis(), Ordering::Relaxed);

        if failures + 1 >= self.failure_threshold {
            // Release: Ensure failure_count and timestamp visible before state change
            self.state.store(OPEN, Ordering::Release);
        }
    }

    pub fn check_state(&self) -> CircuitState {
        // Acquire: Ensure we see latest failure_count and timestamp
        let state = self.state.load(Ordering::Acquire);

        if state == OPEN {
            let last_failure = self.last_failure_time.load(Ordering::Relaxed);
            if elapsed_since(last_failure) > self.timeout {
                // Try transition to HalfOpen
                let _ = self.state.compare_exchange(
                    OPEN,
                    HALF_OPEN,
                    Ordering::Release, // Success: publish state change
                    Ordering::Acquire, // Failure: see winner's state
                );
            }
        }

        match state {
            0 => CircuitState::Closed,
            1 => CircuitState::HalfOpen,
            2 => CircuitState::Open,
            _ => unreachable!(),
        }
    }
}
```

**Why Acquire/Release**:
- `Release` on state write: Ensures all preceding writes (failure_count, timestamp) visible
- `Acquire` on state read: Ensures we see all writes that happened before state change

### 5.3 Health Score Updates - SeqCst (When Necessary)

**Use Case**: Multiple atomic variables with cross-thread visibility requirements.

```rust
pub struct HealthTracker {
    error_rate: AtomicU32,      // Errors per 1000 requests
    latency_p99: AtomicU64,     // P99 latency in microseconds
    health_score: AtomicU32,    // Composite score 0-100
}

impl HealthTracker {
    pub fn update_health(&self) {
        let errors = self.error_rate.load(Ordering::SeqCst);
        let latency = self.latency_p99.load(Ordering::SeqCst);

        let score = calculate_health_score(errors, latency);

        // SeqCst: Ensure all threads see consistent ordering of updates
        self.health_score.store(score, Ordering::SeqCst);
    }

    pub fn is_healthy(&self) -> bool {
        // SeqCst: Ensure we see latest health_score relative to error/latency
        self.health_score.load(Ordering::SeqCst) >= 70
    }
}
```

**Why SeqCst**: Ensures total ordering across all threads. If thread A updates `error_rate`, thread B updating `health_score` will see that change.

**Cost**: Most expensive ordering, use sparingly.

---

## 6. Recommended Architecture Improvements

### 6.1 Metrics Collection - Use Atomics

```rust
// BEFORE (slow, lock contention)
pub struct ProviderMetrics {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    total_latency_ms: u64,
}
// Wrapped in Arc<RwLock<ProviderMetrics>>

// AFTER (fast, lock-free)
use std::sync::atomic::{AtomicU64, Ordering};

pub struct ProviderMetrics {
    total_requests: AtomicU64,
    successful_requests: AtomicU64,
    failed_requests: AtomicU64,
    total_latency_ms: AtomicU64,
}

impl ProviderMetrics {
    pub fn record_request(&self, latency: Duration, success: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_latency_ms.fetch_add(latency.as_millis() as u64, Ordering::Relaxed);

        if success {
            self.successful_requests.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_requests.fetch_add(1, Ordering::Relaxed);
        }
    }
}
```

### 6.2 Provider Registry - Use DashMap

```rust
// BEFORE
pub struct ProviderRegistry {
    providers: Arc<RwLock<HashMap<String, Arc<dyn LLMProvider>>>>,
}

// AFTER
use dashmap::DashMap;

pub struct ProviderRegistry {
    providers: Arc<DashMap<String, Arc<dyn LLMProvider>>>,
}

impl ProviderRegistry {
    pub fn get(&self, provider_id: &str) -> Option<Arc<dyn LLMProvider>> {
        self.providers.get(provider_id).map(|r| r.value().clone())
    }

    pub fn register(&self, provider: Arc<dyn LLMProvider>) {
        self.providers.insert(provider.provider_id().to_string(), provider);
    }
}
```

### 6.3 Configuration - Use ArcSwap

```rust
use arc_swap::ArcSwap;

pub struct Gateway {
    config: Arc<ArcSwap<GatewayConfig>>,
    providers: Arc<ProviderRegistry>,
}

impl Gateway {
    pub async fn handle_request(&self, req: Request) -> Response {
        let config = self.config.load();

        // Use config without any locks
        let provider = self.providers.get(&config.default_provider)?;
        provider.chat_completion(&req).await
    }

    pub fn reload_config(&self, new_config: GatewayConfig) {
        self.config.store(Arc::new(new_config));
    }
}
```

---

## 7. Performance Optimization Checklist

- [ ] Replace `RwLock<u64>` counters with `AtomicU64`
- [ ] Replace `RwLock<HashMap>` with `DashMap` for high-concurrency maps
- [ ] Use `ArcSwap` for configuration hot-reloading
- [ ] Never hold locks across `.await` points
- [ ] Use `Relaxed` ordering for independent counters
- [ ] Use `Acquire/Release` for state machine transitions
- [ ] Avoid `SeqCst` unless absolutely necessary
- [ ] Implement lock timeout wrappers for diagnostics
- [ ] Add deadlock detection in tests
- [ ] Profile lock contention under load

---

## 8. Testing Concurrency

### 8.1 Stress Test Example

```rust
#[tokio::test]
async fn test_concurrent_provider_access() {
    let registry = Arc::new(ProviderRegistry::new(Duration::from_secs(60)));

    // Register provider
    let provider = create_test_provider();
    registry.register(provider).await.unwrap();

    // Spawn 1000 concurrent readers
    let mut handles = vec![];
    for _ in 0..1000 {
        let registry = Arc::clone(&registry);
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                let _ = registry.get("test_provider").await;
            }
        }));
    }

    // Wait for all
    for handle in handles {
        handle.await.unwrap();
    }
}
```

### 8.2 Race Condition Detection

```rust
use tokio::time::sleep;

#[tokio::test]
async fn test_rate_limiter_race_condition() {
    let limiter = RateLimiter::new(RateLimitConfig {
        requests_per_minute: Some(10),
        tokens_per_minute: None,
    });

    // Try to consume 11 tokens concurrently
    let mut handles = vec![];
    for _ in 0..11 {
        let limiter = limiter.clone();
        handles.push(tokio::spawn(async move {
            limiter.check_and_consume(1).await.is_none()
        }));
    }

    // Exactly 10 should succeed, 1 should fail
    let results: Vec<bool> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    assert_eq!(results.iter().filter(|&&x| x).count(), 10);
}
```

---

## Summary

**Key Takeaways**:
1. Use `Arc<T>` for immutable shared data (zero-cost)
2. Use `RwLock<T>` for read-heavy patterns, but avoid holding across `.await`
3. Use `DashMap` instead of `RwLock<HashMap>` for concurrent maps
4. Use `AtomicU64` instead of `RwLock<u64>` for counters
5. Use `ArcSwap` for lock-free configuration reloading
6. Use `Relaxed` ordering for independent operations
7. Use `Acquire/Release` for state synchronization
8. Never hold locks across async boundaries
9. Always use consistent lock ordering to prevent deadlocks
10. Add timeouts to lock acquisitions for debugging

**Contention Hotspots to Fix**:
- Provider metrics (use `AtomicU64`)
- Rate limiter token bucket (use atomic CAS loop)
- Load balancer counter (use `AtomicUsize`)
- Provider registry (use `DashMap`)
