//! Circuit breaker benchmark adapter.
//!
//! Measures the performance of circuit breaker state transitions
//! and request checking overhead.

use super::BenchTarget;
use crate::BenchmarkResult;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::time::Instant;

/// Benchmark for circuit breaker operations.
///
/// This benchmark measures:
/// - Circuit state check latency
/// - State transition overhead
/// - Failure counting performance
pub struct CircuitBreakerBenchmark {
    iterations: u32,
}

impl CircuitBreakerBenchmark {
    /// Create a new circuit breaker benchmark.
    pub fn new() -> Self {
        Self { iterations: 50000 }
    }

    /// Create with custom iteration count.
    pub fn with_iterations(iterations: u32) -> Self {
        Self { iterations }
    }
}

impl Default for CircuitBreakerBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BenchTarget for CircuitBreakerBenchmark {
    fn id(&self) -> &str {
        "circuit_breaker"
    }

    fn description(&self) -> &str {
        "Measures circuit breaker state check and transition performance"
    }

    fn iterations(&self) -> u32 {
        self.iterations
    }

    async fn run(&self) -> Result<BenchmarkResult> {
        let mut latencies = Vec::with_capacity(self.iterations as usize);

        // Create simulated circuit breaker state
        let circuit = SimulatedCircuitBreaker::new();

        // Warmup
        for i in 0..self.warmup_iterations() {
            circuit.check_and_update(i % 10 == 0);
        }

        // Reset circuit
        circuit.reset();

        // Benchmark
        for i in 0..self.iterations {
            let start = Instant::now();

            // Simulate circuit breaker check with occasional failures
            let is_failure = i % 100 == 0;
            circuit.check_and_update(is_failure);

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

        // Get circuit stats
        let state_transitions = circuit.state_transitions.load(Ordering::Relaxed);
        let total_failures = circuit.failure_count.load(Ordering::Relaxed);

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
                "state_transitions": state_transitions,
                "total_failures": total_failures,
                "description": self.description()
            }),
        ))
    }
}

/// Simulated circuit breaker for benchmarking.
struct SimulatedCircuitBreaker {
    state: AtomicU8,
    failure_count: AtomicU32,
    success_count: AtomicU32,
    state_transitions: AtomicU32,
}

impl SimulatedCircuitBreaker {
    const CLOSED: u8 = 0;
    const OPEN: u8 = 1;
    const HALF_OPEN: u8 = 2;
    const FAILURE_THRESHOLD: u32 = 5;
    const SUCCESS_THRESHOLD: u32 = 3;

    fn new() -> Self {
        Self {
            state: AtomicU8::new(Self::CLOSED),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            state_transitions: AtomicU32::new(0),
        }
    }

    fn reset(&self) {
        self.state.store(Self::CLOSED, Ordering::SeqCst);
        self.failure_count.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
        self.state_transitions.store(0, Ordering::SeqCst);
    }

    fn check_and_update(&self, is_failure: bool) -> bool {
        let current_state = self.state.load(Ordering::Acquire);

        match current_state {
            Self::OPEN => {
                // Circuit is open, reject request
                false
            }
            Self::HALF_OPEN => {
                if is_failure {
                    // Failed probe, re-open circuit
                    self.state.store(Self::OPEN, Ordering::Release);
                    self.state_transitions.fetch_add(1, Ordering::Relaxed);
                    false
                } else {
                    let successes = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                    if successes >= Self::SUCCESS_THRESHOLD {
                        // Enough successes, close circuit
                        self.state.store(Self::CLOSED, Ordering::Release);
                        self.success_count.store(0, Ordering::Relaxed);
                        self.failure_count.store(0, Ordering::Relaxed);
                        self.state_transitions.fetch_add(1, Ordering::Relaxed);
                    }
                    true
                }
            }
            _ => {
                // CLOSED state
                if is_failure {
                    let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                    if failures >= Self::FAILURE_THRESHOLD {
                        // Too many failures, open circuit
                        self.state.store(Self::OPEN, Ordering::Release);
                        self.state_transitions.fetch_add(1, Ordering::Relaxed);
                    }
                }
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_benchmark() {
        let benchmark = CircuitBreakerBenchmark::with_iterations(100);
        let result = benchmark.run().await.expect("Benchmark should succeed");

        assert_eq!(result.target_id, "circuit_breaker");
        assert!(result.latency_ms().is_some());
    }

    #[test]
    fn test_simulated_circuit_breaker() {
        let circuit = SimulatedCircuitBreaker::new();

        // Should start closed and allow requests
        assert!(circuit.check_and_update(false));

        // Trigger failures to open circuit
        for _ in 0..5 {
            circuit.check_and_update(true);
        }

        // Circuit should be open now
        assert!(!circuit.check_and_update(false));
    }
}
