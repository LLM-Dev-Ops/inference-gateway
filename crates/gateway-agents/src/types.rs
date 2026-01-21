//! Common types for gateway agents.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Agent version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVersion {
    /// Major version
    pub major: u32,
    /// Minor version
    pub minor: u32,
    /// Patch version
    pub patch: u32,
    /// Build metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
}

impl AgentVersion {
    /// Create a new version
    #[must_use]
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            build: None,
        }
    }

    /// Create version with build metadata
    #[must_use]
    pub fn with_build(mut self, build: impl Into<String>) -> Self {
        self.build = Some(build.into());
        self
    }
}

impl Default for AgentVersion {
    fn default() -> Self {
        Self::new(0, 1, 0)
    }
}

impl std::fmt::Display for AgentVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(ref build) = self.build {
            write!(f, "+{build}")?;
        }
        Ok(())
    }
}

/// Agent health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentHealth {
    /// Agent is healthy and functioning normally
    Healthy,
    /// Agent is experiencing degraded performance
    Degraded,
    /// Agent is unhealthy and may not function properly
    Unhealthy,
    /// Agent health is unknown
    Unknown,
}

impl Default for AgentHealth {
    fn default() -> Self {
        Self::Unknown
    }
}

impl std::fmt::Display for AgentHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded => write!(f, "degraded"),
            Self::Unhealthy => write!(f, "unhealthy"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Agent status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    /// Agent identifier
    pub agent_id: String,
    /// Agent type/name
    pub agent_type: String,
    /// Current health status
    pub health: AgentHealth,
    /// Agent version
    pub version: AgentVersion,
    /// Whether the agent is ready to handle requests
    pub ready: bool,
    /// Number of requests processed
    pub requests_processed: u64,
    /// Number of errors encountered
    pub errors: u64,
    /// Average latency in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_latency_ms: Option<f64>,
    /// Time the agent was started
    pub started_at: DateTime<Utc>,
    /// Last activity timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity: Option<DateTime<Utc>>,
    /// Additional status details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl AgentStatus {
    /// Create a new agent status
    #[must_use]
    pub fn new(agent_id: impl Into<String>, agent_type: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            agent_type: agent_type.into(),
            health: AgentHealth::Unknown,
            version: AgentVersion::default(),
            ready: false,
            requests_processed: 0,
            errors: 0,
            avg_latency_ms: None,
            started_at: Utc::now(),
            last_activity: None,
            details: None,
        }
    }

    /// Set health status
    #[must_use]
    pub fn with_health(mut self, health: AgentHealth) -> Self {
        self.health = health;
        self
    }

    /// Set version
    #[must_use]
    pub fn with_version(mut self, version: AgentVersion) -> Self {
        self.version = version;
        self
    }

    /// Mark as ready
    #[must_use]
    pub fn ready(mut self) -> Self {
        self.ready = true;
        self
    }

    /// Calculate uptime in seconds
    #[must_use]
    pub fn uptime_seconds(&self) -> i64 {
        (Utc::now() - self.started_at).num_seconds()
    }

    /// Calculate success rate
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        if self.requests_processed == 0 {
            1.0
        } else {
            (self.requests_processed - self.errors) as f64 / self.requests_processed as f64
        }
    }
}

/// Agent metadata for discovery and listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    /// Agent identifier
    pub id: String,
    /// Agent type/name
    pub agent_type: String,
    /// Human-readable description
    pub description: String,
    /// Agent version
    pub version: AgentVersion,
    /// Supported capabilities
    pub capabilities: Vec<String>,
    /// Agent-specific configuration schema (JSON Schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<serde_json::Value>,
    /// API endpoints provided by this agent
    pub endpoints: Vec<AgentEndpoint>,
}

impl AgentMetadata {
    /// Create new agent metadata
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        agent_type: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            agent_type: agent_type.into(),
            description: description.into(),
            version: AgentVersion::default(),
            capabilities: Vec::new(),
            config_schema: None,
            endpoints: Vec::new(),
        }
    }

    /// Set version
    #[must_use]
    pub fn with_version(mut self, version: AgentVersion) -> Self {
        self.version = version;
        self
    }

    /// Add capability
    #[must_use]
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.push(capability.into());
        self
    }

    /// Add multiple capabilities
    #[must_use]
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities.extend(capabilities);
        self
    }

    /// Add endpoint
    #[must_use]
    pub fn with_endpoint(mut self, endpoint: AgentEndpoint) -> Self {
        self.endpoints.push(endpoint);
        self
    }
}

/// Agent endpoint description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEndpoint {
    /// HTTP method
    pub method: String,
    /// Path pattern
    pub path: String,
    /// Description
    pub description: String,
}

impl AgentEndpoint {
    /// Create a new endpoint
    #[must_use]
    pub fn new(method: impl Into<String>, path: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            path: path.into(),
            description: description.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_display() {
        let v = AgentVersion::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");

        let v_build = v.with_build("abc123");
        assert_eq!(v_build.to_string(), "1.2.3+abc123");
    }

    #[test]
    fn test_agent_status() {
        let status = AgentStatus::new("agent-1", "inference-routing")
            .with_health(AgentHealth::Healthy)
            .ready();

        assert_eq!(status.agent_id, "agent-1");
        assert_eq!(status.health, AgentHealth::Healthy);
        assert!(status.ready);
        assert_eq!(status.success_rate(), 1.0);
    }

    #[test]
    fn test_agent_metadata() {
        let metadata = AgentMetadata::new(
            "inference-routing",
            "InferenceRoutingAgent",
            "Routes inference requests to optimal providers",
        )
        .with_version(AgentVersion::new(0, 1, 0))
        .with_capability("routing".to_string())
        .with_capability("inspection".to_string())
        .with_endpoint(AgentEndpoint::new("POST", "/agents/route", "Route a request"));

        assert_eq!(metadata.capabilities.len(), 2);
        assert_eq!(metadata.endpoints.len(), 1);
    }
}
