//! Azure OpenAI provider implementation.
//!
//! Supports Azure OpenAI Service with deployment-based model access.
//! Key differences from OpenAI:
//! - URL structure: `{resource}.openai.azure.com/openai/deployments/{deployment}/chat/completions`
//! - Authentication via API key in `api-key` header
//! - API version required as query parameter

use async_stream::try_stream;
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures_util::StreamExt;
use gateway_core::{
    ChatChunk, ChatMessage, Choice, ChunkChoice, ChunkDelta, FinishReason, FunctionCall,
    GatewayError, GatewayRequest, GatewayResponse, HealthStatus, LLMProvider, MessageContent,
    MessageRole, ModelInfo, ProviderCapabilities, ProviderType, ToolCall, Usage,
};
use gateway_core::response::ResponseMessage;
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, trace, warn};

/// Azure OpenAI API version
pub const DEFAULT_API_VERSION: &str = "2024-02-15-preview";

/// Azure OpenAI provider configuration
#[derive(Debug, Clone)]
pub struct AzureOpenAIConfig {
    /// Provider instance ID
    pub id: String,
    /// API key
    pub api_key: SecretString,
    /// Azure resource name (e.g., "my-resource")
    pub resource_name: String,
    /// API version (default: 2024-02-15-preview)
    pub api_version: String,
    /// Request timeout
    pub timeout: Duration,
    /// Deployment to model mapping
    pub deployments: HashMap<String, ModelInfo>,
    /// Whether to use Azure Active Directory authentication
    pub use_aad: bool,
    /// Custom domain (if using private endpoint)
    pub custom_domain: Option<String>,
}

impl AzureOpenAIConfig {
    /// Create a new Azure OpenAI configuration
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        resource_name: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            api_key: SecretString::new(api_key.into()),
            resource_name: resource_name.into(),
            api_version: DEFAULT_API_VERSION.to_string(),
            timeout: Duration::from_secs(120),
            deployments: HashMap::new(),
            use_aad: false,
            custom_domain: None,
        }
    }

    /// Set the API version
    #[must_use]
    pub fn with_api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = version.into();
        self
    }

    /// Set the timeout
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Add a deployment mapping
    #[must_use]
    pub fn with_deployment(mut self, deployment_name: impl Into<String>, model: ModelInfo) -> Self {
        self.deployments.insert(deployment_name.into(), model);
        self
    }

    /// Add multiple deployments
    #[must_use]
    pub fn with_deployments(mut self, deployments: HashMap<String, ModelInfo>) -> Self {
        self.deployments.extend(deployments);
        self
    }

    /// Set custom domain (for private endpoints)
    #[must_use]
    pub fn with_custom_domain(mut self, domain: impl Into<String>) -> Self {
        self.custom_domain = Some(domain.into());
        self
    }

    /// Enable Azure AD authentication
    #[must_use]
    pub fn with_aad_auth(mut self) -> Self {
        self.use_aad = true;
        self
    }

    /// Get the base URL for the Azure OpenAI resource
    #[must_use]
    pub fn base_url(&self) -> String {
        if let Some(ref domain) = self.custom_domain {
            format!("https://{domain}")
        } else {
            format!("https://{}.openai.azure.com", self.resource_name)
        }
    }

    /// Create default GPT-4 deployment
    #[must_use]
    pub fn gpt4_deployment(deployment_name: impl Into<String>) -> (String, ModelInfo) {
        let name = deployment_name.into();
        (
            name.clone(),
            ModelInfo::new(&name)
                .with_name("GPT-4")
                .with_context_length(8_192)
                .with_max_output_tokens(8_192)
                .with_pricing(0.03, 0.06),
        )
    }

    /// Create default GPT-4 Turbo deployment
    #[must_use]
    pub fn gpt4_turbo_deployment(deployment_name: impl Into<String>) -> (String, ModelInfo) {
        let name = deployment_name.into();
        (
            name.clone(),
            ModelInfo::new(&name)
                .with_name("GPT-4 Turbo")
                .with_context_length(128_000)
                .with_max_output_tokens(4_096)
                .with_pricing(0.01, 0.03),
        )
    }

    /// Create default GPT-4o deployment
    #[must_use]
    pub fn gpt4o_deployment(deployment_name: impl Into<String>) -> (String, ModelInfo) {
        let name = deployment_name.into();
        (
            name.clone(),
            ModelInfo::new(&name)
                .with_name("GPT-4o")
                .with_context_length(128_000)
                .with_max_output_tokens(16_384)
                .with_pricing(0.005, 0.015),
        )
    }

    /// Create default GPT-3.5 Turbo deployment
    #[must_use]
    pub fn gpt35_turbo_deployment(deployment_name: impl Into<String>) -> (String, ModelInfo) {
        let name = deployment_name.into();
        (
            name.clone(),
            ModelInfo::new(&name)
                .with_name("GPT-3.5 Turbo")
                .with_context_length(16_385)
                .with_max_output_tokens(4_096)
                .with_pricing(0.0005, 0.0015),
        )
    }
}

/// Azure OpenAI provider implementation
pub struct AzureOpenAIProvider {
    config: AzureOpenAIConfig,
    client: Client,
    capabilities: ProviderCapabilities,
    /// Stored models for the trait implementation
    models: Vec<ModelInfo>,
    /// Base URL string for the trait
    base_url_string: String,
}

impl AzureOpenAIProvider {
    /// Create a new Azure OpenAI provider
    ///
    /// # Errors
    /// Returns error if HTTP client cannot be created
    pub fn new(config: AzureOpenAIConfig) -> Result<Self, GatewayError> {
        if config.deployments.is_empty() {
            return Err(GatewayError::Configuration {
                message: "At least one deployment must be configured for Azure OpenAI".to_string(),
            });
        }

        let client = Client::builder()
            .timeout(config.timeout)
            .pool_max_idle_per_host(100)
            .build()
            .map_err(|e| GatewayError::internal(format!("Failed to create HTTP client: {e}")))?;

        let models: Vec<ModelInfo> = config.deployments.values().cloned().collect();
        let base_url_string = config.base_url();

        Ok(Self {
            config,
            client,
            capabilities: ProviderCapabilities {
                chat: true,
                streaming: true,
                function_calling: true,
                vision: true,
                embeddings: true,
                json_mode: true,
                seed: true,
                logprobs: false,
                max_context_length: Some(128_000),
                max_output_tokens: Some(16_384),
                parallel_tool_calls: true,
            },
            models,
            base_url_string,
        })
    }

    /// Get the chat completions endpoint URL for a deployment
    fn completions_url(&self, deployment: &str) -> String {
        format!(
            "{}/openai/deployments/{}/chat/completions?api-version={}",
            self.config.base_url(),
            deployment,
            self.config.api_version
        )
    }

    /// Find deployment for a model request
    fn find_deployment(&self, model: &str) -> Option<String> {
        // First, try exact match
        if self.config.deployments.contains_key(model) {
            return Some(model.to_string());
        }

        // Try to find by model name
        self.config
            .deployments
            .iter()
            .find(|(_, info)| info.id == model || info.name.as_deref() == Some(model))
            .map(|(deployment, _)| deployment.clone())
    }

    /// Transform gateway request to Azure OpenAI format
    fn transform_request(&self, request: &GatewayRequest) -> AzureOpenAIRequest {
        let messages: Vec<AzureMessage> = request
            .messages
            .iter()
            .map(AzureMessage::from_gateway_message)
            .collect();

        AzureOpenAIRequest {
            messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            top_p: request.top_p,
            frequency_penalty: request.frequency_penalty,
            presence_penalty: request.presence_penalty,
            stop: request.stop.clone(),
            stream: Some(request.stream),
            n: request.n,
            seed: request.seed,
            user: request.user.clone(),
            tools: request.tools.as_ref().map(|tools| {
                tools
                    .iter()
                    .map(|t| AzureTool {
                        tool_type: t.tool_type.clone(),
                        function: AzureFunction {
                            name: t.function.name.clone(),
                            description: t.function.description.clone(),
                            parameters: t.function.parameters.clone(),
                        },
                    })
                    .collect()
            }),
            tool_choice: request.tool_choice.as_ref().map(|tc| {
                match tc {
                    gateway_core::request::ToolChoice::String(s) => {
                        serde_json::Value::String(s.clone())
                    }
                    gateway_core::request::ToolChoice::Tool { tool_type, function } => {
                        serde_json::json!({
                            "type": tool_type,
                            "function": { "name": function.name }
                        })
                    }
                }
            }),
            response_format: request.response_format.as_ref().map(|rf| {
                serde_json::json!({"type": rf.format_type})
            }),
        }
    }

    /// Transform Azure response to gateway format
    fn transform_response(&self, response: AzureResponse, deployment: &str) -> GatewayResponse {
        let choices: Vec<Choice> = response
            .choices
            .into_iter()
            .map(|c| Choice {
                index: c.index,
                message: ResponseMessage {
                    role: MessageRole::Assistant,
                    content: c.message.content,
                    tool_calls: c.message.tool_calls.map(|calls| {
                        calls
                            .into_iter()
                            .map(|tc| ToolCall {
                                id: tc.id,
                                tool_type: tc.tool_type,
                                function: FunctionCall {
                                    name: tc.function.name,
                                    arguments: tc.function.arguments.unwrap_or_default(),
                                },
                            })
                            .collect()
                    }),
                    function_call: c.message.function_call.map(|fc| FunctionCall {
                        name: fc.name,
                        arguments: fc.arguments.unwrap_or_default(),
                    }),
                },
                finish_reason: c.finish_reason.and_then(|r| match r.as_str() {
                    "stop" => Some(FinishReason::Stop),
                    "length" => Some(FinishReason::Length),
                    "tool_calls" | "function_call" => Some(FinishReason::ToolCalls),
                    "content_filter" => Some(FinishReason::ContentFilter),
                    _ => None,
                }),
                logprobs: None,
            })
            .collect();

        GatewayResponse {
            id: response.id,
            object: "chat.completion".to_string(),
            created: response.created as i64,
            model: deployment.to_string(),
            choices,
            usage: Usage {
                prompt_tokens: response.usage.prompt_tokens,
                completion_tokens: response.usage.completion_tokens,
                total_tokens: response.usage.total_tokens,
            },
            system_fingerprint: response.system_fingerprint,
            provider: Some(self.config.id.clone()),
        }
    }

    /// Map Azure-specific error to gateway error
    fn map_azure_error(&self, status: u16, error: &AzureErrorResponse) -> GatewayError {
        let message = &error.error.message;
        let code = error.error.code.as_deref().unwrap_or("unknown");

        match status {
            400 => GatewayError::Validation {
                message: message.clone(),
                field: None,
                code: code.to_string(),
            },
            401 => GatewayError::authentication("Invalid API key"),
            403 => GatewayError::authentication("Access denied"),
            404 if code == "DeploymentNotFound" => {
                GatewayError::model_not_found("Deployment not found")
            }
            404 => GatewayError::model_not_found(message.clone()),
            429 => GatewayError::RateLimit {
                retry_after: Some(Duration::from_secs(60)),
                limit: None,
            },
            500 | 502 | 503 => GatewayError::provider(
                &self.config.id,
                message.clone(),
                Some(status),
                true,
            ),
            _ => GatewayError::provider(
                &self.config.id,
                format!("{code}: {message}"),
                Some(status),
                false,
            ),
        }
    }
}

#[async_trait]
impl LLMProvider for AzureOpenAIProvider {
    fn id(&self) -> &str {
        &self.config.id
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Azure
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn base_url(&self) -> &str {
        &self.base_url_string
    }

    async fn health_check(&self) -> HealthStatus {
        // Use the first deployment for health check
        let deployment = match self.config.deployments.keys().next() {
            Some(d) => d,
            None => return HealthStatus::Unhealthy,
        };

        let url = self.completions_url(deployment);

        let response = self
            .client
            .post(&url)
            .header("api-key", self.config.api_key.expose_secret())
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "messages": [{"role": "user", "content": "test"}],
                "max_tokens": 1
            }))
            .timeout(Duration::from_secs(10))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => HealthStatus::Healthy,
            Ok(resp) if resp.status().as_u16() == 429 => HealthStatus::Degraded,
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                warn!(
                    deployment = %deployment,
                    status = %status,
                    body = %body,
                    "Azure OpenAI health check failed"
                );
                HealthStatus::Unhealthy
            }
            Err(e) => {
                error!(error = %e, "Azure OpenAI health check error");
                HealthStatus::Unhealthy
            }
        }
    }

    async fn chat_completion(
        &self,
        request: &GatewayRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        let deployment = self.find_deployment(&request.model).ok_or_else(|| {
            GatewayError::model_not_found(format!(
                "No deployment found for model '{}' in Azure OpenAI provider '{}'",
                request.model, self.config.id
            ))
        })?;

        let url = self.completions_url(&deployment);
        let azure_request = self.transform_request(request);

        debug!(
            deployment = %deployment,
            url = %url,
            "Sending request to Azure OpenAI"
        );

        let response = self
            .client
            .post(&url)
            .header("api-key", self.config.api_key.expose_secret())
            .header("Content-Type", "application/json")
            .json(&azure_request)
            .send()
            .await
            .map_err(|e| GatewayError::provider(&self.config.id, format!("Request failed: {e}"), None, true))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            // Parse Azure error
            if let Ok(error) = serde_json::from_str::<AzureErrorResponse>(&body) {
                return Err(self.map_azure_error(status.as_u16(), &error));
            }

            return Err(GatewayError::provider(
                &self.config.id,
                format!("Azure OpenAI API error: {} - {}", status, body),
                Some(status.as_u16()),
                status.is_server_error(),
            ));
        }

        let azure_response: AzureResponse = response
            .json()
            .await
            .map_err(|e| GatewayError::provider(&self.config.id, format!("Failed to parse response: {e}"), None, false))?;

        Ok(self.transform_response(azure_response, &deployment))
    }

    async fn chat_completion_stream(
        &self,
        request: &GatewayRequest,
    ) -> Result<BoxStream<'static, Result<ChatChunk, GatewayError>>, GatewayError> {
        let deployment = self.find_deployment(&request.model).ok_or_else(|| {
            GatewayError::model_not_found(format!(
                "No deployment found for model '{}' in Azure OpenAI provider '{}'",
                request.model, self.config.id
            ))
        })?;

        let url = self.completions_url(&deployment);
        let mut azure_request = self.transform_request(request);
        azure_request.stream = Some(true);

        debug!(
            deployment = %deployment,
            "Starting streaming request to Azure OpenAI"
        );

        let request_builder = self
            .client
            .post(&url)
            .header("api-key", self.config.api_key.expose_secret())
            .header("Content-Type", "application/json")
            .json(&azure_request);

        let event_source = EventSource::new(request_builder)
            .map_err(|e| GatewayError::provider(&self.config.id, format!("Failed to create event source: {e}"), None, true))?;

        let deployment_owned = deployment.to_string();
        let provider_id = self.config.id.clone();

        let stream = try_stream! {
            let mut es = event_source;

            while let Some(event) = es.next().await {
                match event {
                    Ok(Event::Open) => {
                        trace!("Azure OpenAI stream opened");
                    }
                    Ok(Event::Message(msg)) => {
                        let data = msg.data.trim();

                        // Check for stream end
                        if data == "[DONE]" {
                            break;
                        }

                        // Parse chunk
                        match serde_json::from_str::<AzureChunk>(data) {
                            Ok(chunk) => {
                                let gateway_chunk = ChatChunk {
                                    id: chunk.id,
                                    object: "chat.completion.chunk".to_string(),
                                    created: chunk.created as i64,
                                    model: deployment_owned.clone(),
                                    choices: chunk.choices.into_iter().map(|c| ChunkChoice {
                                        index: c.index,
                                        delta: ChunkDelta {
                                            role: c.delta.role.and_then(|r| match r.as_str() {
                                                "assistant" => Some(MessageRole::Assistant),
                                                "user" => Some(MessageRole::User),
                                                "system" => Some(MessageRole::System),
                                                _ => None,
                                            }),
                                            content: c.delta.content,
                                            tool_calls: None,
                                            function_call: None,
                                        },
                                        finish_reason: c.finish_reason.and_then(|r| match r.as_str() {
                                            "stop" => Some(FinishReason::Stop),
                                            "length" => Some(FinishReason::Length),
                                            "tool_calls" | "function_call" => Some(FinishReason::ToolCalls),
                                            "content_filter" => Some(FinishReason::ContentFilter),
                                            _ => None,
                                        }),
                                        logprobs: None,
                                    }).collect(),
                                    system_fingerprint: chunk.system_fingerprint,
                                    usage: None,
                                };
                                yield gateway_chunk;
                            }
                            Err(e) => {
                                warn!(error = %e, data = %data, "Failed to parse Azure chunk");
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Azure OpenAI stream error");
                        Err(GatewayError::streaming(format!("Stream error: {e}")))?;
                    }
                }
            }
        };

        // Silence unused variable warning
        let _ = provider_id;

        Ok(Box::pin(stream))
    }
}

// ============================================================================
// Azure OpenAI API Types
// ============================================================================

#[derive(Debug, Serialize)]
struct AzureOpenAIRequest {
    messages: Vec<AzureMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AzureTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AzureMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<AzureContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<AzureToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

impl AzureMessage {
    fn from_gateway_message(msg: &ChatMessage) -> Self {
        let role = match msg.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };

        let content = match &msg.content {
            MessageContent::Text(text) => Some(AzureContent::String(text.clone())),
            MessageContent::Parts(parts) => {
                let azure_parts: Vec<AzureContentPart> = parts
                    .iter()
                    .map(|p| match p {
                        gateway_core::request::ContentPart::Text { text } => {
                            AzureContentPart::Text {
                                text: text.clone(),
                            }
                        }
                        gateway_core::request::ContentPart::ImageUrl { image_url } => {
                            AzureContentPart::ImageUrl {
                                image_url: AzureImageUrl {
                                    url: image_url.url.clone(),
                                    detail: image_url.detail.map(|d| match d {
                                        gateway_core::request::ImageDetail::Auto => "auto".to_string(),
                                        gateway_core::request::ImageDetail::Low => "low".to_string(),
                                        gateway_core::request::ImageDetail::High => "high".to_string(),
                                    }),
                                },
                            }
                        }
                    })
                    .collect();
                Some(AzureContent::Parts(azure_parts))
            }
        };

        Self {
            role: role.to_string(),
            content,
            name: msg.name.clone(),
            tool_calls: msg.tool_calls.as_ref().map(|calls| {
                calls
                    .iter()
                    .map(|tc| AzureToolCall {
                        id: tc.id.clone(),
                        tool_type: tc.tool_type.clone(),
                        function: AzureFunctionCall {
                            name: tc.function.name.clone(),
                            arguments: Some(tc.function.arguments.clone()),
                        },
                    })
                    .collect()
            }),
            tool_call_id: msg.tool_call_id.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum AzureContent {
    String(String),
    Parts(Vec<AzureContentPart>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum AzureContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: AzureImageUrl },
}

#[derive(Debug, Serialize, Deserialize)]
struct AzureImageUrl {
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Debug, Serialize)]
struct AzureTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: AzureFunction,
}

#[derive(Debug, Serialize)]
struct AzureFunction {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AzureToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: AzureFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct AzureFunctionCall {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AzureResponse {
    id: String,
    created: u64,
    choices: Vec<AzureChoice>,
    usage: AzureUsage,
    #[serde(default)]
    system_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AzureChoice {
    index: u32,
    message: AzureResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AzureResponseMessage {
    #[allow(dead_code)]
    role: String,
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<AzureToolCall>>,
    #[serde(default)]
    function_call: Option<AzureFunctionCall>,
}

#[derive(Debug, Deserialize)]
struct AzureUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct AzureChunk {
    id: String,
    created: u64,
    choices: Vec<AzureChunkChoice>,
    #[serde(default)]
    system_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AzureChunkChoice {
    index: u32,
    delta: AzureChunkDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AzureChunkDelta {
    role: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AzureErrorResponse {
    error: AzureError,
}

#[derive(Debug, Deserialize)]
struct AzureError {
    message: String,
    #[serde(default)]
    code: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "type", default)]
    error_type: Option<String>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = AzureOpenAIConfig::new("azure-1", "my-resource", "test-key");
        assert_eq!(config.id, "azure-1");
        assert_eq!(config.resource_name, "my-resource");
        assert_eq!(config.api_version, DEFAULT_API_VERSION);
    }

    #[test]
    fn test_config_builder() {
        let config = AzureOpenAIConfig::new("azure-1", "my-resource", "test-key")
            .with_api_version("2023-12-01-preview")
            .with_timeout(Duration::from_secs(60))
            .with_custom_domain("my-private-endpoint.azure.com");

        assert_eq!(config.api_version, "2023-12-01-preview");
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(
            config.custom_domain,
            Some("my-private-endpoint.azure.com".to_string())
        );
    }

    #[test]
    fn test_base_url() {
        let config = AzureOpenAIConfig::new("azure-1", "my-resource", "test-key");
        assert_eq!(config.base_url(), "https://my-resource.openai.azure.com");

        let config_with_custom = config.with_custom_domain("custom.endpoint.com");
        assert_eq!(config_with_custom.base_url(), "https://custom.endpoint.com");
    }

    #[test]
    fn test_deployment_helpers() {
        let (name, model) = AzureOpenAIConfig::gpt4_deployment("gpt4-deployment");
        assert_eq!(name, "gpt4-deployment");
        assert_eq!(model.name, Some("GPT-4".to_string()));

        let (name, model) = AzureOpenAIConfig::gpt4o_deployment("gpt4o-deployment");
        assert_eq!(name, "gpt4o-deployment");
        assert_eq!(model.context_length, Some(128_000));
    }

    #[test]
    fn test_config_with_deployment() {
        let (name, model) = AzureOpenAIConfig::gpt4_deployment("my-gpt4");
        let config = AzureOpenAIConfig::new("azure-1", "my-resource", "test-key")
            .with_deployment(name, model);

        assert_eq!(config.deployments.len(), 1);
        assert!(config.deployments.contains_key("my-gpt4"));
    }

    #[test]
    fn test_provider_creation_without_deployments() {
        let config = AzureOpenAIConfig::new("azure-1", "my-resource", "test-key");
        let result = AzureOpenAIProvider::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_creation_with_deployment() {
        let (name, model) = AzureOpenAIConfig::gpt4_deployment("my-gpt4");
        let config = AzureOpenAIConfig::new("azure-1", "my-resource", "test-key")
            .with_deployment(name, model);
        let result = AzureOpenAIProvider::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_completions_url() {
        let (name, model) = AzureOpenAIConfig::gpt4_deployment("my-gpt4");
        let config = AzureOpenAIConfig::new("azure-1", "my-resource", "test-key")
            .with_deployment(name, model);
        let provider = AzureOpenAIProvider::new(config).unwrap();

        let url = provider.completions_url("my-gpt4");
        assert!(url.contains("my-resource.openai.azure.com"));
        assert!(url.contains("/openai/deployments/my-gpt4/"));
        assert!(url.contains("api-version="));
    }

    #[test]
    fn test_find_deployment() {
        let config = AzureOpenAIConfig::new("azure-1", "my-resource", "test-key")
            .with_deployment(
                "gpt4-prod".to_string(),
                ModelInfo::new("gpt4-prod").with_name("GPT-4 Production"),
            );
        let provider = AzureOpenAIProvider::new(config).unwrap();

        // Exact match
        assert_eq!(provider.find_deployment("gpt4-prod"), Some("gpt4-prod".to_string()));

        // No match
        assert_eq!(provider.find_deployment("gpt-3.5"), None);
    }

    #[test]
    fn test_provider_type() {
        let (name, model) = AzureOpenAIConfig::gpt4_deployment("my-gpt4");
        let config = AzureOpenAIConfig::new("azure-1", "my-resource", "test-key")
            .with_deployment(name, model);
        let provider = AzureOpenAIProvider::new(config).unwrap();

        assert_eq!(provider.provider_type(), ProviderType::Azure);
        assert_eq!(provider.id(), "azure-1");
    }

    #[test]
    fn test_capabilities() {
        let (name, model) = AzureOpenAIConfig::gpt4_deployment("my-gpt4");
        let config = AzureOpenAIConfig::new("azure-1", "my-resource", "test-key")
            .with_deployment(name, model);
        let provider = AzureOpenAIProvider::new(config).unwrap();

        let caps = provider.capabilities();
        assert!(caps.chat);
        assert!(caps.streaming);
        assert!(caps.function_calling);
        assert!(caps.vision);
    }

    #[test]
    fn test_models_list() {
        let config = AzureOpenAIConfig::new("azure-1", "my-resource", "test-key")
            .with_deployment(
                "gpt4-prod".to_string(),
                ModelInfo::new("gpt4-prod").with_name("GPT-4"),
            )
            .with_deployment(
                "gpt35-prod".to_string(),
                ModelInfo::new("gpt35-prod").with_name("GPT-3.5"),
            );
        let provider = AzureOpenAIProvider::new(config).unwrap();

        let models = provider.models();
        assert_eq!(models.len(), 2);
    }

    #[test]
    fn test_transform_message() {
        let msg = ChatMessage {
            role: MessageRole::User,
            content: MessageContent::Text("Hello".to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        };

        let azure_msg = AzureMessage::from_gateway_message(&msg);
        assert_eq!(azure_msg.role, "user");
        assert!(matches!(azure_msg.content, Some(AzureContent::String(_))));
    }

    #[test]
    fn test_base_url_method() {
        let (name, model) = AzureOpenAIConfig::gpt4_deployment("my-gpt4");
        let config = AzureOpenAIConfig::new("azure-1", "my-resource", "test-key")
            .with_deployment(name, model);
        let provider = AzureOpenAIProvider::new(config).unwrap();

        assert!(provider.base_url().contains("my-resource.openai.azure.com"));
    }
}
