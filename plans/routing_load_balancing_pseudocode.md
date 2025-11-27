# Routing and Load Balancing System - Comprehensive Pseudocode

## Table of Contents
1. [Core Data Structures](#1-core-data-structures)
2. [Router Core](#2-router-core)
3. [Load Balancing Strategies](#3-load-balancing-strategies)
4. [Health-Aware Routing](#4-health-aware-routing)
5. [Routing Rules Engine](#5-routing-rules-engine)
6. [Failover Chain](#6-failover-chain)
7. [Request Context](#7-request-context)
8. [Performance Optimizations](#8-performance-optimizations)

---

## 1. Core Data Structures

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use std::collections::{HashMap, HashSet};
use dashmap::DashMap;
use crossbeam::atomic::AtomicCell;
use hdrhistogram::Histogram;
use parking_lot::RwLock;

// ============================================================================
// PROVIDER CANDIDATE
// ============================================================================

/// Represents a potential provider for routing
#[derive(Clone)]
struct ProviderCandidate {
    provider_id: String,
    provider_type: ProviderType,
    model_name: String,
    endpoint_url: String,

    // Capabilities
    capabilities: ProviderCapabilities,

    // Cost metrics (atomic for lock-free reads)
    cost_per_1k_input_tokens: AtomicU64,  // stored as micro-cents (1/1M of dollar)
    cost_per_1k_output_tokens: AtomicU64,

    // Real-time metrics (atomic counters)
    active_connections: AtomicU32,
    total_requests: AtomicU64,
    successful_requests: AtomicU64,
    failed_requests: AtomicU64,

    // Weight for load balancing (0-1000)
    weight: AtomicU32,

    // Health status
    health_score: Arc<AtomicCell<f64>>,  // 0.0-1.0
    is_available: Arc<AtomicCell<bool>>,

    // Rate limiting
    tokens_per_minute: Option<u32>,
    requests_per_minute: Option<u32>,

    // Latency tracking
    latency_histogram: Arc<RwLock<Histogram<u64>>>,  // microseconds

    // Circuit breaker state
    circuit_state: Arc<AtomicCell<CircuitState>>,
    failure_count: AtomicU32,
    last_failure_time: Arc<AtomicCell<Option<Instant>>>,

    // Metadata
    region: String,
    zone: Option<String>,
    tags: Arc<HashSet<String>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProviderType {
    OpenAI,
    Anthropic,
    AzureOpenAI,
    GoogleVertex,
    AWS_Bedrock,
    Cohere,
    Custom,
}

#[derive(Clone)]
struct ProviderCapabilities {
    max_tokens: u32,
    supports_streaming: bool,
    supports_function_calling: bool,
    supports_vision: bool,
    context_window: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    Closed,      // Normal operation
    Open,        // Circuit broken, rejecting requests
    HalfOpen,    // Testing if service recovered
}

// ============================================================================
// ROUTING TABLE
// ============================================================================

/// Fast lookup routing table with O(1) access
struct RoutingTable {
    // Model name -> List of providers
    model_providers: DashMap<String, Vec<Arc<ProviderCandidate>>>,

    // Provider ID -> Provider (for direct lookup)
    providers_by_id: DashMap<String, Arc<ProviderCandidate>>,

    // Tenant ID -> Custom routing rules
    tenant_overrides: DashMap<String, TenantRoutingConfig>,

    // Model aliases (e.g., "gpt-4" -> "gpt-4-0613")
    model_aliases: DashMap<String, String>,

    // Generation counter for cache invalidation
    generation: AtomicU64,
}

struct TenantRoutingConfig {
    allowed_providers: HashSet<String>,
    preferred_providers: Vec<String>,
    cost_limit_per_request: Option<u64>,  // micro-dollars
    default_strategy: LoadBalancingStrategy,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LoadBalancingStrategy {
    RoundRobin,
    WeightedRoundRobin,
    LeastConnections,
    LeastLatency,
    CostOptimized,
    Adaptive,
    Random,
}

// ============================================================================
// REQUEST AND RESPONSE TYPES
// ============================================================================

struct GatewayRequest {
    request_id: String,
    model: String,

    // Request payload
    messages: Vec<Message>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    stream: bool,

    // Authentication
    api_key_hash: String,
    tenant_id: Option<String>,
    user_id: Option<String>,

    // Routing hints
    preferred_provider: Option<String>,
    excluded_providers: HashSet<String>,

    // Budget constraints
    max_cost: Option<f64>,  // dollars
    max_latency: Option<Duration>,

    // Request metadata
    timestamp: Instant,
    priority: RequestPriority,
    retry_count: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RequestPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

struct Message {
    role: String,
    content: String,
}

struct SelectedProvider {
    candidate: Arc<ProviderCandidate>,
    routing_decision: RoutingDecision,
    estimated_cost: f64,
    expected_latency: Duration,
}

struct RoutingDecision {
    strategy_used: LoadBalancingStrategy,
    rule_matched: Option<String>,
    candidates_considered: usize,
    selection_time_us: u64,
}

struct RequestResult {
    provider_id: String,
    success: bool,
    latency: Duration,
    tokens_used: TokenUsage,
    error: Option<String>,
    status_code: Option<u16>,
}

struct TokenUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

// ============================================================================
// COST BUDGET
// ============================================================================

struct CostBudget {
    max_cost_per_request: f64,
    monthly_budget: Option<f64>,
    current_month_spend: Arc<AtomicU64>,  // stored as micro-dollars
}

impl CostBudget {
    fn can_afford(&self, estimated_cost: f64) -> bool {
        if estimated_cost > self.max_cost_per_request {
            return false;
        }

        if let Some(monthly_limit) = self.monthly_budget {
            let current_spend = self.current_month_spend.load(Ordering::Relaxed) as f64 / 1_000_000.0;
            if current_spend + estimated_cost > monthly_limit {
                return false;
            }
        }

        true
    }

    fn record_spend(&self, actual_cost: f64) {
        let micro_dollars = (actual_cost * 1_000_000.0) as u64;
        self.current_month_spend.fetch_add(micro_dollars, Ordering::Relaxed);
    }
}

// ============================================================================
// TRACE CONTEXT
// ============================================================================

struct TraceContext {
    trace_id: String,
    span_id: String,
    parent_span_id: Option<String>,
    baggage: HashMap<String, String>,
}
```

---

## 2. Router Core

```rust
// ============================================================================
// ROUTER IMPLEMENTATION
// ============================================================================

struct Router {
    // Core routing table
    routing_table: Arc<RoutingTable>,

    // Load balancing strategies (by strategy type)
    strategies: DashMap<LoadBalancingStrategy, Arc<dyn LoadBalancer>>,

    // Health-aware routing
    health_router: Arc<HealthAwareRouter>,

    // Rules engine
    rules_engine: Arc<RoutingRulesEngine>,

    // Metrics collection
    metrics: Arc<RouterMetrics>,

    // Configuration
    config: Arc<RouterConfig>,
}

struct RouterConfig {
    // Circuit breaker settings
    circuit_breaker_threshold: u32,  // failures before opening
    circuit_breaker_timeout: Duration,
    circuit_breaker_half_open_max_requests: u32,

    // Health check settings
    health_check_interval: Duration,
    min_health_score: f64,  // 0.0-1.0

    // Routing settings
    max_routing_time: Duration,  // hard deadline for routing decision
    enable_adaptive_routing: bool,

    // Failover settings
    max_failover_attempts: usize,
    failover_backoff_ms: u64,
}

struct RouterMetrics {
    total_routes: AtomicU64,
    successful_routes: AtomicU64,
    failed_routes: AtomicU64,
    routing_time_histogram: Arc<RwLock<Histogram<u64>>>,  // microseconds
}

impl Router {
    /// Creates a new router with default configuration
    fn new(config: RouterConfig) -> Self {
        let router = Self {
            routing_table: Arc::new(RoutingTable::new()),
            strategies: DashMap::new(),
            health_router: Arc::new(HealthAwareRouter::new()),
            rules_engine: Arc::new(RoutingRulesEngine::new()),
            metrics: Arc::new(RouterMetrics::new()),
            config: Arc::new(config),
        };

        // Register default strategies
        router.register_strategy(
            LoadBalancingStrategy::RoundRobin,
            Arc::new(RoundRobinLoadBalancer::new())
        );
        router.register_strategy(
            LoadBalancingStrategy::WeightedRoundRobin,
            Arc::new(WeightedRoundRobinLoadBalancer::new())
        );
        router.register_strategy(
            LoadBalancingStrategy::LeastConnections,
            Arc::new(LeastConnectionsLoadBalancer::new())
        );
        router.register_strategy(
            LoadBalancingStrategy::LeastLatency,
            Arc::new(LeastLatencyLoadBalancer::new())
        );
        router.register_strategy(
            LoadBalancingStrategy::CostOptimized,
            Arc::new(CostOptimizedLoadBalancer::new())
        );
        router.register_strategy(
            LoadBalancingStrategy::Adaptive,
            Arc::new(AdaptiveLoadBalancer::new())
        );

        router
    }

    /// Main routing function - returns selected provider in sub-millisecond time
    async fn route(
        &self,
        request: &GatewayRequest,
        context: &RoutingContext,
    ) -> Result<SelectedProvider, RoutingError> {
        let start_time = Instant::now();

        // Increment metrics
        self.metrics.total_routes.fetch_add(1, Ordering::Relaxed);

        // Step 1: Get candidate providers (O(1) lookup from routing table)
        let mut candidates = self.get_candidates(request)?;

        if candidates.is_empty() {
            return Err(RoutingError::NoProvidersAvailable);
        }

        // Step 2: Apply tenant-specific filters
        if let Some(tenant_id) = &context.tenant_id {
            candidates = self.apply_tenant_filters(candidates, tenant_id);
        }

        // Step 3: Apply routing rules (priority-based matching)
        let (candidates, strategy) = self.rules_engine.apply_rules(
            candidates,
            request,
            context,
        );

        if candidates.is_empty() {
            return Err(RoutingError::AllProvidersFiltered);
        }

        // Step 4: Filter by health status (remove unhealthy providers)
        let healthy_candidates = self.health_router.filter_healthy(&candidates);

        if healthy_candidates.is_empty() {
            // All providers unhealthy - try circuit breaker recovery
            let recovering = self.try_circuit_breaker_recovery(&candidates);
            if recovering.is_empty() {
                return Err(RoutingError::AllProvidersUnhealthy);
            }
            healthy_candidates = recovering;
        }

        // Step 5: Filter by cost budget
        let affordable_candidates = if let Some(budget) = &context.cost_budget {
            self.filter_by_cost(&healthy_candidates, request, budget)
        } else {
            healthy_candidates
        };

        if affordable_candidates.is_empty() {
            return Err(RoutingError::BudgetExceeded);
        }

        // Step 6: Select provider using load balancing strategy
        let load_balancer = self.get_strategy(strategy)?;

        let selected_candidate = load_balancer
            .select(&affordable_candidates, context)
            .ok_or(RoutingError::SelectionFailed)?;

        // Step 7: Calculate estimated cost and latency
        let estimated_cost = self.estimate_cost(selected_candidate, request);
        let expected_latency = self.estimate_latency(selected_candidate);

        // Step 8: Increment active connections counter (atomic)
        selected_candidate.active_connections.fetch_add(1, Ordering::Relaxed);

        let routing_time = start_time.elapsed();

        // Record metrics
        self.metrics.successful_routes.fetch_add(1, Ordering::Relaxed);
        self.metrics.routing_time_histogram.write()
            .record(routing_time.as_micros() as u64)
            .ok();

        Ok(SelectedProvider {
            candidate: Arc::clone(selected_candidate),
            routing_decision: RoutingDecision {
                strategy_used: strategy,
                rule_matched: None,  // TODO: track which rule matched
                candidates_considered: candidates.len(),
                selection_time_us: routing_time.as_micros() as u64,
            },
            estimated_cost,
            expected_latency,
        })
    }

    /// Get candidate providers for a request (O(1) lookup)
    fn get_candidates(&self, request: &GatewayRequest) -> Result<Vec<Arc<ProviderCandidate>>, RoutingError> {
        // Resolve model alias if needed
        let model = self.routing_table
            .model_aliases
            .get(&request.model)
            .map(|alias| alias.value().clone())
            .unwrap_or_else(|| request.model.clone());

        // Lookup providers for this model
        let providers = self.routing_table
            .model_providers
            .get(&model)
            .ok_or(RoutingError::ModelNotSupported)?;

        // Filter out explicitly excluded providers
        let candidates: Vec<Arc<ProviderCandidate>> = providers
            .value()
            .iter()
            .filter(|p| !request.excluded_providers.contains(&p.provider_id))
            .filter(|p| p.is_available.load())
            .cloned()
            .collect();

        // Apply preferred provider hint
        if let Some(preferred) = &request.preferred_provider {
            if let Some(provider) = candidates.iter().find(|p| &p.provider_id == preferred) {
                // Move preferred provider to front
                let mut reordered = vec![Arc::clone(provider)];
                reordered.extend(
                    candidates.into_iter().filter(|p| &p.provider_id != preferred)
                );
                return Ok(reordered);
            }
        }

        Ok(candidates)
    }

    /// Apply tenant-specific routing filters
    fn apply_tenant_filters(
        &self,
        candidates: Vec<Arc<ProviderCandidate>>,
        tenant_id: &str,
    ) -> Vec<Arc<ProviderCandidate>> {
        if let Some(tenant_config) = self.routing_table.tenant_overrides.get(tenant_id) {
            // Filter to only allowed providers
            let filtered: Vec<_> = candidates
                .into_iter()
                .filter(|p| tenant_config.allowed_providers.contains(&p.provider_id))
                .collect();

            // Reorder by tenant preferences
            if !tenant_config.preferred_providers.is_empty() {
                let mut reordered = Vec::new();

                // Add preferred providers first
                for preferred_id in &tenant_config.preferred_providers {
                    if let Some(provider) = filtered.iter().find(|p| &p.provider_id == preferred_id) {
                        reordered.push(Arc::clone(provider));
                    }
                }

                // Add remaining providers
                for provider in filtered {
                    if !tenant_config.preferred_providers.contains(&provider.provider_id) {
                        reordered.push(provider);
                    }
                }

                return reordered;
            }

            filtered
        } else {
            candidates
        }
    }

    /// Filter candidates by cost budget
    fn filter_by_cost(
        &self,
        candidates: &[Arc<ProviderCandidate>],
        request: &GatewayRequest,
        budget: &CostBudget,
    ) -> Vec<Arc<ProviderCandidate>> {
        candidates
            .iter()
            .filter(|candidate| {
                let estimated_cost = self.estimate_cost(candidate, request);
                budget.can_afford(estimated_cost)
            })
            .cloned()
            .collect()
    }

    /// Estimate cost for a request
    fn estimate_cost(&self, candidate: &ProviderCandidate, request: &GatewayRequest) -> f64 {
        // Estimate token counts (simple heuristic: ~4 chars per token)
        let prompt_tokens = request.messages
            .iter()
            .map(|m| m.content.len() / 4)
            .sum::<usize>() as u32;

        let completion_tokens = request.max_tokens.unwrap_or(1000);

        // Load atomic costs
        let input_cost_micro_cents = candidate.cost_per_1k_input_tokens.load(Ordering::Relaxed);
        let output_cost_micro_cents = candidate.cost_per_1k_output_tokens.load(Ordering::Relaxed);

        // Calculate total cost in dollars
        let input_cost = (prompt_tokens as f64 / 1000.0) * (input_cost_micro_cents as f64 / 100_000_000.0);
        let output_cost = (completion_tokens as f64 / 1000.0) * (output_cost_micro_cents as f64 / 100_000_000.0);

        input_cost + output_cost
    }

    /// Estimate latency based on historical data
    fn estimate_latency(&self, candidate: &ProviderCandidate) -> Duration {
        let histogram = candidate.latency_histogram.read();

        // Use P50 for expected latency
        let p50_micros = histogram.value_at_quantile(0.50);
        Duration::from_micros(p50_micros)
    }

    /// Try to recover providers from circuit breaker half-open state
    fn try_circuit_breaker_recovery(
        &self,
        candidates: &[Arc<ProviderCandidate>],
    ) -> Vec<Arc<ProviderCandidate>> {
        candidates
            .iter()
            .filter(|candidate| {
                let state = candidate.circuit_state.load();
                match state {
                    CircuitState::Closed => true,
                    CircuitState::HalfOpen => {
                        // Allow limited requests in half-open state
                        true
                    }
                    CircuitState::Open => {
                        // Check if timeout has elapsed
                        if let Some(last_failure) = candidate.last_failure_time.load() {
                            if last_failure.elapsed() > self.config.circuit_breaker_timeout {
                                // Transition to half-open
                                candidate.circuit_state.store(CircuitState::HalfOpen);
                                candidate.failure_count.store(0, Ordering::Relaxed);
                                return true;
                            }
                        }
                        false
                    }
                }
            })
            .cloned()
            .collect()
    }

    /// Get load balancing strategy
    fn get_strategy(&self, strategy: LoadBalancingStrategy) -> Result<Arc<dyn LoadBalancer>, RoutingError> {
        self.strategies
            .get(&strategy)
            .map(|s| Arc::clone(s.value()))
            .ok_or(RoutingError::StrategyNotFound)
    }

    /// Register a load balancing strategy
    fn register_strategy(&self, strategy: LoadBalancingStrategy, balancer: Arc<dyn LoadBalancer>) {
        self.strategies.insert(strategy, balancer);
    }

    /// Update routing table with new rules (lock-free update)
    fn update_routing_table(&self, updates: Vec<RoutingTableUpdate>) {
        for update in updates {
            match update {
                RoutingTableUpdate::AddProvider { model, provider } => {
                    self.routing_table
                        .model_providers
                        .entry(model)
                        .or_insert_with(Vec::new)
                        .push(Arc::new(provider));

                    self.routing_table
                        .providers_by_id
                        .insert(provider.provider_id.clone(), Arc::new(provider));
                }

                RoutingTableUpdate::RemoveProvider { provider_id } => {
                    // Remove from all model mappings
                    for mut entry in self.routing_table.model_providers.iter_mut() {
                        entry.value_mut().retain(|p| p.provider_id != provider_id);
                    }

                    self.routing_table.providers_by_id.remove(&provider_id);
                }

                RoutingTableUpdate::UpdateProviderWeight { provider_id, weight } => {
                    if let Some(provider) = self.routing_table.providers_by_id.get(&provider_id) {
                        provider.weight.store(weight, Ordering::Relaxed);
                    }
                }

                RoutingTableUpdate::SetModelAlias { alias, target } => {
                    self.routing_table.model_aliases.insert(alias, target);
                }
            }
        }

        // Increment generation counter for cache invalidation
        self.routing_table.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Record request result for metrics and health tracking
    fn record_result(&self, result: &RequestResult) {
        // Update provider metrics
        if let Some(provider) = self.routing_table.providers_by_id.get(&result.provider_id) {
            // Update counters atomically
            provider.total_requests.fetch_add(1, Ordering::Relaxed);

            if result.success {
                provider.successful_requests.fetch_add(1, Ordering::Relaxed);

                // Record latency in histogram
                provider.latency_histogram.write()
                    .record(result.latency.as_micros() as u64)
                    .ok();

                // Reset failure count on success
                provider.failure_count.store(0, Ordering::Relaxed);

                // Transition circuit breaker state
                match provider.circuit_state.load() {
                    CircuitState::HalfOpen => {
                        // Successful request in half-open - close circuit
                        provider.circuit_state.store(CircuitState::Closed);
                    }
                    _ => {}
                }
            } else {
                provider.failed_requests.fetch_add(1, Ordering::Relaxed);

                // Increment failure count
                let failures = provider.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                provider.last_failure_time.store(Some(Instant::now()));

                // Check circuit breaker threshold
                if failures >= self.config.circuit_breaker_threshold {
                    provider.circuit_state.store(CircuitState::Open);
                }
            }

            // Decrement active connections
            provider.active_connections.fetch_sub(1, Ordering::Relaxed);
        }

        // Update health scores
        self.health_router.update_health(&result.provider_id, result);

        // Update load balancer strategies
        for strategy in self.strategies.iter() {
            strategy.value().record_result(&result.provider_id, result);
        }
    }
}

enum RoutingTableUpdate {
    AddProvider { model: String, provider: ProviderCandidate },
    RemoveProvider { provider_id: String },
    UpdateProviderWeight { provider_id: String, weight: u32 },
    SetModelAlias { alias: String, target: String },
}

#[derive(Debug)]
enum RoutingError {
    NoProvidersAvailable,
    AllProvidersFiltered,
    AllProvidersUnhealthy,
    ModelNotSupported,
    BudgetExceeded,
    SelectionFailed,
    StrategyNotFound,
    Timeout,
}

impl RoutingTable {
    fn new() -> Self {
        Self {
            model_providers: DashMap::new(),
            providers_by_id: DashMap::new(),
            tenant_overrides: DashMap::new(),
            model_aliases: DashMap::new(),
            generation: AtomicU64::new(0),
        }
    }
}

impl RouterMetrics {
    fn new() -> Self {
        Self {
            total_routes: AtomicU64::new(0),
            successful_routes: AtomicU64::new(0),
            failed_routes: AtomicU64::new(0),
            routing_time_histogram: Arc::new(RwLock::new(
                Histogram::new(3).unwrap()  // 3 significant digits
            )),
        }
    }
}
```

---

## 3. Load Balancing Strategies

```rust
// ============================================================================
// LOAD BALANCER TRAIT
// ============================================================================

trait LoadBalancer: Send + Sync {
    /// Select a provider from candidates
    fn select(
        &self,
        candidates: &[Arc<ProviderCandidate>],
        context: &RoutingContext,
    ) -> Option<&Arc<ProviderCandidate>>;

    /// Record request result for learning/adaptation
    fn record_result(&self, provider_id: &str, result: &RequestResult);

    /// Get strategy name
    fn name(&self) -> &'static str;
}

// ============================================================================
// ROUND ROBIN (Lock-free)
// ============================================================================

struct RoundRobinLoadBalancer {
    // Global counter (wraps around)
    counter: AtomicU64,
}

impl RoundRobinLoadBalancer {
    fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }
}

impl LoadBalancer for RoundRobinLoadBalancer {
    fn select(
        &self,
        candidates: &[Arc<ProviderCandidate>],
        _context: &RoutingContext,
    ) -> Option<&Arc<ProviderCandidate>> {
        if candidates.is_empty() {
            return None;
        }

        // Atomic increment and wrap
        let index = self.counter.fetch_add(1, Ordering::Relaxed);
        let selected_index = (index as usize) % candidates.len();

        Some(&candidates[selected_index])
    }

    fn record_result(&self, _provider_id: &str, _result: &RequestResult) {
        // No-op for simple round robin
    }

    fn name(&self) -> &'static str {
        "RoundRobin"
    }
}

// ============================================================================
// WEIGHTED ROUND ROBIN (Lock-free with smoothing)
// ============================================================================

struct WeightedRoundRobinLoadBalancer {
    // Provider ID -> Current weight (dynamic, changes on each selection)
    current_weights: DashMap<String, AtomicU64>,
}

impl WeightedRoundRobinLoadBalancer {
    fn new() -> Self {
        Self {
            current_weights: DashMap::new(),
        }
    }

    /// Smooth weighted round robin algorithm (Nginx-style)
    fn select_weighted(&self, candidates: &[Arc<ProviderCandidate>]) -> Option<&Arc<ProviderCandidate>> {
        if candidates.is_empty() {
            return None;
        }

        let mut total_weight: u64 = 0;
        let mut best_candidate: Option<&Arc<ProviderCandidate>> = None;
        let mut best_current_weight: i64 = i64::MIN;

        for candidate in candidates {
            // Get or initialize current weight
            let current = self.current_weights
                .entry(candidate.provider_id.clone())
                .or_insert_with(|| AtomicU64::new(0));

            let weight = candidate.weight.load(Ordering::Relaxed) as u64;
            total_weight += weight;

            // Increment current weight
            let new_current = current.fetch_add(weight, Ordering::Relaxed) as i64 + weight as i64;

            // Track best candidate
            if new_current > best_current_weight {
                best_current_weight = new_current;
                best_candidate = Some(candidate);
            }
        }

        if let Some(selected) = best_candidate {
            // Decrease selected candidate's current weight by total
            if let Some(current) = self.current_weights.get(&selected.provider_id) {
                current.fetch_sub(total_weight, Ordering::Relaxed);
            }
        }

        best_candidate
    }
}

impl LoadBalancer for WeightedRoundRobinLoadBalancer {
    fn select(
        &self,
        candidates: &[Arc<ProviderCandidate>],
        _context: &RoutingContext,
    ) -> Option<&Arc<ProviderCandidate>> {
        self.select_weighted(candidates)
    }

    fn record_result(&self, _provider_id: &str, _result: &RequestResult) {
        // Weight updates happen externally via routing table updates
    }

    fn name(&self) -> &'static str {
        "WeightedRoundRobin"
    }
}

// ============================================================================
// LEAST CONNECTIONS (Lock-free)
// ============================================================================

struct LeastConnectionsLoadBalancer {
    // No state needed - reads from provider candidates directly
}

impl LeastConnectionsLoadBalancer {
    fn new() -> Self {
        Self {}
    }
}

impl LoadBalancer for LeastConnectionsLoadBalancer {
    fn select(
        &self,
        candidates: &[Arc<ProviderCandidate>],
        _context: &RoutingContext,
    ) -> Option<&Arc<ProviderCandidate>> {
        if candidates.is_empty() {
            return None;
        }

        // Find candidate with minimum active connections
        // In case of tie, prefer higher weight
        candidates
            .iter()
            .min_by_key(|candidate| {
                let connections = candidate.active_connections.load(Ordering::Relaxed);
                let weight = candidate.weight.load(Ordering::Relaxed);

                // Scale connections by inverse of weight
                // Higher weight = lower effective connection count
                let weight_factor = if weight > 0 { 1000 / weight } else { 1000 };
                connections * weight_factor
            })
    }

    fn record_result(&self, _provider_id: &str, _result: &RequestResult) {
        // Connection counts are tracked by Router.record_result
    }

    fn name(&self) -> &'static str {
        "LeastConnections"
    }
}

// ============================================================================
// LEAST LATENCY (P50/P95 aware)
// ============================================================================

struct LeastLatencyLoadBalancer {
    // Configuration
    use_p95: AtomicCell<bool>,  // If true, use P95; else use P50
    min_samples_required: u64,
}

impl LeastLatencyLoadBalancer {
    fn new() -> Self {
        Self {
            use_p95: AtomicCell::new(false),
            min_samples_required: 10,
        }
    }

    fn with_percentile(use_p95: bool) -> Self {
        Self {
            use_p95: AtomicCell::new(use_p95),
            min_samples_required: 10,
        }
    }

    fn get_latency_score(&self, candidate: &ProviderCandidate) -> u64 {
        let histogram = candidate.latency_histogram.read();

        // Check if we have enough samples
        if histogram.len() < self.min_samples_required {
            // Not enough data - return high latency to deprioritize
            return u64::MAX / 2;
        }

        // Get latency at desired percentile
        let quantile = if self.use_p95.load() { 0.95 } else { 0.50 };
        let latency_micros = histogram.value_at_quantile(quantile);

        // Adjust by health score (lower health = higher effective latency)
        let health_score = candidate.health_score.load();
        let health_factor = if health_score > 0.0 {
            1.0 / health_score
        } else {
            10.0  // Very low health
        };

        (latency_micros as f64 * health_factor) as u64
    }
}

impl LoadBalancer for LeastLatencyLoadBalancer {
    fn select(
        &self,
        candidates: &[Arc<ProviderCandidate>],
        context: &RoutingContext,
    ) -> Option<&Arc<ProviderCandidate>> {
        if candidates.is_empty() {
            return None;
        }

        // If context has latency target, filter candidates
        let filtered = if let Some(target) = context.latency_target {
            candidates
                .iter()
                .filter(|c| {
                    let score = self.get_latency_score(c);
                    score <= target.as_micros() as u64
                })
                .collect::<Vec<_>>()
        } else {
            candidates.iter().collect()
        };

        // Select candidate with minimum latency score
        filtered
            .into_iter()
            .min_by_key(|candidate| self.get_latency_score(candidate))
            .cloned()
    }

    fn record_result(&self, _provider_id: &str, _result: &RequestResult) {
        // Latency histograms are updated by Router.record_result
    }

    fn name(&self) -> &'static str {
        "LeastLatency"
    }
}

// ============================================================================
// COST OPTIMIZED (Minimize $/token)
// ============================================================================

struct CostOptimizedLoadBalancer {
    // No state needed - costs read from providers
}

impl CostOptimizedLoadBalancer {
    fn new() -> Self {
        Self {}
    }

    /// Calculate total cost for estimated request
    fn calculate_cost(&self, candidate: &ProviderCandidate, context: &RoutingContext) -> f64 {
        // Estimate tokens from context (simplified - in production, use tokenizer)
        let estimated_prompt_tokens = 1000u32;  // placeholder
        let estimated_completion_tokens = context
            .max_tokens_hint
            .unwrap_or(500);

        // Load atomic costs
        let input_cost_micro_cents = candidate.cost_per_1k_input_tokens.load(Ordering::Relaxed);
        let output_cost_micro_cents = candidate.cost_per_1k_output_tokens.load(Ordering::Relaxed);

        // Calculate total cost in dollars
        let input_cost = (estimated_prompt_tokens as f64 / 1000.0) *
                        (input_cost_micro_cents as f64 / 100_000_000.0);
        let output_cost = (estimated_completion_tokens as f64 / 1000.0) *
                         (output_cost_micro_cents as f64 / 100_000_000.0);

        input_cost + output_cost
    }
}

impl LoadBalancer for CostOptimizedLoadBalancer {
    fn select(
        &self,
        candidates: &[Arc<ProviderCandidate>],
        context: &RoutingContext,
    ) -> Option<&Arc<ProviderCandidate>> {
        if candidates.is_empty() {
            return None;
        }

        // Find lowest cost candidate
        candidates
            .iter()
            .min_by(|a, b| {
                let cost_a = self.calculate_cost(a, context);
                let cost_b = self.calculate_cost(b, context);
                cost_a.partial_cmp(&cost_b).unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    fn record_result(&self, _provider_id: &str, _result: &RequestResult) {
        // Cost tracking happens externally
    }

    fn name(&self) -> &'static str {
        "CostOptimized"
    }
}

// ============================================================================
// ADAPTIVE LOAD BALANCER (ML-based, multi-armed bandit)
// ============================================================================

struct AdaptiveLoadBalancer {
    // Thompson Sampling state for each provider
    provider_stats: DashMap<String, BanditStats>,

    // Configuration
    exploration_factor: f64,  // 0.0-1.0, higher = more exploration
}

struct BanditStats {
    // Beta distribution parameters for success rate
    alpha: AtomicU64,  // successes + 1
    beta: AtomicU64,   // failures + 1

    // Reward tracking (lower latency + lower cost = higher reward)
    total_reward: AtomicU64,  // stored as micro-units
    num_samples: AtomicU64,
}

impl AdaptiveLoadBalancer {
    fn new() -> Self {
        Self {
            provider_stats: DashMap::new(),
            exploration_factor: 0.1,
        }
    }

    /// Thompson Sampling: sample from beta distribution
    fn sample_beta(&self, stats: &BanditStats) -> f64 {
        let alpha = stats.alpha.load(Ordering::Relaxed) as f64;
        let beta = stats.beta.load(Ordering::Relaxed) as f64;

        // Simplified beta sampling (in production, use proper RNG)
        // This is a placeholder - use rand_distr::Beta in real implementation
        let mean = alpha / (alpha + beta);
        mean
    }

    /// Calculate reward for a request result
    fn calculate_reward(&self, result: &RequestResult) -> f64 {
        if !result.success {
            return 0.0;
        }

        // Reward formula: inverse of (normalized_latency + normalized_cost)
        // Lower latency and cost = higher reward

        let latency_ms = result.latency.as_millis() as f64;

        // Normalize latency (assume 5000ms is very bad)
        let normalized_latency = (latency_ms / 5000.0).min(1.0);

        // Cost is already normalized in context
        // For now, prioritize latency
        let reward = 1.0 - normalized_latency;

        reward.max(0.0).min(1.0)
    }
}

impl LoadBalancer for AdaptiveLoadBalancer {
    fn select(
        &self,
        candidates: &[Arc<ProviderCandidate>],
        _context: &RoutingContext,
    ) -> Option<&Arc<ProviderCandidate>> {
        if candidates.is_empty() {
            return None;
        }

        // Thompson Sampling: sample from each provider's posterior distribution
        let mut best_candidate: Option<&Arc<ProviderCandidate>> = None;
        let mut best_sample: f64 = -1.0;

        for candidate in candidates {
            // Get or initialize stats
            let stats = self.provider_stats
                .entry(candidate.provider_id.clone())
                .or_insert_with(|| BanditStats {
                    alpha: AtomicU64::new(1),  // uniform prior
                    beta: AtomicU64::new(1),
                    total_reward: AtomicU64::new(0),
                    num_samples: AtomicU64::new(0),
                });

            // Sample from posterior distribution
            let sample = self.sample_beta(&stats);

            if sample > best_sample {
                best_sample = sample;
                best_candidate = Some(candidate);
            }
        }

        best_candidate
    }

    fn record_result(&self, provider_id: &str, result: &RequestResult) {
        if let Some(stats) = self.provider_stats.get(provider_id) {
            if result.success {
                stats.alpha.fetch_add(1, Ordering::Relaxed);
            } else {
                stats.beta.fetch_add(1, Ordering::Relaxed);
            }

            let reward = self.calculate_reward(result);
            let reward_micro = (reward * 1_000_000.0) as u64;

            stats.total_reward.fetch_add(reward_micro, Ordering::Relaxed);
            stats.num_samples.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn name(&self) -> &'static str {
        "Adaptive"
    }
}

// ============================================================================
// RANDOM (for baseline comparison)
// ============================================================================

struct RandomLoadBalancer {
    // No state needed
}

impl RandomLoadBalancer {
    fn new() -> Self {
        Self {}
    }
}

impl LoadBalancer for RandomLoadBalancer {
    fn select(
        &self,
        candidates: &[Arc<ProviderCandidate>],
        _context: &RoutingContext,
    ) -> Option<&Arc<ProviderCandidate>> {
        if candidates.is_empty() {
            return None;
        }

        // Use fast random (xorshift or similar)
        // In production, use thread_local RNG for performance
        let index = fastrand::usize(..candidates.len());
        Some(&candidates[index])
    }

    fn record_result(&self, _provider_id: &str, _result: &RequestResult) {
        // No-op
    }

    fn name(&self) -> &'static str {
        "Random"
    }
}
```

---

## 4. Health-Aware Routing

```rust
// ============================================================================
// HEALTH-AWARE ROUTER
// ============================================================================

struct HealthAwareRouter {
    // Provider ID -> Health score (0.0-1.0)
    health_scores: DashMap<String, Arc<HealthScore>>,

    // Configuration
    config: HealthConfig,
}

struct HealthConfig {
    // Minimum health score to be considered healthy (0.0-1.0)
    min_health_threshold: f64,

    // Exponential weighted moving average factor (0.0-1.0)
    // Higher = more weight on recent data
    ewma_alpha: f64,

    // Time window for health calculations
    health_window: Duration,

    // Number of consecutive failures before marking unhealthy
    failure_threshold: u32,

    // Success rate threshold
    min_success_rate: f64,

    // Latency thresholds
    max_p50_latency: Duration,
    max_p95_latency: Duration,
}

struct HealthScore {
    // Success rate (0.0-1.0) - EWMA
    success_rate: AtomicCell<f64>,

    // Average latency metrics (microseconds)
    avg_latency_p50: AtomicU64,
    avg_latency_p95: AtomicU64,

    // Error rate (0.0-1.0) - EWMA
    error_rate: AtomicCell<f64>,

    // Consecutive failures
    consecutive_failures: AtomicU32,

    // Last update timestamp
    last_updated: AtomicCell<Instant>,

    // Overall health score (0.0-1.0) - weighted combination
    overall_score: AtomicCell<f64>,
}

impl HealthAwareRouter {
    fn new() -> Self {
        Self {
            health_scores: DashMap::new(),
            config: HealthConfig {
                min_health_threshold: 0.7,
                ewma_alpha: 0.3,
                health_window: Duration::from_secs(60),
                failure_threshold: 5,
                min_success_rate: 0.95,
                max_p50_latency: Duration::from_millis(500),
                max_p95_latency: Duration::from_millis(2000),
            },
        }
    }

    fn with_config(config: HealthConfig) -> Self {
        Self {
            health_scores: DashMap::new(),
            config,
        }
    }

    /// Calculate composite health score for a provider
    fn calculate_health_score(&self, provider_id: &str) -> f64 {
        if let Some(health) = self.health_scores.get(provider_id) {
            health.overall_score.load()
        } else {
            1.0  // New provider, assume healthy
        }
    }

    /// Update health metrics based on request result
    fn update_health(&self, provider_id: &str, result: &RequestResult) {
        // Get or create health score entry
        let health = self.health_scores
            .entry(provider_id.to_string())
            .or_insert_with(|| Arc::new(HealthScore::new()));

        let alpha = self.config.ewma_alpha;

        if result.success {
            // Update success rate (EWMA)
            let current_rate = health.success_rate.load();
            let new_rate = alpha * 1.0 + (1.0 - alpha) * current_rate;
            health.success_rate.store(new_rate);

            // Update error rate (EWMA)
            let current_error_rate = health.error_rate.load();
            let new_error_rate = alpha * 0.0 + (1.0 - alpha) * current_error_rate;
            health.error_rate.store(new_error_rate);

            // Reset consecutive failures
            health.consecutive_failures.store(0, Ordering::Relaxed);

            // Update latency metrics (EWMA)
            let latency_micros = result.latency.as_micros() as u64;

            let current_p50 = health.avg_latency_p50.load(Ordering::Relaxed);
            let new_p50 = ((alpha * latency_micros as f64) +
                          ((1.0 - alpha) * current_p50 as f64)) as u64;
            health.avg_latency_p50.store(new_p50, Ordering::Relaxed);

        } else {
            // Update success rate (EWMA)
            let current_rate = health.success_rate.load();
            let new_rate = alpha * 0.0 + (1.0 - alpha) * current_rate;
            health.success_rate.store(new_rate);

            // Update error rate (EWMA)
            let current_error_rate = health.error_rate.load();
            let new_error_rate = alpha * 1.0 + (1.0 - alpha) * current_error_rate;
            health.error_rate.store(new_error_rate);

            // Increment consecutive failures
            health.consecutive_failures.fetch_add(1, Ordering::Relaxed);
        }

        // Update timestamp
        health.last_updated.store(Instant::now());

        // Recalculate overall health score
        let overall = self.compute_overall_score(&health);
        health.overall_score.store(overall);
    }

    /// Compute overall health score from individual metrics
    fn compute_overall_score(&self, health: &HealthScore) -> f64 {
        // Weighted combination of metrics
        const WEIGHT_SUCCESS_RATE: f64 = 0.5;
        const WEIGHT_ERROR_RATE: f64 = 0.2;
        const WEIGHT_LATENCY: f64 = 0.3;

        let success_rate = health.success_rate.load();
        let error_rate = health.error_rate.load();

        // Normalize latency (0-1 scale, where 0 = max_p50_latency, 1 = 0ms)
        let p50_micros = health.avg_latency_p50.load(Ordering::Relaxed);
        let max_latency_micros = self.config.max_p50_latency.as_micros() as u64;
        let latency_score = 1.0 - (p50_micros as f64 / max_latency_micros as f64).min(1.0);

        // Check consecutive failures
        let consecutive_failures = health.consecutive_failures.load(Ordering::Relaxed);
        let failure_penalty = if consecutive_failures >= self.config.failure_threshold {
            0.5  // 50% penalty for excessive failures
        } else {
            1.0
        };

        // Compute weighted score
        let score = (
            WEIGHT_SUCCESS_RATE * success_rate +
            WEIGHT_ERROR_RATE * (1.0 - error_rate) +
            WEIGHT_LATENCY * latency_score
        ) * failure_penalty;

        score.max(0.0).min(1.0)
    }

    /// Filter candidates to only healthy providers
    fn filter_healthy(&self, candidates: &[Arc<ProviderCandidate>]) -> Vec<&Arc<ProviderCandidate>> {
        candidates
            .iter()
            .filter(|candidate| {
                let health_score = self.calculate_health_score(&candidate.provider_id);
                health_score >= self.config.min_health_threshold
            })
            .collect()
    }

    /// Get detailed health report for a provider
    fn get_health_report(&self, provider_id: &str) -> Option<HealthReport> {
        self.health_scores.get(provider_id).map(|health| {
            HealthReport {
                provider_id: provider_id.to_string(),
                success_rate: health.success_rate.load(),
                error_rate: health.error_rate.load(),
                avg_latency_p50: Duration::from_micros(health.avg_latency_p50.load(Ordering::Relaxed)),
                avg_latency_p95: Duration::from_micros(health.avg_latency_p95.load(Ordering::Relaxed)),
                consecutive_failures: health.consecutive_failures.load(Ordering::Relaxed),
                overall_score: health.overall_score.load(),
                last_updated: health.last_updated.load(),
            }
        })
    }

    /// Periodic health check (run in background task)
    async fn run_health_checks(&self, providers: &[Arc<ProviderCandidate>]) {
        loop {
            tokio::time::sleep(self.config.health_window).await;

            for provider in providers {
                // Check if health data is stale
                if let Some(health) = self.health_scores.get(&provider.provider_id) {
                    let last_update = health.last_updated.load();
                    let elapsed = last_update.elapsed();

                    if elapsed > self.config.health_window * 2 {
                        // No recent data - gradually decay health score
                        let current_score = health.overall_score.load();
                        let decayed_score = current_score * 0.95;  // 5% decay
                        health.overall_score.store(decayed_score);
                    }
                }
            }
        }
    }
}

impl HealthScore {
    fn new() -> Self {
        Self {
            success_rate: AtomicCell::new(1.0),  // Assume healthy initially
            avg_latency_p50: AtomicU64::new(0),
            avg_latency_p95: AtomicU64::new(0),
            error_rate: AtomicCell::new(0.0),
            consecutive_failures: AtomicU32::new(0),
            last_updated: AtomicCell::new(Instant::now()),
            overall_score: AtomicCell::new(1.0),
        }
    }
}

struct HealthReport {
    provider_id: String,
    success_rate: f64,
    error_rate: f64,
    avg_latency_p50: Duration,
    avg_latency_p95: Duration,
    consecutive_failures: u32,
    overall_score: f64,
    last_updated: Instant,
}
```

---

## 5. Routing Rules Engine

```rust
// ============================================================================
// ROUTING RULES ENGINE
// ============================================================================

struct RoutingRulesEngine {
    // Rules sorted by priority (higher priority first)
    rules: Arc<RwLock<Vec<Arc<CompiledRule>>>>,

    // Default route when no rules match
    default_route: Arc<AtomicCell<RouteConfig>>,

    // Rule evaluation cache (request hash -> matched rule)
    rule_cache: DashMap<u64, CachedRuleMatch>,

    // Metrics
    metrics: Arc<RulesMetrics>,
}

struct CompiledRule {
    id: String,
    priority: u32,
    matcher: Box<dyn RequestMatcher>,
    action: RoutingAction,
    enabled: AtomicCell<bool>,
}

struct CachedRuleMatch {
    rule_id: String,
    action: RoutingAction,
    timestamp: Instant,
    ttl: Duration,
}

struct RouteConfig {
    strategy: LoadBalancingStrategy,
    provider_pool: Vec<String>,
}

enum RoutingAction {
    // Route to specific provider
    RouteToProvider(String),

    // Route to pool of providers
    RouteToPool(Vec<String>),

    // Apply specific load balancing strategy
    ApplyStrategy(LoadBalancingStrategy),

    // Modify request priority
    SetPriority(RequestPriority),

    // Reject request
    Reject { reason: RejectionReason },

    // Chain multiple actions
    Chain(Vec<RoutingAction>),
}

#[derive(Clone)]
enum RejectionReason {
    RateLimitExceeded,
    UnauthorizedModel,
    InvalidRequest,
    MaintenanceMode,
    Custom(String),
}

struct RulesMetrics {
    rule_evaluations: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    rule_matches: DashMap<String, AtomicU64>,  // rule_id -> match count
}

// ============================================================================
// REQUEST MATCHERS
// ============================================================================

trait RequestMatcher: Send + Sync {
    /// Check if request matches this rule
    fn matches(&self, request: &GatewayRequest, context: &RoutingContext) -> bool;

    /// Get matcher description for debugging
    fn description(&self) -> String;
}

// --- Model Name Matcher ---

struct ModelMatcher {
    models: HashSet<String>,
}

impl RequestMatcher for ModelMatcher {
    fn matches(&self, request: &GatewayRequest, _context: &RoutingContext) -> bool {
        self.models.contains(&request.model)
    }

    fn description(&self) -> String {
        format!("model in [{}]", self.models.iter().cloned().collect::<Vec<_>>().join(", "))
    }
}

// --- Tenant Matcher ---

struct TenantMatcher {
    tenant_ids: HashSet<String>,
}

impl RequestMatcher for TenantMatcher {
    fn matches(&self, _request: &GatewayRequest, context: &RoutingContext) -> bool {
        context.tenant_id
            .as_ref()
            .map(|id| self.tenant_ids.contains(id))
            .unwrap_or(false)
    }

    fn description(&self) -> String {
        format!("tenant in [{}]", self.tenant_ids.iter().cloned().collect::<Vec<_>>().join(", "))
    }
}

// --- Priority Matcher ---

struct PriorityMatcher {
    min_priority: RequestPriority,
}

impl RequestMatcher for PriorityMatcher {
    fn matches(&self, request: &GatewayRequest, _context: &RoutingContext) -> bool {
        request.priority >= self.min_priority
    }

    fn description(&self) -> String {
        format!("priority >= {:?}", self.min_priority)
    }
}

// --- Region Matcher ---

struct RegionMatcher {
    regions: HashSet<String>,
}

impl RequestMatcher for RegionMatcher {
    fn matches(&self, _request: &GatewayRequest, context: &RoutingContext) -> bool {
        context.region
            .as_ref()
            .map(|r| self.regions.contains(r))
            .unwrap_or(false)
    }

    fn description(&self) -> String {
        format!("region in [{}]", self.regions.iter().cloned().collect::<Vec<_>>().join(", "))
    }
}

// --- Time Window Matcher ---

struct TimeWindowMatcher {
    start_hour: u8,  // 0-23
    end_hour: u8,    // 0-23
}

impl RequestMatcher for TimeWindowMatcher {
    fn matches(&self, _request: &GatewayRequest, _context: &RoutingContext) -> bool {
        let now = chrono::Utc::now();
        let hour = now.hour() as u8;

        if self.start_hour <= self.end_hour {
            hour >= self.start_hour && hour < self.end_hour
        } else {
            // Wraps around midnight
            hour >= self.start_hour || hour < self.end_hour
        }
    }

    fn description(&self) -> String {
        format!("hour in {}..{}", self.start_hour, self.end_hour)
    }
}

// --- Composite Matchers ---

struct AndMatcher {
    matchers: Vec<Box<dyn RequestMatcher>>,
}

impl RequestMatcher for AndMatcher {
    fn matches(&self, request: &GatewayRequest, context: &RoutingContext) -> bool {
        self.matchers.iter().all(|m| m.matches(request, context))
    }

    fn description(&self) -> String {
        let desc = self.matchers.iter().map(|m| m.description()).collect::<Vec<_>>().join(" AND ");
        format!("({})", desc)
    }
}

struct OrMatcher {
    matchers: Vec<Box<dyn RequestMatcher>>,
}

impl RequestMatcher for OrMatcher {
    fn matches(&self, request: &GatewayRequest, context: &RoutingContext) -> bool {
        self.matchers.iter().any(|m| m.matches(request, context))
    }

    fn description(&self) -> String {
        let desc = self.matchers.iter().map(|m| m.description()).collect::<Vec<_>>().join(" OR ");
        format!("({})", desc)
    }
}

struct NotMatcher {
    matcher: Box<dyn RequestMatcher>,
}

impl RequestMatcher for NotMatcher {
    fn matches(&self, request: &GatewayRequest, context: &RoutingContext) -> bool {
        !self.matcher.matches(request, context)
    }

    fn description(&self) -> String {
        format!("NOT ({})", self.matcher.description())
    }
}

// ============================================================================
// RULES ENGINE IMPLEMENTATION
// ============================================================================

impl RoutingRulesEngine {
    fn new() -> Self {
        Self {
            rules: Arc::new(RwLock::new(Vec::new())),
            default_route: Arc::new(AtomicCell::new(RouteConfig {
                strategy: LoadBalancingStrategy::RoundRobin,
                provider_pool: Vec::new(),
            })),
            rule_cache: DashMap::new(),
            metrics: Arc::new(RulesMetrics::new()),
        }
    }

    /// Apply rules to filter candidates and select strategy
    fn apply_rules(
        &self,
        candidates: Vec<Arc<ProviderCandidate>>,
        request: &GatewayRequest,
        context: &RoutingContext,
    ) -> (Vec<Arc<ProviderCandidate>>, LoadBalancingStrategy) {
        self.metrics.rule_evaluations.fetch_add(1, Ordering::Relaxed);

        // Calculate request hash for caching
        let request_hash = self.calculate_request_hash(request, context);

        // Check cache first
        if let Some(cached) = self.rule_cache.get(&request_hash) {
            if cached.timestamp.elapsed() < cached.ttl {
                self.metrics.cache_hits.fetch_add(1, Ordering::Relaxed);
                return self.apply_action(&cached.action, candidates, context);
            }
        }

        self.metrics.cache_misses.fetch_add(1, Ordering::Relaxed);

        // Evaluate rules in priority order
        let rules = self.rules.read();

        for rule in rules.iter() {
            if !rule.enabled.load() {
                continue;
            }

            if rule.matcher.matches(request, context) {
                // Record match
                self.metrics.rule_matches
                    .entry(rule.id.clone())
                    .or_insert_with(|| AtomicU64::new(0))
                    .fetch_add(1, Ordering::Relaxed);

                // Cache the match
                self.rule_cache.insert(request_hash, CachedRuleMatch {
                    rule_id: rule.id.clone(),
                    action: rule.action.clone(),
                    timestamp: Instant::now(),
                    ttl: Duration::from_secs(60),
                });

                return self.apply_action(&rule.action, candidates, context);
            }
        }

        // No rules matched - use default route
        let default = self.default_route.load();
        (candidates, default.strategy)
    }

    /// Apply routing action to candidates
    fn apply_action(
        &self,
        action: &RoutingAction,
        mut candidates: Vec<Arc<ProviderCandidate>>,
        _context: &RoutingContext,
    ) -> (Vec<Arc<ProviderCandidate>>, LoadBalancingStrategy) {
        match action {
            RoutingAction::RouteToProvider(provider_id) => {
                // Filter to only specified provider
                candidates.retain(|c| &c.provider_id == provider_id);
                (candidates, LoadBalancingStrategy::RoundRobin)
            }

            RoutingAction::RouteToPool(provider_ids) => {
                // Filter to only providers in pool
                let pool_set: HashSet<&String> = provider_ids.iter().collect();
                candidates.retain(|c| pool_set.contains(&c.provider_id));
                (candidates, LoadBalancingStrategy::RoundRobin)
            }

            RoutingAction::ApplyStrategy(strategy) => {
                (candidates, *strategy)
            }

            RoutingAction::SetPriority(_priority) => {
                // Priority is set in context, not here
                (candidates, LoadBalancingStrategy::RoundRobin)
            }

            RoutingAction::Reject { .. } => {
                // Clear all candidates to trigger rejection
                (Vec::new(), LoadBalancingStrategy::RoundRobin)
            }

            RoutingAction::Chain(actions) => {
                let mut strategy = LoadBalancingStrategy::RoundRobin;
                for sub_action in actions {
                    let result = self.apply_action(sub_action, candidates, _context);
                    candidates = result.0;
                    strategy = result.1;
                }
                (candidates, strategy)
            }
        }
    }

    /// Calculate hash for request caching
    fn calculate_request_hash(&self, request: &GatewayRequest, context: &RoutingContext) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        request.model.hash(&mut hasher);
        context.tenant_id.hash(&mut hasher);
        request.priority.hash(&mut hasher);

        hasher.finish()
    }

    /// Add or update a routing rule
    fn add_rule(&self, rule: CompiledRule) {
        let mut rules = self.rules.write();

        // Remove existing rule with same ID
        rules.retain(|r| r.id != rule.id);

        // Insert new rule
        rules.push(Arc::new(rule));

        // Sort by priority (descending)
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        // Invalidate cache
        self.rule_cache.clear();
    }

    /// Remove a routing rule
    fn remove_rule(&self, rule_id: &str) {
        let mut rules = self.rules.write();
        rules.retain(|r| r.id != rule_id);

        // Invalidate cache
        self.rule_cache.clear();
    }

    /// Enable/disable a rule
    fn set_rule_enabled(&self, rule_id: &str, enabled: bool) {
        let rules = self.rules.read();
        if let Some(rule) = rules.iter().find(|r| r.id == rule_id) {
            rule.enabled.store(enabled);

            // Invalidate cache
            self.rule_cache.clear();
        }
    }
}

impl RulesMetrics {
    fn new() -> Self {
        Self {
            rule_evaluations: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            rule_matches: DashMap::new(),
        }
    }
}
```

---

## 6. Failover Chain

```rust
// ============================================================================
// FAILOVER CHAIN
// ============================================================================

struct FailoverChain {
    // Primary provider selection
    primary: ProviderSelector,

    // Fallback providers (in order)
    fallbacks: Vec<ProviderSelector>,

    // Configuration
    max_attempts: usize,
    backoff_strategy: BackoffStrategy,
}

enum ProviderSelector {
    // Specific provider ID
    Specific(String),

    // Provider pool with strategy
    Pool {
        provider_ids: Vec<String>,
        strategy: LoadBalancingStrategy,
    },

    // All available providers for model
    AnyForModel(String),
}

enum BackoffStrategy {
    None,
    Linear { increment_ms: u64 },
    Exponential { base_ms: u64, multiplier: f64 },
    Fibonacci { base_ms: u64 },
}

impl FailoverChain {
    fn new(primary: ProviderSelector) -> Self {
        Self {
            primary,
            fallbacks: Vec::new(),
            max_attempts: 3,
            backoff_strategy: BackoffStrategy::Exponential {
                base_ms: 100,
                multiplier: 2.0,
            },
        }
    }

    fn with_fallback(mut self, fallback: ProviderSelector) -> Self {
        self.fallbacks.push(fallback);
        self
    }

    fn with_max_attempts(mut self, max_attempts: usize) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    fn with_backoff(mut self, strategy: BackoffStrategy) -> Self {
        self.backoff_strategy = strategy;
        self
    }

    /// Execute operation with automatic failover
    async fn execute<F, Fut, T>(&self, mut operation: F) -> Result<T, FailoverError>
    where
        F: FnMut(&ProviderSelector, u32) -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        let mut attempt = 0;
        let mut last_error = None;

        // Try primary
        match operation(&self.primary, attempt).await {
            Ok(result) => return Ok(result),
            Err(err) => {
                last_error = Some(err);
                attempt += 1;
            }
        }

        // Try fallbacks
        for fallback in &self.fallbacks {
            if attempt >= self.max_attempts {
                break;
            }

            // Apply backoff delay
            if let Some(delay) = self.calculate_backoff(attempt) {
                tokio::time::sleep(delay).await;
            }

            match operation(fallback, attempt).await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    last_error = Some(err);
                    attempt += 1;
                }
            }
        }

        Err(FailoverError::AllAttemptsFailed {
            attempts: attempt,
            last_error: last_error.unwrap_or_else(|| "Unknown error".to_string()),
        })
    }

    /// Calculate backoff delay for attempt
    fn calculate_backoff(&self, attempt: u32) -> Option<Duration> {
        match &self.backoff_strategy {
            BackoffStrategy::None => None,

            BackoffStrategy::Linear { increment_ms } => {
                Some(Duration::from_millis(*increment_ms * attempt as u64))
            }

            BackoffStrategy::Exponential { base_ms, multiplier } => {
                let delay_ms = *base_ms as f64 * multiplier.powi(attempt as i32);
                Some(Duration::from_millis(delay_ms as u64))
            }

            BackoffStrategy::Fibonacci { base_ms } => {
                let fib = self.fibonacci(attempt as usize);
                Some(Duration::from_millis(*base_ms * fib as u64))
            }
        }
    }

    fn fibonacci(&self, n: usize) -> usize {
        match n {
            0 => 1,
            1 => 1,
            _ => {
                let mut a = 1;
                let mut b = 1;
                for _ in 2..=n {
                    let temp = a + b;
                    a = b;
                    b = temp;
                }
                b
            }
        }
    }
}

#[derive(Debug)]
enum FailoverError {
    AllAttemptsFailed { attempts: u32, last_error: String },
    MaxAttemptsExceeded,
}

// ============================================================================
// RETRY POLICY
// ============================================================================

struct RetryPolicy {
    max_retries: u32,
    retry_on_status_codes: HashSet<u16>,
    retry_on_errors: HashSet<ErrorKind>,
    backoff: BackoffStrategy,
}

#[derive(Hash, Eq, PartialEq)]
enum ErrorKind {
    Timeout,
    ConnectionError,
    RateLimitExceeded,
    ServerError,
    ServiceUnavailable,
}

impl RetryPolicy {
    fn default() -> Self {
        let mut retry_status_codes = HashSet::new();
        retry_status_codes.insert(429);  // Rate limit
        retry_status_codes.insert(500);  // Internal server error
        retry_status_codes.insert(502);  // Bad gateway
        retry_status_codes.insert(503);  // Service unavailable
        retry_status_codes.insert(504);  // Gateway timeout

        let mut retry_errors = HashSet::new();
        retry_errors.insert(ErrorKind::Timeout);
        retry_errors.insert(ErrorKind::ConnectionError);
        retry_errors.insert(ErrorKind::ServiceUnavailable);

        Self {
            max_retries: 3,
            retry_on_status_codes: retry_status_codes,
            retry_on_errors: retry_errors,
            backoff: BackoffStrategy::Exponential {
                base_ms: 100,
                multiplier: 2.0,
            },
        }
    }

    fn should_retry(&self, error: &RequestError, attempt: u32) -> bool {
        if attempt >= self.max_retries {
            return false;
        }

        match error {
            RequestError::StatusCode(code) => self.retry_on_status_codes.contains(code),
            RequestError::Kind(kind) => self.retry_on_errors.contains(kind),
            _ => false,
        }
    }
}

enum RequestError {
    StatusCode(u16),
    Kind(ErrorKind),
    Other(String),
}
```

---

## 7. Request Context

```rust
// ============================================================================
// ROUTING CONTEXT
// ============================================================================

struct RoutingContext {
    // Identity
    tenant_id: Option<String>,
    user_id: Option<String>,
    organization_id: Option<String>,

    // Request properties
    priority: RequestPriority,
    attempt_number: u32,

    // Budget constraints
    cost_budget: Option<CostBudget>,

    // Performance targets
    latency_target: Option<Duration>,
    throughput_target: Option<u32>,  // tokens per second

    // Provider constraints
    excluded_providers: HashSet<String>,
    preferred_regions: Vec<String>,

    // Token estimation hints
    max_tokens_hint: Option<u32>,

    // Geographic routing
    region: Option<String>,
    zone: Option<String>,

    // Trace context (OpenTelemetry)
    trace_context: TraceContext,

    // Feature flags
    features: HashMap<String, bool>,

    // Request metadata
    source_ip: Option<String>,
    user_agent: Option<String>,

    // Timing
    request_start_time: Instant,
    deadline: Option<Instant>,
}

impl RoutingContext {
    fn new() -> Self {
        Self {
            tenant_id: None,
            user_id: None,
            organization_id: None,
            priority: RequestPriority::Normal,
            attempt_number: 0,
            cost_budget: None,
            latency_target: None,
            throughput_target: None,
            excluded_providers: HashSet::new(),
            preferred_regions: Vec::new(),
            max_tokens_hint: None,
            region: None,
            zone: None,
            trace_context: TraceContext::new(),
            features: HashMap::new(),
            source_ip: None,
            user_agent: None,
            request_start_time: Instant::now(),
            deadline: None,
        }
    }

    fn with_tenant(mut self, tenant_id: String) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    fn with_priority(mut self, priority: RequestPriority) -> Self {
        self.priority = priority;
        self
    }

    fn with_cost_budget(mut self, budget: CostBudget) -> Self {
        self.cost_budget = Some(budget);
        self
    }

    fn with_latency_target(mut self, target: Duration) -> Self {
        self.latency_target = Some(target);
        self
    }

    fn with_region(mut self, region: String) -> Self {
        self.region = Some(region);
        self
    }

    fn exclude_provider(mut self, provider_id: String) -> Self {
        self.excluded_providers.insert(provider_id);
        self
    }

    fn with_deadline(mut self, deadline: Duration) -> Self {
        self.deadline = Some(Instant::now() + deadline);
        self
    }

    fn elapsed(&self) -> Duration {
        self.request_start_time.elapsed()
    }

    fn is_deadline_exceeded(&self) -> bool {
        self.deadline
            .map(|d| Instant::now() >= d)
            .unwrap_or(false)
    }

    fn remaining_time(&self) -> Option<Duration> {
        self.deadline.and_then(|d| {
            let now = Instant::now();
            if now < d {
                Some(d - now)
            } else {
                None
            }
        })
    }
}

impl TraceContext {
    fn new() -> Self {
        Self {
            trace_id: uuid::Uuid::new_v4().to_string(),
            span_id: uuid::Uuid::new_v4().to_string(),
            parent_span_id: None,
            baggage: HashMap::new(),
        }
    }

    fn with_parent(mut self, parent_span_id: String) -> Self {
        self.parent_span_id = Some(parent_span_id);
        self
    }

    fn add_baggage(mut self, key: String, value: String) -> Self {
        self.baggage.insert(key, value);
        self
    }
}
```

---

## 8. Performance Optimizations

```rust
// ============================================================================
// PERFORMANCE OPTIMIZATIONS
// ============================================================================

/// Lock-free statistics tracking using atomics
struct LockFreeStats {
    // Use separate cache lines to avoid false sharing
    count: CachePadded<AtomicU64>,
    sum: CachePadded<AtomicU64>,
    min: CachePadded<AtomicU64>,
    max: CachePadded<AtomicU64>,
}

#[repr(align(64))]  // Cache line size
struct CachePadded<T> {
    value: T,
}

impl<T> CachePadded<T> {
    fn new(value: T) -> Self {
        Self { value }
    }
}

impl LockFreeStats {
    fn new() -> Self {
        Self {
            count: CachePadded::new(AtomicU64::new(0)),
            sum: CachePadded::new(AtomicU64::new(0)),
            min: CachePadded::new(AtomicU64::new(u64::MAX)),
            max: CachePadded::new(AtomicU64::new(0)),
        }
    }

    fn record(&self, value: u64) {
        self.count.value.fetch_add(1, Ordering::Relaxed);
        self.sum.value.fetch_add(value, Ordering::Relaxed);

        // Update min (lock-free compare-and-swap loop)
        let mut current_min = self.min.value.load(Ordering::Relaxed);
        while value < current_min {
            match self.min.value.compare_exchange_weak(
                current_min,
                value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_min = x,
            }
        }

        // Update max (lock-free compare-and-swap loop)
        let mut current_max = self.max.value.load(Ordering::Relaxed);
        while value > current_max {
            match self.max.value.compare_exchange_weak(
                current_max,
                value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_max = x,
            }
        }
    }

    fn average(&self) -> f64 {
        let count = self.count.value.load(Ordering::Relaxed);
        if count == 0 {
            return 0.0;
        }
        let sum = self.sum.value.load(Ordering::Relaxed);
        sum as f64 / count as f64
    }
}

// ============================================================================
// EFFICIENT LATENCY TRACKING
// ============================================================================

/// Thread-local histogram for lock-free recording
struct ThreadLocalHistogram {
    // Thread-local storage for histograms
    local: thread_local::ThreadLocal<RefCell<Histogram<u64>>>,

    // Global aggregated histogram (periodically merged)
    global: Arc<RwLock<Histogram<u64>>>,

    // Last merge time
    last_merge: AtomicCell<Instant>,
    merge_interval: Duration,
}

impl ThreadLocalHistogram {
    fn new() -> Self {
        Self {
            local: thread_local::ThreadLocal::new(),
            global: Arc::new(RwLock::new(Histogram::new(3).unwrap())),
            last_merge: AtomicCell::new(Instant::now()),
            merge_interval: Duration::from_secs(1),
        }
    }

    /// Record value in thread-local histogram (lock-free, fast path)
    fn record(&self, value: u64) {
        let histogram = self.local.get_or(|| {
            RefCell::new(Histogram::new(3).unwrap())
        });

        histogram.borrow_mut().record(value).ok();

        // Periodically merge into global histogram
        let last_merge = self.last_merge.load();
        if last_merge.elapsed() >= self.merge_interval {
            self.merge_global();
        }
    }

    /// Merge all thread-local histograms into global (slow path)
    fn merge_global(&self) {
        // Try to acquire merge lock (non-blocking)
        let now = Instant::now();
        let last = self.last_merge.load();

        if now.duration_since(last) < self.merge_interval {
            return;  // Another thread is merging
        }

        // Attempt to claim merge responsibility
        if self.last_merge.compare_exchange(last, now).is_err() {
            return;  // Another thread claimed it
        }

        // Perform merge
        let mut global = self.global.write();

        // Iterate all thread-local histograms
        self.local.iter().for_each(|local| {
            let local_hist = local.borrow();
            global.add(&local_hist).ok();
        });
    }

    /// Get percentile value from global histogram
    fn value_at_quantile(&self, quantile: f64) -> u64 {
        let global = self.global.read();
        global.value_at_quantile(quantile)
    }
}

// ============================================================================
// FAST ROUTING TABLE LOOKUP
// ============================================================================

/// Optimized routing table with bloom filter for negative lookups
struct FastRoutingTable {
    // Primary lookup table
    model_providers: DashMap<String, Arc<Vec<Arc<ProviderCandidate>>>>,

    // Bloom filter for fast negative lookups
    model_bloom: Arc<AtomicCell<BloomFilter>>,

    // Read-optimized view (immutable, copy-on-write)
    snapshot: Arc<AtomicCell<Option<Arc<RoutingSnapshot>>>>,
}

struct RoutingSnapshot {
    models: HashMap<String, Arc<Vec<Arc<ProviderCandidate>>>>,
    generation: u64,
}

struct BloomFilter {
    bits: Vec<AtomicU64>,
    hash_count: usize,
}

impl BloomFilter {
    fn new(size: usize, hash_count: usize) -> Self {
        let num_words = (size + 63) / 64;
        Self {
            bits: (0..num_words).map(|_| AtomicU64::new(0)).collect(),
            hash_count,
        }
    }

    fn add(&mut self, item: &str) {
        for i in 0..self.hash_count {
            let hash = self.hash_with_seed(item, i);
            let word_index = (hash / 64) % self.bits.len();
            let bit_index = hash % 64;

            self.bits[word_index].fetch_or(1u64 << bit_index, Ordering::Relaxed);
        }
    }

    fn contains(&self, item: &str) -> bool {
        for i in 0..self.hash_count {
            let hash = self.hash_with_seed(item, i);
            let word_index = (hash / 64) % self.bits.len();
            let bit_index = hash % 64;

            let word = self.bits[word_index].load(Ordering::Relaxed);
            if (word & (1u64 << bit_index)) == 0 {
                return false;
            }
        }
        true
    }

    fn hash_with_seed(&self, item: &str, seed: usize) -> usize {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        item.hash(&mut hasher);
        seed.hash(&mut hasher);
        hasher.finish() as usize
    }
}

impl FastRoutingTable {
    fn lookup_fast(&self, model: &str) -> Option<Arc<Vec<Arc<ProviderCandidate>>>> {
        // Fast path: check bloom filter first
        let bloom = self.model_bloom.load();
        if !bloom.contains(model) {
            return None;  // Definitely not in table
        }

        // Slow path: actual lookup
        self.model_providers.get(model).map(|v| Arc::clone(v.value()))
    }
}

// ============================================================================
// INLINE OPTIMIZATION HINTS
// ============================================================================

// Use #[inline(always)] for hot path functions
#[inline(always)]
fn fast_token_estimation(text: &str) -> u32 {
    // Simple heuristic: ~4 chars per token
    (text.len() / 4) as u32
}

#[inline(always)]
fn atomic_increment_relaxed(atomic: &AtomicU64) -> u64 {
    atomic.fetch_add(1, Ordering::Relaxed)
}

// Use likely/unlikely hints (via #[cold] for unlikely paths)
#[cold]
fn handle_routing_error(error: RoutingError) {
    // Error handling code (rarely executed)
    eprintln!("Routing error: {:?}", error);
}

// ============================================================================
// SIMD OPTIMIZATIONS (for batch operations)
// ============================================================================

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// Calculate minimum latency from array using SIMD
#[cfg(target_arch = "x86_64")]
unsafe fn min_latency_simd(latencies: &[u64]) -> u64 {
    if latencies.len() < 4 {
        return *latencies.iter().min().unwrap_or(&u64::MAX);
    }

    // Use AVX2 for parallel minimum finding
    let mut min_vec = _mm256_set1_epi64x(i64::MAX);

    for chunk in latencies.chunks_exact(4) {
        let vals = _mm256_loadu_si256(chunk.as_ptr() as *const __m256i);
        min_vec = _mm256_min_epu64(min_vec, vals);
    }

    // Extract minimum from vector
    let mut result = [0u64; 4];
    _mm256_storeu_si256(result.as_mut_ptr() as *mut __m256i, min_vec);

    *result.iter().min().unwrap()
}
```

---

## Summary

This comprehensive pseudocode provides a production-ready foundation for implementing a high-performance routing and load balancing system for an LLM inference gateway. Key features include:

### Performance Characteristics
- Sub-millisecond routing decisions via O(1) lookups and lock-free algorithms
- Lock-free atomic operations for counters and metrics
- Thread-local histogram recording for latency tracking
- Bloom filters for fast negative lookups
- Cache-line padding to avoid false sharing

### Load Balancing Strategies
- Round Robin (simple, lock-free)
- Weighted Round Robin (smooth algorithm, Nginx-style)
- Least Connections (real-time connection tracking)
- Least Latency (P50/P95 aware with health scoring)
- Cost Optimized (minimize $/token)
- Adaptive (Thompson Sampling multi-armed bandit)

### Health & Reliability
- Exponentially weighted moving averages for health metrics
- Circuit breaker pattern with automatic recovery
- Health-aware filtering with configurable thresholds
- Automatic failover chains with backoff strategies

### Routing Intelligence
- Priority-based rule matching with caching
- Composable request matchers (AND/OR/NOT logic)
- Tenant-specific routing overrides
- Time-window based routing
- Cost budget enforcement

### Observability
- Comprehensive metrics with lock-free collection
- OpenTelemetry trace context propagation
- Per-provider latency histograms (HDR Histogram)
- Rule evaluation metrics and caching stats

All implementations use Rust's powerful concurrency primitives (atomics, DashMap, crossbeam) to achieve maximum performance while maintaining safety guarantees.
