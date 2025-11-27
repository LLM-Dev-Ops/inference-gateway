//! Retry policy with exponential backoff.
//!
//! Provides configurable retry logic with jitter for retryable errors.

use gateway_core::GatewayError;
use rand::Rng;
use std::future::Future;
use std::time::Duration;
use tracing::{debug, warn};

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_retries: u32,
    /// Base delay between retries
    pub base_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff multiplier
    pub multiplier: f64,
    /// Jitter factor (0.0 - 1.0)
    pub jitter: f64,
    /// HTTP status codes to retry on
    pub retry_on_status: Vec<u16>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
            jitter: 0.25,
            retry_on_status: vec![429, 500, 502, 503, 504],
        }
    }
}

/// Retry policy implementation
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    config: RetryConfig,
}

impl RetryPolicy {
    /// Create a new retry policy with the given configuration
    #[must_use]
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(RetryConfig::default())
    }

    /// Create a policy with custom max retries
    #[must_use]
    pub fn with_max_retries(max_retries: u32) -> Self {
        Self::new(RetryConfig {
            max_retries,
            ..Default::default()
        })
    }

    /// Calculate delay for a given attempt (0-indexed)
    #[must_use]
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base = self.config.base_delay.as_millis() as f64;
        let delay = base * self.config.multiplier.powi(attempt as i32);
        let delay = delay.min(self.config.max_delay.as_millis() as f64);

        // Apply jitter
        let jitter_range = delay * self.config.jitter;
        let jitter = rand::thread_rng().gen_range(-jitter_range..=jitter_range);
        let final_delay = (delay + jitter).max(0.0);

        Duration::from_millis(final_delay as u64)
    }

    /// Check if an error is retryable
    #[must_use]
    pub fn is_retryable(&self, error: &GatewayError) -> bool {
        match error {
            GatewayError::Provider {
                retryable,
                status_code,
                ..
            } => {
                if *retryable {
                    return true;
                }
                if let Some(code) = status_code {
                    return self.config.retry_on_status.contains(code);
                }
                false
            }
            GatewayError::Timeout { .. } => true,
            GatewayError::RateLimit { .. } => true,
            GatewayError::Streaming { .. } => true,
            _ => error.is_retryable(),
        }
    }

    /// Execute an operation with retry logic
    ///
    /// # Errors
    /// Returns the last error if all retries are exhausted
    pub async fn execute<F, Fut, T>(&self, operation: F) -> Result<T, GatewayError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, GatewayError>>,
    {
        let mut last_error: Option<GatewayError> = None;

        for attempt in 0..=self.config.max_retries {
            match operation().await {
                Ok(result) => {
                    if attempt > 0 {
                        debug!(attempt = attempt, "Retry succeeded");
                    }
                    return Ok(result);
                }
                Err(error) => {
                    if !self.is_retryable(&error) || attempt == self.config.max_retries {
                        return Err(error);
                    }

                    let delay = self.delay_for_attempt(attempt);
                    warn!(
                        attempt = attempt + 1,
                        max_retries = self.config.max_retries,
                        delay_ms = delay.as_millis(),
                        error = %error,
                        "Retrying after error"
                    );

                    tokio::time::sleep(delay).await;
                    last_error = Some(error);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| GatewayError::internal("Retry exhausted without error")))
    }

    /// Execute with a specific number of retries
    ///
    /// # Errors
    /// Returns the last error if all retries are exhausted
    pub async fn execute_with_retries<F, Fut, T>(
        &self,
        operation: F,
        max_retries: u32,
    ) -> Result<T, GatewayError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, GatewayError>>,
    {
        let mut policy = self.clone();
        policy.config.max_retries = max_retries;
        policy.execute(operation).await
    }

    /// Get the configuration
    #[must_use]
    pub fn config(&self) -> &RetryConfig {
        &self.config
    }
}

/// Result of a retry operation
#[derive(Debug)]
pub enum RetryResult<T> {
    /// Operation succeeded
    Success(T),
    /// Operation failed after all retries
    Failed {
        /// The final error
        error: GatewayError,
        /// Number of attempts made
        attempts: u32,
    },
    /// Operation failed with non-retryable error
    NonRetryable {
        /// The error
        error: GatewayError,
    },
}

impl<T> RetryResult<T> {
    /// Convert to a Result
    ///
    /// # Errors
    /// Returns the error if the operation failed
    pub fn into_result(self) -> Result<T, GatewayError> {
        match self {
            Self::Success(value) => Ok(value),
            Self::Failed { error, .. } | Self::NonRetryable { error } => Err(error),
        }
    }

    /// Check if the operation succeeded
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }
}

/// Builder for retry policy
#[derive(Debug, Default)]
pub struct RetryPolicyBuilder {
    config: RetryConfig,
}

impl RetryPolicyBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set max retries
    #[must_use]
    pub fn max_retries(mut self, n: u32) -> Self {
        self.config.max_retries = n;
        self
    }

    /// Set base delay
    #[must_use]
    pub fn base_delay(mut self, delay: Duration) -> Self {
        self.config.base_delay = delay;
        self
    }

    /// Set max delay
    #[must_use]
    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.config.max_delay = delay;
        self
    }

    /// Set backoff multiplier
    #[must_use]
    pub fn multiplier(mut self, multiplier: f64) -> Self {
        self.config.multiplier = multiplier;
        self
    }

    /// Set jitter factor
    #[must_use]
    pub fn jitter(mut self, jitter: f64) -> Self {
        self.config.jitter = jitter.clamp(0.0, 1.0);
        self
    }

    /// Set status codes to retry on
    #[must_use]
    pub fn retry_on_status(mut self, codes: Vec<u16>) -> Self {
        self.config.retry_on_status = codes;
        self
    }

    /// Build the policy
    #[must_use]
    pub fn build(self) -> RetryPolicy {
        RetryPolicy::new(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_delay_calculation() {
        let policy = RetryPolicy::new(RetryConfig {
            base_delay: Duration::from_millis(100),
            multiplier: 2.0,
            jitter: 0.0,
            ..Default::default()
        });

        // Without jitter, delays should be exact
        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(400));
    }

    #[test]
    fn test_delay_with_max() {
        let policy = RetryPolicy::new(RetryConfig {
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(300),
            multiplier: 2.0,
            jitter: 0.0,
            ..Default::default()
        });

        // Should be capped at max_delay
        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(300)); // Capped
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(300)); // Still capped
    }

    #[test]
    fn test_is_retryable() {
        let policy = RetryPolicy::with_defaults();

        // Retryable errors
        assert!(policy.is_retryable(&GatewayError::timeout(Duration::from_secs(30))));
        assert!(policy.is_retryable(&GatewayError::rate_limit(None, None)));
        assert!(policy.is_retryable(&GatewayError::provider("test", "error", Some(503), true)));

        // Non-retryable errors
        assert!(!policy.is_retryable(&GatewayError::validation("test", None, "test")));
        assert!(!policy.is_retryable(&GatewayError::authentication("test")));
        assert!(!policy.is_retryable(&GatewayError::provider("test", "error", Some(400), false)));
    }

    #[tokio::test]
    async fn test_retry_success_first_attempt() {
        let policy = RetryPolicy::with_max_retries(3);
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        let result: Result<u32, GatewayError> = policy
            .execute(|| {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                    Ok(42)
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let policy = RetryPolicy::new(RetryConfig {
            max_retries: 3,
            base_delay: Duration::from_millis(1),
            jitter: 0.0,
            ..Default::default()
        });
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        let result: Result<u32, GatewayError> = policy
            .execute(|| {
                let c = Arc::clone(&counter_clone);
                async move {
                    let attempt = c.fetch_add(1, Ordering::Relaxed);
                    if attempt < 2 {
                        Err(GatewayError::provider("test", "error", Some(503), true))
                    } else {
                        Ok(42)
                    }
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::Relaxed), 3); // Initial + 2 retries
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let policy = RetryPolicy::new(RetryConfig {
            max_retries: 2,
            base_delay: Duration::from_millis(1),
            jitter: 0.0,
            ..Default::default()
        });
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        let result: Result<u32, GatewayError> = policy
            .execute(|| {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                    Err(GatewayError::provider("test", "error", Some(503), true))
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::Relaxed), 3); // Initial + 2 retries
    }

    #[tokio::test]
    async fn test_non_retryable_error() {
        let policy = RetryPolicy::with_max_retries(3);
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        let result: Result<u32, GatewayError> = policy
            .execute(|| {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                    Err(GatewayError::validation("test", None, "test"))
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::Relaxed), 1); // No retries
    }

    #[test]
    fn test_builder() {
        let policy = RetryPolicyBuilder::new()
            .max_retries(5)
            .base_delay(Duration::from_millis(200))
            .max_delay(Duration::from_secs(30))
            .multiplier(3.0)
            .jitter(0.5)
            .build();

        assert_eq!(policy.config().max_retries, 5);
        assert_eq!(policy.config().base_delay, Duration::from_millis(200));
        assert_eq!(policy.config().max_delay, Duration::from_secs(30));
        assert!((policy.config().multiplier - 3.0).abs() < 0.001);
        assert!((policy.config().jitter - 0.5).abs() < 0.001);
    }
}
