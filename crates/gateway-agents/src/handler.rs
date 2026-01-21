//! HTTP handlers for the Inference Routing Agent.
//!
//! This module provides Axum-based HTTP handlers compatible with:
//! - Google Cloud Functions (2nd gen)
//! - Cloud Run
//! - AWS Lambda (via axum-lambda adapter)
//! - Any standard HTTP server
//!
//! ## Endpoints
//!
//! - `POST /agents/route` - Route an inference request
//! - `GET /agents/inspect` - Inspect routing configuration
//! - `GET /agents/health` - Health check endpoint
//! - `GET /agents/status` - Get agent status

use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::inference_routing::{
    InferenceRoutingAgent, InferenceRoutingInput, InferenceRoutingOutput, RoutingInspection,
    AGENT_ID, AGENT_VERSION,
};
use crate::types::{AgentHealth, AgentStatus};
use agentics_contracts::DecisionEvent;

/// Shared state for handlers
pub type AgentState = Arc<InferenceRoutingAgent>;

/// Response for the route endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResponse {
    /// Routing output with selected provider and model
    pub output: InferenceRoutingOutput,
    /// Decision ID for audit trail (matches `DecisionEvent.execution_ref`)
    pub decision_id: String,
}

/// Response for the route endpoint with full decision event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteWithEventResponse {
    /// Routing output with selected provider and model
    pub output: InferenceRoutingOutput,
    /// Full decision event for audit purposes
    pub decision_event: DecisionEvent,
}

/// Error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    /// Error type
    #[serde(rename = "type")]
    pub error_type: String,
    /// Error message
    pub message: String,
    /// Error code (for programmatic handling)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl ApiError {
    /// Create a new API error
    pub fn new(error_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error_type: error_type.into(),
            message: message.into(),
            code: None,
        }
    }

    /// Set the error code
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Create a validation error
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new("validation_error", message)
    }

    /// Create an internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("internal_error", message)
    }

    /// Create a routing error
    pub fn routing(message: impl Into<String>) -> Self {
        Self::new("routing_error", message)
    }
}

/// Wrapper for API error responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    /// Error details
    pub error: ApiError,
}

impl IntoResponse for ApiErrorResponse {
    fn into_response(self) -> axum::response::Response {
        let status = match self.error.error_type.as_str() {
            "validation_error" => StatusCode::BAD_REQUEST,
            "not_found_error" => StatusCode::NOT_FOUND,
            "routing_error" => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(self)).into_response()
    }
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Health status
    pub status: String,
    /// Agent ID
    pub agent_id: String,
    /// Agent version
    pub version: String,
}

/// POST /agents/route - Route an inference request
///
/// This endpoint routes an inference request to the optimal provider.
/// It returns the routing decision along with a decision ID that can
/// be used for audit trail purposes.
///
/// ## Request Body
///
/// The request body should be an `InferenceRoutingInput` containing:
/// - `request`: The `GatewayRequest` to route
/// - `tenant_id`: Optional tenant identifier
/// - `hints`: Optional routing hints
///
/// ## Response
///
/// Returns a `RouteResponse` with:
/// - `output`: The routing output (provider, model, headers)
/// - `decision_id`: Unique identifier for the decision event
pub async fn handle_route(
    State(agent): State<AgentState>,
    Json(input): Json<InferenceRoutingInput>,
) -> Result<Json<RouteResponse>, ApiErrorResponse> {
    let (output, event) = agent
        .route(input)
        .await
        .map_err(|e| ApiErrorResponse {
            error: ApiError::routing(e.to_string()),
        })?;

    Ok(Json(RouteResponse {
        output,
        decision_id: event.execution_ref,
    }))
}

/// POST /agents/route/audit - Route an inference request with full decision event
///
/// This endpoint routes an inference request and returns the full `DecisionEvent`
/// for audit compliance. This is useful when you need the complete audit record
/// for external storage or compliance systems.
///
/// ## Request Body
///
/// Same as `/agents/route`.
///
/// ## Response
///
/// Returns a `RouteWithEventResponse` with:
/// - `output`: The routing output
/// - `decision_event`: The complete `DecisionEvent` for audit
pub async fn handle_route_with_event(
    State(agent): State<AgentState>,
    Json(input): Json<InferenceRoutingInput>,
) -> Result<Json<RouteWithEventResponse>, ApiErrorResponse> {
    let (output, decision_event) = agent
        .route_with_decision_event(input)
        .await
        .map_err(|e| ApiErrorResponse {
            error: ApiError::routing(e.to_string()),
        })?;

    Ok(Json(RouteWithEventResponse {
        output,
        decision_event,
    }))
}

/// GET /agents/inspect - Inspect routing configuration
///
/// Returns the current state of the routing agent including:
/// - Agent metadata and version
/// - Registered providers
/// - Active rules
/// - Configuration summary
pub async fn handle_inspect(
    State(agent): State<AgentState>,
) -> Json<RoutingInspection> {
    Json(agent.inspect())
}

/// GET /agents/status - Get agent status
///
/// Returns the current operational status of the agent including:
/// - Health status
/// - Request counts and error rates
/// - Average latency
/// - Uptime information
pub async fn handle_status(
    State(agent): State<AgentState>,
) -> Json<AgentStatus> {
    Json(agent.status())
}

/// GET /agents/health - Health check endpoint
///
/// Simple health check for load balancers and orchestration systems.
/// Returns 200 OK if the agent is healthy, or an appropriate error status.
pub async fn handle_health(
    State(agent): State<AgentState>,
) -> Result<Json<HealthResponse>, ApiErrorResponse> {
    let status = agent.status();

    if status.health == AgentHealth::Unhealthy {
        return Err(ApiErrorResponse {
            error: ApiError::internal("Agent is unhealthy"),
        });
    }

    Ok(Json(HealthResponse {
        status: status.health.to_string(),
        agent_id: AGENT_ID.to_string(),
        version: AGENT_VERSION.to_string(),
    }))
}

/// Create an Axum router with all agent endpoints.
///
/// ## Example
///
/// ```ignore
/// use gateway_agents::handler::{create_router, AgentState};
/// use gateway_agents::InferenceRoutingAgent;
/// use std::sync::Arc;
///
/// let agent = Arc::new(InferenceRoutingAgent::builder().build());
/// let app = create_router(agent);
/// // Run with: axum::serve(listener, app).await?;
/// ```
pub fn create_router(agent: AgentState) -> axum::Router {
    use axum::routing::{get, post};

    axum::Router::new()
        .route("/agents/route", post(handle_route))
        .route("/agents/route/audit", post(handle_route_with_event))
        .route("/agents/inspect", get(handle_inspect))
        .route("/agents/status", get(handle_status))
        .route("/agents/health", get(handle_health))
        .with_state(agent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference_routing::InferenceRoutingAgent;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use gateway_core::{ChatMessage, GatewayRequest, HealthStatus, LLMProvider, ModelInfo, ProviderCapabilities, ProviderType};
    use futures::stream::BoxStream;
    use tower::ServiceExt;

    struct MockProvider {
        id: String,
        models: Vec<ModelInfo>,
    }

    impl MockProvider {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_string(),
                models: vec![ModelInfo::new("test-model")],
            }
        }
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockProvider {
        fn id(&self) -> &str {
            &self.id
        }

        fn provider_type(&self) -> ProviderType {
            ProviderType::Custom
        }

        async fn chat_completion(&self, _: &GatewayRequest) -> Result<gateway_core::GatewayResponse, gateway_core::GatewayError> {
            unimplemented!()
        }

        async fn chat_completion_stream(
            &self,
            _: &GatewayRequest,
        ) -> Result<BoxStream<'static, Result<gateway_core::ChatChunk, gateway_core::GatewayError>>, gateway_core::GatewayError> {
            unimplemented!()
        }

        async fn health_check(&self) -> HealthStatus {
            HealthStatus::Healthy
        }

        fn capabilities(&self) -> &ProviderCapabilities {
            static CAPS: ProviderCapabilities = ProviderCapabilities {
                chat: true,
                streaming: true,
                function_calling: false,
                vision: false,
                embeddings: false,
                json_mode: false,
                seed: false,
                logprobs: false,
                max_context_length: None,
                max_output_tokens: None,
                parallel_tool_calls: false,
            };
            &CAPS
        }

        fn models(&self) -> &[ModelInfo] {
            &self.models
        }

        fn base_url(&self) -> &str {
            "http://localhost"
        }
    }

    fn create_test_agent() -> AgentState {
        let agent = InferenceRoutingAgent::builder()
            .id("test-agent")
            .build();

        let provider = Arc::new(MockProvider::new("test-provider"));
        agent.register_provider(provider, 100, 100);
        agent.update_health("test-provider", HealthStatus::Healthy);

        Arc::new(agent)
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let agent = create_test_agent();
        let app = create_router(agent);

        let request = Request::builder()
            .uri("/agents/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_inspect_endpoint() {
        let agent = create_test_agent();
        let app = create_router(agent);

        let request = Request::builder()
            .uri("/agents/inspect")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_status_endpoint() {
        let agent = create_test_agent();
        let app = create_router(agent);

        let request = Request::builder()
            .uri("/agents/status")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_route_endpoint() {
        let agent = create_test_agent();
        let app = create_router(agent);

        let gateway_request = GatewayRequest::builder()
            .model("test-model")
            .message(ChatMessage::user("Hello"))
            .build()
            .unwrap();

        let input = InferenceRoutingInput {
            request: gateway_request,
            tenant_id: None,
            hints: None,
        };

        let body = serde_json::to_string(&input).unwrap();

        let request = Request::builder()
            .method("POST")
            .uri("/agents/route")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
