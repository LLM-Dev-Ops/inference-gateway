//! Execution span types for the Agentics execution system.
//!
//! This module defines the hierarchical span model used to instrument
//! this repository as a Foundational Execution Unit. Every externally-invoked
//! operation must produce a span tree:
//!
//! ```text
//! Core (caller)
//!   └─ Repo (this repo)
//!       └─ Agent (one or more)
//! ```
//!
//! The [`ExecutionCollector`] manages span lifecycle and enforces the invariant
//! that at least one agent span must exist for a successful execution.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// The type of execution span in the hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanType {
    /// Span created by the external Core orchestrator.
    Core,
    /// Span representing this repository's execution boundary.
    Repo,
    /// Span representing an individual agent's execution.
    Agent,
}

/// Status of an execution span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanStatus {
    /// Execution is currently in progress.
    InProgress,
    /// Execution completed successfully.
    Succeeded,
    /// Execution failed.
    Failed,
}

/// A single execution span in the hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSpan {
    /// Unique identifier for this span.
    pub span_id: Uuid,
    /// ID of the parent span (repo span for agents, core span for repo).
    pub parent_span_id: Uuid,
    /// Type of this span in the hierarchy.
    pub span_type: SpanType,
    /// Current status of the span.
    pub status: SpanStatus,
    /// Name of the entity (repo name or agent name).
    pub name: String,
    /// When execution started.
    pub start_time: DateTime<Utc>,
    /// When execution ended (None if still in progress).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,
    /// Artifacts produced during this span.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<SpanArtifact>,
    /// Reason for failure (only set when status is Failed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    /// Additional metadata attached to this span.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// An artifact produced during execution and attached to a span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanArtifact {
    /// Type of artifact (e.g. "decision_event", "metrics", "report").
    pub artifact_type: String,
    /// Stable reference for the artifact (ID, URI, hash, or filename).
    pub reference: String,
    /// The artifact data.
    pub data: serde_json::Value,
    /// When the artifact was produced.
    pub timestamp: DateTime<Utc>,
}

/// Execution context provided by the Core orchestrator.
///
/// This must be present on every externally-invoked operation.
/// The `parent_span_id` links this repo's execution to the Core's span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Unique identifier for the overall execution.
    pub execution_id: Uuid,
    /// Span ID from the Core that is the parent of this repo's span.
    pub parent_span_id: Uuid,
}

/// The output contract for an instrumented execution.
///
/// Contains the repo-level span, all agent-level spans, artifacts,
/// and the operation result. This is JSON-serializable and forms
/// the append-only, causally-ordered span tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionOutput<T: Serialize> {
    /// The execution ID from the incoming context.
    pub execution_id: Uuid,
    /// The repo-level span for this execution.
    pub repo_span: ExecutionSpan,
    /// All agent-level spans nested under the repo span.
    pub agent_spans: Vec<ExecutionSpan>,
    /// The operation result (None on failure).
    pub result: Option<T>,
    /// Whether execution succeeded.
    pub success: bool,
}

/// Collects execution spans during a request lifecycle.
///
/// Create one at the start of each externally-invoked operation,
/// start/end agent spans as agents execute, attach artifacts,
/// then finalize to produce the [`ExecutionOutput`].
///
/// # Enforcement
///
/// [`finalize_success`](Self::finalize_success) will automatically convert
/// to a failure if no agent spans were emitted, enforcing the invariant
/// that execution without agent spans is invalid.
pub struct ExecutionCollector {
    execution_id: Uuid,
    repo_span: ExecutionSpan,
    agent_spans: Vec<ExecutionSpan>,
}

impl ExecutionCollector {
    /// Create a new collector, initializing the repo-level span.
    ///
    /// The repo span's `parent_span_id` is set to the Core's span ID
    /// from the execution context.
    #[must_use]
    pub fn new(ctx: &ExecutionContext, repo_name: &str) -> Self {
        let repo_span = ExecutionSpan {
            span_id: Uuid::new_v4(),
            parent_span_id: ctx.parent_span_id,
            span_type: SpanType::Repo,
            status: SpanStatus::InProgress,
            name: repo_name.to_string(),
            start_time: Utc::now(),
            end_time: None,
            artifacts: Vec::new(),
            failure_reason: None,
            metadata: HashMap::new(),
        };

        Self {
            execution_id: ctx.execution_id,
            repo_span,
            agent_spans: Vec::new(),
        }
    }

    /// Get the repo span ID (for attaching repo-level artifacts).
    #[must_use]
    pub fn repo_span_id(&self) -> Uuid {
        self.repo_span.span_id
    }

    /// Start a new agent-level span under the repo span.
    ///
    /// Returns the new span's ID for later use with
    /// [`end_agent_span`](Self::end_agent_span) and
    /// [`attach_artifact`](Self::attach_artifact).
    pub fn start_agent_span(&mut self, agent_name: &str) -> Uuid {
        let span_id = Uuid::new_v4();
        let span = ExecutionSpan {
            span_id,
            parent_span_id: self.repo_span.span_id,
            span_type: SpanType::Agent,
            status: SpanStatus::InProgress,
            name: agent_name.to_string(),
            start_time: Utc::now(),
            end_time: None,
            artifacts: Vec::new(),
            failure_reason: None,
            metadata: HashMap::new(),
        };
        self.agent_spans.push(span);
        span_id
    }

    /// End an agent span, setting its status and optional failure reason.
    pub fn end_agent_span(
        &mut self,
        span_id: Uuid,
        status: SpanStatus,
        failure_reason: Option<String>,
    ) {
        if let Some(span) = self.agent_spans.iter_mut().find(|s| s.span_id == span_id) {
            span.end_time = Some(Utc::now());
            span.status = status;
            span.failure_reason = failure_reason;
        }
    }

    /// Attach an artifact to a specific agent span.
    pub fn attach_artifact(&mut self, span_id: Uuid, artifact: SpanArtifact) {
        if let Some(span) = self.agent_spans.iter_mut().find(|s| s.span_id == span_id) {
            span.artifacts.push(artifact);
        }
    }

    /// Attach an artifact to the repo-level span.
    pub fn attach_repo_artifact(&mut self, artifact: SpanArtifact) {
        self.repo_span.artifacts.push(artifact);
    }

    /// Add metadata to the repo span.
    pub fn set_repo_metadata(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.repo_span.metadata.insert(key.into(), value);
    }

    /// Returns the number of agent spans collected so far.
    #[must_use]
    pub fn agent_span_count(&self) -> usize {
        self.agent_spans.len()
    }

    /// Finalize the execution as successful, producing the output.
    ///
    /// # Enforcement
    ///
    /// If no agent spans were emitted, this automatically converts to
    /// a failure with reason "No agent spans emitted during execution".
    /// This enforces the invariant that execution without agent proof is invalid.
    pub fn finalize_success<T: Serialize>(mut self, result: T) -> ExecutionOutput<T> {
        if self.agent_spans.is_empty() {
            return self.finalize_failure("No agent spans emitted during execution");
        }

        self.repo_span.end_time = Some(Utc::now());
        self.repo_span.status = SpanStatus::Succeeded;

        ExecutionOutput {
            execution_id: self.execution_id,
            repo_span: self.repo_span,
            agent_spans: self.agent_spans,
            result: Some(result),
            success: true,
        }
    }

    /// Finalize the execution as failed, producing the output.
    ///
    /// The repo span is marked as Failed with the given reason.
    /// All emitted spans (including any agent spans) are still returned.
    pub fn finalize_failure<T: Serialize>(mut self, reason: &str) -> ExecutionOutput<T> {
        self.repo_span.end_time = Some(Utc::now());
        self.repo_span.status = SpanStatus::Failed;
        self.repo_span.failure_reason = Some(reason.to_string());

        ExecutionOutput {
            execution_id: self.execution_id,
            repo_span: self.repo_span,
            agent_spans: self.agent_spans,
            result: None,
            success: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> ExecutionContext {
        ExecutionContext {
            execution_id: Uuid::new_v4(),
            parent_span_id: Uuid::new_v4(),
        }
    }

    #[test]
    fn test_collector_creates_repo_span() {
        let ctx = test_context();
        let collector = ExecutionCollector::new(&ctx, "test-repo");

        assert_eq!(collector.repo_span.span_type, SpanType::Repo);
        assert_eq!(collector.repo_span.name, "test-repo");
        assert_eq!(collector.repo_span.parent_span_id, ctx.parent_span_id);
        assert_eq!(collector.repo_span.status, SpanStatus::InProgress);
        assert!(collector.repo_span.end_time.is_none());
    }

    #[test]
    fn test_agent_span_lifecycle() {
        let ctx = test_context();
        let mut collector = ExecutionCollector::new(&ctx, "test-repo");

        let agent_span_id = collector.start_agent_span("test-agent");
        assert_eq!(collector.agent_span_count(), 1);

        // Verify agent span is parented to repo
        let agent_span = &collector.agent_spans[0];
        assert_eq!(agent_span.span_type, SpanType::Agent);
        assert_eq!(agent_span.name, "test-agent");
        assert_eq!(agent_span.parent_span_id, collector.repo_span.span_id);
        assert_eq!(agent_span.status, SpanStatus::InProgress);

        collector.end_agent_span(agent_span_id, SpanStatus::Succeeded, None);

        let agent_span = &collector.agent_spans[0];
        assert_eq!(agent_span.status, SpanStatus::Succeeded);
        assert!(agent_span.end_time.is_some());
        assert!(agent_span.failure_reason.is_none());
    }

    #[test]
    fn test_artifact_attachment() {
        let ctx = test_context();
        let mut collector = ExecutionCollector::new(&ctx, "test-repo");

        let agent_span_id = collector.start_agent_span("test-agent");

        let artifact = SpanArtifact {
            artifact_type: "decision_event".to_string(),
            reference: "evt-123".to_string(),
            data: serde_json::json!({"provider": "openai"}),
            timestamp: Utc::now(),
        };

        collector.attach_artifact(agent_span_id, artifact);
        assert_eq!(collector.agent_spans[0].artifacts.len(), 1);
        assert_eq!(
            collector.agent_spans[0].artifacts[0].artifact_type,
            "decision_event"
        );
    }

    #[test]
    fn test_finalize_success_with_agent_spans() {
        let ctx = test_context();
        let mut collector = ExecutionCollector::new(&ctx, "test-repo");

        let span_id = collector.start_agent_span("test-agent");
        collector.end_agent_span(span_id, SpanStatus::Succeeded, None);

        let output = collector.finalize_success("result-data");

        assert!(output.success);
        assert_eq!(output.result, Some("result-data"));
        assert_eq!(output.repo_span.status, SpanStatus::Succeeded);
        assert!(output.repo_span.end_time.is_some());
        assert_eq!(output.agent_spans.len(), 1);
        assert_eq!(output.execution_id, ctx.execution_id);
    }

    #[test]
    fn test_finalize_success_without_agent_spans_becomes_failure() {
        let ctx = test_context();
        let collector = ExecutionCollector::new(&ctx, "test-repo");

        let output: ExecutionOutput<&str> = collector.finalize_success("result-data");

        // Must be converted to failure
        assert!(!output.success);
        assert!(output.result.is_none());
        assert_eq!(output.repo_span.status, SpanStatus::Failed);
        assert_eq!(
            output.repo_span.failure_reason.as_deref(),
            Some("No agent spans emitted during execution")
        );
    }

    #[test]
    fn test_finalize_failure_preserves_spans() {
        let ctx = test_context();
        let mut collector = ExecutionCollector::new(&ctx, "test-repo");

        let span_id = collector.start_agent_span("test-agent");
        collector.end_agent_span(span_id, SpanStatus::Failed, Some("agent error".to_string()));

        let output: ExecutionOutput<&str> = collector.finalize_failure("overall failure");

        assert!(!output.success);
        assert!(output.result.is_none());
        assert_eq!(output.repo_span.status, SpanStatus::Failed);
        assert_eq!(
            output.repo_span.failure_reason.as_deref(),
            Some("overall failure")
        );
        // Agent spans are still returned
        assert_eq!(output.agent_spans.len(), 1);
        assert_eq!(output.agent_spans[0].status, SpanStatus::Failed);
    }

    #[test]
    fn test_multiple_agent_spans() {
        let ctx = test_context();
        let mut collector = ExecutionCollector::new(&ctx, "test-repo");

        let span1 = collector.start_agent_span("routing-agent");
        let span2 = collector.start_agent_span("provider-agent");

        collector.end_agent_span(span1, SpanStatus::Succeeded, None);
        collector.end_agent_span(span2, SpanStatus::Succeeded, None);

        let output = collector.finalize_success("ok");

        assert!(output.success);
        assert_eq!(output.agent_spans.len(), 2);
        // All agent spans are parented to the repo span
        for span in &output.agent_spans {
            assert_eq!(span.parent_span_id, output.repo_span.span_id);
            assert_eq!(span.span_type, SpanType::Agent);
        }
    }

    #[test]
    fn test_causal_ordering_via_parent_ids() {
        let ctx = test_context();
        let mut collector = ExecutionCollector::new(&ctx, "test-repo");

        let span_id = collector.start_agent_span("agent");
        collector.end_agent_span(span_id, SpanStatus::Succeeded, None);

        let output = collector.finalize_success("ok");

        // Core -> Repo: repo's parent is the core's span
        assert_eq!(output.repo_span.parent_span_id, ctx.parent_span_id);
        // Repo -> Agent: agent's parent is the repo's span
        assert_eq!(
            output.agent_spans[0].parent_span_id,
            output.repo_span.span_id
        );
    }

    #[test]
    fn test_execution_output_serialization() {
        let ctx = test_context();
        let mut collector = ExecutionCollector::new(&ctx, "test-repo");

        let span_id = collector.start_agent_span("agent");
        collector.end_agent_span(span_id, SpanStatus::Succeeded, None);

        let output = collector.finalize_success(serde_json::json!({"key": "value"}));

        // Must be JSON-serializable without loss
        let json = serde_json::to_string(&output).expect("serialization should succeed");
        let _: serde_json::Value =
            serde_json::from_str(&json).expect("deserialization should succeed");
    }

    #[test]
    fn test_repo_artifact_attachment() {
        let ctx = test_context();
        let mut collector = ExecutionCollector::new(&ctx, "test-repo");

        collector.attach_repo_artifact(SpanArtifact {
            artifact_type: "metrics".to_string(),
            reference: "metrics-snapshot".to_string(),
            data: serde_json::json!({"requests": 42}),
            timestamp: Utc::now(),
        });

        let span_id = collector.start_agent_span("agent");
        collector.end_agent_span(span_id, SpanStatus::Succeeded, None);

        let output = collector.finalize_success("ok");
        assert_eq!(output.repo_span.artifacts.len(), 1);
    }
}
