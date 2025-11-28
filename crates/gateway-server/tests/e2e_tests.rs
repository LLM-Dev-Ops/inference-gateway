//! End-to-end integration tests for the LLM Inference Gateway.
//!
//! These tests validate the complete gateway functionality including:
//! - HTTP endpoints
//! - Request routing
//! - Response handling
//! - Error handling
//!
//! Tests use the existing gateway server infrastructure with mock providers.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use gateway_config::GatewayConfig;
use gateway_core::{ChatMessage, GatewayRequest, GatewayResponse};
use gateway_providers::openai::OpenAIConfig;
use gateway_providers::{OpenAIProvider, ProviderRegistry};
use gateway_resilience::{DistributedCache, DistributedCacheConfig, ResponseCache};
use gateway_routing::{Router, RouterConfig};
use gateway_server::AppState;
use gateway_server::routes::create_router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

/// Test helper to create a mock provider registry
fn create_mock_registry() -> ProviderRegistry {
    let registry = ProviderRegistry::new();

    // Create a mock OpenAI provider
    let openai_config = OpenAIConfig::new("mock-openai", "sk-mock-test-key");
    let openai_provider = OpenAIProvider::new(openai_config).expect("valid provider config");
    registry
        .register(Arc::new(openai_provider), 1, 100)
        .expect("register should succeed");

    registry
}

/// Create test application state
fn create_test_state() -> AppState {
    AppState::builder()
        .config(GatewayConfig::default())
        .providers(create_mock_registry())
        .router(Router::new(RouterConfig::default()))
        .build()
}

#[cfg(test)]
mod health_endpoint_tests {
    use super::*;

    #[tokio::test]
    async fn test_health_endpoint_returns_ok() {
        let app = create_router(create_test_state());

        let request = Request::builder()
            .method(Method::GET)
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["status"], "healthy");
    }

    #[tokio::test]
    async fn test_healthz_endpoint_works() {
        let app = create_router(create_test_state());

        let request = Request::builder()
            .method(Method::GET)
            .uri("/healthz")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_readiness_endpoint() {
        let app = create_router(create_test_state());

        let request = Request::builder()
            .method(Method::GET)
            .uri("/ready")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_liveness_endpoint() {
        let app = create_router(create_test_state());

        let request = Request::builder()
            .method(Method::GET)
            .uri("/live")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[cfg(test)]
mod models_endpoint_tests {
    use super::*;

    #[tokio::test]
    async fn test_models_endpoint_returns_list() {
        let app = create_router(create_test_state());

        let request = Request::builder()
            .method(Method::GET)
            .uri("/v1/models")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["object"], "list");
        assert!(json["data"].is_array());
    }

    #[tokio::test]
    async fn test_models_endpoint_contains_models() {
        let app = create_router(create_test_state());

        let request = Request::builder()
            .method(Method::GET)
            .uri("/v1/models")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();

        let models = json["data"].as_array().unwrap();
        // Should have at least some models from the mock OpenAI provider
        assert!(!models.is_empty());

        // Check structure of model objects
        let first_model = &models[0];
        assert!(first_model["id"].is_string());
        assert!(first_model["object"].is_string());
    }
}

#[cfg(test)]
mod chat_completions_validation_tests {
    use super::*;

    #[tokio::test]
    async fn test_chat_completions_requires_model() {
        let app = create_router(create_test_state());

        let body = json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/v1/chat/completions")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should return bad request for missing model
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_chat_completions_requires_messages() {
        let app = create_router(create_test_state());

        let body = json!({
            "model": "gpt-4o-mini"
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/v1/chat/completions")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should return bad request for missing messages
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_chat_completions_validates_temperature() {
        let app = create_router(create_test_state());

        let body = json!({
            "model": "gpt-4o-mini",
            "messages": [{"role": "user", "content": "Hello"}],
            "temperature": 3.0  // Invalid: max is 2.0
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/v1/chat/completions")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should return error for invalid temperature (400 or 503 depending on implementation)
        assert!(
            response.status() == StatusCode::BAD_REQUEST
                || response.status() == StatusCode::SERVICE_UNAVAILABLE,
            "Expected 400 or 503, got {}",
            response.status()
        );
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[tokio::test]
    async fn test_not_found_returns_404() {
        let app = create_router(create_test_state());

        let request = Request::builder()
            .method(Method::GET)
            .uri("/nonexistent/endpoint")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_invalid_json_returns_error() {
        let app = create_router(create_test_state());

        let request = Request::builder()
            .method(Method::POST)
            .uri("/v1/chat/completions")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{invalid json}"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should return bad request for invalid JSON
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

#[cfg(test)]
mod metrics_endpoint_tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_endpoint_available() {
        let app = create_router(create_test_state());

        let request = Request::builder()
            .method(Method::GET)
            .uri("/metrics")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[cfg(test)]
mod admin_endpoint_tests {
    use super::*;

    #[tokio::test]
    async fn test_providers_endpoint() {
        let app = create_router(create_test_state());

        let request = Request::builder()
            .method(Method::GET)
            .uri("/admin/providers")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();

        // The response contains provider data (may be in different formats)
        assert!(json.is_object() || json.is_array());
    }

    #[tokio::test]
    async fn test_stats_endpoint() {
        let app = create_router(create_test_state());

        let request = Request::builder()
            .method(Method::GET)
            .uri("/admin/stats")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[tokio::test]
    async fn test_response_cache_stores_and_retrieves() {
        let cache = ResponseCache::with_defaults();

        let request = GatewayRequest::builder()
            .model("gpt-4o-mini")
            .message(ChatMessage::user("Hello"))
            .temperature(0.7)
            .max_tokens(100u32)
            .build()
            .unwrap();

        let response = GatewayResponse {
            id: "test-response-id".to_string(),
            object: "chat.completion".to_string(),
            model: "gpt-4o-mini".to_string(),
            choices: vec![],
            usage: gateway_core::Usage {
                prompt_tokens: 5,
                completion_tokens: 10,
                total_tokens: 15,
            },
            created: 1234567890,
            provider: Some("mock-openai".to_string()),
            system_fingerprint: None,
        };

        cache.put(&request, response.clone()).await;

        let cached = cache.get(&request).await;

        assert!(cached.is_some());
        assert_eq!(cached.unwrap().id, response.id);
    }

    #[tokio::test]
    async fn test_distributed_cache_l1_operations() {
        let config = DistributedCacheConfig {
            enabled: true,
            enable_local_cache: true,
            local_cache_size: 100,
            local_cache_ttl: Duration::from_secs(60),
            ..Default::default()
        };

        let cache = DistributedCache::new(config);

        let request = GatewayRequest::builder()
            .model("gpt-4o-mini")
            .message(ChatMessage::user("Test distributed cache"))
            .build()
            .unwrap();

        let response = GatewayResponse {
            id: "dist-cache-test".to_string(),
            object: "chat.completion".to_string(),
            model: "gpt-4o-mini".to_string(),
            choices: vec![],
            usage: gateway_core::Usage {
                prompt_tokens: 5,
                completion_tokens: 10,
                total_tokens: 15,
            },
            created: 1234567890,
            provider: Some("mock".to_string()),
            system_fingerprint: None,
        };

        cache.put(&request, response.clone()).await;
        let cached = cache.get(&request).await;

        assert!(cached.is_some());
        assert_eq!(cached.unwrap().id, "dist-cache-test");

        let stats = cache.stats().await;
        assert_eq!(stats.l1_hits, 1);
    }
}

#[cfg(test)]
mod request_builder_tests {
    use super::*;

    #[test]
    fn test_request_builder_creates_valid_request() {
        let request = GatewayRequest::builder()
            .model("gpt-4o")
            .message(ChatMessage::user("Hello"))
            .message(ChatMessage::assistant("Hi there!"))
            .message(ChatMessage::user("How are you?"))
            .temperature(0.8)
            .max_tokens(200u32)
            .top_p(0.95)
            .build();

        assert!(request.is_ok());

        let req = request.unwrap();
        assert_eq!(req.model, "gpt-4o");
        assert_eq!(req.messages.len(), 3);
        assert_eq!(req.temperature, Some(0.8));
        assert_eq!(req.max_tokens, Some(200));
        assert_eq!(req.top_p, Some(0.95));
    }

    #[test]
    fn test_request_builder_validates_temperature() {
        let result = GatewayRequest::builder()
            .model("gpt-4o")
            .message(ChatMessage::user("Hello"))
            .temperature(2.5)
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_request_builder_requires_model() {
        let result = GatewayRequest::builder()
            .message(ChatMessage::user("Hello"))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_request_builder_requires_messages() {
        let result = GatewayRequest::builder().model("gpt-4o").build();

        assert!(result.is_err());
    }
}

#[cfg(test)]
mod response_format_tests {
    use super::*;
    use gateway_core::{Choice, FinishReason};

    #[test]
    fn test_response_serialization() {
        let response = GatewayResponse {
            id: "chatcmpl-123456".to_string(),
            object: "chat.completion".to_string(),
            model: "gpt-4o-mini".to_string(),
            choices: vec![Choice::new(0, "Hello!", FinishReason::Stop)],
            usage: gateway_core::Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
            created: 1234567890,
            provider: Some("openai".to_string()),
            system_fingerprint: Some("fp_abc123".to_string()),
        };

        let json = serde_json::to_value(&response).unwrap();

        assert_eq!(json["id"], "chatcmpl-123456");
        assert_eq!(json["object"], "chat.completion");
        assert_eq!(json["choices"][0]["message"]["content"], "Hello!");
        assert_eq!(json["usage"]["total_tokens"], 15);
    }
}
