//! Integration tests for the LLM Inference Gateway
//!
//! This crate provides comprehensive integration tests covering:
//! - API endpoint testing
//! - Provider integration
//! - Caching behavior
//! - Rate limiting
//! - End-to-end request flows

pub mod fixtures;
pub mod helpers;
pub mod mock_providers;

// Re-export commonly used items
pub use fixtures::*;
pub use helpers::*;
pub use mock_providers::*;

#[cfg(test)]
mod api_tests;
#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod e2e_tests;
#[cfg(test)]
mod provider_tests;
#[cfg(test)]
mod rate_limit_tests;
#[cfg(test)]
mod routing_tests;
