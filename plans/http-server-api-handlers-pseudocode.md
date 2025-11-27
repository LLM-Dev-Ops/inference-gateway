# HTTP Server and API Handlers - Comprehensive Pseudocode

> **Component**: HTTP Server and API Handlers Layer
> **Architecture**: Axum-based HTTP server with OpenAI API compatibility
> **Language**: Rust
> **Version**: 1.0.0
> **Last Updated**: 2025-11-27

---

## Table of Contents

1. [Overview](#overview)
2. [Core Data Structures](#core-data-structures)
3. [Server Setup and Initialization](#server-setup-and-initialization)
4. [Router Configuration](#router-configuration)
5. [Request/Response Types](#requestresponse-types)
6. [API Handler Implementations](#api-handler-implementations)
7. [Streaming Support](#streaming-support)
8. [Error Handling](#error-handling)
9. [Middleware Stack](#middleware-stack)
10. [Health and Admin Endpoints](#health-and-admin-endpoints)
11. [Request Validation and Extraction](#request-validation-and-extraction)
12. [Graceful Shutdown](#graceful-shutdown)
13. [Testing Strategy](#testing-strategy)

---

## Overview

The HTTP Server and API Handlers layer provides the primary interface for clients to interact with the LLM-Inference-Gateway. Built on the Axum web framework, it offers:

- OpenAI-compatible REST API endpoints
- Server-Sent Events (SSE) for streaming responses
- Comprehensive error handling with proper HTTP status codes
- Request validation and sanitization
- Health check endpoints for orchestration platforms
- Admin endpoints for configuration and monitoring
- Graceful shutdown with connection draining

**Design Goals**:
- Sub-5ms p95 routing overhead
- 10,000+ concurrent connections per instance
- Zero-downtime configuration updates
- OpenTelemetry-native observability
- Complete OpenAI API v1 compatibility

---

## Core Data Structures

### Gateway Server

```rust
//==============================================================================
// GATEWAY SERVER - Main server structure
//==============================================================================

/// Main gateway server managing the HTTP server lifecycle
pub struct GatewayServer {
    /// Server configuration (ports, TLS, timeouts)
    config: Arc<GatewayConfig>,

    /// Axum router with all routes and middleware
    router: Router,

    /// Shared application state
    state: Arc<GatewayState>,

    /// Graceful shutdown coordinator
    shutdown: ShutdownCoordinator,

    /// Telemetry handles for cleanup
    telemetry_guard: TelemetryGuard,
}

impl GatewayServer {
    /// Create a new gateway server instance
    ///
    /// # Arguments
    /// * `config` - Server configuration including bind address, TLS settings
    ///
    /// # Returns
    /// * `Result<Self>` - Initialized server or configuration error
    ///
    /// # Implementation Details
    /// 1. Validate configuration (ports, TLS certs, timeouts)
    /// 2. Initialize telemetry (tracing, metrics, logging)
    /// 3. Create shared state with provider registry
    /// 4. Build router with all endpoints and middleware
    /// 5. Set up graceful shutdown handlers
    pub async fn new(config: GatewayConfig) -> Result<Self> {
        // Validate configuration
        config.validate()?;

        // Initialize telemetry stack
        let telemetry_guard = TelemetryCoordinator::init(
            &config.telemetry,
        )?;

        // Create provider registry
        let providers = ProviderRegistry::new(
            &config.providers,
        ).await?;

        // Initialize routing engine
        let router_engine = Router::new(
            config.routing.clone(),
            providers.clone(),
        ).await?;

        // Create middleware stack
        let middleware = MiddlewareStack::new(
            &config.middleware,
        )?;

        // Build shared state
        let state = Arc::new(GatewayState {
            providers: Arc::new(providers),
            router: Arc::new(router_engine),
            middleware: Arc::new(middleware),
            telemetry: Arc::new(telemetry_guard.coordinator()),
            config: Arc::new(ArcSwap::new(Arc::new(config.clone()))),
            active_requests: Arc::new(ActiveRequestTracker::new()),
            circuit_breakers: Arc::new(CircuitBreakerRegistry::new()),
        });

        // Create Axum router
        let router = create_router(state.clone());

        // Initialize shutdown coordinator
        let shutdown = ShutdownCoordinator::new(
            config.server.shutdown_timeout,
        );

        Ok(Self {
            config: Arc::new(config),
            router,
            state,
            shutdown,
            telemetry_guard,
        })
    }

    /// Run the server until shutdown signal
    ///
    /// # Returns
    /// * `Result<()>` - Success or runtime error
    ///
    /// # Implementation Details
    /// 1. Bind to configured address
    /// 2. Configure TLS if enabled
    /// 3. Start serving requests
    /// 4. Wait for shutdown signal
    /// 5. Drain active connections
    /// 6. Flush telemetry
    pub async fn run(self) -> Result<()> {
        let addr = self.config.server.bind_address
            .parse::<SocketAddr>()?;

        info!(
            address = %addr,
            "Starting LLM-Inference-Gateway"
        );

        // Create TCP listener
        let listener = TcpListener::bind(addr).await
            .context("Failed to bind to address")?;

        // Configure TLS if enabled
        let app = if let Some(tls_config) = &self.config.server.tls {
            self.router
                .layer(tls_layer(tls_config)?)
        } else {
            self.router
        };

        // Serve with graceful shutdown
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>()
        )
        .with_graceful_shutdown(self.shutdown.signal())
        .await
        .context("Server error")?;

        // Post-shutdown cleanup
        self.drain_connections().await?;
        self.state.telemetry.flush().await?;

        info!("Gateway shutdown complete");
        Ok(())
    }

    /// Drain active connections during shutdown
    ///
    /// # Implementation Details
    /// 1. Stop accepting new requests
    /// 2. Wait for active requests to complete (with timeout)
    /// 3. Force-close connections after timeout
    /// 4. Log connection drain statistics
    async fn drain_connections(&self) -> Result<()> {
        let timeout = self.config.server.shutdown_timeout;
        let start = Instant::now();

        info!(
            active_requests = self.state.active_requests.count(),
            timeout_secs = timeout.as_secs(),
            "Draining active connections"
        );

        // Wait for active requests with timeout
        tokio::select! {
            _ = self.state.active_requests.wait_idle() => {
                info!(
                    duration_ms = start.elapsed().as_millis(),
                    "All connections drained gracefully"
                );
            }
            _ = tokio::time::sleep(timeout) => {
                warn!(
                    remaining_requests = self.state.active_requests.count(),
                    "Shutdown timeout reached, force-closing connections"
                );
            }
        }

        Ok(())
    }

    /// Reload configuration without restart
    ///
    /// # Arguments
    /// * `new_config` - Updated configuration
    ///
    /// # Implementation Details
    /// 1. Validate new configuration
    /// 2. Atomically swap configuration
    /// 3. Update components (routing rules, middleware)
    /// 4. Emit configuration change event
    pub async fn reload_config(&self, new_config: GatewayConfig) -> Result<()> {
        // Validate before applying
        new_config.validate()?;

        // Atomically swap configuration
        self.state.config.store(Arc::new(new_config.clone()));

        // Update routing rules
        self.state.router.reload_rules(&new_config.routing).await?;

        // Update middleware configuration
        self.state.middleware.reload(&new_config.middleware).await?;

        // Emit telemetry event
        self.state.telemetry.emit_event(
            TelemetryEvent::ConfigurationReloaded {
                timestamp: Utc::now(),
                config_version: new_config.version,
            }
        );

        info!(
            config_version = new_config.version,
            "Configuration reloaded successfully"
        );

        Ok(())
    }
}

//==============================================================================
// GATEWAY STATE - Shared application state
//==============================================================================

/// Shared state accessible to all handlers
pub struct GatewayState {
    /// Provider registry for backend management
    providers: Arc<ProviderRegistry>,

    /// Routing engine for request distribution
    router: Arc<Router>,

    /// Middleware stack for request processing
    middleware: Arc<MiddlewareStack>,

    /// Telemetry coordinator for observability
    telemetry: Arc<TelemetryCoordinator>,

    /// Hot-reloadable configuration
    config: Arc<ArcSwap<GatewayConfig>>,

    /// Active request tracker for graceful shutdown
    active_requests: Arc<ActiveRequestTracker>,

    /// Circuit breaker registry per provider
    circuit_breakers: Arc<CircuitBreakerRegistry>,
}

impl GatewayState {
    /// Execute a chat completion request through the gateway
    ///
    /// # Arguments
    /// * `request` - Chat completion request
    /// * `context` - Request context (auth, metadata)
    ///
    /// # Returns
    /// * `Result<ChatCompletionResponse>` - Response or error
    ///
    /// # Implementation Details
    /// 1. Convert API request to internal format
    /// 2. Pass through middleware pipeline
    /// 3. Execute routing logic
    /// 4. Send to selected provider
    /// 5. Transform response to API format
    pub async fn execute_chat_completion(
        &self,
        request: ChatCompletionRequest,
        context: RequestContext,
    ) -> Result<ChatCompletionResponse, GatewayError> {
        // Create internal request
        let gateway_request = GatewayRequest::from_chat_completion(
            request,
            context.clone(),
        )?;

        // Execute middleware chain (pre-processing)
        let processed_request = self.middleware
            .execute_chain(gateway_request)
            .await?;

        // Route to provider
        let provider_id = self.router
            .select_provider(&processed_request)
            .await?;

        // Check circuit breaker
        let circuit_breaker = self.circuit_breakers
            .get(&provider_id)?;

        if circuit_breaker.is_open() {
            return Err(GatewayError::CircuitBreakerOpen {
                provider_id: provider_id.clone(),
            });
        }

        // Get provider instance
        let provider = self.providers
            .get(&provider_id)
            .await?;

        // Execute request with timeout
        let response = tokio::time::timeout(
            self.config.load().server.request_timeout,
            provider.send_request(processed_request)
        )
        .await
        .map_err(|_| GatewayError::TimeoutError)??;

        // Record circuit breaker success
        circuit_breaker.record_success();

        // Execute middleware chain (post-processing)
        let processed_response = self.middleware
            .execute_chain_response(response)
            .await?;

        // Convert to API format
        let api_response = ChatCompletionResponse::from_gateway_response(
            processed_response,
        )?;

        Ok(api_response)
    }

    /// Execute a streaming chat completion request
    ///
    /// # Arguments
    /// * `request` - Chat completion request with stream=true
    /// * `context` - Request context
    ///
    /// # Returns
    /// * `Result<Stream>` - SSE stream or error
    pub async fn execute_stream(
        &self,
        request: ChatCompletionRequest,
        context: RequestContext,
    ) -> Result<impl Stream<Item = Result<ChatCompletionChunk>>, GatewayError> {
        // Convert to internal request
        let gateway_request = GatewayRequest::from_chat_completion(
            request,
            context.clone(),
        )?;

        // Middleware preprocessing
        let processed_request = self.middleware
            .execute_chain(gateway_request)
            .await?;

        // Route to provider
        let provider_id = self.router
            .select_provider(&processed_request)
            .await?;

        // Check circuit breaker
        let circuit_breaker = self.circuit_breakers
            .get(&provider_id)?;

        if circuit_breaker.is_open() {
            return Err(GatewayError::CircuitBreakerOpen {
                provider_id: provider_id.clone(),
            });
        }

        // Get provider
        let provider = self.providers
            .get(&provider_id)
            .await?;

        // Get stream from provider
        let provider_stream = provider
            .stream_request(processed_request)
            .await?;

        // Transform stream chunks to API format
        let api_stream = provider_stream
            .map(move |chunk_result| {
                match chunk_result {
                    Ok(chunk) => {
                        circuit_breaker.record_success();
                        ChatCompletionChunk::from_gateway_chunk(chunk)
                    }
                    Err(e) => {
                        circuit_breaker.record_failure();
                        Err(e)
                    }
                }
            });

        Ok(api_stream)
    }
}

//==============================================================================
// SERVER CONFIGURATION
//==============================================================================

/// Server configuration
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct GatewayConfig {
    /// Configuration version for hot reload tracking
    pub version: String,

    /// Server settings
    pub server: ServerConfig,

    /// Provider configurations
    pub providers: Vec<ProviderConfig>,

    /// Routing configuration
    pub routing: RoutingConfig,

    /// Middleware configuration
    pub middleware: MiddlewareConfig,

    /// Telemetry settings
    pub telemetry: TelemetryConfig,
}

/// Server-specific configuration
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct ServerConfig {
    /// Bind address (e.g., "0.0.0.0:8080")
    #[validate(custom = "validate_socket_addr")]
    pub bind_address: String,

    /// TLS configuration (optional)
    pub tls: Option<TlsConfig>,

    /// Request timeout
    #[serde(default = "default_request_timeout")]
    pub request_timeout: Duration,

    /// Graceful shutdown timeout
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout: Duration,

    /// Maximum concurrent connections
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,

    /// Request body size limit
    #[serde(default = "default_body_limit")]
    pub body_limit: usize,

    /// Enable compression
    #[serde(default = "default_true")]
    pub enable_compression: bool,

    /// CORS configuration
    pub cors: Option<CorsConfig>,
}

/// TLS configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TlsConfig {
    /// Path to certificate file
    pub cert_path: PathBuf,

    /// Path to private key file
    pub key_path: PathBuf,

    /// Client certificate verification
    #[serde(default)]
    pub client_auth: ClientAuthMode,
}

/// CORS configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CorsConfig {
    /// Allowed origins
    pub allowed_origins: Vec<String>,

    /// Allowed methods
    #[serde(default = "default_cors_methods")]
    pub allowed_methods: Vec<String>,

    /// Allowed headers
    #[serde(default = "default_cors_headers")]
    pub allowed_headers: Vec<String>,

    /// Max age for preflight cache
    #[serde(default = "default_cors_max_age")]
    pub max_age: Duration,
}

//==============================================================================
// SHUTDOWN COORDINATOR
//==============================================================================

/// Coordinates graceful shutdown across components
pub struct ShutdownCoordinator {
    /// Shutdown signal receiver
    shutdown_rx: watch::Receiver<bool>,

    /// Shutdown signal sender
    shutdown_tx: watch::Sender<bool>,

    /// Shutdown timeout
    timeout: Duration,
}

impl ShutdownCoordinator {
    /// Create a new shutdown coordinator
    pub fn new(timeout: Duration) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        Self {
            shutdown_rx,
            shutdown_tx,
            timeout,
        }
    }

    /// Get a future that resolves on shutdown signal
    ///
    /// # Implementation Details
    /// 1. Listen for SIGINT (Ctrl+C)
    /// 2. Listen for SIGTERM (orchestrator shutdown)
    /// 3. Return when either signal received
    pub async fn signal(&self) {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("Failed to listen for ctrl-c");
            info!("Received SIGINT (Ctrl+C)");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to listen for SIGTERM")
                .recv()
                .await;
            info!("Received SIGTERM");
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }

        // Broadcast shutdown signal
        let _ = self.shutdown_tx.send(true);
    }

    /// Subscribe to shutdown notifications
    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.shutdown_rx.clone()
    }
}

//==============================================================================
// ACTIVE REQUEST TRACKER
//==============================================================================

/// Tracks active requests for graceful shutdown
pub struct ActiveRequestTracker {
    /// Current request count
    count: AtomicUsize,

    /// Notify when count reaches zero
    notify: Notify,
}

impl ActiveRequestTracker {
    pub fn new() -> Self {
        Self {
            count: AtomicUsize::new(0),
            notify: Notify::new(),
        }
    }

    /// Increment active request count
    pub fn increment(&self) -> RequestGuard<'_> {
        self.count.fetch_add(1, Ordering::SeqCst);
        RequestGuard { tracker: self }
    }

    /// Get current request count
    pub fn count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }

    /// Wait until all requests complete
    pub async fn wait_idle(&self) {
        loop {
            if self.count() == 0 {
                return;
            }
            self.notify.notified().await;
        }
    }

    /// Decrement request count (called by guard)
    fn decrement(&self) {
        if self.count.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.notify.notify_waiters();
        }
    }
}

/// RAII guard for request tracking
pub struct RequestGuard<'a> {
    tracker: &'a ActiveRequestTracker,
}

impl Drop for RequestGuard<'_> {
    fn drop(&mut self) {
        self.tracker.decrement();
    }
}
```

---

## Router Configuration

```rust
//==============================================================================
// ROUTER SETUP
//==============================================================================

/// Create the Axum router with all endpoints and middleware
///
/// # Arguments
/// * `state` - Shared gateway state
///
/// # Returns
/// * `Router` - Configured Axum router
///
/// # Implementation Details
/// Routes are organized into logical groups:
/// - OpenAI-compatible endpoints (/v1/*)
/// - Health checks (/health/*)
/// - Metrics (/metrics)
/// - Admin endpoints (/admin/*)
pub fn create_router(state: Arc<GatewayState>) -> Router {
    // OpenAI-compatible API routes
    let api_routes = Router::new()
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route("/v1/completions", post(completions_handler))
        .route("/v1/embeddings", post(embeddings_handler))
        .route("/v1/models", get(list_models_handler))
        .route("/v1/models/:model_id", get(get_model_handler));

    // Health check routes
    let health_routes = Router::new()
        .route("/health/live", get(liveness_handler))
        .route("/health/ready", get(readiness_handler))
        .route("/health/providers", get(provider_health_handler));

    // Metrics routes
    let metrics_routes = Router::new()
        .route("/metrics", get(prometheus_metrics_handler));

    // Admin routes (require admin authentication)
    let admin_routes = Router::new()
        .route("/admin/config",
            get(get_config_handler)
            .post(reload_config_handler))
        .route("/admin/providers",
            get(list_providers_handler)
            .post(register_provider_handler))
        .route("/admin/providers/:id",
            delete(deregister_provider_handler))
        .route("/admin/circuit-breakers",
            get(list_circuit_breakers_handler))
        .route("/admin/circuit-breakers/:id/reset",
            post(reset_circuit_breaker_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            admin_auth_middleware
        ));

    // Combine all routes
    Router::new()
        .merge(api_routes)
        .merge(health_routes)
        .merge(metrics_routes)
        .merge(admin_routes)
        // Global middleware (applied in reverse order)
        .layer(
            ServiceBuilder::new()
                // Request ID generation
                .layer(middleware::from_fn(request_id_middleware))
                // Distributed tracing
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(make_span_with_request_id)
                        .on_request(on_request_callback)
                        .on_response(on_response_callback)
                        .on_failure(on_failure_callback)
                )
                // Request timeout
                .layer(TimeoutLayer::new(state.config.load().server.request_timeout))
                // Compression
                .layer(CompressionLayer::new())
                // CORS
                .layer(cors_layer(&state.config.load().server.cors))
                // Request size limit
                .layer(RequestBodyLimitLayer::new(
                    state.config.load().server.body_limit
                ))
        )
        .with_state(state)
}

//==============================================================================
// MIDDLEWARE IMPLEMENTATIONS
//==============================================================================

/// Request ID middleware - generates unique ID for each request
async fn request_id_middleware<B>(
    mut request: Request<B>,
    next: Next<B>,
) -> Response {
    // Check for existing request ID in headers
    let request_id = request
        .headers()
        .get("X-Request-ID")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // Generate new UUID if not provided
            Uuid::new_v4().to_string()
        });

    // Insert into request extensions
    request.extensions_mut().insert(RequestId(request_id.clone()));

    // Execute request
    let mut response = next.run(request).await;

    // Add request ID to response headers
    response.headers_mut().insert(
        "X-Request-ID",
        HeaderValue::from_str(&request_id).unwrap()
    );

    response
}

/// Admin authentication middleware
async fn admin_auth_middleware(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    request: Request<Body>,
    next: Next<Body>,
) -> Result<Response, ApiError> {
    // Extract Authorization header
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::unauthorized("Missing Authorization header"))?;

    // Validate admin token
    if !state.validate_admin_token(auth_header).await? {
        return Err(ApiError::forbidden("Invalid admin credentials"));
    }

    // Continue to handler
    Ok(next.run(request).await)
}

/// CORS layer configuration
fn cors_layer(config: &Option<CorsConfig>) -> CorsLayer {
    if let Some(cors_config) = config {
        CorsLayer::new()
            .allow_origin(
                cors_config.allowed_origins
                    .iter()
                    .map(|o| o.parse::<HeaderValue>().unwrap())
                    .collect::<Vec<_>>()
            )
            .allow_methods(
                cors_config.allowed_methods
                    .iter()
                    .map(|m| m.parse::<Method>().unwrap())
                    .collect::<Vec<_>>()
            )
            .allow_headers(
                cors_config.allowed_headers
                    .iter()
                    .map(|h| h.parse::<HeaderName>().unwrap())
                    .collect::<Vec<_>>()
            )
            .max_age(cors_config.max_age)
    } else {
        // Permissive default
        CorsLayer::permissive()
    }
}

/// Tracing callbacks
fn make_span_with_request_id(request: &Request<Body>) -> Span {
    let request_id = request
        .extensions()
        .get::<RequestId>()
        .map(|id| id.0.as_str())
        .unwrap_or("unknown");

    tracing::info_span!(
        "http_request",
        method = %request.method(),
        uri = %request.uri(),
        version = ?request.version(),
        request_id = %request_id,
    )
}

fn on_request_callback(request: &Request<Body>, _span: &Span) {
    info!(
        method = %request.method(),
        uri = %request.uri(),
        "Request started"
    );
}

fn on_response_callback(
    response: &Response,
    latency: Duration,
    _span: &Span,
) {
    info!(
        status = response.status().as_u16(),
        latency_ms = latency.as_millis(),
        "Request completed"
    );
}

fn on_failure_callback(
    error: ServerErrorsFailureClass,
    latency: Duration,
    _span: &Span,
) {
    error!(
        error = ?error,
        latency_ms = latency.as_millis(),
        "Request failed"
    );
}
```

---

## Request/Response Types

```rust
//==============================================================================
// OPENAI-COMPATIBLE REQUEST/RESPONSE TYPES
//==============================================================================

//------------------------------------------------------------------------------
// Chat Completions
//------------------------------------------------------------------------------

/// Chat completion request (OpenAI-compatible)
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct ChatCompletionRequest {
    /// Model identifier (e.g., "gpt-4", "claude-3-opus")
    #[validate(length(min = 1, max = 256))]
    pub model: String,

    /// Array of messages in the conversation
    #[validate(length(min = 1, max = 1000))]
    pub messages: Vec<ChatMessage>,

    /// Temperature (0.0 to 2.0)
    #[validate(range(min = 0.0, max = 2.0))]
    #[serde(default)]
    pub temperature: Option<f32>,

    /// Maximum tokens to generate
    #[validate(range(min = 1, max = 1000000))]
    #[serde(default)]
    pub max_tokens: Option<u32>,

    /// Top-p sampling
    #[validate(range(min = 0.0, max = 1.0))]
    #[serde(default)]
    pub top_p: Option<f32>,

    /// Number of completions to generate
    #[validate(range(min = 1, max = 128))]
    #[serde(default = "default_n")]
    pub n: u32,

    /// Enable streaming responses
    #[serde(default)]
    pub stream: bool,

    /// Stop sequences
    #[serde(default)]
    pub stop: Option<StopSequence>,

    /// Presence penalty
    #[validate(range(min = -2.0, max = 2.0))]
    #[serde(default)]
    pub presence_penalty: Option<f32>,

    /// Frequency penalty
    #[validate(range(min = -2.0, max = 2.0))]
    #[serde(default)]
    pub frequency_penalty: Option<f32>,

    /// Logit bias
    #[serde(default)]
    pub logit_bias: Option<HashMap<String, f32>>,

    /// User identifier for tracking
    #[serde(default)]
    pub user: Option<String>,

    /// Tools/functions available to the model
    #[serde(default)]
    pub tools: Option<Vec<Tool>>,

    /// Tool choice strategy
    #[serde(default)]
    pub tool_choice: Option<ToolChoice>,

    /// Response format
    #[serde(default)]
    pub response_format: Option<ResponseFormat>,

    /// Random seed for deterministic generation
    #[serde(default)]
    pub seed: Option<u64>,
}

/// Chat message
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct ChatMessage {
    /// Role: "system", "user", "assistant", "tool"
    #[validate(custom = "validate_role")]
    pub role: String,

    /// Message content
    pub content: MessageContent,

    /// Function/tool name (for role="assistant" with function call)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Tool calls made by the assistant
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    /// Tool call ID (for role="tool")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Message content (string or array of content parts)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Simple text content
    Text(String),

    /// Multi-part content (text + images)
    Parts(Vec<ContentPart>),
}

/// Content part (for multimodal messages)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
    },
    ImageUrl {
        image_url: ImageUrl,
    },
}

/// Image URL for multimodal input
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageUrl {
    /// URL or base64-encoded image
    pub url: String,

    /// Detail level: "low", "high", "auto"
    #[serde(default = "default_image_detail")]
    pub detail: String,
}

/// Tool definition
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Tool {
    /// Tool type (currently only "function")
    #[serde(rename = "type")]
    pub tool_type: String,

    /// Function definition
    pub function: FunctionDefinition,
}

/// Function definition
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionDefinition {
    /// Function name
    pub name: String,

    /// Function description
    pub description: Option<String>,

    /// JSON schema for parameters
    pub parameters: serde_json::Value,
}

/// Tool choice strategy
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// "none", "auto", "required"
    Mode(String),

    /// Specific tool selection
    Tool {
        #[serde(rename = "type")]
        tool_type: String,
        function: FunctionName,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionName {
    pub name: String,
}

/// Response format specification
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponseFormat {
    /// Format type: "text", "json_object", "json_schema"
    #[serde(rename = "type")]
    pub format_type: String,

    /// JSON schema (for json_schema type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,
}

/// Stop sequences
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StopSequence {
    Single(String),
    Multiple(Vec<String>),
}

/// Chat completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    /// Unique response ID
    pub id: String,

    /// Object type ("chat.completion")
    pub object: String,

    /// Creation timestamp (Unix seconds)
    pub created: i64,

    /// Model used for generation
    pub model: String,

    /// Array of completion choices
    pub choices: Vec<Choice>,

    /// Token usage statistics
    pub usage: Usage,

    /// System fingerprint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

impl ChatCompletionResponse {
    /// Convert from internal gateway response
    pub fn from_gateway_response(
        response: GatewayResponse,
    ) -> Result<Self, GatewayError> {
        Ok(Self {
            id: response.id,
            object: "chat.completion".to_string(),
            created: response.created_at.timestamp(),
            model: response.model,
            choices: response.choices
                .into_iter()
                .map(Choice::from_gateway_choice)
                .collect(),
            usage: Usage::from_gateway_usage(response.usage),
            system_fingerprint: response.system_fingerprint,
        })
    }
}

/// Completion choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    /// Choice index
    pub index: u32,

    /// Generated message
    pub message: ChatMessage,

    /// Finish reason: "stop", "length", "tool_calls", "content_filter"
    pub finish_reason: Option<String>,

    /// Log probabilities (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<LogProbs>,
}

impl Choice {
    fn from_gateway_choice(choice: GatewayChoice) -> Self {
        Self {
            index: choice.index,
            message: ChatMessage {
                role: choice.message.role,
                content: MessageContent::Text(choice.message.content),
                name: None,
                tool_calls: choice.message.tool_calls,
                tool_call_id: None,
            },
            finish_reason: choice.finish_reason,
            logprobs: choice.logprobs.map(LogProbs::from_gateway_logprobs),
        }
    }
}

/// Log probabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogProbs {
    /// Token log probabilities
    pub content: Vec<TokenLogProb>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenLogProb {
    /// Token string
    pub token: String,

    /// Log probability
    pub logprob: f32,

    /// Byte offsets
    pub bytes: Option<Vec<u8>>,

    /// Top alternatives
    pub top_logprobs: Vec<TopLogProb>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopLogProb {
    pub token: String,
    pub logprob: f32,
    pub bytes: Option<Vec<u8>>,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// Prompt tokens
    pub prompt_tokens: u32,

    /// Completion tokens
    pub completion_tokens: u32,

    /// Total tokens
    pub total_tokens: u32,

    /// Detailed token counts (extended)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<PromptTokensDetails>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens_details: Option<CompletionTokensDetails>,
}

impl Usage {
    fn from_gateway_usage(usage: GatewayUsage) -> Self {
        Self {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.prompt_tokens + usage.completion_tokens,
            prompt_tokens_details: None,
            completion_tokens_details: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTokensDetails {
    pub cached_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionTokensDetails {
    pub reasoning_tokens: u32,
}

//------------------------------------------------------------------------------
// Streaming Response Types
//------------------------------------------------------------------------------

/// Chat completion chunk (for streaming)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    /// Unique ID
    pub id: String,

    /// Object type ("chat.completion.chunk")
    pub object: String,

    /// Creation timestamp
    pub created: i64,

    /// Model identifier
    pub model: String,

    /// Delta choices
    pub choices: Vec<ChoiceDelta>,

    /// System fingerprint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

impl ChatCompletionChunk {
    /// Convert from internal gateway chunk
    pub fn from_gateway_chunk(
        chunk: GatewayChunk,
    ) -> Result<Self, GatewayError> {
        Ok(Self {
            id: chunk.id,
            object: "chat.completion.chunk".to_string(),
            created: chunk.created_at.timestamp(),
            model: chunk.model,
            choices: chunk.choices
                .into_iter()
                .map(ChoiceDelta::from_gateway_delta)
                .collect(),
            system_fingerprint: chunk.system_fingerprint,
        })
    }
}

/// Choice delta (incremental content)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceDelta {
    /// Choice index
    pub index: u32,

    /// Delta message
    pub delta: MessageDelta,

    /// Finish reason (only in final chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,

    /// Log probabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<LogProbs>,
}

impl ChoiceDelta {
    fn from_gateway_delta(delta: GatewayDelta) -> Self {
        Self {
            index: delta.index,
            delta: MessageDelta {
                role: delta.role,
                content: delta.content,
                tool_calls: delta.tool_calls,
            },
            finish_reason: delta.finish_reason,
            logprobs: None,
        }
    }
}

/// Message delta (incremental message)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDelta {
    /// Role (only in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    /// Incremental content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Incremental tool calls
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

/// Tool call delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: u32,
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub tool_type: Option<String>,
    pub function: Option<FunctionCallDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallDelta {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

//------------------------------------------------------------------------------
// Text Completions (Legacy)
//------------------------------------------------------------------------------

/// Text completion request (legacy)
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct CompletionRequest {
    pub model: String,

    #[serde(default)]
    pub prompt: PromptInput,

    #[validate(range(min = 1, max = 1000000))]
    #[serde(default)]
    pub max_tokens: Option<u32>,

    #[validate(range(min = 0.0, max = 2.0))]
    #[serde(default)]
    pub temperature: Option<f32>,

    #[serde(default)]
    pub stream: bool,

    // ... other fields similar to ChatCompletionRequest
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PromptInput {
    Single(String),
    Multiple(Vec<String>),
}

impl Default for PromptInput {
    fn default() -> Self {
        PromptInput::Single(String::new())
    }
}

/// Completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<CompletionChoice>,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionChoice {
    pub text: String,
    pub index: u32,
    pub logprobs: Option<LogProbs>,
    pub finish_reason: Option<String>,
}

//------------------------------------------------------------------------------
// Embeddings
//------------------------------------------------------------------------------

/// Embeddings request
#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct EmbeddingsRequest {
    pub model: String,
    pub input: EmbeddingInput,

    #[serde(default)]
    pub encoding_format: Option<String>,

    #[serde(default)]
    pub dimensions: Option<u32>,

    #[serde(default)]
    pub user: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    Single(String),
    Multiple(Vec<String>),
    TokenArray(Vec<Vec<u32>>),
}

/// Embeddings response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsResponse {
    pub object: String,
    pub data: Vec<Embedding>,
    pub model: String,
    pub usage: EmbeddingUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    pub object: String,
    pub embedding: Vec<f32>,
    pub index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    pub prompt_tokens: u32,
    pub total_tokens: u32,
}

//------------------------------------------------------------------------------
// Models API
//------------------------------------------------------------------------------

/// List models response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListModelsResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

//------------------------------------------------------------------------------
// Tool Calls
//------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}
```

---

## API Handler Implementations

```rust
//==============================================================================
// CHAT COMPLETIONS HANDLER
//==============================================================================

/// Handle chat completion requests (streaming and non-streaming)
///
/// # Arguments
/// * `State(state)` - Shared gateway state
/// * `headers` - Request headers for auth extraction
/// * `Json(request)` - Validated request body
///
/// # Returns
/// * `Result<Response>` - Streaming or non-streaming response
///
/// # Implementation Details
/// 1. Extract authentication from headers
/// 2. Validate request against schema
/// 3. Build request context (user, request ID, metadata)
/// 4. Route to streaming or non-streaming handler
/// 5. Track request metrics
#[axum::debug_handler]
pub async fn chat_completions_handler(
    State(state): State<Arc<GatewayState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    request_id: RequestId,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Response, ApiError> {
    // Track active request
    let _request_guard = state.active_requests.increment();

    // Start telemetry span
    let span = info_span!(
        "chat_completions",
        request_id = %request_id.0,
        model = %request.model,
        stream = request.stream,
    );

    async move {
        // Extract authentication
        let auth = extract_auth(&headers)?;

        // Validate request
        request.validate()
            .map_err(|e| ApiError::validation_error(e))?;

        // Build request context
        let context = RequestContext {
            request_id: request_id.0.clone(),
            auth,
            client_addr: addr,
            headers: headers.clone(),
            timestamp: Utc::now(),
            user: request.user.clone(),
        };

        // Emit request metric
        state.telemetry.record_request(
            "chat.completions",
            &request.model,
            &context,
        );

        // Route based on streaming mode
        if request.stream {
            // Streaming response
            chat_completions_stream(state, request, context)
                .await
                .map(|sse| sse.into_response())
        } else {
            // Non-streaming response
            chat_completions_non_stream(state, request, context)
                .await
                .map(|response| Json(response).into_response())
        }
    }
    .instrument(span)
    .await
}

/// Handle non-streaming chat completion
async fn chat_completions_non_stream(
    state: Arc<GatewayState>,
    request: ChatCompletionRequest,
    context: RequestContext,
) -> Result<ChatCompletionResponse, ApiError> {
    // Execute request through gateway
    let response = state
        .execute_chat_completion(request, context)
        .await
        .map_err(ApiError::from)?;

    // Emit response metric
    state.telemetry.record_response(
        "chat.completions",
        &response.model,
        response.usage.total_tokens,
        true,
    );

    Ok(response)
}

/// Handle streaming chat completion
async fn chat_completions_stream(
    state: Arc<GatewayState>,
    request: ChatCompletionRequest,
    context: RequestContext,
) -> Result<Sse<impl Stream<Item = Result<Event, ApiError>>>, ApiError> {
    // Get stream from gateway
    let stream = state
        .execute_stream(request.clone(), context.clone())
        .await
        .map_err(ApiError::from)?;

    // Transform to SSE events
    let sse_stream = stream
        .then(move |chunk_result| {
            let state = state.clone();
            let model = request.model.clone();

            async move {
                match chunk_result {
                    Ok(chunk) => {
                        // Serialize chunk to JSON
                        let json = serde_json::to_string(&chunk)
                            .map_err(|e| ApiError::serialization_error(e))?;

                        // Create SSE event
                        Ok(Event::default().data(json))
                    }
                    Err(e) => {
                        // Emit error metric
                        state.telemetry.record_error(
                            "chat.completions.stream",
                            &model,
                            &e,
                        );

                        Err(ApiError::from(e))
                    }
                }
            }
        })
        .chain(stream::once(async {
            // Send [DONE] marker
            Ok(Event::default().data("[DONE]"))
        }));

    Ok(Sse::new(sse_stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keepalive")
        ))
}

//==============================================================================
// TEXT COMPLETIONS HANDLER (Legacy)
//==============================================================================

#[axum::debug_handler]
pub async fn completions_handler(
    State(state): State<Arc<GatewayState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    request_id: RequestId,
    Json(request): Json<CompletionRequest>,
) -> Result<Response, ApiError> {
    let _request_guard = state.active_requests.increment();

    // Validate request
    request.validate()
        .map_err(|e| ApiError::validation_error(e))?;

    // Extract auth
    let auth = extract_auth(&headers)?;

    // Build context
    let context = RequestContext {
        request_id: request_id.0.clone(),
        auth,
        client_addr: addr,
        headers: headers.clone(),
        timestamp: Utc::now(),
        user: request.user.clone(),
    };

    // Convert to internal format
    let gateway_request = GatewayRequest::from_completion(
        request.clone(),
        context.clone(),
    )?;

    // Execute
    if request.stream {
        // Streaming
        let stream = state
            .execute_stream_completion(gateway_request)
            .await
            .map_err(ApiError::from)?;

        let sse_stream = stream
            .map(|chunk_result| {
                chunk_result
                    .and_then(|chunk| {
                        let json = serde_json::to_string(&chunk)?;
                        Ok(Event::default().data(json))
                    })
                    .map_err(ApiError::from)
            })
            .chain(stream::once(async {
                Ok(Event::default().data("[DONE]"))
            }));

        Ok(Sse::new(sse_stream)
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
            .into_response())
    } else {
        // Non-streaming
        let response = state
            .execute_completion(gateway_request)
            .await
            .map_err(ApiError::from)?;

        Ok(Json(response).into_response())
    }
}

//==============================================================================
// EMBEDDINGS HANDLER
//==============================================================================

#[axum::debug_handler]
pub async fn embeddings_handler(
    State(state): State<Arc<GatewayState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    request_id: RequestId,
    Json(request): Json<EmbeddingsRequest>,
) -> Result<Json<EmbeddingsResponse>, ApiError> {
    let _request_guard = state.active_requests.increment();

    // Validate
    request.validate()
        .map_err(|e| ApiError::validation_error(e))?;

    // Extract auth
    let auth = extract_auth(&headers)?;

    // Build context
    let context = RequestContext {
        request_id: request_id.0.clone(),
        auth,
        client_addr: addr,
        headers,
        timestamp: Utc::now(),
        user: request.user.clone(),
    };

    // Convert to internal format
    let gateway_request = GatewayRequest::from_embeddings(
        request,
        context,
    )?;

    // Execute
    let response = state
        .execute_embeddings(gateway_request)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(response))
}

//==============================================================================
// MODELS API HANDLERS
//==============================================================================

/// List available models
#[axum::debug_handler]
pub async fn list_models_handler(
    State(state): State<Arc<GatewayState>>,
) -> Result<Json<ListModelsResponse>, ApiError> {
    // Get models from all providers
    let models = state.providers
        .list_all_models()
        .await
        .map_err(ApiError::from)?;

    let model_list = models
        .into_iter()
        .map(|model| ModelInfo {
            id: model.id,
            object: "model".to_string(),
            created: model.created_at.timestamp(),
            owned_by: model.provider_id,
        })
        .collect();

    Ok(Json(ListModelsResponse {
        object: "list".to_string(),
        data: model_list,
    }))
}

/// Get specific model information
#[axum::debug_handler]
pub async fn get_model_handler(
    State(state): State<Arc<GatewayState>>,
    Path(model_id): Path<String>,
) -> Result<Json<ModelInfo>, ApiError> {
    // Get model info
    let model = state.providers
        .get_model(&model_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("Model not found"))?;

    Ok(Json(ModelInfo {
        id: model.id,
        object: "model".to_string(),
        created: model.created_at.timestamp(),
        owned_by: model.provider_id,
    }))
}

//==============================================================================
// AUTHENTICATION EXTRACTION
//==============================================================================

/// Extract authentication from headers
///
/// # Implementation Details
/// Supports multiple authentication methods:
/// 1. Bearer token: `Authorization: Bearer <token>`
/// 2. API key header: `X-API-Key: <key>`
/// 3. Basic auth: `Authorization: Basic <base64>`
fn extract_auth(headers: &HeaderMap) -> Result<Authentication, ApiError> {
    // Try Authorization header first
    if let Some(auth_header) = headers.get(header::AUTHORIZATION) {
        let auth_str = auth_header
            .to_str()
            .map_err(|_| ApiError::invalid_auth_header())?;

        if let Some(token) = auth_str.strip_prefix("Bearer ") {
            return Ok(Authentication::Bearer {
                token: token.to_string(),
            });
        }

        if let Some(basic) = auth_str.strip_prefix("Basic ") {
            let decoded = base64::decode(basic)
                .map_err(|_| ApiError::invalid_auth_header())?;
            let credentials = String::from_utf8(decoded)
                .map_err(|_| ApiError::invalid_auth_header())?;

            let parts: Vec<&str> = credentials.splitn(2, ':').collect();
            if parts.len() != 2 {
                return Err(ApiError::invalid_auth_header());
            }

            return Ok(Authentication::Basic {
                username: parts[0].to_string(),
                password: parts[1].to_string(),
            });
        }
    }

    // Try X-API-Key header
    if let Some(api_key) = headers.get("X-API-Key") {
        let key = api_key
            .to_str()
            .map_err(|_| ApiError::invalid_auth_header())?;

        return Ok(Authentication::ApiKey {
            key: key.to_string(),
        });
    }

    // No authentication provided
    Err(ApiError::missing_auth())
}

/// Authentication types
#[derive(Debug, Clone)]
pub enum Authentication {
    Bearer { token: String },
    ApiKey { key: String },
    Basic { username: String, password: String },
}

//==============================================================================
// REQUEST CONTEXT
//==============================================================================

/// Request context passed through middleware
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Unique request ID
    pub request_id: String,

    /// Authentication info
    pub auth: Authentication,

    /// Client address
    pub client_addr: SocketAddr,

    /// Original headers
    pub headers: HeaderMap,

    /// Request timestamp
    pub timestamp: DateTime<Utc>,

    /// User identifier
    pub user: Option<String>,
}

/// Request ID wrapper
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for RequestId
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<RequestId>()
            .cloned()
            .ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Missing request ID".to_string(),
                )
            })
    }
}
```

---

## Streaming Support

```rust
//==============================================================================
// SERVER-SENT EVENTS (SSE) STREAMING
//==============================================================================

/// Create SSE stream from provider stream
///
/// # Implementation Details
/// 1. Receive chunks from provider stream
/// 2. Transform to OpenAI-compatible format
/// 3. Serialize to JSON
/// 4. Wrap in SSE event format: "data: {json}\n\n"
/// 5. Handle errors gracefully
/// 6. Send [DONE] marker at end
pub fn create_sse_stream(
    provider_stream: impl Stream<Item = Result<GatewayChunk, GatewayError>>,
    request_id: String,
    model: String,
    telemetry: Arc<TelemetryCoordinator>,
) -> impl Stream<Item = Result<Event, ApiError>> {
    let stream = provider_stream
        .enumerate()
        .then(move |(index, chunk_result)| {
            let request_id = request_id.clone();
            let model = model.clone();
            let telemetry = telemetry.clone();

            async move {
                match chunk_result {
                    Ok(gateway_chunk) => {
                        // Convert to API format
                        let api_chunk = ChatCompletionChunk::from_gateway_chunk(
                            gateway_chunk
                        )?;

                        // Serialize to JSON
                        let json = serde_json::to_string(&api_chunk)
                            .map_err(|e| ApiError::serialization_error(e))?;

                        // Record chunk metric
                        telemetry.record_stream_chunk(
                            &request_id,
                            &model,
                            index,
                        );

                        // Create SSE event
                        Ok(Event::default()
                            .data(json)
                            .id(index.to_string()))
                    }
                    Err(e) => {
                        // Log error
                        error!(
                            request_id = %request_id,
                            error = ?e,
                            "Stream error"
                        );

                        // Record error metric
                        telemetry.record_stream_error(
                            &request_id,
                            &model,
                            &e,
                        );

                        Err(ApiError::from(e))
                    }
                }
            }
        })
        // Add [DONE] marker
        .chain(stream::once(async {
            Ok(Event::default().data("[DONE]"))
        }));

    stream
}

/// Keep-alive configuration for SSE
///
/// Sends periodic keepalive messages to prevent connection timeout
pub fn keepalive_config() -> KeepAlive {
    KeepAlive::new()
        .interval(Duration::from_secs(15))
        .text("keepalive")
}

//==============================================================================
// STREAM ERROR HANDLING
//==============================================================================

/// Wrap stream with error recovery
///
/// # Implementation Details
/// 1. Detect stream errors
/// 2. Attempt provider failover if configured
/// 3. Send error event to client
/// 4. Close stream gracefully
pub fn with_error_recovery<S>(
    stream: S,
    state: Arc<GatewayState>,
    request: ChatCompletionRequest,
    context: RequestContext,
) -> impl Stream<Item = Result<Event, ApiError>>
where
    S: Stream<Item = Result<Event, ApiError>>,
{
    stream
        .scan(
            (false, 0usize),
            move |state_tuple, item| {
                let (errored, chunk_count) = state_tuple;

                match item {
                    Ok(event) => {
                        *chunk_count += 1;
                        future::ready(Some(Ok(event)))
                    }
                    Err(e) if !*errored => {
                        // First error - attempt recovery
                        *errored = true;

                        // If we've already sent chunks, can't retry
                        if *chunk_count > 0 {
                            // Send error event
                            let error_event = create_error_event(&e);
                            future::ready(Some(Ok(error_event)))
                        } else {
                            // No chunks sent yet - could retry with different provider
                            // (This would require re-initiating the stream)
                            future::ready(Some(Err(e)))
                        }
                    }
                    Err(e) => {
                        // Subsequent error - terminate
                        future::ready(None)
                    }
                }
            }
        )
}

/// Create error event for SSE stream
fn create_error_event(error: &ApiError) -> Event {
    let error_json = serde_json::json!({
        "error": {
            "type": error.error_type,
            "message": error.message,
            "code": error.code,
        }
    });

    Event::default()
        .event("error")
        .data(error_json.to_string())
}

//==============================================================================
// BACKPRESSURE HANDLING
//==============================================================================

/// Apply backpressure to stream based on client consumption rate
///
/// # Implementation Details
/// 1. Buffer chunks with size limit
/// 2. Apply timeout to send operations
/// 3. Drop stream if client is too slow
pub fn with_backpressure<S>(
    stream: S,
    buffer_size: usize,
    send_timeout: Duration,
) -> impl Stream<Item = Result<Event, ApiError>>
where
    S: Stream<Item = Result<Event, ApiError>>,
{
    stream
        .buffer_unordered(buffer_size)
        .timeout(send_timeout)
        .map(|result| {
            match result {
                Ok(event) => event,
                Err(_timeout) => {
                    Err(ApiError::stream_timeout(
                        "Client not consuming stream fast enough"
                    ))
                }
            }
        })
}
```

---

## Error Handling

```rust
//==============================================================================
// API ERROR TYPES
//==============================================================================

/// API error with HTTP status code and OpenAI-compatible format
#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    /// HTTP status code
    #[serde(skip)]
    pub status: StatusCode,

    /// Error type (OpenAI-compatible)
    #[serde(rename = "type")]
    pub error_type: String,

    /// Human-readable message
    pub message: String,

    /// Machine-readable error code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    /// Parameter that caused the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
}

impl ApiError {
    // Constructors for common errors

    pub fn validation_error(err: validator::ValidationErrors) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            error_type: "invalid_request_error".to_string(),
            message: format!("Validation failed: {}", err),
            code: Some("validation_error".to_string()),
            param: err.field_errors()
                .keys()
                .next()
                .map(|s| s.to_string()),
        }
    }

    pub fn missing_auth() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            error_type: "authentication_error".to_string(),
            message: "Missing authentication credentials".to_string(),
            code: Some("missing_api_key".to_string()),
            param: Some("Authorization".to_string()),
        }
    }

    pub fn invalid_auth_header() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            error_type: "authentication_error".to_string(),
            message: "Invalid Authorization header format".to_string(),
            code: Some("invalid_auth_header".to_string()),
            param: Some("Authorization".to_string()),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            error_type: "authentication_error".to_string(),
            message: message.into(),
            code: Some("invalid_api_key".to_string()),
            param: None,
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            error_type: "authorization_error".to_string(),
            message: message.into(),
            code: Some("insufficient_permissions".to_string()),
            param: None,
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            error_type: "not_found_error".to_string(),
            message: message.into(),
            code: Some("resource_not_found".to_string()),
            param: None,
        }
    }

    pub fn rate_limit(retry_after: u64) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            error_type: "rate_limit_error".to_string(),
            message: format!("Rate limit exceeded. Retry after {} seconds", retry_after),
            code: Some("rate_limit_exceeded".to_string()),
            param: None,
        }
    }

    pub fn payload_too_large(max_size: usize) -> Self {
        Self {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            error_type: "invalid_request_error".to_string(),
            message: format!("Request body exceeds maximum size of {} bytes", max_size),
            code: Some("payload_too_large".to_string()),
            param: None,
        }
    }

    pub fn serialization_error(err: serde_json::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error_type: "internal_error".to_string(),
            message: format!("Serialization error: {}", err),
            code: Some("serialization_error".to_string()),
            param: None,
        }
    }

    pub fn stream_timeout(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::REQUEST_TIMEOUT,
            error_type: "timeout_error".to_string(),
            message: message.into(),
            code: Some("stream_timeout".to_string()),
            param: None,
        }
    }
}

//==============================================================================
// ERROR RESPONSE FORMATTING
//==============================================================================

/// Convert ApiError to HTTP response
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Create error body in OpenAI format
        let body = serde_json::json!({
            "error": {
                "type": self.error_type,
                "message": self.message,
                "code": self.code,
                "param": self.param,
            }
        });

        // Build response with status code and JSON body
        let mut response = (self.status, Json(body)).into_response();

        // Add retry-after header for rate limits
        if self.status == StatusCode::TOO_MANY_REQUESTS {
            if let Some(retry_msg) = &self.message {
                if let Some(seconds) = extract_retry_seconds(retry_msg) {
                    response.headers_mut().insert(
                        header::RETRY_AFTER,
                        HeaderValue::from_str(&seconds.to_string()).unwrap()
                    );
                }
            }
        }

        response
    }
}

/// Extract retry seconds from rate limit message
fn extract_retry_seconds(message: &str) -> Option<u64> {
    // Parse "Retry after N seconds"
    message
        .split_whitespace()
        .nth(2)
        .and_then(|s| s.parse::<u64>().ok())
}

//==============================================================================
// GATEWAY ERROR CONVERSION
//==============================================================================

/// Convert internal GatewayError to API error
impl From<GatewayError> for ApiError {
    fn from(error: GatewayError) -> Self {
        match error {
            // Validation errors
            GatewayError::ValidationError(e) => Self {
                status: StatusCode::BAD_REQUEST,
                error_type: "invalid_request_error".to_string(),
                message: e.to_string(),
                code: Some("validation_error".to_string()),
                param: e.field(),
            },

            // Authentication errors
            GatewayError::AuthenticationError(e) => Self {
                status: StatusCode::UNAUTHORIZED,
                error_type: "authentication_error".to_string(),
                message: e.to_string(),
                code: Some("invalid_api_key".to_string()),
                param: None,
            },

            // Authorization errors
            GatewayError::AuthorizationError(e) => Self {
                status: StatusCode::FORBIDDEN,
                error_type: "authorization_error".to_string(),
                message: e.to_string(),
                code: Some("insufficient_permissions".to_string()),
                param: None,
            },

            // Rate limiting
            GatewayError::RateLimitError { retry_after, .. } => Self {
                status: StatusCode::TOO_MANY_REQUESTS,
                error_type: "rate_limit_error".to_string(),
                message: format!("Rate limit exceeded. Retry after {} seconds", retry_after),
                code: Some("rate_limit_exceeded".to_string()),
                param: None,
            },

            // Model/resource not found
            GatewayError::ModelNotFound(model) => Self {
                status: StatusCode::NOT_FOUND,
                error_type: "not_found_error".to_string(),
                message: format!("Model '{}' not found", model),
                code: Some("model_not_found".to_string()),
                param: Some("model".to_string()),
            },

            GatewayError::ProviderNotFound(provider_id) => Self {
                status: StatusCode::NOT_FOUND,
                error_type: "not_found_error".to_string(),
                message: format!("Provider '{}' not found", provider_id),
                code: Some("provider_not_found".to_string()),
                param: None,
            },

            // Provider errors
            GatewayError::ProviderError { provider_id, error } => {
                // Map provider-specific errors
                match error {
                    ProviderError::InvalidRequest(msg) => Self {
                        status: StatusCode::BAD_REQUEST,
                        error_type: "invalid_request_error".to_string(),
                        message: msg,
                        code: Some("provider_invalid_request".to_string()),
                        param: None,
                    },
                    ProviderError::AuthenticationFailed => Self {
                        status: StatusCode::UNAUTHORIZED,
                        error_type: "authentication_error".to_string(),
                        message: format!("Authentication failed for provider '{}'", provider_id),
                        code: Some("provider_auth_failed".to_string()),
                        param: None,
                    },
                    ProviderError::QuotaExceeded => Self {
                        status: StatusCode::TOO_MANY_REQUESTS,
                        error_type: "rate_limit_error".to_string(),
                        message: format!("Provider '{}' quota exceeded", provider_id),
                        code: Some("provider_quota_exceeded".to_string()),
                        param: None,
                    },
                    ProviderError::ServiceUnavailable => Self {
                        status: StatusCode::BAD_GATEWAY,
                        error_type: "provider_error".to_string(),
                        message: format!("Provider '{}' is unavailable", provider_id),
                        code: Some("provider_unavailable".to_string()),
                        param: None,
                    },
                    _ => Self {
                        status: StatusCode::BAD_GATEWAY,
                        error_type: "provider_error".to_string(),
                        message: format!("Provider '{}' error: {}", provider_id, error),
                        code: Some("provider_error".to_string()),
                        param: None,
                    },
                }
            },

            // Timeout errors
            GatewayError::TimeoutError => Self {
                status: StatusCode::GATEWAY_TIMEOUT,
                error_type: "timeout_error".to_string(),
                message: "Request timed out".to_string(),
                code: Some("request_timeout".to_string()),
                param: None,
            },

            // Circuit breaker
            GatewayError::CircuitBreakerOpen { provider_id } => Self {
                status: StatusCode::SERVICE_UNAVAILABLE,
                error_type: "service_unavailable_error".to_string(),
                message: format!(
                    "Provider '{}' is temporarily unavailable (circuit breaker open)",
                    provider_id
                ),
                code: Some("circuit_breaker_open".to_string()),
                param: None,
            },

            // Configuration errors
            GatewayError::ConfigurationError(msg) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                error_type: "internal_error".to_string(),
                message: format!("Configuration error: {}", msg),
                code: Some("configuration_error".to_string()),
                param: None,
            },

            // Internal errors
            GatewayError::InternalError(msg) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                error_type: "internal_error".to_string(),
                message: "An internal error occurred".to_string(),
                code: Some("internal_error".to_string()),
                param: None,
            },

            // Catch-all
            _ => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                error_type: "internal_error".to_string(),
                message: "An unexpected error occurred".to_string(),
                code: Some("unknown_error".to_string()),
                param: None,
            },
        }
    }
}

//==============================================================================
// STANDARD ERROR CONVERSIONS
//==============================================================================

/// Convert serde_json errors
impl From<serde_json::Error> for ApiError {
    fn from(error: serde_json::Error) -> Self {
        Self::serialization_error(error)
    }
}

/// Convert validation errors
impl From<validator::ValidationErrors> for ApiError {
    fn from(error: validator::ValidationErrors) -> Self {
        Self::validation_error(error)
    }
}
```

(Continued in next message due to length...)

---

## Middleware Stack

```rust
//==============================================================================
// MIDDLEWARE STACK
//==============================================================================

/// Composable middleware pipeline
pub struct MiddlewareStack {
    /// Authentication middleware
    auth: Option<Arc<dyn AuthMiddleware>>,

    /// Rate limiting middleware
    rate_limit: Option<Arc<dyn RateLimitMiddleware>>,

    /// Request logging middleware
    logging: Option<Arc<dyn LoggingMiddleware>>,

    /// Request transformation middleware
    transform: Option<Arc<dyn TransformMiddleware>>,

    /// Content filtering middleware
    content_filter: Option<Arc<dyn ContentFilterMiddleware>>,
}

impl MiddlewareStack {
    /// Create middleware stack from configuration
    pub fn new(config: &MiddlewareConfig) -> Result<Self> {
        Ok(Self {
            auth: if config.auth.enabled {
                Some(Arc::new(AuthMiddleware::new(&config.auth)?))
            } else {
                None
            },
            rate_limit: if config.rate_limit.enabled {
                Some(Arc::new(RateLimitMiddleware::new(&config.rate_limit)?))
            } else {
                None
            },
            logging: if config.logging.enabled {
                Some(Arc::new(LoggingMiddleware::new(&config.logging)?))
            } else {
                None
            },
            transform: if config.transform.enabled {
                Some(Arc::new(TransformMiddleware::new(&config.transform)?))
            } else {
                None
            },
            content_filter: if config.content_filter.enabled {
                Some(Arc::new(ContentFilterMiddleware::new(&config.content_filter)?))
            } else {
                None
            },
        })
    }

    /// Execute middleware chain (pre-processing)
    pub async fn execute_chain(
        &self,
        mut request: GatewayRequest,
    ) -> Result<GatewayRequest, GatewayError> {
        // Authentication
        if let Some(auth) = &self.auth {
            request = auth.process(request).await?;
        }

        // Rate limiting
        if let Some(rate_limit) = &self.rate_limit {
            request = rate_limit.process(request).await?;
        }

        // Content filtering (input)
        if let Some(filter) = &self.content_filter {
            request = filter.process_request(request).await?;
        }

        // Request transformation
        if let Some(transform) = &self.transform {
            request = transform.process(request).await?;
        }

        // Logging
        if let Some(logging) = &self.logging {
            logging.log_request(&request).await;
        }

        Ok(request)
    }

    /// Execute middleware chain (post-processing)
    pub async fn execute_chain_response(
        &self,
        mut response: GatewayResponse,
    ) -> Result<GatewayResponse, GatewayError> {
        // Content filtering (output)
        if let Some(filter) = &self.content_filter {
            response = filter.process_response(response).await?;
        }

        // Logging
        if let Some(logging) = &self.logging {
            logging.log_response(&response).await;
        }

        Ok(response)
    }

    /// Reload middleware configuration
    pub async fn reload(&self, config: &MiddlewareConfig) -> Result<()> {
        // Hot reload each middleware component
        if let Some(auth) = &self.auth {
            auth.reload(&config.auth).await?;
        }

        if let Some(rate_limit) = &self.rate_limit {
            rate_limit.reload(&config.rate_limit).await?;
        }

        // ... reload other middlewares

        Ok(())
    }
}
```

---

## Health and Admin Endpoints

```rust
//==============================================================================
// HEALTH CHECK ENDPOINTS
//==============================================================================

/// Liveness probe - always returns 200 if server is running
///
/// Used by Kubernetes liveness probes
#[axum::debug_handler]
pub async fn liveness_handler() -> impl IntoResponse {
    (StatusCode::OK, "alive")
}

/// Readiness probe - returns 200 only when ready to serve traffic
///
/// Checks:
/// - All providers are reachable
/// - Database connections are healthy (if applicable)
/// - Required dependencies are available
#[axum::debug_handler]
pub async fn readiness_handler(
    State(state): State<Arc<GatewayState>>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if gateway is ready
    let health = state.telemetry.health
        .check_readiness()
        .await;

    if health.is_ready {
        Ok((StatusCode::OK, Json(serde_json::json!({
            "status": "ready",
            "checks": health.checks,
        }))))
    } else {
        Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            error_type: "service_unavailable".to_string(),
            message: "Service not ready".to_string(),
            code: Some("not_ready".to_string()),
            param: None,
        })
    }
}

/// Provider health endpoint - detailed health of all providers
#[axum::debug_handler]
pub async fn provider_health_handler(
    State(state): State<Arc<GatewayState>>,
) -> Json<ProviderHealthResponse> {
    let provider_health = state.providers
        .get_all_health()
        .await;

    Json(ProviderHealthResponse {
        providers: provider_health,
    })
}

#[derive(Debug, Serialize)]
pub struct ProviderHealthResponse {
    pub providers: Vec<ProviderHealth>,
}

#[derive(Debug, Serialize)]
pub struct ProviderHealth {
    pub id: String,
    pub status: HealthStatus,
    pub latency_ms: Option<u64>,
    pub error_rate: f32,
    pub circuit_breaker_state: CircuitBreakerState,
    pub last_check: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

//==============================================================================
// METRICS ENDPOINT
//==============================================================================

/// Prometheus metrics endpoint
#[axum::debug_handler]
pub async fn prometheus_metrics_handler(
    State(state): State<Arc<GatewayState>>,
) -> impl IntoResponse {
    // Gather metrics from telemetry coordinator
    let metrics = state.telemetry
        .gather_prometheus_metrics()
        .await;

    // Return in Prometheus text format
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        metrics,
    )
}

//==============================================================================
// ADMIN ENDPOINTS
//==============================================================================

/// Get current configuration
#[axum::debug_handler]
pub async fn get_config_handler(
    State(state): State<Arc<GatewayState>>,
) -> Json<GatewayConfig> {
    let config = state.config.load();
    Json((**config).clone())
}

/// Reload configuration
#[axum::debug_handler]
pub async fn reload_config_handler(
    State(state): State<Arc<GatewayState>>,
    Json(new_config): Json<GatewayConfig>,
) -> Result<Json<ConfigReloadResponse>, ApiError> {
    // Validate configuration
    new_config.validate()
        .map_err(ApiError::validation_error)?;

    // Apply configuration
    state.reload_config(new_config.clone())
        .await
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error_type: "configuration_error".to_string(),
            message: format!("Failed to reload configuration: {}", e),
            code: Some("reload_failed".to_string()),
            param: None,
        })?;

    Ok(Json(ConfigReloadResponse {
        success: true,
        version: new_config.version,
        reloaded_at: Utc::now(),
    }))
}

#[derive(Debug, Serialize)]
pub struct ConfigReloadResponse {
    pub success: bool,
    pub version: String,
    pub reloaded_at: DateTime<Utc>,
}

/// List all registered providers
#[axum::debug_handler]
pub async fn list_providers_handler(
    State(state): State<Arc<GatewayState>>,
) -> Json<Vec<ProviderInfo>> {
    let providers = state.providers
        .list_all()
        .await;

    Json(providers)
}

/// Register a new provider
#[axum::debug_handler]
pub async fn register_provider_handler(
    State(state): State<Arc<GatewayState>>,
    Json(provider_config): Json<ProviderConfig>,
) -> Result<Json<ProviderInfo>, ApiError> {
    let provider_info = state.providers
        .register(provider_config)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(provider_info))
}

/// Deregister a provider
#[axum::debug_handler]
pub async fn deregister_provider_handler(
    State(state): State<Arc<GatewayState>>,
    Path(provider_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.providers
        .deregister(&provider_id)
        .await
        .map_err(ApiError::from)?;

    Ok(StatusCode::NO_CONTENT)
}

/// List circuit breaker states
#[axum::debug_handler]
pub async fn list_circuit_breakers_handler(
    State(state): State<Arc<GatewayState>>,
) -> Json<Vec<CircuitBreakerInfo>> {
    let circuit_breakers = state.circuit_breakers
        .list_all()
        .await;

    Json(circuit_breakers)
}

/// Reset a circuit breaker
#[axum::debug_handler]
pub async fn reset_circuit_breaker_handler(
    State(state): State<Arc<GatewayState>>,
    Path(provider_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.circuit_breakers
        .reset(&provider_id)
        .await
        .map_err(ApiError::from)?;

    Ok(StatusCode::NO_CONTENT)
}
```

---

## Request Validation and Extraction

```rust
//==============================================================================
// REQUEST VALIDATION
//==============================================================================

/// Custom validator for chat message role
fn validate_role(role: &str) -> Result<(), validator::ValidationError> {
    const VALID_ROLES: &[&str] = &["system", "user", "assistant", "tool", "function"];

    if VALID_ROLES.contains(&role) {
        Ok(())
    } else {
        Err(validator::ValidationError::new("invalid_role"))
    }
}

/// Custom validator for socket address
fn validate_socket_addr(addr: &str) -> Result<(), validator::ValidationError> {
    addr.parse::<SocketAddr>()
        .map(|_| ())
        .map_err(|_| validator::ValidationError::new("invalid_socket_addr"))
}

//==============================================================================
// REQUEST EXTRACTION
//==============================================================================

/// Validated request extractor
///
/// Automatically deserializes, validates, and tracks request
pub struct ValidatedRequest<T> {
    pub inner: T,
    pub context: RequestContext,
}

#[async_trait]
impl<T, S> FromRequest<S> for ValidatedRequest<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
    Arc<GatewayState>: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request(
        mut req: Request<Body>,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        // Extract headers before consuming body
        let headers = req.headers().clone();
        let addr = req.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0)
            .unwrap_or_else(|| "0.0.0.0:0".parse().unwrap());
        let request_id = req.extensions()
            .get::<RequestId>()
            .cloned()
            .ok_or_else(|| ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                error_type: "internal_error".to_string(),
                message: "Missing request ID".to_string(),
                code: Some("missing_request_id".to_string()),
                param: None,
            })?;

        // Extract and parse body
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|e| ApiError {
                status: StatusCode::BAD_REQUEST,
                error_type: "invalid_request_error".to_string(),
                message: format!("Failed to read request body: {}", e),
                code: Some("body_read_error".to_string()),
                param: None,
            })?;

        // Parse JSON
        let inner: T = serde_json::from_slice(&bytes)
            .map_err(|e| ApiError {
                status: StatusCode::BAD_REQUEST,
                error_type: "invalid_request_error".to_string(),
                message: format!("Invalid JSON: {}", e),
                code: Some("json_parse_error".to_string()),
                param: None,
            })?;

        // Validate
        inner.validate()
            .map_err(ApiError::validation_error)?;

        // Extract authentication
        let auth = extract_auth(&headers)?;

        // Build context
        let context = RequestContext {
            request_id: request_id.0,
            auth,
            client_addr: addr,
            headers,
            timestamp: Utc::now(),
            user: None, // Populated by specific handler
        };

        Ok(ValidatedRequest { inner, context })
    }
}
```

---

## Graceful Shutdown

```rust
//==============================================================================
// GRACEFUL SHUTDOWN IMPLEMENTATION
//==============================================================================

/// Comprehensive shutdown procedure
///
/// # Shutdown Sequence
/// 1. Receive shutdown signal (SIGTERM/SIGINT)
/// 2. Stop accepting new connections
/// 3. Wait for active requests to complete (with timeout)
/// 4. Close provider connections
/// 5. Flush telemetry buffers
/// 6. Clean up resources
pub async fn shutdown_procedure(
    state: Arc<GatewayState>,
    shutdown_timeout: Duration,
) -> Result<()> {
    info!("Initiating graceful shutdown");

    let start = Instant::now();

    // Phase 1: Stop accepting new requests
    info!("Phase 1: Stopping new request acceptance");
    state.set_draining_mode(true);

    // Phase 2: Wait for active requests with timeout
    info!(
        active_requests = state.active_requests.count(),
        "Phase 2: Waiting for active requests to complete"
    );

    let drain_result = tokio::time::timeout(
        shutdown_timeout,
        state.active_requests.wait_idle()
    ).await;

    match drain_result {
        Ok(_) => {
            info!(
                duration_ms = start.elapsed().as_millis(),
                "All requests completed gracefully"
            );
        }
        Err(_) => {
            warn!(
                remaining_requests = state.active_requests.count(),
                timeout_secs = shutdown_timeout.as_secs(),
                "Shutdown timeout reached, force-closing connections"
            );
        }
    }

    // Phase 3: Close provider connections
    info!("Phase 3: Closing provider connections");
    state.providers.shutdown_all().await?;

    // Phase 4: Flush telemetry
    info!("Phase 4: Flushing telemetry buffers");
    state.telemetry.flush().await?;

    // Phase 5: Final cleanup
    info!("Phase 5: Final cleanup");
    state.cleanup().await?;

    info!(
        total_duration_ms = start.elapsed().as_millis(),
        "Graceful shutdown complete"
    );

    Ok(())
}

/// Connection draining mode
///
/// When enabled:
/// - Returns 503 Service Unavailable for new requests
/// - Allows existing requests to complete
/// - Adds "Connection: close" header to responses
impl GatewayState {
    pub fn set_draining_mode(&self, draining: bool) {
        self.draining.store(draining, Ordering::SeqCst);
    }

    pub fn is_draining(&self) -> bool {
        self.draining.load(Ordering::SeqCst)
    }
}

/// Draining middleware
///
/// Rejects new requests when in draining mode
pub async fn draining_middleware(
    State(state): State<Arc<GatewayState>>,
    request: Request<Body>,
    next: Next<Body>,
) -> Result<Response, ApiError> {
    if state.is_draining() {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            error_type: "service_unavailable".to_string(),
            message: "Server is shutting down".to_string(),
            code: Some("draining".to_string()),
            param: None,
        });
    }

    let mut response = next.run(request).await;

    // Add Connection: close header when draining
    if state.is_draining() {
        response.headers_mut().insert(
            header::CONNECTION,
            HeaderValue::from_static("close")
        );
    }

    Ok(response)
}
```

---

## Testing Strategy

```rust
//==============================================================================
// TESTING HELPERS
//==============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt; // for `oneshot`

    /// Create test gateway state
    async fn create_test_state() -> Arc<GatewayState> {
        let config = GatewayConfig::default();
        let providers = ProviderRegistry::new_mock();
        let router = Router::new_mock();
        let middleware = MiddlewareStack::new_mock();
        let telemetry = TelemetryCoordinator::new_mock();

        Arc::new(GatewayState {
            providers: Arc::new(providers),
            router: Arc::new(router),
            middleware: Arc::new(middleware),
            telemetry: Arc::new(telemetry),
            config: Arc::new(ArcSwap::new(Arc::new(config))),
            active_requests: Arc::new(ActiveRequestTracker::new()),
            circuit_breakers: Arc::new(CircuitBreakerRegistry::new()),
        })
    }

    #[tokio::test]
    async fn test_chat_completions_non_streaming() {
        let state = create_test_state().await;
        let app = create_router(state);

        let request = Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header("Content-Type", "application/json")
            .header("Authorization", "Bearer test-key")
            .body(Body::from(r#"{
                "model": "gpt-4",
                "messages": [{"role": "user", "content": "Hello"}],
                "stream": false
            }"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_chat_completions_streaming() {
        let state = create_test_state().await;
        let app = create_router(state);

        let request = Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header("Content-Type", "application/json")
            .header("Authorization", "Bearer test-key")
            .body(Body::from(r#"{
                "model": "gpt-4",
                "messages": [{"role": "user", "content": "Hello"}],
                "stream": true
            }"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/event-stream"
        );
    }

    #[tokio::test]
    async fn test_validation_error() {
        let state = create_test_state().await;
        let app = create_router(state);

        // Invalid request (missing required field)
        let request = Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header("Content-Type", "application/json")
            .header("Authorization", "Bearer test-key")
            .body(Body::from(r#"{
                "model": "gpt-4"
            }"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_authentication_error() {
        let state = create_test_state().await;
        let app = create_router(state);

        // No auth header
        let request = Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{
                "model": "gpt-4",
                "messages": [{"role": "user", "content": "Hello"}]
            }"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_health_endpoints() {
        let state = create_test_state().await;
        let app = create_router(state);

        // Liveness
        let request = Request::builder()
            .uri("/health/live")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Readiness
        let request = Request::builder()
            .uri("/health/ready")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
```

---

## Summary

This comprehensive pseudocode provides a production-ready implementation of the HTTP Server and API Handlers layer for the LLM-Inference-Gateway. Key features include:

**Core Functionality**:
- Full OpenAI API v1 compatibility (chat completions, embeddings, models)
- Server-Sent Events (SSE) streaming support
- Request validation with detailed error messages
- Multiple authentication methods (Bearer, API Key, Basic)
- Graceful shutdown with connection draining

**Performance & Reliability**:
- Async I/O with Tokio runtime
- Connection pooling and HTTP/2 multiplexing
- Circuit breaker integration
- Request timeout handling
- Backpressure management for streams

**Observability**:
- Distributed tracing with OpenTelemetry
- Structured logging with context propagation
- Prometheus metrics endpoint
- Health check endpoints for orchestration

**Developer Experience**:
- Type-safe request/response handling
- Comprehensive error mapping to HTTP status codes
- Hot-reloadable configuration
- Admin endpoints for runtime management
- Extensive test coverage

**Security**:
- TLS/SSL termination
- Authentication validation
- CORS support
- Request size limits
- Admin endpoint protection

This implementation serves as the foundation for the LLM-Inference-Gateway's API layer, providing a robust, scalable, and maintainable HTTP server that abstracts the complexity of multi-provider LLM infrastructure.
