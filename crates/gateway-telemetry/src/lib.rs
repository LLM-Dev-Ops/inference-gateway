//! # Gateway Telemetry
//!
//! Observability and telemetry for the LLM Inference Gateway.
//!
//! This crate provides:
//! - Prometheus metrics for monitoring
//! - Distributed tracing with OpenTelemetry
//! - Structured logging
//! - Request/response tracking

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod metrics;
pub mod tracing_setup;
pub mod logging;
pub mod request_tracker;

// Re-export main types
pub use metrics::{Metrics, MetricsConfig, RequestMetrics};
pub use tracing_setup::{TracingConfig, init_tracing, shutdown_tracing};
pub use logging::{LoggingConfig, init_logging};
pub use request_tracker::{RequestTracker, RequestInfo, RequestOutcome};
