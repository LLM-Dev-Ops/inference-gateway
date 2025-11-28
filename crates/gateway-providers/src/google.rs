//! Google Vertex AI / Gemini provider implementation.
//!
//! Supports Google's Gemini models via the Vertex AI API and Google AI Studio API.
//!
//! # API Formats
//! - Vertex AI: `https://{LOCATION}-aiplatform.googleapis.com/v1/projects/{PROJECT}/locations/{LOCATION}/publishers/google/models/{MODEL}:streamGenerateContent`
//! - Google AI Studio: `https://generativelanguage.googleapis.com/v1beta/models/{MODEL}:generateContent`

use async_stream::try_stream;
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures_util::StreamExt;
use gateway_core::{
    ChatChunk, Choice, ChunkChoice, ChunkDelta, FinishReason, GatewayError, GatewayRequest,
    GatewayResponse, HealthStatus, LLMProvider, MessageContent, MessageRole, ModelInfo,
    ProviderCapabilities, ProviderType, Usage,
};
use gateway_core::request::ContentPart;
use gateway_core::response::ResponseMessage;
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error, trace, warn};

/// Google provider API type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GoogleApiType {
    /// Google AI Studio (generativelanguage.googleapis.com)
    #[default]
    GoogleAI,
    /// Vertex AI (aiplatform.googleapis.com)
    VertexAI,
}

/// Google provider configuration
#[derive(Debug, Clone)]
pub struct GoogleConfig {
    /// Provider instance ID
    pub id: String,
    /// API key (for Google AI Studio)
    pub api_key: Option<SecretString>,
    /// Access token (for Vertex AI - usually from service account)
    pub access_token: Option<SecretString>,
    /// API type (Google AI or Vertex AI)
    pub api_type: GoogleApiType,
    /// Google Cloud project ID (required for Vertex AI)
    pub project_id: Option<String>,
    /// Google Cloud location (required for Vertex AI)
    pub location: String,
    /// Request timeout
    pub timeout: Duration,
    /// Supported models
    pub models: Vec<ModelInfo>,
}

impl GoogleConfig {
    /// Create a new Google AI Studio configuration
    #[must_use]
    pub fn google_ai(id: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            api_key: Some(SecretString::new(api_key.into())),
            access_token: None,
            api_type: GoogleApiType::GoogleAI,
            project_id: None,
            location: "us-central1".to_string(),
            timeout: Duration::from_secs(120),
            models: Self::default_models(),
        }
    }

    /// Create a new Vertex AI configuration
    #[must_use]
    pub fn vertex_ai(
        id: impl Into<String>,
        project_id: impl Into<String>,
        location: impl Into<String>,
        access_token: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            api_key: None,
            access_token: Some(SecretString::new(access_token.into())),
            api_type: GoogleApiType::VertexAI,
            project_id: Some(project_id.into()),
            location: location.into(),
            timeout: Duration::from_secs(120),
            models: Self::default_models(),
        }
    }

    /// Set the timeout
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set custom models
    #[must_use]
    pub fn with_models(mut self, models: Vec<ModelInfo>) -> Self {
        self.models = models;
        self
    }

    /// Default Gemini models
    #[must_use]
    pub fn default_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo::new("gemini-1.5-pro")
                .with_name("Gemini 1.5 Pro")
                .with_context_length(2_097_152) // 2M tokens
                .with_max_output_tokens(8_192)
                .with_pricing(0.00125, 0.005), // $1.25/$5 per 1M tokens
            ModelInfo::new("gemini-1.5-flash")
                .with_name("Gemini 1.5 Flash")
                .with_context_length(1_048_576) // 1M tokens
                .with_max_output_tokens(8_192)
                .with_pricing(0.000075, 0.0003), // $0.075/$0.30 per 1M tokens
            ModelInfo::new("gemini-1.5-flash-8b")
                .with_name("Gemini 1.5 Flash 8B")
                .with_context_length(1_048_576)
                .with_max_output_tokens(8_192)
                .with_pricing(0.0000375, 0.00015),
            ModelInfo::new("gemini-1.0-pro")
                .with_name("Gemini 1.0 Pro")
                .with_context_length(32_760)
                .with_max_output_tokens(8_192)
                .with_pricing(0.0005, 0.0015),
            ModelInfo::new("gemini-pro")
                .with_name("Gemini Pro")
                .with_alias("gemini-1.0-pro")
                .with_context_length(32_760)
                .with_max_output_tokens(8_192)
                .with_pricing(0.0005, 0.0015),
            ModelInfo::new("gemini-pro-vision")
                .with_name("Gemini Pro Vision")
                .with_context_length(16_384)
                .with_max_output_tokens(2_048)
                .with_pricing(0.0005, 0.0015),
        ]
    }

    /// Get the base URL for the API
    fn base_url(&self) -> String {
        match self.api_type {
            GoogleApiType::GoogleAI => {
                "https://generativelanguage.googleapis.com/v1beta".to_string()
            }
            GoogleApiType::VertexAI => {
                let project = self.project_id.as_deref().unwrap_or("unknown");
                let location = &self.location;
                format!(
                    "https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models"
                )
            }
        }
    }
}

/// Google Gemini provider implementation
pub struct GoogleProvider {
    config: GoogleConfig,
    client: Client,
    capabilities: ProviderCapabilities,
    base_url_string: String,
}

impl GoogleProvider {
    /// Create a new Google provider
    ///
    /// # Errors
    /// Returns error if HTTP client cannot be created or configuration is invalid
    pub fn new(config: GoogleConfig) -> Result<Self, GatewayError> {
        // Validate configuration
        match config.api_type {
            GoogleApiType::GoogleAI => {
                if config.api_key.is_none() {
                    return Err(GatewayError::Configuration {
                        message: "API key is required for Google AI Studio".to_string(),
                    });
                }
            }
            GoogleApiType::VertexAI => {
                if config.project_id.is_none() {
                    return Err(GatewayError::Configuration {
                        message: "Project ID is required for Vertex AI".to_string(),
                    });
                }
                if config.access_token.is_none() {
                    return Err(GatewayError::Configuration {
                        message: "Access token is required for Vertex AI".to_string(),
                    });
                }
            }
        }

        let client = Client::builder()
            .timeout(config.timeout)
            .pool_max_idle_per_host(100)
            .build()
            .map_err(|e| GatewayError::internal(format!("Failed to create HTTP client: {e}")))?;

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
                seed: false, // Gemini doesn't support seed
                logprobs: false,
                max_context_length: Some(2_097_152),
                max_output_tokens: Some(8_192),
                parallel_tool_calls: true,
            },
            base_url_string,
        })
    }

    /// Build the endpoint URL for a model
    fn endpoint_url(&self, model: &str, streaming: bool) -> String {
        let action = if streaming {
            "streamGenerateContent"
        } else {
            "generateContent"
        };

        match self.config.api_type {
            GoogleApiType::GoogleAI => {
                let api_key = self
                    .config
                    .api_key
                    .as_ref()
                    .map(|k| k.expose_secret().as_str())
                    .unwrap_or("");
                format!(
                    "{}/models/{}:{}?key={}",
                    self.base_url_string, model, action, api_key
                )
            }
            GoogleApiType::VertexAI => {
                format!("{}/{}:{}", self.base_url_string, model, action)
            }
        }
    }

    /// Transform a gateway request to Google's format
    fn transform_request(&self, request: &GatewayRequest) -> GoogleRequest {
        let mut contents = Vec::new();
        let mut system_instruction = None;

        for message in &request.messages {
            match message.role {
                MessageRole::System => {
                    // Gemini uses system_instruction for system messages
                    let text = Self::extract_text_content(&message.content);
                    system_instruction = Some(GoogleContent {
                        role: None,
                        parts: vec![GooglePart::Text { text }],
                    });
                }
                MessageRole::User => {
                    contents.push(GoogleContent {
                        role: Some("user".to_string()),
                        parts: self.transform_content(&message.content),
                    });
                }
                MessageRole::Assistant => {
                    contents.push(GoogleContent {
                        role: Some("model".to_string()),
                        parts: self.transform_content(&message.content),
                    });
                }
                MessageRole::Tool => {
                    // Tool responses in Gemini
                    contents.push(GoogleContent {
                        role: Some("function".to_string()),
                        parts: self.transform_content(&message.content),
                    });
                }
            }
        }

        // Check if JSON mode is requested via response_format
        let json_mode = request
            .response_format
            .as_ref()
            .is_some_and(|f| f.format_type == "json_object");

        // Build generation config
        let generation_config = GoogleGenerationConfig {
            temperature: request.temperature,
            top_p: request.top_p,
            top_k: request.top_k.map(|k| k as i32),
            max_output_tokens: request.max_tokens,
            stop_sequences: request.stop.clone(),
            response_mime_type: if json_mode {
                Some("application/json".to_string())
            } else {
                None
            },
        };

        // Build tools if present
        let tools = request.tools.as_ref().map(|tools| {
            vec![GoogleTool {
                function_declarations: tools
                    .iter()
                    .map(|t| GoogleFunctionDeclaration {
                        name: t.function.name.clone(),
                        description: t.function.description.clone(),
                        parameters: t.function.parameters.clone(),
                    })
                    .collect(),
            }]
        });

        GoogleRequest {
            contents,
            system_instruction,
            generation_config: Some(generation_config),
            tools,
            safety_settings: None,
        }
    }

    /// Transform message content to Google parts
    fn transform_content(&self, content: &MessageContent) -> Vec<GooglePart> {
        match content {
            MessageContent::Text(text) => vec![GooglePart::Text {
                text: text.clone(),
            }],
            MessageContent::Parts(parts) => {
                parts
                    .iter()
                    .filter_map(|part| match part {
                        ContentPart::Text { text } => Some(GooglePart::Text { text: text.clone() }),
                        ContentPart::ImageUrl { image_url } => {
                            // Parse data URLs or external URLs
                            if let Some(data) = Self::parse_data_url(&image_url.url) {
                                Some(GooglePart::InlineData {
                                    inline_data: GoogleInlineData {
                                        mime_type: data.0,
                                        data: data.1,
                                    },
                                })
                            } else {
                                // External URL - Gemini requires inline data
                                warn!(
                                    url = %image_url.url,
                                    "External image URLs not supported by Gemini, skipping"
                                );
                                None
                            }
                        }
                    })
                    .collect()
            }
        }
    }

    /// Extract text from message content
    fn extract_text_content(content: &MessageContent) -> String {
        match content {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Parts(parts) => parts
                .iter()
                .filter_map(|p| match p {
                    ContentPart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// Parse a data URL into mime type and base64 data
    fn parse_data_url(url: &str) -> Option<(String, String)> {
        if !url.starts_with("data:") {
            return None;
        }

        let without_prefix = url.strip_prefix("data:")?;
        let (meta, data) = without_prefix.split_once(",")?;

        let mime_type = if meta.contains(";base64") {
            meta.strip_suffix(";base64")?.to_string()
        } else {
            meta.to_string()
        };

        Some((mime_type, data.to_string()))
    }

    /// Transform Google response to gateway format
    fn transform_response(
        &self,
        response: GoogleResponse,
        model: &str,
    ) -> Result<GatewayResponse, GatewayError> {
        let candidate = response.candidates.into_iter().next().ok_or_else(|| {
            GatewayError::provider("google", "No candidates in response", None, false)
        })?;

        let content = candidate
            .content
            .parts
            .iter()
            .filter_map(|p| match p {
                GooglePart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        let finish_reason = candidate
            .finish_reason
            .as_deref()
            .map(Self::map_finish_reason);

        // Extract tool calls if present
        let tool_calls: Vec<gateway_core::ToolCall> = candidate
            .content
            .parts
            .iter()
            .filter_map(|p| {
                if let GooglePart::FunctionCall { function_call } = p {
                    Some(gateway_core::ToolCall {
                        id: format!("call_{}", uuid::Uuid::new_v4()),
                        tool_type: "function".to_string(),
                        function: gateway_core::FunctionCall {
                            name: function_call.name.clone(),
                            arguments: serde_json::to_string(&function_call.args)
                                .unwrap_or_default(),
                        },
                    })
                } else {
                    None
                }
            })
            .collect();

        let message = if tool_calls.is_empty() {
            ResponseMessage {
                role: MessageRole::Assistant,
                content: Some(content),
                tool_calls: None,
                function_call: None,
            }
        } else {
            ResponseMessage {
                role: MessageRole::Assistant,
                content: if content.is_empty() {
                    None
                } else {
                    Some(content)
                },
                tool_calls: Some(tool_calls),
                function_call: None,
            }
        };

        let usage = response.usage_metadata.map(|u| Usage {
            prompt_tokens: u.prompt_token_count as u32,
            completion_tokens: u.candidates_token_count.unwrap_or(0) as u32,
            total_tokens: u.total_token_count.unwrap_or(
                u.prompt_token_count + u.candidates_token_count.unwrap_or(0),
            ) as u32,
        });

        Ok(GatewayResponse::builder()
            .id(format!("google-{}", uuid::Uuid::new_v4()))
            .model(model.to_string())
            .choice(Choice {
                index: 0,
                message,
                finish_reason,
                logprobs: None,
            })
            .usage(usage.unwrap_or_default())
            .build())
    }

    /// Map Google finish reason to gateway format
    fn map_finish_reason(reason: &str) -> FinishReason {
        match reason {
            "STOP" => FinishReason::Stop,
            "MAX_TOKENS" => FinishReason::Length,
            "SAFETY" => FinishReason::ContentFilter,
            "RECITATION" => FinishReason::ContentFilter,
            "OTHER" => FinishReason::Stop,
            _ => FinishReason::Stop,
        }
    }
}

#[async_trait]
impl LLMProvider for GoogleProvider {
    fn id(&self) -> &str {
        &self.config.id
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Google
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    fn models(&self) -> &[ModelInfo] {
        &self.config.models
    }

    fn base_url(&self) -> &str {
        &self.base_url_string
    }

    fn timeout(&self) -> Duration {
        self.config.timeout
    }

    async fn chat_completion(
        &self,
        request: &GatewayRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        let model = &request.model;
        let url = self.endpoint_url(model, false);

        let google_request = self.transform_request(request);

        debug!(
            provider = "google",
            model = %model,
            url = %url,
            "Sending chat completion request"
        );

        let mut req_builder = self.client.post(&url);

        // Add authorization header for Vertex AI
        if let GoogleApiType::VertexAI = self.config.api_type {
            if let Some(ref token) = self.config.access_token {
                req_builder = req_builder.bearer_auth(token.expose_secret());
            }
        }

        let response = req_builder
            .json(&google_request)
            .send()
            .await
            .map_err(|e| {
                error!(error = %e, "Google API request failed");
                GatewayError::provider("google", format!("Request failed: {e}"), None, true)
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|e| {
            GatewayError::provider("google", format!("Failed to read response: {e}"), None, false)
        })?;

        trace!(status = %status, body = %body, "Received Google response");

        if !status.is_success() {
            return Err(Self::parse_error(status.as_u16(), &body));
        }

        let google_response: GoogleResponse = serde_json::from_str(&body).map_err(|e| {
            GatewayError::provider("google", format!("Invalid response JSON: {e}"), None, false)
        })?;

        self.transform_response(google_response, model)
    }

    async fn chat_completion_stream(
        &self,
        request: &GatewayRequest,
    ) -> Result<BoxStream<'static, Result<ChatChunk, GatewayError>>, GatewayError> {
        let model = request.model.clone();
        let url = self.endpoint_url(&model, true);

        let google_request = self.transform_request(request);

        debug!(
            provider = "google",
            model = %model,
            url = %url,
            "Sending streaming chat completion request"
        );

        // Add alt=sse for server-sent events
        let url_with_sse = if url.contains('?') {
            format!("{url}&alt=sse")
        } else {
            format!("{url}?alt=sse")
        };

        let mut req_builder = self.client.post(&url_with_sse);

        if let GoogleApiType::VertexAI = self.config.api_type {
            if let Some(ref token) = self.config.access_token {
                req_builder = req_builder.bearer_auth(token.expose_secret());
            }
        }

        let response = req_builder
            .json(&google_request)
            .send()
            .await
            .map_err(|e| {
                error!(error = %e, "Google API streaming request failed");
                GatewayError::provider("google", format!("Streaming request failed: {e}"), None, true)
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Self::parse_error(status.as_u16(), &body));
        }

        // Create stream
        let stream = try_stream! {
            let mut byte_stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = byte_stream.next().await {
                let chunk = chunk_result.map_err(|e| {
                    GatewayError::provider("google", format!("Stream error: {e}"), None, false)
                })?;

                let text = String::from_utf8_lossy(&chunk);
                buffer.push_str(&text);

                // Process complete SSE events
                while let Some(pos) = buffer.find("\n\n") {
                    let event = buffer[..pos].to_string();
                    buffer = buffer[pos + 2..].to_string();

                    // Parse SSE data
                    for line in event.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                return;
                            }

                            // Parse the JSON chunk
                            if let Ok(response) = serde_json::from_str::<GoogleResponse>(data) {
                                if let Some(candidate) = response.candidates.into_iter().next() {
                                    let content = candidate.content.parts.iter()
                                        .filter_map(|p| match p {
                                            GooglePart::Text { text } => Some(text.clone()),
                                            _ => None,
                                        })
                                        .collect::<Vec<_>>()
                                        .join("");

                                    let finish_reason = candidate.finish_reason
                                        .as_deref()
                                        .map(Self::map_finish_reason);

                                    let chunk = ChatChunk::builder()
                                        .id(format!("google-{}", uuid::Uuid::new_v4()))
                                        .model(model.clone())
                                        .choice(ChunkChoice {
                                            index: 0,
                                            delta: ChunkDelta {
                                                role: Some(MessageRole::Assistant),
                                                content: if content.is_empty() { None } else { Some(content) },
                                                tool_calls: None,
                                                function_call: None,
                                            },
                                            finish_reason,
                                            logprobs: None,
                                        })
                                        .build();

                                    yield chunk;
                                }
                            }
                        }
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    async fn health_check(&self) -> HealthStatus {
        // Simple health check - try to list models or make a minimal request
        let url = match self.config.api_type {
            GoogleApiType::GoogleAI => {
                let api_key = self
                    .config
                    .api_key
                    .as_ref()
                    .map(|k| k.expose_secret().as_str())
                    .unwrap_or("");
                format!(
                    "https://generativelanguage.googleapis.com/v1beta/models?key={}",
                    api_key
                )
            }
            GoogleApiType::VertexAI => {
                // For Vertex AI, just check if we can reach the endpoint
                format!("{}/gemini-1.5-flash", self.base_url_string)
            }
        };

        let mut req = self.client.get(&url);

        if let GoogleApiType::VertexAI = self.config.api_type {
            if let Some(ref token) = self.config.access_token {
                req = req.bearer_auth(token.expose_secret());
            }
        }

        match req.timeout(Duration::from_secs(10)).send().await {
            Ok(response) if response.status().is_success() => HealthStatus::Healthy,
            Ok(response) if response.status().as_u16() == 429 => HealthStatus::Degraded,
            Ok(_) => HealthStatus::Unhealthy,
            Err(_) => HealthStatus::Unhealthy,
        }
    }
}

impl GoogleProvider {
    /// Parse error response
    fn parse_error(status: u16, body: &str) -> GatewayError {
        // Try to parse Google error format
        #[derive(Deserialize)]
        struct GoogleErrorResponse {
            error: GoogleErrorDetail,
        }

        #[derive(Deserialize)]
        struct GoogleErrorDetail {
            message: String,
            #[allow(dead_code)]
            code: Option<i32>,
            #[allow(dead_code)]
            status: Option<String>,
        }

        if let Ok(error_response) = serde_json::from_str::<GoogleErrorResponse>(body) {
            let message = error_response.error.message;
            match status {
                400 => GatewayError::validation(&message, None, "google_bad_request"),
                401 | 403 => GatewayError::authentication(&message),
                404 => GatewayError::model_not_found(&message),
                429 => GatewayError::rate_limit(None, None),
                500..=599 => GatewayError::provider("google", message, Some(status), true),
                _ => GatewayError::provider("google", message, Some(status), false),
            }
        } else {
            GatewayError::provider("google", format!("HTTP {status}: {body}"), Some(status), false)
        }
    }
}

// Google API Types

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleRequest {
    contents: Vec<GoogleContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GoogleContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GoogleGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GoogleTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    safety_settings: Option<Vec<GoogleSafetySetting>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    parts: Vec<GooglePart>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum GooglePart {
    Text {
        text: String,
    },
    InlineData {
        #[serde(rename = "inlineData")]
        inline_data: GoogleInlineData,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GoogleFunctionCallData,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GoogleFunctionResponseData,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleInlineData {
    mime_type: String,
    data: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GoogleFunctionCallData {
    name: String,
    args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct GoogleFunctionResponseData {
    name: String,
    response: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_mime_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleTool {
    function_declarations: Vec<GoogleFunctionDeclaration>,
}

#[derive(Debug, Serialize)]
struct GoogleFunctionDeclaration {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct GoogleSafetySetting {
    category: String,
    threshold: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleResponse {
    candidates: Vec<GoogleCandidate>,
    #[serde(default)]
    usage_metadata: Option<GoogleUsageMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleCandidate {
    content: GoogleContent,
    #[serde(default)]
    finish_reason: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    safety_ratings: Option<Vec<GoogleSafetyRating>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleUsageMetadata {
    prompt_token_count: i64,
    #[serde(default)]
    candidates_token_count: Option<i64>,
    #[serde(default)]
    total_token_count: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct GoogleSafetyRating {
    category: String,
    probability: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_google_ai_config() {
        let config = GoogleConfig::google_ai("google-1", "test-key");

        assert_eq!(config.id, "google-1");
        assert!(config.api_key.is_some());
        assert_eq!(config.api_type, GoogleApiType::GoogleAI);
        assert!(config.project_id.is_none());
    }

    #[test]
    fn test_vertex_ai_config() {
        let config =
            GoogleConfig::vertex_ai("vertex-1", "my-project", "us-central1", "test-token");

        assert_eq!(config.id, "vertex-1");
        assert!(config.access_token.is_some());
        assert_eq!(config.api_type, GoogleApiType::VertexAI);
        assert_eq!(config.project_id, Some("my-project".to_string()));
        assert_eq!(config.location, "us-central1");
    }

    #[test]
    fn test_default_models() {
        let models = GoogleConfig::default_models();

        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "gemini-1.5-pro"));
        assert!(models.iter().any(|m| m.id == "gemini-1.5-flash"));
    }

    #[test]
    fn test_base_url_google_ai() {
        let config = GoogleConfig::google_ai("google-1", "test-key");
        assert!(config
            .base_url()
            .contains("generativelanguage.googleapis.com"));
    }

    #[test]
    fn test_base_url_vertex_ai() {
        let config =
            GoogleConfig::vertex_ai("vertex-1", "my-project", "us-central1", "test-token");
        let url = config.base_url();

        assert!(url.contains("us-central1-aiplatform.googleapis.com"));
        assert!(url.contains("my-project"));
    }

    #[test]
    fn test_parse_data_url() {
        let result =
            GoogleProvider::parse_data_url("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEA");
        assert!(result.is_some());
        let (mime, data) = result.unwrap();
        assert_eq!(mime, "image/png");
        assert_eq!(data, "iVBORw0KGgoAAAANSUhEUgAAAAEA");
    }

    #[test]
    fn test_parse_data_url_invalid() {
        assert!(GoogleProvider::parse_data_url("https://example.com/image.png").is_none());
    }

    #[test]
    fn test_map_finish_reason() {
        assert_eq!(
            GoogleProvider::map_finish_reason("STOP"),
            FinishReason::Stop
        );
        assert_eq!(
            GoogleProvider::map_finish_reason("MAX_TOKENS"),
            FinishReason::Length
        );
        assert_eq!(
            GoogleProvider::map_finish_reason("SAFETY"),
            FinishReason::ContentFilter
        );
    }

    #[test]
    fn test_provider_creation_google_ai() {
        let config = GoogleConfig::google_ai("google-1", "test-key");
        let provider = GoogleProvider::new(config);

        assert!(provider.is_ok());
        let provider = provider.unwrap();
        assert_eq!(provider.id(), "google-1");
        assert_eq!(provider.provider_type(), ProviderType::Google);
    }

    #[test]
    fn test_provider_creation_vertex_ai() {
        let config =
            GoogleConfig::vertex_ai("vertex-1", "my-project", "us-central1", "test-token");
        let provider = GoogleProvider::new(config);

        assert!(provider.is_ok());
        let provider = provider.unwrap();
        assert_eq!(provider.id(), "vertex-1");
    }

    #[test]
    fn test_provider_creation_missing_api_key() {
        let config = GoogleConfig {
            id: "test".to_string(),
            api_key: None,
            access_token: None,
            api_type: GoogleApiType::GoogleAI,
            project_id: None,
            location: "us-central1".to_string(),
            timeout: Duration::from_secs(120),
            models: vec![],
        };

        let result = GoogleProvider::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_creation_missing_project_id() {
        let config = GoogleConfig {
            id: "test".to_string(),
            api_key: None,
            access_token: Some(SecretString::new("token".into())),
            api_type: GoogleApiType::VertexAI,
            project_id: None,
            location: "us-central1".to_string(),
            timeout: Duration::from_secs(120),
            models: vec![],
        };

        let result = GoogleProvider::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_capabilities() {
        let config = GoogleConfig::google_ai("google-1", "test-key");
        let provider = GoogleProvider::new(config).unwrap();
        let caps = provider.capabilities();

        assert!(caps.chat);
        assert!(caps.streaming);
        assert!(caps.vision);
        assert!(caps.function_calling);
        assert!(!caps.seed); // Gemini doesn't support seed
    }

    #[test]
    fn test_endpoint_url_google_ai() {
        let config = GoogleConfig::google_ai("google-1", "test-key");
        let provider = GoogleProvider::new(config).unwrap();

        let url = provider.endpoint_url("gemini-1.5-pro", false);
        assert!(url.contains("generateContent"));
        assert!(url.contains("gemini-1.5-pro"));
        assert!(url.contains("key=test-key"));

        let stream_url = provider.endpoint_url("gemini-1.5-pro", true);
        assert!(stream_url.contains("streamGenerateContent"));
    }

    #[test]
    fn test_endpoint_url_vertex_ai() {
        let config =
            GoogleConfig::vertex_ai("vertex-1", "my-project", "us-central1", "test-token");
        let provider = GoogleProvider::new(config).unwrap();

        let url = provider.endpoint_url("gemini-1.5-pro", false);
        assert!(url.contains("generateContent"));
        assert!(url.contains("gemini-1.5-pro"));
        assert!(!url.contains("key=")); // No API key in URL for Vertex AI
    }
}
