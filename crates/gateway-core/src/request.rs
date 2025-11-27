//! Request types for the gateway.
//!
//! This module defines the unified request format that abstracts across all LLM providers.

use crate::types::{MaxTokens, ModelId, RequestId, Temperature, TopK, TopP};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unified gateway request that abstracts all providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayRequest {
    /// Unique request identifier
    #[serde(default = "RequestId::generate")]
    pub id: RequestId,

    /// Target model (e.g., "gpt-4", "claude-3-opus")
    pub model: String,

    /// Chat messages for conversation
    pub messages: Vec<ChatMessage>,

    /// Sampling temperature (0.0 - 2.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Maximum tokens to generate
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Top-p (nucleus sampling) parameter
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Top-k sampling parameter (provider-specific)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,

    /// Frequency penalty (-2.0 to 2.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    /// Presence penalty (-2.0 to 2.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    /// Stop sequences
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    /// Enable streaming response
    #[serde(default)]
    pub stream: bool,

    /// Number of completions to generate
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,

    /// Tool/function definitions
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,

    /// Tool choice configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    /// Response format configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,

    /// Seed for deterministic generation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,

    /// User identifier for abuse tracking
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Request metadata for routing/billing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RequestMetadata>,
}

impl GatewayRequest {
    /// Create a new builder for `GatewayRequest`
    #[must_use]
    pub fn builder() -> GatewayRequestBuilder {
        GatewayRequestBuilder::default()
    }

    /// Get validated temperature
    ///
    /// # Errors
    /// Returns error if temperature is out of range
    pub fn validated_temperature(&self) -> Result<Option<Temperature>, crate::error::GatewayError> {
        self.temperature
            .map(Temperature::new)
            .transpose()
            .map_err(Into::into)
    }

    /// Get validated max_tokens
    ///
    /// # Errors
    /// Returns error if max_tokens is out of range
    pub fn validated_max_tokens(&self) -> Result<Option<MaxTokens>, crate::error::GatewayError> {
        self.max_tokens
            .map(MaxTokens::new)
            .transpose()
            .map_err(Into::into)
    }

    /// Get validated top_p
    ///
    /// # Errors
    /// Returns error if top_p is out of range
    pub fn validated_top_p(&self) -> Result<Option<TopP>, crate::error::GatewayError> {
        self.top_p.map(TopP::new).transpose().map_err(Into::into)
    }

    /// Get validated top_k
    ///
    /// # Errors
    /// Returns error if top_k is out of range
    pub fn validated_top_k(&self) -> Result<Option<TopK>, crate::error::GatewayError> {
        self.top_k.map(TopK::new).transpose().map_err(Into::into)
    }

    /// Get validated model ID
    ///
    /// # Errors
    /// Returns error if model ID is invalid
    pub fn validated_model(&self) -> Result<ModelId, crate::error::GatewayError> {
        ModelId::new(&self.model).map_err(Into::into)
    }

    /// Validate the entire request
    ///
    /// # Errors
    /// Returns error if any field is invalid
    pub fn validate(&self) -> Result<(), crate::error::GatewayError> {
        // Validate model
        self.validated_model()?;

        // Validate messages
        if self.messages.is_empty() {
            return Err(crate::error::GatewayError::validation(
                "messages cannot be empty",
                Some("messages".to_string()),
                "empty_messages",
            ));
        }

        // Validate temperature if present
        self.validated_temperature()?;

        // Validate max_tokens if present
        self.validated_max_tokens()?;

        // Validate top_p if present
        self.validated_top_p()?;

        // Validate top_k if present
        self.validated_top_k()?;

        // Validate frequency_penalty if present
        if let Some(fp) = self.frequency_penalty {
            if !(-2.0..=2.0).contains(&fp) {
                return Err(crate::error::GatewayError::validation(
                    format!("frequency_penalty must be between -2.0 and 2.0, got {fp}"),
                    Some("frequency_penalty".to_string()),
                    "invalid_frequency_penalty",
                ));
            }
        }

        // Validate presence_penalty if present
        if let Some(pp) = self.presence_penalty {
            if !(-2.0..=2.0).contains(&pp) {
                return Err(crate::error::GatewayError::validation(
                    format!("presence_penalty must be between -2.0 and 2.0, got {pp}"),
                    Some("presence_penalty".to_string()),
                    "invalid_presence_penalty",
                ));
            }
        }

        // Validate n if present
        if let Some(n) = self.n {
            if n == 0 || n > 128 {
                return Err(crate::error::GatewayError::validation(
                    format!("n must be between 1 and 128, got {n}"),
                    Some("n".to_string()),
                    "invalid_n",
                ));
            }
        }

        Ok(())
    }
}

/// Builder for `GatewayRequest`
#[derive(Debug, Default)]
pub struct GatewayRequestBuilder {
    id: Option<RequestId>,
    model: Option<String>,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    top_p: Option<f32>,
    top_k: Option<u32>,
    frequency_penalty: Option<f32>,
    presence_penalty: Option<f32>,
    stop: Option<Vec<String>>,
    stream: bool,
    n: Option<u32>,
    tools: Option<Vec<ToolDefinition>>,
    tool_choice: Option<ToolChoice>,
    response_format: Option<ResponseFormat>,
    seed: Option<i64>,
    user: Option<String>,
    metadata: Option<RequestMetadata>,
}

impl GatewayRequestBuilder {
    /// Set the request ID
    #[must_use]
    pub fn id(mut self, id: RequestId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the model
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the messages
    #[must_use]
    pub fn messages(mut self, messages: Vec<ChatMessage>) -> Self {
        self.messages = messages;
        self
    }

    /// Add a message
    #[must_use]
    pub fn message(mut self, message: ChatMessage) -> Self {
        self.messages.push(message);
        self
    }

    /// Set the temperature
    #[must_use]
    pub fn temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set max_tokens
    #[must_use]
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set top_p
    #[must_use]
    pub fn top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set top_k
    #[must_use]
    pub fn top_k(mut self, top_k: u32) -> Self {
        self.top_k = Some(top_k);
        self
    }

    /// Set frequency_penalty
    #[must_use]
    pub fn frequency_penalty(mut self, frequency_penalty: f32) -> Self {
        self.frequency_penalty = Some(frequency_penalty);
        self
    }

    /// Set presence_penalty
    #[must_use]
    pub fn presence_penalty(mut self, presence_penalty: f32) -> Self {
        self.presence_penalty = Some(presence_penalty);
        self
    }

    /// Set stop sequences
    #[must_use]
    pub fn stop(mut self, stop: Vec<String>) -> Self {
        self.stop = Some(stop);
        self
    }

    /// Enable streaming
    #[must_use]
    pub fn stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    /// Set n (number of completions)
    #[must_use]
    pub fn n(mut self, n: u32) -> Self {
        self.n = Some(n);
        self
    }

    /// Set tools
    #[must_use]
    pub fn tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set tool_choice
    #[must_use]
    pub fn tool_choice(mut self, tool_choice: ToolChoice) -> Self {
        self.tool_choice = Some(tool_choice);
        self
    }

    /// Set response_format
    #[must_use]
    pub fn response_format(mut self, response_format: ResponseFormat) -> Self {
        self.response_format = Some(response_format);
        self
    }

    /// Set seed
    #[must_use]
    pub fn seed(mut self, seed: i64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Set user
    #[must_use]
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Set metadata
    #[must_use]
    pub fn metadata(mut self, metadata: RequestMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Build the request
    ///
    /// # Errors
    /// Returns error if required fields are missing
    pub fn build(self) -> Result<GatewayRequest, crate::error::GatewayError> {
        let model = self.model.ok_or_else(|| {
            crate::error::GatewayError::validation(
                "model is required",
                Some("model".to_string()),
                "missing_model",
            )
        })?;

        if self.messages.is_empty() {
            return Err(crate::error::GatewayError::validation(
                "messages cannot be empty",
                Some("messages".to_string()),
                "empty_messages",
            ));
        }

        let request = GatewayRequest {
            id: self.id.unwrap_or_else(RequestId::generate),
            model,
            messages: self.messages,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            top_p: self.top_p,
            top_k: self.top_k,
            frequency_penalty: self.frequency_penalty,
            presence_penalty: self.presence_penalty,
            stop: self.stop,
            stream: self.stream,
            n: self.n,
            tools: self.tools,
            tool_choice: self.tool_choice,
            response_format: self.response_format,
            seed: self.seed,
            user: self.user,
            metadata: self.metadata,
        };

        // Validate the built request
        request.validate()?;

        Ok(request)
    }
}

/// Chat message with role and content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role of the message author
    pub role: MessageRole,

    /// Content of the message
    pub content: MessageContent,

    /// Optional name of the author
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Tool calls made by the assistant
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    /// Tool call ID for tool response messages
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    /// Create a system message
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: MessageContent::Text(content.into()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a user message
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: MessageContent::Text(content.into()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create an assistant message
    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: MessageContent::Text(content.into()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a tool response message
    #[must_use]
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: MessageContent::Text(content.into()),
            name: None,
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    /// Get the text content if available
    #[must_use]
    pub fn text_content(&self) -> Option<&str> {
        match &self.content {
            MessageContent::Text(s) => Some(s),
            MessageContent::Parts(_) => None,
        }
    }
}

/// Message role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message
    System,
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// Tool response message
    Tool,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
            Self::Tool => write!(f, "tool"),
        }
    }
}

/// Message content (text or multimodal parts)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Simple text content
    Text(String),
    /// Multimodal content parts
    Parts(Vec<ContentPart>),
}

impl MessageContent {
    /// Get as text if this is a text content
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            Self::Parts(_) => None,
        }
    }

    /// Check if content is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Text(s) => s.is_empty(),
            Self::Parts(parts) => parts.is_empty(),
        }
    }
}

/// Content part for multimodal messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// Text content part
    Text {
        /// The text content
        text: String,
    },
    /// Image content part
    ImageUrl {
        /// Image URL details
        image_url: ImageUrl,
    },
}

/// Image URL for vision models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    /// URL of the image
    pub url: String,
    /// Detail level for processing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<ImageDetail>,
}

/// Image detail level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    /// Auto detail level
    Auto,
    /// Low detail level
    Low,
    /// High detail level
    High,
}

/// Tool/function definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool type (currently only "function" is supported)
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Function definition
    pub function: FunctionDefinition,
}

/// Function definition for tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    /// Function name
    pub name: String,
    /// Function description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Function parameters (JSON Schema)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// Tool call made by the assistant
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique ID for this tool call
    pub id: String,
    /// Tool type
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Function call details
    pub function: FunctionCall,
}

/// Function call details
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionCall {
    /// Function name
    pub name: String,
    /// Function arguments as JSON string
    pub arguments: String,
}

/// Tool choice configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// String choice: "none", "auto", "required"
    String(String),
    /// Specific tool choice
    Tool {
        /// Tool type
        #[serde(rename = "type")]
        tool_type: String,
        /// Function to call
        function: ToolChoiceFunction,
    },
}

/// Function choice for specific tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceFunction {
    /// Function name
    pub name: String,
}

/// Response format configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    /// Format type: "text" or "json_object"
    #[serde(rename = "type")]
    pub format_type: String,
}

/// Request metadata for routing and billing
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestMetadata {
    /// Tenant ID for multi-tenancy
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,

    /// Project ID for cost attribution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,

    /// Environment (development, staging, production)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,

    /// Priority level (0-100, higher = more important)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,

    /// Request tags for filtering/routing
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub tags: HashMap<String, String>,

    /// Preferred provider ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_provider: Option<String>,

    /// Fallback provider IDs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_providers: Option<Vec<String>>,

    /// Request timestamp
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_builder() {
        let request = GatewayRequest::builder()
            .model("gpt-4")
            .message(ChatMessage::user("Hello"))
            .temperature(0.7)
            .max_tokens(100)
            .build();

        assert!(request.is_ok());
        let request = request.expect("should build");
        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(100));
    }

    #[test]
    fn test_request_builder_missing_model() {
        let request = GatewayRequest::builder()
            .message(ChatMessage::user("Hello"))
            .build();

        assert!(request.is_err());
    }

    #[test]
    fn test_request_builder_missing_messages() {
        let request = GatewayRequest::builder().model("gpt-4").build();

        assert!(request.is_err());
    }

    #[test]
    fn test_request_validation_invalid_temperature() {
        let request = GatewayRequest::builder()
            .model("gpt-4")
            .message(ChatMessage::user("Hello"))
            .temperature(3.0)
            .build();

        assert!(request.is_err());
    }

    #[test]
    fn test_chat_message_constructors() {
        let system = ChatMessage::system("You are helpful");
        assert!(matches!(system.role, MessageRole::System));

        let user = ChatMessage::user("Hello");
        assert!(matches!(user.role, MessageRole::User));

        let assistant = ChatMessage::assistant("Hi there!");
        assert!(matches!(assistant.role, MessageRole::Assistant));

        let tool = ChatMessage::tool("call_123", "result");
        assert!(matches!(tool.role, MessageRole::Tool));
        assert_eq!(tool.tool_call_id, Some("call_123".to_string()));
    }

    #[test]
    fn test_message_content_serialization() {
        let text_content = MessageContent::Text("Hello".to_string());
        let json = serde_json::to_string(&text_content).expect("serialize");
        assert_eq!(json, "\"Hello\"");

        let parts_content = MessageContent::Parts(vec![ContentPart::Text {
            text: "Hello".to_string(),
        }]);
        let json = serde_json::to_string(&parts_content).expect("serialize");
        assert!(json.contains("text"));
    }
}
