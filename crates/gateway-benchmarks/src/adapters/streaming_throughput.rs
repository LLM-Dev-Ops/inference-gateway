//! Streaming throughput benchmark adapter.
//!
//! Measures the throughput of streaming response handling
//! including SSE parsing and token emission.

use super::BenchTarget;
use crate::BenchmarkResult;
use anyhow::Result;
use async_trait::async_trait;
use std::time::Instant;

/// Benchmark for streaming response throughput.
///
/// This benchmark measures:
/// - SSE event parsing throughput
/// - Token emission rate
/// - Stream buffer handling latency
pub struct StreamingThroughputBenchmark {
    iterations: u32,
    tokens_per_stream: u32,
}

impl StreamingThroughputBenchmark {
    /// Create a new streaming throughput benchmark.
    pub fn new() -> Self {
        Self {
            iterations: 1000,
            tokens_per_stream: 100,
        }
    }

    /// Create with custom parameters.
    pub fn with_params(iterations: u32, tokens_per_stream: u32) -> Self {
        Self {
            iterations,
            tokens_per_stream,
        }
    }
}

impl Default for StreamingThroughputBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BenchTarget for StreamingThroughputBenchmark {
    fn id(&self) -> &str {
        "streaming_throughput"
    }

    fn description(&self) -> &str {
        "Measures streaming response throughput including SSE parsing and token emission"
    }

    fn iterations(&self) -> u32 {
        self.iterations
    }

    async fn run(&self) -> Result<BenchmarkResult> {
        let mut latencies = Vec::with_capacity(self.iterations as usize);
        let mut tokens_processed = Vec::with_capacity(self.iterations as usize);

        // Warmup
        for _ in 0..self.warmup_iterations() {
            simulate_stream_processing(self.tokens_per_stream).await;
        }

        // Benchmark
        let overall_start = Instant::now();
        let mut total_tokens = 0u64;

        for _ in 0..self.iterations {
            let start = Instant::now();
            let tokens = simulate_stream_processing(self.tokens_per_stream).await;
            latencies.push(start.elapsed().as_nanos() as f64 / 1_000_000.0);
            tokens_processed.push(tokens);
            total_tokens += tokens as u64;
        }

        let total_duration = overall_start.elapsed();

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

        let tokens_per_second =
            total_tokens as f64 / total_duration.as_secs_f64();
        let streams_per_second = self.iterations as f64 / total_duration.as_secs_f64();

        Ok(BenchmarkResult::new(
            self.id(),
            serde_json::json!({
                "iterations": self.iterations,
                "tokens_per_stream": self.tokens_per_stream,
                "latency_ms": avg_ms,
                "min_ms": min_ms,
                "max_ms": max_ms,
                "p50_ms": latencies.get(p50_idx).copied().unwrap_or(0.0),
                "p90_ms": latencies.get(p90_idx).copied().unwrap_or(0.0),
                "p95_ms": latencies.get(p95_idx).copied().unwrap_or(0.0),
                "p99_ms": latencies.get(p99_idx).copied().unwrap_or(0.0),
                "tokens_per_second": tokens_per_second,
                "streams_per_second": streams_per_second,
                "throughput_rps": streams_per_second,
                "total_tokens": total_tokens,
                "description": self.description()
            }),
        ))
    }
}

/// Simulate stream processing returning number of tokens processed.
async fn simulate_stream_processing(token_count: u32) -> u32 {
    // Simulate SSE event parsing and token emission
    for i in 0..token_count {
        // Simulate parsing an SSE event
        let _event = format!("data: {{\"token\": \"word{}\"}}\n\n", i);

        // Yield periodically to simulate async I/O
        if i % 10 == 0 {
            tokio::task::yield_now().await;
        }
    }

    token_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_streaming_throughput_benchmark() {
        let benchmark = StreamingThroughputBenchmark::with_params(100, 50);
        let result = benchmark.run().await.expect("Benchmark should succeed");

        assert_eq!(result.target_id, "streaming_throughput");
        assert!(result.metrics.get("tokens_per_second").is_some());
    }
}
