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

pub mod load_balancer;
pub mod router;
pub mod rules;
pub mod selector;
pub mod strategy;

// Re-export main types
pub use load_balancer::{LoadBalancer, LoadBalancerConfig};
pub use router::{RouteDecision, Router, RouterConfig};
pub use rules::{RoutingRule, RuleAction, RuleMatcher};
pub use selector::{ProviderCandidate, ProviderSelector, SelectionCriteria};
pub use strategy::{LoadBalancingStrategy, StrategyFactory};
