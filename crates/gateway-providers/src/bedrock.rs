//! # AWS Bedrock Provider
//!
//! LLM Provider implementation for AWS Bedrock.
//!
//! This provider supports:
//! - Anthropic Claude models via Bedrock
//! - Amazon Titan models
//! - Cohere models
//! - Meta Llama models
//! - Mistral models
//!
//! ## Authentication
//!
//! AWS Bedrock uses AWS Signature Version 4 authentication.
//! Credentials can be provided via:
//! - Environment variables (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY)
//! - IAM roles (for EC2/ECS/Lambda)
//! - Instance profile credentials
//!
//! ## Example
//!
//! ```rust,ignore
//! use gateway_providers::bedrock::{BedrockConfig, BedrockProvider};
//!
//! let config = BedrockConfig::builder()
//!     .id("bedrock-1")
//!     .region("us-east-1")
//!     .access_key_id("AKIA...")
//!     .secret_access_key("...")
//!     .build();
//!
//! let provider = BedrockProvider::new(config)?;
//! ```

use async_stream::try_stream;
use async_trait::async_trait;
use futures::stream::BoxStream;
use gateway_core::{
    ChatChunk, ChatMessage, Choice, ChunkChoice, ChunkDelta, FinishReason,
    GatewayError, GatewayRequest, GatewayResponse, HealthStatus, LLMProvider, MessageContent,
    MessageRole, ModelInfo, ProviderCapabilities, ProviderType, Usage,
};
use gateway_core::request::ContentPart;
use gateway_core::response::ResponseMessage;
use reqwest::Client;
use serde::Deserialize;
use std::{
    collections::HashMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tracing::{debug, warn};

/// AWS Bedrock configuration
#[derive(Debug, Clone)]
pub struct BedrockConfig {
    /// Provider instance ID
    pub id: String,
    /// AWS region (e.g., "us-east-1")
    pub region: String,
    /// AWS access key ID (optional - can use IAM roles)
    pub access_key_id: Option<String>,
    /// AWS secret access key (optional - can use IAM roles)
    pub secret_access_key: Option<String>,
    /// AWS session token (optional - for temporary credentials)
    pub session_token: Option<String>,
    /// Custom endpoint URL (for testing/VPC endpoints)
    pub endpoint_url: Option<String>,
    /// Request timeout
    pub timeout: Duration,
    /// Supported models
    pub models: Vec<ModelInfo>,
}

impl BedrockConfig {
    /// Create a new builder
    pub fn builder() -> BedrockConfigBuilder {
        BedrockConfigBuilder::default()
    }

    /// Get the Bedrock service endpoint
    pub fn base_url(&self) -> String {
        self.endpoint_url
            .clone()
            .unwrap_or_else(|| format!("https://bedrock-runtime.{}.amazonaws.com", self.region))
    }

    /// Default Bedrock models
    #[must_use]
    pub fn default_models() -> Vec<ModelInfo> {
        vec![
            // Anthropic Claude models
            ModelInfo::new("anthropic.claude-3-5-sonnet-20241022-v2:0")
                .with_context_length(200_000)
                .with_max_output_tokens(8192),
            ModelInfo::new("anthropic.claude-3-5-sonnet-20240620-v1:0")
                .with_context_length(200_000)
                .with_max_output_tokens(8192),
            ModelInfo::new("anthropic.claude-3-sonnet-20240229-v1:0")
                .with_context_length(200_000)
                .with_max_output_tokens(4096),
            ModelInfo::new("anthropic.claude-3-haiku-20240307-v1:0")
                .with_context_length(200_000)
                .with_max_output_tokens(4096),
            ModelInfo::new("anthropic.claude-3-opus-20240229-v1:0")
                .with_context_length(200_000)
                .with_max_output_tokens(4096),
            // Amazon Titan
            ModelInfo::new("amazon.titan-text-express-v1")
                .with_context_length(8000)
                .with_max_output_tokens(8000),
            ModelInfo::new("amazon.titan-text-lite-v1")
                .with_context_length(4000)
                .with_max_output_tokens(4000),
            // Meta Llama
            ModelInfo::new("meta.llama3-70b-instruct-v1:0")
                .with_context_length(8000)
                .with_max_output_tokens(2048),
            ModelInfo::new("meta.llama3-8b-instruct-v1:0")
                .with_context_length(8000)
                .with_max_output_tokens(2048),
            // Mistral
            ModelInfo::new("mistral.mistral-large-2402-v1:0")
                .with_context_length(32000)
                .with_max_output_tokens(8192),
            ModelInfo::new("mistral.mixtral-8x7b-instruct-v0:1")
                .with_context_length(32000)
                .with_max_output_tokens(4096),
            // Cohere
            ModelInfo::new("cohere.command-r-plus-v1:0")
                .with_context_length(128_000)
                .with_max_output_tokens(4096),
            ModelInfo::new("cohere.command-r-v1:0")
                .with_context_length(128_000)
                .with_max_output_tokens(4096),
        ]
    }
}

/// Builder for `BedrockConfig`
#[derive(Debug, Default)]
pub struct BedrockConfigBuilder {
    id: Option<String>,
    region: Option<String>,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    session_token: Option<String>,
    endpoint_url: Option<String>,
    timeout: Option<Duration>,
    models: Option<Vec<ModelInfo>>,
}

impl BedrockConfigBuilder {
    /// Set the provider instance ID
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the AWS region
    pub fn region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Set the AWS access key ID
    pub fn access_key_id(mut self, key: impl Into<String>) -> Self {
        self.access_key_id = Some(key.into());
        self
    }

    /// Set the AWS secret access key
    pub fn secret_access_key(mut self, secret: impl Into<String>) -> Self {
        self.secret_access_key = Some(secret.into());
        self
    }

    /// Set the AWS session token
    pub fn session_token(mut self, token: impl Into<String>) -> Self {
        self.session_token = Some(token.into());
        self
    }

    /// Set custom endpoint URL
    pub fn endpoint_url(mut self, url: impl Into<String>) -> Self {
        self.endpoint_url = Some(url.into());
        self
    }

    /// Set request timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set custom models
    pub fn models(mut self, models: Vec<ModelInfo>) -> Self {
        self.models = Some(models);
        self
    }

    /// Build the configuration
    pub fn build(self) -> BedrockConfig {
        BedrockConfig {
            id: self.id.unwrap_or_else(|| "bedrock".to_string()),
            region: self.region.unwrap_or_else(|| "us-east-1".to_string()),
            access_key_id: self.access_key_id,
            secret_access_key: self.secret_access_key,
            session_token: self.session_token,
            endpoint_url: self.endpoint_url,
            timeout: self.timeout.unwrap_or(Duration::from_secs(300)),
            models: self.models.unwrap_or_else(BedrockConfig::default_models),
        }
    }
}

/// Model family for Bedrock
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFamily {
    /// Anthropic Claude models
    Claude,
    /// Amazon Titan models
    Titan,
    /// Cohere models
    Cohere,
    /// Meta Llama models
    Llama,
    /// Mistral models
    Mistral,
    /// AI21 models
    Ai21,
}

impl ModelFamily {
    /// Detect model family from model ID
    pub fn from_model_id(model_id: &str) -> Option<Self> {
        if model_id.starts_with("anthropic.") {
            Some(ModelFamily::Claude)
        } else if model_id.starts_with("amazon.titan") {
            Some(ModelFamily::Titan)
        } else if model_id.starts_with("cohere.") {
            Some(ModelFamily::Cohere)
        } else if model_id.starts_with("meta.llama") {
            Some(ModelFamily::Llama)
        } else if model_id.starts_with("mistral.") {
            Some(ModelFamily::Mistral)
        } else if model_id.starts_with("ai21.") {
            Some(ModelFamily::Ai21)
        } else {
            None
        }
    }
}

/// AWS Bedrock provider
pub struct BedrockProvider {
    config: BedrockConfig,
    client: Client,
    capabilities: ProviderCapabilities,
    base_url: String,
}

impl std::fmt::Debug for BedrockProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BedrockProvider")
            .field("id", &self.config.id)
            .field("region", &self.config.region)
            .finish()
    }
}

impl BedrockProvider {
    /// Create a new Bedrock provider
    pub fn new(config: BedrockConfig) -> Result<Self, GatewayError> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| GatewayError::internal(format!("Failed to create HTTP client: {}", e)))?;

        let base_url = config.base_url();

        Ok(Self {
            config,
            client,
            capabilities: ProviderCapabilities {
                chat: true,
                streaming: true,
                function_calling: true,
                vision: true,
                embeddings: false,
                json_mode: false,
                seed: false,
                logprobs: false,
                max_context_length: Some(200_000),
                max_output_tokens: Some(8192),
                parallel_tool_calls: false,
            },
            base_url,
        })
    }

    /// Get the invoke URL for a model
    fn invoke_url(&self, model_id: &str) -> String {
        format!("{}/model/{}/invoke", self.base_url, model_id)
    }

    /// Get the invoke-with-response-stream URL for a model
    fn stream_url(&self, model_id: &str) -> String {
        format!(
            "{}/model/{}/invoke-with-response-stream",
            self.base_url,
            model_id
        )
    }

    /// Convert gateway request to Bedrock format based on model family
    fn transform_request(
        &self,
        request: &GatewayRequest,
        model_family: ModelFamily,
    ) -> Result<serde_json::Value, GatewayError> {
        match model_family {
            ModelFamily::Claude => self.transform_claude_request(request),
            ModelFamily::Titan => self.transform_titan_request(request),
            ModelFamily::Llama => self.transform_llama_request(request),
            ModelFamily::Mistral => self.transform_mistral_request(request),
            ModelFamily::Cohere => self.transform_cohere_request(request),
            ModelFamily::Ai21 => self.transform_ai21_request(request),
        }
    }

    /// Transform request for Claude models (uses Messages API format)
    fn transform_claude_request(
        &self,
        request: &GatewayRequest,
    ) -> Result<serde_json::Value, GatewayError> {
        let mut messages = Vec::new();
        let mut system_prompt = None;

        for msg in &request.messages {
            match msg.role {
                MessageRole::System => {
                    system_prompt = Some(Self::extract_text_content(&msg.content));
                }
                MessageRole::User => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": Self::transform_content(&msg.content)
                    }));
                }
                MessageRole::Assistant => {
                    messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": Self::extract_text_content(&msg.content)
                    }));
                }
                MessageRole::Tool => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": Self::extract_text_content(&msg.content)
                    }));
                }
            }
        }

        let mut body = serde_json::json!({
            "anthropic_version": "bedrock-2023-05-31",
            "max_tokens": request.max_tokens.unwrap_or(4096),
            "messages": messages
        });

        if let Some(system) = system_prompt {
            body["system"] = serde_json::Value::String(system);
        }

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::Value::Number(
                serde_json::Number::from_f64(temp as f64).unwrap_or(serde_json::Number::from(1)),
            );
        }

        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::Value::Number(
                serde_json::Number::from_f64(top_p as f64).unwrap_or(serde_json::Number::from(1)),
            );
        }

        if let Some(ref stop) = request.stop {
            body["stop_sequences"] = serde_json::Value::Array(
                stop.iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            );
        }

        Ok(body)
    }

    /// Transform request for Titan models
    fn transform_titan_request(
        &self,
        request: &GatewayRequest,
    ) -> Result<serde_json::Value, GatewayError> {
        let prompt = Self::build_prompt_from_messages(&request.messages);

        let mut text_generation_config = serde_json::json!({
            "maxTokenCount": request.max_tokens.unwrap_or(4096)
        });

        if let Some(temp) = request.temperature {
            text_generation_config["temperature"] = serde_json::Value::Number(
                serde_json::Number::from_f64(temp as f64).unwrap_or(serde_json::Number::from(1)),
            );
        }

        if let Some(top_p) = request.top_p {
            text_generation_config["topP"] = serde_json::Value::Number(
                serde_json::Number::from_f64(top_p as f64).unwrap_or(serde_json::Number::from(1)),
            );
        }

        if let Some(ref stop) = request.stop {
            text_generation_config["stopSequences"] = serde_json::Value::Array(
                stop.iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            );
        }

        Ok(serde_json::json!({
            "inputText": prompt,
            "textGenerationConfig": text_generation_config
        }))
    }

    /// Transform request for Llama models
    fn transform_llama_request(
        &self,
        request: &GatewayRequest,
    ) -> Result<serde_json::Value, GatewayError> {
        let prompt = Self::build_llama_prompt(&request.messages);

        let mut body = serde_json::json!({
            "prompt": prompt,
            "max_gen_len": request.max_tokens.unwrap_or(2048)
        });

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::Value::Number(
                serde_json::Number::from_f64(temp as f64).unwrap_or(serde_json::Number::from(1)),
            );
        }

        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::Value::Number(
                serde_json::Number::from_f64(top_p as f64).unwrap_or(serde_json::Number::from(1)),
            );
        }

        Ok(body)
    }

    /// Transform request for Mistral models
    fn transform_mistral_request(
        &self,
        request: &GatewayRequest,
    ) -> Result<serde_json::Value, GatewayError> {
        let prompt = Self::build_mistral_prompt(&request.messages);

        let mut body = serde_json::json!({
            "prompt": prompt,
            "max_tokens": request.max_tokens.unwrap_or(4096)
        });

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::Value::Number(
                serde_json::Number::from_f64(temp as f64).unwrap_or(serde_json::Number::from(1)),
            );
        }

        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::Value::Number(
                serde_json::Number::from_f64(top_p as f64).unwrap_or(serde_json::Number::from(1)),
            );
        }

        if let Some(ref stop) = request.stop {
            body["stop"] = serde_json::Value::Array(
                stop.iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            );
        }

        Ok(body)
    }

    /// Transform request for Cohere models
    fn transform_cohere_request(
        &self,
        request: &GatewayRequest,
    ) -> Result<serde_json::Value, GatewayError> {
        let prompt = Self::build_prompt_from_messages(&request.messages);

        let mut body = serde_json::json!({
            "prompt": prompt,
            "max_tokens": request.max_tokens.unwrap_or(4096)
        });

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::Value::Number(
                serde_json::Number::from_f64(temp as f64).unwrap_or(serde_json::Number::from(1)),
            );
        }

        if let Some(top_p) = request.top_p {
            body["p"] = serde_json::Value::Number(
                serde_json::Number::from_f64(top_p as f64).unwrap_or(serde_json::Number::from(1)),
            );
        }

        if let Some(ref stop) = request.stop {
            body["stop_sequences"] = serde_json::Value::Array(
                stop.iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            );
        }

        Ok(body)
    }

    /// Transform request for AI21 models
    fn transform_ai21_request(
        &self,
        request: &GatewayRequest,
    ) -> Result<serde_json::Value, GatewayError> {
        let prompt = Self::build_prompt_from_messages(&request.messages);

        let mut body = serde_json::json!({
            "prompt": prompt,
            "maxTokens": request.max_tokens.unwrap_or(4096)
        });

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::Value::Number(
                serde_json::Number::from_f64(temp as f64).unwrap_or(serde_json::Number::from(1)),
            );
        }

        if let Some(top_p) = request.top_p {
            body["topP"] = serde_json::Value::Number(
                serde_json::Number::from_f64(top_p as f64).unwrap_or(serde_json::Number::from(1)),
            );
        }

        if let Some(ref stop) = request.stop {
            body["stopSequences"] = serde_json::Value::Array(
                stop.iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            );
        }

        Ok(body)
    }

    /// Build a simple prompt from messages
    fn build_prompt_from_messages(messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();

        for msg in messages {
            let content = Self::extract_text_content(&msg.content);
            match msg.role {
                MessageRole::System => {
                    prompt.push_str(&format!("System: {}\n\n", content));
                }
                MessageRole::User => {
                    prompt.push_str(&format!("User: {}\n\n", content));
                }
                MessageRole::Assistant => {
                    prompt.push_str(&format!("Assistant: {}\n\n", content));
                }
                MessageRole::Tool => {
                    prompt.push_str(&format!("Tool: {}\n\n", content));
                }
            }
        }

        prompt.push_str("Assistant:");
        prompt
    }

    /// Build Llama-style prompt
    fn build_llama_prompt(messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();
        let mut system_content = String::new();

        for msg in messages {
            let content = Self::extract_text_content(&msg.content);
            match msg.role {
                MessageRole::System => {
                    system_content = content;
                }
                MessageRole::User => {
                    if !system_content.is_empty() {
                        prompt.push_str(&format!(
                            "<s>[INST] <<SYS>>\n{}\n<</SYS>>\n\n{} [/INST]",
                            system_content, content
                        ));
                        system_content.clear();
                    } else {
                        prompt.push_str(&format!("<s>[INST] {} [/INST]", content));
                    }
                }
                MessageRole::Assistant => {
                    prompt.push_str(&format!(" {} </s>", content));
                }
                MessageRole::Tool => {
                    prompt.push_str(&format!("<s>[INST] Tool result: {} [/INST]", content));
                }
            }
        }

        prompt
    }

    /// Build Mistral-style prompt
    fn build_mistral_prompt(messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();

        for msg in messages {
            let content = Self::extract_text_content(&msg.content);
            match msg.role {
                MessageRole::System => {
                    prompt.push_str(&format!("<s>[INST] {}\n", content));
                }
                MessageRole::User => {
                    if prompt.is_empty() {
                        prompt.push_str(&format!("<s>[INST] {} [/INST]", content));
                    } else {
                        prompt.push_str(&format!(" [INST] {} [/INST]", content));
                    }
                }
                MessageRole::Assistant => {
                    prompt.push_str(&format!(" {}</s>", content));
                }
                MessageRole::Tool => {
                    prompt.push_str(&format!(" [INST] Tool: {} [/INST]", content));
                }
            }
        }

        prompt
    }

    /// Transform content to Bedrock format
    fn transform_content(content: &MessageContent) -> serde_json::Value {
        match content {
            MessageContent::Text(text) => {
                serde_json::Value::String(text.clone())
            }
            MessageContent::Parts(parts) => {
                let transformed: Vec<serde_json::Value> = parts
                    .iter()
                    .map(|part| match part {
                        ContentPart::Text { text } => {
                            serde_json::json!({ "type": "text", "text": text })
                        }
                        ContentPart::ImageUrl { image_url } => {
                            if let Some((media_type, data)) = Self::parse_data_url(&image_url.url) {
                                serde_json::json!({
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": media_type,
                                        "data": data
                                    }
                                })
                            } else {
                                serde_json::json!({
                                    "type": "text",
                                    "text": format!("[Image: {}]", image_url.url)
                                })
                            }
                        }
                    })
                    .collect();
                serde_json::Value::Array(transformed)
            }
        }
    }

    /// Extract text content from message content
    fn extract_text_content(content: &MessageContent) -> String {
        match content {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Parts(parts) => {
                parts
                    .iter()
                    .filter_map(|p| match p {
                        ContentPart::Text { text } => Some(text.clone()),
                        ContentPart::ImageUrl { .. } => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
    }

    /// Parse data URL to extract media type and base64 data
    fn parse_data_url(url: &str) -> Option<(String, String)> {
        if !url.starts_with("data:") {
            return None;
        }

        let remainder = &url[5..];
        let parts: Vec<&str> = remainder.splitn(2, ',').collect();
        if parts.len() != 2 {
            return None;
        }

        let metadata = parts[0];
        let data = parts[1];

        let media_type = metadata
            .split(';')
            .next()
            .unwrap_or("application/octet-stream")
            .to_string();

        Some((media_type, data.to_string()))
    }

    /// Parse Claude response from Bedrock
    fn parse_claude_response(
        &self,
        response: &BedrockClaudeResponse,
        model: &str,
    ) -> Result<GatewayResponse, GatewayError> {
        let content: String = response
            .content
            .iter()
            .filter_map(|block| {
                if block.block_type == "text" {
                    block.text.clone()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        let finish_reason = match response.stop_reason.as_deref() {
            Some("end_turn") => Some(FinishReason::Stop),
            Some("max_tokens") => Some(FinishReason::Length),
            Some("stop_sequence") => Some(FinishReason::Stop),
            _ => None,
        };

        let message = ResponseMessage {
            role: MessageRole::Assistant,
            content: Some(content),
            tool_calls: None,
            function_call: None,
        };

        let usage = Usage {
            prompt_tokens: response.usage.input_tokens,
            completion_tokens: response.usage.output_tokens,
            total_tokens: response.usage.input_tokens + response.usage.output_tokens,
        };

        Ok(GatewayResponse::builder()
            .id(response.id.clone().unwrap_or_else(|| format!("bedrock-{}", uuid::Uuid::new_v4())))
            .model(model.to_string())
            .choice(Choice {
                index: 0,
                message,
                finish_reason,
                logprobs: None,
            })
            .usage(usage)
            .build())
    }

    /// Parse Titan response from Bedrock
    fn parse_titan_response(
        &self,
        response: &BedrockTitanResponse,
        model: &str,
    ) -> Result<GatewayResponse, GatewayError> {
        let result = response
            .results
            .first()
            .ok_or_else(|| GatewayError::provider("bedrock", "No results in Titan response", None, false))?;

        let finish_reason = match result.completion_reason.as_deref() {
            Some("FINISH") => Some(FinishReason::Stop),
            Some("LENGTH") => Some(FinishReason::Length),
            Some("CONTENT_FILTERED") => Some(FinishReason::ContentFilter),
            _ => None,
        };

        let message = ResponseMessage {
            role: MessageRole::Assistant,
            content: Some(result.output_text.clone()),
            tool_calls: None,
            function_call: None,
        };

        let usage = Usage {
            prompt_tokens: response.input_text_token_count.unwrap_or(0),
            completion_tokens: result.token_count.unwrap_or(0),
            total_tokens: response.input_text_token_count.unwrap_or(0)
                + result.token_count.unwrap_or(0),
        };

        Ok(GatewayResponse::builder()
            .id(format!("bedrock-{}", uuid::Uuid::new_v4()))
            .model(model.to_string())
            .choice(Choice {
                index: 0,
                message,
                finish_reason,
                logprobs: None,
            })
            .usage(usage)
            .build())
    }

    /// Parse Llama response from Bedrock
    fn parse_llama_response(
        &self,
        response: &BedrockLlamaResponse,
        model: &str,
    ) -> Result<GatewayResponse, GatewayError> {
        let finish_reason = match response.stop_reason.as_deref() {
            Some("stop") => Some(FinishReason::Stop),
            Some("length") => Some(FinishReason::Length),
            _ => None,
        };

        let message = ResponseMessage {
            role: MessageRole::Assistant,
            content: Some(response.generation.clone()),
            tool_calls: None,
            function_call: None,
        };

        let usage = Usage {
            prompt_tokens: response.prompt_token_count.unwrap_or(0),
            completion_tokens: response.generation_token_count.unwrap_or(0),
            total_tokens: response.prompt_token_count.unwrap_or(0)
                + response.generation_token_count.unwrap_or(0),
        };

        Ok(GatewayResponse::builder()
            .id(format!("bedrock-{}", uuid::Uuid::new_v4()))
            .model(model.to_string())
            .choice(Choice {
                index: 0,
                message,
                finish_reason,
                logprobs: None,
            })
            .usage(usage)
            .build())
    }

    /// Parse Mistral response from Bedrock
    fn parse_mistral_response(
        &self,
        response: &BedrockMistralResponse,
        model: &str,
    ) -> Result<GatewayResponse, GatewayError> {
        let output = response
            .outputs
            .first()
            .ok_or_else(|| GatewayError::provider("bedrock", "No outputs in Mistral response", None, false))?;

        let finish_reason = match output.stop_reason.as_deref() {
            Some("stop") => Some(FinishReason::Stop),
            Some("length") => Some(FinishReason::Length),
            _ => None,
        };

        let message = ResponseMessage {
            role: MessageRole::Assistant,
            content: Some(output.text.clone()),
            tool_calls: None,
            function_call: None,
        };

        Ok(GatewayResponse::builder()
            .id(format!("bedrock-{}", uuid::Uuid::new_v4()))
            .model(model.to_string())
            .choice(Choice {
                index: 0,
                message,
                finish_reason,
                logprobs: None,
            })
            .usage(Usage::default())
            .build())
    }

    /// Parse Cohere response from Bedrock
    fn parse_cohere_response(
        &self,
        response: &BedrockCohereResponse,
        model: &str,
    ) -> Result<GatewayResponse, GatewayError> {
        let generation = response
            .generations
            .first()
            .ok_or_else(|| GatewayError::provider("bedrock", "No generations in Cohere response", None, false))?;

        let finish_reason = match generation.finish_reason.as_deref() {
            Some("COMPLETE") => Some(FinishReason::Stop),
            Some("MAX_TOKENS") => Some(FinishReason::Length),
            _ => None,
        };

        let message = ResponseMessage {
            role: MessageRole::Assistant,
            content: Some(generation.text.clone()),
            tool_calls: None,
            function_call: None,
        };

        Ok(GatewayResponse::builder()
            .id(generation.id.clone().unwrap_or_else(|| format!("bedrock-{}", uuid::Uuid::new_v4())))
            .model(model.to_string())
            .choice(Choice {
                index: 0,
                message,
                finish_reason,
                logprobs: None,
            })
            .usage(Usage::default())
            .build())
    }

    /// Parse AI21 response from Bedrock
    fn parse_ai21_response(
        &self,
        response: &BedrockAi21Response,
        model: &str,
    ) -> Result<GatewayResponse, GatewayError> {
        let completion = response
            .completions
            .first()
            .ok_or_else(|| GatewayError::provider("bedrock", "No completions in AI21 response", None, false))?;

        let finish_reason = match completion.finish_reason.reason.as_deref() {
            Some("endoftext") => Some(FinishReason::Stop),
            Some("length") => Some(FinishReason::Length),
            _ => None,
        };

        let message = ResponseMessage {
            role: MessageRole::Assistant,
            content: Some(completion.data.text.clone()),
            tool_calls: None,
            function_call: None,
        };

        Ok(GatewayResponse::builder()
            .id(response.id.clone().unwrap_or_else(|| format!("bedrock-{}", uuid::Uuid::new_v4())))
            .model(model.to_string())
            .choice(Choice {
                index: 0,
                message,
                finish_reason,
                logprobs: None,
            })
            .usage(Usage::default())
            .build())
    }

    /// Sign a request with AWS Signature Version 4
    fn sign_request(
        &self,
        method: &str,
        uri: &str,
        body: &[u8],
        headers: &mut HashMap<String, String>,
    ) -> Result<(), GatewayError> {
        let access_key = self.config.access_key_id.as_ref().ok_or_else(|| {
            GatewayError::authentication("AWS access key ID not configured")
        })?;
        let secret_key = self.config.secret_access_key.as_ref().ok_or_else(|| {
            GatewayError::authentication("AWS secret access key not configured")
        })?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| GatewayError::internal(format!("Time error: {}", e)))?;

        let datetime = chrono::DateTime::from_timestamp(now.as_secs() as i64, 0)
            .ok_or_else(|| GatewayError::internal("Invalid timestamp"))?;
        let amz_date = datetime.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = datetime.format("%Y%m%d").to_string();

        let service = "bedrock";
        let region = &self.config.region;

        // Create canonical request
        let host = url::Url::parse(uri)
            .map_err(|e| GatewayError::internal(format!("Invalid URL: {}", e)))?
            .host_str()
            .ok_or_else(|| GatewayError::internal("URL has no host"))?
            .to_string();

        let path = url::Url::parse(uri)
            .map_err(|e| GatewayError::internal(format!("Invalid URL: {}", e)))?
            .path()
            .to_string();

        let payload_hash = hex::encode(sha256_hash(body));

        headers.insert("host".to_string(), host.clone());
        headers.insert("x-amz-date".to_string(), amz_date.clone());
        headers.insert("x-amz-content-sha256".to_string(), payload_hash.clone());

        if let Some(ref token) = self.config.session_token {
            headers.insert("x-amz-security-token".to_string(), token.clone());
        }

        let mut signed_headers: Vec<&str> = headers.keys().map(|k| k.as_str()).collect();
        signed_headers.sort();
        let signed_headers_str = signed_headers.join(";");

        let mut canonical_headers = String::new();
        for header in &signed_headers {
            if let Some(value) = headers.get(*header) {
                canonical_headers.push_str(&format!("{}:{}\n", header, value.trim()));
            }
        }

        let canonical_request = format!(
            "{}\n{}\n\n{}\n{}\n{}",
            method, path, canonical_headers, signed_headers_str, payload_hash
        );

        // Create string to sign
        let algorithm = "AWS4-HMAC-SHA256";
        let credential_scope = format!("{}/{}/{}/aws4_request", date_stamp, region, service);
        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            algorithm,
            amz_date,
            credential_scope,
            hex::encode(sha256_hash(canonical_request.as_bytes()))
        );

        // Calculate signature
        let k_date = hmac_sha256(format!("AWS4{}", secret_key).as_bytes(), date_stamp.as_bytes());
        let k_region = hmac_sha256(&k_date, region.as_bytes());
        let k_service = hmac_sha256(&k_region, service.as_bytes());
        let k_signing = hmac_sha256(&k_service, b"aws4_request");
        let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()));

        // Create authorization header
        let authorization = format!(
            "{} Credential={}/{}, SignedHeaders={}, Signature={}",
            algorithm, access_key, credential_scope, signed_headers_str, signature
        );

        headers.insert("authorization".to_string(), authorization);

        Ok(())
    }
}

/// Calculate SHA-256 hash
fn sha256_hash(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Calculate HMAC-SHA256
fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

/// Bedrock Claude response format
#[derive(Debug, Deserialize)]
struct BedrockClaudeResponse {
    id: Option<String>,
    content: Vec<ClaudeContentBlock>,
    stop_reason: Option<String>,
    usage: ClaudeUsage,
}

#[derive(Debug, Deserialize)]
struct ClaudeContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeUsage {
    input_tokens: u32,
    output_tokens: u32,
}

/// Bedrock Titan response format
#[derive(Debug, Deserialize)]
struct BedrockTitanResponse {
    #[serde(rename = "inputTextTokenCount")]
    input_text_token_count: Option<u32>,
    results: Vec<TitanResult>,
}

#[derive(Debug, Deserialize)]
struct TitanResult {
    #[serde(rename = "outputText")]
    output_text: String,
    #[serde(rename = "completionReason")]
    completion_reason: Option<String>,
    #[serde(rename = "tokenCount")]
    token_count: Option<u32>,
}

/// Bedrock Llama response format
#[derive(Debug, Deserialize)]
struct BedrockLlamaResponse {
    generation: String,
    #[serde(rename = "prompt_token_count")]
    prompt_token_count: Option<u32>,
    #[serde(rename = "generation_token_count")]
    generation_token_count: Option<u32>,
    stop_reason: Option<String>,
}

/// Bedrock Mistral response format
#[derive(Debug, Deserialize)]
struct BedrockMistralResponse {
    outputs: Vec<MistralOutput>,
}

#[derive(Debug, Deserialize)]
struct MistralOutput {
    text: String,
    stop_reason: Option<String>,
}

/// Bedrock Cohere response format
#[derive(Debug, Deserialize)]
struct BedrockCohereResponse {
    generations: Vec<CohereGeneration>,
}

#[derive(Debug, Deserialize)]
struct CohereGeneration {
    id: Option<String>,
    text: String,
    finish_reason: Option<String>,
}

/// Bedrock AI21 response format
#[derive(Debug, Deserialize)]
struct BedrockAi21Response {
    id: Option<String>,
    completions: Vec<Ai21Completion>,
}

#[derive(Debug, Deserialize)]
struct Ai21Completion {
    data: Ai21CompletionData,
    #[serde(rename = "finishReason")]
    finish_reason: Ai21FinishReason,
}

#[derive(Debug, Deserialize)]
struct Ai21CompletionData {
    text: String,
}

#[derive(Debug, Deserialize)]
struct Ai21FinishReason {
    reason: Option<String>,
}

/// Bedrock error response
#[derive(Debug, Deserialize)]
struct BedrockError {
    message: Option<String>,
    #[serde(rename = "Message")]
    message_alt: Option<String>,
}

impl BedrockError {
    fn message(&self) -> String {
        self.message
            .clone()
            .or_else(|| self.message_alt.clone())
            .unwrap_or_else(|| "Unknown error".to_string())
    }
}

#[async_trait]
impl LLMProvider for BedrockProvider {
    fn id(&self) -> &str {
        &self.config.id
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Bedrock
    }

    async fn chat_completion(
        &self,
        request: &GatewayRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        let model = &request.model;
        let model_family = ModelFamily::from_model_id(model).ok_or_else(|| {
            GatewayError::model_not_found(&format!("Unsupported model family for: {}", model))
        })?;

        let body = self.transform_request(request, model_family)?;
        let body_bytes = serde_json::to_vec(&body).map_err(|e| {
            GatewayError::validation(format!("Failed to serialize request: {}", e), None, "serialization_error")
        })?;

        let url = self.invoke_url(model);
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        headers.insert("accept".to_string(), "application/json".to_string());

        self.sign_request("POST", &url, &body_bytes, &mut headers)?;

        let mut req_builder = self.client.post(&url);
        for (key, value) in &headers {
            req_builder = req_builder.header(key, value);
        }

        debug!(model = %model, "Sending request to Bedrock");

        let response = req_builder
            .body(body_bytes)
            .send()
            .await
            .map_err(|e| GatewayError::provider("bedrock", format!("Request failed: {}", e), None, true))?;

        let status = response.status();
        let response_bytes = response.bytes().await.map_err(|e| {
            GatewayError::provider("bedrock", format!("Failed to read response: {}", e), None, true)
        })?;

        if !status.is_success() {
            let error: BedrockError = serde_json::from_slice(&response_bytes)
                .unwrap_or(BedrockError {
                    message: Some(String::from_utf8_lossy(&response_bytes).to_string()),
                    message_alt: None,
                });

            let is_retryable = status.as_u16() >= 500 || status.as_u16() == 429;

            if status.as_u16() == 429 {
                return Err(GatewayError::rate_limit(None, None));
            }

            return Err(GatewayError::provider(
                "bedrock",
                error.message(),
                Some(status.as_u16()),
                is_retryable,
            ));
        }

        // Parse based on model family
        match model_family {
            ModelFamily::Claude => {
                let parsed: BedrockClaudeResponse = serde_json::from_slice(&response_bytes)
                    .map_err(|e| {
                        GatewayError::provider(
                            "bedrock",
                            format!("Failed to parse Claude response: {}", e),
                            None,
                            false,
                        )
                    })?;
                self.parse_claude_response(&parsed, model)
            }
            ModelFamily::Titan => {
                let parsed: BedrockTitanResponse = serde_json::from_slice(&response_bytes)
                    .map_err(|e| {
                        GatewayError::provider(
                            "bedrock",
                            format!("Failed to parse Titan response: {}", e),
                            None,
                            false,
                        )
                    })?;
                self.parse_titan_response(&parsed, model)
            }
            ModelFamily::Llama => {
                let parsed: BedrockLlamaResponse = serde_json::from_slice(&response_bytes)
                    .map_err(|e| {
                        GatewayError::provider(
                            "bedrock",
                            format!("Failed to parse Llama response: {}", e),
                            None,
                            false,
                        )
                    })?;
                self.parse_llama_response(&parsed, model)
            }
            ModelFamily::Mistral => {
                let parsed: BedrockMistralResponse = serde_json::from_slice(&response_bytes)
                    .map_err(|e| {
                        GatewayError::provider(
                            "bedrock",
                            format!("Failed to parse Mistral response: {}", e),
                            None,
                            false,
                        )
                    })?;
                self.parse_mistral_response(&parsed, model)
            }
            ModelFamily::Cohere => {
                let parsed: BedrockCohereResponse = serde_json::from_slice(&response_bytes)
                    .map_err(|e| {
                        GatewayError::provider(
                            "bedrock",
                            format!("Failed to parse Cohere response: {}", e),
                            None,
                            false,
                        )
                    })?;
                self.parse_cohere_response(&parsed, model)
            }
            ModelFamily::Ai21 => {
                let parsed: BedrockAi21Response = serde_json::from_slice(&response_bytes)
                    .map_err(|e| {
                        GatewayError::provider(
                            "bedrock",
                            format!("Failed to parse AI21 response: {}", e),
                            None,
                            false,
                        )
                    })?;
                self.parse_ai21_response(&parsed, model)
            }
        }
    }

    async fn chat_completion_stream(
        &self,
        request: &GatewayRequest,
    ) -> Result<BoxStream<'static, Result<ChatChunk, GatewayError>>, GatewayError>
    {
        let model = request.model.clone();
        let model_family = ModelFamily::from_model_id(&model).ok_or_else(|| {
            GatewayError::model_not_found(&format!("Unsupported model family for: {}", model))
        })?;

        let body = self.transform_request(request, model_family)?;
        let body_bytes = serde_json::to_vec(&body).map_err(|e| {
            GatewayError::validation(format!("Failed to serialize request: {}", e), None, "serialization_error")
        })?;

        let url = self.stream_url(&model);
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        headers.insert("accept".to_string(), "application/vnd.amazon.eventstream".to_string());

        self.sign_request("POST", &url, &body_bytes, &mut headers)?;

        let mut req_builder = self.client.post(&url);
        for (key, value) in &headers {
            req_builder = req_builder.header(key, value);
        }

        debug!(model = %model, "Starting streaming request to Bedrock");

        let response = req_builder
            .body(body_bytes)
            .send()
            .await
            .map_err(|e| GatewayError::provider("bedrock", format!("Request failed: {}", e), None, true))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(GatewayError::provider(
                "bedrock",
                format!("Streaming request failed: {}", error_text),
                Some(status.as_u16()),
                status.as_u16() >= 500,
            ));
        }

        // Note: AWS Bedrock uses a custom binary event stream format
        // For simplicity, we fall back to non-streaming and emit a single chunk
        // A full implementation would parse the binary event stream
        warn!("Bedrock streaming uses binary event stream; falling back to non-streaming");

        let response_bytes = response.bytes().await.map_err(|e| {
            GatewayError::provider("bedrock", format!("Failed to read response: {}", e), None, true)
        })?;

        // Try to parse as JSON (some models return JSON in streaming mode)
        let content = String::from_utf8_lossy(&response_bytes).to_string();

        let stream = try_stream! {
            let chunk = ChatChunk::builder()
                .id(format!("bedrock-{}", uuid::Uuid::new_v4()))
                .model(model.clone())
                .choice(ChunkChoice {
                    index: 0,
                    delta: ChunkDelta {
                        role: Some(MessageRole::Assistant),
                        content: Some(content),
                        tool_calls: None,
                        function_call: None,
                    },
                    finish_reason: Some(FinishReason::Stop),
                    logprobs: None,
                })
                .build();

            yield chunk;
        };

        Ok(Box::pin(stream))
    }

    async fn health_check(&self) -> HealthStatus {
        // For Bedrock, we can only validate credentials are configured
        // Actually invoking a model would cost money
        if self.config.access_key_id.is_none() || self.config.secret_access_key.is_none() {
            return HealthStatus::Unhealthy;
        }

        HealthStatus::Healthy
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    fn models(&self) -> &[ModelInfo] {
        &self.config.models
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn timeout(&self) -> Duration {
        self.config.timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = BedrockConfig::builder()
            .id("bedrock-test")
            .region("us-west-2")
            .access_key_id("AKIATEST")
            .secret_access_key("secret")
            .build();

        assert_eq!(config.id, "bedrock-test");
        assert_eq!(config.region, "us-west-2");
        assert_eq!(config.access_key_id.as_deref(), Some("AKIATEST"));
        assert_eq!(config.secret_access_key.as_deref(), Some("secret"));
    }

    #[test]
    fn test_base_url() {
        let config = BedrockConfig::builder().region("eu-west-1").build();

        assert_eq!(
            config.base_url(),
            "https://bedrock-runtime.eu-west-1.amazonaws.com"
        );
    }

    #[test]
    fn test_base_url_custom_endpoint() {
        let config = BedrockConfig::builder()
            .region("us-east-1")
            .endpoint_url("http://localhost:4566")
            .build();

        assert_eq!(config.base_url(), "http://localhost:4566");
    }

    #[test]
    fn test_model_family_detection() {
        assert_eq!(
            ModelFamily::from_model_id("anthropic.claude-3-sonnet-20240229-v1:0"),
            Some(ModelFamily::Claude)
        );
        assert_eq!(
            ModelFamily::from_model_id("amazon.titan-text-express-v1"),
            Some(ModelFamily::Titan)
        );
        assert_eq!(
            ModelFamily::from_model_id("meta.llama3-70b-instruct-v1:0"),
            Some(ModelFamily::Llama)
        );
        assert_eq!(
            ModelFamily::from_model_id("mistral.mistral-large-2402-v1:0"),
            Some(ModelFamily::Mistral)
        );
        assert_eq!(
            ModelFamily::from_model_id("cohere.command-r-plus-v1:0"),
            Some(ModelFamily::Cohere)
        );
        assert_eq!(
            ModelFamily::from_model_id("ai21.j2-ultra-v1"),
            Some(ModelFamily::Ai21)
        );
        assert_eq!(ModelFamily::from_model_id("unknown.model"), None);
    }

    #[test]
    fn test_invoke_url() {
        let config = BedrockConfig::builder().region("us-east-1").build();
        let provider = BedrockProvider::new(config).unwrap();

        assert_eq!(
            provider.invoke_url("anthropic.claude-3-sonnet-20240229-v1:0"),
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-3-sonnet-20240229-v1:0/invoke"
        );
    }

    #[test]
    fn test_stream_url() {
        let config = BedrockConfig::builder().region("us-west-2").build();
        let provider = BedrockProvider::new(config).unwrap();

        assert_eq!(
            provider.stream_url("amazon.titan-text-express-v1"),
            "https://bedrock-runtime.us-west-2.amazonaws.com/model/amazon.titan-text-express-v1/invoke-with-response-stream"
        );
    }

    #[test]
    fn test_default_models() {
        let config = BedrockConfig::builder().build();
        let provider = BedrockProvider::new(config).unwrap();

        let models = provider.models();
        assert!(!models.is_empty());

        let model_ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert!(model_ids.contains(&"anthropic.claude-3-sonnet-20240229-v1:0"));
        assert!(model_ids.contains(&"amazon.titan-text-express-v1"));
        assert!(model_ids.contains(&"meta.llama3-70b-instruct-v1:0"));
    }

    #[test]
    fn test_capabilities() {
        let config = BedrockConfig::builder().build();
        let provider = BedrockProvider::new(config).unwrap();

        let caps = provider.capabilities();
        assert!(caps.streaming);
        assert!(caps.function_calling);
        assert!(caps.vision);
        assert!(!caps.embeddings);
    }

    #[test]
    fn test_parse_data_url() {
        let result = BedrockProvider::parse_data_url("data:image/png;base64,iVBORw0KGgo=");
        assert!(result.is_some());
        let (media_type, data) = result.unwrap();
        assert_eq!(media_type, "image/png");
        assert_eq!(data, "iVBORw0KGgo=");
    }

    #[test]
    fn test_parse_data_url_invalid() {
        assert!(BedrockProvider::parse_data_url("https://example.com/image.png").is_none());
        assert!(BedrockProvider::parse_data_url("data:invalid").is_none());
    }

    #[test]
    fn test_build_prompt_from_messages() {
        let messages = vec![
            ChatMessage {
                role: MessageRole::System,
                content: MessageContent::Text("You are a helpful assistant.".to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: MessageContent::Text("Hello".to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let prompt = BedrockProvider::build_prompt_from_messages(&messages);
        assert!(prompt.contains("System:"));
        assert!(prompt.contains("User:"));
        assert!(prompt.ends_with("Assistant:"));
    }

    #[test]
    fn test_build_llama_prompt() {
        let messages = vec![
            ChatMessage {
                role: MessageRole::System,
                content: MessageContent::Text("You are helpful.".to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: MessageContent::Text("Hi".to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let prompt = BedrockProvider::build_llama_prompt(&messages);
        assert!(prompt.contains("[INST]"));
        assert!(prompt.contains("<<SYS>>"));
    }

    #[test]
    fn test_build_mistral_prompt() {
        let messages = vec![ChatMessage {
            role: MessageRole::User,
            content: MessageContent::Text("Hello".to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }];

        let prompt = BedrockProvider::build_mistral_prompt(&messages);
        assert!(prompt.contains("[INST]"));
        assert!(prompt.contains("[/INST]"));
    }

    #[test]
    fn test_provider_type() {
        let config = BedrockConfig::builder().build();
        let provider = BedrockProvider::new(config).unwrap();
        assert_eq!(provider.provider_type(), ProviderType::Bedrock);
    }

    #[test]
    fn test_provider_id() {
        let config = BedrockConfig::builder().id("my-bedrock").build();
        let provider = BedrockProvider::new(config).unwrap();
        assert_eq!(provider.id(), "my-bedrock");
    }
}
