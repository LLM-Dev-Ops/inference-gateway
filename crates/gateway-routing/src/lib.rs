//! # Gateway Routing
//!
//! Intelligent routing and load balancing for the LLM Inference Gateway.
//!
//! This crate provides:
//! - Rule-based routing with pattern matching
//! - Multiple load balancing strategies
//! - Model-aware routing
//! - Tenant-based routing
//! - Provider selection with health awareness

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod router;
pub mod rules;
pub mod load_balancer;
pub mod strategy;
pub mod selector;

// Re-export main types
pub use router::{Router, RouterConfig, RouteDecision};
pub use rules::{RoutingRule, RuleMatcher, RuleAction};
pub use load_balancer::{LoadBalancer, LoadBalancerConfig};
pub use strategy::{LoadBalancingStrategy, StrategyFactory};
pub use selector::{ProviderSelector, SelectionCriteria, ProviderCandidate};
