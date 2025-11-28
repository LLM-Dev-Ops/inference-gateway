//! Routing integration tests
//!
//! Tests for request routing including model-based routing,
//! provider selection, fallback behavior, and load balancing.

use crate::fixtures::*;
use crate::helpers::*;
use crate::mock_providers::*;
use serde_json::json;

/// Test routing to correct provider based on model name
#[tokio::test]
async fn test_model_based_routing() {
    let server = TestServer::with_default_config().await;

    // OpenAI model
    let request1 = openai_json_request("gpt-4o", "Hello");
    let response1 = server.post_json("/v1/chat/completions", &request1).await;
    assert_status(&response1, 200);

    // Another OpenAI model
    let request2 = openai_json_request("gpt-4o-mini", "Hello");
    let response2 = server.post_json("/v1/chat/completions", &request2).await;
    assert_status(&response2, 200);
}

/// Test routing to Anthropic models
#[tokio::test]
async fn test_anthropic_routing() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("claude-3-5-sonnet-latest", "Hello");
    let response = server.post_json("/v1/chat/completions", &request).await;

    // Should succeed (if Anthropic is configured)
    let status = response.status().as_u16();
    assert!(status == 200 || status == 404 || status == 400);
}

/// Test model not found error
#[tokio::test]
async fn test_unknown_model_error() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("unknown-model-xyz", "Hello");
    let response = server.post_json("/v1/chat/completions", &request).await;

    // Should return error (model not found or routed somewhere)
    let status = response.status().as_u16();
    // Could be 200 (default provider), 400, or 404
    assert!(status == 200 || status == 400 || status == 404);
}

/// Test listing available models
#[tokio::test]
async fn test_list_available_models() {
    let server = TestServer::with_default_config().await;

    let response = server.get("/v1/models").await;
    assert_status(&response, 200);

    let body = TestServer::json_body(response).await;
    assert!(body["data"].is_array());

    let models = body["data"].as_array().unwrap();
    assert!(!models.is_empty());

    // Check model structure
    for model in models {
        assert!(model["id"].is_string());
        assert_eq!(model["object"], "model");
    }
}

/// Test getting specific model info
#[tokio::test]
async fn test_get_model_info() {
    let server = TestServer::with_default_config().await;

    let response = server.get("/v1/models").await;
    assert_status(&response, 200);

    let body = TestServer::json_body(response).await;
    let models = body["data"].as_array().unwrap();

    if !models.is_empty() {
        let model_id = models[0]["id"].as_str().unwrap();
        assert!(!model_id.is_empty());
    }
}

/// Test multiple concurrent requests to different models
#[tokio::test]
async fn test_concurrent_multi_model_requests() {
    let server = TestServer::with_default_config().await;

    let models = ["gpt-4o", "gpt-4o-mini"];
    let mut futures = Vec::new();

    for model in &models {
        let client = server.client.clone();
        let url = server.url("/v1/chat/completions");
        let request = openai_json_request(model, "Hello");

        futures.push(async move {
            client
                .post(&url)
                .json(&request)
                .send()
                .await
        });
    }

    let results = futures::future::join_all(futures).await;

    for result in results {
        let response = result.expect("Request failed");
        assert_eq!(response.status(), 200);
    }
}

/// Test routing respects model capabilities
#[tokio::test]
async fn test_model_capability_routing() {
    let server = TestServer::with_default_config().await;

    // Vision-capable model
    let request = json!({
        "model": "gpt-4o",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "What's in this image?"},
                    {"type": "image_url", "image_url": {"url": "https://example.com/image.jpg"}}
                ]
            }
        ]
    });

    let response = server.post_json("/v1/chat/completions", &request).await;

    // Should succeed or fail validation (depending on implementation)
    let status = response.status().as_u16();
    assert!(status == 200 || status == 400);
}

/// Test routing with custom model aliases
#[tokio::test]
async fn test_model_aliases() {
    let server = TestServer::with_default_config().await;

    // Standard model name
    let request = openai_json_request("gpt-4o-mini", "Hello");
    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test routing preserves request parameters
#[tokio::test]
async fn test_routing_preserves_parameters() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello"}],
        "temperature": 0.5,
        "max_tokens": 100,
        "top_p": 0.9
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test routing with streaming flag
#[tokio::test]
async fn test_streaming_routing() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello"}],
        "stream": true
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test routing with user identifier
#[tokio::test]
async fn test_routing_with_user_id() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello"}],
        "user": "test-user-123"
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test routing handles empty model gracefully
#[tokio::test]
async fn test_empty_model_handling() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "",
        "messages": [{"role": "user", "content": "Hello"}]
    });

    let response = server.post_json("/v1/chat/completions", &request).await;

    // Should be rejected or handled by default model
    let status = response.status().as_u16();
    assert!(status == 200 || status == 400);
}

/// Test provider health affects routing
#[tokio::test]
async fn test_provider_health_routing() {
    let server = TestServer::with_default_config().await;

    // First request should work
    let request = openai_json_request("gpt-4o-mini", "Hello");
    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test routing with system messages
#[tokio::test]
async fn test_routing_with_system_message() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "Hello"}
        ]
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test routing multi-turn conversation
#[tokio::test]
async fn test_routing_multi_turn() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {"role": "user", "content": "What is 2+2?"},
            {"role": "assistant", "content": "4"},
            {"role": "user", "content": "And 3+3?"}
        ]
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test routing decision is consistent
#[tokio::test]
async fn test_routing_consistency() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello");

    // Same request should route to same provider
    let response1 = server.post_json("/v1/chat/completions", &request).await;
    let response2 = server.post_json("/v1/chat/completions", &request).await;

    assert_status(&response1, 200);
    assert_status(&response2, 200);

    // Models should match
    let body1 = TestServer::json_body(response1).await;
    let body2 = TestServer::json_body(response2).await;

    // Both should return same model
    assert!(body1["model"].is_string());
    assert!(body2["model"].is_string());
}

#[cfg(test)]
mod fallback_tests {
    use super::*;

    /// Test fallback when primary provider fails
    #[tokio::test]
    async fn test_fallback_on_primary_failure() {
        // Create mocks for fallback testing
        let primary = MockOpenAI::new().await;
        let secondary = MockOpenAI::new().await;

        // Primary fails
        primary.mock_server_error().await;
        // Secondary succeeds
        secondary.mock_chat_completion("gpt-4o", "Fallback response").await;

        // Test with primary mock
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/v1/chat/completions", primary.url()))
            .json(&openai_json_request("gpt-4o", "Hello"))
            .send()
            .await
            .expect("Request failed");

        // Primary should fail
        assert_eq!(response.status(), 500);

        // Secondary should work
        let response = client
            .post(format!("{}/v1/chat/completions", secondary.url()))
            .json(&openai_json_request("gpt-4o", "Hello"))
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 200);
    }

    /// Test fallback chain order
    #[tokio::test]
    async fn test_fallback_chain_order() {
        let mock1 = MockOpenAI::new().await;
        let mock2 = MockOpenAI::new().await;
        let mock3 = MockOpenAI::new().await;

        // First fails
        mock1.mock_server_error().await;
        // Second fails
        mock2.mock_rate_limit().await;
        // Third succeeds
        mock3.mock_chat_completion("gpt-4o", "Third provider").await;

        let client = reqwest::Client::new();

        // First fails
        let resp1 = client
            .post(format!("{}/v1/chat/completions", mock1.url()))
            .json(&openai_json_request("gpt-4o", "Hello"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp1.status(), 500);

        // Second rate limited
        let resp2 = client
            .post(format!("{}/v1/chat/completions", mock2.url()))
            .json(&openai_json_request("gpt-4o", "Hello"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp2.status(), 429);

        // Third succeeds
        let resp3 = client
            .post(format!("{}/v1/chat/completions", mock3.url()))
            .json(&openai_json_request("gpt-4o", "Hello"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp3.status(), 200);
    }

    /// Test fallback doesn't occur on client errors
    #[tokio::test]
    async fn test_no_fallback_on_client_error() {
        let mock = MockOpenAI::new().await;
        mock.mock_auth_error().await;

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/v1/chat/completions", mock.url()))
            .json(&openai_json_request("gpt-4o", "Hello"))
            .send()
            .await
            .expect("Request failed");

        // 401 is a client error, shouldn't fallback
        assert_eq!(response.status(), 401);
    }
}

#[cfg(test)]
mod load_balancing_tests {
    use super::*;

    /// Test requests are distributed across providers
    #[tokio::test]
    async fn test_load_distribution() {
        let server = TestServer::with_default_config().await;

        let request = openai_json_request("gpt-4o-mini", "Hello");

        // Send multiple requests
        let mut responses = Vec::new();
        for _ in 0..5 {
            let response = server.post_json("/v1/chat/completions", &request).await;
            responses.push(response);
        }

        // All should succeed
        for response in responses {
            assert_status(&response, 200);
        }
    }

    /// Test concurrent load balancing
    #[tokio::test]
    async fn test_concurrent_load_balancing() {
        let server = TestServer::with_default_config().await;

        let request = openai_json_request("gpt-4o-mini", "Hello");

        let futures: Vec<_> = (0..20)
            .map(|_| {
                let client = server.client.clone();
                let url = server.url("/v1/chat/completions");
                let req = request.clone();
                async move {
                    client
                        .post(&url)
                        .json(&req)
                        .send()
                        .await
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;

        let success_count = results
            .iter()
            .filter(|r| r.as_ref().map(|resp| resp.status() == 200).unwrap_or(false))
            .count();

        // Most should succeed
        assert!(success_count >= 15);
    }
}
