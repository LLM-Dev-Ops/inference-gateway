//! Benchmark adapters implementing the canonical BenchTarget trait.
//!
//! This module provides the adapter system for exposing gateway operations
//! as benchmark targets without modifying existing logic.

mod backend_routing;
mod circuit_breaker;
mod concurrency;
mod health_check;
mod rate_limiting;
mod request_transform;
mod streaming_throughput;
mod vendor_fallback;

use crate::BenchmarkResult;
use anyhow::Result;
use async_trait::async_trait;

pub use backend_routing::BackendRoutingBenchmark;
pub use circuit_breaker::CircuitBreakerBenchmark;
pub use concurrency::ConcurrencyBenchmark;
pub use health_check::HealthCheckBenchmark;
pub use rate_limiting::RateLimitingBenchmark;
pub use request_transform::RequestTransformBenchmark;
pub use streaming_throughput::StreamingThroughputBenchmark;
pub use vendor_fallback::VendorFallbackBenchmark;

/// Canonical benchmark target trait.
///
/// All benchmark targets must implement this trait to be registered
/// in the benchmark system. The trait provides:
/// - `id()`: A unique identifier for the benchmark
/// - `run()`: Execute the benchmark and return results
///
/// # Example
///
/// ```rust,ignore
/// use gateway_benchmarks::{BenchTarget, BenchmarkResult};
/// use async_trait::async_trait;
///
/// struct MyBenchmark;
///
/// #[async_trait]
/// impl BenchTarget for MyBenchmark {
///     fn id(&self) -> &str {
///         "my_benchmark"
///     }
///
///     async fn run(&self) -> anyhow::Result<BenchmarkResult> {
///         // Run benchmark logic
///         Ok(BenchmarkResult::new("my_benchmark", serde_json::json!({
///             "latency_ms": 10.0
///         })))
///     }
/// }
/// ```
#[async_trait]
pub trait BenchTarget: Send + Sync {
    /// Returns the unique identifier for this benchmark target.
    fn id(&self) -> &str;

    /// Execute the benchmark and return the result.
    ///
    /// This method should:
    /// 1. Set up any required test fixtures
    /// 2. Run the benchmark iterations
    /// 3. Collect metrics (latency, throughput, etc.)
    /// 4. Return a `BenchmarkResult` with the collected metrics
    async fn run(&self) -> Result<BenchmarkResult>;

    /// Returns a description of what this benchmark measures.
    fn description(&self) -> &str {
        "No description provided"
    }

    /// Returns the number of iterations to run (default: 1000).
    fn iterations(&self) -> u32 {
        1000
    }

    /// Returns the warmup iterations (default: 100).
    fn warmup_iterations(&self) -> u32 {
        100
    }
}

/// Returns all registered benchmark targets.
///
/// This is the canonical registry function that returns a vector of all
/// available benchmark targets. Each target is boxed as a trait object
/// for polymorphic handling.
///
/// # Returns
///
/// A `Vec<Box<dyn BenchTarget>>` containing all registered benchmarks.
pub fn all_targets() -> Vec<Box<dyn BenchTarget>> {
    vec![
        Box::new(BackendRoutingBenchmark::new()),
        Box::new(VendorFallbackBenchmark::new()),
        Box::new(StreamingThroughputBenchmark::new()),
        Box::new(ConcurrencyBenchmark::new()),
        Box::new(RequestTransformBenchmark::new()),
        Box::new(HealthCheckBenchmark::new()),
        Box::new(CircuitBreakerBenchmark::new()),
        Box::new(RateLimitingBenchmark::new()),
    ]
}

/// Get a specific benchmark target by ID.
pub fn get_target(id: &str) -> Option<Box<dyn BenchTarget>> {
    all_targets().into_iter().find(|t| t.id() == id)
}

/// List all available benchmark target IDs.
pub fn list_target_ids() -> Vec<&'static str> {
    vec![
        "backend_routing",
        "vendor_fallback",
        "streaming_throughput",
        "concurrency_handling",
        "request_transform",
        "health_check_latency",
        "circuit_breaker",
        "rate_limiting",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_targets_not_empty() {
        let targets = all_targets();
        assert!(!targets.is_empty());
    }

    #[test]
    fn test_all_targets_unique_ids() {
        let targets = all_targets();
        let mut ids: Vec<&str> = targets.iter().map(|t| t.id()).collect();
        let original_len = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), original_len, "Duplicate target IDs found");
    }

    #[test]
    fn test_get_target() {
        let target = get_target("backend_routing");
        assert!(target.is_some());
        assert_eq!(target.unwrap().id(), "backend_routing");
    }

    #[test]
    fn test_get_nonexistent_target() {
        let target = get_target("nonexistent");
        assert!(target.is_none());
    }
}
