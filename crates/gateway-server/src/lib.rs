//! # Gateway Server
//!
//! HTTP server implementation for the LLM Inference Gateway.
//!
//! This crate provides:
//! - Axum-based HTTP server
//! - OpenAI-compatible API endpoints
//! - Request/response handling
//! - Middleware for auth, rate limiting, etc.
//! - Health check endpoints

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod handlers;
pub mod middleware;
pub mod state;
pub mod server;
pub mod routes;
pub mod error;
pub mod extractors;

// Re-export main types
pub use server::{Server, ServerConfig};
pub use state::AppState;
pub use error::ApiError;
