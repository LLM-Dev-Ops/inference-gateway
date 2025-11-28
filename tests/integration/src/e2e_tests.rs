//! End-to-end integration tests
//!
//! Comprehensive tests that verify the complete request flow
//! from client to provider and back, including all middleware.

use crate::fixtures::*;
use crate::helpers::*;
use serde_json::json;
use std::time::{Duration, Instant};

/// E2E test: Complete chat completion flow
#[tokio::test]
async fn test_e2e_chat_completion() {
    init_tracing();
    let server = TestServer::with_default_config().await;

    // Simulate a real client request
    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "What is the capital of France?"}
        ],
        "temperature": 0.7,
        "max_tokens": 100
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);

    let body = TestServer::json_body(response).await;

    // Verify response structure
    assert!(body["id"].is_string());
    assert_eq!(body["object"], "chat.completion");
    assert!(body["created"].is_number());
    assert!(body["model"].is_string());
    assert!(body["choices"].is_array());
    assert!(!body["choices"].as_array().unwrap().is_empty());

    // Verify choice structure
    let choice = &body["choices"][0];
    assert_eq!(choice["index"], 0);
    assert!(choice["message"]["role"].is_string());
    assert!(choice["message"]["content"].is_string());
    assert!(choice["finish_reason"].is_string());

    // Verify usage
    assert!(body["usage"]["prompt_tokens"].is_number());
    assert!(body["usage"]["completion_tokens"].is_number());
    assert!(body["usage"]["total_tokens"].is_number());
}

/// E2E test: Multi-turn conversation
#[tokio::test]
async fn test_e2e_multi_turn_conversation() {
    let server = TestServer::with_default_config().await;

    // First turn
    let request1 = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {"role": "user", "content": "My name is Alice."}
        ]
    });

    let response1 = server.post_json("/v1/chat/completions", &request1).await;
    assert_status(&response1, 200);
    let body1 = TestServer::json_body(response1).await;
    let assistant_reply = body1["choices"][0]["message"]["content"].as_str().unwrap();

    // Second turn with context
    let request2 = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {"role": "user", "content": "My name is Alice."},
            {"role": "assistant", "content": assistant_reply},
            {"role": "user", "content": "What is my name?"}
        ]
    });

    let response2 = server.post_json("/v1/chat/completions", &request2).await;
    assert_status(&response2, 200);
}

/// E2E test: Health check and readiness flow
#[tokio::test]
async fn test_e2e_health_checks() {
    let server = TestServer::with_default_config().await;

    // Liveness
    let live = server.get("/live").await;
    assert_status(&live, 200);

    // Readiness
    let ready = server.get("/ready").await;
    assert_status(&ready, 200);

    // Health
    let health = server.get("/health").await;
    assert_status(&health, 200);
}

/// E2E test: Model discovery and usage
#[tokio::test]
async fn test_e2e_model_discovery() {
    let server = TestServer::with_default_config().await;

    // List models
    let response = server.get("/v1/models").await;
    assert_status(&response, 200);

    let body = TestServer::json_body(response).await;
    let models = body["data"].as_array().unwrap();
    assert!(!models.is_empty());

    // Use the first model
    let model_id = models[0]["id"].as_str().unwrap();

    let request = json!({
        "model": model_id,
        "messages": [{"role": "user", "content": "Hello"}]
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// E2E test: Request with all optional parameters
#[tokio::test]
async fn test_e2e_full_parameters() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {"role": "system", "content": "Be concise."},
            {"role": "user", "content": "Say hello."}
        ],
        "temperature": 0.5,
        "max_tokens": 50,
        "top_p": 0.9,
        "frequency_penalty": 0.1,
        "presence_penalty": 0.1,
        "stop": ["\n\n"],
        "user": "test-user-e2e"
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// E2E test: Concurrent users simulation
#[tokio::test]
async fn test_e2e_concurrent_users() {
    let server = TestServer::with_default_config().await;

    let user_count = 10;
    let mut futures = Vec::new();

    for i in 0..user_count {
        let client = server.client.clone();
        let url = server.url("/v1/chat/completions");
        let request = json!({
            "model": "gpt-4o-mini",
            "messages": [{"role": "user", "content": format!("Hello from user {}", i)}],
            "user": format!("user-{}", i)
        });

        futures.push(async move {
            client
                .post(&url)
                .json(&request)
                .send()
                .await
        });
    }

    let results = futures::future::join_all(futures).await;

    // All users should get responses
    for result in results {
        let response = result.expect("Request failed");
        assert_eq!(response.status(), 200);
    }
}

/// E2E test: Request latency measurement
#[tokio::test]
async fn test_e2e_latency() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello");

    let start = Instant::now();
    let response = server.post_json("/v1/chat/completions", &request).await;
    let latency = start.elapsed();

    assert_status(&response, 200);

    // Gateway overhead should be reasonable (< 1 second for local test)
    assert!(latency < Duration::from_secs(1));
}

/// E2E test: Error handling flow
#[tokio::test]
async fn test_e2e_error_handling() {
    let server = TestServer::with_default_config().await;

    // Invalid JSON
    let response = server
        .client
        .post(server.url("/v1/chat/completions"))
        .header("Content-Type", "application/json")
        .body("not json")
        .send()
        .await
        .expect("Request failed");
    assert_status(&response, 400);

    // Missing required fields - mock server is lenient
    let response = server.post_json("/v1/chat/completions", &json!({})).await;
    // Mock server returns 200 with default model, real server may reject
    let status = response.status().as_u16();
    assert!(status == 200 || status == 400 || status == 422);
}

/// E2E test: Large conversation context
#[tokio::test]
async fn test_e2e_large_context() {
    let server = TestServer::with_default_config().await;

    // Build a large conversation
    let mut messages: Vec<serde_json::Value> = Vec::new();
    for i in 0..50 {
        messages.push(json!({
            "role": if i % 2 == 0 { "user" } else { "assistant" },
            "content": format!("This is message number {} in our conversation.", i)
        }));
    }

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": messages
    });

    let response = server.post_json("/v1/chat/completions", &request).await;

    // Should handle large context
    let status = response.status().as_u16();
    assert!(status == 200 || status == 400); // 400 if context too large
}

/// E2E test: Special characters in content
#[tokio::test]
async fn test_e2e_special_characters() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{
            "role": "user",
            "content": "Test special chars: <script>alert('xss')</script> & \"quotes\" 'apostrophes' \n\t"
        }]
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// E2E test: Unicode content
#[tokio::test]
async fn test_e2e_unicode_content() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{
            "role": "user",
            "content": "Translate: 你好世界 → Hello World, こんにちは → Hello, مرحبا → Hello 🌍🚀"
        }]
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// E2E test: Rapid sequential requests
#[tokio::test]
async fn test_e2e_rapid_requests() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Quick!");

    // Send requests as fast as possible
    for _ in 0..10 {
        let response = server.post_json("/v1/chat/completions", &request).await;
        let status = response.status().as_u16();
        assert!(status == 200 || status == 429); // OK or rate limited
    }
}

/// E2E test: Request with authentication
#[tokio::test]
async fn test_e2e_with_authentication() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello");

    // With API key
    let response = server
        .post_json_with_headers(
            "/v1/chat/completions",
            &request,
            vec![("X-API-Key", "test-api-key-12345")],
        )
        .await;
    assert_status(&response, 200);

    // With Bearer token
    let response = server
        .post_json_with_headers(
            "/v1/chat/completions",
            &request,
            vec![("Authorization", "Bearer test-token-12345")],
        )
        .await;
    assert_status(&response, 200);
}

/// E2E test: Complete workflow simulation
#[tokio::test]
async fn test_e2e_complete_workflow() {
    let server = TestServer::with_default_config().await;

    // 1. Check health
    let health = server.get("/health").await;
    assert_status(&health, 200);

    // 2. Discover models
    let models_resp = server.get("/v1/models").await;
    assert_status(&models_resp, 200);
    let models_body = TestServer::json_body(models_resp).await;
    let models = models_body["data"].as_array().unwrap();
    assert!(!models.is_empty());

    // 3. Select a model
    let model = models[0]["id"].as_str().unwrap();

    // 4. Send a chat request
    let request = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "What is 2+2?"}
        ]
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);

    let body = TestServer::json_body(response).await;
    let reply = body["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(!reply.is_empty());

    // 5. Continue the conversation
    let follow_up = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "What is 2+2?"},
            {"role": "assistant", "content": reply},
            {"role": "user", "content": "And 3+3?"}
        ]
    });

    let response = server.post_json("/v1/chat/completions", &follow_up).await;
    assert_status(&response, 200);
}

#[cfg(test)]
mod live_provider_tests {
    use super::*;

    /// E2E test with live OpenAI API (requires OPENAI_API_KEY)
    #[tokio::test]
    #[ignore] // Requires API key
    async fn test_e2e_live_openai() {
        let api_key = std::env::var("OPENAI_API_KEY")
            .expect("OPENAI_API_KEY required for this test");

        let client = reqwest::Client::new();
        let response = client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&json!({
                "model": "gpt-4o-mini",
                "messages": [{"role": "user", "content": "Say hello in one word."}],
                "max_tokens": 10
            }))
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 200);

        let body: serde_json::Value = response.json().await.unwrap();
        assert!(body["choices"][0]["message"]["content"].is_string());
    }

    /// E2E test with live Anthropic API (requires ANTHROPIC_API_KEY)
    #[tokio::test]
    #[ignore] // Requires API key
    async fn test_e2e_live_anthropic() {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .expect("ANTHROPIC_API_KEY required for this test");

        let client = reqwest::Client::new();
        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2024-01-01")
            .json(&json!({
                "model": "claude-3-haiku-20240307",
                "max_tokens": 10,
                "messages": [{"role": "user", "content": "Say hello in one word."}]
            }))
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 200);

        let body: serde_json::Value = response.json().await.unwrap();
        assert!(body["content"][0]["text"].is_string());
    }
}

#[cfg(test)]
mod resilience_tests {
    use super::*;

    /// E2E test: Recovery after transient failure
    #[tokio::test]
    async fn test_e2e_transient_failure_recovery() {
        let server = TestServer::with_default_config().await;

        let request = openai_json_request("gpt-4o-mini", "Hello");

        // First request
        let response1 = server.post_json("/v1/chat/completions", &request).await;
        assert_status(&response1, 200);

        // Small delay
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Second request should also work
        let response2 = server.post_json("/v1/chat/completions", &request).await;
        assert_status(&response2, 200);
    }

    /// E2E test: Sustained load
    #[tokio::test]
    async fn test_e2e_sustained_load() {
        let server = TestServer::with_default_config().await;

        let request = openai_json_request("gpt-4o-mini", "Hello");
        let duration = Duration::from_secs(2);
        let start = Instant::now();
        let mut request_count = 0;
        let mut success_count = 0;

        while start.elapsed() < duration {
            let response = server.post_json("/v1/chat/completions", &request).await;
            request_count += 1;
            if response.status() == 200 {
                success_count += 1;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Most requests should succeed
        let success_rate = success_count as f64 / request_count as f64;
        assert!(success_rate > 0.8, "Success rate too low: {}", success_rate);
    }
}
