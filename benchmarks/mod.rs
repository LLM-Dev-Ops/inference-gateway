//! Canonical benchmark module entrypoint.
//!
//! This module re-exports the benchmark infrastructure from the
//! gateway-benchmarks crate for convenience.
//!
//! # Usage
//!
//! ```rust,ignore
//! use benchmarks::{run_all_benchmarks, BenchmarkResult};
//!
//! let results = run_all_benchmarks().await;
//! ```

pub use gateway_benchmarks::*;
