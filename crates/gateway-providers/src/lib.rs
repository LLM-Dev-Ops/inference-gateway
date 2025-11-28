//! # Gateway Providers
//!
//! LLM Provider implementations for the Inference Gateway.
//!
//! This crate provides implementations for various LLM providers:
//! - OpenAI (GPT-4, GPT-3.5-turbo, etc.)
//! - Anthropic (Claude)
//! - Google AI (Gemini) / Vertex AI
//! - Azure OpenAI
//! - AWS Bedrock
//! - vLLM (self-hosted)
//! - Ollama (self-hosted)
//! - Together AI

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod registry;

#[cfg(feature = "openai")]
pub mod openai;

#[cfg(feature = "anthropic")]
pub mod anthropic;

#[cfg(feature = "azure")]
pub mod azure;

#[cfg(feature = "google")]
pub mod google;

#[cfg(feature = "bedrock")]
pub mod bedrock;

// Re-export main types
pub use registry::{ProviderEntry, ProviderRegistry};

#[cfg(feature = "openai")]
pub use openai::OpenAIProvider;

#[cfg(feature = "anthropic")]
pub use anthropic::AnthropicProvider;

#[cfg(feature = "azure")]
pub use azure::AzureOpenAIProvider;

#[cfg(feature = "google")]
pub use google::{GoogleApiType, GoogleConfig, GoogleProvider};

#[cfg(feature = "bedrock")]
pub use bedrock::{BedrockConfig, BedrockProvider, ModelFamily as BedrockModelFamily};
