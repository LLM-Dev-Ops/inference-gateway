//! # Gateway Telemetry
//!
//! Observability and telemetry for the LLM Inference Gateway.
//!
//! This crate provides:
//! - Prometheus metrics for monitoring
//! - Distributed tracing with OpenTelemetry
//! - Structured logging
//! - Request/response tracking
//! - Audit logging for compliance
//! - Cost tracking and billing
//! - PII redaction for logs

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod audit;
pub mod cost;
pub mod logging;
pub mod metrics;
pub mod pii;
pub mod request_tracker;
pub mod tracing_setup;

// Re-export main types
pub use audit::{
    AuditActor, AuditEvent, AuditEventBuilder, AuditEventType, AuditLogConfig, AuditLogger,
    AuditOutcome, AuditResource, AuditSeverity, AuditStats,
};
pub use cost::{
    Budget, BudgetStatus, CostConfig, CostReport, CostTracker, ModelPricing, UsageEvent,
    UsageStats,
};
pub use logging::{init_logging, LoggingConfig};
pub use metrics::{Metrics, MetricsConfig, RequestMetrics};
pub use request_tracker::{RequestInfo, RequestOutcome, RequestTracker};
pub use pii::{
    CustomPattern, PiiAnalysis, PiiConfig, PiiPattern, PiiPatternConfig, PiiRedactor,
    RedactPii, RedactionStyle,
};
pub use tracing_setup::{init_tracing, shutdown_tracing, TracingConfig};
