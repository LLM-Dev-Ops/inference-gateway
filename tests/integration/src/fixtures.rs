//! Test fixtures and sample data for integration tests

use gateway_core::{
    ChatMessage, Choice, FinishReason, GatewayRequest, GatewayResponse, MessageContent,
    MessageRole, ModelObject, Usage,
};
use serde_json::{json, Value};

/// Create a simple chat request for testing
pub fn simple_chat_request(model: &str) -> GatewayRequest {
    GatewayRequest::builder()
        .model(model)
        .messages(vec![ChatMessage {
            role: MessageRole::User,
            content: MessageContent::Text("Hello, how are you?".to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }])
        .build()
        .expect("Failed to build request")
}

/// Create a chat request with system message
pub fn chat_request_with_system(model: &str, system: &str, user: &str) -> GatewayRequest {
    GatewayRequest::builder()
        .model(model)
        .messages(vec![
            ChatMessage {
                role: MessageRole::System,
                content: MessageContent::Text(system.to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: MessageContent::Text(user.to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ])
        .build()
        .expect("Failed to build request")
}

/// Create a multi-turn conversation request
pub fn multi_turn_chat_request(model: &str) -> GatewayRequest {
    GatewayRequest::builder()
        .model(model)
        .messages(vec![
            ChatMessage {
                role: MessageRole::User,
                content: MessageContent::Text("What is 2 + 2?".to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: MessageRole::Assistant,
                content: MessageContent::Text("2 + 2 equals 4.".to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: MessageContent::Text("And what is that multiplied by 3?".to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ])
        .build()
        .expect("Failed to build request")
}

/// Create a streaming chat request
pub fn streaming_chat_request(model: &str) -> GatewayRequest {
    GatewayRequest::builder()
        .model(model)
        .messages(vec![ChatMessage {
            role: MessageRole::User,
            content: MessageContent::Text("Tell me a short story.".to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }])
        .stream(true)
        .build()
        .expect("Failed to build request")
}

/// Create a chat request with specific parameters
pub fn parameterized_chat_request(
    model: &str,
    temperature: f32,
    max_tokens: u32,
) -> GatewayRequest {
    GatewayRequest::builder()
        .model(model)
        .messages(vec![ChatMessage {
            role: MessageRole::User,
            content: MessageContent::Text("Hello!".to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }])
        .temperature(temperature)
        .max_tokens(max_tokens)
        .build()
        .expect("Failed to build request")
}

/// Create a sample successful response
pub fn sample_response(model: &str) -> GatewayResponse {
    GatewayResponse::builder()
        .id("chatcmpl-test123")
        .model(model)
        .choice(Choice::new(
            0,
            "Hello! I'm doing well, thank you for asking.",
            FinishReason::Stop,
        ))
        .usage(Usage {
            prompt_tokens: 15,
            completion_tokens: 12,
            total_tokens: 27,
        })
        .build()
}

/// Create an OpenAI-format JSON request
pub fn openai_json_request(model: &str, message: &str) -> Value {
    json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": message
            }
        ]
    })
}

/// Create an OpenAI-format JSON request with all parameters
pub fn openai_json_request_full(
    model: &str,
    messages: Vec<Value>,
    temperature: f64,
    max_tokens: u32,
    stream: bool,
) -> Value {
    json!({
        "model": model,
        "messages": messages,
        "temperature": temperature,
        "max_tokens": max_tokens,
        "stream": stream
    })
}

/// Create a sample OpenAI-format JSON response
pub fn openai_json_response(model: &str, content: &str) -> Value {
    json!({
        "id": "chatcmpl-test123",
        "object": "chat.completion",
        "created": 1698959748,
        "model": model,
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 15,
            "completion_tokens": 12,
            "total_tokens": 27
        }
    })
}

/// Create a sample streaming chunk
pub fn openai_streaming_chunk(model: &str, content: &str, is_done: bool) -> Value {
    if is_done {
        json!({
            "id": "chatcmpl-test123",
            "object": "chat.completion.chunk",
            "created": 1698959748,
            "model": model,
            "choices": [
                {
                    "index": 0,
                    "delta": {},
                    "finish_reason": "stop"
                }
            ]
        })
    } else {
        json!({
            "id": "chatcmpl-test123",
            "object": "chat.completion.chunk",
            "created": 1698959748,
            "model": model,
            "choices": [
                {
                    "index": 0,
                    "delta": {
                        "content": content
                    },
                    "finish_reason": null
                }
            ]
        })
    }
}

/// Create an Anthropic-format JSON request
pub fn anthropic_json_request(model: &str, message: &str) -> Value {
    json!({
        "model": model,
        "max_tokens": 1024,
        "messages": [
            {
                "role": "user",
                "content": message
            }
        ]
    })
}

/// Create a sample Anthropic-format JSON response
pub fn anthropic_json_response(model: &str, content: &str) -> Value {
    json!({
        "id": "msg_test123",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "text",
                "text": content
            }
        ],
        "model": model,
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 15,
            "output_tokens": 12
        }
    })
}

/// Create a sample models list response
pub fn models_list_response() -> Value {
    json!({
        "object": "list",
        "data": [
            {
                "id": "gpt-4o",
                "object": "model",
                "created": 1698959748,
                "owned_by": "openai"
            },
            {
                "id": "gpt-4o-mini",
                "object": "model",
                "created": 1698959748,
                "owned_by": "openai"
            },
            {
                "id": "claude-3-5-sonnet-latest",
                "object": "model",
                "created": 1698959748,
                "owned_by": "anthropic"
            }
        ]
    })
}

/// Create sample model objects
pub fn sample_models() -> Vec<ModelObject> {
    vec![
        ModelObject {
            id: "gpt-4o".to_string(),
            object: "model".to_string(),
            created: 1698959748,
            owned_by: "openai".to_string(),
        },
        ModelObject {
            id: "gpt-4o-mini".to_string(),
            object: "model".to_string(),
            created: 1698959748,
            owned_by: "openai".to_string(),
        },
        ModelObject {
            id: "claude-3-5-sonnet-latest".to_string(),
            object: "model".to_string(),
            created: 1698959748,
            owned_by: "anthropic".to_string(),
        },
    ]
}

/// Create an error response
pub fn error_response(error_type: &str, message: &str, code: &str) -> Value {
    json!({
        "error": {
            "type": error_type,
            "message": message,
            "code": code
        }
    })
}

/// Create a rate limit error response
pub fn rate_limit_error_response() -> Value {
    error_response(
        "rate_limit_error",
        "Rate limit exceeded. Please retry after 60 seconds.",
        "rate_limit_exceeded",
    )
}

/// Create an authentication error response
pub fn auth_error_response() -> Value {
    error_response(
        "authentication_error",
        "Invalid API key provided.",
        "invalid_api_key",
    )
}

/// Create a model not found error response
pub fn model_not_found_error_response(model: &str) -> Value {
    error_response(
        "invalid_request_error",
        &format!("The model '{}' does not exist", model),
        "model_not_found",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_chat_request() {
        let request = simple_chat_request("gpt-4o");
        assert_eq!(request.model, "gpt-4o");
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, MessageRole::User);
    }

    #[test]
    fn test_chat_request_with_system() {
        let request = chat_request_with_system("gpt-4o", "You are helpful.", "Hello");
        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.messages[0].role, MessageRole::System);
        assert_eq!(request.messages[1].role, MessageRole::User);
    }

    #[test]
    fn test_streaming_chat_request() {
        let request = streaming_chat_request("gpt-4o");
        assert!(request.stream);
    }

    #[test]
    fn test_openai_json_request() {
        let json = openai_json_request("gpt-4o", "Hello");
        assert_eq!(json["model"], "gpt-4o");
        assert_eq!(json["messages"][0]["content"], "Hello");
    }

    #[test]
    fn test_openai_json_response() {
        let json = openai_json_response("gpt-4o", "Hi there!");
        assert_eq!(json["model"], "gpt-4o");
        assert_eq!(json["choices"][0]["message"]["content"], "Hi there!");
    }
}
