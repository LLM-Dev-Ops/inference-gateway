//! HTTP client for the Gateway SDK.

use crate::config::ClientConfig;
use crate::error::{ApiErrorResponse, Error, Result};
use crate::request::{ChatRequest, ChatRequestBuilder, Message};
use crate::response::{ChatResponse, HealthResponse, ModelsListResponse};
use crate::streaming::ChatStream;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use secrecy::Secret;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, instrument};
use url::Url;

/// Client for interacting with the LLM Inference Gateway.
///
/// # Example
///
/// ```rust,no_run
/// use gateway_sdk::Client;
///
/// #[tokio::main]
/// async fn main() -> Result<(), gateway_sdk::Error> {
///     let client = Client::builder()
///         .base_url("http://localhost:8080")
///         .api_key("your-api-key")
///         .build()?;
///
///     let response = client
///         .chat()
///         .model("gpt-4o")
///         .user_message("Hello!")
///         .send()
///         .await?;
///
///     println!("{}", response.content());
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct Client {
    /// HTTP client.
    http: reqwest::Client,
    /// Client configuration.
    config: Arc<ClientConfig>,
}

impl Client {
    /// Create a new client builder.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Create a new client with the given configuration.
    pub fn new(config: ClientConfig) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&config.user_agent)
                .map_err(|e| Error::configuration(format!("Invalid user agent: {}", e)))?,
        );

        // Add API key header if present
        if let Some(api_key) = config.api_key_value() {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", api_key))
                    .map_err(|e| Error::configuration(format!("Invalid API key: {}", e)))?,
            );
        }

        // Add custom headers
        for (name, value) in &config.custom_headers {
            let header_name = HeaderName::try_from(name.as_str())
                .map_err(|e| Error::configuration(format!("Invalid header name '{}': {}", name, e)))?;
            let header_value = HeaderValue::from_str(value)
                .map_err(|e| Error::configuration(format!("Invalid header value for '{}': {}", name, e)))?;
            headers.insert(header_name, header_value);
        }

        // Add tenant ID header if present
        if let Some(tenant_id) = &config.tenant_id {
            headers.insert(
                HeaderName::from_static("x-tenant-id"),
                HeaderValue::from_str(tenant_id)
                    .map_err(|e| Error::configuration(format!("Invalid tenant ID: {}", e)))?,
            );
        }

        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .connect_timeout(config.connect_timeout)
            .default_headers(headers)
            .build()
            .map_err(|e| Error::configuration(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            http,
            config: Arc::new(config),
        })
    }

    /// Get the client configuration.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Create a chat request builder.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use gateway_sdk::Client;
    /// # async fn example(client: &Client) -> Result<(), gateway_sdk::Error> {
    /// let response = client
    ///     .chat()
    ///     .model("gpt-4o")
    ///     .system_message("You are helpful")
    ///     .user_message("Hello!")
    ///     .temperature(0.7)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn chat(&self) -> ChatBuilder {
        ChatBuilder::new(self.clone())
    }

    /// Send a chat completion request.
    #[instrument(skip(self, request), fields(model = %request.model))]
    pub async fn chat_completion(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let url = self.url("/v1/chat/completions")?;

        debug!("Sending chat completion request to {}", url);

        let response = self.execute_with_retry(|| async {
            self.http
                .post(url.clone())
                .json(request)
                .send()
                .await
        })
        .await?;

        self.handle_response(response).await
    }

    /// Send a streaming chat completion request.
    #[instrument(skip(self, request), fields(model = %request.model))]
    pub async fn chat_completion_stream(&self, request: &ChatRequest) -> Result<ChatStream> {
        let url = self.url("/v1/chat/completions")?;

        // Ensure stream is enabled
        let mut request = request.clone();
        request.stream = Some(true);

        debug!("Sending streaming chat completion request to {}", url);

        let response = self.http
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        if !response.status().is_success() {
            return Err(self.handle_error_response(response).await);
        }

        Ok(ChatStream::new(response.bytes_stream()))
    }

    /// List available models.
    #[instrument(skip(self))]
    pub async fn list_models(&self) -> Result<ModelsListResponse> {
        let url = self.url("/v1/models")?;

        debug!("Listing models from {}", url);

        let response = self.execute_with_retry(|| async {
            self.http.get(url.clone()).send().await
        })
        .await?;

        self.handle_response(response).await
    }

    /// Check the health of the gateway.
    #[instrument(skip(self))]
    pub async fn health(&self) -> Result<HealthResponse> {
        let url = self.url("/health")?;

        debug!("Checking health at {}", url);

        let response = self.http
            .get(url)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        self.handle_response(response).await
    }

    /// Check if the gateway is healthy.
    pub async fn is_healthy(&self) -> bool {
        self.health().await.map(|h| h.is_healthy()).unwrap_or(false)
    }

    /// Check readiness of the gateway.
    #[instrument(skip(self))]
    pub async fn ready(&self) -> Result<HealthResponse> {
        let url = self.url("/ready")?;

        debug!("Checking readiness at {}", url);

        let response = self.http
            .get(url)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        self.handle_response(response).await
    }

    /// Build a URL for the given path.
    fn url(&self, path: &str) -> Result<Url> {
        self.config
            .base_url
            .join(path)
            .map_err(|e| Error::configuration(format!("Invalid URL path '{}': {}", path, e)))
    }

    /// Execute a request with retry logic.
    async fn execute_with_retry<F, Fut>(&self, f: F) -> Result<reqwest::Response>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<reqwest::Response, reqwest::Error>>,
    {
        let max_retries = self.config.max_retries;
        let mut attempts = 0;
        let mut delay = self.config.retry_initial_delay;

        loop {
            attempts += 1;

            match f().await {
                Ok(response) => {
                    // Check if we should retry based on status code
                    let status = response.status().as_u16();
                    if should_retry_status(status) && attempts <= max_retries {
                        debug!(
                            "Received status {} on attempt {}, retrying after {:?}",
                            status, attempts, delay
                        );

                        // Get retry-after header if present
                        if let Some(retry_after) = response.headers().get("retry-after") {
                            if let Ok(secs) = retry_after.to_str().unwrap_or("").parse::<u64>() {
                                delay = Duration::from_secs(secs);
                            }
                        }

                        tokio::time::sleep(delay).await;
                        delay = std::cmp::min(delay * 2, self.config.retry_max_delay);
                        continue;
                    }
                    return Ok(response);
                }
                Err(e) => {
                    let error = self.map_reqwest_error(e);

                    if error.is_retryable() && attempts <= max_retries {
                        debug!(
                            "Request failed on attempt {}: {}, retrying after {:?}",
                            attempts, error, delay
                        );
                        tokio::time::sleep(delay).await;
                        delay = std::cmp::min(delay * 2, self.config.retry_max_delay);
                        continue;
                    }

                    if attempts > 1 {
                        return Err(Error::retry_exhausted(attempts, error));
                    }
                    return Err(error);
                }
            }
        }
    }

    /// Handle a successful response.
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T> {
        let status = response.status();

        if status.is_success() {
            response
                .json()
                .await
                .map_err(|e| Error::parse_error(format!("Failed to parse response: {}", e)))
        } else {
            Err(self.handle_error_response(response).await)
        }
    }

    /// Handle an error response.
    async fn handle_error_response(&self, response: reqwest::Response) -> Error {
        let status = response.status().as_u16();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        // Try to parse error response
        let body = response.text().await.unwrap_or_default();

        if let Ok(api_error) = serde_json::from_str::<ApiErrorResponse>(&body) {
            return Error::Api {
                status,
                message: api_error.error.message,
                error_type: api_error.error.error_type.or(api_error.error.code),
                request_id,
            };
        }

        // Handle specific status codes
        match status {
            401 => Error::Authentication {
                message: body.clone(),
            },
            429 => {
                let retry_after = None; // Could parse from headers
                Error::RateLimited {
                    retry_after,
                    request_id,
                }
            }
            404 => Error::Api {
                status,
                message: "Not found".to_string(),
                error_type: Some("not_found".to_string()),
                request_id,
            },
            503 => Error::Unavailable {
                message: body.clone(),
            },
            _ => Error::Api {
                status,
                message: if body.is_empty() {
                    format!("HTTP {}", status)
                } else {
                    body
                },
                error_type: None,
                request_id,
            },
        }
    }

    /// Map a reqwest error to an SDK error.
    fn map_reqwest_error(&self, error: reqwest::Error) -> Error {
        if error.is_timeout() {
            Error::Timeout {
                duration_ms: self.config.timeout.as_millis() as u64,
            }
        } else if error.is_connect() {
            Error::Connection {
                message: error.to_string(),
            }
        } else {
            Error::Http(error)
        }
    }
}

/// Check if a status code should trigger a retry.
fn should_retry_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

/// Builder for creating a Client.
#[derive(Debug)]
pub struct ClientBuilder {
    base_url: Option<Url>,
    api_key: Option<Secret<String>>,
    timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    max_retries: Option<u32>,
    retry_initial_delay: Option<Duration>,
    retry_max_delay: Option<Duration>,
    user_agent: Option<String>,
    default_model: Option<String>,
    custom_headers: Vec<(String, String)>,
    enable_tracing: bool,
    tenant_id: Option<String>,
}

impl ClientBuilder {
    /// Create a new client builder.
    pub fn new() -> Self {
        Self {
            base_url: None,
            api_key: None,
            timeout: None,
            connect_timeout: None,
            max_retries: None,
            retry_initial_delay: None,
            retry_max_delay: None,
            user_agent: None,
            default_model: None,
            custom_headers: Vec::new(),
            enable_tracing: false,
            tenant_id: None,
        }
    }

    /// Set the base URL.
    pub fn base_url(mut self, url: impl AsRef<str>) -> Self {
        self.base_url = Url::parse(url.as_ref()).ok();
        self
    }

    /// Set the API key.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(Secret::new(key.into()));
        self
    }

    /// Set the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the connection timeout.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// Set the maximum number of retries.
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = Some(retries);
        self
    }

    /// Set the initial retry delay.
    pub fn retry_initial_delay(mut self, delay: Duration) -> Self {
        self.retry_initial_delay = Some(delay);
        self
    }

    /// Set the maximum retry delay.
    pub fn retry_max_delay(mut self, delay: Duration) -> Self {
        self.retry_max_delay = Some(delay);
        self
    }

    /// Set the user agent.
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Set the default model.
    pub fn default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    /// Add a custom header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.push((name.into(), value.into()));
        self
    }

    /// Enable request tracing.
    pub fn enable_tracing(mut self, enable: bool) -> Self {
        self.enable_tracing = enable;
        self
    }

    /// Set the tenant ID.
    pub fn tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<Client> {
        let base_url = self.base_url.unwrap_or_else(|| {
            Url::parse("http://localhost:8080").expect("valid default URL")
        });

        let config = ClientConfig {
            base_url,
            api_key: self.api_key,
            timeout: self.timeout.unwrap_or(ClientConfig::DEFAULT_TIMEOUT),
            connect_timeout: self.connect_timeout.unwrap_or(ClientConfig::DEFAULT_CONNECT_TIMEOUT),
            max_retries: self.max_retries.unwrap_or(ClientConfig::DEFAULT_MAX_RETRIES),
            retry_initial_delay: self.retry_initial_delay.unwrap_or(ClientConfig::DEFAULT_RETRY_INITIAL_DELAY),
            retry_max_delay: self.retry_max_delay.unwrap_or(ClientConfig::DEFAULT_RETRY_MAX_DELAY),
            user_agent: self.user_agent.unwrap_or_else(|| ClientConfig::DEFAULT_USER_AGENT.to_string()),
            default_model: self.default_model,
            custom_headers: self.custom_headers,
            enable_tracing: self.enable_tracing,
            tenant_id: self.tenant_id,
        };

        Client::new(config)
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for chat completion requests.
pub struct ChatBuilder {
    client: Client,
    builder: ChatRequestBuilder,
}

impl ChatBuilder {
    fn new(client: Client) -> Self {
        let mut builder = ChatRequestBuilder::new();

        // Apply default model if set
        if let Some(model) = client.config.default_model() {
            builder = builder.model(model);
        }

        Self { client, builder }
    }

    /// Set the model to use.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.builder = self.builder.model(model);
        self
    }

    /// Add a message.
    pub fn message(mut self, message: Message) -> Self {
        self.builder = self.builder.message(message);
        self
    }

    /// Set the system message.
    pub fn system_message(mut self, content: impl Into<String>) -> Self {
        self.builder = self.builder.system_message(content);
        self
    }

    /// Add a user message.
    pub fn user_message(mut self, content: impl Into<String>) -> Self {
        self.builder = self.builder.user_message(content);
        self
    }

    /// Add an assistant message.
    pub fn assistant_message(mut self, content: impl Into<String>) -> Self {
        self.builder = self.builder.assistant_message(content);
        self
    }

    /// Set the temperature.
    pub fn temperature(mut self, temperature: f32) -> Self {
        self.builder = self.builder.temperature(temperature);
        self
    }

    /// Set max tokens.
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.builder = self.builder.max_tokens(max_tokens);
        self
    }

    /// Set top_p.
    pub fn top_p(mut self, top_p: f32) -> Self {
        self.builder = self.builder.top_p(top_p);
        self
    }

    /// Set the user ID.
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.builder = self.builder.user(user);
        self
    }

    /// Set the seed.
    pub fn seed(mut self, seed: i64) -> Self {
        self.builder = self.builder.seed(seed);
        self
    }

    /// Send the request.
    pub async fn send(self) -> Result<ChatResponse> {
        let request = self.builder.build()?;
        self.client.chat_completion(&request).await
    }

    /// Send as a streaming request.
    pub async fn stream(mut self) -> Result<ChatStream> {
        self.builder = self.builder.streaming(true);
        let request = self.builder.build()?;
        self.client.chat_completion_stream(&request).await
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.config.base_url)
            .field("has_api_key", &self.config.has_api_key())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_builder() {
        let client = Client::builder()
            .base_url("http://localhost:8080")
            .api_key("test-key")
            .timeout(Duration::from_secs(60))
            .max_retries(5)
            .build()
            .unwrap();

        assert_eq!(client.config.base_url.as_str(), "http://localhost:8080/");
        assert!(client.config.has_api_key());
        assert_eq!(client.config.timeout, Duration::from_secs(60));
        assert_eq!(client.config.max_retries, 5);
    }

    #[test]
    fn test_client_default_url() {
        let client = Client::builder()
            .build()
            .unwrap();

        assert_eq!(client.config.base_url.as_str(), "http://localhost:8080/");
    }

    #[test]
    fn test_should_retry_status() {
        assert!(should_retry_status(429));
        assert!(should_retry_status(500));
        assert!(should_retry_status(502));
        assert!(should_retry_status(503));
        assert!(should_retry_status(504));
        assert!(!should_retry_status(200));
        assert!(!should_retry_status(400));
        assert!(!should_retry_status(401));
        assert!(!should_retry_status(404));
    }
}
