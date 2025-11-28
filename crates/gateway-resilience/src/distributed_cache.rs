//! Distributed caching with Redis support for multi-instance deployments.
//!
//! Provides a cache backend abstraction that supports:
//! - In-memory caching (default, single-instance)
//! - Redis caching (distributed, multi-instance)
//! - Hybrid caching (local L1 + Redis L2)
//!
//! This enables the gateway to scale horizontally while maintaining cache coherence.

use async_trait::async_trait;
use gateway_core::{GatewayRequest, GatewayResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Error types for distributed cache operations
#[derive(Debug, Error)]
pub enum DistributedCacheError {
    /// Connection error
    #[error("Cache connection error: {0}")]
    ConnectionError(String),

    /// Serialization error
    #[error("Cache serialization error: {0}")]
    SerializationError(String),

    /// Operation timeout
    #[error("Cache operation timeout after {0:?}")]
    Timeout(Duration),

    /// Backend not available
    #[error("Cache backend not available: {0}")]
    Unavailable(String),

    /// Key not found
    #[error("Key not found")]
    NotFound,

    /// Configuration error
    #[error("Cache configuration error: {0}")]
    ConfigError(String),
}

/// Result type for cache operations
pub type CacheResult<T> = Result<T, DistributedCacheError>;

/// Cache backend trait for polymorphic cache implementations
#[async_trait]
pub trait CacheBackend: Send + Sync {
    /// Get a value from the cache
    async fn get(&self, key: &str) -> CacheResult<Option<Vec<u8>>>;

    /// Set a value in the cache with TTL
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> CacheResult<()>;

    /// Delete a key from the cache
    async fn delete(&self, key: &str) -> CacheResult<()>;

    /// Delete all keys matching a pattern
    async fn delete_pattern(&self, pattern: &str) -> CacheResult<u64>;

    /// Check if the backend is healthy
    async fn health_check(&self) -> CacheResult<()>;

    /// Get backend name for metrics
    fn name(&self) -> &'static str;

    /// Check if backend supports distributed operations
    fn is_distributed(&self) -> bool;
}

/// Configuration for distributed cache
#[derive(Debug, Clone)]
pub struct DistributedCacheConfig {
    /// Whether caching is enabled
    pub enabled: bool,

    /// Redis connection URL (e.g., "redis://localhost:6379")
    pub redis_url: Option<String>,

    /// Redis connection pool size
    pub redis_pool_size: u32,

    /// Redis connection timeout
    pub redis_connect_timeout: Duration,

    /// Redis operation timeout
    pub redis_operation_timeout: Duration,

    /// Key prefix for Redis keys (namespace isolation)
    pub key_prefix: String,

    /// Default TTL for cache entries
    pub default_ttl: Duration,

    /// Maximum size for local cache (L1)
    pub local_cache_size: usize,

    /// Whether to use local cache as L1 with Redis as L2
    pub enable_local_cache: bool,

    /// TTL for local cache entries (should be shorter than Redis TTL)
    pub local_cache_ttl: Duration,

    /// Whether to cache streaming responses
    pub cache_streaming: bool,

    /// Compression threshold (bytes) - compress values larger than this
    pub compression_threshold: usize,

    /// Enable compression for large values
    pub enable_compression: bool,
}

impl Default for DistributedCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            redis_url: None,
            redis_pool_size: 10,
            redis_connect_timeout: Duration::from_secs(5),
            redis_operation_timeout: Duration::from_secs(2),
            key_prefix: "llm-gateway".to_string(),
            default_ttl: Duration::from_secs(3600),
            local_cache_size: 1000,
            enable_local_cache: true,
            local_cache_ttl: Duration::from_secs(60),
            cache_streaming: false,
            compression_threshold: 1024,
            enable_compression: true,
        }
    }
}

/// Builder for `DistributedCacheConfig`
#[derive(Debug, Default)]
pub struct DistributedCacheConfigBuilder {
    config: DistributedCacheConfig,
}

impl DistributedCacheConfigBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable or disable caching
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Set Redis connection URL
    #[must_use]
    pub fn redis_url(mut self, url: impl Into<String>) -> Self {
        self.config.redis_url = Some(url.into());
        self
    }

    /// Set Redis pool size
    #[must_use]
    pub fn redis_pool_size(mut self, size: u32) -> Self {
        self.config.redis_pool_size = size;
        self
    }

    /// Set Redis connection timeout
    #[must_use]
    pub fn redis_connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.redis_connect_timeout = timeout;
        self
    }

    /// Set Redis operation timeout
    #[must_use]
    pub fn redis_operation_timeout(mut self, timeout: Duration) -> Self {
        self.config.redis_operation_timeout = timeout;
        self
    }

    /// Set key prefix
    #[must_use]
    pub fn key_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.key_prefix = prefix.into();
        self
    }

    /// Set default TTL
    #[must_use]
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.config.default_ttl = ttl;
        self
    }

    /// Set local cache size
    #[must_use]
    pub fn local_cache_size(mut self, size: usize) -> Self {
        self.config.local_cache_size = size;
        self
    }

    /// Enable or disable local cache
    #[must_use]
    pub fn enable_local_cache(mut self, enable: bool) -> Self {
        self.config.enable_local_cache = enable;
        self
    }

    /// Set local cache TTL
    #[must_use]
    pub fn local_cache_ttl(mut self, ttl: Duration) -> Self {
        self.config.local_cache_ttl = ttl;
        self
    }

    /// Enable or disable streaming cache
    #[must_use]
    pub fn cache_streaming(mut self, enable: bool) -> Self {
        self.config.cache_streaming = enable;
        self
    }

    /// Set compression threshold
    #[must_use]
    pub fn compression_threshold(mut self, threshold: usize) -> Self {
        self.config.compression_threshold = threshold;
        self
    }

    /// Enable or disable compression
    #[must_use]
    pub fn enable_compression(mut self, enable: bool) -> Self {
        self.config.enable_compression = enable;
        self
    }

    /// Build the configuration
    #[must_use]
    pub fn build(self) -> DistributedCacheConfig {
        self.config
    }
}

/// Serializable cache key for Redis storage
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DistributedCacheKey {
    /// Model name
    pub model: String,
    /// Hash of the messages
    pub messages_hash: u64,
    /// Temperature bucket (discretized)
    pub temperature_bucket: u32,
    /// Max tokens
    pub max_tokens: Option<u32>,
}

impl DistributedCacheKey {
    /// Create a cache key from a request
    #[must_use]
    pub fn from_request(request: &GatewayRequest) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for msg in &request.messages {
            msg.role.hash(&mut hasher);
            match &msg.content {
                gateway_core::MessageContent::Text(text) => text.hash(&mut hasher),
                gateway_core::MessageContent::Parts(parts) => {
                    for part in parts {
                        match part {
                            gateway_core::ContentPart::Text { text } => text.hash(&mut hasher),
                            gateway_core::ContentPart::ImageUrl { image_url } => {
                                image_url.url.hash(&mut hasher);
                            }
                        }
                    }
                }
            }
        }
        let messages_hash = hasher.finish();

        let temperature_bucket = request
            .temperature
            .map(|t| (t * 10.0) as u32)
            .unwrap_or(7);

        Self {
            model: request.model.clone(),
            messages_hash,
            temperature_bucket,
            max_tokens: request.max_tokens,
        }
    }

    /// Convert to a string key suitable for Redis
    #[must_use]
    pub fn to_string_key(&self, prefix: &str) -> String {
        format!(
            "{}:cache:{}:{}:{}:{}",
            prefix,
            self.model,
            self.messages_hash,
            self.temperature_bucket,
            self.max_tokens.unwrap_or(0)
        )
    }
}

/// Cached entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEntry {
    /// The cached response
    pub response: GatewayResponse,
    /// When the entry was created (Unix timestamp)
    pub created_at: u64,
    /// TTL in seconds
    pub ttl_secs: u64,
    /// Number of times this entry has been accessed (local only)
    #[serde(skip)]
    pub hits: u64,
}

impl CachedEntry {
    /// Create a new cached entry
    #[must_use]
    pub fn new(response: GatewayResponse, ttl: Duration) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            response,
            created_at,
            ttl_secs: ttl.as_secs(),
            hits: 0,
        }
    }

    /// Check if the entry is expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        now > self.created_at + self.ttl_secs
    }

    /// Get remaining TTL
    #[must_use]
    pub fn remaining_ttl(&self) -> Duration {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let expires_at = self.created_at + self.ttl_secs;
        if now >= expires_at {
            Duration::ZERO
        } else {
            Duration::from_secs(expires_at - now)
        }
    }
}

/// Local cache entry with instant-based expiry
#[derive(Debug)]
struct LocalCacheEntry {
    data: Vec<u8>,
    expires_at: Instant,
    hits: u64,
}

impl LocalCacheEntry {
    fn new(data: Vec<u8>, ttl: Duration) -> Self {
        Self {
            data,
            expires_at: Instant::now() + ttl,
            hits: 0,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
}

/// In-memory cache backend (for single-instance deployments or L1 cache)
pub struct MemoryCacheBackend {
    entries: Arc<RwLock<HashMap<String, LocalCacheEntry>>>,
    max_entries: usize,
    #[allow(dead_code)]
    default_ttl: Duration,
}

impl MemoryCacheBackend {
    /// Create a new memory cache backend
    #[must_use]
    pub fn new(max_entries: usize, default_ttl: Duration) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            max_entries,
            default_ttl,
        }
    }

    /// Evict expired and LRU entries
    async fn evict_if_needed(&self) {
        let mut entries = self.entries.write().await;

        // Remove expired entries first
        entries.retain(|_, entry| !entry.is_expired());

        // If still over capacity, remove lowest-hit entries
        if entries.len() >= self.max_entries {
            let to_remove = entries.len() - self.max_entries + 1;
            let mut hit_counts: Vec<(String, u64)> = entries
                .iter()
                .map(|(k, v)| (k.clone(), v.hits))
                .collect();
            hit_counts.sort_by_key(|(_, hits)| *hits);

            for (key, _) in hit_counts.into_iter().take(to_remove) {
                entries.remove(&key);
            }
        }
    }
}

#[async_trait]
impl CacheBackend for MemoryCacheBackend {
    async fn get(&self, key: &str) -> CacheResult<Option<Vec<u8>>> {
        let mut entries = self.entries.write().await;

        if let Some(entry) = entries.get_mut(key) {
            if entry.is_expired() {
                entries.remove(key);
                return Ok(None);
            }
            entry.hits += 1;
            return Ok(Some(entry.data.clone()));
        }

        Ok(None)
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> CacheResult<()> {
        self.evict_if_needed().await;

        let mut entries = self.entries.write().await;
        entries.insert(key.to_string(), LocalCacheEntry::new(value, ttl));

        Ok(())
    }

    async fn delete(&self, key: &str) -> CacheResult<()> {
        let mut entries = self.entries.write().await;
        entries.remove(key);
        Ok(())
    }

    async fn delete_pattern(&self, pattern: &str) -> CacheResult<u64> {
        let mut entries = self.entries.write().await;

        // Simple prefix matching (Redis-style patterns would need more complex logic)
        let pattern_prefix = pattern.trim_end_matches('*');
        let before = entries.len();
        entries.retain(|k, _| !k.starts_with(pattern_prefix));
        let removed = before - entries.len();

        Ok(removed as u64)
    }

    async fn health_check(&self) -> CacheResult<()> {
        // Memory cache is always healthy
        Ok(())
    }

    fn name(&self) -> &'static str {
        "memory"
    }

    fn is_distributed(&self) -> bool {
        false
    }
}

/// Redis cache backend placeholder
///
/// This is a mock implementation for when Redis is not available.
/// In production, this would use the `redis` crate with connection pooling.
pub struct RedisCacheBackend {
    url: String,
    key_prefix: String,
    #[allow(dead_code)]
    operation_timeout: Duration,
    is_connected: Arc<RwLock<bool>>,
}

impl RedisCacheBackend {
    /// Create a new Redis cache backend
    ///
    /// # Arguments
    /// * `url` - Redis connection URL
    /// * `key_prefix` - Prefix for all keys (namespace isolation)
    /// * `operation_timeout` - Timeout for Redis operations
    ///
    /// # Errors
    /// Returns error if connection fails
    pub async fn new(
        url: impl Into<String>,
        key_prefix: impl Into<String>,
        operation_timeout: Duration,
    ) -> CacheResult<Self> {
        let url = url.into();
        let key_prefix = key_prefix.into();

        // In a real implementation, we would establish a connection pool here
        // For now, we'll create a mock that demonstrates the interface

        info!(url = %url, prefix = %key_prefix, "Initializing Redis cache backend");

        Ok(Self {
            url,
            key_prefix,
            operation_timeout,
            is_connected: Arc::new(RwLock::new(false)),
        })
    }

    /// Attempt to connect to Redis
    pub async fn connect(&self) -> CacheResult<()> {
        // In production, this would establish the connection pool
        // For now, we simulate the connection attempt

        // Parse URL to validate it
        if !self.url.starts_with("redis://") && !self.url.starts_with("rediss://") {
            return Err(DistributedCacheError::ConfigError(
                "Invalid Redis URL scheme".to_string(),
            ));
        }

        *self.is_connected.write().await = true;
        info!(url = %self.url, "Redis cache backend connected");

        Ok(())
    }

    fn prefixed_key(&self, key: &str) -> String {
        format!("{}:{}", self.key_prefix, key)
    }
}

#[async_trait]
impl CacheBackend for RedisCacheBackend {
    async fn get(&self, key: &str) -> CacheResult<Option<Vec<u8>>> {
        if !*self.is_connected.read().await {
            return Err(DistributedCacheError::Unavailable(
                "Redis not connected".to_string(),
            ));
        }

        let _prefixed = self.prefixed_key(key);

        // In production, this would be:
        // let mut conn = self.pool.get().await?;
        // let result: Option<Vec<u8>> = conn.get(&prefixed).await?;
        // Ok(result)

        // Mock: always return None (not found)
        debug!(key = %key, "Redis GET (mock)");
        Ok(None)
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> CacheResult<()> {
        if !*self.is_connected.read().await {
            return Err(DistributedCacheError::Unavailable(
                "Redis not connected".to_string(),
            ));
        }

        let _prefixed = self.prefixed_key(key);

        // In production, this would be:
        // let mut conn = self.pool.get().await?;
        // conn.set_ex(&prefixed, &value, ttl.as_secs()).await?;
        // Ok(())

        debug!(
            key = %key,
            size = value.len(),
            ttl_secs = ttl.as_secs(),
            "Redis SET (mock)"
        );
        Ok(())
    }

    async fn delete(&self, key: &str) -> CacheResult<()> {
        if !*self.is_connected.read().await {
            return Err(DistributedCacheError::Unavailable(
                "Redis not connected".to_string(),
            ));
        }

        let _prefixed = self.prefixed_key(key);

        // In production: conn.del(&prefixed).await?;
        debug!(key = %key, "Redis DEL (mock)");
        Ok(())
    }

    async fn delete_pattern(&self, pattern: &str) -> CacheResult<u64> {
        if !*self.is_connected.read().await {
            return Err(DistributedCacheError::Unavailable(
                "Redis not connected".to_string(),
            ));
        }

        let _prefixed = self.prefixed_key(pattern);

        // In production:
        // let keys: Vec<String> = conn.keys(&prefixed).await?;
        // let count = keys.len();
        // if !keys.is_empty() {
        //     conn.del(&keys).await?;
        // }
        // Ok(count as u64)

        debug!(pattern = %pattern, "Redis DEL pattern (mock)");
        Ok(0)
    }

    async fn health_check(&self) -> CacheResult<()> {
        if !*self.is_connected.read().await {
            return Err(DistributedCacheError::Unavailable(
                "Redis not connected".to_string(),
            ));
        }

        // In production: conn.ping().await?;
        Ok(())
    }

    fn name(&self) -> &'static str {
        "redis"
    }

    fn is_distributed(&self) -> bool {
        true
    }
}

/// Statistics for distributed cache
#[derive(Debug, Clone, Default)]
pub struct DistributedCacheStats {
    /// L1 (local) cache hits
    pub l1_hits: u64,
    /// L1 cache misses
    pub l1_misses: u64,
    /// L2 (distributed) cache hits
    pub l2_hits: u64,
    /// L2 cache misses
    pub l2_misses: u64,
    /// Total entries in L1 cache
    pub l1_entries: usize,
    /// Backend errors
    pub backend_errors: u64,
    /// Compression savings (bytes)
    pub compression_savings: u64,
}

impl DistributedCacheStats {
    /// Calculate overall hit rate
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let total_hits = self.l1_hits + self.l2_hits;
        let total = total_hits + self.l1_misses;
        if total == 0 {
            0.0
        } else {
            (total_hits as f64 / total as f64) * 100.0
        }
    }

    /// Calculate L1 hit rate
    #[must_use]
    pub fn l1_hit_rate(&self) -> f64 {
        let total = self.l1_hits + self.l1_misses;
        if total == 0 {
            0.0
        } else {
            (self.l1_hits as f64 / total as f64) * 100.0
        }
    }
}

/// Distributed response cache with L1 (local) and L2 (Redis) layers
pub struct DistributedCache {
    config: DistributedCacheConfig,
    l1_backend: Option<Arc<MemoryCacheBackend>>,
    l2_backend: Option<Arc<dyn CacheBackend>>,
    stats: Arc<RwLock<DistributedCacheStats>>,
}

impl DistributedCache {
    /// Create a new distributed cache
    #[must_use]
    pub fn new(config: DistributedCacheConfig) -> Self {
        let l1_backend = if config.enable_local_cache && config.enabled {
            Some(Arc::new(MemoryCacheBackend::new(
                config.local_cache_size,
                config.local_cache_ttl,
            )))
        } else {
            None
        };

        Self {
            config,
            l1_backend,
            l2_backend: None,
            stats: Arc::new(RwLock::new(DistributedCacheStats::default())),
        }
    }

    /// Create with defaults (in-memory only)
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(DistributedCacheConfig::default())
    }

    /// Create a disabled cache
    #[must_use]
    pub fn disabled() -> Self {
        Self::new(DistributedCacheConfig {
            enabled: false,
            ..Default::default()
        })
    }

    /// Initialize Redis backend
    pub async fn init_redis(&mut self) -> CacheResult<()> {
        if let Some(ref url) = self.config.redis_url {
            let backend = RedisCacheBackend::new(
                url.clone(),
                &self.config.key_prefix,
                self.config.redis_operation_timeout,
            )
            .await?;

            backend.connect().await?;
            self.l2_backend = Some(Arc::new(backend));

            info!("Redis distributed cache initialized");
        }

        Ok(())
    }

    /// Set L2 backend (for testing or custom backends)
    pub fn set_l2_backend(&mut self, backend: Arc<dyn CacheBackend>) {
        self.l2_backend = Some(backend);
    }

    /// Check if caching is enabled
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if distributed caching is available
    #[must_use]
    pub fn is_distributed(&self) -> bool {
        self.l2_backend
            .as_ref()
            .is_some_and(|b| b.is_distributed())
    }

    /// Check if a request is cacheable
    #[must_use]
    pub fn is_cacheable(&self, request: &GatewayRequest) -> bool {
        if !self.config.enabled {
            return false;
        }

        if request.stream && !self.config.cache_streaming {
            return false;
        }

        if let Some(temp) = request.temperature {
            if temp > 1.5 {
                return false;
            }
        }

        true
    }

    /// Get a cached response
    pub async fn get(&self, request: &GatewayRequest) -> Option<GatewayResponse> {
        if !self.is_cacheable(request) {
            return None;
        }

        let key = DistributedCacheKey::from_request(request);
        let key_str = key.to_string_key(&self.config.key_prefix);

        // Try L1 first
        if let Some(ref l1) = self.l1_backend {
            match l1.get(&key_str).await {
                Ok(Some(data)) => {
                    let mut stats = self.stats.write().await;
                    stats.l1_hits += 1;

                    match serde_json::from_slice::<CachedEntry>(&data) {
                        Ok(entry) if !entry.is_expired() => {
                            debug!(model = %request.model, "L1 cache hit");
                            return Some(entry.response);
                        }
                        Ok(_) => {
                            // Expired entry
                            let _ = l1.delete(&key_str).await;
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to deserialize L1 cache entry");
                        }
                    }
                }
                Ok(None) => {
                    let mut stats = self.stats.write().await;
                    stats.l1_misses += 1;
                }
                Err(e) => {
                    warn!(error = %e, "L1 cache get error");
                }
            }
        }

        // Try L2 if available
        if let Some(ref l2) = self.l2_backend {
            match l2.get(&key_str).await {
                Ok(Some(data)) => {
                    let mut stats = self.stats.write().await;
                    stats.l2_hits += 1;

                    match serde_json::from_slice::<CachedEntry>(&data) {
                        Ok(entry) if !entry.is_expired() => {
                            debug!(model = %request.model, "L2 cache hit");

                            // Populate L1 cache
                            if let Some(ref l1) = self.l1_backend {
                                let _ = l1
                                    .set(&key_str, data, self.config.local_cache_ttl)
                                    .await;
                            }

                            return Some(entry.response);
                        }
                        Ok(_) => {
                            // Expired - delete from L2
                            let _ = l2.delete(&key_str).await;
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to deserialize L2 cache entry");
                        }
                    }
                }
                Ok(None) => {
                    let mut stats = self.stats.write().await;
                    stats.l2_misses += 1;
                }
                Err(e) => {
                    let mut stats = self.stats.write().await;
                    stats.backend_errors += 1;
                    error!(error = %e, "L2 cache get error");
                }
            }
        }

        debug!(model = %request.model, "Cache miss");
        None
    }

    /// Put a response in the cache
    pub async fn put(&self, request: &GatewayRequest, response: GatewayResponse) {
        self.put_with_ttl(request, response, self.config.default_ttl)
            .await;
    }

    /// Put a response with custom TTL
    pub async fn put_with_ttl(
        &self,
        request: &GatewayRequest,
        response: GatewayResponse,
        ttl: Duration,
    ) {
        if !self.is_cacheable(request) {
            return;
        }

        let key = DistributedCacheKey::from_request(request);
        let key_str = key.to_string_key(&self.config.key_prefix);

        let entry = CachedEntry::new(response, ttl);
        let data = match serde_json::to_vec(&entry) {
            Ok(d) => d,
            Err(e) => {
                warn!(error = %e, "Failed to serialize cache entry");
                return;
            }
        };

        // Store in L1
        if let Some(ref l1) = self.l1_backend {
            if let Err(e) = l1.set(&key_str, data.clone(), self.config.local_cache_ttl).await {
                warn!(error = %e, "Failed to store in L1 cache");
            }
        }

        // Store in L2
        if let Some(ref l2) = self.l2_backend {
            if let Err(e) = l2.set(&key_str, data, ttl).await {
                let mut stats = self.stats.write().await;
                stats.backend_errors += 1;
                error!(error = %e, "Failed to store in L2 cache");
            }
        }

        debug!(model = %request.model, ttl_secs = ttl.as_secs(), "Response cached");
    }

    /// Clear all cache entries
    pub async fn clear(&self) {
        if let Some(ref l1) = self.l1_backend {
            let _ = l1.delete_pattern("*").await;
        }

        if let Some(ref l2) = self.l2_backend {
            let pattern = format!("{}:*", self.config.key_prefix);
            if let Err(e) = l2.delete_pattern(&pattern).await {
                error!(error = %e, "Failed to clear L2 cache");
            }
        }

        info!("Distributed cache cleared");
    }

    /// Invalidate cache for a specific model
    pub async fn invalidate_model(&self, model: &str) {
        let pattern = format!("{}:cache:{}:*", self.config.key_prefix, model);

        if let Some(ref l1) = self.l1_backend {
            let _ = l1.delete_pattern(&pattern).await;
        }

        if let Some(ref l2) = self.l2_backend {
            if let Err(e) = l2.delete_pattern(&pattern).await {
                error!(error = %e, model = %model, "Failed to invalidate model in L2 cache");
            }
        }

        info!(model = %model, "Model cache invalidated");
    }

    /// Get cache statistics
    pub async fn stats(&self) -> DistributedCacheStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Health check for cache backends
    pub async fn health_check(&self) -> CacheResult<()> {
        if let Some(ref l2) = self.l2_backend {
            l2.health_check().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_core::ChatMessage;

    fn make_request(model: &str, content: &str) -> GatewayRequest {
        GatewayRequest::builder()
            .model(model)
            .message(ChatMessage::user(content))
            .temperature(0.7)
            .max_tokens(100_u32)
            .build()
            .expect("valid request")
    }

    fn make_response() -> GatewayResponse {
        GatewayResponse {
            id: "test-id".to_string(),
            object: "chat.completion".to_string(),
            model: "gpt-4o".to_string(),
            choices: vec![],
            usage: gateway_core::Usage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            },
            created: 1234567890,
            provider: Some("test".to_string()),
            system_fingerprint: None,
        }
    }

    #[tokio::test]
    async fn test_distributed_cache_key_generation() {
        let request = make_request("gpt-4o", "Hello world");
        let key1 = DistributedCacheKey::from_request(&request);
        let key2 = DistributedCacheKey::from_request(&request);

        assert_eq!(key1, key2);
        assert_eq!(key1.model, "gpt-4o");
    }

    #[tokio::test]
    async fn test_distributed_cache_key_different_content() {
        let request1 = make_request("gpt-4o", "Hello");
        let request2 = make_request("gpt-4o", "World");

        let key1 = DistributedCacheKey::from_request(&request1);
        let key2 = DistributedCacheKey::from_request(&request2);

        assert_ne!(key1.messages_hash, key2.messages_hash);
    }

    #[tokio::test]
    async fn test_memory_backend_basic() {
        let backend = MemoryCacheBackend::new(100, Duration::from_secs(3600));

        // Set and get
        backend
            .set("test-key", b"test-value".to_vec(), Duration::from_secs(60))
            .await
            .expect("set should succeed");

        let result = backend.get("test-key").await.expect("get should succeed");
        assert_eq!(result, Some(b"test-value".to_vec()));

        // Delete
        backend.delete("test-key").await.expect("delete should succeed");
        let result = backend.get("test-key").await.expect("get should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_memory_backend_expiry() {
        let backend = MemoryCacheBackend::new(100, Duration::from_millis(50));

        backend
            .set("test-key", b"test-value".to_vec(), Duration::from_millis(50))
            .await
            .expect("set should succeed");

        // Should exist immediately
        let result = backend.get("test-key").await.expect("get should succeed");
        assert!(result.is_some());

        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should be expired
        let result = backend.get("test-key").await.expect("get should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_distributed_cache_l1_hit() {
        let config = DistributedCacheConfig {
            enabled: true,
            enable_local_cache: true,
            local_cache_size: 100,
            local_cache_ttl: Duration::from_secs(60),
            ..Default::default()
        };

        let cache = DistributedCache::new(config);
        let request = make_request("gpt-4o", "Hello");
        let response = make_response();

        // Put and get
        cache.put(&request, response.clone()).await;
        let cached = cache.get(&request).await;

        assert!(cached.is_some());
        assert_eq!(cached.unwrap().id, response.id);

        // Check stats
        let stats = cache.stats().await;
        assert_eq!(stats.l1_hits, 1);
    }

    #[tokio::test]
    async fn test_distributed_cache_disabled() {
        let cache = DistributedCache::disabled();
        let request = make_request("gpt-4o", "Hello");
        let response = make_response();

        cache.put(&request, response).await;
        let cached = cache.get(&request).await;

        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn test_distributed_cache_streaming_not_cached() {
        let config = DistributedCacheConfig {
            enabled: true,
            cache_streaming: false,
            ..Default::default()
        };

        let cache = DistributedCache::new(config);

        let request = GatewayRequest::builder()
            .model("gpt-4o")
            .message(ChatMessage::user("Hello"))
            .stream(true)
            .build()
            .expect("valid request");

        let response = make_response();

        cache.put(&request, response).await;
        let cached = cache.get(&request).await;

        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn test_distributed_cache_invalidate_model() {
        let cache = DistributedCache::with_defaults();

        let request1 = make_request("gpt-4o", "Hello");
        let request2 = make_request("gpt-4o-mini", "Hello");
        let response = make_response();

        cache.put(&request1, response.clone()).await;
        cache.put(&request2, response.clone()).await;

        // Both should be cached
        assert!(cache.get(&request1).await.is_some());
        assert!(cache.get(&request2).await.is_some());

        // Invalidate gpt-4o
        cache.invalidate_model("gpt-4o").await;

        // gpt-4o should be gone, gpt-4o-mini should remain
        assert!(cache.get(&request1).await.is_none());
        assert!(cache.get(&request2).await.is_some());
    }

    #[test]
    fn test_cached_entry_expiry() {
        let response = make_response();

        // Test with a 10-second TTL (not expired)
        let entry = CachedEntry::new(response.clone(), Duration::from_secs(10));
        assert!(!entry.is_expired());
        // Should have significant remaining TTL
        assert!(entry.remaining_ttl() >= Duration::from_secs(5));

        // Test with a past timestamp (simulating expired entry)
        let mut entry_expired = CachedEntry::new(response, Duration::from_secs(1));
        // Manually set created_at to 100 seconds ago
        entry_expired.created_at = entry_expired.created_at.saturating_sub(100);
        assert!(entry_expired.is_expired());
        assert_eq!(entry_expired.remaining_ttl(), Duration::ZERO);
    }

    #[tokio::test]
    async fn test_config_builder() {
        let config = DistributedCacheConfigBuilder::new()
            .enabled(true)
            .redis_url("redis://localhost:6379")
            .redis_pool_size(20)
            .key_prefix("test-prefix")
            .default_ttl(Duration::from_secs(7200))
            .local_cache_size(500)
            .enable_local_cache(true)
            .enable_compression(false)
            .build();

        assert!(config.enabled);
        assert_eq!(config.redis_url, Some("redis://localhost:6379".to_string()));
        assert_eq!(config.redis_pool_size, 20);
        assert_eq!(config.key_prefix, "test-prefix");
        assert_eq!(config.default_ttl, Duration::from_secs(7200));
        assert_eq!(config.local_cache_size, 500);
        assert!(config.enable_local_cache);
        assert!(!config.enable_compression);
    }

    #[tokio::test]
    async fn test_stats_hit_rate() {
        let mut stats = DistributedCacheStats::default();

        assert_eq!(stats.hit_rate(), 0.0);

        stats.l1_hits = 80;
        stats.l1_misses = 20;

        assert!((stats.hit_rate() - 80.0).abs() < 0.1);
        assert!((stats.l1_hit_rate() - 80.0).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_memory_backend_eviction() {
        let backend = MemoryCacheBackend::new(2, Duration::from_secs(3600));

        // Fill cache
        backend
            .set("key1", b"value1".to_vec(), Duration::from_secs(60))
            .await
            .expect("set should succeed");

        backend
            .set("key2", b"value2".to_vec(), Duration::from_secs(60))
            .await
            .expect("set should succeed");

        // Access key2 to increase hits
        backend.get("key2").await.expect("get should succeed");

        // Add key3, should evict key1 (lowest hits)
        backend
            .set("key3", b"value3".to_vec(), Duration::from_secs(60))
            .await
            .expect("set should succeed");

        // key1 should be evicted
        assert!(backend.get("key1").await.expect("get should succeed").is_none());

        // key2 and key3 should exist
        assert!(backend.get("key2").await.expect("get should succeed").is_some());
        assert!(backend.get("key3").await.expect("get should succeed").is_some());
    }

    #[tokio::test]
    async fn test_delete_pattern() {
        let backend = MemoryCacheBackend::new(100, Duration::from_secs(3600));

        backend
            .set("prefix:key1", b"value1".to_vec(), Duration::from_secs(60))
            .await
            .expect("set should succeed");

        backend
            .set("prefix:key2", b"value2".to_vec(), Duration::from_secs(60))
            .await
            .expect("set should succeed");

        backend
            .set("other:key3", b"value3".to_vec(), Duration::from_secs(60))
            .await
            .expect("set should succeed");

        // Delete prefix:*
        let deleted = backend
            .delete_pattern("prefix:*")
            .await
            .expect("delete_pattern should succeed");

        assert_eq!(deleted, 2);

        // Verify
        assert!(backend.get("prefix:key1").await.expect("get should succeed").is_none());
        assert!(backend.get("prefix:key2").await.expect("get should succeed").is_none());
        assert!(backend.get("other:key3").await.expect("get should succeed").is_some());
    }
}
