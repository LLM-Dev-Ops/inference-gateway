//! RuVector Service Client Adapter for persistence.
//!
//! This adapter persists DecisionEvents and other data to ruvector-service,
//! which is backed by Google SQL (Postgres). The LLM-Inference-Gateway
//! NEVER connects directly to the database - all persistence happens via
//! ruvector-service client calls.

use crate::config::RuVectorConfig;
use crate::error::{IntegrationError, IntegrationResult};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info, instrument, warn};

/// DecisionEvent represents a decision made by the inference gateway.
///
/// These events are persisted to ruvector-service for audit trails,
/// analytics, and decision replay capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEvent {
    /// Unique event identifier
    pub id: String,
    /// Reference to the execution context (e.g., request ID)
    pub execution_ref: String,
    /// Agent or component that made the decision
    pub agent_id: String,
    /// Type of decision (e.g., "routing", "fallback", "policy")
    pub decision_type: String,
    /// The input that led to the decision
    pub input: serde_json::Value,
    /// The decision output/result
    pub output: serde_json::Value,
    /// Decision confidence score (0.0-1.0)
    pub confidence: Option<f32>,
    /// Latency in milliseconds for the decision
    pub latency_ms: Option<u64>,
    /// Whether the decision was successful
    pub success: bool,
    /// Error message if the decision failed
    pub error_message: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Timestamp of the decision
    pub timestamp: DateTime<Utc>,
}

impl DecisionEvent {
    /// Create a new DecisionEvent builder.
    pub fn builder() -> DecisionEventBuilder {
        DecisionEventBuilder::default()
    }
}

/// Builder for creating DecisionEvent instances.
#[derive(Debug, Default)]
pub struct DecisionEventBuilder {
    execution_ref: Option<String>,
    agent_id: Option<String>,
    decision_type: Option<String>,
    input: Option<serde_json::Value>,
    output: Option<serde_json::Value>,
    confidence: Option<f32>,
    latency_ms: Option<u64>,
    success: bool,
    error_message: Option<String>,
    metadata: HashMap<String, serde_json::Value>,
}

impl DecisionEventBuilder {
    /// Set the execution reference.
    pub fn execution_ref(mut self, execution_ref: impl Into<String>) -> Self {
        self.execution_ref = Some(execution_ref.into());
        self
    }

    /// Set the agent ID.
    pub fn agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// Set the decision type.
    pub fn decision_type(mut self, decision_type: impl Into<String>) -> Self {
        self.decision_type = Some(decision_type.into());
        self
    }

    /// Set the input data.
    pub fn input(mut self, input: serde_json::Value) -> Self {
        self.input = Some(input);
        self
    }

    /// Set the output data.
    pub fn output(mut self, output: serde_json::Value) -> Self {
        self.output = Some(output);
        self
    }

    /// Set the confidence score.
    pub fn confidence(mut self, confidence: f32) -> Self {
        self.confidence = Some(confidence);
        self
    }

    /// Set the latency in milliseconds.
    pub fn latency_ms(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    /// Set the success flag.
    pub fn success(mut self, success: bool) -> Self {
        self.success = success;
        self
    }

    /// Set the error message.
    pub fn error_message(mut self, error_message: impl Into<String>) -> Self {
        self.error_message = Some(error_message.into());
        self
    }

    /// Add metadata.
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Build the DecisionEvent.
    pub fn build(self) -> Result<DecisionEvent, &'static str> {
        Ok(DecisionEvent {
            id: uuid::Uuid::new_v4().to_string(),
            execution_ref: self.execution_ref.ok_or("execution_ref is required")?,
            agent_id: self.agent_id.ok_or("agent_id is required")?,
            decision_type: self.decision_type.ok_or("decision_type is required")?,
            input: self.input.unwrap_or(serde_json::Value::Null),
            output: self.output.unwrap_or(serde_json::Value::Null),
            confidence: self.confidence,
            latency_ms: self.latency_ms,
            success: self.success,
            error_message: self.error_message,
            metadata: self.metadata,
            timestamp: Utc::now(),
        })
    }
}

/// Query parameters for searching DecisionEvents.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventQuery {
    /// Filter by agent ID
    pub agent_id: Option<String>,
    /// Filter by decision type
    pub decision_type: Option<String>,
    /// Filter by execution reference
    pub execution_ref: Option<String>,
    /// Filter by time range (start, end)
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    /// Filter by success status
    pub success: Option<bool>,
    /// Maximum number of results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

impl EventQuery {
    /// Create a new EventQuery builder.
    pub fn builder() -> EventQueryBuilder {
        EventQueryBuilder::default()
    }
}

/// Builder for creating EventQuery instances.
#[derive(Debug, Default)]
pub struct EventQueryBuilder {
    query: EventQuery,
}

impl EventQueryBuilder {
    /// Filter by agent ID.
    pub fn agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.query.agent_id = Some(agent_id.into());
        self
    }

    /// Filter by decision type.
    pub fn decision_type(mut self, decision_type: impl Into<String>) -> Self {
        self.query.decision_type = Some(decision_type.into());
        self
    }

    /// Filter by execution reference.
    pub fn execution_ref(mut self, execution_ref: impl Into<String>) -> Self {
        self.query.execution_ref = Some(execution_ref.into());
        self
    }

    /// Filter by time range.
    pub fn time_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.query.time_range = Some((start, end));
        self
    }

    /// Filter by success status.
    pub fn success(mut self, success: bool) -> Self {
        self.query.success = Some(success);
        self
    }

    /// Set the maximum number of results.
    pub fn limit(mut self, limit: u32) -> Self {
        self.query.limit = Some(limit);
        self
    }

    /// Set the offset for pagination.
    pub fn offset(mut self, offset: u32) -> Self {
        self.query.offset = Some(offset);
        self
    }

    /// Build the EventQuery.
    pub fn build(self) -> EventQuery {
        self.query
    }
}

/// Response from persisting a DecisionEvent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistResponse {
    /// The ID of the persisted event
    pub id: String,
    /// Server-assigned version
    pub version: Option<u64>,
    /// Server timestamp
    pub server_timestamp: DateTime<Utc>,
}

/// Paginated response for event queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventsResponse {
    /// The events matching the query
    pub events: Vec<DecisionEvent>,
    /// Total count of matching events
    pub total_count: u64,
    /// Whether there are more results
    pub has_more: bool,
}

/// Trait for RuVector persistence operations.
#[async_trait]
pub trait RuVectorPersistence: Send + Sync {
    /// Persist a DecisionEvent to ruvector-service.
    async fn persist_decision_event(&self, event: &DecisionEvent) -> IntegrationResult<String>;

    /// Retrieve DecisionEvents by execution reference.
    async fn get_events_by_execution(
        &self,
        execution_ref: &str,
    ) -> IntegrationResult<Vec<DecisionEvent>>;

    /// Search DecisionEvents by criteria.
    async fn search_events(&self, query: &EventQuery) -> IntegrationResult<EventsResponse>;

    /// Delete a DecisionEvent by ID.
    async fn delete_event(&self, event_id: &str) -> IntegrationResult<bool>;

    /// Batch persist multiple DecisionEvents.
    async fn persist_batch(&self, events: &[DecisionEvent]) -> IntegrationResult<Vec<String>>;
}

/// Client adapter for ruvector-service persistence.
///
/// This adapter handles all communication with ruvector-service,
/// which is the only persistence layer for DecisionEvents.
/// The gateway NEVER connects directly to the database.
pub struct RuVectorClient {
    /// Configuration
    config: RuVectorConfig,
    /// HTTP client
    http_client: reqwest::Client,
}

impl RuVectorClient {
    /// Create a new RuVector client.
    pub fn new(config: RuVectorConfig) -> IntegrationResult<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(config.timeout)
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(config.pool_size as usize)
            .build()
            .map_err(|e| IntegrationError::ruvector(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            http_client,
        })
    }

    /// Check if the client is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the configured endpoint.
    pub fn endpoint(&self) -> Option<&str> {
        self.config.endpoint.as_deref()
    }

    /// Build the full URL for an API path.
    fn build_url(&self, path: &str) -> IntegrationResult<String> {
        let endpoint = self
            .config
            .endpoint
            .as_ref()
            .ok_or_else(|| IntegrationError::ruvector("No endpoint configured"))?;

        Ok(format!("{}{}", endpoint.trim_end_matches('/'), path))
    }

    /// Add authentication headers to a request builder.
    fn add_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.config.api_key {
            Some(key) => builder.header("Authorization", format!("Bearer {}", key)),
            None => builder,
        }
    }

    /// Execute an HTTP request with retry logic.
    async fn execute_with_retry<T, F, Fut>(&self, operation: F) -> IntegrationResult<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = IntegrationResult<T>>,
    {
        let mut last_error = None;
        let mut delay = Duration::from_millis(100);

        for attempt in 0..=self.config.retry_count {
            if attempt > 0 {
                debug!(attempt = attempt, delay_ms = delay.as_millis(), "Retrying request");
                tokio::time::sleep(delay).await;
                // Exponential backoff with jitter
                delay = std::cmp::min(
                    delay * 2 + Duration::from_millis(rand_jitter()),
                    Duration::from_secs(30),
                );
            }

            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if !is_retryable(&e) {
                        return Err(e);
                    }
                    warn!(attempt = attempt, error = %e, "Request failed, will retry");
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            IntegrationError::ruvector("All retry attempts exhausted")
        }))
    }

    /// Check service health.
    #[instrument(skip(self))]
    pub async fn health_check(&self) -> IntegrationResult<bool> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("ruvector".to_string()));
        }

        let url = self.build_url("/health")?;
        let response = self
            .add_auth(self.http_client.get(&url))
            .send()
            .await
            .map_err(|e| IntegrationError::ruvector(format!("Health check failed: {}", e)))?;

        Ok(response.status().is_success())
    }

    /// Phase 7 startup check - CRASHES if RuVector is unavailable.
    ///
    /// This method is called at service startup BEFORE any agent logic runs.
    /// For Phase 7+ repositories, RuVector is REQUIRED infrastructure.
    ///
    /// # Behavior
    /// - If `config.required` is false: returns Ok(()) immediately (non-Phase 7 mode)
    /// - If `config.required` is true and health check passes: returns Ok(())
    /// - If `config.required` is true and health check fails: PANICS (service crash)
    ///
    /// # Panics
    /// Panics with a fatal error message if RuVector is required but unavailable.
    /// There is NO degraded mode, NO fallback logic, NO silent success.
    #[instrument(skip(self), name = "phase7_startup_check")]
    pub async fn phase7_startup_check(&self) -> Result<(), IntegrationError> {
        // If not required, just return Ok - non-Phase 7 mode
        if !self.config.required {
            debug!(
                target: "phase7",
                ruvector_required = false,
                "RuVector not marked as required, skipping Phase 7 check"
            );
            return Ok(());
        }

        // Assert required flag for contract clarity
        assert!(
            self.config.required,
            "ruvector.required must be true for Phase 7"
        );

        info!(
            target: "phase7",
            endpoint = ?self.config.endpoint,
            "Performing Phase 7 RuVector startup check (REQUIRED mode)"
        );

        // Perform health check
        match self.health_check().await {
            Ok(true) => {
                info!(
                    target: "phase7",
                    ruvector = true,
                    endpoint = ?self.config.endpoint,
                    "RuVector health check PASSED - service may proceed"
                );
                Ok(())
            }
            Ok(false) => {
                error!(
                    target: "phase7",
                    endpoint = ?self.config.endpoint,
                    "RuVector health check returned false - ABORTING SERVICE"
                );
                // CRASH - no degraded mode
                panic!("FATAL: RuVector is REQUIRED but health check returned false. Service cannot start.");
            }
            Err(e) => {
                error!(
                    target: "phase7",
                    endpoint = ?self.config.endpoint,
                    error = %e,
                    "RuVector health check FAILED - ABORTING SERVICE"
                );
                // CRASH - no degraded mode
                panic!(
                    "FATAL: RuVector is REQUIRED but unavailable ({}). Service cannot start.",
                    e
                );
            }
        }
    }

    /// Runtime contract assertion for Phase 7.
    ///
    /// This method validates that the Phase 7 contract is being honored
    /// at runtime. Call this at critical points to ensure no code path
    /// bypasses the required RuVector integration.
    ///
    /// # Panics
    /// Panics if the Phase 7 contract is violated:
    /// - `config.required` must be true
    /// - `config.enabled` must be true
    #[inline]
    pub fn assert_phase7_contract(&self) {
        assert!(
            self.config.required,
            "Phase 7 contract violation: ruvector.required must be true"
        );
        assert!(
            self.is_enabled(),
            "Phase 7 contract violation: ruvector must be enabled"
        );
    }

    /// Check if this client is configured for Phase 7 required mode.
    #[inline]
    pub fn is_required(&self) -> bool {
        self.config.required
    }
}

#[async_trait]
impl RuVectorPersistence for RuVectorClient {
    #[instrument(skip(self, event), fields(event_id = %event.id, execution_ref = %event.execution_ref))]
    async fn persist_decision_event(&self, event: &DecisionEvent) -> IntegrationResult<String> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("ruvector".to_string()));
        }

        debug!(
            event_id = %event.id,
            execution_ref = %event.execution_ref,
            decision_type = %event.decision_type,
            "Persisting decision event to ruvector-service"
        );

        let url = self.build_url("/api/v1/events")?;

        self.execute_with_retry(|| async {
            let response = self
                .add_auth(self.http_client.post(&url))
                .json(event)
                .send()
                .await
                .map_err(|e| IntegrationError::ruvector(format!("Request failed: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(IntegrationError::ruvector(format!(
                    "Persist failed with status {}: {}",
                    status, body
                )));
            }

            let persist_response: PersistResponse = response
                .json()
                .await
                .map_err(|e| IntegrationError::ruvector(format!("Failed to parse response: {}", e)))?;

            debug!(
                event_id = %persist_response.id,
                "Successfully persisted decision event"
            );

            Ok(persist_response.id)
        })
        .await
    }

    #[instrument(skip(self), fields(execution_ref = %execution_ref))]
    async fn get_events_by_execution(
        &self,
        execution_ref: &str,
    ) -> IntegrationResult<Vec<DecisionEvent>> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("ruvector".to_string()));
        }

        debug!(
            execution_ref = %execution_ref,
            "Retrieving events by execution reference"
        );

        let url = self.build_url(&format!("/api/v1/events/by-execution/{}", execution_ref))?;

        self.execute_with_retry(|| async {
            let response = self
                .add_auth(self.http_client.get(&url))
                .send()
                .await
                .map_err(|e| IntegrationError::ruvector(format!("Request failed: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(IntegrationError::ruvector(format!(
                    "Get events failed with status {}: {}",
                    status, body
                )));
            }

            let events: Vec<DecisionEvent> = response
                .json()
                .await
                .map_err(|e| IntegrationError::ruvector(format!("Failed to parse response: {}", e)))?;

            debug!(count = events.len(), "Retrieved events by execution");

            Ok(events)
        })
        .await
    }

    #[instrument(skip(self, query))]
    async fn search_events(&self, query: &EventQuery) -> IntegrationResult<EventsResponse> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("ruvector".to_string()));
        }

        debug!(?query, "Searching decision events");

        let url = self.build_url("/api/v1/events/search")?;

        self.execute_with_retry(|| async {
            let response = self
                .add_auth(self.http_client.post(&url))
                .json(query)
                .send()
                .await
                .map_err(|e| IntegrationError::ruvector(format!("Request failed: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(IntegrationError::ruvector(format!(
                    "Search failed with status {}: {}",
                    status, body
                )));
            }

            let events_response: EventsResponse = response
                .json()
                .await
                .map_err(|e| IntegrationError::ruvector(format!("Failed to parse response: {}", e)))?;

            debug!(
                count = events_response.events.len(),
                total = events_response.total_count,
                "Search completed"
            );

            Ok(events_response)
        })
        .await
    }

    #[instrument(skip(self), fields(event_id = %event_id))]
    async fn delete_event(&self, event_id: &str) -> IntegrationResult<bool> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("ruvector".to_string()));
        }

        debug!(event_id = %event_id, "Deleting decision event");

        let url = self.build_url(&format!("/api/v1/events/{}", event_id))?;

        self.execute_with_retry(|| async {
            let response = self
                .add_auth(self.http_client.delete(&url))
                .send()
                .await
                .map_err(|e| IntegrationError::ruvector(format!("Request failed: {}", e)))?;

            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(false);
            }

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(IntegrationError::ruvector(format!(
                    "Delete failed with status {}: {}",
                    status, body
                )));
            }

            debug!(event_id = %event_id, "Successfully deleted event");
            Ok(true)
        })
        .await
    }

    #[instrument(skip(self, events), fields(count = events.len()))]
    async fn persist_batch(&self, events: &[DecisionEvent]) -> IntegrationResult<Vec<String>> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("ruvector".to_string()));
        }

        if events.is_empty() {
            return Ok(Vec::new());
        }

        debug!(count = events.len(), "Batch persisting decision events");

        let url = self.build_url("/api/v1/events/batch")?;

        self.execute_with_retry(|| async {
            let response = self
                .add_auth(self.http_client.post(&url))
                .json(events)
                .send()
                .await
                .map_err(|e| IntegrationError::ruvector(format!("Request failed: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(IntegrationError::ruvector(format!(
                    "Batch persist failed with status {}: {}",
                    status, body
                )));
            }

            let ids: Vec<String> = response
                .json()
                .await
                .map_err(|e| IntegrationError::ruvector(format!("Failed to parse response: {}", e)))?;

            debug!(count = ids.len(), "Successfully batch persisted events");

            Ok(ids)
        })
        .await
    }
}

impl std::fmt::Debug for RuVectorClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuVectorClient")
            .field("enabled", &self.config.enabled)
            .field("required", &self.config.required)
            .field("endpoint", &self.config.endpoint)
            .field("timeout", &self.config.timeout)
            .field("retry_count", &self.config.retry_count)
            .finish()
    }
}

/// Builder for `RuVectorClient`.
pub struct RuVectorClientBuilder {
    config: RuVectorConfig,
}

impl RuVectorClientBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: RuVectorConfig::default(),
        }
    }

    /// Set the configuration.
    pub fn config(mut self, config: RuVectorConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable the client.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Set the endpoint.
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.endpoint = Some(endpoint.into());
        self
    }

    /// Set the API key.
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.config.api_key = Some(api_key.into());
        self
    }

    /// Set the timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// Set the retry count.
    pub fn retry_count(mut self, retry_count: u32) -> Self {
        self.config.retry_count = retry_count;
        self
    }

    /// Set whether RuVector is required (Phase 7 enforcement).
    ///
    /// When set to true, the service will CRASH at startup if RuVector
    /// is unavailable. There is NO degraded mode.
    pub fn required(mut self, required: bool) -> Self {
        self.config.required = required;
        self
    }

    /// Build the client.
    pub fn build(self) -> IntegrationResult<RuVectorClient> {
        RuVectorClient::new(self.config)
    }
}

impl Default for RuVectorClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if an error is retryable.
fn is_retryable(error: &IntegrationError) -> bool {
    matches!(
        error,
        IntegrationError::Connection(_) | IntegrationError::Timeout(_)
    ) || matches!(error, IntegrationError::RuVector { retryable: true, .. })
}

/// Generate a random jitter value for exponential backoff.
fn rand_jitter() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0));
    hasher.finish() % 100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_event_builder() {
        let event = DecisionEvent::builder()
            .execution_ref("exec-123")
            .agent_id("router-agent")
            .decision_type("routing")
            .input(serde_json::json!({"model": "gpt-4"}))
            .output(serde_json::json!({"provider": "openai"}))
            .confidence(0.95)
            .latency_ms(42)
            .success(true)
            .metadata("source", serde_json::json!("gateway"))
            .build()
            .expect("should build event");

        assert_eq!(event.execution_ref, "exec-123");
        assert_eq!(event.agent_id, "router-agent");
        assert_eq!(event.decision_type, "routing");
        assert!(event.success);
        assert_eq!(event.confidence, Some(0.95));
    }

    #[test]
    fn test_decision_event_builder_missing_required() {
        let result = DecisionEvent::builder()
            .agent_id("test")
            .decision_type("test")
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_event_query_builder() {
        let query = EventQuery::builder()
            .agent_id("router-agent")
            .decision_type("routing")
            .success(true)
            .limit(100)
            .offset(0)
            .build();

        assert_eq!(query.agent_id, Some("router-agent".to_string()));
        assert_eq!(query.decision_type, Some("routing".to_string()));
        assert_eq!(query.success, Some(true));
        assert_eq!(query.limit, Some(100));
    }

    #[test]
    fn test_client_disabled_by_default() {
        let client = RuVectorClient::new(RuVectorConfig::default()).unwrap();
        assert!(!client.is_enabled());
    }

    #[test]
    fn test_client_builder() {
        let client = RuVectorClientBuilder::new()
            .enabled(true)
            .endpoint("http://localhost:8080")
            .api_key("test-key")
            .timeout(Duration::from_secs(60))
            .retry_count(5)
            .build()
            .expect("should build client");

        assert!(client.is_enabled());
        assert_eq!(client.endpoint(), Some("http://localhost:8080"));
    }

    #[tokio::test]
    async fn test_disabled_returns_not_enabled() {
        let client = RuVectorClient::new(RuVectorConfig::default()).unwrap();

        let event = DecisionEvent::builder()
            .execution_ref("exec-123")
            .agent_id("test")
            .decision_type("test")
            .success(true)
            .build()
            .unwrap();

        let result = client.persist_decision_event(&event).await;
        assert!(matches!(result, Err(IntegrationError::NotEnabled(_))));
    }

    #[test]
    fn test_required_defaults_to_false() {
        let config = RuVectorConfig::default();
        assert!(!config.required);
    }

    #[test]
    fn test_is_required() {
        let mut config = RuVectorConfig::default();
        config.required = true;
        let client = RuVectorClient::new(config).unwrap();
        assert!(client.is_required());
    }

    #[test]
    fn test_builder_required() {
        let client = RuVectorClientBuilder::new()
            .enabled(true)
            .required(true)
            .endpoint("http://localhost:8080")
            .build()
            .expect("should build client");

        assert!(client.is_required());
    }

    #[tokio::test]
    async fn test_phase7_startup_check_not_required_skips() {
        // When required=false, phase7_startup_check should return Ok immediately
        let client = RuVectorClient::new(RuVectorConfig::default()).unwrap();
        assert!(!client.is_required());

        let result = client.phase7_startup_check().await;
        assert!(result.is_ok());
    }

    #[test]
    #[should_panic(expected = "Phase 7 contract violation: ruvector.required must be true")]
    fn test_assert_phase7_contract_fails_when_not_required() {
        let client = RuVectorClient::new(RuVectorConfig::default()).unwrap();
        client.assert_phase7_contract();
    }

    #[test]
    #[should_panic(expected = "Phase 7 contract violation: ruvector must be enabled")]
    fn test_assert_phase7_contract_fails_when_not_enabled() {
        let mut config = RuVectorConfig::default();
        config.required = true;
        config.enabled = false;
        let client = RuVectorClient::new(config).unwrap();
        client.assert_phase7_contract();
    }

    #[test]
    fn test_assert_phase7_contract_passes_when_required_and_enabled() {
        let mut config = RuVectorConfig::default();
        config.required = true;
        config.enabled = true;
        config.endpoint = Some("http://localhost:8080".to_string());
        let client = RuVectorClient::new(config).unwrap();
        // This should not panic
        client.assert_phase7_contract();
    }
}
