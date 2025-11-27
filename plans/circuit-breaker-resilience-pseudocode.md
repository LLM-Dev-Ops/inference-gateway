# Circuit Breaker and Resilience System - Comprehensive Pseudocode

## Overview
Production-grade fault tolerance and resilience mechanisms for the LLM Inference Gateway with per-provider isolation, graceful degradation, and adaptive recovery.

---

## 1. Circuit Breaker Core

```rust
// ============================================================================
// CIRCUIT BREAKER - State Machine Implementation
// ============================================================================

use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use tokio::time;

/// Circuit breaker states represented as atomic integers
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    Closed = 0,    // Normal operation, requests allowed
    Open = 1,      // Failures exceeded threshold, requests blocked
    HalfOpen = 2,  // Testing if system recovered, limited requests
}

impl From<u8> for CircuitState {
    fn from(value: u8) -> Self {
        match value {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed, // Default to closed for safety
        }
    }
}

/// Configuration for circuit breaker behavior
#[derive(Debug, Clone)]
struct CircuitBreakerConfig {
    // Failure detection
    failure_threshold: u32,           // consecutive failures before opening
    failure_rate_threshold: f64,      // 0.0-1.0, percentage of failures
    success_threshold: u32,           // successes in half-open to close

    // Time windows
    timeout: Duration,                // time in open state before half-open
    sampling_window: Duration,        // rolling window for failure rate
    half_open_timeout: Duration,      // timeout for half-open state

    // Request thresholds
    min_requests: u32,                // minimum requests before evaluating
    half_open_max_requests: u32,      // max concurrent requests in half-open

    // Error classification
    count_timeouts_as_failures: bool,
    count_5xx_as_failures: bool,
    count_429_as_failures: bool,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            failure_rate_threshold: 0.5,
            success_threshold: 3,
            timeout: Duration::from_secs(60),
            sampling_window: Duration::from_secs(10),
            half_open_timeout: Duration::from_secs(30),
            min_requests: 10,
            half_open_max_requests: 3,
            count_timeouts_as_failures: true,
            count_5xx_as_failures: true,
            count_429_as_failures: false,
        }
    }
}

/// Time-bucketed metrics for sliding window calculations
struct SlidingWindowBucket {
    timestamp: Instant,
    successes: AtomicU32,
    failures: AtomicU32,
    timeouts: AtomicU32,
    rejections: AtomicU32,
}

impl SlidingWindowBucket {
    fn new() -> Self {
        Self {
            timestamp: Instant::now(),
            successes: AtomicU32::new(0),
            failures: AtomicU32::new(0),
            timeouts: AtomicU32::new(0),
            rejections: AtomicU32::new(0),
        }
    }

    fn total_requests(&self) -> u32 {
        self.successes.load(Ordering::Relaxed) +
        self.failures.load(Ordering::Relaxed)
    }

    fn failure_rate(&self) -> f64 {
        let total = self.total_requests();
        if total == 0 {
            return 0.0;
        }
        self.failures.load(Ordering::Relaxed) as f64 / total as f64
    }
}

/// Circuit breaker metrics with sliding window
struct CircuitBreakerMetrics {
    // Sliding window of buckets
    buckets: RwLock<Vec<SlidingWindowBucket>>,
    bucket_duration: Duration,
    num_buckets: usize,

    // Consecutive counters (for threshold-based opening)
    consecutive_successes: AtomicU32,
    consecutive_failures: AtomicU32,

    // Lifetime statistics
    total_successes: AtomicU64,
    total_failures: AtomicU64,
    total_rejections: AtomicU64,

    // State transition tracking
    last_state_change: AtomicU64,  // Unix timestamp in nanos
    state_change_count: AtomicU64,
    open_count: AtomicU64,
    half_open_count: AtomicU64,
}

impl CircuitBreakerMetrics {
    fn new(sampling_window: Duration) -> Self {
        let num_buckets = 10;
        let bucket_duration = sampling_window / num_buckets as u32;

        let mut buckets = Vec::with_capacity(num_buckets);
        for _ in 0..num_buckets {
            buckets.push(SlidingWindowBucket::new());
        }

        Self {
            buckets: RwLock::new(buckets),
            bucket_duration,
            num_buckets,
            consecutive_successes: AtomicU32::new(0),
            consecutive_failures: AtomicU32::new(0),
            total_successes: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
            total_rejections: AtomicU64::new(0),
            last_state_change: AtomicU64::new(0),
            state_change_count: AtomicU64::new(0),
            open_count: AtomicU64::new(0),
            half_open_count: AtomicU64::new(0),
        }
    }

    fn record_success(&self) {
        // Update current bucket
        let buckets = self.buckets.read();
        if let Some(current) = buckets.last() {
            current.successes.fetch_add(1, Ordering::Relaxed);
        }
        drop(buckets);

        // Update consecutive counters
        self.consecutive_successes.fetch_add(1, Ordering::Relaxed);
        self.consecutive_failures.store(0, Ordering::Relaxed);
        self.total_successes.fetch_add(1, Ordering::Relaxed);

        // Rotate buckets if needed
        self.maybe_rotate_buckets();
    }

    fn record_failure(&self) {
        let buckets = self.buckets.read();
        if let Some(current) = buckets.last() {
            current.failures.fetch_add(1, Ordering::Relaxed);
        }
        drop(buckets);

        self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
        self.consecutive_successes.store(0, Ordering::Relaxed);
        self.total_failures.fetch_add(1, Ordering::Relaxed);

        self.maybe_rotate_buckets();
    }

    fn record_timeout(&self) {
        let buckets = self.buckets.read();
        if let Some(current) = buckets.last() {
            current.timeouts.fetch_add(1, Ordering::Relaxed);
        }
        drop(buckets);

        self.maybe_rotate_buckets();
    }

    fn record_rejection(&self) {
        let buckets = self.buckets.read();
        if let Some(current) = buckets.last() {
            current.rejections.fetch_add(1, Ordering::Relaxed);
        }
        drop(buckets);

        self.total_rejections.fetch_add(1, Ordering::Relaxed);
    }

    fn maybe_rotate_buckets(&self) {
        let buckets = self.buckets.read();
        if let Some(last) = buckets.last() {
            if last.timestamp.elapsed() >= self.bucket_duration {
                drop(buckets);
                let mut buckets = self.buckets.write();

                // Remove oldest bucket, add new one
                if buckets.len() >= self.num_buckets {
                    buckets.remove(0);
                }
                buckets.push(SlidingWindowBucket::new());
            }
        }
    }

    fn get_window_stats(&self) -> WindowStats {
        let buckets = self.buckets.read();

        let mut total_successes = 0u32;
        let mut total_failures = 0u32;
        let now = Instant::now();

        for bucket in buckets.iter() {
            // Only count buckets within the sampling window
            if now.duration_since(bucket.timestamp) <= self.bucket_duration * self.num_buckets as u32 {
                total_successes += bucket.successes.load(Ordering::Relaxed);
                total_failures += bucket.failures.load(Ordering::Relaxed);
            }
        }

        let total_requests = total_successes + total_failures;
        let failure_rate = if total_requests > 0 {
            total_failures as f64 / total_requests as f64
        } else {
            0.0
        };

        WindowStats {
            total_requests,
            total_successes,
            total_failures,
            failure_rate,
            consecutive_failures: self.consecutive_failures.load(Ordering::Relaxed),
            consecutive_successes: self.consecutive_successes.load(Ordering::Relaxed),
        }
    }

    fn record_state_change(&self, new_state: CircuitState) {
        self.last_state_change.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            Ordering::Relaxed
        );
        self.state_change_count.fetch_add(1, Ordering::Relaxed);

        match new_state {
            CircuitState::Open => {
                self.open_count.fetch_add(1, Ordering::Relaxed);
            }
            CircuitState::HalfOpen => {
                self.half_open_count.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
struct WindowStats {
    total_requests: u32,
    total_successes: u32,
    total_failures: u32,
    failure_rate: f64,
    consecutive_failures: u32,
    consecutive_successes: u32,
}

/// Main circuit breaker implementation
struct CircuitBreaker {
    state: AtomicU8,
    config: CircuitBreakerConfig,
    metrics: CircuitBreakerMetrics,

    // State transition timestamps
    opened_at: AtomicU64,
    half_opened_at: AtomicU64,

    // Half-open state tracking
    half_open_requests: AtomicU32,
}

impl CircuitBreaker {
    fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: AtomicU8::new(CircuitState::Closed as u8),
            metrics: CircuitBreakerMetrics::new(config.sampling_window),
            config,
            opened_at: AtomicU64::new(0),
            half_opened_at: AtomicU64::new(0),
            half_open_requests: AtomicU32::new(0),
        }
    }

    /// Check if a request should be allowed through the circuit breaker
    fn allow_request(&self) -> Result<CircuitBreakerGuard, CircuitBreakerError> {
        let current_state = self.current_state();

        match current_state {
            CircuitState::Closed => {
                // Always allow in closed state
                Ok(CircuitBreakerGuard::new(self))
            }

            CircuitState::Open => {
                // Check if timeout has elapsed to transition to half-open
                let opened_at = self.opened_at.load(Ordering::Relaxed);
                let now = Self::now_nanos();

                if now - opened_at >= self.config.timeout.as_nanos() as u64 {
                    // Attempt transition to half-open
                    if self.try_transition(CircuitState::Open, CircuitState::HalfOpen) {
                        self.half_opened_at.store(now, Ordering::Relaxed);
                        self.half_open_requests.store(0, Ordering::Relaxed);
                        self.metrics.record_state_change(CircuitState::HalfOpen);
                    }

                    // Try again now that we might be half-open
                    return self.allow_request();
                }

                // Still in open state, reject
                self.metrics.record_rejection();
                Err(CircuitBreakerError::Open {
                    opened_at: Duration::from_nanos(opened_at),
                    retry_after: self.config.timeout - Duration::from_nanos(now - opened_at),
                })
            }

            CircuitState::HalfOpen => {
                // Check if half-open timeout expired
                let half_opened_at = self.half_opened_at.load(Ordering::Relaxed);
                let now = Self::now_nanos();

                if now - half_opened_at >= self.config.half_open_timeout.as_nanos() as u64 {
                    // Half-open state timed out, reopen circuit
                    if self.try_transition(CircuitState::HalfOpen, CircuitState::Open) {
                        self.opened_at.store(now, Ordering::Relaxed);
                        self.metrics.record_state_change(CircuitState::Open);
                    }
                    return Err(CircuitBreakerError::Open {
                        opened_at: Duration::from_nanos(now),
                        retry_after: self.config.timeout,
                    });
                }

                // Allow limited concurrent requests in half-open
                let current_requests = self.half_open_requests.load(Ordering::Relaxed);

                if current_requests >= self.config.half_open_max_requests {
                    self.metrics.record_rejection();
                    return Err(CircuitBreakerError::HalfOpenLimitReached {
                        max_requests: self.config.half_open_max_requests,
                    });
                }

                // Increment half-open request counter
                self.half_open_requests.fetch_add(1, Ordering::Relaxed);
                Ok(CircuitBreakerGuard::new(self))
            }
        }
    }

    /// Record a successful operation
    fn record_success(&self) {
        self.metrics.record_success();

        let current_state = self.current_state();

        if current_state == CircuitState::HalfOpen {
            // Check if we have enough successes to close the circuit
            let stats = self.metrics.get_window_stats();

            if stats.consecutive_successes >= self.config.success_threshold {
                if self.try_transition(CircuitState::HalfOpen, CircuitState::Closed) {
                    self.metrics.record_state_change(CircuitState::Closed);
                    // Reset metrics on successful close
                    self.metrics.consecutive_failures.store(0, Ordering::Relaxed);
                    self.metrics.consecutive_successes.store(0, Ordering::Relaxed);
                }
            }
        }
    }

    /// Record a failed operation
    fn record_failure(&self) {
        self.metrics.record_failure();

        let current_state = self.current_state();

        if current_state == CircuitState::HalfOpen {
            // Any failure in half-open immediately reopens the circuit
            if self.try_transition(CircuitState::HalfOpen, CircuitState::Open) {
                let now = Self::now_nanos();
                self.opened_at.store(now, Ordering::Relaxed);
                self.metrics.record_state_change(CircuitState::Open);
            }
        } else if current_state == CircuitState::Closed {
            // Check if we should open the circuit
            let stats = self.metrics.get_window_stats();

            let should_open =
                // Threshold-based: consecutive failures
                stats.consecutive_failures >= self.config.failure_threshold ||
                // Rate-based: failure rate in window (if enough requests)
                (stats.total_requests >= self.config.min_requests &&
                 stats.failure_rate >= self.config.failure_rate_threshold);

            if should_open {
                if self.try_transition(CircuitState::Closed, CircuitState::Open) {
                    let now = Self::now_nanos();
                    self.opened_at.store(now, Ordering::Relaxed);
                    self.metrics.record_state_change(CircuitState::Open);
                }
            }
        }
    }

    /// Record a timeout (may be treated as failure based on config)
    fn record_timeout(&self) {
        self.metrics.record_timeout();

        if self.config.count_timeouts_as_failures {
            self.record_failure();
        }
    }

    /// Get current circuit state
    fn current_state(&self) -> CircuitState {
        CircuitState::from(self.state.load(Ordering::Acquire))
    }

    /// Attempt atomic state transition
    fn try_transition(&self, from: CircuitState, to: CircuitState) -> bool {
        self.state.compare_exchange(
            from as u8,
            to as u8,
            Ordering::AcqRel,
            Ordering::Acquire
        ).is_ok()
    }

    /// Force reset to closed state (for manual recovery)
    fn reset(&self) {
        self.state.store(CircuitState::Closed as u8, Ordering::Release);
        self.metrics.consecutive_failures.store(0, Ordering::Relaxed);
        self.metrics.consecutive_successes.store(0, Ordering::Relaxed);
        self.metrics.record_state_change(CircuitState::Closed);
    }

    /// Get current metrics snapshot
    fn get_metrics(&self) -> CircuitBreakerMetricsSnapshot {
        let stats = self.metrics.get_window_stats();

        CircuitBreakerMetricsSnapshot {
            state: self.current_state(),
            total_successes: self.metrics.total_successes.load(Ordering::Relaxed),
            total_failures: self.metrics.total_failures.load(Ordering::Relaxed),
            total_rejections: self.metrics.total_rejections.load(Ordering::Relaxed),
            window_failure_rate: stats.failure_rate,
            consecutive_failures: stats.consecutive_failures,
            consecutive_successes: stats.consecutive_successes,
            state_change_count: self.metrics.state_change_count.load(Ordering::Relaxed),
        }
    }

    fn now_nanos() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }
}

/// RAII guard for circuit breaker requests
struct CircuitBreakerGuard<'a> {
    breaker: &'a CircuitBreaker,
    completed: AtomicU8, // 0=pending, 1=success, 2=failure
}

impl<'a> CircuitBreakerGuard<'a> {
    fn new(breaker: &'a CircuitBreaker) -> Self {
        Self {
            breaker,
            completed: AtomicU8::new(0),
        }
    }

    fn success(self) {
        self.completed.store(1, Ordering::Release);
        self.breaker.record_success();
    }

    fn failure(self) {
        self.completed.store(2, Ordering::Release);
        self.breaker.record_failure();
    }
}

impl<'a> Drop for CircuitBreakerGuard<'a> {
    fn drop(&mut self) {
        // If not explicitly marked as success/failure, treat as failure
        let status = self.completed.load(Ordering::Acquire);
        if status == 0 {
            self.breaker.record_failure();
        }

        // Decrement half-open request counter if in half-open state
        if self.breaker.current_state() == CircuitState::HalfOpen {
            self.breaker.half_open_requests.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

#[derive(Debug, Clone)]
struct CircuitBreakerMetricsSnapshot {
    state: CircuitState,
    total_successes: u64,
    total_failures: u64,
    total_rejections: u64,
    window_failure_rate: f64,
    consecutive_failures: u32,
    consecutive_successes: u32,
    state_change_count: u64,
}

#[derive(Debug, thiserror::Error)]
enum CircuitBreakerError {
    #[error("Circuit breaker is OPEN (opened at {opened_at:?}), retry after {retry_after:?}")]
    Open {
        opened_at: Duration,
        retry_after: Duration,
    },

    #[error("Circuit breaker is HALF-OPEN with max concurrent requests reached ({max_requests})")]
    HalfOpenLimitReached {
        max_requests: u32,
    },
}

// ============================================================================
// CIRCUIT BREAKER REGISTRY - Per-Provider Management
// ============================================================================

use dashmap::DashMap;
use std::collections::HashMap;

/// Registry managing circuit breakers per provider
struct CircuitBreakerRegistry {
    breakers: DashMap<String, Arc<CircuitBreaker>>,
    default_config: CircuitBreakerConfig,
    provider_configs: DashMap<String, CircuitBreakerConfig>,
}

impl CircuitBreakerRegistry {
    fn new(default_config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: DashMap::new(),
            default_config,
            provider_configs: DashMap::new(),
        }
    }

    /// Get or create circuit breaker for a provider
    fn get_or_create(&self, provider_id: &str) -> Arc<CircuitBreaker> {
        self.breakers
            .entry(provider_id.to_string())
            .or_insert_with(|| {
                let config = self.provider_configs
                    .get(provider_id)
                    .map(|c| c.clone())
                    .unwrap_or_else(|| self.default_config.clone());

                Arc::new(CircuitBreaker::new(config))
            })
            .clone()
    }

    /// Set custom config for a specific provider
    fn set_provider_config(&self, provider_id: &str, config: CircuitBreakerConfig) {
        self.provider_configs.insert(provider_id.to_string(), config);

        // Update existing breaker if present
        if let Some(mut breaker_ref) = self.breakers.get_mut(provider_id) {
            *breaker_ref = Arc::new(CircuitBreaker::new(config));
        }
    }

    /// Get all circuit breaker states
    fn get_all_states(&self) -> HashMap<String, CircuitBreakerMetricsSnapshot> {
        self.breakers
            .iter()
            .map(|entry| {
                let provider_id = entry.key().clone();
                let metrics = entry.value().get_metrics();
                (provider_id, metrics)
            })
            .collect()
    }

    /// Get specific provider state
    fn get_state(&self, provider_id: &str) -> Option<CircuitBreakerMetricsSnapshot> {
        self.breakers
            .get(provider_id)
            .map(|breaker| breaker.get_metrics())
    }

    /// Reset all circuit breakers
    fn reset_all(&self) {
        for entry in self.breakers.iter() {
            entry.value().reset();
        }
    }

    /// Reset specific provider circuit breaker
    fn reset(&self, provider_id: &str) -> bool {
        if let Some(breaker) = self.breakers.get(provider_id) {
            breaker.reset();
            true
        } else {
            false
        }
    }

    /// Remove circuit breaker for a provider
    fn remove(&self, provider_id: &str) -> bool {
        self.breakers.remove(provider_id).is_some()
    }

    /// Get count of active circuit breakers
    fn count(&self) -> usize {
        self.breakers.len()
    }
}

```

---

## 2. Retry Policy with Exponential Backoff

```rust
// ============================================================================
// RETRY POLICY - Exponential Backoff with Jitter
// ============================================================================

use rand::Rng;
use std::collections::HashSet;

/// Classification of errors for retry decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ErrorKind {
    // Transient network errors (retryable)
    ConnectionRefused,
    ConnectionReset,
    ConnectionTimeout,
    Timeout,
    TooManyRequests,  // 429

    // Server errors (potentially retryable)
    InternalServerError,  // 500
    BadGateway,           // 502
    ServiceUnavailable,   // 503
    GatewayTimeout,       // 504

    // Client errors (non-retryable)
    BadRequest,           // 400
    Unauthorized,         // 401
    Forbidden,            // 403
    NotFound,             // 404
    UnprocessableEntity,  // 422

    // Application errors
    RateLimitExceeded,
    QuotaExceeded,
    InvalidResponse,
    CircuitBreakerOpen,
}

impl ErrorKind {
    fn from_http_status(status: u16) -> Self {
        match status {
            400 => ErrorKind::BadRequest,
            401 => ErrorKind::Unauthorized,
            403 => ErrorKind::Forbidden,
            404 => ErrorKind::NotFound,
            422 => ErrorKind::UnprocessableEntity,
            429 => ErrorKind::TooManyRequests,
            500 => ErrorKind::InternalServerError,
            502 => ErrorKind::BadGateway,
            503 => ErrorKind::ServiceUnavailable,
            504 => ErrorKind::GatewayTimeout,
            _ => ErrorKind::InvalidResponse,
        }
    }
}

/// Retry policy configuration
#[derive(Debug, Clone)]
struct RetryPolicy {
    max_retries: u32,
    base_delay: Duration,
    max_delay: Duration,
    multiplier: f64,
    jitter_strategy: JitterStrategy,
    retryable_errors: HashSet<ErrorKind>,
    backoff_strategy: BackoffStrategy,
}

#[derive(Debug, Clone, Copy)]
enum JitterStrategy {
    None,
    Full,          // Random between 0 and computed delay
    Equal,         // Half computed delay + random half
    Decorrelated,  // More sophisticated, considers previous delay
}

#[derive(Debug, Clone, Copy)]
enum BackoffStrategy {
    Exponential,   // delay = base * multiplier^attempt
    Linear,        // delay = base * attempt
    Fibonacci,     // delay follows fibonacci sequence
}

impl Default for RetryPolicy {
    fn default() -> Self {
        let mut retryable_errors = HashSet::new();
        retryable_errors.insert(ErrorKind::ConnectionRefused);
        retryable_errors.insert(ErrorKind::ConnectionReset);
        retryable_errors.insert(ErrorKind::ConnectionTimeout);
        retryable_errors.insert(ErrorKind::Timeout);
        retryable_errors.insert(ErrorKind::TooManyRequests);
        retryable_errors.insert(ErrorKind::InternalServerError);
        retryable_errors.insert(ErrorKind::BadGateway);
        retryable_errors.insert(ErrorKind::ServiceUnavailable);
        retryable_errors.insert(ErrorKind::GatewayTimeout);

        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            jitter_strategy: JitterStrategy::Decorrelated,
            retryable_errors,
            backoff_strategy: BackoffStrategy::Exponential,
        }
    }
}

impl RetryPolicy {
    /// Check if an error should be retried
    fn should_retry(&self, error_kind: ErrorKind, attempt: u32) -> bool {
        if attempt >= self.max_retries {
            return false;
        }

        self.retryable_errors.contains(&error_kind)
    }

    /// Calculate delay for next retry attempt
    fn get_delay(&self, attempt: u32, previous_delay: Option<Duration>) -> Duration {
        let base_delay = self.calculate_base_delay(attempt);
        let jittered_delay = self.apply_jitter(base_delay, previous_delay);

        // Cap at max delay
        std::cmp::min(jittered_delay, self.max_delay)
    }

    fn calculate_base_delay(&self, attempt: u32) -> Duration {
        match self.backoff_strategy {
            BackoffStrategy::Exponential => {
                let delay_ms = self.base_delay.as_millis() as f64
                    * self.multiplier.powi(attempt as i32);
                Duration::from_millis(delay_ms as u64)
            }

            BackoffStrategy::Linear => {
                let delay_ms = self.base_delay.as_millis() as u64 * (attempt as u64 + 1);
                Duration::from_millis(delay_ms)
            }

            BackoffStrategy::Fibonacci => {
                let fib = Self::fibonacci(attempt as usize);
                let delay_ms = self.base_delay.as_millis() as u64 * fib;
                Duration::from_millis(delay_ms)
            }
        }
    }

    fn apply_jitter(&self, base_delay: Duration, previous_delay: Option<Duration>) -> Duration {
        let mut rng = rand::thread_rng();

        match self.jitter_strategy {
            JitterStrategy::None => base_delay,

            JitterStrategy::Full => {
                // Random between 0 and base_delay
                let max_ms = base_delay.as_millis() as u64;
                let jittered_ms = rng.gen_range(0..=max_ms);
                Duration::from_millis(jittered_ms)
            }

            JitterStrategy::Equal => {
                // Half base_delay + random half
                let base_ms = base_delay.as_millis() as u64;
                let half = base_ms / 2;
                let jitter = rng.gen_range(0..=half);
                Duration::from_millis(half + jitter)
            }

            JitterStrategy::Decorrelated => {
                // Decorrelated jitter: random between base_delay and 3 * previous_delay
                // This creates better distribution and avoids thundering herd
                let base_ms = base_delay.as_millis() as u64;

                if let Some(prev) = previous_delay {
                    let prev_ms = prev.as_millis() as u64;
                    let max_ms = std::cmp::min(base_ms, prev_ms * 3);
                    let jittered_ms = rng.gen_range(base_ms..=max_ms);
                    Duration::from_millis(jittered_ms)
                } else {
                    // First retry, use full jitter
                    let jittered_ms = rng.gen_range(0..=base_ms);
                    Duration::from_millis(jittered_ms)
                }
            }
        }
    }

    fn fibonacci(n: usize) -> u64 {
        match n {
            0 => 1,
            1 => 1,
            _ => {
                let mut a = 1u64;
                let mut b = 1u64;
                for _ in 2..=n {
                    let temp = a + b;
                    a = b;
                    b = temp;
                }
                b
            }
        }
    }

    /// Execute an operation with retry logic
    async fn execute<F, T, Fut>(&self, mut operation: F) -> Result<T, RetryError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, GatewayError>>,
    {
        let mut attempt = 0u32;
        let mut previous_delay = None;
        let mut last_error = None;

        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    let error_kind = Self::classify_error(&err);

                    if !self.should_retry(error_kind, attempt) {
                        return Err(RetryError::NonRetryable {
                            error: err,
                            attempts: attempt + 1,
                        });
                    }

                    if attempt >= self.max_retries {
                        return Err(RetryError::MaxRetriesExceeded {
                            last_error: err,
                            attempts: attempt + 1,
                        });
                    }

                    // Calculate and apply backoff
                    let delay = self.get_delay(attempt, previous_delay);
                    previous_delay = Some(delay);

                    tokio::time::sleep(delay).await;

                    attempt += 1;
                    last_error = Some(err);
                }
            }
        }
    }

    /// Execute with retry budget check
    async fn execute_with_budget<F, T, Fut>(
        &self,
        retry_budget: &RetryBudget,
        mut operation: F,
    ) -> Result<T, RetryError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, GatewayError>>,
    {
        let mut attempt = 0u32;
        let mut previous_delay = None;

        loop {
            match operation().await {
                Ok(result) => {
                    // Release budget on success if we used retries
                    if attempt > 0 {
                        retry_budget.release();
                    }
                    return Ok(result);
                }
                Err(err) => {
                    let error_kind = Self::classify_error(&err);

                    if !self.should_retry(error_kind, attempt) {
                        return Err(RetryError::NonRetryable {
                            error: err,
                            attempts: attempt + 1,
                        });
                    }

                    if attempt >= self.max_retries {
                        return Err(RetryError::MaxRetriesExceeded {
                            last_error: err,
                            attempts: attempt + 1,
                        });
                    }

                    // Check retry budget before retrying
                    if !retry_budget.try_acquire() {
                        return Err(RetryError::BudgetExhausted {
                            error: err,
                            attempts: attempt + 1,
                        });
                    }

                    let delay = self.get_delay(attempt, previous_delay);
                    previous_delay = Some(delay);

                    tokio::time::sleep(delay).await;

                    attempt += 1;
                }
            }
        }
    }

    fn classify_error(error: &GatewayError) -> ErrorKind {
        // Classify error based on error type
        match error {
            GatewayError::Timeout(_) => ErrorKind::Timeout,
            GatewayError::ConnectionFailed(_) => ErrorKind::ConnectionRefused,
            GatewayError::HttpStatus(status, _) => ErrorKind::from_http_status(*status),
            GatewayError::RateLimitExceeded(_) => ErrorKind::RateLimitExceeded,
            GatewayError::CircuitBreakerOpen(_) => ErrorKind::CircuitBreakerOpen,
            _ => ErrorKind::InvalidResponse,
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum RetryError {
    #[error("Max retries exceeded ({attempts} attempts): {last_error}")]
    MaxRetriesExceeded {
        last_error: GatewayError,
        attempts: u32,
    },

    #[error("Non-retryable error after {attempts} attempts: {error}")]
    NonRetryable {
        error: GatewayError,
        attempts: u32,
    },

    #[error("Retry budget exhausted after {attempts} attempts: {error}")]
    BudgetExhausted {
        error: GatewayError,
        attempts: u32,
    },
}

/// Placeholder for gateway error type
#[derive(Debug, thiserror::Error)]
enum GatewayError {
    #[error("Request timeout: {0}")]
    Timeout(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("HTTP status {0}: {1}")]
    HttpStatus(u16, String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Circuit breaker open: {0}")]
    CircuitBreakerOpen(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

```

---

## 3. Retry Budget

```rust
// ============================================================================
// RETRY BUDGET - Token Bucket for Retry Rate Limiting
// ============================================================================

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Token bucket implementation for retry budget management
struct RetryBudget {
    budget: AtomicU32,
    max_budget: u32,
    refill_rate: f64,  // permits per second
    last_refill: AtomicU64,  // timestamp in nanos
}

impl RetryBudget {
    fn new(max_budget: u32, refill_rate: f64) -> Self {
        Self {
            budget: AtomicU32::new(max_budget),
            max_budget,
            refill_rate,
            last_refill: AtomicU64::new(Self::now_nanos()),
        }
    }

    /// Try to acquire a retry permit
    fn try_acquire(&self) -> bool {
        // Refill bucket based on elapsed time
        self.refill();

        // Try to acquire a permit
        let mut current = self.budget.load(Ordering::Acquire);

        loop {
            if current == 0 {
                return false;
            }

            match self.budget.compare_exchange_weak(
                current,
                current - 1,
                Ordering::AcqRel,
                Ordering::Acquire
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }

    /// Release a permit (called on successful retry)
    fn release(&self) {
        let mut current = self.budget.load(Ordering::Acquire);

        loop {
            if current >= self.max_budget {
                return;
            }

            match self.budget.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire
            ) {
                Ok(_) => return,
                Err(actual) => current = actual,
            }
        }
    }

    /// Get available permits
    fn available(&self) -> u32 {
        self.refill();
        self.budget.load(Ordering::Relaxed)
    }

    /// Refill bucket based on elapsed time
    fn refill(&self) {
        let now = Self::now_nanos();
        let last = self.last_refill.load(Ordering::Acquire);

        let elapsed = Duration::from_nanos(now - last);
        let elapsed_secs = elapsed.as_secs_f64();

        let tokens_to_add = (elapsed_secs * self.refill_rate) as u32;

        if tokens_to_add > 0 {
            // Update last refill time
            self.last_refill.compare_exchange(
                last,
                now,
                Ordering::AcqRel,
                Ordering::Acquire
            ).ok();

            // Add tokens up to max
            let mut current = self.budget.load(Ordering::Acquire);

            loop {
                let new_budget = std::cmp::min(current + tokens_to_add, self.max_budget);

                if new_budget == current {
                    break;
                }

                match self.budget.compare_exchange_weak(
                    current,
                    new_budget,
                    Ordering::AcqRel,
                    Ordering::Acquire
                ) {
                    Ok(_) => break,
                    Err(actual) => current = actual,
                }
            }
        }
    }

    /// Get refill rate
    fn get_refill_rate(&self) -> f64 {
        self.refill_rate
    }

    /// Update refill rate dynamically
    fn set_refill_rate(&mut self, rate: f64) {
        self.refill_rate = rate;
    }

    fn now_nanos() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }
}

/// Adaptive retry budget that adjusts based on success rate
struct AdaptiveRetryBudget {
    inner: RetryBudget,
    min_refill_rate: f64,
    max_refill_rate: f64,
    success_count: AtomicU32,
    failure_count: AtomicU32,
    last_adjustment: AtomicU64,
    adjustment_interval: Duration,
}

impl AdaptiveRetryBudget {
    fn new(
        max_budget: u32,
        initial_refill_rate: f64,
        min_refill_rate: f64,
        max_refill_rate: f64,
    ) -> Self {
        Self {
            inner: RetryBudget::new(max_budget, initial_refill_rate),
            min_refill_rate,
            max_refill_rate,
            success_count: AtomicU32::new(0),
            failure_count: AtomicU32::new(0),
            last_adjustment: AtomicU64::new(RetryBudget::now_nanos()),
            adjustment_interval: Duration::from_secs(60),
        }
    }

    fn try_acquire(&self) -> bool {
        self.adjust_if_needed();
        self.inner.try_acquire()
    }

    fn release_success(&self) {
        self.success_count.fetch_add(1, Ordering::Relaxed);
        self.inner.release();
    }

    fn release_failure(&self) {
        self.failure_count.fetch_add(1, Ordering::Relaxed);
    }

    fn adjust_if_needed(&self) {
        let now = RetryBudget::now_nanos();
        let last = self.last_adjustment.load(Ordering::Acquire);

        if Duration::from_nanos(now - last) < self.adjustment_interval {
            return;
        }

        // Try to acquire adjustment lock
        if self.last_adjustment.compare_exchange(
            last,
            now,
            Ordering::AcqRel,
            Ordering::Acquire
        ).is_err() {
            return; // Another thread is adjusting
        }

        let successes = self.success_count.swap(0, Ordering::Relaxed);
        let failures = self.failure_count.swap(0, Ordering::Relaxed);

        let total = successes + failures;
        if total < 10 {
            return; // Not enough data
        }

        let success_rate = successes as f64 / total as f64;

        // Adjust refill rate based on success rate
        let current_rate = self.inner.get_refill_rate();
        let new_rate = if success_rate > 0.8 {
            // High success rate, increase refill rate
            (current_rate * 1.1).min(self.max_refill_rate)
        } else if success_rate < 0.5 {
            // Low success rate, decrease refill rate
            (current_rate * 0.9).max(self.min_refill_rate)
        } else {
            current_rate
        };

        // Update refill rate (requires mutable access in real implementation)
        // This would need interior mutability or atomic float operations
        // For now, this is conceptual
    }

    fn available(&self) -> u32 {
        self.inner.available()
    }
}

```

---

## 4. Bulkhead Pattern

```rust
// ============================================================================
// BULKHEAD PATTERN - Concurrency Limiting with Isolation
// ============================================================================

use tokio::sync::{Semaphore, SemaphorePermit};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Bulkhead for isolating concurrent operations
struct Bulkhead {
    semaphore: Arc<Semaphore>,
    max_concurrent: usize,
    max_wait: Duration,
    metrics: BulkheadMetrics,
}

impl Bulkhead {
    fn new(max_concurrent: usize, max_wait: Duration) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            max_concurrent,
            max_wait,
            metrics: BulkheadMetrics::new(),
        }
    }

    /// Acquire permit with timeout
    async fn acquire(&self) -> Result<BulkheadPermit, BulkheadError> {
        let start = Instant::now();

        self.metrics.record_acquire_attempt();

        let permit = tokio::time::timeout(
            self.max_wait,
            self.semaphore.clone().acquire_owned()
        )
        .await
        .map_err(|_| BulkheadError::AcquireTimeout {
            max_wait: self.max_wait,
        })?
        .map_err(|_| BulkheadError::Closed)?;

        let wait_time = start.elapsed();
        self.metrics.record_acquire_success(wait_time);

        Ok(BulkheadPermit::new(
            permit,
            Arc::clone(&self.semaphore),
            &self.metrics,
            start
        ))
    }

    /// Try to acquire permit without waiting
    fn try_acquire(&self) -> Option<BulkheadPermit> {
        let start = Instant::now();

        self.metrics.record_acquire_attempt();

        self.semaphore.clone().try_acquire_owned().ok().map(|permit| {
            self.metrics.record_acquire_success(Duration::ZERO);
            BulkheadPermit::new(
                permit,
                Arc::clone(&self.semaphore),
                &self.metrics,
                start
            )
        })
    }

    /// Get number of available permits
    fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Get utilization percentage (0.0 - 1.0)
    fn utilization(&self) -> f64 {
        let available = self.available_permits();
        let used = self.max_concurrent - available;
        used as f64 / self.max_concurrent as f64
    }

    fn get_metrics(&self) -> BulkheadMetricsSnapshot {
        self.metrics.snapshot()
    }
}

/// RAII guard for bulkhead permit
struct BulkheadPermit {
    _permit: SemaphorePermit<'static>,
    semaphore: Arc<Semaphore>,
    metrics: *const BulkheadMetrics,  // Raw pointer for lifetime flexibility
    acquired_at: Instant,
}

impl BulkheadPermit {
    fn new(
        permit: SemaphorePermit<'static>,
        semaphore: Arc<Semaphore>,
        metrics: &BulkheadMetrics,
        acquired_at: Instant,
    ) -> Self {
        Self {
            _permit: permit,
            semaphore,
            metrics: metrics as *const BulkheadMetrics,
            acquired_at,
        }
    }
}

impl Drop for BulkheadPermit {
    fn drop(&mut self) {
        let duration = self.acquired_at.elapsed();

        // Safe because metrics pointer is valid for the lifetime of Bulkhead
        unsafe {
            if let Some(metrics) = self.metrics.as_ref() {
                metrics.record_release(duration);
            }
        }
    }
}

/// Metrics for bulkhead monitoring
struct BulkheadMetrics {
    acquire_attempts: AtomicU64,
    acquire_successes: AtomicU64,
    acquire_failures: AtomicU64,

    total_wait_time_nanos: AtomicU64,
    total_hold_time_nanos: AtomicU64,

    current_utilization: AtomicU32,  // Percentage * 100
    peak_utilization: AtomicU32,     // Percentage * 100
}

impl BulkheadMetrics {
    fn new() -> Self {
        Self {
            acquire_attempts: AtomicU64::new(0),
            acquire_successes: AtomicU64::new(0),
            acquire_failures: AtomicU64::new(0),
            total_wait_time_nanos: AtomicU64::new(0),
            total_hold_time_nanos: AtomicU64::new(0),
            current_utilization: AtomicU32::new(0),
            peak_utilization: AtomicU32::new(0),
        }
    }

    fn record_acquire_attempt(&self) {
        self.acquire_attempts.fetch_add(1, Ordering::Relaxed);
    }

    fn record_acquire_success(&self, wait_time: Duration) {
        self.acquire_successes.fetch_add(1, Ordering::Relaxed);
        self.total_wait_time_nanos.fetch_add(
            wait_time.as_nanos() as u64,
            Ordering::Relaxed
        );
    }

    fn record_acquire_failure(&self) {
        self.acquire_failures.fetch_add(1, Ordering::Relaxed);
    }

    fn record_release(&self, hold_time: Duration) {
        self.total_hold_time_nanos.fetch_add(
            hold_time.as_nanos() as u64,
            Ordering::Relaxed
        );
    }

    fn update_utilization(&self, utilization: f64) {
        let util_percent = (utilization * 100.0) as u32;
        self.current_utilization.store(util_percent, Ordering::Relaxed);

        // Update peak
        let mut peak = self.peak_utilization.load(Ordering::Relaxed);
        while util_percent > peak {
            match self.peak_utilization.compare_exchange_weak(
                peak,
                util_percent,
                Ordering::Relaxed,
                Ordering::Relaxed
            ) {
                Ok(_) => break,
                Err(actual) => peak = actual,
            }
        }
    }

    fn snapshot(&self) -> BulkheadMetricsSnapshot {
        let attempts = self.acquire_attempts.load(Ordering::Relaxed);
        let successes = self.acquire_successes.load(Ordering::Relaxed);
        let failures = self.acquire_failures.load(Ordering::Relaxed);

        let avg_wait_time = if successes > 0 {
            Duration::from_nanos(
                self.total_wait_time_nanos.load(Ordering::Relaxed) / successes
            )
        } else {
            Duration::ZERO
        };

        let avg_hold_time = if successes > 0 {
            Duration::from_nanos(
                self.total_hold_time_nanos.load(Ordering::Relaxed) / successes
            )
        } else {
            Duration::ZERO
        };

        BulkheadMetricsSnapshot {
            acquire_attempts: attempts,
            acquire_successes: successes,
            acquire_failures: failures,
            avg_wait_time,
            avg_hold_time,
            current_utilization: self.current_utilization.load(Ordering::Relaxed) as f64 / 100.0,
            peak_utilization: self.peak_utilization.load(Ordering::Relaxed) as f64 / 100.0,
        }
    }
}

#[derive(Debug, Clone)]
struct BulkheadMetricsSnapshot {
    acquire_attempts: u64,
    acquire_successes: u64,
    acquire_failures: u64,
    avg_wait_time: Duration,
    avg_hold_time: Duration,
    current_utilization: f64,
    peak_utilization: f64,
}

#[derive(Debug, thiserror::Error)]
enum BulkheadError {
    #[error("Failed to acquire bulkhead permit within {max_wait:?}")]
    AcquireTimeout {
        max_wait: Duration,
    },

    #[error("Bulkhead is closed")]
    Closed,
}

/// Registry for managing multiple bulkheads
struct BulkheadRegistry {
    bulkheads: DashMap<String, Arc<Bulkhead>>,
    default_max_concurrent: usize,
    default_max_wait: Duration,
}

impl BulkheadRegistry {
    fn new(default_max_concurrent: usize, default_max_wait: Duration) -> Self {
        Self {
            bulkheads: DashMap::new(),
            default_max_concurrent,
            default_max_wait,
        }
    }

    fn get_or_create(&self, name: &str) -> Arc<Bulkhead> {
        self.bulkheads
            .entry(name.to_string())
            .or_insert_with(|| {
                Arc::new(Bulkhead::new(
                    self.default_max_concurrent,
                    self.default_max_wait
                ))
            })
            .clone()
    }

    fn get_all_metrics(&self) -> HashMap<String, BulkheadMetricsSnapshot> {
        self.bulkheads
            .iter()
            .map(|entry| {
                (entry.key().clone(), entry.value().get_metrics())
            })
            .collect()
    }
}

```

---

## 5. Timeout Management

```rust
// ============================================================================
// TIMEOUT MANAGEMENT - Hierarchical Timeout Control
// ============================================================================

use tokio::time::{timeout, Duration};
use std::future::Future;

#[derive(Debug, Clone, Copy)]
enum TimeoutType {
    Gateway,      // Overall request timeout
    Provider,     // Per-provider attempt timeout
    Connection,   // TCP/TLS connection timeout
    Stream,       // Streaming response timeout
    Idle,         // Idle connection timeout
}

/// Timeout configuration manager
#[derive(Debug, Clone)]
struct TimeoutManager {
    connect_timeout: Duration,
    request_timeout: Duration,
    stream_timeout: Duration,
    idle_timeout: Duration,
    provider_timeout: Duration,
    gateway_timeout: Duration,
}

impl Default for TimeoutManager {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            stream_timeout: Duration::from_secs(60),
            idle_timeout: Duration::from_secs(90),
            provider_timeout: Duration::from_secs(25),
            gateway_timeout: Duration::from_secs(60),
        }
    }
}

impl TimeoutManager {
    fn new(
        connect_timeout: Duration,
        request_timeout: Duration,
        stream_timeout: Duration,
        idle_timeout: Duration,
    ) -> Self {
        Self {
            connect_timeout,
            request_timeout,
            stream_timeout,
            idle_timeout,
            provider_timeout: request_timeout - Duration::from_secs(5),
            gateway_timeout: request_timeout + Duration::from_secs(30),
        }
    }

    /// Execute future with specific timeout
    async fn with_timeout<F, T>(
        &self,
        timeout_type: TimeoutType,
        future: F,
    ) -> Result<T, TimeoutError>
    where
        F: Future<Output = Result<T, GatewayError>>,
    {
        let timeout_duration = self.get_timeout(timeout_type);

        match timeout(timeout_duration, future).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(err)) => Err(TimeoutError::OperationFailed(err)),
            Err(_) => Err(TimeoutError::Timeout {
                timeout_type,
                duration: timeout_duration,
            }),
        }
    }

    fn get_timeout(&self, timeout_type: TimeoutType) -> Duration {
        match timeout_type {
            TimeoutType::Gateway => self.gateway_timeout,
            TimeoutType::Provider => self.provider_timeout,
            TimeoutType::Connection => self.connect_timeout,
            TimeoutType::Stream => self.stream_timeout,
            TimeoutType::Idle => self.idle_timeout,
        }
    }

    /// Create nested timeout context (provider within gateway)
    fn create_nested_context(&self) -> NestedTimeoutContext {
        NestedTimeoutContext {
            gateway_deadline: Instant::now() + self.gateway_timeout,
            provider_timeout: self.provider_timeout,
            connection_timeout: self.connect_timeout,
        }
    }
}

/// Hierarchical timeout context for nested operations
struct NestedTimeoutContext {
    gateway_deadline: Instant,
    provider_timeout: Duration,
    connection_timeout: Duration,
}

impl NestedTimeoutContext {
    /// Get remaining time for provider operation
    fn remaining_provider_time(&self) -> Option<Duration> {
        let now = Instant::now();
        if now >= self.gateway_deadline {
            return None;
        }

        let remaining = self.gateway_deadline - now;
        Some(std::cmp::min(remaining, self.provider_timeout))
    }

    /// Check if gateway timeout exceeded
    fn is_expired(&self) -> bool {
        Instant::now() >= self.gateway_deadline
    }

    /// Execute with remaining provider timeout
    async fn with_provider_timeout<F, T>(
        &self,
        future: F,
    ) -> Result<T, TimeoutError>
    where
        F: Future<Output = Result<T, GatewayError>>,
    {
        let remaining = self.remaining_provider_time()
            .ok_or(TimeoutError::Timeout {
                timeout_type: TimeoutType::Gateway,
                duration: Duration::ZERO,
            })?;

        match timeout(remaining, future).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(err)) => Err(TimeoutError::OperationFailed(err)),
            Err(_) => Err(TimeoutError::Timeout {
                timeout_type: TimeoutType::Provider,
                duration: remaining,
            }),
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum TimeoutError {
    #[error("Operation timed out ({timeout_type:?} after {duration:?})")]
    Timeout {
        timeout_type: TimeoutType,
        duration: Duration,
    },

    #[error("Operation failed: {0}")]
    OperationFailed(GatewayError),
}

```

---

## 6. Resilience Coordinator

```rust
// ============================================================================
// RESILIENCE COORDINATOR - Unified Fault Tolerance Layer
// ============================================================================

use std::collections::HashMap;
use std::sync::Arc;

/// Central coordinator for all resilience mechanisms
struct ResilienceCoordinator {
    circuit_breakers: Arc<CircuitBreakerRegistry>,
    bulkheads: Arc<BulkheadRegistry>,
    retry_policies: HashMap<String, RetryPolicy>,
    timeout_manager: TimeoutManager,
    retry_budget: Arc<AdaptiveRetryBudget>,
    default_retry_policy: RetryPolicy,
}

impl ResilienceCoordinator {
    fn new(
        circuit_breaker_config: CircuitBreakerConfig,
        default_retry_policy: RetryPolicy,
        timeout_manager: TimeoutManager,
        retry_budget: Arc<AdaptiveRetryBudget>,
    ) -> Self {
        Self {
            circuit_breakers: Arc::new(CircuitBreakerRegistry::new(circuit_breaker_config)),
            bulkheads: Arc::new(BulkheadRegistry::new(100, Duration::from_secs(5))),
            retry_policies: HashMap::new(),
            timeout_manager,
            retry_budget,
            default_retry_policy,
        }
    }

    /// Execute operation with full resilience stack
    async fn execute_with_resilience<F, T, Fut>(
        &self,
        provider_id: &str,
        operation: F,
    ) -> Result<T, ResilienceError>
    where
        F: Fn() -> Fut + Clone,
        Fut: Future<Output = Result<T, GatewayError>>,
    {
        // 1. Create timeout context
        let timeout_ctx = self.timeout_manager.create_nested_context();

        // 2. Acquire bulkhead permit
        let bulkhead = self.bulkheads.get_or_create(provider_id);
        let _permit = bulkhead.acquire()
            .await
            .map_err(|e| ResilienceError::BulkheadRejection {
                provider: provider_id.to_string(),
                error: e,
            })?;

        // 3. Check circuit breaker
        let circuit_breaker = self.circuit_breakers.get_or_create(provider_id);

        // 4. Get retry policy
        let retry_policy = self.retry_policies
            .get(provider_id)
            .unwrap_or(&self.default_retry_policy);

        // 5. Execute with retry logic
        let result = self.execute_with_retry_and_circuit_breaker(
            &circuit_breaker,
            retry_policy,
            &timeout_ctx,
            operation,
        ).await;

        result
    }

    async fn execute_with_retry_and_circuit_breaker<F, T, Fut>(
        &self,
        circuit_breaker: &CircuitBreaker,
        retry_policy: &RetryPolicy,
        timeout_ctx: &NestedTimeoutContext,
        operation: F,
    ) -> Result<T, ResilienceError>
    where
        F: Fn() -> Fut + Clone,
        Fut: Future<Output = Result<T, GatewayError>>,
    {
        let mut attempt = 0u32;
        let mut previous_delay = None;

        loop {
            // Check gateway timeout
            if timeout_ctx.is_expired() {
                return Err(ResilienceError::GatewayTimeout);
            }

            // Check circuit breaker
            let cb_guard = circuit_breaker.allow_request()
                .map_err(|e| ResilienceError::CircuitBreakerOpen {
                    error: e,
                })?;

            // Execute operation with provider timeout
            let result = timeout_ctx.with_provider_timeout(operation()).await;

            match result {
                Ok(value) => {
                    // Success
                    cb_guard.success();

                    if attempt > 0 {
                        self.retry_budget.release_success();
                    }

                    return Ok(value);
                }
                Err(TimeoutError::Timeout { .. }) => {
                    // Timeout
                    circuit_breaker.record_timeout();
                    drop(cb_guard);

                    if !retry_policy.should_retry(ErrorKind::Timeout, attempt) {
                        return Err(ResilienceError::ProviderTimeout);
                    }
                }
                Err(TimeoutError::OperationFailed(err)) => {
                    // Operation error
                    let error_kind = RetryPolicy::classify_error(&err);
                    cb_guard.failure();

                    if !retry_policy.should_retry(error_kind, attempt) {
                        return Err(ResilienceError::NonRetryableError { error: err });
                    }
                }
            }

            // Check retry budget
            if !self.retry_budget.try_acquire() {
                return Err(ResilienceError::RetryBudgetExhausted);
            }

            if attempt >= retry_policy.max_retries {
                return Err(ResilienceError::MaxRetriesExceeded {
                    attempts: attempt + 1,
                });
            }

            // Calculate and apply backoff
            let delay = retry_policy.get_delay(attempt, previous_delay);
            previous_delay = Some(delay);

            tokio::time::sleep(delay).await;
            attempt += 1;
        }
    }

    /// Execute with graceful degradation
    async fn execute_with_fallback<F, T, Fut, Fallback, FallbackFut>(
        &self,
        provider_id: &str,
        primary: F,
        fallback: Fallback,
    ) -> Result<T, ResilienceError>
    where
        F: Fn() -> Fut + Clone,
        Fut: Future<Output = Result<T, GatewayError>>,
        Fallback: Fn() -> FallbackFut,
        FallbackFut: Future<Output = Result<T, GatewayError>>,
    {
        match self.execute_with_resilience(provider_id, primary).await {
            Ok(result) => Ok(result),
            Err(primary_error) => {
                // Attempt fallback
                match fallback().await {
                    Ok(result) => Ok(result),
                    Err(fallback_error) => Err(ResilienceError::AllProvidersFailed {
                        primary_error: Box::new(primary_error),
                        fallback_error,
                    }),
                }
            }
        }
    }

    /// Set custom retry policy for provider
    fn set_retry_policy(&mut self, provider_id: &str, policy: RetryPolicy) {
        self.retry_policies.insert(provider_id.to_string(), policy);
    }

    /// Get resilience health status
    fn get_health_status(&self) -> ResilienceHealthStatus {
        let circuit_states = self.circuit_breakers.get_all_states();
        let bulkhead_metrics = self.bulkheads.get_all_metrics();
        let retry_budget_available = self.retry_budget.available();

        // Determine overall health
        let open_circuits = circuit_states.iter()
            .filter(|(_, state)| state.state == CircuitState::Open)
            .count();

        let degraded_providers = circuit_states.iter()
            .filter(|(_, state)| {
                state.state == CircuitState::HalfOpen ||
                state.window_failure_rate > 0.3
            })
            .count();

        let health_status = if open_circuits > 0 {
            HealthState::Degraded
        } else if degraded_providers > 0 || retry_budget_available < 10 {
            HealthState::Warning
        } else {
            HealthState::Healthy
        };

        ResilienceHealthStatus {
            status: health_status,
            circuit_breaker_states: circuit_states,
            bulkhead_metrics,
            retry_budget_available,
            open_circuit_count: open_circuits,
            degraded_provider_count: degraded_providers,
        }
    }

    /// Reset all resilience components
    fn reset_all(&self) {
        self.circuit_breakers.reset_all();
    }

    /// Reset specific provider
    fn reset_provider(&self, provider_id: &str) {
        self.circuit_breakers.reset(provider_id);
    }
}

#[derive(Debug)]
struct ResilienceHealthStatus {
    status: HealthState,
    circuit_breaker_states: HashMap<String, CircuitBreakerMetricsSnapshot>,
    bulkhead_metrics: HashMap<String, BulkheadMetricsSnapshot>,
    retry_budget_available: u32,
    open_circuit_count: usize,
    degraded_provider_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HealthState {
    Healthy,
    Warning,
    Degraded,
    Critical,
}

#[derive(Debug, thiserror::Error)]
enum ResilienceError {
    #[error("Circuit breaker is open: {error}")]
    CircuitBreakerOpen {
        error: CircuitBreakerError,
    },

    #[error("Bulkhead rejection for provider {provider}: {error}")]
    BulkheadRejection {
        provider: String,
        error: BulkheadError,
    },

    #[error("Gateway timeout exceeded")]
    GatewayTimeout,

    #[error("Provider timeout exceeded")]
    ProviderTimeout,

    #[error("Retry budget exhausted")]
    RetryBudgetExhausted,

    #[error("Max retries exceeded ({attempts} attempts)")]
    MaxRetriesExceeded {
        attempts: u32,
    },

    #[error("Non-retryable error: {error}")]
    NonRetryableError {
        error: GatewayError,
    },

    #[error("All providers failed - primary: {primary_error}, fallback: {fallback_error}")]
    AllProvidersFailed {
        primary_error: Box<ResilienceError>,
        fallback_error: GatewayError,
    },
}

```

---

## 7. Graceful Degradation Strategies

```rust
// ============================================================================
// GRACEFUL DEGRADATION - Fallback and Recovery Strategies
// ============================================================================

/// Degradation strategy for handling failures
#[derive(Debug, Clone)]
enum DegradationStrategy {
    /// Return cached response if available
    CachedResponse {
        max_age: Duration,
    },

    /// Use fallback provider
    FallbackProvider {
        provider_id: String,
    },

    /// Return simplified response
    SimplifiedResponse,

    /// Return error with suggested alternatives
    ErrorWithAlternatives {
        alternatives: Vec<String>,
    },

    /// Queue request for later processing
    QueueForRetry {
        queue_timeout: Duration,
    },
}

/// Degradation policy manager
struct DegradationPolicy {
    strategies: Vec<DegradationStrategy>,
    cache: Arc<ResponseCache>,
}

impl DegradationPolicy {
    fn new(strategies: Vec<DegradationStrategy>) -> Self {
        Self {
            strategies,
            cache: Arc::new(ResponseCache::new(1000, Duration::from_secs(300))),
        }
    }

    /// Apply degradation strategies in order
    async fn apply_degradation<T>(
        &self,
        request_id: &str,
        error: &ResilienceError,
    ) -> Result<T, ResilienceError> {
        for strategy in &self.strategies {
            match strategy {
                DegradationStrategy::CachedResponse { max_age } => {
                    if let Some(cached) = self.cache.get(request_id, *max_age) {
                        return Ok(cached);
                    }
                }

                DegradationStrategy::FallbackProvider { provider_id } => {
                    // Attempt fallback provider
                    // (Implementation would involve calling resilience coordinator)
                    continue;
                }

                DegradationStrategy::SimplifiedResponse => {
                    // Return simplified version
                    // (Implementation specific to response type)
                    continue;
                }

                DegradationStrategy::ErrorWithAlternatives { alternatives } => {
                    // Return error with alternatives
                    continue;
                }

                DegradationStrategy::QueueForRetry { queue_timeout } => {
                    // Queue for retry
                    continue;
                }
            }
        }

        // No degradation strategy succeeded
        Err(error.clone())
    }
}

/// Simple response cache for degradation
struct ResponseCache {
    cache: DashMap<String, CachedResponse>,
    max_size: usize,
    default_ttl: Duration,
}

struct CachedResponse {
    data: Vec<u8>,
    cached_at: Instant,
}

impl ResponseCache {
    fn new(max_size: usize, default_ttl: Duration) -> Self {
        Self {
            cache: DashMap::new(),
            max_size,
            default_ttl,
        }
    }

    fn get<T>(&self, key: &str, max_age: Duration) -> Option<T> {
        let entry = self.cache.get(key)?;

        if entry.cached_at.elapsed() > max_age {
            return None;
        }

        // Deserialize cached response
        // (Implementation would use actual serialization)
        None
    }

    fn set(&self, key: String, data: Vec<u8>) {
        if self.cache.len() >= self.max_size {
            // Evict oldest entry
            // (Simple implementation, could use LRU)
            if let Some(oldest) = self.cache.iter().next() {
                self.cache.remove(oldest.key());
            }
        }

        self.cache.insert(key, CachedResponse {
            data,
            cached_at: Instant::now(),
        });
    }
}

```

---

## 8. Integration with Health Monitoring

```rust
// ============================================================================
// HEALTH MONITORING INTEGRATION
// ============================================================================

/// Health check result from resilience layer
#[derive(Debug, Clone)]
struct ResilienceHealthCheck {
    timestamp: Instant,
    overall_status: HealthState,
    circuit_breakers: HashMap<String, CircuitBreakerHealth>,
    bulkheads: HashMap<String, BulkheadHealth>,
    retry_budget: RetryBudgetHealth,
    recommendations: Vec<HealthRecommendation>,
}

#[derive(Debug, Clone)]
struct CircuitBreakerHealth {
    provider_id: String,
    state: CircuitState,
    failure_rate: f64,
    consecutive_failures: u32,
    last_state_change: Option<Duration>,
    recommendation: Option<String>,
}

#[derive(Debug, Clone)]
struct BulkheadHealth {
    name: String,
    utilization: f64,
    queue_depth: usize,
    avg_wait_time: Duration,
    recommendation: Option<String>,
}

#[derive(Debug, Clone)]
struct RetryBudgetHealth {
    available: u32,
    max_budget: u32,
    utilization: f64,
    refill_rate: f64,
    recommendation: Option<String>,
}

#[derive(Debug, Clone)]
struct HealthRecommendation {
    severity: RecommendationSeverity,
    component: String,
    message: String,
    action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RecommendationSeverity {
    Info,
    Warning,
    Critical,
}

impl ResilienceCoordinator {
    /// Perform comprehensive health check
    fn health_check(&self) -> ResilienceHealthCheck {
        let timestamp = Instant::now();
        let mut recommendations = Vec::new();

        // Check circuit breakers
        let cb_states = self.circuit_breakers.get_all_states();
        let circuit_breakers = cb_states
            .into_iter()
            .map(|(provider_id, metrics)| {
                let mut recommendation = None;

                if metrics.state == CircuitState::Open {
                    recommendations.push(HealthRecommendation {
                        severity: RecommendationSeverity::Critical,
                        component: format!("circuit_breaker.{}", provider_id),
                        message: format!("Circuit breaker OPEN for provider {}", provider_id),
                        action: "Investigate provider health and consider manual reset".to_string(),
                    });
                    recommendation = Some("Circuit is open, requests are being rejected".to_string());
                } else if metrics.window_failure_rate > 0.3 {
                    recommendations.push(HealthRecommendation {
                        severity: RecommendationSeverity::Warning,
                        component: format!("circuit_breaker.{}", provider_id),
                        message: format!(
                            "High failure rate ({:.1}%) for provider {}",
                            metrics.window_failure_rate * 100.0,
                            provider_id
                        ),
                        action: "Monitor closely, may open soon".to_string(),
                    });
                    recommendation = Some("High failure rate detected".to_string());
                }

                (
                    provider_id.clone(),
                    CircuitBreakerHealth {
                        provider_id,
                        state: metrics.state,
                        failure_rate: metrics.window_failure_rate,
                        consecutive_failures: metrics.consecutive_failures,
                        last_state_change: None,
                        recommendation,
                    },
                )
            })
            .collect();

        // Check bulkheads
        let bulkhead_metrics = self.bulkheads.get_all_metrics();
        let bulkheads = bulkhead_metrics
            .into_iter()
            .map(|(name, metrics)| {
                let mut recommendation = None;

                if metrics.current_utilization > 0.9 {
                    recommendations.push(HealthRecommendation {
                        severity: RecommendationSeverity::Warning,
                        component: format!("bulkhead.{}", name),
                        message: format!(
                            "High bulkhead utilization ({:.1}%) for {}",
                            metrics.current_utilization * 100.0,
                            name
                        ),
                        action: "Consider increasing capacity or implementing backpressure".to_string(),
                    });
                    recommendation = Some("High utilization, may reject requests".to_string());
                }

                (
                    name.clone(),
                    BulkheadHealth {
                        name,
                        utilization: metrics.current_utilization,
                        queue_depth: 0, // Would be tracked separately
                        avg_wait_time: metrics.avg_wait_time,
                        recommendation,
                    },
                )
            })
            .collect();

        // Check retry budget
        let available = self.retry_budget.available();
        let max_budget = 100; // Would be from config
        let utilization = 1.0 - (available as f64 / max_budget as f64);

        let mut retry_recommendation = None;
        if available < 10 {
            recommendations.push(HealthRecommendation {
                severity: RecommendationSeverity::Warning,
                component: "retry_budget".to_string(),
                message: format!("Retry budget low: {} permits remaining", available),
                action: "System is under stress, retries may be throttled".to_string(),
            });
            retry_recommendation = Some("Retry budget depleted".to_string());
        }

        let retry_budget_health = RetryBudgetHealth {
            available,
            max_budget,
            utilization,
            refill_rate: 10.0, // Would be from config
            recommendation: retry_recommendation,
        };

        // Determine overall status
        let overall_status = if recommendations.iter().any(|r| r.severity == RecommendationSeverity::Critical) {
            HealthState::Critical
        } else if recommendations.iter().any(|r| r.severity == RecommendationSeverity::Warning) {
            HealthState::Warning
        } else {
            HealthState::Healthy
        };

        ResilienceHealthCheck {
            timestamp,
            overall_status,
            circuit_breakers,
            bulkheads,
            retry_budget: retry_budget_health,
            recommendations,
        }
    }
}

```

---

## 9. Usage Example

```rust
// ============================================================================
// EXAMPLE USAGE
// ============================================================================

/// Example of integrating resilience layer into request handler
async fn handle_llm_request(
    coordinator: &ResilienceCoordinator,
    provider_id: &str,
    request: LLMRequest,
) -> Result<LLMResponse, ResilienceError> {
    // Execute with full resilience stack
    coordinator.execute_with_resilience(
        provider_id,
        || async {
            // Actual provider call
            call_provider(provider_id, &request).await
        }
    ).await
}

/// Example with fallback to secondary provider
async fn handle_llm_request_with_fallback(
    coordinator: &ResilienceCoordinator,
    primary_provider: &str,
    fallback_provider: &str,
    request: LLMRequest,
) -> Result<LLMResponse, ResilienceError> {
    coordinator.execute_with_fallback(
        primary_provider,
        || async {
            call_provider(primary_provider, &request).await
        },
        || async {
            call_provider(fallback_provider, &request).await
        }
    ).await
}

/// Initialize resilience coordinator with custom configuration
fn initialize_resilience() -> ResilienceCoordinator {
    // Circuit breaker config
    let cb_config = CircuitBreakerConfig {
        failure_threshold: 5,
        failure_rate_threshold: 0.5,
        success_threshold: 3,
        timeout: Duration::from_secs(60),
        sampling_window: Duration::from_secs(10),
        half_open_timeout: Duration::from_secs(30),
        min_requests: 10,
        half_open_max_requests: 3,
        count_timeouts_as_failures: true,
        count_5xx_as_failures: true,
        count_429_as_failures: false,
    };

    // Retry policy
    let retry_policy = RetryPolicy {
        max_retries: 3,
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(30),
        multiplier: 2.0,
        jitter_strategy: JitterStrategy::Decorrelated,
        retryable_errors: {
            let mut errors = HashSet::new();
            errors.insert(ErrorKind::Timeout);
            errors.insert(ErrorKind::ServiceUnavailable);
            errors.insert(ErrorKind::GatewayTimeout);
            errors
        },
        backoff_strategy: BackoffStrategy::Exponential,
    };

    // Timeout manager
    let timeout_manager = TimeoutManager::new(
        Duration::from_secs(5),   // connect
        Duration::from_secs(30),  // request
        Duration::from_secs(60),  // stream
        Duration::from_secs(90),  // idle
    );

    // Retry budget
    let retry_budget = Arc::new(AdaptiveRetryBudget::new(
        100,   // max budget
        10.0,  // initial refill rate (permits/sec)
        1.0,   // min refill rate
        50.0,  // max refill rate
    ));

    ResilienceCoordinator::new(
        cb_config,
        retry_policy,
        timeout_manager,
        retry_budget,
    )
}

// Placeholder types
struct LLMRequest;
struct LLMResponse;

async fn call_provider(provider_id: &str, request: &LLMRequest) -> Result<LLMResponse, GatewayError> {
    // Implementation
    unimplemented!()
}
```

---

## Summary

This comprehensive pseudocode provides:

1. **Circuit Breaker**: State machine with atomic transitions, sliding window metrics, and configurable thresholds
2. **Retry Policy**: Exponential backoff with multiple jitter strategies and error classification
3. **Retry Budget**: Token bucket with adaptive refill rates based on success/failure patterns
4. **Bulkhead**: Semaphore-based concurrency limiting with metrics and timeout handling
5. **Timeout Management**: Hierarchical timeouts with nested contexts
6. **Resilience Coordinator**: Unified layer combining all resilience patterns
7. **Graceful Degradation**: Multiple fallback strategies including caching and provider fallback
8. **Health Monitoring**: Comprehensive health checks with actionable recommendations

Key Features:
- Thread-safe using atomic operations and lock-free data structures where possible
- Metrics collection for observability
- Configurable per-provider policies
- Integration-ready with health monitoring systems
- Production-grade error handling and recovery
