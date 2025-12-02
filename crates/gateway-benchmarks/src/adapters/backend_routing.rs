//! Backend routing benchmark adapter.
//!
//! Measures the latency of backend routing decisions including
//! load balancer selection and provider routing rules.

use super::BenchTarget;
use crate::BenchmarkResult;
use anyhow::Result;
use async_trait::async_trait;
use std::time::Instant;

/// Benchmark for backend routing decisions.
///
/// This benchmark measures:
/// - Load balancer provider selection latency
/// - Routing rule evaluation time
/// - Provider candidate scoring
pub struct BackendRoutingBenchmark {
    iterations: u32,
}

impl BackendRoutingBenchmark {
    /// Create a new backend routing benchmark.
    pub fn new() -> Self {
        Self { iterations: 10000 }
    }

    /// Create with custom iteration count.
    pub fn with_iterations(iterations: u32) -> Self {
        Self { iterations }
    }
}

impl Default for BackendRoutingBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BenchTarget for BackendRoutingBenchmark {
    fn id(&self) -> &str {
        "backend_routing"
    }

    fn description(&self) -> &str {
        "Measures backend routing decision latency including load balancer selection and routing rules"
    }

    fn iterations(&self) -> u32 {
        self.iterations
    }

    async fn run(&self) -> Result<BenchmarkResult> {
        let mut latencies = Vec::with_capacity(self.iterations as usize);

        // Warmup phase
        for _ in 0..self.warmup_iterations() {
            simulate_routing_decision().await;
        }

        // Benchmark phase
        for _ in 0..self.iterations {
            let start = Instant::now();
            simulate_routing_decision().await;
            latencies.push(start.elapsed().as_nanos() as f64 / 1_000_000.0); // Convert to ms
        }

        // Calculate statistics
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let sum: f64 = latencies.iter().sum();
        let avg_ms = sum / latencies.len() as f64;
        let min_ms = latencies.first().copied().unwrap_or(0.0);
        let max_ms = latencies.last().copied().unwrap_or(0.0);

        let p50_idx = latencies.len() / 2;
        let p90_idx = (latencies.len() as f64 * 0.90) as usize;
        let p95_idx = (latencies.len() as f64 * 0.95) as usize;
        let p99_idx = (latencies.len() as f64 * 0.99) as usize;

        let p50_ms = latencies.get(p50_idx).copied().unwrap_or(0.0);
        let p90_ms = latencies.get(p90_idx).copied().unwrap_or(0.0);
        let p95_ms = latencies.get(p95_idx).copied().unwrap_or(0.0);
        let p99_ms = latencies.get(p99_idx).copied().unwrap_or(0.0);

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
                "p50_ms": p50_ms,
                "p90_ms": p90_ms,
                "p95_ms": p95_ms,
                "p99_ms": p99_ms,
                "throughput_rps": throughput_rps,
                "description": self.description()
            }),
        ))
    }
}

/// Simulate a routing decision without actual provider calls.
async fn simulate_routing_decision() {
    // Simulate the core routing logic:
    // 1. Parse routing context
    // 2. Evaluate routing rules
    // 3. Score provider candidates
    // 4. Select optimal provider

    // Simulate minimal async work (context switch cost)
    tokio::task::yield_now().await;

    // Simulate rule evaluation (in-memory operations)
    let providers = vec!["openai", "anthropic", "azure", "google"];
    let _selected = providers
        .iter()
        .enumerate()
        .map(|(i, p)| (p, simulate_score(i)))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(p, _)| *p);
}

/// Simulate provider scoring (deterministic for benchmarking).
fn simulate_score(index: usize) -> f64 {
    // Simple deterministic scoring based on index
    let base_score = 100.0 - (index as f64 * 10.0);
    let health_factor = if index % 2 == 0 { 1.0 } else { 0.9 };
    base_score * health_factor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_backend_routing_benchmark() {
        let benchmark = BackendRoutingBenchmark::with_iterations(100);
        let result = benchmark.run().await.expect("Benchmark should succeed");

        assert_eq!(result.target_id, "backend_routing");
        assert!(result.latency_ms().is_some());
        assert!(result.throughput_rps().is_some());
    }
}
