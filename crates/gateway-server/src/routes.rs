//! Route definitions for the gateway API.

use axum::{
    routing::{get, post},
    Router,
};

use crate::{handlers, middleware, state::AppState};

/// Create the main API router
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health endpoints
        .route("/health", get(handlers::health_check))
        .route("/healthz", get(handlers::health_check))
        .route("/ready", get(handlers::readiness_check))
        .route("/readyz", get(handlers::readiness_check))
        .route("/live", get(handlers::liveness_check))
        .route("/livez", get(handlers::liveness_check))
        // Metrics endpoint
        .route("/metrics", get(handlers::metrics_endpoint))
        // OpenAI-compatible endpoints
        .nest("/v1", openai_routes())
        // Admin endpoints
        .nest("/admin", admin_routes())
        // Agent endpoints
        .nest("/", agent_routes())
        // Apply middleware
        .layer(axum::middleware::from_fn(middleware::request_id_middleware))
        .layer(axum::middleware::from_fn(middleware::response_time_middleware))
        .layer(axum::middleware::from_fn(middleware::logging_middleware))
        .layer(axum::middleware::from_fn(middleware::security_headers_middleware))
        .layer(middleware::cors_layer())
        // Add state
        .with_state(state)
}

/// OpenAI-compatible API routes
fn openai_routes() -> Router<AppState> {
    Router::new()
        // Chat completions
        .route("/chat/completions", post(handlers::chat_completion))
        // Models
        .route("/models", get(handlers::list_models))
        .route("/models/:model_id", get(handlers::get_model))
}

/// Admin/management routes
fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/providers", get(handlers::list_providers))
        .route("/stats", get(handlers::gateway_stats))
}

/// Agent routes for the Inference Routing Agent
///
/// Provides endpoints for:
/// - `POST /agents/route` - Route an inference request
/// - `GET /agents/inspect` - Inspect agent configuration
/// - `GET /agents/status` - Get agent operational status
/// - `GET /agents` - List available agents
/// - `GET /agents/health` - Agent health check
pub fn agent_routes() -> Router<AppState> {
    Router::new()
        .route("/agents/route", post(handlers::agent_route))
        .route("/agents/inspect", get(handlers::agent_inspect))
        .route("/agents/status", get(handlers::agent_status))
        .route("/agents", get(handlers::list_agents))
        .route("/agents/health", get(handlers::agent_health))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use gateway_config::GatewayConfig;
    use tower::ServiceExt;

    fn create_test_state() -> AppState {
        AppState::builder()
            .config(GatewayConfig::default())
            .build()
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = create_router(create_test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_metrics_endpoint() {
        let app = create_router(create_test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_models_endpoint() {
        let app = create_router(create_test_state());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
