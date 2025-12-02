//! Concurrency handling benchmark adapter.
//!
//! Measures the performance of concurrent request handling
//! including connection pooling and request multiplexing.

use super::BenchTarget;
use crate::BenchmarkResult;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Benchmark for concurrent request handling.
///
/// This benchmark measures:
/// - Concurrent request processing throughput
/// - Lock contention overhead
/// - Connection pool efficiency
pub struct ConcurrencyBenchmark {
    iterations: u32,
    concurrency: u32,
}

impl ConcurrencyBenchmark {
    /// Create a new concurrency benchmark.
    pub fn new() -> Self {
        Self {
            iterations: 1000,
            concurrency: 100,
        }
    }

    /// Create with custom parameters.
    pub fn with_params(iterations: u32, concurrency: u32) -> Self {
        Self {
            iterations,
            concurrency,
        }
    }
}

impl Default for ConcurrencyBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BenchTarget for ConcurrencyBenchmark {
    fn id(&self) -> &str {
        "concurrency_handling"
    }

    fn description(&self) -> &str {
        "Measures concurrent request handling performance and lock contention"
    }

    fn iterations(&self) -> u32 {
        self.iterations
    }

    async fn run(&self) -> Result<BenchmarkResult> {
        // Shared state for measuring contention
        let counter = Arc::new(AtomicU64::new(0));
        let completed = Arc::new(AtomicU64::new(0));

        // Warmup with reduced concurrency
        let warmup_tasks: Vec<_> = (0..self.warmup_iterations())
            .map(|_| {
                let counter = Arc::clone(&counter);
                tokio::spawn(async move {
                    simulate_concurrent_request(&counter).await;
                })
            })
            .collect();

        for task in warmup_tasks {
            let _ = task.await;
        }

        // Reset counters
        counter.store(0, Ordering::SeqCst);
        completed.store(0, Ordering::SeqCst);

        // Benchmark phase with full concurrency
        let start = Instant::now();
        let mut latencies = Vec::with_capacity(self.iterations as usize);

        // Process in batches to control concurrency level
        let batches = (self.iterations + self.concurrency - 1) / self.concurrency;

        for batch in 0..batches {
            let batch_size = std::cmp::min(
                self.concurrency,
                self.iterations - batch * self.concurrency,
            );

            let batch_start = Instant::now();

            let tasks: Vec<_> = (0..batch_size)
                .map(|_| {
                    let counter = Arc::clone(&counter);
                    let completed = Arc::clone(&completed);
                    tokio::spawn(async move {
                        let task_start = Instant::now();
                        simulate_concurrent_request(&counter).await;
                        completed.fetch_add(1, Ordering::Relaxed);
                        task_start.elapsed().as_nanos() as f64 / 1_000_000.0
                    })
                })
                .collect();

            for task in tasks {
                if let Ok(latency) = task.await {
                    latencies.push(latency);
                }
            }

            let _batch_duration = batch_start.elapsed();
        }

        let total_duration = start.elapsed();
        let total_completed = completed.load(Ordering::SeqCst);

        // Statistics
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let sum: f64 = latencies.iter().sum();
        let avg_ms = if !latencies.is_empty() {
            sum / latencies.len() as f64
        } else {
            0.0
        };
        let min_ms = latencies.first().copied().unwrap_or(0.0);
        let max_ms = latencies.last().copied().unwrap_or(0.0);

        let p50_idx = latencies.len() / 2;
        let p90_idx = (latencies.len() as f64 * 0.90) as usize;
        let p95_idx = (latencies.len() as f64 * 0.95) as usize;
        let p99_idx = (latencies.len() as f64 * 0.99) as usize;

        let throughput_rps = total_completed as f64 / total_duration.as_secs_f64();

        Ok(BenchmarkResult::new(
            self.id(),
            serde_json::json!({
                "iterations": self.iterations,
                "concurrency": self.concurrency,
                "latency_ms": avg_ms,
                "min_ms": min_ms,
                "max_ms": max_ms,
                "p50_ms": latencies.get(p50_idx).copied().unwrap_or(0.0),
                "p90_ms": latencies.get(p90_idx).copied().unwrap_or(0.0),
                "p95_ms": latencies.get(p95_idx).copied().unwrap_or(0.0),
                "p99_ms": latencies.get(p99_idx).copied().unwrap_or(0.0),
                "throughput_rps": throughput_rps,
                "total_completed": total_completed,
                "total_duration_ms": total_duration.as_millis(),
                "description": self.description()
            }),
        ))
    }
}

/// Simulate a concurrent request with shared state access.
async fn simulate_concurrent_request(counter: &AtomicU64) {
    // Simulate request processing with atomic counter access
    counter.fetch_add(1, Ordering::Relaxed);

    // Simulate some async work
    tokio::task::yield_now().await;

    // Simulate response handling
    let _count = counter.load(Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrency_benchmark() {
        let benchmark = ConcurrencyBenchmark::with_params(100, 10);
        let result = benchmark.run().await.expect("Benchmark should succeed");

        assert_eq!(result.target_id, "concurrency_handling");
        assert!(result.throughput_rps().is_some());
    }
}
