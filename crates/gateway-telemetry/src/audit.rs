//! Audit logging for compliance and security monitoring.
//!
//! Provides structured audit logging for:
//! - Request/response events
//! - Authentication events
//! - Configuration changes
//! - Security events
//! - Administrative actions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Audit event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    /// API request received
    RequestReceived,
    /// API response sent
    ResponseSent,
    /// Authentication successful
    AuthSuccess,
    /// Authentication failed
    AuthFailure,
    /// Rate limit exceeded
    RateLimitExceeded,
    /// Configuration changed
    ConfigChange,
    /// Provider added
    ProviderAdded,
    /// Provider removed
    ProviderRemoved,
    /// Provider health change
    ProviderHealthChange,
    /// Circuit breaker state change
    CircuitBreakerChange,
    /// Security event (suspicious activity)
    SecurityEvent,
    /// Administrative action
    AdminAction,
    /// System startup
    SystemStartup,
    /// System shutdown
    SystemShutdown,
}

impl std::fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestReceived => write!(f, "request_received"),
            Self::ResponseSent => write!(f, "response_sent"),
            Self::AuthSuccess => write!(f, "auth_success"),
            Self::AuthFailure => write!(f, "auth_failure"),
            Self::RateLimitExceeded => write!(f, "rate_limit_exceeded"),
            Self::ConfigChange => write!(f, "config_change"),
            Self::ProviderAdded => write!(f, "provider_added"),
            Self::ProviderRemoved => write!(f, "provider_removed"),
            Self::ProviderHealthChange => write!(f, "provider_health_change"),
            Self::CircuitBreakerChange => write!(f, "circuit_breaker_change"),
            Self::SecurityEvent => write!(f, "security_event"),
            Self::AdminAction => write!(f, "admin_action"),
            Self::SystemStartup => write!(f, "system_startup"),
            Self::SystemShutdown => write!(f, "system_shutdown"),
        }
    }
}

/// Audit event severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditSeverity {
    /// Informational event
    Info,
    /// Warning event
    Warning,
    /// Error event
    Error,
    /// Critical security event
    Critical,
}

impl std::fmt::Display for AuditSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Actor who performed the action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditActor {
    /// Actor type (user, service, system)
    pub actor_type: String,
    /// Actor identifier (user ID, service name, etc.)
    pub id: String,
    /// IP address if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    /// User agent if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

impl AuditActor {
    /// Create a system actor
    #[must_use]
    pub fn system() -> Self {
        Self {
            actor_type: "system".to_string(),
            id: "gateway".to_string(),
            ip_address: None,
            user_agent: None,
        }
    }

    /// Create a user actor
    #[must_use]
    pub fn user(id: impl Into<String>) -> Self {
        Self {
            actor_type: "user".to_string(),
            id: id.into(),
            ip_address: None,
            user_agent: None,
        }
    }

    /// Create a service actor
    #[must_use]
    pub fn service(name: impl Into<String>) -> Self {
        Self {
            actor_type: "service".to_string(),
            id: name.into(),
            ip_address: None,
            user_agent: None,
        }
    }

    /// Add IP address
    #[must_use]
    pub fn with_ip(mut self, ip: impl Into<String>) -> Self {
        self.ip_address = Some(ip.into());
        self
    }

    /// Add user agent
    #[must_use]
    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }
}

/// Resource affected by the action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResource {
    /// Resource type (request, provider, config, etc.)
    pub resource_type: String,
    /// Resource identifier
    pub id: String,
    /// Additional resource attributes
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub attributes: HashMap<String, String>,
}

impl AuditResource {
    /// Create a new resource
    #[must_use]
    pub fn new(resource_type: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            resource_type: resource_type.into(),
            id: id.into(),
            attributes: HashMap::new(),
        }
    }

    /// Add an attribute
    #[must_use]
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

/// Complete audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique event identifier
    pub id: String,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: AuditEventType,
    /// Event severity
    pub severity: AuditSeverity,
    /// Actor who performed the action
    pub actor: AuditActor,
    /// Resource affected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<AuditResource>,
    /// Event description
    pub description: String,
    /// Outcome (success/failure)
    pub outcome: AuditOutcome,
    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Request ID for correlation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Tenant ID for multi-tenancy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
}

/// Audit event outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditOutcome {
    /// Action succeeded
    Success,
    /// Action failed
    Failure,
    /// Action denied
    Denied,
    /// Unknown outcome
    Unknown,
}

impl std::fmt::Display for AuditOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "success"),
            Self::Failure => write!(f, "failure"),
            Self::Denied => write!(f, "denied"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Builder for audit events
#[derive(Debug)]
pub struct AuditEventBuilder {
    event_type: AuditEventType,
    severity: AuditSeverity,
    actor: Option<AuditActor>,
    resource: Option<AuditResource>,
    description: Option<String>,
    outcome: AuditOutcome,
    metadata: HashMap<String, serde_json::Value>,
    request_id: Option<String>,
    tenant_id: Option<String>,
}

impl AuditEventBuilder {
    /// Create a new audit event builder
    #[must_use]
    pub fn new(event_type: AuditEventType) -> Self {
        Self {
            event_type,
            severity: AuditSeverity::Info,
            actor: None,
            resource: None,
            description: None,
            outcome: AuditOutcome::Unknown,
            metadata: HashMap::new(),
            request_id: None,
            tenant_id: None,
        }
    }

    /// Set severity
    #[must_use]
    pub fn severity(mut self, severity: AuditSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Set actor
    #[must_use]
    pub fn actor(mut self, actor: AuditActor) -> Self {
        self.actor = Some(actor);
        self
    }

    /// Set resource
    #[must_use]
    pub fn resource(mut self, resource: AuditResource) -> Self {
        self.resource = Some(resource);
        self
    }

    /// Set description
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set outcome
    #[must_use]
    pub fn outcome(mut self, outcome: AuditOutcome) -> Self {
        self.outcome = outcome;
        self
    }

    /// Add metadata
    #[must_use]
    pub fn metadata(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.metadata.insert(key.into(), json_value);
        }
        self
    }

    /// Set request ID
    #[must_use]
    pub fn request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }

    /// Set tenant ID
    #[must_use]
    pub fn tenant_id(mut self, id: impl Into<String>) -> Self {
        self.tenant_id = Some(id.into());
        self
    }

    /// Build the audit event
    #[must_use]
    pub fn build(self) -> AuditEvent {
        AuditEvent {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            event_type: self.event_type,
            severity: self.severity,
            actor: self.actor.unwrap_or_else(AuditActor::system),
            resource: self.resource,
            description: self.description.unwrap_or_else(|| self.event_type.to_string()),
            outcome: self.outcome,
            metadata: self.metadata,
            request_id: self.request_id,
            tenant_id: self.tenant_id,
        }
    }
}

/// Audit log configuration
#[derive(Debug, Clone)]
pub struct AuditLogConfig {
    /// Whether audit logging is enabled
    pub enabled: bool,
    /// Log to stdout
    pub log_to_stdout: bool,
    /// Log to file
    pub log_to_file: bool,
    /// File path for audit logs
    pub file_path: Option<String>,
    /// Include request/response bodies (may contain sensitive data)
    pub include_bodies: bool,
    /// Redact sensitive fields
    pub redact_sensitive: bool,
    /// Maximum events to keep in memory buffer
    pub buffer_size: usize,
}

impl Default for AuditLogConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_to_stdout: true,
            log_to_file: false,
            file_path: None,
            include_bodies: false,
            redact_sensitive: true,
            buffer_size: 1000,
        }
    }
}

/// Audit logger for recording events
pub struct AuditLogger {
    config: AuditLogConfig,
    /// In-memory buffer for recent events
    buffer: Arc<RwLock<Vec<AuditEvent>>>,
}

impl AuditLogger {
    /// Create a new audit logger
    #[must_use]
    pub fn new(config: AuditLogConfig) -> Self {
        Self {
            config,
            buffer: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create with default configuration
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(AuditLogConfig::default())
    }

    /// Create a disabled audit logger
    #[must_use]
    pub fn disabled() -> Self {
        Self::new(AuditLogConfig {
            enabled: false,
            ..Default::default()
        })
    }

    /// Check if audit logging is enabled
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Log an audit event
    pub async fn log(&self, event: AuditEvent) {
        if !self.config.enabled {
            return;
        }

        // Redact sensitive data if configured
        let event = if self.config.redact_sensitive {
            self.redact_event(event)
        } else {
            event
        };

        // Log to stdout
        if self.config.log_to_stdout {
            self.log_to_stdout(&event);
        }

        // Store in buffer
        self.store_in_buffer(event.clone()).await;
    }

    /// Log an audit event (convenience method)
    pub async fn log_event(&self, builder: AuditEventBuilder) {
        self.log(builder.build()).await;
    }

    /// Log a request received event
    pub async fn log_request(
        &self,
        request_id: &str,
        model: &str,
        actor: AuditActor,
        tenant_id: Option<&str>,
    ) {
        let mut builder = AuditEventBuilder::new(AuditEventType::RequestReceived)
            .actor(actor)
            .resource(AuditResource::new("request", request_id).with_attribute("model", model))
            .description(format!("API request received for model {model}"))
            .outcome(AuditOutcome::Success)
            .request_id(request_id);

        if let Some(tenant) = tenant_id {
            builder = builder.tenant_id(tenant);
        }

        self.log(builder.build()).await;
    }

    /// Log a response sent event
    pub async fn log_response(
        &self,
        request_id: &str,
        status: u16,
        latency_ms: u64,
        success: bool,
    ) {
        let outcome = if success {
            AuditOutcome::Success
        } else {
            AuditOutcome::Failure
        };

        let severity = if success {
            AuditSeverity::Info
        } else {
            AuditSeverity::Warning
        };

        let event = AuditEventBuilder::new(AuditEventType::ResponseSent)
            .severity(severity)
            .resource(AuditResource::new("response", request_id))
            .description(format!("Response sent with status {status}"))
            .outcome(outcome)
            .request_id(request_id)
            .metadata("status_code", status)
            .metadata("latency_ms", latency_ms)
            .build();

        self.log(event).await;
    }

    /// Log an authentication event
    pub async fn log_auth(&self, actor: AuditActor, success: bool, reason: Option<&str>) {
        let event_type = if success {
            AuditEventType::AuthSuccess
        } else {
            AuditEventType::AuthFailure
        };

        let severity = if success {
            AuditSeverity::Info
        } else {
            AuditSeverity::Warning
        };

        let outcome = if success {
            AuditOutcome::Success
        } else {
            AuditOutcome::Denied
        };

        let mut builder = AuditEventBuilder::new(event_type)
            .severity(severity)
            .actor(actor)
            .outcome(outcome);

        if let Some(r) = reason {
            builder = builder.description(r).metadata("reason", r);
        } else {
            builder = builder.description(if success {
                "Authentication successful"
            } else {
                "Authentication failed"
            });
        }

        self.log(builder.build()).await;
    }

    /// Log a rate limit event
    pub async fn log_rate_limit(&self, actor: AuditActor, key: &str, limit: u32) {
        let event = AuditEventBuilder::new(AuditEventType::RateLimitExceeded)
            .severity(AuditSeverity::Warning)
            .actor(actor)
            .description(format!("Rate limit exceeded for key {key}"))
            .outcome(AuditOutcome::Denied)
            .metadata("rate_limit_key", key)
            .metadata("limit", limit)
            .build();

        self.log(event).await;
    }

    /// Log a security event
    pub async fn log_security(
        &self,
        actor: AuditActor,
        description: &str,
        severity: AuditSeverity,
    ) {
        let event = AuditEventBuilder::new(AuditEventType::SecurityEvent)
            .severity(severity)
            .actor(actor)
            .description(description)
            .outcome(AuditOutcome::Unknown)
            .build();

        self.log(event).await;
    }

    /// Log system startup
    pub async fn log_startup(&self, version: &str) {
        let event = AuditEventBuilder::new(AuditEventType::SystemStartup)
            .actor(AuditActor::system())
            .description(format!("System started, version {version}"))
            .outcome(AuditOutcome::Success)
            .metadata("version", version)
            .build();

        self.log(event).await;
    }

    /// Log system shutdown
    pub async fn log_shutdown(&self, reason: &str) {
        let event = AuditEventBuilder::new(AuditEventType::SystemShutdown)
            .actor(AuditActor::system())
            .description(format!("System shutting down: {reason}"))
            .outcome(AuditOutcome::Success)
            .metadata("reason", reason)
            .build();

        self.log(event).await;
    }

    /// Log provider health change
    pub async fn log_provider_health(&self, provider_id: &str, healthy: bool, reason: &str) {
        let severity = if healthy {
            AuditSeverity::Info
        } else {
            AuditSeverity::Warning
        };

        let event = AuditEventBuilder::new(AuditEventType::ProviderHealthChange)
            .severity(severity)
            .actor(AuditActor::system())
            .resource(AuditResource::new("provider", provider_id))
            .description(format!(
                "Provider {} health changed to {}",
                provider_id,
                if healthy { "healthy" } else { "unhealthy" }
            ))
            .outcome(AuditOutcome::Success)
            .metadata("healthy", healthy)
            .metadata("reason", reason)
            .build();

        self.log(event).await;
    }

    /// Log circuit breaker state change
    pub async fn log_circuit_breaker(&self, provider_id: &str, state: &str) {
        let severity = if state == "closed" {
            AuditSeverity::Info
        } else {
            AuditSeverity::Warning
        };

        let event = AuditEventBuilder::new(AuditEventType::CircuitBreakerChange)
            .severity(severity)
            .actor(AuditActor::system())
            .resource(AuditResource::new("circuit_breaker", provider_id))
            .description(format!(
                "Circuit breaker for {} changed to {}",
                provider_id, state
            ))
            .outcome(AuditOutcome::Success)
            .metadata("state", state)
            .build();

        self.log(event).await;
    }

    /// Get recent events from buffer
    pub async fn get_recent_events(&self, limit: usize) -> Vec<AuditEvent> {
        let buffer = self.buffer.read().await;
        buffer.iter().rev().take(limit).cloned().collect()
    }

    /// Get events filtered by type
    pub async fn get_events_by_type(
        &self,
        event_type: AuditEventType,
        limit: usize,
    ) -> Vec<AuditEvent> {
        let buffer = self.buffer.read().await;
        buffer
            .iter()
            .rev()
            .filter(|e| e.event_type == event_type)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Clear the event buffer
    pub async fn clear_buffer(&self) {
        let mut buffer = self.buffer.write().await;
        buffer.clear();
    }

    /// Redact sensitive data from event
    fn redact_event(&self, mut event: AuditEvent) -> AuditEvent {
        // Redact IP addresses (keep first two octets)
        if let Some(ref mut ip) = event.actor.ip_address {
            if let Some(idx) = ip.rfind('.') {
                if let Some(idx2) = ip[..idx].rfind('.') {
                    *ip = format!("{}.*.*", &ip[..idx2]);
                }
            }
        }

        // Redact sensitive metadata keys
        let sensitive_keys = ["api_key", "token", "password", "secret", "authorization"];
        for key in &sensitive_keys {
            if event.metadata.contains_key(*key) {
                event
                    .metadata
                    .insert((*key).to_string(), serde_json::json!("[REDACTED]"));
            }
        }

        event
    }

    /// Log event to stdout
    fn log_to_stdout(&self, event: &AuditEvent) {
        let json = serde_json::to_string(event).unwrap_or_else(|_| format!("{event:?}"));

        match event.severity {
            AuditSeverity::Critical | AuditSeverity::Error => {
                warn!(
                    target: "audit",
                    event_type = %event.event_type,
                    severity = %event.severity,
                    outcome = %event.outcome,
                    "{}",
                    json
                );
            }
            _ => {
                info!(
                    target: "audit",
                    event_type = %event.event_type,
                    severity = %event.severity,
                    outcome = %event.outcome,
                    "{}",
                    json
                );
            }
        }
    }

    /// Store event in memory buffer
    async fn store_in_buffer(&self, event: AuditEvent) {
        let mut buffer = self.buffer.write().await;

        // Remove oldest events if at capacity
        while buffer.len() >= self.config.buffer_size {
            buffer.remove(0);
        }

        buffer.push(event);
    }
}

/// Statistics about audit events
#[derive(Debug, Clone, Default)]
pub struct AuditStats {
    /// Total events logged
    pub total_events: u64,
    /// Events by type
    pub events_by_type: HashMap<String, u64>,
    /// Events by severity
    pub events_by_severity: HashMap<String, u64>,
    /// Events by outcome
    pub events_by_outcome: HashMap<String, u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_audit_event_builder() {
        let event = AuditEventBuilder::new(AuditEventType::RequestReceived)
            .severity(AuditSeverity::Info)
            .actor(AuditActor::user("user-123"))
            .resource(AuditResource::new("request", "req-456"))
            .description("Test request")
            .outcome(AuditOutcome::Success)
            .request_id("req-456")
            .metadata("model", "gpt-4")
            .build();

        assert_eq!(event.event_type, AuditEventType::RequestReceived);
        assert_eq!(event.severity, AuditSeverity::Info);
        assert_eq!(event.actor.id, "user-123");
        assert_eq!(event.outcome, AuditOutcome::Success);
        assert!(event.metadata.contains_key("model"));
    }

    #[tokio::test]
    async fn test_audit_logger_disabled() {
        let logger = AuditLogger::disabled();
        assert!(!logger.is_enabled());

        // Should not panic when logging to disabled logger
        let event = AuditEventBuilder::new(AuditEventType::RequestReceived).build();
        logger.log(event).await;

        let events = logger.get_recent_events(10).await;
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_audit_logger_buffer() {
        let logger = AuditLogger::new(AuditLogConfig {
            enabled: true,
            log_to_stdout: false, // Disable stdout for tests
            buffer_size: 5,
            ..Default::default()
        });

        // Log more events than buffer size
        for i in 0..10 {
            let event = AuditEventBuilder::new(AuditEventType::RequestReceived)
                .description(format!("Event {i}"))
                .build();
            logger.log(event).await;
        }

        let events = logger.get_recent_events(10).await;
        assert_eq!(events.len(), 5); // Only last 5 should be in buffer
    }

    #[tokio::test]
    async fn test_audit_logger_filter_by_type() {
        let logger = AuditLogger::new(AuditLogConfig {
            enabled: true,
            log_to_stdout: false,
            buffer_size: 100,
            ..Default::default()
        });

        // Log different event types
        logger
            .log(AuditEventBuilder::new(AuditEventType::RequestReceived).build())
            .await;
        logger
            .log(AuditEventBuilder::new(AuditEventType::ResponseSent).build())
            .await;
        logger
            .log(AuditEventBuilder::new(AuditEventType::RequestReceived).build())
            .await;
        logger
            .log(AuditEventBuilder::new(AuditEventType::AuthSuccess).build())
            .await;

        let request_events = logger
            .get_events_by_type(AuditEventType::RequestReceived, 10)
            .await;
        assert_eq!(request_events.len(), 2);

        let auth_events = logger
            .get_events_by_type(AuditEventType::AuthSuccess, 10)
            .await;
        assert_eq!(auth_events.len(), 1);
    }

    #[tokio::test]
    async fn test_audit_logger_log_request() {
        let logger = AuditLogger::new(AuditLogConfig {
            enabled: true,
            log_to_stdout: false,
            ..Default::default()
        });

        let actor = AuditActor::user("user-123").with_ip("192.168.1.1");
        logger
            .log_request("req-1", "gpt-4", actor, Some("tenant-1"))
            .await;

        let events = logger.get_recent_events(1).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, AuditEventType::RequestReceived);
        assert_eq!(events[0].tenant_id, Some("tenant-1".to_string()));
    }

    #[tokio::test]
    async fn test_audit_logger_redaction() {
        let logger = AuditLogger::new(AuditLogConfig {
            enabled: true,
            log_to_stdout: false,
            redact_sensitive: true,
            ..Default::default()
        });

        let event = AuditEventBuilder::new(AuditEventType::AuthFailure)
            .actor(AuditActor::user("user").with_ip("192.168.1.100"))
            .metadata("api_key", "sk-secret-key")
            .build();

        logger.log(event).await;

        let events = logger.get_recent_events(1).await;
        assert_eq!(events.len(), 1);

        // IP should be partially redacted
        let ip = events[0].actor.ip_address.as_ref().unwrap();
        assert!(ip.contains("*"));

        // API key should be redacted
        let api_key = events[0].metadata.get("api_key").unwrap();
        assert_eq!(api_key, "[REDACTED]");
    }

    #[tokio::test]
    async fn test_audit_actor_builders() {
        let system = AuditActor::system();
        assert_eq!(system.actor_type, "system");

        let user = AuditActor::user("user-1")
            .with_ip("10.0.0.1")
            .with_user_agent("Mozilla/5.0");
        assert_eq!(user.actor_type, "user");
        assert_eq!(user.id, "user-1");
        assert_eq!(user.ip_address, Some("10.0.0.1".to_string()));

        let service = AuditActor::service("api-gateway");
        assert_eq!(service.actor_type, "service");
    }

    #[tokio::test]
    async fn test_audit_resource_builder() {
        let resource = AuditResource::new("provider", "openai")
            .with_attribute("model", "gpt-4")
            .with_attribute("region", "us-east-1");

        assert_eq!(resource.resource_type, "provider");
        assert_eq!(resource.id, "openai");
        assert_eq!(resource.attributes.len(), 2);
    }

    #[test]
    fn test_event_type_display() {
        assert_eq!(AuditEventType::RequestReceived.to_string(), "request_received");
        assert_eq!(AuditEventType::AuthFailure.to_string(), "auth_failure");
    }

    #[test]
    fn test_audit_event_serialization() {
        let event = AuditEventBuilder::new(AuditEventType::RequestReceived)
            .actor(AuditActor::user("test"))
            .outcome(AuditOutcome::Success)
            .build();

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("request_received"));
        assert!(json.contains("success"));

        // Deserialize back
        let parsed: AuditEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type, AuditEventType::RequestReceived);
    }
}
