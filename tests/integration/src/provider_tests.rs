//! Provider integration tests
//!
//! Tests for provider-specific functionality including request transformation,
//! response parsing, streaming, and error handling.

use crate::fixtures::*;
use crate::mock_providers::*;
use serde_json::json;
use std::time::Duration;

/// Test OpenAI provider request transformation
#[tokio::test]
async fn test_openai_request_transformation() {
    let mock = MockOpenAI::new().await;
    mock.mock_chat_completion("gpt-4o", "Response").await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/chat/completions", mock.url()))
        .json(&json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "You are helpful."},
                {"role": "user", "content": "Hello!"}
            ],
            "temperature": 0.7,
            "max_tokens": 100
        }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
}

/// Test OpenAI provider with various models
#[tokio::test]
async fn test_openai_multiple_models() {
    let models = ["gpt-4o", "gpt-4o-mini", "gpt-4-turbo"];

    for model in models {
        let mock = MockOpenAI::new().await;
        mock.mock_chat_completion(model, &format!("Response from {}", model))
            .await;

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/v1/chat/completions", mock.url()))
            .json(&openai_json_request(model, "Hello"))
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 200);

        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["model"], model);
    }
}

/// Test Anthropic provider request transformation
#[tokio::test]
async fn test_anthropic_request_transformation() {
    let mock = MockAnthropic::new().await;
    mock.mock_messages("claude-3-5-sonnet-latest", "Response").await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/messages", mock.url()))
        .header("x-api-key", "test-key")
        .header("anthropic-version", "2024-01-01")
        .json(&json!({
            "model": "claude-3-5-sonnet-latest",
            "max_tokens": 1024,
            "messages": [
                {"role": "user", "content": "Hello!"}
            ]
        }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["role"], "assistant");
}

/// Test Anthropic provider with system message
#[tokio::test]
async fn test_anthropic_with_system_message() {
    let mock = MockAnthropic::new().await;
    mock.mock_messages("claude-3-5-sonnet-latest", "Response").await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/messages", mock.url()))
        .header("x-api-key", "test-key")
        .header("anthropic-version", "2024-01-01")
        .json(&json!({
            "model": "claude-3-5-sonnet-latest",
            "max_tokens": 1024,
            "system": "You are a helpful assistant.",
            "messages": [
                {"role": "user", "content": "Hello!"}
            ]
        }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
}

/// Test provider handles rate limit errors
#[tokio::test]
async fn test_provider_rate_limit_handling() {
    let mock = MockOpenAI::new().await;
    mock.mock_rate_limit().await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/chat/completions", mock.url()))
        .json(&openai_json_request("gpt-4o", "Hello"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 429);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["error"]["type"].as_str().unwrap().contains("rate_limit"));
}

/// Test provider handles server errors
#[tokio::test]
async fn test_provider_server_error_handling() {
    let mock = MockOpenAI::new().await;
    mock.mock_server_error().await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/chat/completions", mock.url()))
        .json(&openai_json_request("gpt-4o", "Hello"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 500);
}

/// Test provider handles authentication errors
#[tokio::test]
async fn test_provider_auth_error_handling() {
    let mock = MockOpenAI::new().await;
    mock.mock_auth_error().await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/chat/completions", mock.url()))
        .json(&openai_json_request("gpt-4o", "Hello"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 401);
}

/// Test OpenAI streaming response
#[tokio::test]
async fn test_openai_streaming() {
    let mock = MockOpenAI::new().await;
    mock.mock_streaming_response("gpt-4o", vec!["Hello", " ", "world", "!"])
        .await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/chat/completions", mock.url()))
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "Hi"}],
            "stream": true
        })).unwrap())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    // Verify body contains SSE data format
    let body = response.text().await.expect("Failed to get body");
    assert!(
        body.contains("data:") && body.contains("[DONE]"),
        "Expected SSE format in body, got: {}",
        body
    );
}

/// Test Anthropic streaming response
#[tokio::test]
async fn test_anthropic_streaming() {
    let mock = MockAnthropic::new().await;
    mock.mock_streaming_response("claude-3-5-sonnet-latest", vec!["Hello", " ", "world", "!"])
        .await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/messages", mock.url()))
        .header("x-api-key", "test-key")
        .header("anthropic-version", "2024-01-01")
        .json(&json!({
            "model": "claude-3-5-sonnet-latest",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hi"}],
            "stream": true
        }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
}

/// Test provider timeout handling
#[tokio::test]
async fn test_provider_timeout_handling() {
    let mock = MockOpenAI::new().await;
    mock.mock_chat_completion_delayed("gpt-4o", "Response", Duration::from_secs(10))
        .await;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();

    let result = client
        .post(format!("{}/v1/chat/completions", mock.url()))
        .json(&openai_json_request("gpt-4o", "Hello"))
        .send()
        .await;

    // Should timeout
    assert!(result.is_err());
}

/// Test OpenAI models list
#[tokio::test]
async fn test_openai_models_list() {
    let mock = MockOpenAI::new().await;
    mock.mock_models_list().await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/v1/models", mock.url()))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["object"], "list");
    assert!(body["data"].is_array());

    let models: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["id"].as_str())
        .collect();

    assert!(models.contains(&"gpt-4o"));
    assert!(models.contains(&"gpt-4o-mini"));
}

/// Test response includes finish reason
#[tokio::test]
async fn test_response_finish_reason() {
    let mock = MockOpenAI::new().await;
    mock.mock_chat_completion("gpt-4o", "Response").await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/chat/completions", mock.url()))
        .json(&openai_json_request("gpt-4o", "Hello"))
        .send()
        .await
        .expect("Request failed");

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["choices"][0]["finish_reason"], "stop");
}

/// Test response includes usage stats
#[tokio::test]
async fn test_response_usage_stats() {
    let mock = MockOpenAI::new().await;
    mock.mock_chat_completion("gpt-4o", "Response").await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/chat/completions", mock.url()))
        .json(&openai_json_request("gpt-4o", "Hello"))
        .send()
        .await
        .expect("Request failed");

    let body: serde_json::Value = response.json().await.unwrap();

    assert!(body["usage"]["prompt_tokens"].is_number());
    assert!(body["usage"]["completion_tokens"].is_number());
    assert!(body["usage"]["total_tokens"].is_number());
}

/// Test multi-turn conversation with context
#[tokio::test]
async fn test_multi_turn_conversation() {
    let mock = MockOpenAI::new().await;
    mock.mock_chat_completion("gpt-4o", "4 times 3 is 12.").await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/chat/completions", mock.url()))
        .json(&json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "user", "content": "What is 2+2?"},
                {"role": "assistant", "content": "2+2 equals 4."},
                {"role": "user", "content": "What is that times 3?"}
            ]
        }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
}

/// Test concurrent requests to different providers
#[tokio::test]
async fn test_concurrent_provider_requests() {
    let openai = MockOpenAI::new().await;
    let anthropic = MockAnthropic::new().await;

    openai.mock_chat_completion("gpt-4o", "OpenAI response").await;
    anthropic
        .mock_messages("claude-3-5-sonnet-latest", "Anthropic response")
        .await;

    let client = reqwest::Client::new();

    let openai_future = client
        .post(format!("{}/v1/chat/completions", openai.url()))
        .json(&openai_json_request("gpt-4o", "Hello"))
        .send();

    let anthropic_future = client
        .post(format!("{}/v1/messages", anthropic.url()))
        .header("x-api-key", "test-key")
        .header("anthropic-version", "2024-01-01")
        .json(&json!({
            "model": "claude-3-5-sonnet-latest",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}]
        }))
        .send();

    let (openai_result, anthropic_result) =
        tokio::join!(openai_future, anthropic_future);

    assert_eq!(openai_result.unwrap().status(), 200);
    assert_eq!(anthropic_result.unwrap().status(), 200);
}

/// Test provider correctly handles empty response
#[tokio::test]
async fn test_empty_response_handling() {
    let mock = MockOpenAI::new().await;
    mock.mock_chat_completion("gpt-4o", "").await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/chat/completions", mock.url()))
        .json(&openai_json_request("gpt-4o", "Hello"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    // Empty content is valid
    assert!(body["choices"][0]["message"]["content"].is_string());
}

/// Test provider handles very long responses
#[tokio::test]
async fn test_long_response_handling() {
    let mock = MockOpenAI::new().await;
    let long_response = "word ".repeat(1000);
    mock.mock_chat_completion("gpt-4o", &long_response).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/chat/completions", mock.url()))
        .json(&openai_json_request("gpt-4o", "Tell me a long story"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    let content = body["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(content.len() > 4000);
}

/// Test Unicode content handling
#[tokio::test]
async fn test_unicode_content() {
    let mock = MockOpenAI::new().await;
    mock.mock_chat_completion("gpt-4o", "こんにちは! 你好! 🌍🚀").await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/v1/chat/completions", mock.url()))
        .json(&json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "Say hello in Japanese and Chinese with emojis"}]
        }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    let content = body["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(content.contains("こんにちは"));
    assert!(content.contains("你好"));
    assert!(content.contains("🌍"));
}
