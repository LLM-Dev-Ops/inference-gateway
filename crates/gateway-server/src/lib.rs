//! # Gateway Server
//!
//! HTTP server implementation for the LLM Inference Gateway.
//!
//! This crate provides:
//! - Axum-based HTTP server
//! - OpenAI-compatible API endpoints
//! - Request/response handling
//! - Middleware for auth, rate limiting, etc.
//! - Enterprise health check system
//! - Graceful shutdown handling
//! - JWT/OIDC authentication

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod auth;
pub mod error;
pub mod extractors;
pub mod handlers;
pub mod health;
pub mod middleware;
pub mod routes;
pub mod server;
pub mod shutdown;
pub mod state;

// Re-export main types
pub use auth::{
    ApiKeyConfig, ApiKeyMetadata, AuthConfig, AuthConfigBuilder, AuthError, AuthMethod,
    AuthState, AuthenticatedEntity, JwtConfig, JwtMode, auth_middleware,
};
pub use error::ApiError;
pub use health::{
    ComponentHealth, HealthChecker, HealthConfig, HealthResponse, HealthStatus,
    LivenessResponse, ProviderHealthResult, ReadinessResponse, StartupResponse,
};
pub use server::{Server, ServerConfig};
pub use shutdown::{
    GracefulServer, RequestGuard, ShutdownConfig, ShutdownCoordinator, ShutdownEvent,
    ShutdownPhase, ShutdownStats,
};
pub use state::AppState;
