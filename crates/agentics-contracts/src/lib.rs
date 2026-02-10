//! # Agentics Contracts
//!
//! Agent schemas and contracts for the LLM Inference Gateway.
//!
//! This crate defines all agent schemas and contracts according to the gateway constitution:
//! - Decision event schemas for audit logging
//! - Routing agent input/output contracts
//! - Agent metadata and trait definitions
//! - Contract validation and error types
//!
//! ## Architecture
//!
//! The agentics system follows a constitutional design where all agents must:
//! - Emit structured decision events for every routing decision
//! - Apply constraints from policies, providers, and capabilities
//! - Provide confidence scores for decision transparency
//! - Support audit trail via execution references

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod agent;
pub mod decision_event;
pub mod error;
pub mod execution_span;
pub mod routing;

// Re-export commonly used types
pub use agent::{Agent, AgentMetadata, AgentType};
pub use decision_event::{
    Confidence, Constraint, ConstraintEffect, DecisionEvent, DecisionOutput, DecisionType,
};
pub use error::AgentError;
pub use execution_span::{
    ExecutionCollector, ExecutionContext, ExecutionOutput, ExecutionSpan, SpanArtifact, SpanStatus,
    SpanType,
};
pub use routing::{InferenceRoutingInput, InferenceRoutingOutput, RoutingStep};
