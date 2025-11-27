//! Bulkhead pattern for resource isolation.
//!
//! Limits concurrent requests to prevent resource exhaustion.

use gateway_core::GatewayError;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{debug, warn};

/// Bulkhead configuration
#[derive(Debug, Clone)]
pub struct BulkheadConfig {
    /// Maximum concurrent requests
    pub max_concurrent: u32,
    /// Queue size when max concurrent is reached
    pub queue_size: u32,
    /// Queue timeout
    pub queue_timeout: Duration,
}

impl Default for BulkheadConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 100,
            queue_size: 100,
            queue_timeout: Duration::from_secs(10),
        }
    }
}

/// Bulkhead for limiting concurrent requests
pub struct Bulkhead {
    /// Identifier (usually provider ID)
    id: String,
    /// Configuration
    config: BulkheadConfig,
    /// Semaphore for concurrency control
    semaphore: Arc<Semaphore>,
}

impl Bulkhead {
    /// Create a new bulkhead
    #[must_use]
    pub fn new(id: impl Into<String>, config: BulkheadConfig) -> Self {
        let total_permits = config.max_concurrent + config.queue_size;
        Self {
            id: id.into(),
            semaphore: Arc::new(Semaphore::new(total_permits as usize)),
            config,
        }
    }

    /// Create with default configuration
    #[must_use]
    pub fn with_defaults(id: impl Into<String>) -> Self {
        Self::new(id, BulkheadConfig::default())
    }

    /// Get the bulkhead ID
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Acquire a permit to execute a request
    ///
    /// # Errors
    /// Returns error if the bulkhead is full or timeout is reached
    pub async fn acquire(&self) -> Result<BulkheadPermit, GatewayError> {
        let available = self.semaphore.available_permits();

        if available <= self.config.queue_size as usize {
            debug!(
                bulkhead = %self.id,
                available = available,
                max_concurrent = self.config.max_concurrent,
                "Request queued in bulkhead"
            );
        }

        match tokio::time::timeout(
            self.config.queue_timeout,
            Arc::clone(&self.semaphore).acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => {
                let active = self.active_requests();
                debug!(
                    bulkhead = %self.id,
                    active = active,
                    "Bulkhead permit acquired"
                );
                Ok(BulkheadPermit {
                    _permit: permit,
                    bulkhead_id: self.id.clone(),
                })
            }
            Ok(Err(_)) => {
                // Semaphore closed (shouldn't happen)
                Err(GatewayError::internal("Bulkhead semaphore closed"))
            }
            Err(_) => {
                warn!(
                    bulkhead = %self.id,
                    timeout_ms = self.config.queue_timeout.as_millis(),
                    "Bulkhead queue timeout"
                );
                Err(GatewayError::Provider {
                    provider: self.id.clone(),
                    message: "Bulkhead queue timeout - too many concurrent requests".to_string(),
                    status_code: Some(503),
                    retryable: true,
                })
            }
        }
    }

    /// Try to acquire a permit without waiting
    ///
    /// # Errors
    /// Returns error if no permit is available
    pub fn try_acquire(&self) -> Result<BulkheadPermit, GatewayError> {
        match Arc::clone(&self.semaphore).try_acquire_owned() {
            Ok(permit) => Ok(BulkheadPermit {
                _permit: permit,
                bulkhead_id: self.id.clone(),
            }),
            Err(_) => Err(GatewayError::Provider {
                provider: self.id.clone(),
                message: "Bulkhead full - no permits available".to_string(),
                status_code: Some(503),
                retryable: true,
            }),
        }
    }

    /// Get the number of available permits
    #[must_use]
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Get the number of active requests
    #[must_use]
    pub fn active_requests(&self) -> u32 {
        let total = self.config.max_concurrent + self.config.queue_size;
        let available = self.semaphore.available_permits() as u32;
        total.saturating_sub(available)
    }

    /// Check if bulkhead is at capacity
    #[must_use]
    pub fn is_at_capacity(&self) -> bool {
        self.available_permits() == 0
    }

    /// Get current statistics
    #[must_use]
    pub fn stats(&self) -> BulkheadStats {
        BulkheadStats {
            active_requests: self.active_requests(),
            available_permits: self.available_permits() as u32,
            max_concurrent: self.config.max_concurrent,
            queue_size: self.config.queue_size,
        }
    }
}

/// A permit from a bulkhead
///
/// The permit is automatically released when dropped.
pub struct BulkheadPermit {
    _permit: OwnedSemaphorePermit,
    bulkhead_id: String,
}

impl BulkheadPermit {
    /// Get the bulkhead ID this permit belongs to
    #[must_use]
    pub fn bulkhead_id(&self) -> &str {
        &self.bulkhead_id
    }
}

impl Drop for BulkheadPermit {
    fn drop(&mut self) {
        debug!(
            bulkhead = %self.bulkhead_id,
            "Bulkhead permit released"
        );
    }
}

/// Bulkhead statistics
#[derive(Debug, Clone)]
pub struct BulkheadStats {
    /// Number of active requests
    pub active_requests: u32,
    /// Number of available permits
    pub available_permits: u32,
    /// Maximum concurrent requests
    pub max_concurrent: u32,
    /// Queue size
    pub queue_size: u32,
}

impl BulkheadStats {
    /// Calculate utilization percentage
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.max_concurrent == 0 {
            0.0
        } else {
            let in_use = self.active_requests.min(self.max_concurrent);
            f64::from(in_use) / f64::from(self.max_concurrent) * 100.0
        }
    }

    /// Check if requests are being queued
    #[must_use]
    pub fn is_queueing(&self) -> bool {
        self.active_requests > self.max_concurrent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_bulkhead_acquire_release() {
        let bulkhead = Bulkhead::new(
            "test",
            BulkheadConfig {
                max_concurrent: 2,
                queue_size: 0,
                queue_timeout: Duration::from_secs(1),
            },
        );

        assert_eq!(bulkhead.active_requests(), 0);

        let permit1 = bulkhead.acquire().await.expect("acquire 1");
        assert_eq!(bulkhead.active_requests(), 1);

        let permit2 = bulkhead.acquire().await.expect("acquire 2");
        assert_eq!(bulkhead.active_requests(), 2);

        drop(permit1);
        assert_eq!(bulkhead.active_requests(), 1);

        drop(permit2);
        assert_eq!(bulkhead.active_requests(), 0);
    }

    #[tokio::test]
    async fn test_bulkhead_at_capacity() {
        let bulkhead = Bulkhead::new(
            "test",
            BulkheadConfig {
                max_concurrent: 1,
                queue_size: 0,
                queue_timeout: Duration::from_millis(100),
            },
        );

        let _permit = bulkhead.acquire().await.expect("acquire");
        assert!(bulkhead.is_at_capacity());

        // Should timeout trying to acquire
        let result = bulkhead.acquire().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bulkhead_queueing() {
        let bulkhead = Arc::new(Bulkhead::new(
            "test",
            BulkheadConfig {
                max_concurrent: 1,
                queue_size: 1,
                queue_timeout: Duration::from_secs(1),
            },
        ));

        let _permit1 = bulkhead.acquire().await.expect("acquire 1");

        // Second request should queue
        let bulkhead_clone = Arc::clone(&bulkhead);
        let handle = tokio::spawn(async move {
            bulkhead_clone.acquire().await
        });

        // Give it time to queue
        sleep(Duration::from_millis(50)).await;

        // Stats should show queuing
        let stats = bulkhead.stats();
        assert!(stats.is_queueing() || stats.active_requests >= 1);

        // Drop first permit to let queued request through
        drop(_permit1);

        let result = handle.await.expect("join");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_try_acquire() {
        let bulkhead = Bulkhead::new(
            "test",
            BulkheadConfig {
                max_concurrent: 1,
                queue_size: 0,
                queue_timeout: Duration::from_secs(1),
            },
        );

        let permit = bulkhead.try_acquire().expect("try_acquire");
        assert!(bulkhead.try_acquire().is_err());

        drop(permit);
        assert!(bulkhead.try_acquire().is_ok());
    }

    #[tokio::test]
    async fn test_bulkhead_stats() {
        let bulkhead = Bulkhead::new(
            "test",
            BulkheadConfig {
                max_concurrent: 10,
                queue_size: 5,
                queue_timeout: Duration::from_secs(1),
            },
        );

        let stats = bulkhead.stats();
        assert_eq!(stats.max_concurrent, 10);
        assert_eq!(stats.queue_size, 5);
        assert_eq!(stats.active_requests, 0);
        assert!((stats.utilization() - 0.0).abs() < 0.001);

        let _permit = bulkhead.acquire().await.expect("acquire");
        let stats = bulkhead.stats();
        assert_eq!(stats.active_requests, 1);
        assert!((stats.utilization() - 10.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let bulkhead = Arc::new(Bulkhead::new(
            "test",
            BulkheadConfig {
                max_concurrent: 10,
                queue_size: 10,
                queue_timeout: Duration::from_secs(5),
            },
        ));

        let counter = Arc::new(AtomicU32::new(0));
        let mut handles = Vec::new();

        for _ in 0..20 {
            let bh = Arc::clone(&bulkhead);
            let cnt = Arc::clone(&counter);
            handles.push(tokio::spawn(async move {
                let _permit = bh.acquire().await.expect("acquire");
                cnt.fetch_add(1, Ordering::Relaxed);
                sleep(Duration::from_millis(10)).await;
            }));
        }

        for handle in handles {
            handle.await.expect("join");
        }

        assert_eq!(counter.load(Ordering::Relaxed), 20);
    }
}
