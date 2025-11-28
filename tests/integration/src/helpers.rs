//! Test helper utilities for integration tests

use axum::routing::{get, post};
use axum::{Json, Router};
use gateway_config::GatewayConfig;
use once_cell::sync::Lazy;
use reqwest::{Client, Response};
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

/// Base port for test servers (incremented for each test)
static PORT_COUNTER: AtomicU16 = AtomicU16::new(18080);

/// Initialize tracing for tests (only once)
static TRACING: Lazy<()> = Lazy::new(|| {
    if std::env::var("TEST_LOG").is_ok() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .with_test_writer()
            .try_init();
    }
});

/// Initialize tracing for tests
pub fn init_tracing() {
    Lazy::force(&TRACING);
}

/// Get a unique port for a test server
pub fn get_test_port() -> u16 {
    PORT_COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Test server wrapper for integration tests
pub struct TestServer {
    /// The server address
    pub addr: SocketAddr,
    /// HTTP client for making requests
    pub client: Client,
    /// Base URL for the server
    pub base_url: String,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl TestServer {
    /// Create a new test server with the given router
    pub async fn new(router: Router) -> Self {
        let port = get_test_port();
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = TcpListener::bind(addr).await.expect("Failed to bind");
        let actual_addr = listener.local_addr().expect("Failed to get local addr");

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // Spawn the server
        tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("Server error");
        });

        // Wait for server to be ready
        tokio::time::sleep(Duration::from_millis(50)).await;

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create client");

        Self {
            addr: actual_addr,
            client,
            base_url: format!("http://{}", actual_addr),
            shutdown_tx: Some(shutdown_tx),
        }
    }

    /// Create a test server with default gateway configuration
    pub async fn with_default_config() -> Self {
        let router = create_test_router();
        Self::new(router).await
    }

    /// Create a test server with custom configuration
    pub async fn with_config(_config: &GatewayConfig) -> Self {
        let router = create_test_router();
        Self::new(router).await
    }

    /// Get the full URL for a path
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Make a GET request
    pub async fn get(&self, path: &str) -> Response {
        self.client
            .get(self.url(path))
            .send()
            .await
            .expect("Request failed")
    }

    /// Make a GET request with headers
    pub async fn get_with_headers(
        &self,
        path: &str,
        headers: Vec<(&str, &str)>,
    ) -> Response {
        let mut builder = self.client.get(self.url(path));
        for (key, value) in headers {
            builder = builder.header(key, value);
        }
        builder.send().await.expect("Request failed")
    }

    /// Make a POST request with JSON body
    pub async fn post_json(&self, path: &str, body: &Value) -> Response {
        self.client
            .post(self.url(path))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .expect("Request failed")
    }

    /// Make a POST request with JSON body and headers
    pub async fn post_json_with_headers(
        &self,
        path: &str,
        body: &Value,
        headers: Vec<(&str, &str)>,
    ) -> Response {
        let mut builder = self.client
            .post(self.url(path))
            .header("Content-Type", "application/json")
            .json(body);
        for (key, value) in headers {
            builder = builder.header(key, value);
        }
        builder.send().await.expect("Request failed")
    }

    /// Make a streaming POST request and collect chunks
    pub async fn post_streaming(&self, path: &str, body: &Value) -> Vec<String> {
        use futures::StreamExt;

        let response = self.client
            .post(self.url(path))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .expect("Request failed");

        let mut chunks = Vec::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            if let Ok(bytes) = chunk {
                if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                    chunks.push(text);
                }
            }
        }

        chunks
    }

    /// Parse response body as JSON
    pub async fn json_body(response: Response) -> Value {
        response.json().await.expect("Failed to parse JSON")
    }

    /// Shutdown the test server
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Create a test router with mock endpoints
fn create_test_router() -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/live", get(live_handler))
        .route("/v1/models", get(models_handler))
        .route("/v1/chat/completions", post(chat_completions_handler))
}

async fn health_handler() -> Json<Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": "0.1.0"
    }))
}

async fn ready_handler() -> Json<Value> {
    Json(serde_json::json!({
        "status": "ready",
        "providers": {
            "openai": true,
            "anthropic": true
        }
    }))
}

async fn live_handler() -> Json<Value> {
    Json(serde_json::json!({"status": "alive"}))
}

async fn models_handler() -> Json<Value> {
    Json(crate::fixtures::models_list_response())
}

async fn chat_completions_handler(Json(body): Json<Value>) -> Json<Value> {
    let model = body["model"].as_str().unwrap_or("gpt-4o-mini");
    let _stream = body["stream"].as_bool().unwrap_or(false);

    // Generate a mock response
    Json(crate::fixtures::openai_json_response(
        model,
        "Hello! I'm a test assistant. How can I help you today?",
    ))
}

/// Assert that a response has the expected status code
pub fn assert_status(response: &Response, expected: u16) {
    assert_eq!(
        response.status().as_u16(),
        expected,
        "Expected status {}, got {}",
        expected,
        response.status()
    );
}

/// Assert that a JSON response contains expected fields
pub fn assert_json_contains(json: &Value, expected: &Value) {
    for (key, value) in expected.as_object().expect("Expected object") {
        assert!(
            json.get(key).is_some(),
            "Missing key '{}' in response",
            key
        );
        if value.is_object() {
            assert_json_contains(&json[key], value);
        } else {
            assert_eq!(
                &json[key], value,
                "Mismatch for key '{}': expected {:?}, got {:?}",
                key, value, json[key]
            );
        }
    }
}

/// Wait for a condition to be true with timeout
pub async fn wait_for<F, Fut>(condition: F, timeout: Duration) -> bool
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if condition().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

/// Generate a random API key for testing
pub fn random_api_key() -> String {
    format!("sk-test-{}", uuid::Uuid::new_v4())
}

/// Generate a random request ID
pub fn random_request_id() -> String {
    format!("req_{}", uuid::Uuid::new_v4().to_string().replace('-', ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_health() {
        init_tracing();
        let server = TestServer::with_default_config().await;

        let response = server.get("/health").await;
        assert_status(&response, 200);

        let json = TestServer::json_body(response).await;
        assert_eq!(json["status"], "healthy");
    }

    #[tokio::test]
    async fn test_server_ready() {
        let server = TestServer::with_default_config().await;

        let response = server.get("/ready").await;
        assert_status(&response, 200);
    }

    #[tokio::test]
    async fn test_server_live() {
        let server = TestServer::with_default_config().await;

        let response = server.get("/live").await;
        assert_status(&response, 200);
    }

    #[test]
    fn test_random_api_key() {
        let key1 = random_api_key();
        let key2 = random_api_key();
        assert!(key1.starts_with("sk-test-"));
        assert!(key2.starts_with("sk-test-"));
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_random_request_id() {
        let id1 = random_request_id();
        let id2 = random_request_id();
        assert!(id1.starts_with("req_"));
        assert!(id2.starts_with("req_"));
        assert_ne!(id1, id2);
    }
}
