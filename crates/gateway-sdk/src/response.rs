//! Response types for the Gateway SDK.

use serde::{Deserialize, Serialize};

/// Response from a chat completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Unique identifier for this completion.
    pub id: String,
    /// Object type (always "chat.completion").
    pub object: String,
    /// Unix timestamp of when the completion was created.
    pub created: i64,
    /// Model used for the completion.
    pub model: String,
    /// List of completion choices.
    pub choices: Vec<ChatChoice>,
    /// Token usage statistics.
    #[serde(default)]
    pub usage: Option<Usage>,
    /// System fingerprint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

impl ChatResponse {
    /// Get the content of the first choice.
    ///
    /// This is a convenience method for the common case of single-choice responses.
    pub fn content(&self) -> &str {
        self.choices
            .first()
            .map(|c| c.message.content.as_str())
            .unwrap_or("")
    }

    /// Get the first choice.
    pub fn first_choice(&self) -> Option<&ChatChoice> {
        self.choices.first()
    }

    /// Get all choices.
    pub fn all_choices(&self) -> &[ChatChoice] {
        &self.choices
    }

    /// Get the finish reason of the first choice.
    pub fn finish_reason(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|c| c.finish_reason.as_deref())
    }

    /// Check if the response was completed normally.
    pub fn is_complete(&self) -> bool {
        self.finish_reason() == Some("stop")
    }

    /// Check if the response was truncated due to length.
    pub fn is_truncated(&self) -> bool {
        self.finish_reason() == Some("length")
    }

    /// Get the total number of tokens used.
    pub fn total_tokens(&self) -> Option<u32> {
        self.usage.as_ref().map(|u| u.total_tokens)
    }

    /// Get the number of prompt tokens.
    pub fn prompt_tokens(&self) -> Option<u32> {
        self.usage.as_ref().map(|u| u.prompt_tokens)
    }

    /// Get the number of completion tokens.
    pub fn completion_tokens(&self) -> Option<u32> {
        self.usage.as_ref().map(|u| u.completion_tokens)
    }
}

/// A single completion choice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    /// Index of this choice.
    pub index: u32,
    /// The generated message.
    pub message: ChatMessage,
    /// Reason for completion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    /// Log probabilities (if requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
}

impl ChatChoice {
    /// Get the content of the message.
    pub fn content(&self) -> &str {
        &self.message.content
    }

    /// Get the role of the message.
    pub fn role(&self) -> &str {
        &self.message.role
    }

    /// Check if this choice completed normally.
    pub fn is_complete(&self) -> bool {
        self.finish_reason.as_deref() == Some("stop")
    }
}

/// A message in a chat response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role of the message sender.
    pub role: String,
    /// Content of the message.
    #[serde(default)]
    pub content: String,
    /// Tool calls made by the assistant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Function call (deprecated, use tool_calls).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,
}

/// A tool call made by the assistant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for the tool call.
    pub id: String,
    /// Type of tool (always "function" for now).
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Function details.
    pub function: FunctionCall,
}

/// A function call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// Name of the function.
    pub name: String,
    /// Arguments as a JSON string.
    pub arguments: String,
}

/// Token usage statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Number of tokens in the prompt.
    pub prompt_tokens: u32,
    /// Number of tokens in the completion.
    pub completion_tokens: u32,
    /// Total number of tokens.
    pub total_tokens: u32,
}

impl Usage {
    /// Create new usage statistics.
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }

    /// Check if any tokens were used.
    pub fn has_usage(&self) -> bool {
        self.total_tokens > 0
    }
}

impl std::ops::Add for Usage {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            prompt_tokens: self.prompt_tokens + other.prompt_tokens,
            completion_tokens: self.completion_tokens + other.completion_tokens,
            total_tokens: self.total_tokens + other.total_tokens,
        }
    }
}

impl std::ops::AddAssign for Usage {
    fn add_assign(&mut self, other: Self) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
    }
}

/// Response containing a list of models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsListResponse {
    /// Object type (always "list").
    pub object: String,
    /// List of available models.
    pub data: Vec<ModelInfo>,
}

/// Information about a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier.
    pub id: String,
    /// Object type (always "model").
    pub object: String,
    /// Unix timestamp of when the model was created.
    pub created: i64,
    /// Organization that owns the model.
    pub owned_by: String,
}

/// Health check response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Health status.
    pub status: String,
    /// Timestamp of the health check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// Version of the gateway.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Additional details.
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

impl HealthResponse {
    /// Check if the status is healthy.
    pub fn is_healthy(&self) -> bool {
        matches!(self.status.to_lowercase().as_str(), "healthy" | "ok" | "up")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_response_content() {
        let response = ChatResponse {
            id: "test-123".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "gpt-4o".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: "Hello, world!".to_string(),
                    tool_calls: None,
                    function_call: None,
                },
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            usage: Some(Usage::new(10, 5)),
            system_fingerprint: None,
        };

        assert_eq!(response.content(), "Hello, world!");
        assert!(response.is_complete());
        assert!(!response.is_truncated());
        assert_eq!(response.total_tokens(), Some(15));
    }

    #[test]
    fn test_usage_arithmetic() {
        let usage1 = Usage::new(10, 5);
        let usage2 = Usage::new(20, 15);
        let total = usage1 + usage2;

        assert_eq!(total.prompt_tokens, 30);
        assert_eq!(total.completion_tokens, 20);
        assert_eq!(total.total_tokens, 50);
    }

    #[test]
    fn test_health_response() {
        let response = HealthResponse {
            status: "healthy".to_string(),
            timestamp: Some("2024-01-01T00:00:00Z".to_string()),
            version: Some("1.0.0".to_string()),
            extra: std::collections::HashMap::new(),
        };

        assert!(response.is_healthy());
    }

    #[test]
    fn test_chat_response_deserialization() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello!"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 9,
                "completion_tokens": 12,
                "total_tokens": 21
            }
        }"#;

        let response: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "chatcmpl-123");
        assert_eq!(response.content(), "Hello!");
        assert_eq!(response.total_tokens(), Some(21));
    }
}
