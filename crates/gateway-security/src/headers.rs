//! Security headers middleware.

use crate::config::HeadersConfig;
use axum::http::{header, HeaderValue, Request, Response};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Security headers layer.
#[derive(Debug, Clone)]
pub struct SecurityHeadersLayer {
    config: HeadersConfig,
}

impl SecurityHeadersLayer {
    /// Create a new security headers layer.
    #[must_use]
    pub fn new(config: HeadersConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    #[must_use]
    pub fn default_headers() -> Self {
        Self::new(HeadersConfig::default())
    }

    /// Create with strict configuration.
    #[must_use]
    pub fn strict() -> Self {
        Self::new(HeadersConfig::strict())
    }
}

impl<S> Layer<S> for SecurityHeadersLayer {
    type Service = SecurityHeaders<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SecurityHeaders {
            inner,
            config: self.config.clone(),
        }
    }
}

/// Security headers service.
#[derive(Debug, Clone)]
pub struct SecurityHeaders<S> {
    inner: S,
    config: HeadersConfig,
}

impl<S> SecurityHeaders<S> {
    /// Create a new security headers service.
    #[must_use]
    pub fn new(inner: S, config: HeadersConfig) -> Self {
        Self { inner, config }
    }
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for SecurityHeaders<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    S::Future: Send + 'static,
{
    type Response = Response<ResBody>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<ReqBody>) -> Self::Future {
        let config = self.config.clone();
        let future = self.inner.call(request);

        Box::pin(async move {
            let mut response = future.await?;
            apply_security_headers(&config, response.headers_mut());
            Ok(response)
        })
    }
}

/// Apply security headers to a header map.
pub fn apply_security_headers(config: &HeadersConfig, headers: &mut http::HeaderMap) {
    // HSTS
    if config.hsts_enabled {
        let mut hsts_value = format!("max-age={}", config.hsts_max_age);
        if config.hsts_include_subdomains {
            hsts_value.push_str("; includeSubDomains");
        }
        if config.hsts_preload {
            hsts_value.push_str("; preload");
        }
        if let Ok(value) = HeaderValue::from_str(&hsts_value) {
            headers.insert(header::STRICT_TRANSPORT_SECURITY, value);
        }
    }

    // X-Content-Type-Options
    if config.nosniff {
        headers.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
    }

    // X-Frame-Options
    if !config.frame_options.is_empty() {
        if let Ok(value) = HeaderValue::from_str(&config.frame_options) {
            headers.insert(header::X_FRAME_OPTIONS, value);
        }
    }

    // X-XSS-Protection
    if config.xss_protection {
        headers.insert(
            header::X_XSS_PROTECTION,
            HeaderValue::from_static("1; mode=block"),
        );
    }

    // Content-Security-Policy
    if let Some(ref csp) = config.content_security_policy {
        if let Ok(value) = HeaderValue::from_str(csp) {
            headers.insert(header::CONTENT_SECURITY_POLICY, value);
        }
    }

    // Referrer-Policy
    if !config.referrer_policy.is_empty() {
        if let Ok(value) = HeaderValue::from_str(&config.referrer_policy) {
            headers.insert(header::REFERRER_POLICY, value);
        }
    }

    // Permissions-Policy
    if let Some(ref pp) = config.permissions_policy {
        if let Ok(value) = HeaderValue::from_str(pp) {
            headers.insert(
                http::header::HeaderName::from_static("permissions-policy"),
                value,
            );
        }
    }

    // Remove server header
    if config.remove_server_header {
        headers.remove(header::SERVER);
    }

    // Custom headers
    for (name, value) in &config.custom_headers {
        if let (Ok(name), Ok(value)) = (
            http::header::HeaderName::try_from(name.as_str()),
            HeaderValue::from_str(value),
        ) {
            headers.insert(name, value);
        }
    }

    // Always add these security headers
    headers.insert(
        http::header::HeaderName::from_static("x-permitted-cross-domain-policies"),
        HeaderValue::from_static("none"),
    );

    headers.insert(
        http::header::HeaderName::from_static("cross-origin-embedder-policy"),
        HeaderValue::from_static("require-corp"),
    );

    headers.insert(
        http::header::HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );

    headers.insert(
        http::header::HeaderName::from_static("cross-origin-resource-policy"),
        HeaderValue::from_static("same-origin"),
    );
}

/// Build HSTS header value.
#[must_use]
pub fn build_hsts_header(max_age: u64, include_subdomains: bool, preload: bool) -> String {
    let mut value = format!("max-age={}", max_age);
    if include_subdomains {
        value.push_str("; includeSubDomains");
    }
    if preload {
        value.push_str("; preload");
    }
    value
}

/// Build Content-Security-Policy header.
#[derive(Debug, Default)]
pub struct CspBuilder {
    directives: Vec<(String, Vec<String>)>,
}

impl CspBuilder {
    /// Create a new CSP builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add default-src directive.
    #[must_use]
    pub fn default_src(mut self, sources: &[&str]) -> Self {
        self.directives.push((
            "default-src".to_string(),
            sources.iter().map(|s| s.to_string()).collect(),
        ));
        self
    }

    /// Add script-src directive.
    #[must_use]
    pub fn script_src(mut self, sources: &[&str]) -> Self {
        self.directives.push((
            "script-src".to_string(),
            sources.iter().map(|s| s.to_string()).collect(),
        ));
        self
    }

    /// Add style-src directive.
    #[must_use]
    pub fn style_src(mut self, sources: &[&str]) -> Self {
        self.directives.push((
            "style-src".to_string(),
            sources.iter().map(|s| s.to_string()).collect(),
        ));
        self
    }

    /// Add img-src directive.
    #[must_use]
    pub fn img_src(mut self, sources: &[&str]) -> Self {
        self.directives.push((
            "img-src".to_string(),
            sources.iter().map(|s| s.to_string()).collect(),
        ));
        self
    }

    /// Add connect-src directive.
    #[must_use]
    pub fn connect_src(mut self, sources: &[&str]) -> Self {
        self.directives.push((
            "connect-src".to_string(),
            sources.iter().map(|s| s.to_string()).collect(),
        ));
        self
    }

    /// Add font-src directive.
    #[must_use]
    pub fn font_src(mut self, sources: &[&str]) -> Self {
        self.directives.push((
            "font-src".to_string(),
            sources.iter().map(|s| s.to_string()).collect(),
        ));
        self
    }

    /// Add frame-src directive.
    #[must_use]
    pub fn frame_src(mut self, sources: &[&str]) -> Self {
        self.directives.push((
            "frame-src".to_string(),
            sources.iter().map(|s| s.to_string()).collect(),
        ));
        self
    }

    /// Add frame-ancestors directive.
    #[must_use]
    pub fn frame_ancestors(mut self, sources: &[&str]) -> Self {
        self.directives.push((
            "frame-ancestors".to_string(),
            sources.iter().map(|s| s.to_string()).collect(),
        ));
        self
    }

    /// Add form-action directive.
    #[must_use]
    pub fn form_action(mut self, sources: &[&str]) -> Self {
        self.directives.push((
            "form-action".to_string(),
            sources.iter().map(|s| s.to_string()).collect(),
        ));
        self
    }

    /// Add base-uri directive.
    #[must_use]
    pub fn base_uri(mut self, sources: &[&str]) -> Self {
        self.directives.push((
            "base-uri".to_string(),
            sources.iter().map(|s| s.to_string()).collect(),
        ));
        self
    }

    /// Add object-src directive.
    #[must_use]
    pub fn object_src(mut self, sources: &[&str]) -> Self {
        self.directives.push((
            "object-src".to_string(),
            sources.iter().map(|s| s.to_string()).collect(),
        ));
        self
    }

    /// Add upgrade-insecure-requests directive.
    #[must_use]
    pub fn upgrade_insecure_requests(mut self) -> Self {
        self.directives
            .push(("upgrade-insecure-requests".to_string(), Vec::new()));
        self
    }

    /// Add block-all-mixed-content directive.
    #[must_use]
    pub fn block_all_mixed_content(mut self) -> Self {
        self.directives
            .push(("block-all-mixed-content".to_string(), Vec::new()));
        self
    }

    /// Add report-uri directive.
    #[must_use]
    pub fn report_uri(mut self, uri: &str) -> Self {
        self.directives
            .push(("report-uri".to_string(), vec![uri.to_string()]));
        self
    }

    /// Add report-to directive.
    #[must_use]
    pub fn report_to(mut self, group: &str) -> Self {
        self.directives
            .push(("report-to".to_string(), vec![group.to_string()]));
        self
    }

    /// Build the CSP header value.
    #[must_use]
    pub fn build(self) -> String {
        self.directives
            .into_iter()
            .map(|(directive, sources)| {
                if sources.is_empty() {
                    directive
                } else {
                    format!("{} {}", directive, sources.join(" "))
                }
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// Create a strict CSP for API endpoints.
#[must_use]
pub fn api_csp() -> String {
    CspBuilder::new()
        .default_src(&["'none'"])
        .frame_ancestors(&["'none'"])
        .build()
}

/// Create a standard CSP for web applications.
#[must_use]
pub fn web_csp() -> String {
    CspBuilder::new()
        .default_src(&["'self'"])
        .script_src(&["'self'"])
        .style_src(&["'self'", "'unsafe-inline'"])
        .img_src(&["'self'", "data:", "https:"])
        .font_src(&["'self'"])
        .connect_src(&["'self'"])
        .frame_ancestors(&["'none'"])
        .form_action(&["'self'"])
        .base_uri(&["'self'"])
        .upgrade_insecure_requests()
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_security_headers() {
        let config = HeadersConfig::default();
        let mut headers = http::HeaderMap::new();

        apply_security_headers(&config, &mut headers);

        assert!(headers.contains_key(header::STRICT_TRANSPORT_SECURITY));
        assert!(headers.contains_key(header::X_CONTENT_TYPE_OPTIONS));
        assert!(headers.contains_key(header::X_FRAME_OPTIONS));
        assert!(headers.contains_key(header::X_XSS_PROTECTION));
        assert!(headers.contains_key(header::CONTENT_SECURITY_POLICY));
    }

    #[test]
    fn test_strict_headers() {
        let config = HeadersConfig::strict();
        let mut headers = http::HeaderMap::new();

        apply_security_headers(&config, &mut headers);

        let hsts = headers
            .get(header::STRICT_TRANSPORT_SECURITY)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(hsts.contains("preload"));
        assert!(hsts.contains("includeSubDomains"));
    }

    #[test]
    fn test_permissive_headers() {
        let config = HeadersConfig::permissive();
        let mut headers = http::HeaderMap::new();

        apply_security_headers(&config, &mut headers);

        assert!(!headers.contains_key(header::STRICT_TRANSPORT_SECURITY));
    }

    #[test]
    fn test_hsts_builder() {
        let hsts = build_hsts_header(31536000, true, true);
        assert_eq!(hsts, "max-age=31536000; includeSubDomains; preload");

        let hsts_simple = build_hsts_header(3600, false, false);
        assert_eq!(hsts_simple, "max-age=3600");
    }

    #[test]
    fn test_csp_builder() {
        let csp = CspBuilder::new()
            .default_src(&["'self'"])
            .script_src(&["'self'", "'unsafe-inline'"])
            .frame_ancestors(&["'none'"])
            .build();

        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("script-src 'self' 'unsafe-inline'"));
        assert!(csp.contains("frame-ancestors 'none'"));
    }

    #[test]
    fn test_api_csp() {
        let csp = api_csp();
        assert!(csp.contains("default-src 'none'"));
        assert!(csp.contains("frame-ancestors 'none'"));
    }

    #[test]
    fn test_web_csp() {
        let csp = web_csp();
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("upgrade-insecure-requests"));
    }

    #[test]
    fn test_custom_headers() {
        let config = HeadersConfig {
            custom_headers: vec![
                ("X-Custom-Header".to_string(), "custom-value".to_string()),
                ("X-Another".to_string(), "another-value".to_string()),
            ],
            ..Default::default()
        };

        let mut headers = http::HeaderMap::new();
        apply_security_headers(&config, &mut headers);

        assert_eq!(
            headers.get("x-custom-header").unwrap().to_str().unwrap(),
            "custom-value"
        );
        assert_eq!(
            headers.get("x-another").unwrap().to_str().unwrap(),
            "another-value"
        );
    }
}
