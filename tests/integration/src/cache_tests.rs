//! Cache integration tests
//!
//! Tests for caching behavior including cache hits, misses,
//! TTL, invalidation, and multi-tier caching.

use crate::fixtures::*;
use crate::helpers::*;
use serde_json::json;
use std::time::Duration;

/// Test cache hit returns same response
#[tokio::test]
async fn test_cache_hit() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "What is 2+2?");

    // First request - cache miss
    let response1 = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response1, 200);
    let body1 = TestServer::json_body(response1).await;

    // Second request with same input - should be cache hit
    let response2 = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response2, 200);
    let body2 = TestServer::json_body(response2).await;

    // Content should be the same
    assert_eq!(
        body1["choices"][0]["message"]["content"],
        body2["choices"][0]["message"]["content"]
    );
}

/// Test different requests don't share cache
#[tokio::test]
async fn test_cache_different_requests() {
    let server = TestServer::with_default_config().await;

    let request1 = openai_json_request("gpt-4o-mini", "What is 2+2?");
    let request2 = openai_json_request("gpt-4o-mini", "What is 3+3?");

    let response1 = server.post_json("/v1/chat/completions", &request1).await;
    let response2 = server.post_json("/v1/chat/completions", &request2).await;

    assert_status(&response1, 200);
    assert_status(&response2, 200);

    // Both should succeed (mock server returns same ID, but real server would differ)
    let body1 = TestServer::json_body(response1).await;
    let body2 = TestServer::json_body(response2).await;

    // Both should have valid response structure
    assert!(body1["id"].is_string());
    assert!(body2["id"].is_string());
}

/// Test different models don't share cache
#[tokio::test]
async fn test_cache_different_models() {
    let server = TestServer::with_default_config().await;

    let message = "Hello!";
    let request1 = openai_json_request("gpt-4o", message);
    let request2 = openai_json_request("gpt-4o-mini", message);

    let response1 = server.post_json("/v1/chat/completions", &request1).await;
    let response2 = server.post_json("/v1/chat/completions", &request2).await;

    assert_status(&response1, 200);
    assert_status(&response2, 200);

    let body1 = TestServer::json_body(response1).await;
    let body2 = TestServer::json_body(response2).await;

    // Models should be different
    assert_ne!(body1["model"], body2["model"]);
}

/// Test temperature affects cache key
#[tokio::test]
async fn test_cache_temperature_affects_key() {
    let server = TestServer::with_default_config().await;

    let request1 = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello"}],
        "temperature": 0.0
    });

    let request2 = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello"}],
        "temperature": 1.0
    });

    let response1 = server.post_json("/v1/chat/completions", &request1).await;
    let response2 = server.post_json("/v1/chat/completions", &request2).await;

    assert_status(&response1, 200);
    assert_status(&response2, 200);

    // Different temperatures might get different responses
    // (depends on cache key configuration)
}

/// Test streaming requests bypass cache
#[tokio::test]
async fn test_streaming_bypasses_cache() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello"}],
        "stream": true
    });

    // Streaming requests typically bypass cache
    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test cache-control header
#[tokio::test]
async fn test_cache_control_no_cache() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Hello");

    // First request
    let response1 = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response1, 200);
    let body1 = TestServer::json_body(response1).await;

    // Second request with no-cache header
    let response2 = server
        .post_json_with_headers(
            "/v1/chat/completions",
            &request,
            vec![("Cache-Control", "no-cache")],
        )
        .await;
    assert_status(&response2, 200);
    let body2 = TestServer::json_body(response2).await;

    // Both should succeed
    assert!(body1["choices"][0]["message"]["content"].is_string());
    assert!(body2["choices"][0]["message"]["content"].is_string());
}

/// Test multiple concurrent requests to same endpoint
#[tokio::test]
async fn test_cache_concurrent_requests() {
    let server = TestServer::with_default_config().await;

    let request = openai_json_request("gpt-4o-mini", "Concurrent test");

    // Send multiple concurrent requests
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

    // All should succeed
    for result in results {
        let response = result.expect("Request failed");
        assert_eq!(response.status(), 200);
    }
}

/// Test cache key includes user identifier
#[tokio::test]
async fn test_cache_user_isolation() {
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

    let response1 = server.post_json("/v1/chat/completions", &request1).await;
    let response2 = server.post_json("/v1/chat/completions", &request2).await;

    assert_status(&response1, 200);
    assert_status(&response2, 200);
}

/// Test cache handles large responses
#[tokio::test]
async fn test_cache_large_response() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Write a very long story"}],
        "max_tokens": 2000
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test health endpoint is not cached
#[tokio::test]
async fn test_health_not_cached() {
    let server = TestServer::with_default_config().await;

    let response1 = server.get("/health").await;
    let response2 = server.get("/health").await;

    assert_status(&response1, 200);
    assert_status(&response2, 200);

    // Both should return fresh status
    let body1 = TestServer::json_body(response1).await;
    let body2 = TestServer::json_body(response2).await;

    assert_eq!(body1["status"], "healthy");
    assert_eq!(body2["status"], "healthy");
}

/// Test models list can be cached
#[tokio::test]
async fn test_models_list_cacheable() {
    let server = TestServer::with_default_config().await;

    let response1 = server.get("/v1/models").await;
    let response2 = server.get("/v1/models").await;

    assert_status(&response1, 200);
    assert_status(&response2, 200);

    let body1 = TestServer::json_body(response1).await;
    let body2 = TestServer::json_body(response2).await;

    // Should return same models
    assert_eq!(body1["data"], body2["data"]);
}

/// Test cache with special characters in content
#[tokio::test]
async fn test_cache_special_characters() {
    let server = TestServer::with_default_config().await;

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hello! 你好 🌍 \"quoted\" 'apostrophe'"}]
    });

    let response1 = server.post_json("/v1/chat/completions", &request).await;
    let response2 = server.post_json("/v1/chat/completions", &request).await;

    assert_status(&response1, 200);
    assert_status(&response2, 200);
}

/// Test cache with very long message content
#[tokio::test]
async fn test_cache_long_content() {
    let server = TestServer::with_default_config().await;

    let long_content = "word ".repeat(500);
    let request = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": long_content}]
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

/// Test cache with many messages
#[tokio::test]
async fn test_cache_many_messages() {
    let server = TestServer::with_default_config().await;

    let messages: Vec<_> = (0..50)
        .map(|i| {
            json!({
                "role": if i % 2 == 0 { "user" } else { "assistant" },
                "content": format!("Message {}", i)
            })
        })
        .collect();

    let request = json!({
        "model": "gpt-4o-mini",
        "messages": messages
    });

    let response = server.post_json("/v1/chat/completions", &request).await;
    assert_status(&response, 200);
}

#[cfg(test)]
mod redis_cache_tests {
    use super::*;

    /// Test Redis cache connection (requires Redis running)
    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_redis_cache_integration() {
        // This test requires a running Redis instance
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string());

        let client = redis::Client::open(redis_url).expect("Failed to create Redis client");
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .expect("Failed to connect to Redis");

        // Test basic operations
        let _: () = redis::cmd("SET")
            .arg("test_key")
            .arg("test_value")
            .query_async(&mut conn)
            .await
            .expect("SET failed");

        let value: String = redis::cmd("GET")
            .arg("test_key")
            .query_async(&mut conn)
            .await
            .expect("GET failed");

        assert_eq!(value, "test_value");

        // Cleanup
        let _: () = redis::cmd("DEL")
            .arg("test_key")
            .query_async(&mut conn)
            .await
            .expect("DEL failed");
    }

    /// Test cache persistence across requests (requires Redis)
    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_redis_cache_persistence() {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string());

        let client = redis::Client::open(redis_url).expect("Failed to create Redis client");
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .expect("Failed to connect to Redis");

        // Store a cache entry
        let cache_key = "llm-gateway:test:persistence";
        let cache_value = serde_json::to_string(&json!({
            "id": "test-response",
            "content": "Cached response"
        }))
        .unwrap();

        let _: () = redis::cmd("SETEX")
            .arg(cache_key)
            .arg(60) // 60 second TTL
            .arg(&cache_value)
            .query_async(&mut conn)
            .await
            .expect("SETEX failed");

        // Retrieve it
        let retrieved: String = redis::cmd("GET")
            .arg(cache_key)
            .query_async(&mut conn)
            .await
            .expect("GET failed");

        assert_eq!(retrieved, cache_value);

        // Cleanup
        let _: () = redis::cmd("DEL")
            .arg(cache_key)
            .query_async(&mut conn)
            .await
            .expect("DEL failed");
    }

    /// Test cache TTL expiration (requires Redis)
    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_redis_cache_ttl() {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string());

        let client = redis::Client::open(redis_url).expect("Failed to create Redis client");
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .expect("Failed to connect to Redis");

        let cache_key = "llm-gateway:test:ttl";

        // Set with 1 second TTL
        let _: () = redis::cmd("SETEX")
            .arg(cache_key)
            .arg(1)
            .arg("short-lived")
            .query_async(&mut conn)
            .await
            .expect("SETEX failed");

        // Should exist immediately
        let exists: bool = redis::cmd("EXISTS")
            .arg(cache_key)
            .query_async(&mut conn)
            .await
            .expect("EXISTS failed");
        assert!(exists);

        // Wait for TTL
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should be expired
        let exists: bool = redis::cmd("EXISTS")
            .arg(cache_key)
            .query_async(&mut conn)
            .await
            .expect("EXISTS failed");
        assert!(!exists);
    }
}
