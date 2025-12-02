//! Canonical BenchmarkResult struct for standardized benchmark output.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Canonical benchmark result structure.
///
/// This struct contains exactly the fields required by the canonical benchmark
/// interface used across all benchmark-target repositories:
/// - `target_id`: Unique identifier for the benchmark target
/// - `metrics`: JSON value containing benchmark-specific metrics
/// - `timestamp`: UTC timestamp when the benchmark was executed
///
/// # Example
///
/// ```rust
/// use gateway_benchmarks::BenchmarkResult;
/// use chrono::Utc;
///
/// let result = BenchmarkResult {
///     target_id: "backend_routing".to_string(),
///     metrics: serde_json::json!({
///         "latency_ms": 15.5,
///         "throughput_rps": 1000
///     }),
///     timestamp: Utc::now(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Unique identifier for the benchmark target.
    pub target_id: String,

    /// JSON value containing benchmark-specific metrics.
    ///
    /// This field is flexible to accommodate different benchmark types.
    /// Common metrics include:
    /// - `latency_ms`: Operation latency in milliseconds
    /// - `throughput_rps`: Requests per second
    /// - `iterations`: Number of benchmark iterations
    /// - `min_ms`, `max_ms`, `avg_ms`: Latency statistics
    /// - `p50_ms`, `p90_ms`, `p95_ms`, `p99_ms`: Latency percentiles
    pub metrics: serde_json::Value,

    /// UTC timestamp when the benchmark was executed.
    pub timestamp: DateTime<Utc>,
}

impl BenchmarkResult {
    /// Create a new BenchmarkResult with the current timestamp.
    pub fn new(target_id: impl Into<String>, metrics: serde_json::Value) -> Self {
        Self {
            target_id: target_id.into(),
            metrics,
            timestamp: Utc::now(),
        }
    }

    /// Check if this benchmark result indicates a failure.
    pub fn is_error(&self) -> bool {
        self.metrics.get("error").is_some() || self.metrics.get("status") == Some(&serde_json::json!("failed"))
    }

    /// Get the latency in milliseconds if available.
    pub fn latency_ms(&self) -> Option<f64> {
        self.metrics.get("latency_ms").and_then(|v| v.as_f64())
    }

    /// Get the throughput in requests per second if available.
    pub fn throughput_rps(&self) -> Option<f64> {
        self.metrics.get("throughput_rps").and_then(|v| v.as_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_result_new() {
        let result = BenchmarkResult::new(
            "test_target",
            serde_json::json!({
                "latency_ms": 10.5,
                "throughput_rps": 500
            }),
        );

        assert_eq!(result.target_id, "test_target");
        assert_eq!(result.latency_ms(), Some(10.5));
        assert_eq!(result.throughput_rps(), Some(500.0));
        assert!(!result.is_error());
    }

    #[test]
    fn test_benchmark_result_error() {
        let result = BenchmarkResult::new(
            "failed_target",
            serde_json::json!({
                "error": "Connection failed",
                "status": "failed"
            }),
        );

        assert!(result.is_error());
    }

    #[test]
    fn test_benchmark_result_serialization() {
        let result = BenchmarkResult::new("test", serde_json::json!({"value": 42}));

        let json = serde_json::to_string(&result).expect("Failed to serialize");
        let deserialized: BenchmarkResult = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(deserialized.target_id, result.target_id);
        assert_eq!(deserialized.metrics, result.metrics);
    }
}
