// ============================================================================
// LLM Inference Gateway - Provider Abstraction Layer
// Comprehensive Rust Pseudocode for Production Implementation
// ============================================================================

use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use async_trait::async_trait;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use bytes::Bytes;
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;

// ============================================================================
// SECTION 1: Core Error Types
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Provider not found: {0}")]
    NotFound(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Provider timeout: {0}")]
    Timeout(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Provider internal error: {0}")]
    ProviderInternalError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Unsupported capability: {0}")]
    UnsupportedCapability(String),
}

pub type Result<T> = std::result::Result<T, ProviderError>;

// ============================================================================
// SECTION 2: Unified Request/Response Types
// ============================================================================

/// Unified chat request format across all providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayRequest {
    /// Unique request ID for tracing
    pub request_id: String,

    /// Model identifier (provider-agnostic or provider-specific)
    pub model: String,

    /// Conversation messages
    pub messages: Vec<Message>,

    /// Sampling parameters
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub stop_sequences: Option<Vec<String>>,

    /// Streaming control
    pub stream: bool,

    /// System prompt (for providers that support it)
    pub system: Option<String>,

    /// Tool/Function calling support
    pub tools: Option<Vec<Tool>>,
    pub tool_choice: Option<ToolChoice>,

    /// Provider-specific metadata
    pub metadata: HashMap<String, serde_json::Value>,

    /// Timeout override
    pub timeout: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: MessageContent,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    MultiModal(Vec<ContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    Image {
        source: ImageSource,
        detail: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    Url { url: String },
    Base64 {
        media_type: String,
        data: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    Auto,
    Required,
    None,
    Specific { name: String },
}

/// Unified response format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayResponse {
    pub request_id: String,
    pub provider: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
    pub created_at: u64,
    pub finish_reason: FinishReason,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: Option<FinishReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Streaming chunk format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChunk {
    pub request_id: String,
    pub provider: String,
    pub model: String,
    pub delta: Delta,
    pub finish_reason: Option<FinishReason>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    pub role: Option<MessageRole>,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

// ============================================================================
// SECTION 3: Provider Capabilities
// ============================================================================

#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    /// Provider supports streaming responses
    pub supports_streaming: bool,

    /// Provider supports function/tool calling
    pub supports_tools: bool,

    /// Provider supports multimodal inputs (images, etc.)
    pub supports_multimodal: bool,

    /// Provider supports system messages
    pub supports_system_messages: bool,

    /// Maximum context window size
    pub max_context_tokens: u32,

    /// Maximum output tokens
    pub max_output_tokens: u32,

    /// Supported models
    pub models: Vec<String>,

    /// Rate limit information
    pub rate_limits: RateLimitInfo,
}

#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    pub requests_per_minute: Option<u32>,
    pub tokens_per_minute: Option<u32>,
    pub concurrent_requests: Option<u32>,
}

/// Health status for provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub is_healthy: bool,
    pub latency_ms: Option<u64>,
    pub error_rate: f32,
    pub last_check: Instant,
    pub details: HashMap<String, String>,
}

// ============================================================================
// SECTION 4: Core Provider Trait
// ============================================================================

#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Get provider identifier
    fn provider_id(&self) -> &str;

    /// Get provider capabilities
    fn capabilities(&self) -> &ProviderCapabilities;

    /// Perform health check
    async fn health_check(&self) -> Result<HealthStatus>;

    /// Non-streaming chat completion
    async fn chat_completion(&self, request: &GatewayRequest) -> Result<GatewayResponse>;

    /// Streaming chat completion
    async fn chat_completion_stream(
        &self,
        request: &GatewayRequest,
    ) -> Result<impl Stream<Item = Result<ChatChunk>> + Send>;

    /// Validate request before sending
    fn validate_request(&self, request: &GatewayRequest) -> Result<()> {
        // Default implementation
        if request.messages.is_empty() {
            return Err(ProviderError::InvalidRequest(
                "Messages cannot be empty".to_string()
            ));
        }

        if let Some(temp) = request.temperature {
            if temp < 0.0 || temp > 2.0 {
                return Err(ProviderError::InvalidRequest(
                    "Temperature must be between 0 and 2".to_string()
                ));
            }
        }

        // Check tool support
        if request.tools.is_some() && !self.capabilities().supports_tools {
            return Err(ProviderError::UnsupportedCapability(
                "Provider does not support tools".to_string()
            ));
        }

        Ok(())
    }

    /// Transform unified request to provider-specific format
    fn transform_request(&self, request: &GatewayRequest) -> Result<Bytes>;

    /// Transform provider response to unified format
    fn transform_response(&self, response: Bytes) -> Result<GatewayResponse>;

    /// Handle rate limiting (returns wait duration if rate limited)
    async fn check_rate_limit(&self) -> Option<Duration>;

    /// Initialize provider (async setup)
    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    /// Shutdown provider gracefully
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// SECTION 5: Provider Registry
// ============================================================================

pub struct ProviderRegistry {
    /// Thread-safe provider storage
    providers: Arc<RwLock<HashMap<String, Arc<dyn LLMProvider>>>>,

    /// Health check interval
    health_check_interval: Duration,

    /// Cached health status
    health_cache: Arc<RwLock<HashMap<String, HealthStatus>>>,
}

impl ProviderRegistry {
    pub fn new(health_check_interval: Duration) -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            health_check_interval,
            health_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new provider
    pub async fn register(&self, provider: Arc<dyn LLMProvider>) -> Result<()> {
        let provider_id = provider.provider_id().to_string();

        // Validate provider by running health check
        let health = provider.health_check().await?;

        if !health.is_healthy {
            return Err(ProviderError::ProviderInternalError(
                format!("Provider {} is unhealthy", provider_id)
            ));
        }

        // Store provider
        let mut providers = self.providers.write().await;
        providers.insert(provider_id.clone(), provider);

        // Cache health status
        let mut health_cache = self.health_cache.write().await;
        health_cache.insert(provider_id, health);

        Ok(())
    }

    /// Deregister a provider
    pub async fn deregister(&self, provider_id: &str) -> Result<()> {
        let mut providers = self.providers.write().await;

        if let Some(provider) = providers.remove(provider_id) {
            // Graceful shutdown
            provider.shutdown().await?;

            // Remove health cache
            let mut health_cache = self.health_cache.write().await;
            health_cache.remove(provider_id);

            Ok(())
        } else {
            Err(ProviderError::NotFound(provider_id.to_string()))
        }
    }

    /// Get provider by ID
    pub async fn get(&self, provider_id: &str) -> Option<Arc<dyn LLMProvider>> {
        let providers = self.providers.read().await;
        providers.get(provider_id).cloned()
    }

    /// List all providers
    pub async fn list_all(&self) -> Vec<(String, Arc<dyn LLMProvider>)> {
        let providers = self.providers.read().await;
        providers
            .iter()
            .map(|(id, provider)| (id.clone(), Arc::clone(provider)))
            .collect()
    }

    /// List only healthy providers
    pub async fn list_healthy(&self) -> Vec<Arc<dyn LLMProvider>> {
        let providers = self.providers.read().await;
        let health_cache = self.health_cache.read().await;

        providers
            .iter()
            .filter(|(id, _)| {
                health_cache
                    .get(*id)
                    .map(|h| h.is_healthy)
                    .unwrap_or(false)
            })
            .map(|(_, provider)| Arc::clone(provider))
            .collect()
    }

    /// Get providers supporting specific capability
    pub async fn get_providers_with_capability<F>(&self, check: F) -> Vec<Arc<dyn LLMProvider>>
    where
        F: Fn(&ProviderCapabilities) -> bool,
    {
        let providers = self.providers.read().await;

        providers
            .values()
            .filter(|provider| check(provider.capabilities()))
            .map(|provider| Arc::clone(provider))
            .collect()
    }

    /// Background health check task
    pub async fn start_health_checks(&self) {
        let providers = Arc::clone(&self.providers);
        let health_cache = Arc::clone(&self.health_cache);
        let interval = self.health_check_interval;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                ticker.tick().await;

                let provider_list = {
                    let providers = providers.read().await;
                    providers.clone()
                };

                // Run health checks in parallel
                let health_futures = provider_list.iter().map(|(id, provider)| {
                    let id = id.clone();
                    let provider = Arc::clone(provider);

                    async move {
                        match provider.health_check().await {
                            Ok(health) => Some((id, health)),
                            Err(e) => {
                                eprintln!("Health check failed for {}: {}", id, e);
                                None
                            }
                        }
                    }
                });

                let results = futures::future::join_all(health_futures).await;

                // Update health cache
                let mut cache = health_cache.write().await;
                for result in results.into_iter().flatten() {
                    cache.insert(result.0, result.1);
                }
            }
        });
    }
}

// ============================================================================
// SECTION 6: Connection Pool Management
// ============================================================================

pub struct ConnectionPool {
    /// HTTP client with connection pooling
    client: hyper::Client<HttpsConnector<HttpConnector>>,

    /// Per-provider connection limits
    connection_limits: Arc<RwLock<HashMap<String, Arc<Semaphore>>>>,

    /// Configuration
    config: ConnectionPoolConfig,
}

#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Maximum idle connections per host
    pub max_idle_per_host: usize,

    /// Idle connection timeout
    pub idle_timeout: Duration,

    /// Connection timeout
    pub connect_timeout: Duration,

    /// Maximum concurrent connections per provider
    pub max_connections_per_provider: usize,

    /// Keep-alive duration
    pub keep_alive: Duration,

    /// Enable HTTP/2
    pub http2_only: bool,

    /// TCP nodelay
    pub tcp_nodelay: bool,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_idle_per_host: 32,
            idle_timeout: Duration::from_secs(90),
            connect_timeout: Duration::from_secs(10),
            max_connections_per_provider: 100,
            keep_alive: Duration::from_secs(60),
            http2_only: true,
            tcp_nodelay: true,
        }
    }
}

impl ConnectionPool {
    pub fn new(config: ConnectionPoolConfig) -> Self {
        // Configure TLS
        let mut http_connector = HttpConnector::new();
        http_connector.set_connect_timeout(Some(config.connect_timeout));
        http_connector.set_nodelay(config.tcp_nodelay);
        http_connector.set_keepalive(Some(config.keep_alive));

        let https_connector = HttpsConnector::new_with_connector(http_connector);

        // Build HTTP client with connection pooling
        let client = hyper::Client::builder()
            .pool_idle_timeout(config.idle_timeout)
            .pool_max_idle_per_host(config.max_idle_per_host)
            .http2_only(config.http2_only)
            .build(https_connector);

        Self {
            client,
            connection_limits: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Get HTTP client
    pub fn client(&self) -> &hyper::Client<HttpsConnector<HttpConnector>> {
        &self.client
    }

    /// Acquire connection permit for provider
    pub async fn acquire_permit(&self, provider_id: &str) -> Result<ConnectionPermit> {
        let semaphore = {
            let mut limits = self.connection_limits.write().await;
            limits
                .entry(provider_id.to_string())
                .or_insert_with(|| {
                    Arc::new(Semaphore::new(self.config.max_connections_per_provider))
                })
                .clone()
        };

        let permit = semaphore
            .acquire()
            .await
            .map_err(|e| ProviderError::NetworkError(format!("Failed to acquire permit: {}", e)))?;

        Ok(ConnectionPermit {
            _permit: permit,
            acquired_at: Instant::now(),
        })
    }

    /// Get pool statistics
    pub async fn stats(&self, provider_id: &str) -> PoolStats {
        let limits = self.connection_limits.read().await;

        let available_permits = limits
            .get(provider_id)
            .map(|sem| sem.available_permits())
            .unwrap_or(self.config.max_connections_per_provider);

        PoolStats {
            max_connections: self.config.max_connections_per_provider,
            available_connections: available_permits,
            active_connections: self.config.max_connections_per_provider - available_permits,
        }
    }
}

/// Connection permit - automatically released on drop
pub struct ConnectionPermit {
    _permit: tokio::sync::SemaphorePermit<'static>,
    acquired_at: Instant,
}

#[derive(Debug, Clone)]
pub struct PoolStats {
    pub max_connections: usize,
    pub available_connections: usize,
    pub active_connections: usize,
}

// ============================================================================
// SECTION 7: Rate Limiter
// ============================================================================

pub struct RateLimiter {
    /// Token bucket for requests
    request_tokens: Arc<RwLock<TokenBucket>>,

    /// Token bucket for input tokens
    input_tokens: Arc<RwLock<TokenBucket>>,

    /// Configuration
    config: RateLimitConfig,
}

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub requests_per_minute: Option<u32>,
    pub tokens_per_minute: Option<u32>,
}

struct TokenBucket {
    capacity: f64,
    tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: u32, refill_per_minute: u32) -> Self {
        let capacity = capacity as f64;
        Self {
            capacity,
            tokens: capacity,
            refill_rate: refill_per_minute as f64 / 60.0,
            last_refill: Instant::now(),
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        let new_tokens = elapsed * self.refill_rate;
        self.tokens = (self.tokens + new_tokens).min(self.capacity);
        self.last_refill = now;
    }

    fn try_consume(&mut self, amount: f64) -> Option<Duration> {
        self.refill();

        if self.tokens >= amount {
            self.tokens -= amount;
            None
        } else {
            // Calculate wait time
            let tokens_needed = amount - self.tokens;
            let wait_secs = tokens_needed / self.refill_rate;
            Some(Duration::from_secs_f64(wait_secs))
        }
    }
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        let request_tokens = Arc::new(RwLock::new(TokenBucket::new(
            config.requests_per_minute.unwrap_or(1000),
            config.requests_per_minute.unwrap_or(1000),
        )));

        let input_tokens = Arc::new(RwLock::new(TokenBucket::new(
            config.tokens_per_minute.unwrap_or(100_000),
            config.tokens_per_minute.unwrap_or(100_000),
        )));

        Self {
            request_tokens,
            input_tokens,
            config,
        }
    }

    pub async fn check_and_consume(&self, estimated_tokens: u32) -> Option<Duration> {
        // Check request rate limit
        if let Some(wait) = self.request_tokens.write().await.try_consume(1.0) {
            return Some(wait);
        }

        // Check token rate limit
        if let Some(wait) = self.input_tokens.write().await.try_consume(estimated_tokens as f64) {
            return Some(wait);
        }

        None
    }
}

// ============================================================================
// SECTION 8: OpenAI Provider Implementation
// ============================================================================

pub struct OpenAIProvider {
    provider_id: String,
    client: Arc<ConnectionPool>,
    config: OpenAIConfig,
    capabilities: ProviderCapabilities,
    rate_limiter: RateLimiter,
    metrics: Arc<RwLock<ProviderMetrics>>,
}

#[derive(Debug, Clone)]
pub struct OpenAIConfig {
    pub api_key: String,
    pub base_url: String,
    pub organization: Option<String>,
    pub timeout: Duration,
    pub retry_config: RetryConfig,
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            backoff_multiplier: 2.0,
        }
    }
}

struct ProviderMetrics {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    total_latency_ms: u64,
    last_error: Option<String>,
    last_success: Option<Instant>,
}

impl OpenAIProvider {
    pub fn new(config: OpenAIConfig, connection_pool: Arc<ConnectionPool>) -> Self {
        let capabilities = ProviderCapabilities {
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: true,
            supports_system_messages: true,
            max_context_tokens: 128_000, // GPT-4 Turbo
            max_output_tokens: 4_096,
            models: vec![
                "gpt-4-turbo".to_string(),
                "gpt-4".to_string(),
                "gpt-3.5-turbo".to_string(),
            ],
            rate_limits: RateLimitInfo {
                requests_per_minute: Some(500),
                tokens_per_minute: Some(150_000),
                concurrent_requests: Some(100),
            },
        };

        let rate_limiter = RateLimiter::new(RateLimitConfig {
            requests_per_minute: Some(500),
            tokens_per_minute: Some(150_000),
        });

        Self {
            provider_id: "openai".to_string(),
            client: connection_pool,
            config,
            capabilities,
            rate_limiter,
            metrics: Arc::new(RwLock::new(ProviderMetrics {
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                total_latency_ms: 0,
                last_error: None,
                last_success: None,
            })),
        }
    }

    async fn execute_with_retry<F, Fut, T>(&self, mut operation: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut backoff = self.config.retry_config.initial_backoff;

        for attempt in 0..=self.config.retry_config.max_retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempt == self.config.retry_config.max_retries {
                        return Err(e);
                    }

                    // Check if error is retryable
                    if !self.is_retryable_error(&e) {
                        return Err(e);
                    }

                    // Exponential backoff
                    tokio::time::sleep(backoff).await;
                    backoff = std::cmp::min(
                        Duration::from_secs_f64(
                            backoff.as_secs_f64() * self.config.retry_config.backoff_multiplier
                        ),
                        self.config.retry_config.max_backoff,
                    );
                }
            }
        }

        unreachable!()
    }

    fn is_retryable_error(&self, error: &ProviderError) -> bool {
        matches!(
            error,
            ProviderError::Timeout(_) |
            ProviderError::NetworkError(_) |
            ProviderError::RateLimitExceeded(_)
        )
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        let start = Instant::now();

        // Make a minimal request to /v1/models endpoint
        let url = format!("{}/v1/models", self.config.base_url);

        let request = hyper::Request::builder()
            .method(hyper::Method::GET)
            .uri(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .body(hyper::Body::empty())
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let _permit = self.client.acquire_permit(&self.provider_id).await?;

        match tokio::time::timeout(
            Duration::from_secs(5),
            self.client.client().request(request)
        ).await {
            Ok(Ok(response)) => {
                let latency = start.elapsed();
                let is_healthy = response.status().is_success();

                let metrics = self.metrics.read().await;
                let error_rate = if metrics.total_requests > 0 {
                    metrics.failed_requests as f32 / metrics.total_requests as f32
                } else {
                    0.0
                };

                Ok(HealthStatus {
                    is_healthy,
                    latency_ms: Some(latency.as_millis() as u64),
                    error_rate,
                    last_check: Instant::now(),
                    details: HashMap::new(),
                })
            }
            Ok(Err(e)) => Ok(HealthStatus {
                is_healthy: false,
                latency_ms: None,
                error_rate: 1.0,
                last_check: Instant::now(),
                details: [("error".to_string(), e.to_string())].into(),
            }),
            Err(_) => Ok(HealthStatus {
                is_healthy: false,
                latency_ms: None,
                error_rate: 1.0,
                last_check: Instant::now(),
                details: [("error".to_string(), "timeout".to_string())].into(),
            }),
        }
    }

    async fn check_rate_limit(&self) -> Option<Duration> {
        // Estimate token count (rough approximation)
        let estimated_tokens = 1000; // Would be calculated from request
        self.rate_limiter.check_and_consume(estimated_tokens).await
    }

    fn transform_request(&self, request: &GatewayRequest) -> Result<Bytes> {
        // Transform to OpenAI format
        let mut openai_messages = Vec::new();

        // Add system message if present
        if let Some(system) = &request.system {
            openai_messages.push(serde_json::json!({
                "role": "system",
                "content": system,
            }));
        }

        // Convert messages
        for msg in &request.messages {
            let role = match msg.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
            };

            let content = match &msg.content {
                MessageContent::Text(text) => serde_json::json!(text),
                MessageContent::MultiModal(parts) => {
                    let mut content_array = Vec::new();
                    for part in parts {
                        match part {
                            ContentPart::Text { text } => {
                                content_array.push(serde_json::json!({
                                    "type": "text",
                                    "text": text,
                                }));
                            }
                            ContentPart::Image { source, detail } => {
                                let image_url = match source {
                                    ImageSource::Url { url } => url.clone(),
                                    ImageSource::Base64 { media_type, data } => {
                                        format!("data:{};base64,{}", media_type, data)
                                    }
                                };

                                content_array.push(serde_json::json!({
                                    "type": "image_url",
                                    "image_url": {
                                        "url": image_url,
                                        "detail": detail.as_ref().unwrap_or(&"auto".to_string()),
                                    }
                                }));
                            }
                        }
                    }
                    serde_json::json!(content_array)
                }
            };

            openai_messages.push(serde_json::json!({
                "role": role,
                "content": content,
            }));
        }

        // Build request body
        let mut body = serde_json::json!({
            "model": request.model,
            "messages": openai_messages,
            "stream": request.stream,
        });

        // Add optional parameters
        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(stop) = &request.stop_sequences {
            body["stop"] = serde_json::json!(stop);
        }

        // Add tools if present
        if let Some(tools) = &request.tools {
            let openai_tools: Vec<_> = tools.iter().map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters,
                    }
                })
            }).collect();

            body["tools"] = serde_json::json!(openai_tools);

            if let Some(tool_choice) = &request.tool_choice {
                body["tool_choice"] = match tool_choice {
                    ToolChoice::Auto => serde_json::json!("auto"),
                    ToolChoice::Required => serde_json::json!("required"),
                    ToolChoice::None => serde_json::json!("none"),
                    ToolChoice::Specific { name } => serde_json::json!({
                        "type": "function",
                        "function": { "name": name }
                    }),
                };
            }
        }

        let json_bytes = serde_json::to_vec(&body)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        Ok(Bytes::from(json_bytes))
    }

    fn transform_response(&self, response: Bytes) -> Result<GatewayResponse> {
        // Parse OpenAI response
        let openai_response: serde_json::Value = serde_json::from_slice(&response)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        // Extract fields
        let request_id = openai_response["id"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        let model = openai_response["model"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        let created = openai_response["created"].as_u64().unwrap_or(0);

        // Parse choices
        let mut choices = Vec::new();
        if let Some(choice_array) = openai_response["choices"].as_array() {
            for choice in choice_array {
                let index = choice["index"].as_u64().unwrap_or(0) as u32;

                let message = if let Some(msg) = choice["message"].as_object() {
                    let role = match msg["role"].as_str().unwrap_or("assistant") {
                        "system" => MessageRole::System,
                        "user" => MessageRole::User,
                        "assistant" => MessageRole::Assistant,
                        "tool" => MessageRole::Tool,
                        _ => MessageRole::Assistant,
                    };

                    let content = msg["content"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    Message {
                        role,
                        content: MessageContent::Text(content),
                        name: None,
                    }
                } else {
                    Message {
                        role: MessageRole::Assistant,
                        content: MessageContent::Text(String::new()),
                        name: None,
                    }
                };

                let finish_reason = match choice["finish_reason"].as_str() {
                    Some("stop") => Some(FinishReason::Stop),
                    Some("length") => Some(FinishReason::Length),
                    Some("tool_calls") => Some(FinishReason::ToolCalls),
                    Some("content_filter") => Some(FinishReason::ContentFilter),
                    _ => None,
                };

                choices.push(Choice {
                    index,
                    message,
                    finish_reason,
                });
            }
        }

        // Parse usage
        let usage = if let Some(usage_obj) = openai_response["usage"].as_object() {
            Usage {
                prompt_tokens: usage_obj["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: usage_obj["completion_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: usage_obj["total_tokens"].as_u64().unwrap_or(0) as u32,
            }
        } else {
            Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            }
        };

        let finish_reason = choices
            .first()
            .and_then(|c| c.finish_reason.clone())
            .unwrap_or(FinishReason::Stop);

        Ok(GatewayResponse {
            request_id,
            provider: self.provider_id.clone(),
            model,
            choices,
            usage,
            created_at: created,
            finish_reason,
            metadata: HashMap::new(),
        })
    }

    async fn chat_completion(&self, request: &GatewayRequest) -> Result<GatewayResponse> {
        // Validate request
        self.validate_request(request)?;

        // Check rate limit
        if let Some(wait_duration) = self.check_rate_limit().await {
            tokio::time::sleep(wait_duration).await;
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_requests += 1;
        }

        let start = Instant::now();

        // Execute with retry
        let result = self.execute_with_retry(|| async {
            // Transform request
            let body = self.transform_request(request)?;

            // Build HTTP request
            let url = format!("{}/v1/chat/completions", self.config.base_url);

            let mut req_builder = hyper::Request::builder()
                .method(hyper::Method::POST)
                .uri(&url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", self.config.api_key));

            if let Some(org) = &self.config.organization {
                req_builder = req_builder.header("OpenAI-Organization", org);
            }

            let http_request = req_builder
                .body(hyper::Body::from(body))
                .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

            // Acquire connection permit
            let _permit = self.client.acquire_permit(&self.provider_id).await?;

            // Send request with timeout
            let response = tokio::time::timeout(
                self.config.timeout,
                self.client.client().request(http_request)
            )
            .await
            .map_err(|_| ProviderError::Timeout("Request timeout".to_string()))?
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

            // Check status
            let status = response.status();
            if !status.is_success() {
                let body_bytes = hyper::body::to_bytes(response.into_body())
                    .await
                    .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

                let error_msg = String::from_utf8_lossy(&body_bytes);

                return Err(match status.as_u16() {
                    401 => ProviderError::AuthenticationFailed(error_msg.to_string()),
                    429 => ProviderError::RateLimitExceeded(error_msg.to_string()),
                    _ => ProviderError::ProviderInternalError(error_msg.to_string()),
                });
            }

            // Read response body
            let body_bytes = hyper::body::to_bytes(response.into_body())
                .await
                .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

            // Transform response
            self.transform_response(body_bytes)
        }).await;

        // Update metrics
        let latency = start.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_latency_ms += latency.as_millis() as u64;

        match &result {
            Ok(_) => {
                metrics.successful_requests += 1;
                metrics.last_success = Some(Instant::now());
            }
            Err(e) => {
                metrics.failed_requests += 1;
                metrics.last_error = Some(e.to_string());
            }
        }

        result
    }

    async fn chat_completion_stream(
        &self,
        request: &GatewayRequest,
    ) -> Result<impl Stream<Item = Result<ChatChunk>> + Send> {
        // Validate request
        self.validate_request(request)?;

        // Check rate limit
        if let Some(wait_duration) = self.check_rate_limit().await {
            tokio::time::sleep(wait_duration).await;
        }

        // Transform request (ensure stream = true)
        let mut streaming_request = request.clone();
        streaming_request.stream = true;
        let body = self.transform_request(&streaming_request)?;

        // Build HTTP request
        let url = format!("{}/v1/chat/completions", self.config.base_url);

        let mut req_builder = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.config.api_key));

        if let Some(org) = &self.config.organization {
            req_builder = req_builder.header("OpenAI-Organization", org);
        }

        let http_request = req_builder
            .body(hyper::Body::from(body))
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        // Acquire connection permit
        let _permit = self.client.acquire_permit(&self.provider_id).await?;

        // Send request
        let response = self.client.client().request(http_request)
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        // Check status
        if !response.status().is_success() {
            let body_bytes = hyper::body::to_bytes(response.into_body())
                .await
                .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

            let error_msg = String::from_utf8_lossy(&body_bytes);
            return Err(ProviderError::ProviderInternalError(error_msg.to_string()));
        }

        // Create stream from response body
        let provider_id = self.provider_id.clone();
        let body_stream = response.into_body();

        // Process SSE stream
        use futures::stream::StreamExt;

        let chunk_stream = body_stream.map(move |chunk_result| {
            match chunk_result {
                Ok(chunk) => {
                    // Parse SSE format: "data: {...}\n\n"
                    let data = String::from_utf8_lossy(&chunk);

                    // Split by SSE delimiters
                    let events: Vec<_> = data
                        .lines()
                        .filter(|line| line.starts_with("data: "))
                        .map(|line| &line[6..]) // Remove "data: " prefix
                        .filter(|data| *data != "[DONE]")
                        .collect();

                    let mut chunks = Vec::new();
                    for event_data in events {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(event_data) {
                            // Transform OpenAI chunk to unified format
                            let request_id = json["id"].as_str().unwrap_or("").to_string();
                            let model = json["model"].as_str().unwrap_or("").to_string();

                            if let Some(choices) = json["choices"].as_array() {
                                for choice in choices {
                                    let delta = choice["delta"].as_object();

                                    let role = delta
                                        .and_then(|d| d["role"].as_str())
                                        .map(|r| match r {
                                            "assistant" => MessageRole::Assistant,
                                            "user" => MessageRole::User,
                                            "system" => MessageRole::System,
                                            _ => MessageRole::Assistant,
                                        });

                                    let content = delta
                                        .and_then(|d| d["content"].as_str())
                                        .map(|s| s.to_string());

                                    let finish_reason = choice["finish_reason"]
                                        .as_str()
                                        .map(|r| match r {
                                            "stop" => FinishReason::Stop,
                                            "length" => FinishReason::Length,
                                            "tool_calls" => FinishReason::ToolCalls,
                                            _ => FinishReason::Stop,
                                        });

                                    chunks.push(Ok(ChatChunk {
                                        request_id: request_id.clone(),
                                        provider: provider_id.clone(),
                                        model: model.clone(),
                                        delta: Delta {
                                            role,
                                            content,
                                            tool_calls: None,
                                        },
                                        finish_reason,
                                        usage: None,
                                    }));
                                }
                            }
                        }
                    }

                    futures::stream::iter(chunks)
                }
                Err(e) => {
                    futures::stream::iter(vec![Err(ProviderError::StreamError(e.to_string()))])
                }
            }
        })
        .flatten();

        Ok(chunk_stream)
    }
}

// ============================================================================
// SECTION 9: Anthropic Provider Implementation
// ============================================================================

pub struct AnthropicProvider {
    provider_id: String,
    client: Arc<ConnectionPool>,
    config: AnthropicConfig,
    capabilities: ProviderCapabilities,
    rate_limiter: RateLimiter,
    metrics: Arc<RwLock<ProviderMetrics>>,
}

#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    pub api_key: String,
    pub base_url: String,
    pub api_version: String,
    pub timeout: Duration,
    pub retry_config: RetryConfig,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.anthropic.com".to_string(),
            api_version: "2023-06-01".to_string(),
            timeout: Duration::from_secs(300),
            retry_config: RetryConfig::default(),
        }
    }
}

impl AnthropicProvider {
    pub fn new(config: AnthropicConfig, connection_pool: Arc<ConnectionPool>) -> Self {
        let capabilities = ProviderCapabilities {
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: true,
            supports_system_messages: true,
            max_context_tokens: 200_000, // Claude 3 context window
            max_output_tokens: 4_096,
            models: vec![
                "claude-3-opus-20240229".to_string(),
                "claude-3-sonnet-20240229".to_string(),
                "claude-3-haiku-20240307".to_string(),
            ],
            rate_limits: RateLimitInfo {
                requests_per_minute: Some(1000),
                tokens_per_minute: Some(400_000),
                concurrent_requests: Some(100),
            },
        };

        let rate_limiter = RateLimiter::new(RateLimitConfig {
            requests_per_minute: Some(1000),
            tokens_per_minute: Some(400_000),
        });

        Self {
            provider_id: "anthropic".to_string(),
            client: connection_pool,
            config,
            capabilities,
            rate_limiter,
            metrics: Arc::new(RwLock::new(ProviderMetrics {
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                total_latency_ms: 0,
                last_error: None,
                last_success: None,
            })),
        }
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        // Similar to OpenAI implementation
        // Make request to Anthropic API to verify connectivity
        let start = Instant::now();

        // Anthropic doesn't have a dedicated health endpoint
        // We'll make a minimal message request to verify
        let minimal_request = serde_json::json!({
            "model": "claude-3-haiku-20240307",
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "Hi"}]
        });

        let body = serde_json::to_vec(&minimal_request)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        let request = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(format!("{}/v1/messages", self.config.base_url))
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", &self.config.api_version)
            .body(hyper::Body::from(body))
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let _permit = self.client.acquire_permit(&self.provider_id).await?;

        match tokio::time::timeout(
            Duration::from_secs(5),
            self.client.client().request(request)
        ).await {
            Ok(Ok(response)) => {
                let latency = start.elapsed();
                let is_healthy = response.status().is_success();

                let metrics = self.metrics.read().await;
                let error_rate = if metrics.total_requests > 0 {
                    metrics.failed_requests as f32 / metrics.total_requests as f32
                } else {
                    0.0
                };

                Ok(HealthStatus {
                    is_healthy,
                    latency_ms: Some(latency.as_millis() as u64),
                    error_rate,
                    last_check: Instant::now(),
                    details: HashMap::new(),
                })
            }
            Ok(Err(e)) => Ok(HealthStatus {
                is_healthy: false,
                latency_ms: None,
                error_rate: 1.0,
                last_check: Instant::now(),
                details: [("error".to_string(), e.to_string())].into(),
            }),
            Err(_) => Ok(HealthStatus {
                is_healthy: false,
                latency_ms: None,
                error_rate: 1.0,
                last_check: Instant::now(),
                details: [("error".to_string(), "timeout".to_string())].into(),
            }),
        }
    }

    async fn check_rate_limit(&self) -> Option<Duration> {
        let estimated_tokens = 1000;
        self.rate_limiter.check_and_consume(estimated_tokens).await
    }

    fn transform_request(&self, request: &GatewayRequest) -> Result<Bytes> {
        // Transform to Anthropic Messages API format
        let mut anthropic_messages = Vec::new();

        // Anthropic doesn't allow system role in messages array
        // System prompt is a separate field
        for msg in &request.messages {
            // Skip system messages (handled separately)
            if matches!(msg.role, MessageRole::System) {
                continue;
            }

            let role = match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                _ => continue, // Skip unsupported roles
            };

            let content = match &msg.content {
                MessageContent::Text(text) => serde_json::json!(text),
                MessageContent::MultiModal(parts) => {
                    let mut content_array = Vec::new();
                    for part in parts {
                        match part {
                            ContentPart::Text { text } => {
                                content_array.push(serde_json::json!({
                                    "type": "text",
                                    "text": text,
                                }));
                            }
                            ContentPart::Image { source, .. } => {
                                match source {
                                    ImageSource::Base64 { media_type, data } => {
                                        content_array.push(serde_json::json!({
                                            "type": "image",
                                            "source": {
                                                "type": "base64",
                                                "media_type": media_type,
                                                "data": data,
                                            }
                                        }));
                                    }
                                    ImageSource::Url { .. } => {
                                        // Anthropic doesn't support image URLs directly
                                        // Would need to download and convert to base64
                                        return Err(ProviderError::UnsupportedCapability(
                                            "Anthropic requires base64 images".to_string()
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    serde_json::json!(content_array)
                }
            };

            anthropic_messages.push(serde_json::json!({
                "role": role,
                "content": content,
            }));
        }

        // Build request body
        let mut body = serde_json::json!({
            "model": request.model,
            "messages": anthropic_messages,
            "max_tokens": request.max_tokens.unwrap_or(4096),
            "stream": request.stream,
        });

        // Add system prompt
        if let Some(system) = &request.system {
            body["system"] = serde_json::json!(system);
        }

        // Add optional parameters
        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(top_k) = request.top_k {
            body["top_k"] = serde_json::json!(top_k);
        }
        if let Some(stop) = &request.stop_sequences {
            body["stop_sequences"] = serde_json::json!(stop);
        }

        // Add tools if present
        if let Some(tools) = &request.tools {
            let anthropic_tools: Vec<_> = tools.iter().map(|tool| {
                serde_json::json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.parameters,
                })
            }).collect();

            body["tools"] = serde_json::json!(anthropic_tools);
        }

        let json_bytes = serde_json::to_vec(&body)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        Ok(Bytes::from(json_bytes))
    }

    fn transform_response(&self, response: Bytes) -> Result<GatewayResponse> {
        // Parse Anthropic response
        let anthropic_response: serde_json::Value = serde_json::from_slice(&response)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        let request_id = anthropic_response["id"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        let model = anthropic_response["model"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        // Anthropic returns single content array
        let content_array = anthropic_response["content"]
            .as_array()
            .ok_or_else(|| ProviderError::SerializationError("Missing content".to_string()))?;

        // Combine text content
        let mut combined_text = String::new();
        for content_block in content_array {
            if content_block["type"] == "text" {
                if let Some(text) = content_block["text"].as_str() {
                    combined_text.push_str(text);
                }
            }
        }

        let message = Message {
            role: MessageRole::Assistant,
            content: MessageContent::Text(combined_text),
            name: None,
        };

        let finish_reason = match anthropic_response["stop_reason"].as_str() {
            Some("end_turn") => FinishReason::Stop,
            Some("max_tokens") => FinishReason::Length,
            Some("tool_use") => FinishReason::ToolCalls,
            _ => FinishReason::Stop,
        };

        // Parse usage
        let usage = if let Some(usage_obj) = anthropic_response["usage"].as_object() {
            Usage {
                prompt_tokens: usage_obj["input_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: usage_obj["output_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: (usage_obj["input_tokens"].as_u64().unwrap_or(0) +
                              usage_obj["output_tokens"].as_u64().unwrap_or(0)) as u32,
            }
        } else {
            Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            }
        };

        Ok(GatewayResponse {
            request_id,
            provider: self.provider_id.clone(),
            model,
            choices: vec![Choice {
                index: 0,
                message,
                finish_reason: Some(finish_reason.clone()),
            }],
            usage,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            finish_reason,
            metadata: HashMap::new(),
        })
    }

    async fn chat_completion(&self, request: &GatewayRequest) -> Result<GatewayResponse> {
        // Validate request
        self.validate_request(request)?;

        // Check rate limit
        if let Some(wait_duration) = self.check_rate_limit().await {
            tokio::time::sleep(wait_duration).await;
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_requests += 1;
        }

        let start = Instant::now();

        // Transform request
        let body = self.transform_request(request)?;

        // Build HTTP request
        let url = format!("{}/v1/messages", self.config.base_url);

        let http_request = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(&url)
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", &self.config.api_version)
            .body(hyper::Body::from(body))
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        // Acquire connection permit
        let _permit = self.client.acquire_permit(&self.provider_id).await?;

        // Send request with timeout
        let response = tokio::time::timeout(
            self.config.timeout,
            self.client.client().request(http_request)
        )
        .await
        .map_err(|_| ProviderError::Timeout("Request timeout".to_string()))?
        .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        // Check status
        let status = response.status();
        if !status.is_success() {
            let body_bytes = hyper::body::to_bytes(response.into_body())
                .await
                .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

            let error_msg = String::from_utf8_lossy(&body_bytes);

            let error = match status.as_u16() {
                401 => ProviderError::AuthenticationFailed(error_msg.to_string()),
                429 => ProviderError::RateLimitExceeded(error_msg.to_string()),
                _ => ProviderError::ProviderInternalError(error_msg.to_string()),
            };

            let mut metrics = self.metrics.write().await;
            metrics.failed_requests += 1;
            metrics.last_error = Some(error.to_string());

            return Err(error);
        }

        // Read response body
        let body_bytes = hyper::body::to_bytes(response.into_body())
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        // Transform response
        let result = self.transform_response(body_bytes);

        // Update metrics
        let latency = start.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.total_latency_ms += latency.as_millis() as u64;

        match &result {
            Ok(_) => {
                metrics.successful_requests += 1;
                metrics.last_success = Some(Instant::now());
            }
            Err(e) => {
                metrics.failed_requests += 1;
                metrics.last_error = Some(e.to_string());
            }
        }

        result
    }

    async fn chat_completion_stream(
        &self,
        request: &GatewayRequest,
    ) -> Result<impl Stream<Item = Result<ChatChunk>> + Send> {
        // Validate request
        self.validate_request(request)?;

        // Check rate limit
        if let Some(wait_duration) = self.check_rate_limit().await {
            tokio::time::sleep(wait_duration).await;
        }

        // Transform request (ensure stream = true)
        let mut streaming_request = request.clone();
        streaming_request.stream = true;
        let body = self.transform_request(&streaming_request)?;

        // Build HTTP request
        let url = format!("{}/v1/messages", self.config.base_url);

        let http_request = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(&url)
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", &self.config.api_version)
            .body(hyper::Body::from(body))
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        // Acquire connection permit
        let _permit = self.client.acquire_permit(&self.provider_id).await?;

        // Send request
        let response = self.client.client().request(http_request)
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        // Check status
        if !response.status().is_success() {
            let body_bytes = hyper::body::to_bytes(response.into_body())
                .await
                .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

            let error_msg = String::from_utf8_lossy(&body_bytes);
            return Err(ProviderError::ProviderInternalError(error_msg.to_string()));
        }

        // Create stream from response body
        let provider_id = self.provider_id.clone();
        let body_stream = response.into_body();

        // Process SSE stream
        use futures::stream::StreamExt;

        let chunk_stream = body_stream.map(move |chunk_result| {
            match chunk_result {
                Ok(chunk) => {
                    let data = String::from_utf8_lossy(&chunk);

                    let events: Vec<_> = data
                        .lines()
                        .filter(|line| line.starts_with("data: "))
                        .map(|line| &line[6..])
                        .collect();

                    let mut chunks = Vec::new();
                    for event_data in events {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(event_data) {
                            let event_type = json["type"].as_str().unwrap_or("");

                            match event_type {
                                "content_block_delta" => {
                                    let delta = &json["delta"];
                                    if delta["type"] == "text_delta" {
                                        let text = delta["text"].as_str().unwrap_or("");

                                        chunks.push(Ok(ChatChunk {
                                            request_id: String::new(),
                                            provider: provider_id.clone(),
                                            model: String::new(),
                                            delta: Delta {
                                                role: None,
                                                content: Some(text.to_string()),
                                                tool_calls: None,
                                            },
                                            finish_reason: None,
                                            usage: None,
                                        }));
                                    }
                                }
                                "message_stop" => {
                                    chunks.push(Ok(ChatChunk {
                                        request_id: String::new(),
                                        provider: provider_id.clone(),
                                        model: String::new(),
                                        delta: Delta {
                                            role: None,
                                            content: None,
                                            tool_calls: None,
                                        },
                                        finish_reason: Some(FinishReason::Stop),
                                        usage: None,
                                    }));
                                }
                                _ => {}
                            }
                        }
                    }

                    futures::stream::iter(chunks)
                }
                Err(e) => {
                    futures::stream::iter(vec![Err(ProviderError::StreamError(e.to_string()))])
                }
            }
        })
        .flatten();

        Ok(chunk_stream)
    }
}

// ============================================================================
// SECTION 10: Additional Provider Stubs
// ============================================================================

// Google Provider
pub struct GoogleProvider {
    provider_id: String,
    client: Arc<ConnectionPool>,
    config: GoogleConfig,
    capabilities: ProviderCapabilities,
    rate_limiter: RateLimiter,
}

#[derive(Debug, Clone)]
pub struct GoogleConfig {
    pub api_key: String,
    pub base_url: String,
    pub project_id: Option<String>,
    pub timeout: Duration,
}

// vLLM Provider (OpenAI-compatible)
pub struct VLLMProvider {
    provider_id: String,
    client: Arc<ConnectionPool>,
    config: VLLMConfig,
    capabilities: ProviderCapabilities,
}

#[derive(Debug, Clone)]
pub struct VLLMConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub timeout: Duration,
}

// Ollama Provider
pub struct OllamaProvider {
    provider_id: String,
    client: Arc<ConnectionPool>,
    config: OllamaConfig,
    capabilities: ProviderCapabilities,
}

#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub base_url: String,
    pub timeout: Duration,
}

// Together AI Provider (OpenAI-compatible)
pub struct TogetherProvider {
    provider_id: String,
    client: Arc<ConnectionPool>,
    config: TogetherConfig,
    capabilities: ProviderCapabilities,
    rate_limiter: RateLimiter,
}

#[derive(Debug, Clone)]
pub struct TogetherConfig {
    pub api_key: String,
    pub base_url: String,
    pub timeout: Duration,
}

// AWS Bedrock Provider
pub struct BedrockProvider {
    provider_id: String,
    client: Arc<ConnectionPool>,
    config: BedrockConfig,
    capabilities: ProviderCapabilities,
    rate_limiter: RateLimiter,
}

#[derive(Debug, Clone)]
pub struct BedrockConfig {
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
    pub timeout: Duration,
}

// Azure OpenAI Provider
pub struct AzureOpenAIProvider {
    provider_id: String,
    client: Arc<ConnectionPool>,
    config: AzureOpenAIConfig,
    capabilities: ProviderCapabilities,
    rate_limiter: RateLimiter,
}

#[derive(Debug, Clone)]
pub struct AzureOpenAIConfig {
    pub api_key: String,
    pub endpoint: String,
    pub deployment_name: String,
    pub api_version: String,
    pub timeout: Duration,
}

// ============================================================================
// SECTION 11: Provider Factory
// ============================================================================

pub struct ProviderFactory {
    connection_pool: Arc<ConnectionPool>,
}

impl ProviderFactory {
    pub fn new(connection_pool: Arc<ConnectionPool>) -> Self {
        Self { connection_pool }
    }

    pub fn create_openai(&self, config: OpenAIConfig) -> Arc<dyn LLMProvider> {
        Arc::new(OpenAIProvider::new(config, Arc::clone(&self.connection_pool)))
    }

    pub fn create_anthropic(&self, config: AnthropicConfig) -> Arc<dyn LLMProvider> {
        Arc::new(AnthropicProvider::new(config, Arc::clone(&self.connection_pool)))
    }

    pub fn create_from_config(&self, provider_type: &str, config: serde_json::Value)
        -> Result<Arc<dyn LLMProvider>>
    {
        match provider_type {
            "openai" => {
                let config: OpenAIConfig = serde_json::from_value(config)
                    .map_err(|e| ProviderError::InvalidRequest(e.to_string()))?;
                Ok(self.create_openai(config))
            }
            "anthropic" => {
                let config: AnthropicConfig = serde_json::from_value(config)
                    .map_err(|e| ProviderError::InvalidRequest(e.to_string()))?;
                Ok(self.create_anthropic(config))
            }
            // Add other providers...
            _ => Err(ProviderError::NotFound(format!("Unknown provider type: {}", provider_type)))
        }
    }
}

// ============================================================================
// SECTION 12: Usage Example
// ============================================================================

/*
async fn example_usage() -> Result<()> {
    // Initialize connection pool
    let pool_config = ConnectionPoolConfig::default();
    let connection_pool = Arc::new(ConnectionPool::new(pool_config));

    // Create provider registry
    let registry = Arc::new(ProviderRegistry::new(Duration::from_secs(60)));

    // Start background health checks
    registry.start_health_checks().await;

    // Create provider factory
    let factory = ProviderFactory::new(Arc::clone(&connection_pool));

    // Create OpenAI provider
    let openai_config = OpenAIConfig {
        api_key: "sk-...".to_string(),
        base_url: "https://api.openai.com".to_string(),
        organization: None,
        timeout: Duration::from_secs(60),
        retry_config: RetryConfig::default(),
    };

    let openai_provider = factory.create_openai(openai_config);

    // Register provider
    registry.register(openai_provider.clone()).await?;

    // Create Anthropic provider
    let anthropic_config = AnthropicConfig::default();
    let anthropic_provider = factory.create_anthropic(anthropic_config);
    registry.register(anthropic_provider).await?;

    // Make a request
    let request = GatewayRequest {
        request_id: "req-123".to_string(),
        model: "gpt-4-turbo".to_string(),
        messages: vec![
            Message {
                role: MessageRole::User,
                content: MessageContent::Text("Hello!".to_string()),
                name: None,
            }
        ],
        temperature: Some(0.7),
        max_tokens: Some(100),
        top_p: None,
        top_k: None,
        stop_sequences: None,
        stream: false,
        system: None,
        tools: None,
        tool_choice: None,
        metadata: HashMap::new(),
        timeout: None,
    };

    // Get response
    let response = openai_provider.chat_completion(&request).await?;
    println!("Response: {:?}", response);

    // Streaming example
    use futures::stream::StreamExt;

    let mut streaming_request = request.clone();
    streaming_request.stream = true;

    let mut stream = openai_provider.chat_completion_stream(&streaming_request).await?;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                if let Some(content) = chunk.delta.content {
                    print!("{}", content);
                }
            }
            Err(e) => eprintln!("Stream error: {}", e),
        }
    }

    Ok(())
}
*/
