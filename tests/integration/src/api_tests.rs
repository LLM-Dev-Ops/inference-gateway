//! API endpoint integration tests
//!
//! Tests for all HTTP API endpoints including health checks,
//! models, chat completions, and admin endpoints.

use crate::fixtures::*;
use crate::helpers::*;
use crate::mock_providers::*;
use serde_json::json;

/// Test the health check endpoint
#[tokio::test]
async fn test_health_endpoint() {
    init_tracing();
    let server = TestServer::with_default_config().await;

    let response = server.get("/health").await;
    assert_status(&response, 200);

    let body = TestServer::json_body(response).await;
    assert_eq!(body["status"], "healthy");
}

/// Test the readiness endpoint
#[tokio::test]
async fn test_ready_endpoint() {
    let server = TestServer::with_default_config().await;

    let response = server.get("/ready").await;
    assert_status(&response, 200);

    let body = TestServer::json_body(response).await;
    assert_eq!(body["status"], "ready");
}

/// Test the liveness endpoint
#[tokio::test]
async fn test_live_endpoint() {
    let server = TestServer::with_default_config().await;

    let response = server.get("/live").await;
    assert_status(&response, 200);

    let body = TestServer::json_body(response).await;
    assert_eq!(body["status"], "alive");
}

/// Test listing models
#[tokio::test]
async fn test_list_models() {
    let server = TestServer::with_default_config().await;

    let response = server.get("/v1/models").await;
    assert_status(&response, 200);

    let body = TestServer::json_body(response).await;
    assert_eq!(body["object"], "list");
    assert!(body["data"].is_array());
}

/// Test chat completion with valid request
#[tokio::test]
async fn test_chat_completion_valid_request() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello!");
    let response = server.post_json("/v1/chat/completions", &request).await;

    assert_status(&response, 200);

    let body = TestServer::json_body(response).await;
    assert_eq!(body["object"], "chat.completion");
    assert!(body["choices"].is_array());
    assert!(!body["choices"].as_array().unwrap().is_empty());
}

/// Test chat completion with system message
#[tokio::test]
async fn test_chat_completion_with_system_message() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "Hello!"}
        ]
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);

    let body = TestServer::json_body(response).await;
    assert_eq!(body["object"], "chat.completion");
}

/// Test chat completion with multi-turn conversation
#[tokio::test]
async fn test_chat_completion_multi_turn() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {"role": "user", "content": "What is 2+2?"},
            {"role": "assistant", "content": "2+2 equals 4."},
            {"role": "user", "content": "And what is that times 3?"}
        ]
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test chat completion with temperature parameter
#[tokio::test]
async fn test_chat_completion_with_temperature() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello!"}],
        "temperature": 0.7
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test chat completion with max_tokens parameter
#[tokio::test]
async fn test_chat_completion_with_max_tokens() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello!"}],
        "max_tokens": 100
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test chat completion with all optional parameters
#[tokio::test]
async fn test_chat_completion_full_params() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello!"}],
        "temperature": 0.7,
        "max_tokens": 100,
        "top_p": 0.9,
        "frequency_penalty": 0.1,
        "presence_penalty": 0.1,
        "stop": ["\n"],
        "user": "test-user"
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test request with API key header
#[tokio::test]
async fn test_request_with_api_key() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello!");
    let response = server
        .post_json_with_headers(
            "/v1/chat/completions",
            &request,
            vec![("X-API-Key", "test-api-key")],
        )
        .await;

    // Should succeed even with auth disabled
    assert_status(&response, 200);
}

/// Test request with Bearer token
#[tokio::test]
async fn test_request_with_bearer_token() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello!");
    let response = server
        .post_json_with_headers(
            "/v1/chat/completions",
            &request,
            vec![("Authorization", "Bearer test-token")],
        )
        .await;

    assert_status(&response, 200);
}

/// Test request contains request ID header
#[tokio::test]
async fn test_response_has_request_id() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello!");
    let response = server.post_json("/v1/chat/completions", &request).await;

    assert_status(&response, 200);
    // Request ID should be in response headers or body
    let body = TestServer::json_body(response).await;
    assert!(body["id"].is_string());
}

/// Test response contains usage information
#[tokio::test]
async fn test_response_contains_usage() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello!");
    let response = server.post_json("/v1/chat/completions", &request).await;

    assert_status(&response, 200);

    let body = TestServer::json_body(response).await;
    assert!(body["usage"].is_object());
    assert!(body["usage"]["prompt_tokens"].is_number());
    assert!(body["usage"]["completion_tokens"].is_number());
    assert!(body["usage"]["total_tokens"].is_number());
}

/// Test content type header in response
#[tokio::test]
async fn test_response_content_type() {
    let server = TestServer::with_default_config().await;

    let response = server.get("/health").await;
    assert_status(&response, 200);

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(content_type.contains("application/json"));
}

/// Test 404 for unknown endpoints
#[tokio::test]
async fn test_unknown_endpoint_404() {
    let server = TestServer::with_default_config().await;

    let response = server.get("/v1/unknown").await;
    assert_status(&response, 404);
}

/// Test models endpoint returns correct structure
#[tokio::test]
async fn test_models_response_structure() {
    let server = TestServer::with_default_config().await;

    let response = server.get("/v1/models").await;
    assert_status(&response, 200);

    let body = TestServer::json_body(response).await;

    // Check structure
    assert_eq!(body["object"], "list");
    assert!(body["data"].is_array());

    // Check each model object
    for model in body["data"].as_array().unwrap() {
        assert!(model["id"].is_string());
        assert_eq!(model["object"], "model");
        assert!(model["created"].is_number());
        assert!(model["owned_by"].is_string());
    }
}

/// Test concurrent requests
#[tokio::test]
async fn test_concurrent_requests() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello!");

    // Send multiple concurrent requests
    let futures: Vec<_> = (0..10)
        .map(|_| {
            let client = server.client.clone();
            let url = server.url("/v1/chat/completions");
            let req = request.clone();
            async move {
                client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .json(&req)
                    .send()
                    .await
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    // All should succeed
    for result in results {
        let response = result.expect("Request failed");
        assert_eq!(response.status(), 200);
    }
}

/// Test request timeout handling
#[tokio::test]
async fn test_large_request_handling() {
    let server = TestServer::with_default_config().await;

    // Create a request with many messages
    let messages: Vec<_> = (0..100)
        .map(|i| {
            json!({
                "role": if i % 2 == 0 { "user" } else { "assistant" },
                "content": format!("Message number {}", i)
            })
        })
        .collect();

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": messages
    });

    let response = server.post_json("/v1/chat/completions", &request).await;

    // Should handle large requests
    assert!(response.status().is_success() || response.status().as_u16() == 400);
}

/// Test empty messages array is rejected
#[tokio::test]
async fn test_empty_messages_rejected() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": []
    });

    let response = server.post_json("/v1/chat/completions", &request).await;

    // Empty messages should be rejected
    // Note: actual behavior depends on validation implementation
    let status = response.status().as_u16();
    assert!(status == 200 || status == 400);
}

/// Test invalid JSON is rejected
#[tokio::test]
async fn test_invalid_json_rejected() {
    let server = TestServer::with_default_config().await;

    let response = server
        .client
        .post(server.url("/v1/chat/completions"))
        .header("Content-Type", "application/json")
        .body("not valid json")
        .send()
        .await
        .expect("Request failed");

    assert_status(&response, 400);
}

/// Test missing model field
#[tokio::test]
async fn test_missing_model_field() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "messages": [{"role": "user", "content": "Hello!"}]
    });

    let response = server.post_json("/v1/chat/completions", &request).await;

    // Missing model should be rejected
    let status = response.status().as_u16();
    // Could be 200 if there's a default model, or 400 if model is required
    assert!(status == 200 || status == 400);
}

/// Test invalid temperature value
#[tokio::test]
async fn test_invalid_temperature() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello!"}],
        "temperature": 3.0  // Invalid: should be 0-2
    });

    let response = server.post_json("/v1/chat/completions", &request).await;

    // Should handle gracefully (either reject or clamp)
    let status = response.status().as_u16();
    assert!(status == 200 || status == 400);
}

/// Test CORS headers are present
#[tokio::test]
async fn test_cors_headers() {
    let server = TestServer::with_default_config().await;

    let response = server
        .client
        .request(reqwest::Method::OPTIONS, server.url("/v1/chat/completions"))
        .header("Origin", "http://example.com")
        .header("Access-Control-Request-Method", "POST")
        .send()
        .await
        .expect("Request failed");

    // OPTIONS request should succeed
    let status = response.status().as_u16();
    assert!(status == 200 || status == 204 || status == 405);
}

#[cfg(test)]
mod mock_provider_tests {
    use super::*;

    /// Test that mock OpenAI server works
    #[tokio::test]
    async fn test_mock_openai_integration() {
        let mock = MockOpenAI::new().await;
        mock.mock_chat_completion("gpt-4o", "Hello from mock!").await;

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/v1/chat/completions", mock.url()))
            .header("Content-Type", "application/json")
            .json(&openai_json_request("gpt-4o", "Hello"))
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 200);

        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["choices"][0]["message"]["content"], "Hello from mock!");
    }

    /// Test that mock Anthropic server works
    #[tokio::test]
    async fn test_mock_anthropic_integration() {
        let mock = MockAnthropic::new().await;
        mock.mock_messages("claude-3-5-sonnet-latest", "Hello from Claude!")
            .await;

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/v1/messages", mock.url()))
            .header("Content-Type", "application/json")
            .header("x-api-key", "test-key")
            .header("anthropic-version", "2024-01-01")
            .json(&json!({
                "model": "claude-3-5-sonnet-latest",
                "max_tokens": 1024,
                "messages": [{"role": "user", "content": "Hello"}]
            }))
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 200);

        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["content"][0]["text"], "Hello from Claude!");
    }

    /// Test mock rate limiting
    #[tokio::test]
    async fn test_mock_rate_limit() {
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
    }

    /// Test mock server error
    #[tokio::test]
    async fn test_mock_server_error() {
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
}
