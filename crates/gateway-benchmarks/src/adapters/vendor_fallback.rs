//! Vendor fallback resolution benchmark adapter.
//!
//! Measures the latency of vendor fallback resolution when
//! primary providers are unavailable.

use super::BenchTarget;
use crate::BenchmarkResult;
use anyhow::Result;
use async_trait::async_trait;
use std::time::Instant;

/// Benchmark for vendor fallback resolution.
///
/// This benchmark measures:
/// - Fallback chain traversal latency
/// - Provider health check integration
/// - Failover decision time
pub struct VendorFallbackBenchmark {
    iterations: u32,
}

impl VendorFallbackBenchmark {
    /// Create a new vendor fallback benchmark.
    pub fn new() -> Self {
        Self { iterations: 5000 }
    }

    /// Create with custom iteration count.
    pub fn with_iterations(iterations: u32) -> Self {
        Self { iterations }
    }
}

impl Default for VendorFallbackBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BenchTarget for VendorFallbackBenchmark {
    fn id(&self) -> &str {
        "vendor_fallback"
    }

    fn description(&self) -> &str {
        "Measures vendor fallback resolution latency when primary providers fail"
    }

    fn iterations(&self) -> u32 {
        self.iterations
    }

    async fn run(&self) -> Result<BenchmarkResult> {
        let mut latencies = Vec::with_capacity(self.iterations as usize);
        let mut fallback_counts = Vec::with_capacity(self.iterations as usize);

        // Warmup
        for _ in 0..self.warmup_iterations() {
            simulate_fallback_resolution().await;
        }

        // Benchmark
        for _ in 0..self.iterations {
            let start = Instant::now();
            let fallbacks = simulate_fallback_resolution().await;
            latencies.push(start.elapsed().as_nanos() as f64 / 1_000_000.0);
            fallback_counts.push(fallbacks);
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

        let avg_fallbacks: f64 =
            fallback_counts.iter().sum::<u32>() as f64 / fallback_counts.len() as f64;

        let throughput_rps = if avg_ms > 0.0 {
            1000.0 / avg_ms
        } else {
            0.0
        };

        Ok(BenchmarkResult::new(
            self.id(),
            serde_json::json!({
                "iterations": self.iterations,
                "latency_ms": avg_ms,
                "min_ms": min_ms,
                "max_ms": max_ms,
                "p50_ms": latencies.get(p50_idx).copied().unwrap_or(0.0),
                "p90_ms": latencies.get(p90_idx).copied().unwrap_or(0.0),
                "p95_ms": latencies.get(p95_idx).copied().unwrap_or(0.0),
                "p99_ms": latencies.get(p99_idx).copied().unwrap_or(0.0),
                "throughput_rps": throughput_rps,
                "avg_fallbacks": avg_fallbacks,
                "description": self.description()
            }),
        ))
    }
}

/// Simulate fallback resolution returning number of fallbacks tried.
async fn simulate_fallback_resolution() -> u32 {
    // Simulate fallback chain: primary -> secondary -> tertiary
    let providers = [
        ("openai", false),     // Primary: unavailable
        ("anthropic", false),  // Secondary: unavailable
        ("azure", true),       // Tertiary: available
        ("google", true),      // Quaternary: available
    ];

    let mut fallbacks = 0;

    for (provider, available) in providers.iter() {
        // Simulate health check
        tokio::task::yield_now().await;

        if *available {
            // Provider available, use it
            let _ = provider;
            break;
        }
        fallbacks += 1;
    }

    fallbacks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vendor_fallback_benchmark() {
        let benchmark = VendorFallbackBenchmark::with_iterations(100);
        let result = benchmark.run().await.expect("Benchmark should succeed");

        assert_eq!(result.target_id, "vendor_fallback");
        assert!(result.latency_ms().is_some());
    }
}
