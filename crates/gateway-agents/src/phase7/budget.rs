//! Performance budget enforcement for Phase 7 agents.
//!
//! This module provides performance budget tracking and enforcement to ensure
//! agents operate within defined resource constraints. When any budget limit
//! is exceeded, the agent execution is aborted without automatic retry.
//!
//! ## Budget Limits
//!
//! - **Tokens**: Maximum tokens consumed per execution (default: 2500)
//! - **Latency**: Maximum wall-clock time per execution (default: 5000ms)
//! - **API Calls**: Maximum LLM API calls per run (default: 5)
//!
//! ## Example
//!
//! ```ignore
//! use gateway_agents::phase7::budget::{BudgetTracker, BudgetExceededReason};
//! use gateway_agents::phase7::AgentIdentity;
//!
//! let mut budget = BudgetTracker::new();
//!
//! // Record usage during execution
//! budget.record_tokens(500);
//! budget.record_call();
//!
//! // Check if budget is exceeded
//! if let Some(reason) = budget.check_exceeded() {
//!     // Handle budget exceeded - abort execution
//!     let event = create_abort_event(&agent_identity, reason, &execution_ref);
//!     return Err(event);
//! }
//! ```

use std::time::Instant;

use serde::{Deserialize, Serialize};

use agentics_contracts::{
    Confidence, Constraint, ConstraintEffect, DecisionEvent, DecisionOutput, DecisionType,
};

use super::identity::AgentIdentity;

// Re-export constants from config for convenience (they are the source of truth)
pub use super::config::{MAX_CALLS_PER_RUN, MAX_LATENCY_MS, MAX_TOKENS};

// =============================================================================
// Budget Exceeded Reason
// =============================================================================

/// Reason why a performance budget was exceeded.
///
/// This enum captures the specific limit that was breached, along with
/// the actual and maximum values for diagnostic purposes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BudgetExceededReason {
    /// Token usage exceeded the maximum allowed.
    TokensExceeded {
        /// Number of tokens actually used.
        used: u32,
        /// Maximum tokens allowed.
        max: u32,
    },

    /// Execution latency exceeded the maximum allowed.
    LatencyExceeded {
        /// Elapsed time in milliseconds.
        elapsed_ms: u64,
        /// Maximum latency allowed in milliseconds.
        max_ms: u64,
    },

    /// Number of API calls exceeded the maximum allowed.
    CallsExceeded {
        /// Number of calls actually made.
        made: u32,
        /// Maximum calls allowed.
        max: u32,
    },
}

impl std::fmt::Display for BudgetExceededReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokensExceeded { used, max } => {
                write!(f, "Token budget exceeded: {used} tokens used, max {max}")
            }
            Self::LatencyExceeded { elapsed_ms, max_ms } => {
                write!(
                    f,
                    "Latency budget exceeded: {elapsed_ms}ms elapsed, max {max_ms}ms"
                )
            }
            Self::CallsExceeded { made, max } => {
                write!(f, "API call budget exceeded: {made} calls made, max {max}")
            }
        }
    }
}

// =============================================================================
// Budget Tracker
// =============================================================================

/// Tracks resource usage during agent execution and enforces budget limits.
///
/// A `BudgetTracker` is created at the start of agent execution and
/// accumulates usage metrics. The `check_exceeded` method should be called
/// after each operation to verify the agent is still within budget.
///
/// This is the runtime counterpart to `PerformanceBudget` (which holds constants).
#[derive(Debug, Clone)]
pub struct BudgetTracker {
    /// Total tokens consumed so far.
    pub tokens_used: u32,

    /// Total elapsed time since budget creation (computed dynamically).
    pub latency_ms: u64,

    /// Number of API calls made so far.
    pub calls_made: u32,

    /// Instant when the budget was created.
    pub started_at: Instant,
}

impl Default for BudgetTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl BudgetTracker {
    /// Creates a new budget tracker with zero usage.
    ///
    /// The latency timer starts immediately upon creation.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tokens_used: 0,
            latency_ms: 0,
            calls_made: 0,
            started_at: Instant::now(),
        }
    }

    /// Records token usage from an operation.
    ///
    /// Token counts are accumulated across all operations.
    ///
    /// # Arguments
    ///
    /// * `tokens` - Number of tokens consumed by the operation
    pub fn record_tokens(&mut self, tokens: u32) {
        self.tokens_used = self.tokens_used.saturating_add(tokens);
    }

    /// Records an API call.
    ///
    /// Call counts are accumulated and checked against `MAX_CALLS_PER_RUN`.
    pub fn record_call(&mut self) {
        self.calls_made = self.calls_made.saturating_add(1);
    }

    /// Returns the current elapsed latency in milliseconds.
    ///
    /// This is computed from the instant the budget was created.
    #[must_use]
    pub fn current_latency_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }

    /// Checks if any budget limit is exceeded.
    ///
    /// This method checks all limits in order of severity:
    /// 1. Token limit (cost-related)
    /// 2. Call limit (loop prevention)
    /// 3. Latency limit (timeout)
    ///
    /// # Returns
    ///
    /// - `Some(BudgetExceededReason)` if any limit is exceeded
    /// - `None` if all limits are within bounds
    #[must_use]
    pub fn check_exceeded(&self) -> Option<BudgetExceededReason> {
        // Check token limit first (most likely to be hit in normal operation)
        if self.tokens_used > MAX_TOKENS {
            return Some(BudgetExceededReason::TokensExceeded {
                used: self.tokens_used,
                max: MAX_TOKENS,
            });
        }

        // Check call limit (prevents infinite loops)
        if self.calls_made > MAX_CALLS_PER_RUN {
            return Some(BudgetExceededReason::CallsExceeded {
                made: self.calls_made,
                max: MAX_CALLS_PER_RUN,
            });
        }

        // Check latency limit last (uses syscall to get current time)
        let elapsed = self.current_latency_ms();
        if elapsed > MAX_LATENCY_MS {
            return Some(BudgetExceededReason::LatencyExceeded {
                elapsed_ms: elapsed,
                max_ms: MAX_LATENCY_MS,
            });
        }

        None
    }

    /// Returns a summary of current usage as a percentage of limits.
    ///
    /// Each value is a percentage from 0.0 to potentially > 1.0 if exceeded.
    #[must_use]
    pub fn usage_summary(&self) -> BudgetUsageSummary {
        BudgetUsageSummary {
            tokens_percent: (self.tokens_used as f64 / MAX_TOKENS as f64) * 100.0,
            calls_percent: (self.calls_made as f64 / MAX_CALLS_PER_RUN as f64) * 100.0,
            latency_percent: (self.current_latency_ms() as f64 / MAX_LATENCY_MS as f64) * 100.0,
        }
    }
}

/// Summary of budget usage as percentages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetUsageSummary {
    /// Token usage as percentage of limit.
    pub tokens_percent: f64,

    /// API call usage as percentage of limit.
    pub calls_percent: f64,

    /// Latency usage as percentage of limit.
    pub latency_percent: f64,
}

// =============================================================================
// Abort Event Creation
// =============================================================================

/// Creates an abort `DecisionEvent` when a performance budget is exceeded.
///
/// The abort event captures:
/// - The agent identity (ID and version)
/// - The specific budget violation reason
/// - The execution reference for tracing
///
/// # Arguments
///
/// * `agent_identity` - Identity of the agent being aborted
/// * `reason` - The specific budget limit that was exceeded
/// * `execution_ref` - Reference to the execution context (request ID, trace ID)
///
/// # Returns
///
/// A `DecisionEvent` with `DecisionType::RouteReject` indicating the abort.
#[must_use]
pub fn create_abort_event(
    agent_identity: &AgentIdentity,
    reason: BudgetExceededReason,
    execution_ref: &str,
) -> DecisionEvent {
    let rejection_message = format!("Agent execution aborted: {reason}");

    // Create the decision output indicating rejection due to budget
    let output = DecisionOutput::rejected(&rejection_message);

    // Create a budget constraint to document what was violated
    #[allow(unused_variables)]
    let constraint = match &reason {
        BudgetExceededReason::TokensExceeded { used, max } => Constraint::Policy {
            policy_id: "performance_budget_tokens".to_string(),
            effect: ConstraintEffect::Deny,
        },
        BudgetExceededReason::LatencyExceeded { elapsed_ms, max_ms } => Constraint::Policy {
            policy_id: "performance_budget_latency".to_string(),
            effect: ConstraintEffect::Deny,
        },
        BudgetExceededReason::CallsExceeded { made, max } => Constraint::Policy {
            policy_id: "performance_budget_calls".to_string(),
            effect: ConstraintEffect::Deny,
        },
    };

    // Create the decision event
    DecisionEvent::new(
        agent_identity.qualified_id(),
        &agent_identity.agent_version,
        DecisionType::RouteReject,
        // Use a placeholder hash since we don't have the original input
        "0".repeat(64),
        output,
        Confidence::zero(),
        vec![constraint],
        execution_ref,
    )
}

/// Creates a detailed abort event with full budget state captured.
///
/// This variant includes the complete budget state in the event for
/// detailed diagnostics and post-mortem analysis.
///
/// # Arguments
///
/// * `agent_identity` - Identity of the agent being aborted
/// * `budget` - The budget tracker that was exceeded
/// * `reason` - The specific budget limit that was exceeded
/// * `execution_ref` - Reference to the execution context
/// * `inputs_hash` - SHA-256 hash of the input data
///
/// # Returns
///
/// A `DecisionEvent` with full budget diagnostics.
#[must_use]
pub fn create_detailed_abort_event(
    agent_identity: &AgentIdentity,
    budget: &BudgetTracker,
    reason: BudgetExceededReason,
    execution_ref: &str,
    inputs_hash: &str,
) -> DecisionEvent {
    let rejection_message = format!(
        "Agent execution aborted: {}. Budget state: tokens={}/{}, calls={}/{}, latency={}ms/{}ms",
        reason,
        budget.tokens_used,
        MAX_TOKENS,
        budget.calls_made,
        MAX_CALLS_PER_RUN,
        budget.current_latency_ms(),
        MAX_LATENCY_MS
    );

    let output = DecisionOutput::rejected(&rejection_message);

    // Include all budget constraints as documentation
    let constraints = vec![
        Constraint::Policy {
            policy_id: format!("budget_tokens_{}_{}", budget.tokens_used, MAX_TOKENS),
            effect: if budget.tokens_used > MAX_TOKENS {
                ConstraintEffect::Deny
            } else {
                ConstraintEffect::Allow
            },
        },
        Constraint::Policy {
            policy_id: format!("budget_calls_{}_{}", budget.calls_made, MAX_CALLS_PER_RUN),
            effect: if budget.calls_made > MAX_CALLS_PER_RUN {
                ConstraintEffect::Deny
            } else {
                ConstraintEffect::Allow
            },
        },
        Constraint::Policy {
            policy_id: format!(
                "budget_latency_{}_{}",
                budget.current_latency_ms(),
                MAX_LATENCY_MS
            ),
            effect: if budget.current_latency_ms() > MAX_LATENCY_MS {
                ConstraintEffect::Deny
            } else {
                ConstraintEffect::Allow
            },
        },
    ];

    DecisionEvent::new(
        agent_identity.qualified_id(),
        &agent_identity.agent_version,
        DecisionType::RouteReject,
        inputs_hash,
        output,
        Confidence::zero(),
        constraints,
        execution_ref,
    )
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    fn create_test_identity() -> AgentIdentity {
        AgentIdentity::new(
            "test-agent",
            "routing",
            "phase7",
            "layer2",
            "1.0.0",
        )
    }

    #[test]
    fn test_budget_creation() {
        let budget = BudgetTracker::new();
        assert_eq!(budget.tokens_used, 0);
        assert_eq!(budget.calls_made, 0);
        assert!(budget.check_exceeded().is_none());
    }

    #[test]
    fn test_budget_default() {
        let budget = BudgetTracker::default();
        assert_eq!(budget.tokens_used, 0);
        assert_eq!(budget.calls_made, 0);
    }

    #[test]
    fn test_record_tokens() {
        let mut budget = BudgetTracker::new();
        budget.record_tokens(500);
        assert_eq!(budget.tokens_used, 500);

        budget.record_tokens(300);
        assert_eq!(budget.tokens_used, 800);
    }

    #[test]
    fn test_record_calls() {
        let mut budget = BudgetTracker::new();
        budget.record_call();
        assert_eq!(budget.calls_made, 1);

        budget.record_call();
        budget.record_call();
        assert_eq!(budget.calls_made, 3);
    }

    #[test]
    fn test_tokens_exceeded() {
        let mut budget = BudgetTracker::new();
        budget.record_tokens(MAX_TOKENS + 1);

        let reason = budget.check_exceeded();
        assert!(reason.is_some());

        match reason.unwrap() {
            BudgetExceededReason::TokensExceeded { used, max } => {
                assert_eq!(used, MAX_TOKENS + 1);
                assert_eq!(max, MAX_TOKENS);
            }
            _ => panic!("Expected TokensExceeded"),
        }
    }

    #[test]
    fn test_calls_exceeded() {
        let mut budget = BudgetTracker::new();
        for _ in 0..=MAX_CALLS_PER_RUN {
            budget.record_call();
        }

        let reason = budget.check_exceeded();
        assert!(reason.is_some());

        match reason.unwrap() {
            BudgetExceededReason::CallsExceeded { made, max } => {
                assert_eq!(made, MAX_CALLS_PER_RUN + 1);
                assert_eq!(max, MAX_CALLS_PER_RUN);
            }
            _ => panic!("Expected CallsExceeded"),
        }
    }

    #[test]
    fn test_current_latency() {
        let budget = BudgetTracker::new();
        sleep(Duration::from_millis(10));
        let latency = budget.current_latency_ms();
        assert!(latency >= 10);
    }

    #[test]
    fn test_usage_summary() {
        let mut budget = BudgetTracker::new();
        budget.record_tokens(MAX_TOKENS / 2);
        budget.record_call();
        budget.record_call();

        let summary = budget.usage_summary();
        assert!((summary.tokens_percent - 50.0).abs() < 0.1);
        assert!((summary.calls_percent - 40.0).abs() < 0.1);
    }

    #[test]
    fn test_saturating_add() {
        let mut budget = BudgetTracker::new();
        budget.tokens_used = u32::MAX;
        budget.record_tokens(1);
        assert_eq!(budget.tokens_used, u32::MAX); // Should not overflow
    }

    #[test]
    fn test_budget_exceeded_reason_display() {
        let token_reason = BudgetExceededReason::TokensExceeded {
            used: 3000,
            max: 2500,
        };
        assert_eq!(
            token_reason.to_string(),
            "Token budget exceeded: 3000 tokens used, max 2500"
        );

        let latency_reason = BudgetExceededReason::LatencyExceeded {
            elapsed_ms: 6000,
            max_ms: 5000,
        };
        assert_eq!(
            latency_reason.to_string(),
            "Latency budget exceeded: 6000ms elapsed, max 5000ms"
        );

        let calls_reason = BudgetExceededReason::CallsExceeded { made: 6, max: 5 };
        assert_eq!(
            calls_reason.to_string(),
            "API call budget exceeded: 6 calls made, max 5"
        );
    }

    #[test]
    fn test_agent_identity() {
        let identity = create_test_identity();
        assert_eq!(identity.source_agent, "test-agent");
        assert_eq!(identity.domain, "routing");
        assert_eq!(identity.phase, "phase7");
        assert_eq!(identity.layer, "layer2");
        assert_eq!(identity.agent_version, "1.0.0");
    }

    #[test]
    fn test_create_abort_event() {
        let identity = AgentIdentity::new(
            "phase7-agent",
            "routing",
            "phase7",
            "layer2",
            "0.1.0",
        );
        let reason = BudgetExceededReason::TokensExceeded {
            used: 3000,
            max: 2500,
        };

        let event = create_abort_event(&identity, reason, "req-123");

        assert_eq!(event.agent_id, "phase7-agent:routing:phase7:layer2");
        assert_eq!(event.agent_version, "0.1.0");
        assert_eq!(event.decision_type, DecisionType::RouteReject);
        assert_eq!(event.execution_ref, "req-123");
        assert!(event.outputs.rejection_reason.is_some());
        assert!(event
            .outputs
            .rejection_reason
            .as_ref()
            .unwrap()
            .contains("Token budget exceeded"));
    }

    #[test]
    fn test_create_detailed_abort_event() {
        let identity = AgentIdentity::new(
            "phase7-agent",
            "routing",
            "phase7",
            "layer2",
            "0.1.0",
        );
        let mut budget = BudgetTracker::new();
        budget.record_tokens(3000);
        budget.record_call();
        budget.record_call();

        let reason = BudgetExceededReason::TokensExceeded {
            used: 3000,
            max: 2500,
        };

        let event = create_detailed_abort_event(
            &identity,
            &budget,
            reason,
            "req-456",
            &"a".repeat(64),
        );

        assert_eq!(event.agent_id, "phase7-agent:routing:phase7:layer2");
        assert_eq!(event.decision_type, DecisionType::RouteReject);
        assert_eq!(event.constraints_applied.len(), 3);

        // Verify the rejection message contains budget state
        let rejection = event.outputs.rejection_reason.unwrap();
        assert!(rejection.contains("tokens=3000/2500"));
        assert!(rejection.contains("calls=2/5"));
    }

    #[test]
    fn test_constraint_serialization() {
        let reason = BudgetExceededReason::LatencyExceeded {
            elapsed_ms: 6000,
            max_ms: 5000,
        };

        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("\"type\":\"latency_exceeded\""));
        assert!(json.contains("\"elapsed_ms\":6000"));
        assert!(json.contains("\"max_ms\":5000"));

        let deserialized: BudgetExceededReason = serde_json::from_str(&json).unwrap();
        assert_eq!(reason, deserialized);
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_TOKENS, 2500);
        assert_eq!(MAX_LATENCY_MS, 5000);
        assert_eq!(MAX_CALLS_PER_RUN, 5);
    }

    #[test]
    fn test_within_budget() {
        let mut budget = BudgetTracker::new();
        budget.record_tokens(1000);
        budget.record_call();
        budget.record_call();
        budget.record_call();

        // Should be within all limits
        assert!(budget.check_exceeded().is_none());
    }

    #[test]
    fn test_multiple_budget_checks() {
        let mut budget = BudgetTracker::new();

        // Check initially empty
        assert!(budget.check_exceeded().is_none());

        // Add some usage, still within limits
        budget.record_tokens(1000);
        budget.record_call();
        assert!(budget.check_exceeded().is_none());

        // Exceed token limit
        budget.record_tokens(2000);
        assert!(budget.check_exceeded().is_some());
    }
}
