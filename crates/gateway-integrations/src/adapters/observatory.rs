//! LLM-Observatory adapter for telemetry.
//!
//! This adapter emits and consumes telemetry traces, latency profiles,
//! and performance metrics via LLM-Observatory.

use crate::config::ObservatoryConfig;
use crate::error::{IntegrationError, IntegrationResult};
use crate::traits::{
    LatencyBreakdown, LatencyProfile, Metric, MetricType, ObservabilityEmitter,
    PerformanceFeedback, SpanEvent, SpanStatus, TraceSpan,
};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, instrument, warn};

/// Adapter for emitting and consuming telemetry via LLM-Observatory.
///
/// This is a thin wrapper that handles trace emission, metrics,
/// latency profiles, and performance feedback consumption.
pub struct ObservatoryAdapter {
    /// Configuration
    config: ObservatoryConfig,
    /// Pending traces buffer
    trace_buffer: Mutex<VecDeque<TraceSpan>>,
    /// Pending metrics buffer
    metrics_buffer: Mutex<VecDeque<Metric>>,
    /// Pending latency profiles buffer
    latency_buffer: Mutex<VecDeque<LatencyProfile>>,
}

impl ObservatoryAdapter {
    /// Create a new observatory adapter.
    pub fn new(config: ObservatoryConfig) -> Self {
        Self {
            config,
            trace_buffer: Mutex::new(VecDeque::with_capacity(100)),
            metrics_buffer: Mutex::new(VecDeque::with_capacity(100)),
            latency_buffer: Mutex::new(VecDeque::with_capacity(100)),
        }
    }

    /// Check if the adapter is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if trace emission is enabled.
    pub fn emits_traces(&self) -> bool {
        self.config.emit_traces
    }

    /// Check if metrics emission is enabled.
    pub fn emits_metrics(&self) -> bool {
        self.config.emit_metrics
    }

    /// Get the batch size.
    pub fn batch_size(&self) -> usize {
        self.config.batch_size
    }

    /// Check if buffer needs flushing.
    async fn should_flush(&self) -> bool {
        let traces = self.trace_buffer.lock().await;
        let metrics = self.metrics_buffer.lock().await;
        let latency = self.latency_buffer.lock().await;

        traces.len() >= self.config.batch_size
            || metrics.len() >= self.config.batch_size
            || latency.len() >= self.config.batch_size
    }
}

#[async_trait]
impl ObservabilityEmitter for ObservatoryAdapter {
    #[instrument(skip(self, trace), fields(trace_id = %trace.trace_id, operation = %trace.operation))]
    async fn emit_trace(&self, trace: TraceSpan) -> IntegrationResult<()> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("observatory".to_string()));
        }

        if !self.config.emit_traces {
            debug!("Trace emission is disabled");
            return Ok(());
        }

        debug!(
            trace_id = %trace.trace_id,
            span_id = %trace.span_id,
            operation = %trace.operation,
            "Buffering trace for observatory"
        );

        let mut buffer = self.trace_buffer.lock().await;
        buffer.push_back(trace);

        // Auto-flush if buffer is full
        if buffer.len() >= self.config.batch_size {
            drop(buffer); // Release lock before flush
            debug!("Trace buffer full, triggering flush");
            // Phase 2B: Actual flush to observatory would happen here
        }

        Ok(())
    }

    #[instrument(skip(self, metrics), fields(count = metrics.len()))]
    async fn emit_metrics(&self, metrics: Vec<Metric>) -> IntegrationResult<()> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("observatory".to_string()));
        }

        if !self.config.emit_metrics {
            debug!("Metrics emission is disabled");
            return Ok(());
        }

        debug!(count = metrics.len(), "Buffering metrics for observatory");

        let mut buffer = self.metrics_buffer.lock().await;
        for metric in metrics {
            buffer.push_back(metric);
        }

        if buffer.len() >= self.config.batch_size {
            drop(buffer);
            debug!("Metrics buffer full, triggering flush");
        }

        Ok(())
    }

    #[instrument(skip(self, profile), fields(request_id = %profile.request_id, provider = %profile.provider_id))]
    async fn emit_latency_profile(&self, profile: LatencyProfile) -> IntegrationResult<()> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("observatory".to_string()));
        }

        if !self.config.emit_latency_profiles {
            debug!("Latency profile emission is disabled");
            return Ok(());
        }

        debug!(
            request_id = %profile.request_id,
            total_ms = profile.total_ms,
            ttft_ms = ?profile.ttft_ms,
            "Buffering latency profile for observatory"
        );

        let mut buffer = self.latency_buffer.lock().await;
        buffer.push_back(profile);

        if buffer.len() >= self.config.batch_size {
            drop(buffer);
            debug!("Latency buffer full, triggering flush");
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn consume_performance_feedback(
        &self,
    ) -> IntegrationResult<Vec<PerformanceFeedback>> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("observatory".to_string()));
        }

        if !self.config.consume_performance_feedback {
            debug!("Performance feedback consumption is disabled");
            return Ok(Vec::new());
        }

        debug!("Consuming performance feedback from observatory");

        // Phase 2B: Performance feedback consumption interface ready.
        // Actual observatory client would fetch feedback here.

        Ok(Vec::new())
    }

    #[instrument(skip(self))]
    async fn flush(&self) -> IntegrationResult<()> {
        if !self.is_enabled() {
            return Err(IntegrationError::NotEnabled("observatory".to_string()));
        }

        debug!("Flushing telemetry buffers to observatory");

        let traces: Vec<TraceSpan> = {
            let mut buffer = self.trace_buffer.lock().await;
            buffer.drain(..).collect()
        };

        let metrics: Vec<Metric> = {
            let mut buffer = self.metrics_buffer.lock().await;
            buffer.drain(..).collect()
        };

        let latencies: Vec<LatencyProfile> = {
            let mut buffer = self.latency_buffer.lock().await;
            buffer.drain(..).collect()
        };

        debug!(
            traces = traces.len(),
            metrics = metrics.len(),
            latencies = latencies.len(),
            "Flushed telemetry buffers"
        );

        // Phase 2B: Actual flush to observatory service would happen here.

        Ok(())
    }
}

impl std::fmt::Debug for ObservatoryAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObservatoryAdapter")
            .field("enabled", &self.config.enabled)
            .field("emit_traces", &self.config.emit_traces)
            .field("emit_metrics", &self.config.emit_metrics)
            .field("batch_size", &self.config.batch_size)
            .finish()
    }
}

/// Builder for `ObservatoryAdapter`
pub struct ObservatoryAdapterBuilder {
    config: ObservatoryConfig,
}

impl ObservatoryAdapterBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: ObservatoryConfig::default(),
        }
    }

    /// Set the configuration.
    pub fn config(mut self, config: ObservatoryConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable the adapter.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Enable/disable trace emission.
    pub fn emit_traces(mut self, enabled: bool) -> Self {
        self.config.emit_traces = enabled;
        self
    }

    /// Enable/disable metrics emission.
    pub fn emit_metrics(mut self, enabled: bool) -> Self {
        self.config.emit_metrics = enabled;
        self
    }

    /// Set batch size.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.config.batch_size = size;
        self
    }

    /// Build the adapter.
    pub fn build(self) -> ObservatoryAdapter {
        ObservatoryAdapter::new(self.config)
    }
}

impl Default for ObservatoryAdapterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create a latency profile from timing data.
pub fn create_latency_profile(
    request_id: impl Into<String>,
    provider_id: impl Into<String>,
    model: impl Into<String>,
    total_ms: u64,
    ttft_ms: Option<u64>,
    queue_ms: u64,
    routing_ms: u64,
    provider_ms: u64,
    transform_ms: u64,
) -> LatencyProfile {
    LatencyProfile {
        request_id: request_id.into(),
        provider_id: provider_id.into(),
        model: model.into(),
        total_ms,
        ttft_ms,
        breakdown: LatencyBreakdown {
            queue_ms,
            routing_ms,
            provider_ms,
            transform_ms,
        },
        timestamp: chrono::Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_disabled_by_default() {
        let adapter = ObservatoryAdapter::new(ObservatoryConfig::default());
        assert!(!adapter.is_enabled());
    }

    #[test]
    fn test_adapter_builder() {
        let adapter = ObservatoryAdapterBuilder::new()
            .enabled(true)
            .emit_traces(true)
            .emit_metrics(false)
            .batch_size(50)
            .build();

        assert!(adapter.is_enabled());
        assert!(adapter.emits_traces());
        assert!(!adapter.emits_metrics());
        assert_eq!(adapter.batch_size(), 50);
    }

    #[tokio::test]
    async fn test_disabled_returns_not_enabled() {
        let adapter = ObservatoryAdapter::new(ObservatoryConfig::default());

        let trace = TraceSpan {
            trace_id: "test".to_string(),
            span_id: "span".to_string(),
            parent_span_id: None,
            operation: "test".to_string(),
            start_time: chrono::Utc::now(),
            end_time: None,
            duration_ms: None,
            status: SpanStatus::Ok,
            attributes: std::collections::HashMap::new(),
            events: Vec::new(),
        };

        let result = adapter.emit_trace(trace).await;
        assert!(matches!(result, Err(IntegrationError::NotEnabled(_))));
    }
}
