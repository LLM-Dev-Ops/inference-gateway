//! # Gateway Resilience
//!
//! Resilience patterns for the LLM Inference Gateway:
//! - Circuit breaker for preventing cascading failures
//! - Retry policy with exponential backoff
//! - Bulkhead pattern for resource isolation
//! - Timeout management
//! - Rate limiting with token bucket algorithm
//! - Response caching for performance optimization
//! - Distributed caching with Redis support

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod bulkhead;
pub mod cache;
pub mod circuit_breaker;
pub mod distributed_cache;
pub mod rate_limiter;
pub mod retry;
pub mod timeout;

// Re-export main types
pub use bulkhead::{Bulkhead, BulkheadConfig, BulkheadPermit};
pub use cache::{CacheConfig, CacheKey, CacheLookupResult, CacheStats, ResponseCache};
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
pub use distributed_cache::{
    CacheBackend, CacheResult, CachedEntry, DistributedCache, DistributedCacheConfig,
    DistributedCacheConfigBuilder, DistributedCacheError, DistributedCacheKey,
    DistributedCacheStats, MemoryCacheBackend, RedisCacheBackend,
};
pub use rate_limiter::{
    BucketStats, RateLimitExceeded, RateLimitType, RateLimiter, RateLimiterConfig,
};
pub use retry::{RetryConfig, RetryPolicy, RetryResult};
pub use timeout::{TimeoutConfig, TimeoutManager};
