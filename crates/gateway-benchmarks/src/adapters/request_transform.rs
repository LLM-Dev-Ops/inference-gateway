//! Request transformation benchmark adapter.
//!
//! Measures the latency of request transformation between
//! gateway format and provider-specific formats.

use super::BenchTarget;
use crate::BenchmarkResult;
use anyhow::Result;
use async_trait::async_trait;
use std::time::Instant;

/// Benchmark for request transformation.
///
/// This benchmark measures:
/// - Request parsing latency
/// - Provider format conversion time
/// - Response transformation overhead
pub struct RequestTransformBenchmark {
    iterations: u32,
}

impl RequestTransformBenchmark {
    /// Create a new request transform benchmark.
    pub fn new() -> Self {
        Self { iterations: 10000 }
    }

    /// Create with custom iteration count.
    pub fn with_iterations(iterations: u32) -> Self {
        Self { iterations }
    }
}

impl Default for RequestTransformBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BenchTarget for RequestTransformBenchmark {
    fn id(&self) -> &str {
        "request_transform"
    }

    fn description(&self) -> &str {
        "Measures request/response transformation latency for provider format conversion"
    }

    fn iterations(&self) -> u32 {
        self.iterations
    }

    async fn run(&self) -> Result<BenchmarkResult> {
        let mut latencies = Vec::with_capacity(self.iterations as usize);

        // Sample request for transformation
        let sample_request = create_sample_request();

        // Warmup
        for _ in 0..self.warmup_iterations() {
            transform_request(&sample_request);
        }

        // Benchmark
        for _ in 0..self.iterations {
            let start = Instant::now();
            let _transformed = transform_request(&sample_request);
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

/// Create a sample request for benchmarking.
fn create_sample_request() -> serde_json::Value {
    serde_json::json!({
        "model": "gpt-4",
        "messages": [
            {
                "role": "system",
                "content": "You are a helpful assistant."
            },
            {
                "role": "user",
                "content": "Hello, how are you today? Can you help me with a programming question?"
            }
        ],
        "temperature": 0.7,
        "max_tokens": 1000,
        "stream": false
    })
}

/// Transform request to provider format (simulated).
fn transform_request(request: &serde_json::Value) -> serde_json::Value {
    // Simulate transformation logic:
    // 1. Extract common fields
    // 2. Map to provider-specific format
    // 3. Add provider-specific headers/fields

    let model = request.get("model").and_then(|v| v.as_str()).unwrap_or("gpt-4");
    let messages = request.get("messages").cloned().unwrap_or(serde_json::json!([]));
    let temperature = request.get("temperature").and_then(|v| v.as_f64()).unwrap_or(0.7);
    let max_tokens = request.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(1000);

    // Simulate OpenAI format transformation
    serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": temperature,
        "max_tokens": max_tokens,
        "n": 1,
        "presence_penalty": 0,
        "frequency_penalty": 0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_request_transform_benchmark() {
        let benchmark = RequestTransformBenchmark::with_iterations(100);
        let result = benchmark.run().await.expect("Benchmark should succeed");

        assert_eq!(result.target_id, "request_transform");
        assert!(result.latency_ms().is_some());
    }

    #[test]
    fn test_transform_request() {
        let request = create_sample_request();
        let transformed = transform_request(&request);

        assert_eq!(transformed.get("model").and_then(|v| v.as_str()), Some("gpt-4"));
        assert!(transformed.get("messages").is_some());
    }
}
