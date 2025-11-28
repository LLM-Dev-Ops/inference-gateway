//! Mock LLM providers for integration testing
//!
//! Provides wiremock-based mock servers that simulate OpenAI and Anthropic APIs.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Mock OpenAI API server
pub struct MockOpenAI {
    pub server: MockServer,
    pub call_count: Arc<AtomicUsize>,
}

impl MockOpenAI {
    /// Create a new mock OpenAI server
    pub async fn new() -> Self {
        let server = MockServer::start().await;
        let call_count = Arc::new(AtomicUsize::new(0));

        Self { server, call_count }
    }

    /// Get the base URL for this mock server
    pub fn url(&self) -> String {
        self.server.uri()
    }

    /// Get the number of calls made to the mock
    pub fn calls(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    /// Setup a successful chat completion response
    pub async fn mock_chat_completion(&self, model: &str, response_content: &str) {
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(openai_chat_response(model, response_content))
                    .append_header("Content-Type", "application/json"),
            )
            .expect(1..)
            .mount(&self.server)
            .await;
    }

    /// Setup a chat completion that returns after a delay
    pub async fn mock_chat_completion_delayed(
        &self,
        model: &str,
        response_content: &str,
        delay: Duration,
    ) {
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(openai_chat_response(model, response_content))
                    .set_delay(delay),
            )
            .mount(&self.server)
            .await;
    }

    /// Setup a rate limit error response
    pub async fn mock_rate_limit(&self) {
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(429)
                    .set_body_json(openai_error_response(
                        "rate_limit_exceeded",
                        "Rate limit exceeded",
                    ))
                    .append_header("Retry-After", "60"),
            )
            .mount(&self.server)
            .await;
    }

    /// Setup a server error response
    pub async fn mock_server_error(&self) {
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_json(openai_error_response("server_error", "Internal server error")),
            )
            .mount(&self.server)
            .await;
    }

    /// Setup an authentication error response
    pub async fn mock_auth_error(&self) {
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(openai_error_response(
                        "invalid_api_key",
                        "Incorrect API key provided",
                    )),
            )
            .mount(&self.server)
            .await;
    }

    /// Setup a models list response
    pub async fn mock_models_list(&self) {
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(openai_models_response()),
            )
            .mount(&self.server)
            .await;
    }

    /// Setup a streaming response
    pub async fn mock_streaming_response(&self, model: &str, chunks: Vec<&str>) {
        let mut body = String::new();
        for chunk in chunks.iter() {
            body.push_str(&format!(
                "data: {}\n\n",
                serde_json::to_string(&openai_streaming_chunk(model, chunk, false)).unwrap()
            ));
        }
        // Final chunk with finish_reason
        body.push_str(&format!(
            "data: {}\n\n",
            serde_json::to_string(&openai_streaming_chunk(model, "", true)).unwrap()
        ));
        body.push_str("data: [DONE]\n\n");

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(body_partial_json(json!({"stream": true})))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(body)
                    .append_header("Content-Type", "text/event-stream"),
            )
            .mount(&self.server)
            .await;
    }

    /// Setup a response that fails N times then succeeds
    pub async fn mock_flaky_endpoint(&self, model: &str, fail_count: usize) {
        // First, mount the failure responses
        for _ in 0..fail_count {
            Mock::given(method("POST"))
                .and(path("/v1/chat/completions"))
                .respond_with(ResponseTemplate::new(503))
                .expect(1)
                .mount(&self.server)
                .await;
        }

        // Then mount the success response
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(openai_chat_response(model, "Success after retries")),
            )
            .mount(&self.server)
            .await;
    }
}

/// Mock Anthropic API server
pub struct MockAnthropic {
    pub server: MockServer,
    pub call_count: Arc<AtomicUsize>,
}

impl MockAnthropic {
    /// Create a new mock Anthropic server
    pub async fn new() -> Self {
        let server = MockServer::start().await;
        let call_count = Arc::new(AtomicUsize::new(0));

        Self { server, call_count }
    }

    /// Get the base URL for this mock server
    pub fn url(&self) -> String {
        self.server.uri()
    }

    /// Get the number of calls made to the mock
    pub fn calls(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    /// Setup a successful message response
    pub async fn mock_messages(&self, model: &str, response_content: &str) {
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(anthropic_message_response(model, response_content)),
            )
            .mount(&self.server)
            .await;
    }

    /// Setup a rate limit error response
    pub async fn mock_rate_limit(&self) {
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(429)
                    .set_body_json(anthropic_error_response(
                        "rate_limit_error",
                        "Rate limit exceeded",
                    )),
            )
            .mount(&self.server)
            .await;
    }

    /// Setup a server error response
    pub async fn mock_server_error(&self) {
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_json(anthropic_error_response("api_error", "Internal error")),
            )
            .mount(&self.server)
            .await;
    }

    /// Setup a streaming response
    pub async fn mock_streaming_response(&self, model: &str, chunks: Vec<&str>) {
        let mut body = String::new();

        // Message start event
        body.push_str(&format!(
            "event: message_start\ndata: {}\n\n",
            serde_json::to_string(&json!({
                "type": "message_start",
                "message": {
                    "id": "msg_test123",
                    "type": "message",
                    "role": "assistant",
                    "model": model,
                    "content": []
                }
            }))
            .unwrap()
        ));

        // Content block start
        body.push_str(&format!(
            "event: content_block_start\ndata: {}\n\n",
            serde_json::to_string(&json!({
                "type": "content_block_start",
                "index": 0,
                "content_block": {
                    "type": "text",
                    "text": ""
                }
            }))
            .unwrap()
        ));

        // Content deltas
        for chunk in chunks {
            body.push_str(&format!(
                "event: content_block_delta\ndata: {}\n\n",
                serde_json::to_string(&json!({
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {
                        "type": "text_delta",
                        "text": chunk
                    }
                }))
                .unwrap()
            ));
        }

        // Content block stop
        body.push_str(&format!(
            "event: content_block_stop\ndata: {}\n\n",
            serde_json::to_string(&json!({
                "type": "content_block_stop",
                "index": 0
            }))
            .unwrap()
        ));

        // Message stop
        body.push_str(&format!(
            "event: message_stop\ndata: {}\n\n",
            serde_json::to_string(&json!({
                "type": "message_stop"
            }))
            .unwrap()
        ));

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(body_partial_json(json!({"stream": true})))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(body)
                    .append_header("Content-Type", "text/event-stream"),
            )
            .mount(&self.server)
            .await;
    }
}

// Helper functions for creating response payloads

fn openai_chat_response(model: &str, content: &str) -> Value {
    json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
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
            "prompt_tokens": 10,
            "completion_tokens": content.split_whitespace().count(),
            "total_tokens": 10 + content.split_whitespace().count()
        }
    })
}

fn openai_streaming_chunk(model: &str, content: &str, is_final: bool) -> Value {
    if is_final {
        json!({
            "id": "chatcmpl-test",
            "object": "chat.completion.chunk",
            "created": chrono::Utc::now().timestamp(),
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
            "id": "chatcmpl-test",
            "object": "chat.completion.chunk",
            "created": chrono::Utc::now().timestamp(),
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

fn openai_error_response(error_type: &str, message: &str) -> Value {
    json!({
        "error": {
            "type": error_type,
            "message": message,
            "param": null,
            "code": error_type
        }
    })
}

fn openai_models_response() -> Value {
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
                "id": "gpt-4-turbo",
                "object": "model",
                "created": 1698959748,
                "owned_by": "openai"
            }
        ]
    })
}

fn anthropic_message_response(model: &str, content: &str) -> Value {
    json!({
        "id": format!("msg_{}", uuid::Uuid::new_v4()),
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
        "stop_sequence": null,
        "usage": {
            "input_tokens": 10,
            "output_tokens": content.split_whitespace().count()
        }
    })
}

fn anthropic_error_response(error_type: &str, message: &str) -> Value {
    json!({
        "type": "error",
        "error": {
            "type": error_type,
            "message": message
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_openai_chat() {
        let mock = MockOpenAI::new().await;
        mock.mock_chat_completion("gpt-4o", "Hello!").await;

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/v1/chat/completions", mock.url()))
            .json(&json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "Hi"}]
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["choices"][0]["message"]["content"], "Hello!");
    }

    #[tokio::test]
    async fn test_mock_openai_rate_limit() {
        let mock = MockOpenAI::new().await;
        mock.mock_rate_limit().await;

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/v1/chat/completions", mock.url()))
            .json(&json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "Hi"}]
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 429);
    }

    #[tokio::test]
    async fn test_mock_anthropic_messages() {
        let mock = MockAnthropic::new().await;
        mock.mock_messages("claude-3-5-sonnet-latest", "Hi there!").await;

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/v1/messages", mock.url()))
            .header("x-api-key", "test-key")
            .header("anthropic-version", "2024-01-01")
            .json(&json!({
                "model": "claude-3-5-sonnet-latest",
                "max_tokens": 1024,
                "messages": [{"role": "user", "content": "Hello"}]
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["content"][0]["text"], "Hi there!");
    }

    #[tokio::test]
    async fn test_mock_openai_models() {
        let mock = MockOpenAI::new().await;
        mock.mock_models_list().await;

        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/v1/models", mock.url()))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["object"], "list");
        assert!(body["data"].as_array().unwrap().len() >= 3);
    }
}
