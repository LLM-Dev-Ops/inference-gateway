//! # LLM Inference Gateway SDK
//!
//! A Rust SDK for interacting with the LLM Inference Gateway.
//!
//! ## Features
//!
//! - Async-first design with full `tokio` support
//! - Streaming responses with Server-Sent Events
//! - Automatic retries with exponential backoff
//! - Type-safe request and response handling
//! - Builder pattern for easy configuration
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use gateway_sdk::{Client, ChatRequest};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), gateway_sdk::Error> {
//!     let client = Client::builder()
//!         .base_url("http://localhost:8080")
//!         .api_key("your-api-key")
//!         .build()?;
//!
//!     let response = client
//!         .chat()
//!         .model("gpt-4o")
//!         .user_message("Hello, world!")
//!         .send()
//!         .await?;
//!
//!     println!("Response: {}", response.content());
//!     Ok(())
//! }
//! ```
//!
//! ## Streaming
//!
//! ```rust,no_run
//! use gateway_sdk::{Client, ChatRequest};
//! use futures::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), gateway_sdk::Error> {
//!     let client = Client::builder()
//!         .base_url("http://localhost:8080")
//!         .build()?;
//!
//!     let mut stream = client
//!         .chat()
//!         .model("gpt-4o")
//!         .user_message("Tell me a story")
//!         .stream()
//!         .await?;
//!
//!     while let Some(chunk) = stream.next().await {
//!         match chunk {
//!             Ok(chunk) => print!("{}", chunk.content()),
//!             Err(e) => eprintln!("Error: {}", e),
//!         }
//!     }
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

mod client;
mod config;
mod error;
mod request;
mod response;
mod streaming;

pub use client::{Client, ClientBuilder};
pub use config::ClientConfig;
pub use error::{Error, Result};
pub use request::{ChatRequest, ChatRequestBuilder, Message, MessageRole};
pub use response::{ChatResponse, ChatChoice, Usage};
pub use streaming::{ChatStream, StreamChunk, StreamResult};

// Re-export core types for convenience
pub use gateway_core::{
    ChatMessage, FinishReason, GatewayRequest, GatewayResponse,
    ModelObject, ModelsResponse,
};
