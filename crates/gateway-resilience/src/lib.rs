//! # Gateway Resilience
//!
//! Resilience patterns for the LLM Inference Gateway:
//! - Circuit breaker for preventing cascading failures
//! - Retry policy with exponential backoff
//! - Bulkhead pattern for resource isolation
//! - Timeout management

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod circuit_breaker;
pub mod retry;
pub mod bulkhead;
pub mod timeout;

// Re-export main types
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
pub use retry::{RetryPolicy, RetryConfig, RetryResult};
pub use bulkhead::{Bulkhead, BulkheadConfig, BulkheadPermit};
pub use timeout::{TimeoutManager, TimeoutConfig};
