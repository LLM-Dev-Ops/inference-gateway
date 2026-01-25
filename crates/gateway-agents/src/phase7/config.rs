//! Phase 7 configuration with mandatory environment variable validation.
//!
//! This module enforces strict startup requirements for Phase 7 agents:
//! - ALL required environment variables MUST be present
//! - Missing variables cause immediate startup abort (panic)
//! - NO graceful degradation or fallback logic

use std::env;
use tracing::error;

/// Maximum tokens allowed per agent run.
pub const MAX_TOKENS: u32 = 2500;

/// Maximum latency in milliseconds before timeout.
pub const MAX_LATENCY_MS: u64 = 5000;

/// Maximum LLM API calls allowed per run.
pub const MAX_CALLS_PER_RUN: u32 = 5;

/// Phase 7 configuration loaded from environment variables.
///
/// All fields are mandatory. If any are missing, the agent will abort.
#[derive(Debug, Clone)]
pub struct Phase7Config {
    /// Ruvector service URL for telemetry and intelligence.
    pub ruvector_service_url: String,
    /// API key for Ruvector authentication.
    pub ruvector_api_key: String,
    /// Agent name identifier.
    pub agent_name: String,
    /// Agent domain (e.g., "routing", "inference").
    pub agent_domain: String,
    /// Agent phase - MUST be "phase7".
    pub agent_phase: String,
    /// Agent layer - MUST be "layer2".
    pub agent_layer: String,
    /// Agent semantic version.
    pub agent_version: String,
}

impl Phase7Config {
    /// Load configuration from environment variables.
    ///
    /// # Panics
    ///
    /// Panics immediately if ANY required environment variable is missing.
    /// This is intentional - Phase 7 agents MUST NOT start in a degraded state.
    #[must_use]
    pub fn from_env() -> Self {
        let mut missing_vars = Vec::new();

        let ruvector_service_url = env::var("RUVECTOR_SERVICE_URL").ok();
        let ruvector_api_key = env::var("RUVECTOR_API_KEY").ok();
        let agent_name = env::var("AGENT_NAME").ok();
        let agent_domain = env::var("AGENT_DOMAIN").ok();
        let agent_phase = env::var("AGENT_PHASE").ok();
        let agent_layer = env::var("AGENT_LAYER").ok();
        let agent_version = env::var("AGENT_VERSION").ok();

        if ruvector_service_url.is_none() {
            missing_vars.push("RUVECTOR_SERVICE_URL");
        }
        if ruvector_api_key.is_none() {
            missing_vars.push("RUVECTOR_API_KEY");
        }
        if agent_name.is_none() {
            missing_vars.push("AGENT_NAME");
        }
        if agent_domain.is_none() {
            missing_vars.push("AGENT_DOMAIN");
        }
        if agent_phase.is_none() {
            missing_vars.push("AGENT_PHASE");
        }
        if agent_layer.is_none() {
            missing_vars.push("AGENT_LAYER");
        }
        if agent_version.is_none() {
            missing_vars.push("AGENT_VERSION");
        }

        if !missing_vars.is_empty() {
            error!(
                target: "phase7",
                reason = format!("Missing required environment variables: {}", missing_vars.join(", ")),
                "agent_abort"
            );
            panic!(
                "Phase 7 startup aborted: Missing required environment variables: {}",
                missing_vars.join(", ")
            );
        }

        let agent_phase_value = agent_phase.unwrap();
        let agent_layer_value = agent_layer.unwrap();

        // Validate phase and layer values
        if agent_phase_value != "phase7" {
            error!(
                target: "phase7",
                reason = format!("AGENT_PHASE must be 'phase7', got '{}'", agent_phase_value),
                "agent_abort"
            );
            panic!(
                "Phase 7 startup aborted: AGENT_PHASE must be 'phase7', got '{}'",
                agent_phase_value
            );
        }

        if agent_layer_value != "layer2" {
            error!(
                target: "phase7",
                reason = format!("AGENT_LAYER must be 'layer2', got '{}'", agent_layer_value),
                "agent_abort"
            );
            panic!(
                "Phase 7 startup aborted: AGENT_LAYER must be 'layer2', got '{}'",
                agent_layer_value
            );
        }

        Self {
            ruvector_service_url: ruvector_service_url.unwrap(),
            ruvector_api_key: ruvector_api_key.unwrap(),
            agent_name: agent_name.unwrap(),
            agent_domain: agent_domain.unwrap(),
            agent_phase: agent_phase_value,
            agent_layer: agent_layer_value,
            agent_version: agent_version.unwrap(),
        }
    }

    /// Get performance budget constants.
    #[must_use]
    pub const fn performance_budget() -> PerformanceBudget {
        PerformanceBudget {
            max_tokens: MAX_TOKENS,
            max_latency_ms: MAX_LATENCY_MS,
            max_calls_per_run: MAX_CALLS_PER_RUN,
        }
    }
}

/// Performance budget constraints for Phase 7 agents.
#[derive(Debug, Clone, Copy)]
pub struct PerformanceBudget {
    /// Maximum tokens per run.
    pub max_tokens: u32,
    /// Maximum latency in milliseconds.
    pub max_latency_ms: u64,
    /// Maximum API calls per run.
    pub max_calls_per_run: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_budget_constants() {
        let budget = Phase7Config::performance_budget();
        assert_eq!(budget.max_tokens, 2500);
        assert_eq!(budget.max_latency_ms, 5000);
        assert_eq!(budget.max_calls_per_run, 5);
    }

    #[test]
    #[should_panic(expected = "Phase 7 startup aborted")]
    fn test_from_env_panics_on_missing_vars() {
        // Clear any existing env vars
        env::remove_var("RUVECTOR_SERVICE_URL");
        env::remove_var("RUVECTOR_API_KEY");
        env::remove_var("AGENT_NAME");
        env::remove_var("AGENT_DOMAIN");
        env::remove_var("AGENT_PHASE");
        env::remove_var("AGENT_LAYER");
        env::remove_var("AGENT_VERSION");

        // This should panic
        let _ = Phase7Config::from_env();
    }

    #[test]
    #[should_panic(expected = "AGENT_PHASE must be 'phase7'")]
    fn test_from_env_panics_on_wrong_phase() {
        env::set_var("RUVECTOR_SERVICE_URL", "http://localhost:8080");
        env::set_var("RUVECTOR_API_KEY", "test-key");
        env::set_var("AGENT_NAME", "test-agent");
        env::set_var("AGENT_DOMAIN", "routing");
        env::set_var("AGENT_PHASE", "phase6"); // Wrong phase
        env::set_var("AGENT_LAYER", "layer2");
        env::set_var("AGENT_VERSION", "1.0.0");

        let _ = Phase7Config::from_env();
    }

    #[test]
    #[should_panic(expected = "AGENT_LAYER must be 'layer2'")]
    fn test_from_env_panics_on_wrong_layer() {
        env::set_var("RUVECTOR_SERVICE_URL", "http://localhost:8080");
        env::set_var("RUVECTOR_API_KEY", "test-key");
        env::set_var("AGENT_NAME", "test-agent");
        env::set_var("AGENT_DOMAIN", "routing");
        env::set_var("AGENT_PHASE", "phase7");
        env::set_var("AGENT_LAYER", "layer1"); // Wrong layer
        env::set_var("AGENT_VERSION", "1.0.0");

        let _ = Phase7Config::from_env();
    }
}
