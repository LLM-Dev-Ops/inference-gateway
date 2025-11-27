//! # Gateway Providers
//!
//! LLM Provider implementations for the Inference Gateway.
//!
//! This crate provides implementations for various LLM providers:
//! - OpenAI (GPT-4, GPT-3.5-turbo, etc.)
//! - Anthropic (Claude)
//! - Google AI (Gemini)
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

// Re-export main types
pub use registry::{ProviderRegistry, ProviderEntry};

#[cfg(feature = "openai")]
pub use openai::OpenAIProvider;

#[cfg(feature = "anthropic")]
pub use anthropic::AnthropicProvider;
