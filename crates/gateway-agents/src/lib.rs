//! # Gateway Agents
//!
//! Intelligent routing agents for the LLM Inference Gateway.
//!
//! This crate provides:
//! - [`InferenceRoutingAgent`]: Intelligent request routing with telemetry
//! - Agent inspection and status monitoring
//! - HTTP handlers for Edge Function deployment (Google Cloud, AWS Lambda, Cloud Run)
//! - Integration with `agentics-contracts` for audit compliance
//!
//! ## Constitutional Guarantees
//!
//! All agents in this crate adhere to the following rules:
//! - **Stateless at runtime**: No persistent state is modified during routing
//! - **One DecisionEvent per invocation**: Exactly one event is emitted per call
//! - **Deterministic**: Same input produces same routing decision
//! - **No inference execution**: Agents do not execute model inference
//! - **No prompt modification**: Agents do not modify prompts or responses
//! - **No orchestration**: Agents do not trigger orchestration workflows
//!
//! ## Example
//!
//! ```ignore
//! use gateway_agents::{InferenceRoutingAgent, InferenceRoutingInput};
//! use gateway_core::{GatewayRequest, ChatMessage};
//!
//! let agent = InferenceRoutingAgent::builder()
//!     .id("my-agent")
//!     .build();
//!
//! let request = GatewayRequest::builder()
//!     .model("gpt-4")
//!     .message(ChatMessage::user("Hello"))
//!     .build()?;
//!
//! let input = InferenceRoutingInput {
//!     request,
//!     tenant_id: Some("tenant-123".to_string()),
//!     hints: None,
//! };
//!
//! let (output, event) = agent.route(input).await?;
//! println!("Routed to: {}", output.provider_id);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod handler;
pub mod inference_routing;
pub mod phase7;
pub mod telemetry;
pub mod types;

// Re-export main types
pub use inference_routing::{
    InferenceRoutingAgent, InferenceRoutingAgentBuilder, InferenceRoutingInput,
    InferenceRoutingOutput, RoutingEvent, RoutingInspection, AGENT_ID, AGENT_VERSION,
};
pub use telemetry::{TelemetryEmitter, TelemetryEvent};
pub use types::{AgentHealth, AgentMetadata, AgentStatus, AgentVersion};

// Re-export handler types for convenience
pub use handler::{
    create_router, handle_health, handle_inspect, handle_route, handle_route_with_event,
    handle_status, AgentState, ApiError, ApiErrorResponse, HealthResponse, RouteResponse,
    RouteWithEventResponse,
};
