//! Custom Axum extractors for the gateway.

use agentics_contracts::ExecutionContext;
use axum::{
    async_trait,
    extract::{FromRequestParts, Request},
    http::{header, request::Parts},
};
use serde::de::DeserializeOwned;
use tracing::debug;

use crate::error::ApiError;

/// Extract tenant ID from request headers or API key
#[derive(Debug, Clone)]
pub struct TenantId(pub Option<String>);

#[async_trait]
impl<S> FromRequestParts<S> for TenantId
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Try X-Tenant-ID header first
        if let Some(tenant) = parts.headers.get("x-tenant-id") {
            if let Ok(id) = tenant.to_str() {
                return Ok(Self(Some(id.to_string())));
            }
        }

        // Try to extract from API key (format: sk-tenant_xxx)
        if let Some(auth) = parts.headers.get(header::AUTHORIZATION) {
            if let Ok(auth_str) = auth.to_str() {
                if let Some(key) = auth_str.strip_prefix("Bearer ") {
                    if let Some(tenant) = extract_tenant_from_key(key) {
                        return Ok(Self(Some(tenant)));
                    }
                }
            }
        }

        Ok(Self(None))
    }
}

/// Extract API key from Authorization header
#[derive(Debug, Clone)]
pub struct ApiKey(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for ApiKey
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .ok_or_else(|| ApiError::unauthorized("Missing Authorization header"))?;

        let auth_str = auth_header
            .to_str()
            .map_err(|_| ApiError::unauthorized("Invalid Authorization header"))?;

        let api_key = auth_str
            .strip_prefix("Bearer ")
            .ok_or_else(|| ApiError::unauthorized("Invalid Authorization format. Expected: Bearer <token>"))?;

        if api_key.is_empty() {
            return Err(ApiError::unauthorized("Empty API key"));
        }

        Ok(Self(api_key.to_string()))
    }
}

/// Optional API key extractor
#[derive(Debug, Clone)]
pub struct OptionalApiKey(pub Option<String>);

#[async_trait]
impl<S> FromRequestParts<S> for OptionalApiKey
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(auth_header) = parts.headers.get(header::AUTHORIZATION) {
            if let Ok(auth_str) = auth_header.to_str() {
                if let Some(key) = auth_str.strip_prefix("Bearer ") {
                    if !key.is_empty() {
                        return Ok(Self(Some(key.to_string())));
                    }
                }
            }
        }
        Ok(Self(None))
    }
}

/// Extract request ID from headers or generate one
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for RequestId
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Try various request ID headers
        let id = parts
            .headers
            .get("x-request-id")
            .or_else(|| parts.headers.get("x-correlation-id"))
            .or_else(|| parts.headers.get("request-id"))
            .and_then(|v| v.to_str().ok()).map_or_else(|| uuid::Uuid::new_v4().to_string(), String::from);

        Ok(Self(id))
    }
}

/// Extract client IP address
#[derive(Debug, Clone)]
pub struct ClientIp(pub Option<String>);

#[async_trait]
impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Try various headers for client IP
        let ip = parts
            .headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(|s| s.trim().to_string())
            .or_else(|| {
                parts
                    .headers
                    .get("x-real-ip")
                    .and_then(|v| v.to_str().ok())
                    .map(String::from)
            });

        Ok(Self(ip))
    }
}

/// Extract execution context from the Agentics execution system headers.
///
/// Reads `X-Parent-Span-Id` (required) and `X-Execution-Id` (optional).
/// Returns 400 Bad Request if `X-Parent-Span-Id` is missing or not a valid UUID.
/// If `X-Execution-Id` is missing, a new UUID is generated.
#[derive(Debug, Clone)]
pub struct ExecutionCtx(pub ExecutionContext);

#[async_trait]
impl<S> FromRequestParts<S> for ExecutionCtx
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let parent_span_id = parts
            .headers
            .get("x-parent-span-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| uuid::Uuid::parse_str(v).ok())
            .ok_or_else(|| {
                ApiError::bad_request(
                    "Missing or invalid X-Parent-Span-Id header. \
                     All execution requests must include a valid parent span UUID.",
                )
            })?;

        let execution_id = parts
            .headers
            .get("x-execution-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| uuid::Uuid::parse_str(v).ok())
            .unwrap_or_else(uuid::Uuid::new_v4);

        Ok(Self(ExecutionContext {
            execution_id,
            parent_span_id,
        }))
    }
}

/// JSON body extractor with better error handling
#[derive(Debug)]
pub struct JsonBody<T>(pub T);

#[async_trait]
impl<S, T> axum::extract::FromRequest<S> for JsonBody<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = axum::body::Bytes::from_request(req, state)
            .await
            .map_err(|e| ApiError::bad_request(format!("Failed to read request body: {e}")))?;

        let value: T = serde_json::from_slice(&bytes).map_err(|e| {
            let msg = format!("Invalid JSON: {e}");
            debug!(error = %e, "JSON parse error");
            ApiError::bad_request(msg)
        })?;

        Ok(Self(value))
    }
}

/// Extract tenant from API key if it follows the format sk-tenant_xxx
fn extract_tenant_from_key(key: &str) -> Option<String> {
    // Format: sk-<tenant>_<key>
    if let Some(rest) = key.strip_prefix("sk-") {
        if let Some(pos) = rest.find('_') {
            let tenant = &rest[..pos];
            if !tenant.is_empty() {
                return Some(tenant.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    #[test]
    fn test_extract_tenant_from_key() {
        assert_eq!(
            extract_tenant_from_key("sk-tenant1_abc123"),
            Some("tenant1".to_string())
        );
        assert_eq!(
            extract_tenant_from_key("sk-myorg_secretkey"),
            Some("myorg".to_string())
        );
        assert_eq!(extract_tenant_from_key("sk-abc123"), None);
        assert_eq!(extract_tenant_from_key("invalid"), None);
        assert_eq!(extract_tenant_from_key("sk-_key"), None);
    }

    #[tokio::test]
    async fn test_execution_ctx_rejects_missing_parent_span_id() {
        let req = Request::builder()
            .uri("/test")
            .body(())
            .expect("valid request");
        let (mut parts, _body) = req.into_parts();

        let result = ExecutionCtx::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::BAD_REQUEST);
        assert!(err.message.contains("X-Parent-Span-Id"));
    }

    #[tokio::test]
    async fn test_execution_ctx_rejects_invalid_uuid() {
        let req = Request::builder()
            .uri("/test")
            .header("x-parent-span-id", "not-a-uuid")
            .body(())
            .expect("valid request");
        let (mut parts, _body) = req.into_parts();

        let result = ExecutionCtx::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execution_ctx_extracts_both_headers() {
        let exec_id = uuid::Uuid::new_v4();
        let parent_id = uuid::Uuid::new_v4();

        let req = Request::builder()
            .uri("/test")
            .header("x-execution-id", exec_id.to_string())
            .header("x-parent-span-id", parent_id.to_string())
            .body(())
            .expect("valid request");
        let (mut parts, _body) = req.into_parts();

        let result = ExecutionCtx::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
        let ctx = result.unwrap().0;
        assert_eq!(ctx.execution_id, exec_id);
        assert_eq!(ctx.parent_span_id, parent_id);
    }

    #[tokio::test]
    async fn test_execution_ctx_generates_execution_id_if_missing() {
        let parent_id = uuid::Uuid::new_v4();

        let req = Request::builder()
            .uri("/test")
            .header("x-parent-span-id", parent_id.to_string())
            .body(())
            .expect("valid request");
        let (mut parts, _body) = req.into_parts();

        let result = ExecutionCtx::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
        let ctx = result.unwrap().0;
        assert_eq!(ctx.parent_span_id, parent_id);
        // execution_id should be a valid UUID (auto-generated)
        assert_ne!(ctx.execution_id, uuid::Uuid::nil());
    }
}
