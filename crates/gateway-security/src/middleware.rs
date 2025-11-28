//! Security middleware layer.

use crate::config::SecurityConfig;
use crate::error::SecurityError;
use crate::headers::{apply_security_headers, SecurityHeadersLayer};
use crate::ip_filter::IpFilter;
use crate::sanitize::Sanitizer;
use crate::validation::InputValidator;
use axum::http::{header, Request, Response, StatusCode};
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Security layer for comprehensive security middleware.
#[derive(Clone)]
pub struct SecurityLayer {
    config: Arc<SecurityConfig>,
    ip_filter: Option<Arc<IpFilter>>,
}

impl SecurityLayer {
    /// Create a new security layer.
    #[must_use]
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            config: Arc::new(config),
            ip_filter: None,
        }
    }

    /// Create with default configuration.
    #[must_use]
    pub fn default_layer() -> Self {
        Self::new(SecurityConfig::default())
    }

    /// Create with strict configuration.
    #[must_use]
    pub fn strict() -> Self {
        Self::new(SecurityConfig::strict())
    }

    /// Create with permissive configuration.
    #[must_use]
    pub fn permissive() -> Self {
        Self::new(SecurityConfig::permissive())
    }

    /// Set IP filter.
    #[must_use]
    pub fn with_ip_filter(mut self, filter: IpFilter) -> Self {
        self.ip_filter = Some(Arc::new(filter));
        self
    }
}

impl<S> Layer<S> for SecurityLayer {
    type Service = SecurityMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SecurityMiddleware {
            inner,
            config: Arc::clone(&self.config),
            ip_filter: self.ip_filter.clone(),
            validator: InputValidator::new(self.config.validation.clone()),
            sanitizer: Sanitizer::default_sanitizer()
                .with_security_config(self.config.content.clone()),
        }
    }
}

/// Security middleware service.
#[derive(Clone)]
pub struct SecurityMiddleware<S> {
    inner: S,
    config: Arc<SecurityConfig>,
    ip_filter: Option<Arc<IpFilter>>,
    validator: InputValidator,
    sanitizer: Sanitizer,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for SecurityMiddleware<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send,
    ReqBody: Send + 'static,
    ResBody: Default + Send + 'static,
{
    type Response = Response<ResBody>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<ReqBody>) -> Self::Future {
        let config = Arc::clone(&self.config);
        let ip_filter = self.ip_filter.clone();
        let validator = self.validator.clone();
        let sanitizer = self.sanitizer.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Skip if security is disabled
            if !config.enabled {
                return inner.call(request).await;
            }

            // Check IP filtering
            if let Some(filter) = &ip_filter {
                if let Some(ip) = extract_client_ip(&request) {
                    if let Err(e) = filter.check(ip).await {
                        return Ok(error_response(e));
                    }
                }
            }

            // Validate content type
            if let Some(content_type) = request.headers().get(header::CONTENT_TYPE) {
                if let Ok(ct) = content_type.to_str() {
                    if let Err(e) = validator.validate_content_type(ct) {
                        return Ok(error_response(e));
                    }
                }
            }

            // Validate content length
            if let Some(content_length) = request.headers().get(header::CONTENT_LENGTH) {
                if let Ok(ct_str) = content_length.to_str() {
                    if let Ok(length) = ct_str.parse::<usize>() {
                        if let Err(e) = validator.validate_body_size(length) {
                            return Ok(error_response(e));
                        }
                    }
                }
            }

            // Check for suspicious patterns in URL path
            let path = request.uri().path();
            if let Err(e) = sanitizer.check(path) {
                tracing::warn!(path = %path, "Suspicious path detected");
                return Ok(error_response(e));
            }

            // Check query string
            if let Some(query) = request.uri().query() {
                if let Err(e) = sanitizer.check(query) {
                    tracing::warn!(query = %query, "Suspicious query string detected");
                    return Ok(error_response(e));
                }
            }

            // Call inner service
            let mut response = inner.call(request).await?;

            // Apply security headers
            apply_security_headers(&config.headers, response.headers_mut());

            Ok(response)
        })
    }
}

/// Extract client IP from request.
fn extract_client_ip<B>(request: &Request<B>) -> Option<IpAddr> {
    // Try X-Forwarded-For first
    if let Some(xff) = request.headers().get("x-forwarded-for") {
        if let Ok(value) = xff.to_str() {
            if let Some(ip) = value.split(',').next() {
                if let Ok(addr) = ip.trim().parse() {
                    return Some(addr);
                }
            }
        }
    }

    // Try X-Real-IP
    if let Some(real_ip) = request.headers().get("x-real-ip") {
        if let Ok(value) = real_ip.to_str() {
            if let Ok(addr) = value.trim().parse() {
                return Some(addr);
            }
        }
    }

    // Try to get from connection info (this would need to be set by the server)
    // In practice, this comes from the socket address

    None
}

/// Create an error response.
fn error_response<B: Default>(error: SecurityError) -> Response<B> {
    let status = StatusCode::from_u16(error.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    Response::builder()
        .status(status)
        .body(B::default())
        .unwrap()
}

/// Builder for creating a security middleware stack.
#[derive(Debug, Default)]
pub struct SecurityStackBuilder {
    config: SecurityConfig,
    enable_headers: bool,
    enable_ip_filter: bool,
    enable_validation: bool,
    ip_filter_config: Option<crate::config::IpFilterSettings>,
}

impl SecurityStackBuilder {
    /// Create a new security stack builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: SecurityConfig::default(),
            enable_headers: true,
            enable_ip_filter: false,
            enable_validation: true,
            ip_filter_config: None,
        }
    }

    /// Set the security configuration.
    #[must_use]
    pub fn config(mut self, config: SecurityConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable or disable security headers.
    #[must_use]
    pub fn headers(mut self, enable: bool) -> Self {
        self.enable_headers = enable;
        self
    }

    /// Enable IP filtering with config.
    #[must_use]
    pub fn ip_filter(mut self, config: crate::config::IpFilterSettings) -> Self {
        self.enable_ip_filter = true;
        self.ip_filter_config = Some(config);
        self
    }

    /// Enable or disable input validation.
    #[must_use]
    pub fn validation(mut self, enable: bool) -> Self {
        self.enable_validation = enable;
        self
    }

    /// Build the security layer.
    #[must_use]
    pub fn build(self) -> SecurityLayer {
        let mut layer = SecurityLayer::new(self.config);

        if self.enable_ip_filter {
            if let Some(ip_config) = self.ip_filter_config {
                layer = layer.with_ip_filter(IpFilter::new(ip_config));
            }
        }

        layer
    }
}

/// Convenience function to create a headers-only layer.
#[must_use]
pub fn security_headers_layer() -> SecurityHeadersLayer {
    SecurityHeadersLayer::default_headers()
}

/// Convenience function to create a strict headers layer.
#[must_use]
pub fn strict_security_headers_layer() -> SecurityHeadersLayer {
    SecurityHeadersLayer::strict()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http::Request;

    #[test]
    fn test_security_layer_creation() {
        let _layer = SecurityLayer::new(SecurityConfig::default());
        let _layer = SecurityLayer::strict();
        let _layer = SecurityLayer::permissive();
    }

    #[test]
    fn test_security_stack_builder() {
        let layer = SecurityStackBuilder::new()
            .headers(true)
            .validation(true)
            .build();

        assert!(layer.config.enabled);
    }

    #[test]
    fn test_extract_client_ip_xff() {
        let request = Request::builder()
            .header("x-forwarded-for", "1.2.3.4, 10.0.0.1")
            .body(Body::empty())
            .unwrap();

        let ip = extract_client_ip(&request);
        assert_eq!(ip, Some("1.2.3.4".parse().unwrap()));
    }

    #[test]
    fn test_extract_client_ip_real_ip() {
        let request = Request::builder()
            .header("x-real-ip", "5.6.7.8")
            .body(Body::empty())
            .unwrap();

        let ip = extract_client_ip(&request);
        assert_eq!(ip, Some("5.6.7.8".parse().unwrap()));
    }

    #[test]
    fn test_extract_client_ip_none() {
        let request = Request::builder().body(Body::empty()).unwrap();

        let ip = extract_client_ip(&request);
        assert!(ip.is_none());
    }

    #[test]
    fn test_error_response() {
        let error = SecurityError::IpBlocked("1.2.3.4".to_string());
        let response: Response<Body> = error_response(error);

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_error_response_validation() {
        let error = SecurityError::Validation("invalid".to_string());
        let response: Response<Body> = error_response(error);

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_error_response_rate_limit() {
        let error = SecurityError::RateLimitExceeded("test".to_string());
        let response: Response<Body> = error_response(error);

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
