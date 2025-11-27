// ============================================================================
// Provider Abstraction Layer - Advanced Features
// Circuit Breakers, Load Balancing, Caching, and Observability
// ============================================================================

use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use async_trait::async_trait;

// Re-use types from main implementation
use crate::{
    LLMProvider, GatewayRequest, GatewayResponse, ChatChunk,
    ProviderError, Result, HealthStatus,
};

// ============================================================================
// SECTION 1: Circuit Breaker Pattern
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    Closed,      // Normal operation
    Open,        // Failing, rejecting requests
    HalfOpen,    // Testing if service recovered
}

pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_threshold: u32,
    success_threshold: u32,
    timeout: Duration,
    failure_count: Arc<RwLock<u32>>,
    success_count: Arc<RwLock<u32>>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
}

impl CircuitBreaker {
    pub fn new(
        failure_threshold: u32,
        success_threshold: u32,
        timeout: Duration,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_threshold,
            success_threshold,
            timeout,
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn call<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // Check if circuit should transition from Open to HalfOpen
        self.check_state_transition().await;

        let current_state = *self.state.read().await;

        match current_state {
            CircuitState::Open => {
                Err(ProviderError::ProviderInternalError(
                    "Circuit breaker is open".to_string()
                ))
            }
            CircuitState::Closed | CircuitState::HalfOpen => {
                match operation().await {
                    Ok(result) => {
                        self.on_success().await;
                        Ok(result)
                    }
                    Err(e) => {
                        self.on_failure().await;
                        Err(e)
                    }
                }
            }
        }
    }

    async fn check_state_transition(&self) {
        let mut state = self.state.write().await;

        if *state == CircuitState::Open {
            let last_failure = self.last_failure_time.read().await;

            if let Some(last_fail) = *last_failure {
                if last_fail.elapsed() >= self.timeout {
                    // Transition to HalfOpen
                    *state = CircuitState::HalfOpen;
                    *self.success_count.write().await = 0;
                    *self.failure_count.write().await = 0;
                }
            }
        }
    }

    async fn on_success(&self) {
        let mut state = self.state.write().await;

        match *state {
            CircuitState::HalfOpen => {
                let mut success_count = self.success_count.write().await;
                *success_count += 1;

                if *success_count >= self.success_threshold {
                    // Transition to Closed
                    *state = CircuitState::Closed;
                    *self.failure_count.write().await = 0;
                    *self.success_count.write().await = 0;
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success
                *self.failure_count.write().await = 0;
            }
            _ => {}
        }
    }

    async fn on_failure(&self) {
        let mut state = self.state.write().await;
        let mut failure_count = self.failure_count.write().await;
        let mut last_failure_time = self.last_failure_time.write().await;

        *failure_count += 1;
        *last_failure_time = Some(Instant::now());

        match *state {
            CircuitState::Closed => {
                if *failure_count >= self.failure_threshold {
                    // Transition to Open
                    *state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in HalfOpen immediately reopens circuit
                *state = CircuitState::Open;
                *failure_count = 0;
                *self.success_count.write().await = 0;
            }
            _ => {}
        }
    }

    pub async fn get_state(&self) -> CircuitState {
        *self.state.read().await
    }
}

/// Provider wrapper with circuit breaker
pub struct CircuitBreakerProvider {
    inner: Arc<dyn LLMProvider>,
    circuit_breaker: CircuitBreaker,
}

impl CircuitBreakerProvider {
    pub fn new(provider: Arc<dyn LLMProvider>) -> Self {
        Self {
            inner: provider,
            circuit_breaker: CircuitBreaker::new(
                5,  // 5 failures
                2,  // 2 successes to recover
                Duration::from_secs(30), // 30 second timeout
            ),
        }
    }

    pub fn with_config(
        provider: Arc<dyn LLMProvider>,
        failure_threshold: u32,
        success_threshold: u32,
        timeout: Duration,
    ) -> Self {
        Self {
            inner: provider,
            circuit_breaker: CircuitBreaker::new(
                failure_threshold,
                success_threshold,
                timeout,
            ),
        }
    }
}

#[async_trait]
impl LLMProvider for CircuitBreakerProvider {
    fn provider_id(&self) -> &str {
        self.inner.provider_id()
    }

    fn capabilities(&self) -> &crate::ProviderCapabilities {
        self.inner.capabilities()
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        self.circuit_breaker.call(|| self.inner.health_check()).await
    }

    async fn chat_completion(&self, request: &GatewayRequest) -> Result<GatewayResponse> {
        self.circuit_breaker.call(|| self.inner.chat_completion(request)).await
    }

    async fn chat_completion_stream(
        &self,
        request: &GatewayRequest,
    ) -> Result<impl futures::stream::Stream<Item = Result<ChatChunk>> + Send> {
        self.circuit_breaker.call(|| self.inner.chat_completion_stream(request)).await
    }

    async fn check_rate_limit(&self) -> Option<Duration> {
        self.inner.check_rate_limit().await
    }

    fn transform_request(&self, request: &GatewayRequest) -> Result<bytes::Bytes> {
        self.inner.transform_request(request)
    }

    fn transform_response(&self, response: bytes::Bytes) -> Result<GatewayResponse> {
        self.inner.transform_response(response)
    }
}

// ============================================================================
// SECTION 2: Load Balancing Strategies
// ============================================================================

#[async_trait]
pub trait LoadBalancer: Send + Sync {
    /// Select next provider from pool
    async fn select(&self, providers: &[Arc<dyn LLMProvider>]) -> Option<Arc<dyn LLMProvider>>;

    /// Record result for adaptive balancing
    async fn record_result(&self, provider_id: &str, success: bool, latency: Duration);
}

/// Round-robin load balancer
pub struct RoundRobinBalancer {
    counter: Arc<Mutex<usize>>,
}

impl RoundRobinBalancer {
    pub fn new() -> Self {
        Self {
            counter: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl LoadBalancer for RoundRobinBalancer {
    async fn select(&self, providers: &[Arc<dyn LLMProvider>]) -> Option<Arc<dyn LLMProvider>> {
        if providers.is_empty() {
            return None;
        }

        let mut counter = self.counter.lock().await;
        let index = *counter % providers.len();
        *counter = (*counter + 1) % providers.len();

        Some(Arc::clone(&providers[index]))
    }

    async fn record_result(&self, _provider_id: &str, _success: bool, _latency: Duration) {
        // No-op for round-robin
    }
}

/// Weighted round-robin based on latency
pub struct LatencyWeightedBalancer {
    latencies: Arc<RwLock<HashMap<String, VecDeque<Duration>>>>,
    window_size: usize,
}

impl LatencyWeightedBalancer {
    pub fn new(window_size: usize) -> Self {
        Self {
            latencies: Arc::new(RwLock::new(HashMap::new())),
            window_size,
        }
    }

    async fn get_average_latency(&self, provider_id: &str) -> Option<Duration> {
        let latencies = self.latencies.read().await;

        latencies.get(provider_id).and_then(|queue| {
            if queue.is_empty() {
                None
            } else {
                let sum: Duration = queue.iter().sum();
                Some(sum / queue.len() as u32)
            }
        })
    }
}

#[async_trait]
impl LoadBalancer for LatencyWeightedBalancer {
    async fn select(&self, providers: &[Arc<dyn LLMProvider>]) -> Option<Arc<dyn LLMProvider>> {
        if providers.is_empty() {
            return None;
        }

        // Select provider with lowest average latency
        let mut best_provider: Option<Arc<dyn LLMProvider>> = None;
        let mut best_latency = Duration::MAX;

        for provider in providers {
            let avg_latency = self.get_average_latency(provider.provider_id())
                .await
                .unwrap_or(Duration::from_millis(100)); // Default for new providers

            if avg_latency < best_latency {
                best_latency = avg_latency;
                best_provider = Some(Arc::clone(provider));
            }
        }

        best_provider
    }

    async fn record_result(&self, provider_id: &str, _success: bool, latency: Duration) {
        let mut latencies = self.latencies.write().await;

        let queue = latencies.entry(provider_id.to_string())
            .or_insert_with(VecDeque::new);

        queue.push_back(latency);

        if queue.len() > self.window_size {
            queue.pop_front();
        }
    }
}

/// Least connections balancer
pub struct LeastConnectionsBalancer {
    active_connections: Arc<RwLock<HashMap<String, usize>>>,
}

impl LeastConnectionsBalancer {
    pub fn new() -> Self {
        Self {
            active_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl LoadBalancer for LeastConnectionsBalancer {
    async fn select(&self, providers: &[Arc<dyn LLMProvider>]) -> Option<Arc<dyn LLMProvider>> {
        if providers.is_empty() {
            return None;
        }

        let connections = self.active_connections.read().await;

        let mut best_provider: Option<Arc<dyn LLMProvider>> = None;
        let mut min_connections = usize::MAX;

        for provider in providers {
            let conn_count = connections.get(provider.provider_id())
                .copied()
                .unwrap_or(0);

            if conn_count < min_connections {
                min_connections = conn_count;
                best_provider = Some(Arc::clone(provider));
            }
        }

        // Increment connection count
        drop(connections);
        if let Some(ref provider) = best_provider {
            let mut connections = self.active_connections.write().await;
            *connections.entry(provider.provider_id().to_string())
                .or_insert(0) += 1;
        }

        best_provider
    }

    async fn record_result(&self, provider_id: &str, _success: bool, _latency: Duration) {
        // Decrement connection count
        let mut connections = self.active_connections.write().await;
        if let Some(count) = connections.get_mut(provider_id) {
            if *count > 0 {
                *count -= 1;
            }
        }
    }
}

/// Load balancing provider pool
pub struct LoadBalancedProvider {
    providers: Arc<RwLock<Vec<Arc<dyn LLMProvider>>>>,
    balancer: Arc<dyn LoadBalancer>,
}

impl LoadBalancedProvider {
    pub fn new(balancer: Arc<dyn LoadBalancer>) -> Self {
        Self {
            providers: Arc::new(RwLock::new(Vec::new())),
            balancer,
        }
    }

    pub async fn add_provider(&self, provider: Arc<dyn LLMProvider>) {
        let mut providers = self.providers.write().await;
        providers.push(provider);
    }

    pub async fn remove_provider(&self, provider_id: &str) {
        let mut providers = self.providers.write().await;
        providers.retain(|p| p.provider_id() != provider_id);
    }

    async fn select_provider(&self) -> Result<Arc<dyn LLMProvider>> {
        let providers = self.providers.read().await;

        self.balancer.select(&providers)
            .await
            .ok_or_else(|| ProviderError::NotFound("No available providers".to_string()))
    }
}

// ============================================================================
// SECTION 3: Response Caching
// ============================================================================

use std::hash::{Hash, Hasher};

pub struct CacheKey {
    model: String,
    messages_hash: u64,
    temperature: Option<u32>, // Store as integer (temp * 100)
    max_tokens: Option<u32>,
}

impl CacheKey {
    pub fn from_request(request: &GatewayRequest) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        // Hash messages
        for msg in &request.messages {
            format!("{:?}", msg.role).hash(&mut hasher);
            format!("{:?}", msg.content).hash(&mut hasher);
        }

        let messages_hash = hasher.finish();

        Self {
            model: request.model.clone(),
            messages_hash,
            temperature: request.temperature.map(|t| (t * 100.0) as u32),
            max_tokens: request.max_tokens,
        }
    }
}

impl Hash for CacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.model.hash(state);
        self.messages_hash.hash(state);
        self.temperature.hash(state);
        self.max_tokens.hash(state);
    }
}

impl PartialEq for CacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.model == other.model &&
        self.messages_hash == other.messages_hash &&
        self.temperature == other.temperature &&
        self.max_tokens == other.max_tokens
    }
}

impl Eq for CacheKey {}

pub struct CachedResponse {
    response: GatewayResponse,
    cached_at: Instant,
}

pub struct ResponseCache {
    cache: Arc<RwLock<HashMap<CacheKey, CachedResponse>>>,
    ttl: Duration,
    max_size: usize,
}

impl ResponseCache {
    pub fn new(ttl: Duration, max_size: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            ttl,
            max_size,
        }
    }

    pub async fn get(&self, key: &CacheKey) -> Option<GatewayResponse> {
        let cache = self.cache.read().await;

        cache.get(key).and_then(|cached| {
            if cached.cached_at.elapsed() < self.ttl {
                Some(cached.response.clone())
            } else {
                None
            }
        })
    }

    pub async fn put(&self, key: CacheKey, response: GatewayResponse) {
        let mut cache = self.cache.write().await;

        // Evict expired entries
        cache.retain(|_, v| v.cached_at.elapsed() < self.ttl);

        // Evict oldest if at capacity
        if cache.len() >= self.max_size {
            if let Some(oldest_key) = cache.iter()
                .min_by_key(|(_, v)| v.cached_at)
                .map(|(k, _)| k.clone())
            {
                cache.remove(&oldest_key);
            }
        }

        cache.insert(key, CachedResponse {
            response,
            cached_at: Instant::now(),
        });
    }

    pub async fn invalidate(&self, key: &CacheKey) {
        let mut cache = self.cache.write().await;
        cache.remove(key);
    }

    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    pub async fn size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }
}

/// Provider wrapper with caching
pub struct CachedProvider {
    inner: Arc<dyn LLMProvider>,
    cache: Arc<ResponseCache>,
}

impl CachedProvider {
    pub fn new(provider: Arc<dyn LLMProvider>, cache: Arc<ResponseCache>) -> Self {
        Self {
            inner: provider,
            cache,
        }
    }
}

#[async_trait]
impl LLMProvider for CachedProvider {
    fn provider_id(&self) -> &str {
        self.inner.provider_id()
    }

    fn capabilities(&self) -> &crate::ProviderCapabilities {
        self.inner.capabilities()
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        self.inner.health_check().await
    }

    async fn chat_completion(&self, request: &GatewayRequest) -> Result<GatewayResponse> {
        // Don't cache streaming requests
        if request.stream {
            return self.inner.chat_completion(request).await;
        }

        // Check cache
        let cache_key = CacheKey::from_request(request);

        if let Some(cached_response) = self.cache.get(&cache_key).await {
            return Ok(cached_response);
        }

        // Cache miss - fetch from provider
        let response = self.inner.chat_completion(request).await?;

        // Store in cache
        self.cache.put(cache_key, response.clone()).await;

        Ok(response)
    }

    async fn chat_completion_stream(
        &self,
        request: &GatewayRequest,
    ) -> Result<impl futures::stream::Stream<Item = Result<ChatChunk>> + Send> {
        // No caching for streaming
        self.inner.chat_completion_stream(request).await
    }

    async fn check_rate_limit(&self) -> Option<Duration> {
        self.inner.check_rate_limit().await
    }

    fn transform_request(&self, request: &GatewayRequest) -> Result<bytes::Bytes> {
        self.inner.transform_request(request)
    }

    fn transform_response(&self, response: bytes::Bytes) -> Result<GatewayResponse> {
        self.inner.transform_response(response)
    }
}

// ============================================================================
// SECTION 4: Observability and Metrics
// ============================================================================

use prometheus::{
    Counter, Histogram, IntGauge, Registry,
    HistogramOpts, Opts,
};

pub struct ProviderMetrics {
    // Request counters
    pub requests_total: Counter,
    pub requests_success: Counter,
    pub requests_failure: Counter,

    // Latency histograms
    pub request_duration: Histogram,

    // Token usage
    pub tokens_prompt: Counter,
    pub tokens_completion: Counter,

    // Active connections
    pub active_connections: IntGauge,

    // Circuit breaker state
    pub circuit_breaker_state: IntGauge,

    // Cache metrics
    pub cache_hits: Counter,
    pub cache_misses: Counter,
}

impl ProviderMetrics {
    pub fn new(provider_id: &str, registry: &Registry) -> Result<Self> {
        let requests_total = Counter::with_opts(
            Opts::new("llm_requests_total", "Total number of requests")
                .const_label("provider", provider_id)
        ).map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;

        let requests_success = Counter::with_opts(
            Opts::new("llm_requests_success", "Successful requests")
                .const_label("provider", provider_id)
        ).map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;

        let requests_failure = Counter::with_opts(
            Opts::new("llm_requests_failure", "Failed requests")
                .const_label("provider", provider_id)
        ).map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;

        let request_duration = Histogram::with_opts(
            HistogramOpts::new("llm_request_duration_seconds", "Request duration")
                .const_label("provider", provider_id)
                .buckets(vec![0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0])
        ).map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;

        let tokens_prompt = Counter::with_opts(
            Opts::new("llm_tokens_prompt", "Prompt tokens used")
                .const_label("provider", provider_id)
        ).map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;

        let tokens_completion = Counter::with_opts(
            Opts::new("llm_tokens_completion", "Completion tokens used")
                .const_label("provider", provider_id)
        ).map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;

        let active_connections = IntGauge::with_opts(
            Opts::new("llm_active_connections", "Active connections")
                .const_label("provider", provider_id)
        ).map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;

        let circuit_breaker_state = IntGauge::with_opts(
            Opts::new("llm_circuit_breaker_state", "Circuit breaker state (0=closed, 1=half-open, 2=open)")
                .const_label("provider", provider_id)
        ).map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;

        let cache_hits = Counter::with_opts(
            Opts::new("llm_cache_hits", "Cache hits")
                .const_label("provider", provider_id)
        ).map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;

        let cache_misses = Counter::with_opts(
            Opts::new("llm_cache_misses", "Cache misses")
                .const_label("provider", provider_id)
        ).map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;

        // Register all metrics
        registry.register(Box::new(requests_total.clone()))
            .map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;
        registry.register(Box::new(requests_success.clone()))
            .map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;
        registry.register(Box::new(requests_failure.clone()))
            .map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;
        registry.register(Box::new(request_duration.clone()))
            .map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;
        registry.register(Box::new(tokens_prompt.clone()))
            .map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;
        registry.register(Box::new(tokens_completion.clone()))
            .map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;
        registry.register(Box::new(active_connections.clone()))
            .map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;
        registry.register(Box::new(circuit_breaker_state.clone()))
            .map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;
        registry.register(Box::new(cache_hits.clone()))
            .map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;
        registry.register(Box::new(cache_misses.clone()))
            .map_err(|e| ProviderError::ProviderInternalError(e.to_string()))?;

        Ok(Self {
            requests_total,
            requests_success,
            requests_failure,
            request_duration,
            tokens_prompt,
            tokens_completion,
            active_connections,
            circuit_breaker_state,
            cache_hits,
            cache_misses,
        })
    }
}

/// Provider wrapper with metrics collection
pub struct ObservableProvider {
    inner: Arc<dyn LLMProvider>,
    metrics: Arc<ProviderMetrics>,
}

impl ObservableProvider {
    pub fn new(provider: Arc<dyn LLMProvider>, metrics: Arc<ProviderMetrics>) -> Self {
        Self {
            inner: provider,
            metrics,
        }
    }
}

#[async_trait]
impl LLMProvider for ObservableProvider {
    fn provider_id(&self) -> &str {
        self.inner.provider_id()
    }

    fn capabilities(&self) -> &crate::ProviderCapabilities {
        self.inner.capabilities()
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        self.inner.health_check().await
    }

    async fn chat_completion(&self, request: &GatewayRequest) -> Result<GatewayResponse> {
        self.metrics.requests_total.inc();
        self.metrics.active_connections.inc();

        let start = Instant::now();

        let result = self.inner.chat_completion(request).await;

        let duration = start.elapsed();
        self.metrics.request_duration.observe(duration.as_secs_f64());
        self.metrics.active_connections.dec();

        match &result {
            Ok(response) => {
                self.metrics.requests_success.inc();
                self.metrics.tokens_prompt.inc_by(response.usage.prompt_tokens as f64);
                self.metrics.tokens_completion.inc_by(response.usage.completion_tokens as f64);
            }
            Err(_) => {
                self.metrics.requests_failure.inc();
            }
        }

        result
    }

    async fn chat_completion_stream(
        &self,
        request: &GatewayRequest,
    ) -> Result<impl futures::stream::Stream<Item = Result<ChatChunk>> + Send> {
        self.metrics.requests_total.inc();
        self.inner.chat_completion_stream(request).await
    }

    async fn check_rate_limit(&self) -> Option<Duration> {
        self.inner.check_rate_limit().await
    }

    fn transform_request(&self, request: &GatewayRequest) -> Result<bytes::Bytes> {
        self.inner.transform_request(request)
    }

    fn transform_response(&self, response: bytes::Bytes) -> Result<GatewayResponse> {
        self.inner.transform_response(response)
    }
}

// ============================================================================
// SECTION 5: Request/Response Logging and Tracing
// ============================================================================

use tracing::{info, warn, error, instrument, Span};
use opentelemetry::trace::{TraceContextExt, Tracer};

pub struct TracedProvider {
    inner: Arc<dyn LLMProvider>,
}

impl TracedProvider {
    pub fn new(provider: Arc<dyn LLMProvider>) -> Self {
        Self { inner: provider }
    }
}

#[async_trait]
impl LLMProvider for TracedProvider {
    fn provider_id(&self) -> &str {
        self.inner.provider_id()
    }

    fn capabilities(&self) -> &crate::ProviderCapabilities {
        self.inner.capabilities()
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        self.inner.health_check().await
    }

    #[instrument(skip(self, request), fields(
        provider = %self.provider_id(),
        model = %request.model,
        request_id = %request.request_id
    ))]
    async fn chat_completion(&self, request: &GatewayRequest) -> Result<GatewayResponse> {
        let span = Span::current();

        span.record("message_count", request.messages.len());
        span.record("stream", request.stream);

        info!(
            provider = %self.provider_id(),
            model = %request.model,
            "Starting chat completion request"
        );

        let start = Instant::now();

        let result = self.inner.chat_completion(request).await;

        let duration = start.elapsed();

        match &result {
            Ok(response) => {
                info!(
                    provider = %self.provider_id(),
                    model = %request.model,
                    duration_ms = duration.as_millis(),
                    prompt_tokens = response.usage.prompt_tokens,
                    completion_tokens = response.usage.completion_tokens,
                    "Chat completion succeeded"
                );

                span.record("prompt_tokens", response.usage.prompt_tokens);
                span.record("completion_tokens", response.usage.completion_tokens);
            }
            Err(e) => {
                error!(
                    provider = %self.provider_id(),
                    model = %request.model,
                    duration_ms = duration.as_millis(),
                    error = %e,
                    "Chat completion failed"
                );
            }
        }

        result
    }

    async fn chat_completion_stream(
        &self,
        request: &GatewayRequest,
    ) -> Result<impl futures::stream::Stream<Item = Result<ChatChunk>> + Send> {
        info!(
            provider = %self.provider_id(),
            model = %request.model,
            "Starting streaming chat completion"
        );

        self.inner.chat_completion_stream(request).await
    }

    async fn check_rate_limit(&self) -> Option<Duration> {
        self.inner.check_rate_limit().await
    }

    fn transform_request(&self, request: &GatewayRequest) -> Result<bytes::Bytes> {
        self.inner.transform_request(request)
    }

    fn transform_response(&self, response: bytes::Bytes) -> Result<GatewayResponse> {
        self.inner.transform_response(response)
    }
}

// ============================================================================
// SECTION 6: Fallback and Retry Strategies
// ============================================================================

pub struct FallbackProvider {
    primary: Arc<dyn LLMProvider>,
    fallbacks: Vec<Arc<dyn LLMProvider>>,
}

impl FallbackProvider {
    pub fn new(primary: Arc<dyn LLMProvider>) -> Self {
        Self {
            primary,
            fallbacks: Vec::new(),
        }
    }

    pub fn add_fallback(mut self, provider: Arc<dyn LLMProvider>) -> Self {
        self.fallbacks.push(provider);
        self
    }
}

#[async_trait]
impl LLMProvider for FallbackProvider {
    fn provider_id(&self) -> &str {
        self.primary.provider_id()
    }

    fn capabilities(&self) -> &crate::ProviderCapabilities {
        self.primary.capabilities()
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        self.primary.health_check().await
    }

    async fn chat_completion(&self, request: &GatewayRequest) -> Result<GatewayResponse> {
        // Try primary first
        match self.primary.chat_completion(request).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                warn!(
                    "Primary provider {} failed: {}",
                    self.primary.provider_id(),
                    e
                );
            }
        }

        // Try fallbacks in order
        for fallback in &self.fallbacks {
            match fallback.chat_completion(request).await {
                Ok(response) => {
                    info!(
                        "Fallback provider {} succeeded",
                        fallback.provider_id()
                    );
                    return Ok(response);
                }
                Err(e) => {
                    warn!(
                        "Fallback provider {} failed: {}",
                        fallback.provider_id(),
                        e
                    );
                }
            }
        }

        Err(ProviderError::ProviderInternalError(
            "All providers failed".to_string()
        ))
    }

    async fn chat_completion_stream(
        &self,
        request: &GatewayRequest,
    ) -> Result<impl futures::stream::Stream<Item = Result<ChatChunk>> + Send> {
        // Try primary first
        if let Ok(stream) = self.primary.chat_completion_stream(request).await {
            return Ok(stream);
        }

        // Try fallbacks
        for fallback in &self.fallbacks {
            if let Ok(stream) = fallback.chat_completion_stream(request).await {
                return Ok(stream);
            }
        }

        Err(ProviderError::ProviderInternalError(
            "All providers failed for streaming".to_string()
        ))
    }

    async fn check_rate_limit(&self) -> Option<Duration> {
        self.primary.check_rate_limit().await
    }

    fn transform_request(&self, request: &GatewayRequest) -> Result<bytes::Bytes> {
        self.primary.transform_request(request)
    }

    fn transform_response(&self, response: bytes::Bytes) -> Result<GatewayResponse> {
        self.primary.transform_response(response)
    }
}

// ============================================================================
// SECTION 7: Complete Provider Stack Builder
// ============================================================================

pub struct ProviderStackBuilder {
    provider: Arc<dyn LLMProvider>,
}

impl ProviderStackBuilder {
    pub fn new(provider: Arc<dyn LLMProvider>) -> Self {
        Self { provider }
    }

    pub fn with_circuit_breaker(self) -> Self {
        Self {
            provider: Arc::new(CircuitBreakerProvider::new(self.provider)),
        }
    }

    pub fn with_cache(self, cache: Arc<ResponseCache>) -> Self {
        Self {
            provider: Arc::new(CachedProvider::new(self.provider, cache)),
        }
    }

    pub fn with_metrics(self, metrics: Arc<ProviderMetrics>) -> Self {
        Self {
            provider: Arc::new(ObservableProvider::new(self.provider, metrics)),
        }
    }

    pub fn with_tracing(self) -> Self {
        Self {
            provider: Arc::new(TracedProvider::new(self.provider)),
        }
    }

    pub fn build(self) -> Arc<dyn LLMProvider> {
        self.provider
    }
}

// Usage example:
/*
async fn create_production_provider() -> Arc<dyn LLMProvider> {
    let base_provider = create_openai_provider();
    let cache = Arc::new(ResponseCache::new(Duration::from_secs(300), 1000));
    let metrics = Arc::new(ProviderMetrics::new("openai", &Registry::new()).unwrap());

    ProviderStackBuilder::new(base_provider)
        .with_tracing()
        .with_circuit_breaker()
        .with_cache(cache)
        .with_metrics(metrics)
        .build()
}
*/
