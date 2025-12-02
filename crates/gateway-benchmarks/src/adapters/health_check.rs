//! Health check latency benchmark adapter.
//!
//! Measures the latency of provider health check operations
//! including endpoint probing and status aggregation.

use super::BenchTarget;
use crate::BenchmarkResult;
use anyhow::Result;
use async_trait::async_trait;
use std::time::Instant;

/// Benchmark for health check latency.
///
/// This benchmark measures:
/// - Provider health endpoint probe latency
/// - Health status aggregation time
/// - Health cache lookup performance
pub struct HealthCheckBenchmark {
    iterations: u32,
    providers_count: u32,
}

impl HealthCheckBenchmark {
    /// Create a new health check benchmark.
    pub fn new() -> Self {
        Self {
            iterations: 5000,
            providers_count: 5,
        }
    }

    /// Create with custom parameters.
    pub fn with_params(iterations: u32, providers_count: u32) -> Self {
        Self {
            iterations,
            providers_count,
        }
    }
}

impl Default for HealthCheckBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BenchTarget for HealthCheckBenchmark {
    fn id(&self) -> &str {
        "health_check_latency"
    }

    fn description(&self) -> &str {
        "Measures provider health check latency and status aggregation"
    }

    fn iterations(&self) -> u32 {
        self.iterations
    }

    async fn run(&self) -> Result<BenchmarkResult> {
        let mut latencies = Vec::with_capacity(self.iterations as usize);

        // Warmup
        for _ in 0..self.warmup_iterations() {
            simulate_health_check(self.providers_count).await;
        }

        // Benchmark
        for _ in 0..self.iterations {
            let start = Instant::now();
            let _status = simulate_health_check(self.providers_count).await;
            latencies.push(start.elapsed().as_nanos() as f64 / 1_000_000.0);
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
                "providers_count": self.providers_count,
                "latency_ms": avg_ms,
                "min_ms": min_ms,
                "max_ms": max_ms,
                "p50_ms": latencies.get(p50_idx).copied().unwrap_or(0.0),
                "p90_ms": latencies.get(p90_idx).copied().unwrap_or(0.0),
                "p95_ms": latencies.get(p95_idx).copied().unwrap_or(0.0),
                "p99_ms": latencies.get(p99_idx).copied().unwrap_or(0.0),
                "throughput_rps": throughput_rps,
                "description": self.description()
            }),
        ))
    }
}

/// Simulated health status.
#[derive(Debug, Clone)]
struct HealthStatus {
    healthy: bool,
    latency_ms: f64,
    provider: String,
}

/// Simulate health check for multiple providers.
async fn simulate_health_check(providers_count: u32) -> Vec<HealthStatus> {
    let providers = vec!["openai", "anthropic", "azure", "google", "bedrock"];
    let mut results = Vec::with_capacity(providers_count as usize);

    for i in 0..providers_count {
        let provider = providers.get(i as usize % providers.len()).unwrap_or(&"unknown");

        // Simulate async health check
        tokio::task::yield_now().await;

        // Simulate health status lookup from cache
        results.push(HealthStatus {
            healthy: i % 3 != 0, // Simulate some unhealthy providers
            latency_ms: (i as f64 * 0.1) + 0.5,
            provider: (*provider).to_string(),
        });
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check_benchmark() {
        let benchmark = HealthCheckBenchmark::with_params(100, 3);
        let result = benchmark.run().await.expect("Benchmark should succeed");

        assert_eq!(result.target_id, "health_check_latency");
        assert!(result.latency_ms().is_some());
    }
}
