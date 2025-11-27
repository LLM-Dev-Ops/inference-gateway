//! Circuit breaker pattern implementation.
//!
//! The circuit breaker prevents cascading failures by stopping requests
//! to a failing service and allowing it time to recover.

use gateway_core::GatewayError;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed = 0,
    /// Circuit is open, requests are rejected
    Open = 1,
    /// Circuit is half-open, testing if service recovered
    HalfOpen = 2,
}

impl From<u8> for CircuitState {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Closed,
            1 => Self::Open,
            2 => Self::HalfOpen,
            _ => Self::Closed,
        }
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// Number of successes required to close the circuit
    pub success_threshold: u32,
    /// Time to wait before testing the circuit (half-open)
    pub timeout: Duration,
    /// Sliding window size for failure rate calculation
    pub window_size: u32,
    /// Minimum requests before failure rate is considered
    pub min_requests: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(30),
            window_size: 100,
            min_requests: 10,
        }
    }
}

/// Circuit breaker for a single provider
pub struct CircuitBreaker {
    /// Provider identifier
    provider_id: String,
    /// Configuration
    config: CircuitBreakerConfig,
    /// Current state (atomic for lock-free reads)
    state: AtomicU8,
    /// Failure count in current window
    failure_count: AtomicU32,
    /// Success count in half-open state
    half_open_successes: AtomicU32,
    /// Total request count in window
    request_count: AtomicU32,
    /// Timestamp when circuit opened (milliseconds since epoch)
    opened_at: AtomicU64,
    /// Lock for state transitions
    transition_lock: RwLock<()>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    #[must_use]
    pub fn new(provider_id: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            provider_id: provider_id.into(),
            config,
            state: AtomicU8::new(CircuitState::Closed as u8),
            failure_count: AtomicU32::new(0),
            half_open_successes: AtomicU32::new(0),
            request_count: AtomicU32::new(0),
            opened_at: AtomicU64::new(0),
            transition_lock: RwLock::new(()),
        }
    }

    /// Create with default configuration
    #[must_use]
    pub fn with_defaults(provider_id: impl Into<String>) -> Self {
        Self::new(provider_id, CircuitBreakerConfig::default())
    }

    /// Get the provider ID
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Get the current state
    #[must_use]
    pub fn state(&self) -> CircuitState {
        CircuitState::from(self.state.load(Ordering::Acquire))
    }

    /// Check if the circuit allows requests
    ///
    /// Returns Ok if request can proceed, Err if circuit is open
    ///
    /// # Errors
    /// Returns `GatewayError::CircuitBreakerOpen` if circuit is open
    pub fn check(&self) -> Result<(), GatewayError> {
        let current_state = self.state();

        match current_state {
            CircuitState::Closed => Ok(()),
            CircuitState::HalfOpen => {
                // Allow limited requests in half-open state
                Ok(())
            }
            CircuitState::Open => {
                // Check if timeout has elapsed
                if self.should_attempt_reset() {
                    self.transition_to_half_open();
                    Ok(())
                } else {
                    Err(GatewayError::circuit_breaker_open(&self.provider_id))
                }
            }
        }
    }

    /// Record a successful request
    pub fn record_success(&self) {
        self.request_count.fetch_add(1, Ordering::Relaxed);

        let current_state = self.state();
        match current_state {
            CircuitState::Closed => {
                // Reset failure count on success in closed state
                // (optional, depends on your failure rate calculation strategy)
            }
            CircuitState::HalfOpen => {
                let successes = self.half_open_successes.fetch_add(1, Ordering::Relaxed) + 1;
                debug!(
                    provider = %self.provider_id,
                    successes = successes,
                    threshold = self.config.success_threshold,
                    "Circuit breaker half-open success"
                );

                if successes >= self.config.success_threshold {
                    self.transition_to_closed();
                }
            }
            CircuitState::Open => {
                // Shouldn't happen, but handle gracefully
            }
        }
    }

    /// Record a failed request
    pub fn record_failure(&self) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
        let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;

        let current_state = self.state();
        match current_state {
            CircuitState::Closed => {
                let requests = self.request_count.load(Ordering::Relaxed);

                // Only consider failure threshold if we have minimum requests
                if requests >= self.config.min_requests && failures >= self.config.failure_threshold {
                    debug!(
                        provider = %self.provider_id,
                        failures = failures,
                        threshold = self.config.failure_threshold,
                        "Circuit breaker failure threshold reached"
                    );
                    self.transition_to_open();
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state opens the circuit
                debug!(
                    provider = %self.provider_id,
                    "Circuit breaker half-open failure, reopening"
                );
                self.transition_to_open();
            }
            CircuitState::Open => {
                // Already open, nothing to do
            }
        }
    }

    /// Check if we should attempt to reset (timeout elapsed)
    fn should_attempt_reset(&self) -> bool {
        let opened_at = self.opened_at.load(Ordering::Acquire);
        if opened_at == 0 {
            return false;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let elapsed = now.saturating_sub(opened_at);
        elapsed >= self.config.timeout.as_millis() as u64
    }

    /// Transition to open state
    fn transition_to_open(&self) {
        let _guard = self.transition_lock.write();

        let prev_state = self.state.swap(CircuitState::Open as u8, Ordering::Release);

        if prev_state != CircuitState::Open as u8 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            self.opened_at.store(now, Ordering::Release);
            self.half_open_successes.store(0, Ordering::Relaxed);

            warn!(
                provider = %self.provider_id,
                "Circuit breaker opened"
            );
        }
    }

    /// Transition to half-open state
    fn transition_to_half_open(&self) {
        let _guard = self.transition_lock.write();

        let prev_state = self.state.swap(CircuitState::HalfOpen as u8, Ordering::Release);

        if prev_state == CircuitState::Open as u8 {
            self.half_open_successes.store(0, Ordering::Relaxed);

            info!(
                provider = %self.provider_id,
                "Circuit breaker half-open, testing"
            );
        }
    }

    /// Transition to closed state
    fn transition_to_closed(&self) {
        let _guard = self.transition_lock.write();

        self.state
            .store(CircuitState::Closed as u8, Ordering::Release);
        self.failure_count.store(0, Ordering::Relaxed);
        self.half_open_successes.store(0, Ordering::Relaxed);
        self.request_count.store(0, Ordering::Relaxed);
        self.opened_at.store(0, Ordering::Release);

        info!(
            provider = %self.provider_id,
            "Circuit breaker closed"
        );
    }

    /// Reset the circuit breaker to closed state
    pub fn reset(&self) {
        self.transition_to_closed();
    }

    /// Force the circuit open (for testing or manual intervention)
    pub fn force_open(&self) {
        self.transition_to_open();
    }

    /// Get current statistics
    #[must_use]
    pub fn stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            state: self.state(),
            failure_count: self.failure_count.load(Ordering::Relaxed),
            request_count: self.request_count.load(Ordering::Relaxed),
            half_open_successes: self.half_open_successes.load(Ordering::Relaxed),
        }
    }
}

/// Circuit breaker statistics
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    /// Current state
    pub state: CircuitState,
    /// Failure count
    pub failure_count: u32,
    /// Total request count
    pub request_count: u32,
    /// Success count in half-open state
    pub half_open_successes: u32,
}

impl CircuitBreakerStats {
    /// Calculate failure rate
    #[must_use]
    pub fn failure_rate(&self) -> f64 {
        if self.request_count == 0 {
            0.0
        } else {
            f64::from(self.failure_count) / f64::from(self.request_count)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_initial_state() {
        let cb = CircuitBreaker::with_defaults("test-provider");
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.check().is_ok());
    }

    #[test]
    fn test_circuit_breaker_opens_on_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            min_requests: 1,
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test-provider", config);

        // Record failures
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(cb.check().is_err());
    }

    #[test]
    fn test_circuit_breaker_half_open_success() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_millis(10),
            min_requests: 1,
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test-provider", config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(20));

        // Should transition to half-open on check
        assert!(cb.check().is_ok());
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Record successes to close
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_half_open_failure() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_millis(10),
            min_requests: 1,
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test-provider", config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(20));

        // Transition to half-open
        assert!(cb.check().is_ok());
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Failure in half-open reopens circuit
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            min_requests: 1,
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test-provider", config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Reset
        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.check().is_ok());
    }

    #[test]
    fn test_circuit_breaker_stats() {
        let config = CircuitBreakerConfig {
            failure_threshold: 5,
            min_requests: 1,
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test-provider", config);

        cb.record_success();
        cb.record_failure();
        cb.record_failure();

        let stats = cb.stats();
        assert_eq!(stats.request_count, 3);
        assert_eq!(stats.failure_count, 2);
        assert!((stats.failure_rate() - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_min_requests_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            min_requests: 10,
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test-provider", config);

        // Record failures but below min_requests
        for _ in 0..5 {
            cb.record_failure();
        }

        // Should still be closed because we haven't hit min_requests
        assert_eq!(cb.state(), CircuitState::Closed);
    }
}
