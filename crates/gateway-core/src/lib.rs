//! # Gateway Core
//!
//! Core types, traits, and error handling for the LLM Inference Gateway.
//!
//! This crate provides the foundational types used throughout the gateway:
//! - Request and response types
//! - Provider traits and abstractions
//! - Error types and handling
//! - Validated domain types (newtypes)

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod provider;
pub mod request;
pub mod response;
pub mod streaming;
pub mod types;

// Re-export commonly used types
pub use error::{GatewayError, GatewayResult};
pub use provider::{
    HealthStatus, LLMProvider, ModelInfo, ProviderCapabilities, ProviderType,
};
pub use request::{
    ChatMessage, ContentPart, FunctionCall, GatewayRequest, MessageContent, MessageRole,
    RequestMetadata, ToolCall, ToolChoice,
};
pub use response::{Choice, FinishReason, GatewayResponse, ModelObject, ModelsResponse, Usage};
pub use streaming::{ChatChunk, ChunkChoice, ChunkDelta};
pub use types::{
    ApiKey, MaxTokens, ModelId, ProviderId, RequestId, Temperature, TenantId, TopK, TopP,
};
