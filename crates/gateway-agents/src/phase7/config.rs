//! Phase 7 configuration with mandatory environment variable validation.
//!
//! This module enforces strict startup requirements for Phase 7 agents:
//! - ALL required environment variables MUST be present
//! - Missing variables cause immediate startup abort (panic)
//! - NO graceful degradation or fallback logic

use std::{env, fmt};
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
#[derive(Clone)]
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

// Startup diagnostics must never disclose the configured authentication key.
impl fmt::Debug for Phase7Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Phase7Config")
            .field("ruvector_service_url", &self.ruvector_service_url)
            .field("ruvector_api_key", &"[REDACTED]")
            .field("agent_name", &self.agent_name)
            .field("agent_domain", &self.agent_domain)
            .field("agent_phase", &self.agent_phase)
            .field("agent_layer", &self.agent_layer)
            .field("agent_version", &self.agent_version)
            .finish()
    }
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
        Self::from_lookup(|name| env::var(name).ok())
    }

    // Keep parsing independent of global environment mutation in unit tests.
    // The public startup entry point still reads the real environment above.
    fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Self {
        let mut missing_vars = Vec::new();

        let mut required = |name: &str| lookup(name).filter(|value| !value.trim().is_empty());
        let ruvector_service_url = required("RUVECTOR_SERVICE_URL");
        let ruvector_api_key = required("RUVECTOR_API_KEY");
        let agent_name = required("AGENT_NAME");
        let agent_domain = required("AGENT_DOMAIN");
        let agent_phase = required("AGENT_PHASE");
        let agent_layer = required("AGENT_LAYER");
        let agent_version = required("AGENT_VERSION");

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

        let (
            Some(ruvector_service_url),
            Some(ruvector_api_key),
            Some(agent_name),
            Some(agent_domain),
            Some(agent_phase_value),
            Some(agent_layer_value),
            Some(agent_version),
        ) = (
            ruvector_service_url,
            ruvector_api_key,
            agent_name,
            agent_domain,
            agent_phase,
            agent_layer,
            agent_version,
        )
        else {
            error!(
                target: "phase7",
                reason = format!("Missing required environment variables: {}", missing_vars.join(", ")),
                "agent_abort"
            );
            panic!(
                "Phase 7 startup aborted: Missing required environment variables: {}",
                missing_vars.join(", ")
            );
        };

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
            ruvector_service_url,
            ruvector_api_key,
            agent_name,
            agent_domain,
            agent_phase: agent_phase_value,
            agent_layer: agent_layer_value,
            agent_version,
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
    use std::collections::BTreeMap;

    fn valid_values() -> BTreeMap<&'static str, String> {
        [
            ("RUVECTOR_SERVICE_URL", "http://localhost:8080"),
            ("RUVECTOR_API_KEY", "synthetic-config-test-value"),
            ("AGENT_NAME", "test-agent"),
            ("AGENT_DOMAIN", "routing"),
            ("AGENT_PHASE", "phase7"),
            ("AGENT_LAYER", "layer2"),
            ("AGENT_VERSION", "1.0.0"),
        ]
        .into_iter()
        .map(|(name, value)| (name, value.to_owned()))
        .collect()
    }

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
        let _ = Phase7Config::from_lookup(|_| None);
    }

    #[test]
    #[should_panic(expected = "AGENT_PHASE must be 'phase7'")]
    fn test_from_env_panics_on_wrong_phase() {
        let mut values = valid_values();
        values.insert("AGENT_PHASE", "phase6".to_owned());
        let _ = Phase7Config::from_lookup(|name| values.get(name).cloned());
    }

    #[test]
    #[should_panic(expected = "AGENT_LAYER must be 'layer2'")]
    fn test_from_env_panics_on_wrong_layer() {
        let mut values = valid_values();
        values.insert("AGENT_LAYER", "layer1".to_owned());
        let _ = Phase7Config::from_lookup(|name| values.get(name).cloned());
    }

    #[test]
    fn test_required_values_reject_blanks() {
        for name in valid_values().keys() {
            for blank in ["", " \t "] {
                let mut values = valid_values();
                values.insert(name, blank.to_owned());
                let result = std::panic::catch_unwind(|| {
                    Phase7Config::from_lookup(|key| values.get(key).cloned())
                });
                assert!(result.is_err(), "{name} must reject blank values");
            }
        }
    }

    #[test]
    fn test_debug_redacts_api_key() {
        let values = valid_values();
        let config = Phase7Config::from_lookup(|name| values.get(name).cloned());
        let debug = format!("{config:?}");
        assert!(!debug.contains(&config.ruvector_api_key));
        assert!(debug.contains("[REDACTED]"));
        assert!(debug.contains(&config.agent_name));
    }

    #[test]
    fn test_parallel_configurations_are_isolated() {
        let threads: Vec<_> = (0..8)
            .map(|thread| {
                std::thread::spawn(move || {
                    let name = format!("agent-{thread}");
                    let mut values = valid_values();
                    values.insert("AGENT_NAME", name.clone());
                    for _ in 0..30 {
                        let config = Phase7Config::from_lookup(|key| values.get(key).cloned());
                        assert_eq!(config.agent_name, name);
                    }
                })
            })
            .collect();
        for thread in threads {
            assert!(thread.join().is_ok());
        }
    }
}
