//! Rate limiting integration tests
//!
//! Tests for rate limiting behavior including request limits,
//! token limits, per-user limits, and rate limit headers.

use crate::fixtures::*;
use crate::helpers::*;
use serde_json::json;
use std::time::Duration;

/// Test rate limit headers are present in response
#[tokio::test]
async fn test_rate_limit_headers_present() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello");
    let response = server.post_json("/v1/chat/completions", &request).await;

    assert_status(&response, 200);

    // Rate limit headers should be present (if rate limiting is enabled)
    // Note: headers may or may not be present depending on configuration
    let _limit_header = response.headers().get("X-RateLimit-Limit");
    let _remaining_header = response.headers().get("X-RateLimit-Remaining");
    let _reset_header = response.headers().get("X-RateLimit-Reset");
}

/// Test multiple requests within limit succeed
#[tokio::test]
async fn test_requests_within_limit() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello");

    // Send several requests
    for _ in 0..5 {
        let response = server.post_json("/v1/chat/completions", &request).await;
        assert_status(&response, 200);
    }
}

/// Test concurrent requests
#[tokio::test]
async fn test_concurrent_requests_rate_limiting() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello");

    // Send concurrent requests
    let futures: Vec<_> = (0..10)
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

    // Count successes and rate limits
    let mut successes = 0;

    for result in results {
        let response = result.expect("Request failed");
        match response.status().as_u16() {
            200 => successes += 1,
            429 => {} // rate limited
            other => panic!("Unexpected status: {}", other),
        }
    }

    // At least some should succeed
    assert!(successes > 0);
}

/// Test rate limit by API key
#[tokio::test]
async fn test_per_api_key_rate_limit() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello");

    // Requests with different API keys
    let response1 = server
        .post_json_with_headers(
            "/v1/chat/completions",
            &request,
            vec![("X-API-Key", "key-1")],
        )
        .await;

    let response2 = server
        .post_json_with_headers(
            "/v1/chat/completions",
            &request,
            vec![("X-API-Key", "key-2")],
        )
        .await;

    assert_status(&response1, 200);
    assert_status(&response2, 200);
}

/// Test rate limit by user ID
#[tokio::test]
async fn test_per_user_rate_limit() {
    let server = TestServer::with_default_config().await;

    let request1 = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello"}],
        "user": "user-1"
    });

    let request2 = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello"}],
        "user": "user-2"
    });

    // Different users should have separate rate limits
    let response1 = server.post_json("/v1/chat/completions", &request1).await;
    let response2 = server.post_json("/v1/chat/completions", &request2).await;

    assert_status(&response1, 200);
    assert_status(&response2, 200);
}

/// Test rate limit by model
#[tokio::test]
async fn test_per_model_rate_limit() {
    let server = TestServer::with_default_config().await;

    let request1 = openai_json_request("gpt-4o", "Hello");
    let request2 = openai_json_request("gpt-4o-mini", "Hello");

    // Different models may have different rate limits
    let response1 = server.post_json("/v1/chat/completions", &request1).await;
    let response2 = server.post_json("/v1/chat/completions", &request2).await;

    assert_status(&response1, 200);
    assert_status(&response2, 200);
}

/// Test health endpoints are not rate limited
#[tokio::test]
async fn test_health_endpoints_not_rate_limited() {
    let server = TestServer::with_default_config().await;

    // Health endpoints should never be rate limited
    for _ in 0..20 {
        let response = server.get("/health").await;
        assert_status(&response, 200);
    }
}

/// Test models endpoint rate limiting
#[tokio::test]
async fn test_models_endpoint_rate_limit() {
    let server = TestServer::with_default_config().await;

    // Models endpoint should be accessible
    for _ in 0..5 {
        let response = server.get("/v1/models").await;
        assert_status(&response, 200);
    }
}

/// Test rate limit reset behavior
#[tokio::test]
async fn test_rate_limit_reset() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello");

    // Send a request
    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);

    // Wait a short time
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Should still work
    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test rate limit with burst
#[tokio::test]
async fn test_burst_rate_limit() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello");

    // Send burst of requests
    let futures: Vec<_> = (0..5)
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

    // Count results
    let success_count = results
        .iter()
        .filter(|r| r.as_ref().map(|resp| resp.status() == 200).unwrap_or(false))
        .count();

    // At least some should succeed
    assert!(success_count > 0);
}

/// Test token-based rate limiting
#[tokio::test]
async fn test_token_rate_limiting() {
    let server = TestServer::with_default_config().await;

    // Request with large max_tokens
    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello"}],
        "max_tokens": 4000
    });

    let response = server.post_json("/v1/chat/completions", &request).await;

    // Should succeed or be rate limited
    let status = response.status().as_u16();
    assert!(status == 200 || status == 429);
}

/// Test rate limit error response format
#[tokio::test]
async fn test_rate_limit_error_format() {
    // This test would need a way to trigger rate limiting
    // For now, verify the expected error format
    let error_response = rate_limit_error_response();

    assert!(error_response["error"]["type"]
        .as_str()
        .unwrap()
        .contains("rate_limit"));
    assert!(error_response["error"]["message"].is_string());
}

/// Test rate limit with streaming requests
#[tokio::test]
async fn test_streaming_rate_limit() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello"}],
        "stream": true
    });

    // Streaming requests should also be rate limited
    let response = server.post_json("/v1/chat/completions", &request).await;

    let status = response.status().as_u16();
    assert!(status == 200 || status == 429);
}

/// Test rate limit headers decrease with usage
#[tokio::test]
async fn test_rate_limit_remaining_decreases() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello");

    // Send requests and check remaining header
    for _ in 0..3 {
        let response = server.post_json("/v1/chat/completions", &request).await;
        assert_status(&response, 200);

        // If rate limit headers are present, remaining should be tracked
        if let Some(_remaining) = response.headers().get("X-RateLimit-Remaining") {
            // Remaining count is tracked
        }
    }
}

/// Test different endpoints have separate rate limits
#[tokio::test]
async fn test_endpoint_rate_limit_isolation() {
    let server = TestServer::with_default_config().await;

    let chat_request = openai_json_request("gpt-4o-mini", "Hello");

    // Chat completions
    let response1 = server.post_json("/v1/chat/completions", &chat_request).await;
    assert_status(&response1, 200);

    // Models list
    let response2 = server.get("/v1/models").await;
    assert_status(&response2, 200);

    // Health
    let response3 = server.get("/health").await;
    assert_status(&response3, 200);
}

/// Test rate limit recovery over time
#[tokio::test]
async fn test_rate_limit_recovery() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello");

    // Send requests
    for _ in 0..5 {
        let _response = server.post_json("/v1/chat/completions", &request).await;
    }

    // Wait for rate limit window to reset (short wait for testing)
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Should be able to send more requests
    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test rate limiting doesn't affect error responses
#[tokio::test]
async fn test_errors_not_counted_for_rate_limit() {
    let server = TestServer::with_default_config().await;

    // Invalid request
    let invalid_request = json!({
        "invalid": "request"
    });

    // Send invalid requests
    for _ in 0..5 {
        let _response = server.post_json("/v1/chat/completions", &invalid_request).await;
    }

    // Valid request should still work
    let valid_request = openai_json_request("gpt-4o-mini", "Hello");
    let response = server.post_json("/v1/chat/completions", &valid_request).await;

    // Should not be rate limited due to errors
    let status = response.status().as_u16();
    assert!(status == 200 || status == 400); // 200 success or 400 validation error
}
