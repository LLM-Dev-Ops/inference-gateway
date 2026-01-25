//! # Phase 7 Startup Hardening
//!
//! Provides mandatory environment variable validation and Ruvector health checking
//! for Phase 7 agents. This module enforces strict startup requirements:
//!
//! - ALL required environment variables MUST be present
//! - Ruvector health check MUST pass
//! - NO graceful degradation or fallback logic
//! - Startup failure results in immediate abort (panic)
//!
//! ## Required Environment Variables
//!
//! - `RUVECTOR_SERVICE_URL`: URL of the Ruvector service
//! - `RUVECTOR_API_KEY`: API key for Ruvector authentication
//! - `AGENT_NAME`: Name identifier for this agent
//! - `AGENT_DOMAIN`: Operational domain (e.g., "routing", "inference")
//! - `AGENT_PHASE`: Must be "phase7"
//! - `AGENT_LAYER`: Must be "layer2"
//! - `AGENT_VERSION`: Semantic version of the agent
//!
//! ## Usage
//!
//! ```ignore
//! use gateway_agents::phase7::{Phase7Bootstrap, Phase7Config};
//!
//! // This will panic if any required env vars are missing
//! // or if Ruvector health check fails
//! let bootstrap = Phase7Bootstrap::init().await;
//!
//! // Access configuration
//! let config = bootstrap.config();
//! let identity = bootstrap.identity();
//! ```

pub mod budget;
pub mod config;
pub mod identity;

pub use budget::{
    create_abort_event, create_detailed_abort_event, BudgetExceededReason, BudgetTracker,
    BudgetUsageSummary,
};
pub use config::{Phase7Config, PerformanceBudget, MAX_CALLS_PER_RUN, MAX_LATENCY_MS, MAX_TOKENS};
pub use identity::AgentIdentity;

use agentics_contracts::{Confidence, DecisionEvent, DecisionType};
use std::time::Duration;
use tracing::{error, info};

/// Phase 7 bootstrap controller.
///
/// Manages startup validation and Ruvector connectivity.
/// All validation failures result in immediate abort.
#[derive(Debug, Clone)]
pub struct Phase7Bootstrap {
    config: Phase7Config,
    identity: AgentIdentity,
}

impl Phase7Bootstrap {
    /// Initialize Phase 7 bootstrap with full validation.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - Any required environment variable is missing
    /// - Environment variable values are invalid
    /// - Ruvector health check fails
    ///
    /// There is NO fallback or degraded mode.
    pub async fn init() -> Self {
        // Load and validate configuration (panics on failure)
        let config = Phase7Config::from_env();

        // Create identity from config
        let identity = AgentIdentity::from_config(&config);

        // Perform Ruvector health check (panics on failure)
        Self::check_ruvector_health(&config).await;

        // Log successful startup
        info!(
            target: "phase7",
            agent_name = %config.agent_name,
            agent_version = %config.agent_version,
            phase = "phase7",
            layer = "layer2",
            ruvector = true,
            "agent_started"
        );

        Self { config, identity }
    }

    /// Check Ruvector service health.
    ///
    /// # Panics
    ///
    /// Panics immediately if the health check fails.
    /// NO degraded mode. NO fallback.
    async fn check_ruvector_health(config: &Phase7Config) {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config::MAX_LATENCY_MS))
            .build()
            .unwrap_or_else(|e| {
                error!(
                    target: "phase7",
                    reason = format!("Failed to create HTTP client: {}", e),
                    "agent_abort"
                );
                panic!("Phase 7 startup aborted: Failed to create HTTP client: {}", e);
            });

        let health_url = format!("{}/health", config.ruvector_service_url);

        let result = client
            .get(&health_url)
            .header("Authorization", format!("Bearer {}", config.ruvector_api_key))
            .send()
            .await;

        match result {
            Ok(response) => {
                if !response.status().is_success() {
                    error!(
                        target: "phase7",
                        reason = format!(
                            "Ruvector health check failed with status: {}",
                            response.status()
                        ),
                        "agent_abort"
                    );
                    panic!(
                        "Phase 7 startup aborted: Ruvector health check failed with status: {}",
                        response.status()
                    );
                }
            }
            Err(e) => {
                error!(
                    target: "phase7",
                    reason = format!("Ruvector health check request failed: {}", e),
                    "agent_abort"
                );
                panic!(
                    "Phase 7 startup aborted: Ruvector health check request failed: {}",
                    e
                );
            }
        }
    }

    /// Get the validated configuration.
    #[must_use]
    pub fn config(&self) -> &Phase7Config {
        &self.config
    }

    /// Get the agent identity.
    #[must_use]
    pub fn identity(&self) -> &AgentIdentity {
        &self.identity
    }

    /// Inject identity into a `DecisionEvent`.
    #[must_use]
    pub fn inject_identity(&self, event: DecisionEvent) -> DecisionEvent {
        self.identity.inject_into_event(event)
    }

    /// Log a decision event emission.
    ///
    /// Emits the required observability log:
    /// `decision_event_emitted { event_type, confidence }`
    pub fn log_decision_event(&self, event: &DecisionEvent) {
        info!(
            target: "phase7",
            event_type = %event.decision_type.signal_name(),
            confidence = %event.confidence.overall,
            "decision_event_emitted"
        );
    }

    /// Create a decision event with identity injection.
    ///
    /// This is a convenience method that creates a `DecisionEvent`
    /// with identity already injected.
    #[must_use]
    pub fn create_decision_event(
        &self,
        decision_type: DecisionType,
        inputs_hash: impl Into<String>,
        outputs: agentics_contracts::DecisionOutput,
        confidence: Confidence,
        constraints_applied: Vec<agentics_contracts::Constraint>,
        execution_ref: impl Into<String>,
    ) -> DecisionEvent {
        let event = DecisionEvent::new(
            self.identity.qualified_id(),
            &self.config.agent_version,
            decision_type,
            inputs_hash,
            outputs,
            confidence,
            constraints_applied,
            execution_ref,
        );

        // Log the emission
        self.log_decision_event(&event);

        event
    }

    /// Get performance budget constants.
    #[must_use]
    pub const fn performance_budget() -> PerformanceBudget {
        Phase7Config::performance_budget()
    }
}

/// Abort the agent with a reason.
///
/// This function logs the abort reason and panics immediately.
/// Use this for any unrecoverable error during Phase 7 operation.
pub fn abort(reason: impl AsRef<str>) -> ! {
    error!(
        target: "phase7",
        reason = %reason.as_ref(),
        "agent_abort"
    );
    panic!("Phase 7 agent aborted: {}", reason.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abort_panics() {
        let result = std::panic::catch_unwind(|| {
            abort("test abort reason");
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_performance_budget() {
        let budget = Phase7Bootstrap::performance_budget();
        assert_eq!(budget.max_tokens, 2500);
        assert_eq!(budget.max_latency_ms, 5000);
        assert_eq!(budget.max_calls_per_run, 5);
    }
}
