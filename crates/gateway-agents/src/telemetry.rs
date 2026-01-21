//! Telemetry emission for agent events.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info, instrument};

/// Telemetry event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TelemetryEvent {
    /// Routing decision made
    RoutingDecision {
        /// Unique execution reference
        execution_ref: String,
        /// Source model requested
        source_model: String,
        /// Selected provider
        provider: String,
        /// Target model (after transformation)
        target_model: String,
        /// Confidence score (0.0-1.0)
        confidence: f64,
        /// Decision latency in microseconds
        latency_us: u64,
        /// Timestamp
        timestamp: DateTime<Utc>,
        /// Additional metadata
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
    },
    /// Agent error occurred
    AgentError {
        /// Unique execution reference
        execution_ref: String,
        /// Error code
        error_code: String,
        /// Error message
        message: String,
        /// Timestamp
        timestamp: DateTime<Utc>,
    },
    /// Agent health check
    HealthCheck {
        /// Agent ID
        agent_id: String,
        /// Health status
        healthy: bool,
        /// Check latency in microseconds
        latency_us: u64,
        /// Timestamp
        timestamp: DateTime<Utc>,
    },
    /// Inspection performed
    Inspection {
        /// Agent ID
        agent_id: String,
        /// Number of providers
        provider_count: usize,
        /// Number of rules
        rule_count: usize,
        /// Timestamp
        timestamp: DateTime<Utc>,
    },
}

impl TelemetryEvent {
    /// Get the event type as a string
    #[must_use]
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::RoutingDecision { .. } => "routing_decision",
            Self::AgentError { .. } => "agent_error",
            Self::HealthCheck { .. } => "health_check",
            Self::Inspection { .. } => "inspection",
        }
    }

    /// Get the timestamp
    #[must_use]
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::RoutingDecision { timestamp, .. }
            | Self::AgentError { timestamp, .. }
            | Self::HealthCheck { timestamp, .. }
            | Self::Inspection { timestamp, .. } => *timestamp,
        }
    }
}

/// Telemetry emitter trait
#[async_trait::async_trait]
pub trait TelemetryEmitter: Send + Sync + std::fmt::Debug {
    /// Emit a telemetry event
    async fn emit(&self, event: TelemetryEvent);

    /// Flush any buffered events
    async fn flush(&self);
}

/// Default telemetry emitter that logs events via tracing
#[derive(Debug, Clone, Default)]
pub struct TracingTelemetryEmitter {
    /// Namespace for events
    namespace: String,
}

impl TracingTelemetryEmitter {
    /// Create a new tracing telemetry emitter
    #[must_use]
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
        }
    }
}

#[async_trait::async_trait]
impl TelemetryEmitter for TracingTelemetryEmitter {
    #[instrument(skip(self), fields(namespace = %self.namespace))]
    async fn emit(&self, event: TelemetryEvent) {
        match &event {
            TelemetryEvent::RoutingDecision {
                execution_ref,
                source_model,
                provider,
                target_model,
                confidence,
                latency_us,
                ..
            } => {
                info!(
                    execution_ref = %execution_ref,
                    source_model = %source_model,
                    provider = %provider,
                    target_model = %target_model,
                    confidence = %confidence,
                    latency_us = %latency_us,
                    "Routing decision made"
                );
            }
            TelemetryEvent::AgentError {
                execution_ref,
                error_code,
                message,
                ..
            } => {
                tracing::error!(
                    execution_ref = %execution_ref,
                    error_code = %error_code,
                    message = %message,
                    "Agent error"
                );
            }
            TelemetryEvent::HealthCheck {
                agent_id,
                healthy,
                latency_us,
                ..
            } => {
                debug!(
                    agent_id = %agent_id,
                    healthy = %healthy,
                    latency_us = %latency_us,
                    "Health check"
                );
            }
            TelemetryEvent::Inspection {
                agent_id,
                provider_count,
                rule_count,
                ..
            } => {
                debug!(
                    agent_id = %agent_id,
                    provider_count = %provider_count,
                    rule_count = %rule_count,
                    "Inspection performed"
                );
            }
        }
    }

    async fn flush(&self) {
        // Tracing emitter doesn't buffer
    }
}

/// Composite telemetry emitter that sends to multiple emitters
#[derive(Debug)]
pub struct CompositeTelemetryEmitter {
    emitters: Vec<Arc<dyn TelemetryEmitter>>,
}

impl CompositeTelemetryEmitter {
    /// Create a new composite emitter
    #[must_use]
    pub fn new(emitters: Vec<Arc<dyn TelemetryEmitter>>) -> Self {
        Self { emitters }
    }

    /// Add an emitter
    pub fn add(&mut self, emitter: Arc<dyn TelemetryEmitter>) {
        self.emitters.push(emitter);
    }
}

#[async_trait::async_trait]
impl TelemetryEmitter for CompositeTelemetryEmitter {
    async fn emit(&self, event: TelemetryEvent) {
        for emitter in &self.emitters {
            emitter.emit(event.clone()).await;
        }
    }

    async fn flush(&self) {
        for emitter in &self.emitters {
            emitter.flush().await;
        }
    }
}

/// No-op telemetry emitter for testing
#[derive(Debug, Clone, Default)]
pub struct NoOpTelemetryEmitter;

#[async_trait::async_trait]
impl TelemetryEmitter for NoOpTelemetryEmitter {
    async fn emit(&self, _event: TelemetryEvent) {
        // No-op
    }

    async fn flush(&self) {
        // No-op
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type() {
        let event = TelemetryEvent::RoutingDecision {
            execution_ref: "test-123".to_string(),
            source_model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            target_model: "gpt-4".to_string(),
            confidence: 0.95,
            latency_us: 100,
            timestamp: Utc::now(),
            metadata: None,
        };

        assert_eq!(event.event_type(), "routing_decision");
    }

    #[tokio::test]
    async fn test_tracing_emitter() {
        let emitter = TracingTelemetryEmitter::new("test");

        let event = TelemetryEvent::RoutingDecision {
            execution_ref: "test-123".to_string(),
            source_model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            target_model: "gpt-4".to_string(),
            confidence: 0.95,
            latency_us: 100,
            timestamp: Utc::now(),
            metadata: None,
        };

        // Should not panic
        emitter.emit(event).await;
        emitter.flush().await;
    }

    #[tokio::test]
    async fn test_noop_emitter() {
        let emitter = NoOpTelemetryEmitter;

        let event = TelemetryEvent::AgentError {
            execution_ref: "test-123".to_string(),
            error_code: "TEST_ERROR".to_string(),
            message: "Test error".to_string(),
            timestamp: Utc::now(),
        };

        // Should not panic
        emitter.emit(event).await;
        emitter.flush().await;
    }
}
