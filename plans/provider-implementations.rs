// ============================================================================
// Additional Provider Implementations
// Google Gemini, vLLM, Ollama, Together AI, AWS Bedrock, Azure OpenAI
// ============================================================================

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::sync::RwLock;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use bytes::Bytes;

use crate::{
    LLMProvider, GatewayRequest, GatewayResponse, ChatChunk,
    ProviderError, Result, HealthStatus, ProviderCapabilities,
    ConnectionPool, RateLimiter, RateLimitConfig, RetryConfig,
    ProviderMetrics, Message, MessageRole, MessageContent,
    ContentPart, ImageSource, Usage, FinishReason, Choice, Delta,
};

// ============================================================================
// SECTION 1: Google Gemini Provider
// ============================================================================

pub struct GoogleProvider {
    provider_id: String,
    client: Arc<ConnectionPool>,
    config: GoogleConfig,
    capabilities: ProviderCapabilities,
    rate_limiter: RateLimiter,
    metrics: Arc<RwLock<ProviderMetrics>>,
}

#[derive(Debug, Clone)]
pub struct GoogleConfig {
    pub api_key: String,
    pub base_url: String,
    pub project_id: Option<String>,
    pub location: Option<String>, // For Vertex AI
    pub timeout: Duration,
    pub retry_config: RetryConfig,
}

impl Default for GoogleConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://generativelanguage.googleapis.com".to_string(),
            project_id: None,
            location: None,
            timeout: Duration::from_secs(120),
            retry_config: RetryConfig::default(),
        }
    }
}

impl GoogleProvider {
    pub fn new(config: GoogleConfig, connection_pool: Arc<ConnectionPool>) -> Self {
        let capabilities = ProviderCapabilities {
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: true,
            supports_system_messages: true,
            max_context_tokens: 32_000, // Gemini Pro
            max_output_tokens: 2_048,
            models: vec![
                "gemini-pro".to_string(),
                "gemini-pro-vision".to_string(),
                "gemini-ultra".to_string(),
            ],
            rate_limits: crate::RateLimitInfo {
                requests_per_minute: Some(60),
                tokens_per_minute: Some(120_000),
                concurrent_requests: Some(10),
            },
        };

        let rate_limiter = RateLimiter::new(RateLimitConfig {
            requests_per_minute: Some(60),
            tokens_per_minute: Some(120_000),
        });

        Self {
            provider_id: "google".to_string(),
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
impl LLMProvider for GoogleProvider {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        // Google Gemini health check via models list
        let url = format!(
            "{}/v1beta/models?key={}",
            self.config.base_url,
            self.config.api_key
        );

        let request = hyper::Request::builder()
            .method(hyper::Method::GET)
            .uri(&url)
            .body(hyper::Body::empty())
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let start = Instant::now();
        let _permit = self.client.acquire_permit(&self.provider_id).await?;

        match tokio::time::timeout(
            Duration::from_secs(5),
            self.client.client().request(request)
        ).await {
            Ok(Ok(response)) => {
                let latency = start.elapsed();
                Ok(HealthStatus {
                    is_healthy: response.status().is_success(),
                    latency_ms: Some(latency.as_millis() as u64),
                    error_rate: 0.0,
                    last_check: Instant::now(),
                    details: HashMap::new(),
                })
            }
            _ => Ok(HealthStatus {
                is_healthy: false,
                latency_ms: None,
                error_rate: 1.0,
                last_check: Instant::now(),
                details: HashMap::new(),
            }),
        }
    }

    async fn check_rate_limit(&self) -> Option<Duration> {
        self.rate_limiter.check_and_consume(1000).await
    }

    fn transform_request(&self, request: &GatewayRequest) -> Result<Bytes> {
        // Transform to Gemini format
        let mut contents = Vec::new();

        for msg in &request.messages {
            let role = match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "model",
                _ => continue,
            };

            let parts = match &msg.content {
                MessageContent::Text(text) => {
                    vec![serde_json::json!({"text": text})]
                }
                MessageContent::MultiModal(parts) => {
                    parts.iter().map(|part| match part {
                        ContentPart::Text { text } => {
                            serde_json::json!({"text": text})
                        }
                        ContentPart::Image { source, .. } => {
                            match source {
                                ImageSource::Base64 { media_type, data } => {
                                    serde_json::json!({
                                        "inline_data": {
                                            "mime_type": media_type,
                                            "data": data
                                        }
                                    })
                                }
                                ImageSource::Url { url } => {
                                    serde_json::json!({"image_url": url})
                                }
                            }
                        }
                    }).collect()
                }
            };

            contents.push(serde_json::json!({
                "role": role,
                "parts": parts
            }));
        }

        let mut body = serde_json::json!({
            "contents": contents,
        });

        // Generation config
        let mut generation_config = serde_json::json!({});

        if let Some(temp) = request.temperature {
            generation_config["temperature"] = serde_json::json!(temp);
        }
        if let Some(max_tokens) = request.max_tokens {
            generation_config["maxOutputTokens"] = serde_json::json!(max_tokens);
        }
        if let Some(top_p) = request.top_p {
            generation_config["topP"] = serde_json::json!(top_p);
        }
        if let Some(top_k) = request.top_k {
            generation_config["topK"] = serde_json::json!(top_k);
        }

        body["generationConfig"] = generation_config;

        let json_bytes = serde_json::to_vec(&body)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        Ok(Bytes::from(json_bytes))
    }

    fn transform_response(&self, response: Bytes) -> Result<GatewayResponse> {
        let gemini_response: serde_json::Value = serde_json::from_slice(&response)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        // Extract content from candidates
        let candidates = gemini_response["candidates"]
            .as_array()
            .ok_or_else(|| ProviderError::SerializationError("Missing candidates".to_string()))?;

        let mut choices = Vec::new();

        for (index, candidate) in candidates.iter().enumerate() {
            let content = candidate["content"]["parts"]
                .as_array()
                .and_then(|parts| parts.first())
                .and_then(|part| part["text"].as_str())
                .unwrap_or("")
                .to_string();

            let finish_reason = match candidate["finishReason"].as_str() {
                Some("STOP") => FinishReason::Stop,
                Some("MAX_TOKENS") => FinishReason::Length,
                Some("SAFETY") => FinishReason::ContentFilter,
                _ => FinishReason::Stop,
            };

            choices.push(Choice {
                index: index as u32,
                message: Message {
                    role: MessageRole::Assistant,
                    content: MessageContent::Text(content),
                    name: None,
                },
                finish_reason: Some(finish_reason.clone()),
            });
        }

        // Extract usage metadata
        let usage_metadata = &gemini_response["usageMetadata"];
        let usage = Usage {
            prompt_tokens: usage_metadata["promptTokenCount"].as_u64().unwrap_or(0) as u32,
            completion_tokens: usage_metadata["candidatesTokenCount"].as_u64().unwrap_or(0) as u32,
            total_tokens: usage_metadata["totalTokenCount"].as_u64().unwrap_or(0) as u32,
        };

        Ok(GatewayResponse {
            request_id: "gemini-req".to_string(),
            provider: self.provider_id.clone(),
            model: "gemini-pro".to_string(),
            choices,
            usage,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            finish_reason: FinishReason::Stop,
            metadata: HashMap::new(),
        })
    }

    async fn chat_completion(&self, request: &GatewayRequest) -> Result<GatewayResponse> {
        self.validate_request(request)?;

        if let Some(wait_duration) = self.check_rate_limit().await {
            tokio::time::sleep(wait_duration).await;
        }

        let body = self.transform_request(request)?;

        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.config.base_url,
            request.model,
            self.config.api_key
        );

        let http_request = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(&url)
            .header("Content-Type", "application/json")
            .body(hyper::Body::from(body))
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let _permit = self.client.acquire_permit(&self.provider_id).await?;

        let response = tokio::time::timeout(
            self.config.timeout,
            self.client.client().request(http_request)
        )
        .await
        .map_err(|_| ProviderError::Timeout("Request timeout".to_string()))?
        .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let body_bytes = hyper::body::to_bytes(response.into_body())
                .await
                .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

            return Err(ProviderError::ProviderInternalError(
                String::from_utf8_lossy(&body_bytes).to_string()
            ));
        }

        let body_bytes = hyper::body::to_bytes(response.into_body())
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        self.transform_response(body_bytes)
    }

    async fn chat_completion_stream(
        &self,
        request: &GatewayRequest,
    ) -> Result<impl futures::stream::Stream<Item = Result<ChatChunk>> + Send> {
        // Gemini streaming implementation
        self.validate_request(request)?;

        let body = self.transform_request(request)?;

        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?key={}",
            self.config.base_url,
            request.model,
            self.config.api_key
        );

        let http_request = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(&url)
            .header("Content-Type", "application/json")
            .body(hyper::Body::from(body))
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let _permit = self.client.acquire_permit(&self.provider_id).await?;

        let response = self.client.client().request(http_request)
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        use futures::stream::StreamExt;

        let provider_id = self.provider_id.clone();
        let body_stream = response.into_body();

        let chunk_stream = body_stream.map(move |chunk_result| {
            match chunk_result {
                Ok(chunk) => {
                    // Parse Gemini streaming format
                    futures::stream::iter(vec![Ok(ChatChunk {
                        request_id: String::new(),
                        provider: provider_id.clone(),
                        model: "gemini-pro".to_string(),
                        delta: Delta {
                            role: None,
                            content: Some(String::from_utf8_lossy(&chunk).to_string()),
                            tool_calls: None,
                        },
                        finish_reason: None,
                        usage: None,
                    })])
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
// SECTION 2: vLLM Provider (OpenAI-Compatible)
// ============================================================================

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
    pub available_models: Vec<String>,
}

impl Default for VLLMConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8000".to_string(),
            api_key: None,
            timeout: Duration::from_secs(120),
            available_models: vec![],
        }
    }
}

impl VLLMProvider {
    pub fn new(config: VLLMConfig, connection_pool: Arc<ConnectionPool>) -> Self {
        let capabilities = ProviderCapabilities {
            supports_streaming: true,
            supports_tools: false, // Depends on model
            supports_multimodal: false, // Depends on model
            supports_system_messages: true,
            max_context_tokens: 8192, // Model dependent
            max_output_tokens: 2048,
            models: config.available_models.clone(),
            rate_limits: crate::RateLimitInfo {
                requests_per_minute: None, // No rate limits for local deployment
                tokens_per_minute: None,
                concurrent_requests: Some(10),
            },
        };

        Self {
            provider_id: "vllm".to_string(),
            client: connection_pool,
            config,
            capabilities,
        }
    }
}

#[async_trait]
impl LLMProvider for VLLMProvider {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        let url = format!("{}/health", self.config.base_url);

        let request = hyper::Request::builder()
            .method(hyper::Method::GET)
            .uri(&url)
            .body(hyper::Body::empty())
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let start = Instant::now();

        match tokio::time::timeout(
            Duration::from_secs(5),
            self.client.client().request(request)
        ).await {
            Ok(Ok(response)) => {
                let latency = start.elapsed();
                Ok(HealthStatus {
                    is_healthy: response.status().is_success(),
                    latency_ms: Some(latency.as_millis() as u64),
                    error_rate: 0.0,
                    last_check: Instant::now(),
                    details: HashMap::new(),
                })
            }
            _ => Ok(HealthStatus {
                is_healthy: false,
                latency_ms: None,
                error_rate: 1.0,
                last_check: Instant::now(),
                details: HashMap::new(),
            }),
        }
    }

    async fn check_rate_limit(&self) -> Option<Duration> {
        None // No rate limiting for local vLLM
    }

    fn transform_request(&self, request: &GatewayRequest) -> Result<Bytes> {
        // vLLM uses OpenAI-compatible format
        // Reuse OpenAI transformation logic
        crate::OpenAIProvider::transform_openai_request(request)
    }

    fn transform_response(&self, response: Bytes) -> Result<GatewayResponse> {
        // vLLM uses OpenAI-compatible format
        crate::OpenAIProvider::transform_openai_response(response, &self.provider_id)
    }

    async fn chat_completion(&self, request: &GatewayRequest) -> Result<GatewayResponse> {
        self.validate_request(request)?;

        let body = self.transform_request(request)?;
        let url = format!("{}/v1/chat/completions", self.config.base_url);

        let mut req_builder = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(&url)
            .header("Content-Type", "application/json");

        if let Some(api_key) = &self.config.api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
        }

        let http_request = req_builder
            .body(hyper::Body::from(body))
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let _permit = self.client.acquire_permit(&self.provider_id).await?;

        let response = tokio::time::timeout(
            self.config.timeout,
            self.client.client().request(http_request)
        )
        .await
        .map_err(|_| ProviderError::Timeout("Request timeout".to_string()))?
        .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let body_bytes = hyper::body::to_bytes(response.into_body())
                .await
                .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

            return Err(ProviderError::ProviderInternalError(
                String::from_utf8_lossy(&body_bytes).to_string()
            ));
        }

        let body_bytes = hyper::body::to_bytes(response.into_body())
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        self.transform_response(body_bytes)
    }

    async fn chat_completion_stream(
        &self,
        request: &GatewayRequest,
    ) -> Result<impl futures::stream::Stream<Item = Result<ChatChunk>> + Send> {
        // Similar to OpenAI streaming implementation
        // vLLM uses same SSE format
        todo!("Implement vLLM streaming")
    }
}

// ============================================================================
// SECTION 3: Ollama Provider
// ============================================================================

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

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            timeout: Duration::from_secs(120),
        }
    }
}

impl OllamaProvider {
    pub fn new(config: OllamaConfig, connection_pool: Arc<ConnectionPool>) -> Self {
        let capabilities = ProviderCapabilities {
            supports_streaming: true,
            supports_tools: false,
            supports_multimodal: true, // Ollama supports vision models
            supports_system_messages: true,
            max_context_tokens: 4096, // Model dependent
            max_output_tokens: 2048,
            models: vec![], // Dynamic from Ollama
            rate_limits: crate::RateLimitInfo {
                requests_per_minute: None,
                tokens_per_minute: None,
                concurrent_requests: Some(5),
            },
        };

        Self {
            provider_id: "ollama".to_string(),
            client: connection_pool,
            config,
            capabilities,
        }
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.config.base_url);

        let request = hyper::Request::builder()
            .method(hyper::Method::GET)
            .uri(&url)
            .body(hyper::Body::empty())
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let response = self.client.client().request(request)
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let body_bytes = hyper::body::to_bytes(response.into_body())
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let tags: serde_json::Value = serde_json::from_slice(&body_bytes)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        let models = tags["models"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(models)
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        match self.list_models().await {
            Ok(_) => Ok(HealthStatus {
                is_healthy: true,
                latency_ms: Some(10),
                error_rate: 0.0,
                last_check: Instant::now(),
                details: HashMap::new(),
            }),
            Err(_) => Ok(HealthStatus {
                is_healthy: false,
                latency_ms: None,
                error_rate: 1.0,
                last_check: Instant::now(),
                details: HashMap::new(),
            }),
        }
    }

    async fn check_rate_limit(&self) -> Option<Duration> {
        None
    }

    fn transform_request(&self, request: &GatewayRequest) -> Result<Bytes> {
        // Ollama chat format
        let messages: Vec<_> = request.messages.iter().map(|msg| {
            let role = match msg.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                _ => "user",
            };

            let content = match &msg.content {
                MessageContent::Text(text) => text.clone(),
                MessageContent::MultiModal(parts) => {
                    // Extract text from multimodal (Ollama handles images separately)
                    parts.iter()
                        .filter_map(|p| match p {
                            ContentPart::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                }
            };

            serde_json::json!({
                "role": role,
                "content": content
            })
        }).collect();

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "stream": request.stream,
        });

        if let Some(temp) = request.temperature {
            body["options"] = serde_json::json!({"temperature": temp});
        }

        let json_bytes = serde_json::to_vec(&body)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        Ok(Bytes::from(json_bytes))
    }

    fn transform_response(&self, response: Bytes) -> Result<GatewayResponse> {
        let ollama_response: serde_json::Value = serde_json::from_slice(&response)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        let content = ollama_response["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let message = Message {
            role: MessageRole::Assistant,
            content: MessageContent::Text(content),
            name: None,
        };

        Ok(GatewayResponse {
            request_id: "ollama-req".to_string(),
            provider: self.provider_id.clone(),
            model: ollama_response["model"].as_str().unwrap_or("unknown").to_string(),
            choices: vec![Choice {
                index: 0,
                message,
                finish_reason: Some(FinishReason::Stop),
            }],
            usage: Usage {
                prompt_tokens: ollama_response["prompt_eval_count"].as_u64().unwrap_or(0) as u32,
                completion_tokens: ollama_response["eval_count"].as_u64().unwrap_or(0) as u32,
                total_tokens: 0, // Calculated
            },
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            finish_reason: FinishReason::Stop,
            metadata: HashMap::new(),
        })
    }

    async fn chat_completion(&self, request: &GatewayRequest) -> Result<GatewayResponse> {
        self.validate_request(request)?;

        let body = self.transform_request(request)?;
        let url = format!("{}/api/chat", self.config.base_url);

        let http_request = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(&url)
            .header("Content-Type", "application/json")
            .body(hyper::Body::from(body))
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let _permit = self.client.acquire_permit(&self.provider_id).await?;

        let response = tokio::time::timeout(
            self.config.timeout,
            self.client.client().request(http_request)
        )
        .await
        .map_err(|_| ProviderError::Timeout("Request timeout".to_string()))?
        .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let body_bytes = hyper::body::to_bytes(response.into_body())
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        self.transform_response(body_bytes)
    }

    async fn chat_completion_stream(
        &self,
        _request: &GatewayRequest,
    ) -> Result<impl futures::stream::Stream<Item = Result<ChatChunk>> + Send> {
        // Ollama streaming implementation
        todo!("Implement Ollama streaming")
    }
}

// ============================================================================
// SECTION 4: AWS Bedrock Provider
// ============================================================================

use aws_sdk_bedrockruntime::{Client as BedrockClient, types::ContentBlock};
use aws_config::BehaviorVersion;

pub struct BedrockProvider {
    provider_id: String,
    client: BedrockClient,
    config: BedrockConfig,
    capabilities: ProviderCapabilities,
    rate_limiter: RateLimiter,
}

#[derive(Debug, Clone)]
pub struct BedrockConfig {
    pub region: String,
    pub model_id: String,
    pub timeout: Duration,
}

impl Default for BedrockConfig {
    fn default() -> Self {
        Self {
            region: "us-east-1".to_string(),
            model_id: "anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
            timeout: Duration::from_secs(120),
        }
    }
}

impl BedrockProvider {
    pub async fn new(config: BedrockConfig) -> Self {
        // Initialize AWS SDK
        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_sdk_bedrockruntime::config::Region::new(config.region.clone()))
            .load()
            .await;

        let client = BedrockClient::new(&aws_config);

        let capabilities = ProviderCapabilities {
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: true,
            supports_system_messages: true,
            max_context_tokens: 200_000,
            max_output_tokens: 4_096,
            models: vec![
                "anthropic.claude-3-opus-20240229-v1:0".to_string(),
                "anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
                "anthropic.claude-3-haiku-20240307-v1:0".to_string(),
            ],
            rate_limits: crate::RateLimitInfo {
                requests_per_minute: Some(100),
                tokens_per_minute: Some(200_000),
                concurrent_requests: Some(10),
            },
        };

        let rate_limiter = RateLimiter::new(RateLimitConfig {
            requests_per_minute: Some(100),
            tokens_per_minute: Some(200_000),
        });

        Self {
            provider_id: "bedrock".to_string(),
            client,
            config,
            capabilities,
            rate_limiter,
        }
    }
}

#[async_trait]
impl LLMProvider for BedrockProvider {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        // Bedrock doesn't have a health endpoint
        // We'll consider it healthy if we can initialize the client
        Ok(HealthStatus {
            is_healthy: true,
            latency_ms: Some(0),
            error_rate: 0.0,
            last_check: Instant::now(),
            details: HashMap::new(),
        })
    }

    async fn check_rate_limit(&self) -> Option<Duration> {
        self.rate_limiter.check_and_consume(1000).await
    }

    fn transform_request(&self, request: &GatewayRequest) -> Result<Bytes> {
        // Transform to Bedrock format (similar to Anthropic)
        // Model-specific formatting
        todo!("Implement Bedrock request transformation")
    }

    fn transform_response(&self, response: Bytes) -> Result<GatewayResponse> {
        todo!("Implement Bedrock response transformation")
    }

    async fn chat_completion(&self, request: &GatewayRequest) -> Result<GatewayResponse> {
        self.validate_request(request)?;

        // Use AWS SDK for Bedrock
        // Invoke model with converse API
        todo!("Implement Bedrock chat completion")
    }

    async fn chat_completion_stream(
        &self,
        _request: &GatewayRequest,
    ) -> Result<impl futures::stream::Stream<Item = Result<ChatChunk>> + Send> {
        todo!("Implement Bedrock streaming")
    }
}

// ============================================================================
// SECTION 5: Azure OpenAI Provider
// ============================================================================

pub struct AzureOpenAIProvider {
    provider_id: String,
    client: Arc<ConnectionPool>,
    config: AzureOpenAIConfig,
    capabilities: ProviderCapabilities,
    rate_limiter: RateLimiter,
    metrics: Arc<RwLock<ProviderMetrics>>,
}

#[derive(Debug, Clone)]
pub struct AzureOpenAIConfig {
    pub api_key: String,
    pub endpoint: String, // e.g., https://YOUR_RESOURCE.openai.azure.com
    pub deployment_name: String,
    pub api_version: String,
    pub timeout: Duration,
}

impl Default for AzureOpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            endpoint: String::new(),
            deployment_name: String::new(),
            api_version: "2024-02-15-preview".to_string(),
            timeout: Duration::from_secs(120),
        }
    }
}

impl AzureOpenAIProvider {
    pub fn new(config: AzureOpenAIConfig, connection_pool: Arc<ConnectionPool>) -> Self {
        let capabilities = ProviderCapabilities {
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: true,
            supports_system_messages: true,
            max_context_tokens: 128_000,
            max_output_tokens: 4_096,
            models: vec![config.deployment_name.clone()],
            rate_limits: crate::RateLimitInfo {
                requests_per_minute: Some(300),
                tokens_per_minute: Some(120_000),
                concurrent_requests: Some(50),
            },
        };

        let rate_limiter = RateLimiter::new(RateLimitConfig {
            requests_per_minute: Some(300),
            tokens_per_minute: Some(120_000),
        });

        Self {
            provider_id: "azure-openai".to_string(),
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
impl LLMProvider for AzureOpenAIProvider {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        // Azure OpenAI health check
        Ok(HealthStatus {
            is_healthy: true,
            latency_ms: Some(0),
            error_rate: 0.0,
            last_check: Instant::now(),
            details: HashMap::new(),
        })
    }

    async fn check_rate_limit(&self) -> Option<Duration> {
        self.rate_limiter.check_and_consume(1000).await
    }

    fn transform_request(&self, request: &GatewayRequest) -> Result<Bytes> {
        // Use OpenAI format (Azure OpenAI is compatible)
        crate::OpenAIProvider::transform_openai_request(request)
    }

    fn transform_response(&self, response: Bytes) -> Result<GatewayResponse> {
        crate::OpenAIProvider::transform_openai_response(response, &self.provider_id)
    }

    async fn chat_completion(&self, request: &GatewayRequest) -> Result<GatewayResponse> {
        self.validate_request(request)?;

        if let Some(wait_duration) = self.check_rate_limit().await {
            tokio::time::sleep(wait_duration).await;
        }

        let body = self.transform_request(request)?;

        // Azure OpenAI URL format
        let url = format!(
            "{}/openai/deployments/{}/chat/completions?api-version={}",
            self.config.endpoint,
            self.config.deployment_name,
            self.config.api_version
        );

        let http_request = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(&url)
            .header("Content-Type", "application/json")
            .header("api-key", &self.config.api_key)
            .body(hyper::Body::from(body))
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let _permit = self.client.acquire_permit(&self.provider_id).await?;

        let response = tokio::time::timeout(
            self.config.timeout,
            self.client.client().request(http_request)
        )
        .await
        .map_err(|_| ProviderError::Timeout("Request timeout".to_string()))?
        .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let body_bytes = hyper::body::to_bytes(response.into_body())
                .await
                .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

            return Err(ProviderError::ProviderInternalError(
                String::from_utf8_lossy(&body_bytes).to_string()
            ));
        }

        let body_bytes = hyper::body::to_bytes(response.into_body())
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        self.transform_response(body_bytes)
    }

    async fn chat_completion_stream(
        &self,
        _request: &GatewayRequest,
    ) -> Result<impl futures::stream::Stream<Item = Result<ChatChunk>> + Send> {
        // Similar to OpenAI streaming
        todo!("Implement Azure OpenAI streaming")
    }
}

// ============================================================================
// SECTION 6: Together AI Provider (OpenAI-Compatible)
// ============================================================================

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

impl Default for TogetherConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.together.xyz".to_string(),
            timeout: Duration::from_secs(120),
        }
    }
}

impl TogetherProvider {
    pub fn new(config: TogetherConfig, connection_pool: Arc<ConnectionPool>) -> Self {
        let capabilities = ProviderCapabilities {
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: false,
            supports_system_messages: true,
            max_context_tokens: 8192,
            max_output_tokens: 2048,
            models: vec![
                "meta-llama/Llama-3-70b-chat-hf".to_string(),
                "mistralai/Mixtral-8x7B-Instruct-v0.1".to_string(),
            ],
            rate_limits: crate::RateLimitInfo {
                requests_per_minute: Some(600),
                tokens_per_minute: Some(1_000_000),
                concurrent_requests: Some(100),
            },
        };

        let rate_limiter = RateLimiter::new(RateLimitConfig {
            requests_per_minute: Some(600),
            tokens_per_minute: Some(1_000_000),
        });

        Self {
            provider_id: "together".to_string(),
            client: connection_pool,
            config,
            capabilities,
            rate_limiter,
        }
    }
}

#[async_trait]
impl LLMProvider for TogetherProvider {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus {
            is_healthy: true,
            latency_ms: Some(0),
            error_rate: 0.0,
            last_check: Instant::now(),
            details: HashMap::new(),
        })
    }

    async fn check_rate_limit(&self) -> Option<Duration> {
        self.rate_limiter.check_and_consume(1000).await
    }

    fn transform_request(&self, request: &GatewayRequest) -> Result<Bytes> {
        // Together AI uses OpenAI-compatible format
        crate::OpenAIProvider::transform_openai_request(request)
    }

    fn transform_response(&self, response: Bytes) -> Result<GatewayResponse> {
        crate::OpenAIProvider::transform_openai_response(response, &self.provider_id)
    }

    async fn chat_completion(&self, request: &GatewayRequest) -> Result<GatewayResponse> {
        self.validate_request(request)?;

        if let Some(wait_duration) = self.check_rate_limit().await {
            tokio::time::sleep(wait_duration).await;
        }

        let body = self.transform_request(request)?;
        let url = format!("{}/v1/chat/completions", self.config.base_url);

        let http_request = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .body(hyper::Body::from(body))
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let _permit = self.client.acquire_permit(&self.provider_id).await?;

        let response = tokio::time::timeout(
            self.config.timeout,
            self.client.client().request(http_request)
        )
        .await
        .map_err(|_| ProviderError::Timeout("Request timeout".to_string()))?
        .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let body_bytes = hyper::body::to_bytes(response.into_body())
                .await
                .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

            return Err(ProviderError::ProviderInternalError(
                String::from_utf8_lossy(&body_bytes).to_string()
            ));
        }

        let body_bytes = hyper::body::to_bytes(response.into_body())
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        self.transform_response(body_bytes)
    }

    async fn chat_completion_stream(
        &self,
        _request: &GatewayRequest,
    ) -> Result<impl futures::stream::Stream<Item = Result<ChatChunk>> + Send> {
        todo!("Implement Together AI streaming")
    }
}
