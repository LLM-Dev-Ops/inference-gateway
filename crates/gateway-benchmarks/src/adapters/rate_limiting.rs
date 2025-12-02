//! Rate limiting benchmark adapter.
//!
//! Measures the performance of rate limiting operations
//! including token bucket updates and limit checks.

use super::BenchTarget;
use crate::BenchmarkResult;
use anyhow::Result;
use async_trait::async_trait;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::time::Instant;

/// Benchmark for rate limiting operations.
///
/// This benchmark measures:
/// - Rate limit check latency
/// - Token bucket update performance
/// - Multi-key rate limiting overhead
pub struct RateLimitingBenchmark {
    iterations: u32,
    keys_count: u32,
}

impl RateLimitingBenchmark {
    /// Create a new rate limiting benchmark.
    pub fn new() -> Self {
        Self {
            iterations: 50000,
            keys_count: 100,
        }
    }

    /// Create with custom parameters.
    pub fn with_params(iterations: u32, keys_count: u32) -> Self {
        Self {
            iterations,
            keys_count,
        }
    }
}

impl Default for RateLimitingBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BenchTarget for RateLimitingBenchmark {
    fn id(&self) -> &str {
        "rate_limiting"
    }

    fn description(&self) -> &str {
        "Measures rate limiting check and token bucket update performance"
    }

    fn iterations(&self) -> u32 {
        self.iterations
    }

    async fn run(&self) -> Result<BenchmarkResult> {
        let mut latencies = Vec::with_capacity(self.iterations as usize);

        // Create simulated rate limiter
        let limiter = SimulatedRateLimiter::new(1000); // 1000 requests per window

        // Pre-populate with keys
        let keys: Vec<String> = (0..self.keys_count)
            .map(|i| format!("tenant_{}", i))
            .collect();

        // Warmup
        for i in 0..self.warmup_iterations() {
            let key = &keys[i as usize % keys.len()];
            limiter.check_rate_limit(key, 1);
        }

        // Reset limiter
        limiter.reset();

        // Benchmark
        let mut allowed = 0u64;
        let mut denied = 0u64;

        for i in 0..self.iterations {
            let key = &keys[i as usize % keys.len()];

            let start = Instant::now();
            let result = limiter.check_rate_limit(key, 1);
            latencies.push(start.elapsed().as_nanos() as f64 / 1_000_000.0);

            if result {
                allowed += 1;
            } else {
                denied += 1;
            }
        }

        // Statistics
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let sum: f64 = latencies.iter().sum();
        let avg_ms = sum / latencies.len() as f64;
        let min_ms = latencies.first().copied().unwrap_or(0.0);
        let max_ms = latencies.last().copied().unwrap_or(0.0);

        let p50_idx = latencies.len() / 2;
        let p90_idx = (latencies.len() as f64 * 0.90) as usize;
        let p95_idx = (latencies.len() as f64 * 0.95) as usize;
        let p99_idx = (latencies.len() as f64 * 0.99) as usize;

        let throughput_rps = if avg_ms > 0.0 {
            1000.0 / avg_ms
        } else {
            0.0
        };

        Ok(BenchmarkResult::new(
            self.id(),
            serde_json::json!({
                "iterations": self.iterations,
                "keys_count": self.keys_count,
                "latency_ms": avg_ms,
                "min_ms": min_ms,
                "max_ms": max_ms,
                "p50_ms": latencies.get(p50_idx).copied().unwrap_or(0.0),
                "p90_ms": latencies.get(p90_idx).copied().unwrap_or(0.0),
                "p95_ms": latencies.get(p95_idx).copied().unwrap_or(0.0),
                "p99_ms": latencies.get(p99_idx).copied().unwrap_or(0.0),
                "throughput_rps": throughput_rps,
                "allowed_requests": allowed,
                "denied_requests": denied,
                "description": self.description()
            }),
        ))
    }
}

/// Simulated token bucket rate limiter for benchmarking.
struct SimulatedRateLimiter {
    buckets: Mutex<HashMap<String, TokenBucket>>,
    limit: u32,
}

struct TokenBucket {
    tokens: f64,
    last_update: Instant,
}

impl SimulatedRateLimiter {
    fn new(limit: u32) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            limit,
        }
    }

    fn reset(&self) {
        let mut buckets = self.buckets.lock();
        buckets.clear();
    }

    fn check_rate_limit(&self, key: &str, tokens_needed: u32) -> bool {
        let mut buckets = self.buckets.lock();
        let now = Instant::now();

        let bucket = buckets.entry(key.to_string()).or_insert_with(|| TokenBucket {
            tokens: self.limit as f64,
            last_update: now,
        });

        // Refill tokens based on elapsed time
        let elapsed = now.duration_since(bucket.last_update).as_secs_f64();
        let refill_rate = self.limit as f64; // tokens per second
        bucket.tokens = (bucket.tokens + elapsed * refill_rate).min(self.limit as f64 * 1.5);
        bucket.last_update = now;

        // Check if we have enough tokens
        if bucket.tokens >= tokens_needed as f64 {
            bucket.tokens -= tokens_needed as f64;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiting_benchmark() {
        let benchmark = RateLimitingBenchmark::with_params(100, 10);
        let result = benchmark.run().await.expect("Benchmark should succeed");

        assert_eq!(result.target_id, "rate_limiting");
        assert!(result.latency_ms().is_some());
    }

    #[test]
    fn test_simulated_rate_limiter() {
        let limiter = SimulatedRateLimiter::new(10);

        // Should allow initial requests
        assert!(limiter.check_rate_limit("key1", 1));

        // Different keys should have separate limits
        assert!(limiter.check_rate_limit("key2", 1));
    }
}
