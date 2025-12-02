//! Canonical benchmark infrastructure for the LLM Inference Gateway.
//!
//! This crate provides a standardized benchmark interface compatible with
//! the canonical benchmark pattern used across all benchmark-target repositories.
//!
//! # Usage
//!
//! ```rust,ignore
//! use gateway_benchmarks::{run_all_benchmarks, BenchmarkResult};
//!
//! let results = run_all_benchmarks().await;
//! for result in results {
//!     println!("{}: {:?}", result.target_id, result.metrics);
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod adapters;
pub mod io;
pub mod markdown;
pub mod result;

pub use adapters::{all_targets, BenchTarget};
pub use result::BenchmarkResult;

use anyhow::Result;
use std::path::Path;

/// Run all registered benchmarks and return their results.
///
/// This is the canonical entrypoint for the benchmark system, returning
/// a `Vec<BenchmarkResult>` containing results from all registered benchmark targets.
///
/// # Example
///
/// ```rust,ignore
/// let results = run_all_benchmarks().await;
/// ```
pub async fn run_all_benchmarks() -> Vec<BenchmarkResult> {
    let targets = all_targets();
    let mut results = Vec::with_capacity(targets.len());

    for target in targets {
        match target.run().await {
            Ok(result) => results.push(result),
            Err(e) => {
                // Create an error result for failed benchmarks
                results.push(BenchmarkResult {
                    target_id: target.id().to_string(),
                    metrics: serde_json::json!({
                        "error": e.to_string(),
                        "status": "failed"
                    }),
                    timestamp: chrono::Utc::now(),
                });
            }
        }
    }

    results
}

/// Run all benchmarks and write results to the canonical output directories.
///
/// This function:
/// 1. Runs all registered benchmarks via `run_all_benchmarks()`
/// 2. Writes raw JSON results to `benchmarks/output/raw/`
/// 3. Generates a summary markdown report at `benchmarks/output/summary.md`
///
/// # Arguments
///
/// * `output_dir` - Base output directory (typically `benchmarks/output/`)
pub async fn run_and_save_benchmarks<P: AsRef<Path>>(output_dir: P) -> Result<Vec<BenchmarkResult>> {
    let results = run_all_benchmarks().await;

    // Write raw results
    io::write_raw_results(&results, output_dir.as_ref())?;

    // Generate summary markdown
    let summary = markdown::generate_summary(&results);
    io::write_summary(&summary, output_dir.as_ref())?;

    Ok(results)
}

/// Get the default output directory for benchmark results.
pub fn default_output_dir() -> &'static str {
    "benchmarks/output"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_all_benchmarks() {
        let results = run_all_benchmarks().await;
        // Should have at least one benchmark target
        assert!(!results.is_empty());
    }
}
