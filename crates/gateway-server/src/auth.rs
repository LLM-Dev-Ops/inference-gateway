//! # JWT/OIDC Authentication Middleware
//!
//! Enterprise-grade authentication middleware supporting:
//! - JWT Bearer token validation
//! - OIDC provider integration (Auth0, Okta, Keycloak, Azure AD, etc.)
//! - JWKS (JSON Web Key Set) fetching and caching
//! - Role-based access control (RBAC)
//! - API key authentication
//! - Multiple authentication methods
//!
//! ## Features
//!
//! - **JWT Validation**: Validates JWT tokens with configurable claims
//! - **JWKS Caching**: Automatic key rotation with configurable refresh
//! - **Multiple Issuers**: Support for multiple OIDC providers
//! - **Custom Claims**: Extract custom claims for tenant isolation
//! - **Flexible Auth**: Support both JWT and API key authentication
//!
//! ## Example
//!
//! ```rust,ignore
//! use gateway_server::auth::{AuthConfig, AuthMiddleware, JwtConfig};
//!
//! let config = AuthConfig::builder()
//!     .jwt(JwtConfig::oidc("https://auth.example.com"))
//!     .required_claims(vec!["sub", "email"])
//!     .build();
//!
//! let auth = AuthMiddleware::new(config).await?;
//! ```

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use jsonwebtoken::{
    decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, TokenData, Validation,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::{debug, error, info, warn};

/// Authentication configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// JWT configuration (optional)
    pub jwt: Option<JwtConfig>,
    /// API key configuration (optional)
    pub api_keys: Option<ApiKeyConfig>,
    /// Required authentication (if false, unauthenticated requests are allowed)
    pub required: bool,
    /// Required claims that must be present in JWT
    pub required_claims: Vec<String>,
    /// Required scopes for access
    pub required_scopes: Vec<String>,
    /// Custom claim to use as tenant ID
    pub tenant_claim: Option<String>,
    /// Custom claim to use as user ID
    pub user_claim: Option<String>,
    /// Paths that bypass authentication
    pub public_paths: Vec<String>,
    /// Enable detailed auth error messages (disable in production)
    pub verbose_errors: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt: None,
            api_keys: None,
            required: true,
            required_claims: vec!["sub".to_string()],
            required_scopes: Vec::new(),
            tenant_claim: Some("tenant_id".to_string()),
            user_claim: Some("sub".to_string()),
            public_paths: vec![
                "/health".to_string(),
                "/health/live".to_string(),
                "/health/ready".to_string(),
                "/metrics".to_string(),
            ],
            verbose_errors: false,
        }
    }
}

impl AuthConfig {
    /// Create a new builder
    pub fn builder() -> AuthConfigBuilder {
        AuthConfigBuilder::default()
    }

    /// Check if a path is public (bypasses auth)
    pub fn is_public_path(&self, path: &str) -> bool {
        self.public_paths.iter().any(|p| {
            if p.ends_with('*') {
                path.starts_with(&p[..p.len() - 1])
            } else {
                path == p
            }
        })
    }

    /// Check if authentication is disabled
    pub fn is_disabled(&self) -> bool {
        self.jwt.is_none() && self.api_keys.is_none()
    }
}

/// Builder for `AuthConfig`
#[derive(Debug, Default)]
pub struct AuthConfigBuilder {
    jwt: Option<JwtConfig>,
    api_keys: Option<ApiKeyConfig>,
    required: Option<bool>,
    required_claims: Option<Vec<String>>,
    required_scopes: Option<Vec<String>>,
    tenant_claim: Option<String>,
    user_claim: Option<String>,
    public_paths: Option<Vec<String>>,
    verbose_errors: Option<bool>,
}

impl AuthConfigBuilder {
    /// Set JWT configuration
    pub fn jwt(mut self, config: JwtConfig) -> Self {
        self.jwt = Some(config);
        self
    }

    /// Set API key configuration
    pub fn api_keys(mut self, config: ApiKeyConfig) -> Self {
        self.api_keys = Some(config);
        self
    }

    /// Set whether authentication is required
    pub fn required(mut self, required: bool) -> Self {
        self.required = Some(required);
        self
    }

    /// Set required claims
    pub fn required_claims(mut self, claims: Vec<String>) -> Self {
        self.required_claims = Some(claims);
        self
    }

    /// Set required scopes
    pub fn required_scopes(mut self, scopes: Vec<String>) -> Self {
        self.required_scopes = Some(scopes);
        self
    }

    /// Set tenant claim name
    pub fn tenant_claim(mut self, claim: impl Into<String>) -> Self {
        self.tenant_claim = Some(claim.into());
        self
    }

    /// Set user claim name
    pub fn user_claim(mut self, claim: impl Into<String>) -> Self {
        self.user_claim = Some(claim.into());
        self
    }

    /// Set public paths
    pub fn public_paths(mut self, paths: Vec<String>) -> Self {
        self.public_paths = Some(paths);
        self
    }

    /// Enable verbose error messages
    pub fn verbose_errors(mut self, verbose: bool) -> Self {
        self.verbose_errors = Some(verbose);
        self
    }

    /// Build the configuration
    pub fn build(self) -> AuthConfig {
        let default = AuthConfig::default();
        AuthConfig {
            jwt: self.jwt,
            api_keys: self.api_keys,
            required: self.required.unwrap_or(default.required),
            required_claims: self.required_claims.unwrap_or(default.required_claims),
            required_scopes: self.required_scopes.unwrap_or(default.required_scopes),
            tenant_claim: self.tenant_claim.or(default.tenant_claim),
            user_claim: self.user_claim.or(default.user_claim),
            public_paths: self.public_paths.unwrap_or(default.public_paths),
            verbose_errors: self.verbose_errors.unwrap_or(default.verbose_errors),
        }
    }
}

/// JWT configuration
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// Token validation mode
    pub mode: JwtMode,
    /// Expected issuer(s)
    pub issuers: Vec<String>,
    /// Expected audience(s)
    pub audiences: Vec<String>,
    /// Allowed algorithms
    pub algorithms: Vec<Algorithm>,
    /// Clock skew tolerance
    pub leeway: Duration,
    /// JWKS cache TTL
    pub jwks_cache_ttl: Duration,
}

impl JwtConfig {
    /// Create OIDC configuration with discovery
    pub fn oidc(issuer_url: impl Into<String>) -> Self {
        let issuer = issuer_url.into();
        Self {
            mode: JwtMode::Oidc {
                discovery_url: format!(
                    "{}/.well-known/openid-configuration",
                    issuer.trim_end_matches('/')
                ),
            },
            issuers: vec![issuer],
            audiences: Vec::new(),
            algorithms: vec![Algorithm::RS256, Algorithm::RS384, Algorithm::RS512],
            leeway: Duration::from_secs(60),
            jwks_cache_ttl: Duration::from_secs(3600),
        }
    }

    /// Create configuration with static JWKS URL
    pub fn jwks(jwks_url: impl Into<String>) -> Self {
        Self {
            mode: JwtMode::Jwks {
                url: jwks_url.into(),
            },
            issuers: Vec::new(),
            audiences: Vec::new(),
            algorithms: vec![Algorithm::RS256, Algorithm::RS384, Algorithm::RS512],
            leeway: Duration::from_secs(60),
            jwks_cache_ttl: Duration::from_secs(3600),
        }
    }

    /// Create configuration with static secret (HMAC)
    pub fn secret(secret: impl Into<String>) -> Self {
        Self {
            mode: JwtMode::Secret {
                secret: secret.into(),
            },
            issuers: Vec::new(),
            audiences: Vec::new(),
            algorithms: vec![Algorithm::HS256, Algorithm::HS384, Algorithm::HS512],
            leeway: Duration::from_secs(60),
            jwks_cache_ttl: Duration::from_secs(0), // Not used for secrets
        }
    }

    /// Create configuration with static public key (RSA/EC)
    pub fn public_key(pem: impl Into<String>) -> Self {
        Self {
            mode: JwtMode::PublicKey { pem: pem.into() },
            issuers: Vec::new(),
            audiences: Vec::new(),
            algorithms: vec![Algorithm::RS256, Algorithm::RS384, Algorithm::RS512],
            leeway: Duration::from_secs(60),
            jwks_cache_ttl: Duration::from_secs(0), // Not used for static keys
        }
    }

    /// Set expected issuers
    pub fn with_issuers(mut self, issuers: Vec<String>) -> Self {
        self.issuers = issuers;
        self
    }

    /// Set expected audiences
    pub fn with_audiences(mut self, audiences: Vec<String>) -> Self {
        self.audiences = audiences;
        self
    }

    /// Set allowed algorithms
    pub fn with_algorithms(mut self, algorithms: Vec<Algorithm>) -> Self {
        self.algorithms = algorithms;
        self
    }

    /// Set clock skew leeway
    pub fn with_leeway(mut self, leeway: Duration) -> Self {
        self.leeway = leeway;
        self
    }

    /// Set JWKS cache TTL
    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.jwks_cache_ttl = ttl;
        self
    }
}

/// JWT validation mode
#[derive(Debug, Clone)]
pub enum JwtMode {
    /// OIDC discovery
    Oidc {
        /// Discovery endpoint URL
        discovery_url: String,
    },
    /// Static JWKS URL
    Jwks {
        /// JWKS endpoint URL
        url: String,
    },
    /// Symmetric secret (HMAC)
    Secret {
        /// Shared secret
        secret: String,
    },
    /// Static public key
    PublicKey {
        /// PEM-encoded public key
        pem: String,
    },
}

/// API key configuration
#[derive(Debug, Clone)]
pub struct ApiKeyConfig {
    /// Header name for API key (default: X-API-Key)
    pub header_name: String,
    /// Query parameter name (optional)
    pub query_param: Option<String>,
    /// Static API keys with metadata
    pub keys: HashMap<String, ApiKeyMetadata>,
    /// Enable hashed key storage
    pub hash_keys: bool,
}

impl Default for ApiKeyConfig {
    fn default() -> Self {
        Self {
            header_name: "X-API-Key".to_string(),
            query_param: None,
            keys: HashMap::new(),
            hash_keys: false,
        }
    }
}

impl ApiKeyConfig {
    /// Create new API key configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an API key
    pub fn with_key(
        mut self,
        key: impl Into<String>,
        metadata: ApiKeyMetadata,
    ) -> Self {
        let key_str = key.into();
        let storage_key = if self.hash_keys {
            hash_api_key(&key_str)
        } else {
            key_str
        };
        self.keys.insert(storage_key, metadata);
        self
    }

    /// Set custom header name
    pub fn with_header(mut self, header: impl Into<String>) -> Self {
        self.header_name = header.into();
        self
    }

    /// Enable hashed key storage
    pub fn with_hashing(mut self, enabled: bool) -> Self {
        self.hash_keys = enabled;
        self
    }
}

/// API key metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiKeyMetadata {
    /// Key name/description
    pub name: Option<String>,
    /// Associated tenant ID
    pub tenant_id: Option<String>,
    /// Associated user ID
    pub user_id: Option<String>,
    /// Allowed scopes
    pub scopes: Vec<String>,
    /// Rate limit override
    pub rate_limit: Option<u32>,
    /// Expiration time
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether the key is enabled
    pub enabled: bool,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

impl ApiKeyMetadata {
    /// Create new API key metadata
    pub fn new() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }

    /// Set tenant ID
    pub fn with_tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    /// Set user ID
    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set scopes
    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.scopes = scopes;
        self
    }

    /// Set expiration
    pub fn with_expiration(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }
}

/// Hash an API key for secure storage
fn hash_api_key(key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Hex encoding for API key hashing
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

/// Authenticated user/client information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedEntity {
    /// Entity ID (from sub claim or API key user)
    pub id: String,
    /// Tenant ID (if multi-tenant)
    pub tenant_id: Option<String>,
    /// Email (if available)
    pub email: Option<String>,
    /// Name (if available)
    pub name: Option<String>,
    /// Authentication method used
    pub auth_method: AuthMethod,
    /// Scopes/permissions
    pub scopes: Vec<String>,
    /// Token expiration (if JWT)
    pub expires_at: Option<DateTime<Utc>>,
    /// Custom claims/metadata
    pub claims: HashMap<String, serde_json::Value>,
}

/// Authentication method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    /// JWT Bearer token
    Jwt,
    /// API key
    ApiKey,
    /// Basic auth (username/password)
    Basic,
    /// Anonymous (unauthenticated)
    Anonymous,
}

/// JWT claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (user ID)
    pub sub: Option<String>,
    /// Issuer
    pub iss: Option<String>,
    /// Audience
    pub aud: Option<ClaimValue>,
    /// Expiration time
    pub exp: Option<i64>,
    /// Not before time
    pub nbf: Option<i64>,
    /// Issued at time
    pub iat: Option<i64>,
    /// JWT ID
    pub jti: Option<String>,
    /// Email
    pub email: Option<String>,
    /// Email verified
    pub email_verified: Option<bool>,
    /// Name
    pub name: Option<String>,
    /// Given name
    pub given_name: Option<String>,
    /// Family name
    pub family_name: Option<String>,
    /// Scopes (space-separated string or array)
    pub scope: Option<String>,
    /// Scopes as array
    pub scopes: Option<Vec<String>>,
    /// Azure AD specific: roles
    pub roles: Option<Vec<String>>,
    /// Tenant ID (custom claim)
    pub tenant_id: Option<String>,
    /// Organization ID (custom claim)
    pub org_id: Option<String>,
    /// Additional claims (catch-all)
    #[serde(flatten)]
    pub additional: HashMap<String, serde_json::Value>,
}

/// Claim value that can be string or array of strings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClaimValue {
    /// Single value
    Single(String),
    /// Multiple values
    Multiple(Vec<String>),
}

impl ClaimValue {
    /// Check if the claim contains a value
    pub fn contains(&self, value: &str) -> bool {
        match self {
            Self::Single(s) => s == value,
            Self::Multiple(v) => v.contains(&value.to_string()),
        }
    }

    /// Get all values as a vector
    pub fn as_vec(&self) -> Vec<String> {
        match self {
            Self::Single(s) => vec![s.clone()],
            Self::Multiple(v) => v.clone(),
        }
    }
}

/// OIDC discovery document
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct OidcDiscovery {
    issuer: String,
    jwks_uri: String,
    #[serde(default)]
    authorization_endpoint: Option<String>,
    #[serde(default)]
    token_endpoint: Option<String>,
    #[serde(default)]
    id_token_signing_alg_values_supported: Vec<String>,
}

/// Cached JWKS
struct CachedJwks {
    jwks: JwkSet,
    fetched_at: Instant,
}

/// Authentication middleware state
#[derive(Clone)]
pub struct AuthState {
    config: Arc<AuthConfig>,
    http_client: Client,
    jwks_cache: Arc<DashMap<String, CachedJwks>>,
    oidc_cache: Arc<DashMap<String, OidcDiscovery>>,
    static_key: Option<DecodingKey>,
}

impl AuthState {
    /// Create new authentication state
    pub async fn new(config: AuthConfig) -> Result<Self, AuthError> {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| AuthError::Configuration(format!("Failed to create HTTP client: {e}")))?;

        // Pre-compute static decoding key if applicable
        let static_key = if let Some(jwt_config) = &config.jwt {
            match &jwt_config.mode {
                JwtMode::Secret { secret } => {
                    Some(DecodingKey::from_secret(secret.as_bytes()))
                }
                JwtMode::PublicKey { pem } => {
                    Some(DecodingKey::from_rsa_pem(pem.as_bytes()).map_err(|e| {
                        AuthError::Configuration(format!("Invalid RSA public key: {e}"))
                    })?)
                }
                _ => None,
            }
        } else {
            None
        };

        let state = Self {
            config: Arc::new(config),
            http_client,
            jwks_cache: Arc::new(DashMap::new()),
            oidc_cache: Arc::new(DashMap::new()),
            static_key,
        };

        // Pre-fetch JWKS if using OIDC/JWKS mode
        if let Some(jwt_config) = &state.config.jwt {
            match &jwt_config.mode {
                JwtMode::Oidc { discovery_url } => {
                    state.fetch_oidc_config(discovery_url).await?;
                }
                JwtMode::Jwks { url } => {
                    state.fetch_jwks(url).await?;
                }
                _ => {}
            }
        }

        Ok(state)
    }

    /// Create a disabled auth state
    pub fn disabled() -> Self {
        Self {
            config: Arc::new(AuthConfig {
                jwt: None,
                api_keys: None,
                required: false,
                ..Default::default()
            }),
            http_client: Client::new(),
            jwks_cache: Arc::new(DashMap::new()),
            oidc_cache: Arc::new(DashMap::new()),
            static_key: None,
        }
    }

    /// Fetch OIDC discovery document
    async fn fetch_oidc_config(&self, discovery_url: &str) -> Result<OidcDiscovery, AuthError> {
        // Check cache first
        if let Some(cached) = self.oidc_cache.get(discovery_url) {
            return Ok(cached.clone());
        }

        debug!(url = %discovery_url, "Fetching OIDC discovery document");

        let response = self
            .http_client
            .get(discovery_url)
            .send()
            .await
            .map_err(|e| AuthError::Configuration(format!("Failed to fetch OIDC config: {e}")))?;

        if !response.status().is_success() {
            return Err(AuthError::Configuration(format!(
                "OIDC discovery failed with status: {}",
                response.status()
            )));
        }

        let discovery: OidcDiscovery = response.json().await.map_err(|e| {
            AuthError::Configuration(format!("Failed to parse OIDC config: {e}"))
        })?;

        info!(issuer = %discovery.issuer, "Loaded OIDC configuration");

        // Cache the discovery document
        self.oidc_cache
            .insert(discovery_url.to_string(), discovery.clone());

        // Also fetch JWKS
        self.fetch_jwks(&discovery.jwks_uri).await?;

        Ok(discovery)
    }

    /// Fetch JWKS from URL
    async fn fetch_jwks(&self, jwks_url: &str) -> Result<JwkSet, AuthError> {
        // Check cache first
        if let Some(cached) = self.jwks_cache.get(jwks_url) {
            if let Some(jwt_config) = &self.config.jwt {
                if cached.fetched_at.elapsed() < jwt_config.jwks_cache_ttl {
                    return Ok(cached.jwks.clone());
                }
            }
        }

        debug!(url = %jwks_url, "Fetching JWKS");

        let response = self
            .http_client
            .get(jwks_url)
            .send()
            .await
            .map_err(|e| AuthError::Configuration(format!("Failed to fetch JWKS: {e}")))?;

        if !response.status().is_success() {
            return Err(AuthError::Configuration(format!(
                "JWKS fetch failed with status: {}",
                response.status()
            )));
        }

        let jwks: JwkSet = response
            .json()
            .await
            .map_err(|e| AuthError::Configuration(format!("Failed to parse JWKS: {e}")))?;

        info!(keys = jwks.keys.len(), "Loaded JWKS");

        // Cache the JWKS
        self.jwks_cache.insert(
            jwks_url.to_string(),
            CachedJwks {
                jwks: jwks.clone(),
                fetched_at: Instant::now(),
            },
        );

        Ok(jwks)
    }

    /// Get decoding key for a JWT
    async fn get_decoding_key(
        &self,
        token: &str,
    ) -> Result<(DecodingKey, Algorithm), AuthError> {
        let jwt_config = self
            .config
            .jwt
            .as_ref()
            .ok_or(AuthError::Configuration("JWT not configured".to_string()))?;

        match &jwt_config.mode {
            JwtMode::Secret { .. } | JwtMode::PublicKey { .. } => {
                let key = self
                    .static_key
                    .clone()
                    .ok_or(AuthError::Configuration("Static key not initialized".to_string()))?;
                let algo = jwt_config.algorithms.first().copied().unwrap_or(Algorithm::RS256);
                Ok((key, algo))
            }
            JwtMode::Oidc { discovery_url } => {
                let discovery = self.fetch_oidc_config(discovery_url).await?;
                self.get_key_from_jwks(&discovery.jwks_uri, token).await
            }
            JwtMode::Jwks { url } => self.get_key_from_jwks(url, token).await,
        }
    }

    /// Get decoding key from JWKS based on token header
    async fn get_key_from_jwks(
        &self,
        jwks_url: &str,
        token: &str,
    ) -> Result<(DecodingKey, Algorithm), AuthError> {
        let header = decode_header(token).map_err(|e| {
            AuthError::InvalidToken(format!("Failed to decode token header: {e}"))
        })?;

        let jwks = self.fetch_jwks(jwks_url).await?;

        // Find the key by kid
        let kid = header
            .kid
            .ok_or_else(|| AuthError::InvalidToken("Token missing kid header".to_string()))?;

        let jwk = jwks
            .find(&kid)
            .ok_or_else(|| AuthError::InvalidToken(format!("Key not found in JWKS: {kid}")))?;

        let key = DecodingKey::from_jwk(jwk)
            .map_err(|e| AuthError::InvalidToken(format!("Invalid JWK: {e}")))?;

        Ok((key, header.alg))
    }

    /// Validate a JWT token
    async fn validate_jwt(&self, token: &str) -> Result<AuthenticatedEntity, AuthError> {
        let jwt_config = self
            .config
            .jwt
            .as_ref()
            .ok_or(AuthError::Configuration("JWT not configured".to_string()))?;

        let (decoding_key, algorithm) = self.get_decoding_key(token).await?;

        // Build validation
        let mut validation = Validation::new(algorithm);
        validation.leeway = jwt_config.leeway.as_secs();

        // Set allowed algorithms
        validation.algorithms = jwt_config.algorithms.clone();

        // Set issuer validation
        if !jwt_config.issuers.is_empty() {
            validation.set_issuer(&jwt_config.issuers);
        }

        // Set audience validation
        if !jwt_config.audiences.is_empty() {
            validation.set_audience(&jwt_config.audiences);
        }

        // Decode and validate token
        let token_data: TokenData<JwtClaims> =
            decode(token, &decoding_key, &validation).map_err(|e| {
                debug!(error = %e, "JWT validation failed");
                AuthError::InvalidToken(format!("Token validation failed: {e}"))
            })?;

        let claims = token_data.claims;

        // Check required claims
        for required_claim in &self.config.required_claims {
            if !self.has_claim(&claims, required_claim) {
                return Err(AuthError::MissingClaim(required_claim.clone()));
            }
        }

        // Extract scopes
        let scopes = self.extract_scopes(&claims);

        // Check required scopes
        if !self.config.required_scopes.is_empty() {
            let scope_set: HashSet<_> = scopes.iter().collect();
            for required_scope in &self.config.required_scopes {
                if !scope_set.contains(required_scope) {
                    return Err(AuthError::InsufficientScope(required_scope.clone()));
                }
            }
        }

        // Extract tenant ID
        let tenant_id = self.extract_tenant_id(&claims);

        // Build authenticated entity
        let entity = AuthenticatedEntity {
            id: claims.sub.clone().unwrap_or_else(|| "unknown".to_string()),
            tenant_id,
            email: claims.email.clone(),
            name: claims.name.clone(),
            auth_method: AuthMethod::Jwt,
            scopes,
            expires_at: claims.exp.map(|e| {
                DateTime::from_timestamp(e, 0).unwrap_or_else(Utc::now)
            }),
            claims: claims.additional.clone(),
        };

        Ok(entity)
    }

    /// Check if a claim exists
    fn has_claim(&self, claims: &JwtClaims, claim_name: &str) -> bool {
        match claim_name {
            "sub" => claims.sub.is_some(),
            "iss" => claims.iss.is_some(),
            "aud" => claims.aud.is_some(),
            "exp" => claims.exp.is_some(),
            "email" => claims.email.is_some(),
            "name" => claims.name.is_some(),
            "tenant_id" => claims.tenant_id.is_some(),
            "org_id" => claims.org_id.is_some(),
            _ => claims.additional.contains_key(claim_name),
        }
    }

    /// Extract scopes from claims
    fn extract_scopes(&self, claims: &JwtClaims) -> Vec<String> {
        let mut scopes = Vec::new();

        // Try scopes array first
        if let Some(s) = &claims.scopes {
            scopes.extend(s.clone());
        }

        // Try space-separated scope string
        if let Some(scope_str) = &claims.scope {
            scopes.extend(scope_str.split_whitespace().map(String::from));
        }

        // Include Azure AD roles
        if let Some(roles) = &claims.roles {
            scopes.extend(roles.clone());
        }

        scopes
    }

    /// Extract tenant ID from claims
    fn extract_tenant_id(&self, claims: &JwtClaims) -> Option<String> {
        // Try configured tenant claim
        if let Some(tenant_claim) = &self.config.tenant_claim {
            if tenant_claim == "tenant_id" {
                if let Some(tid) = &claims.tenant_id {
                    return Some(tid.clone());
                }
            } else if tenant_claim == "org_id" {
                if let Some(oid) = &claims.org_id {
                    return Some(oid.clone());
                }
            } else if let Some(value) = claims.additional.get(tenant_claim) {
                if let Some(s) = value.as_str() {
                    return Some(s.to_string());
                }
            }
        }

        // Fall back to tenant_id or org_id
        claims
            .tenant_id
            .clone()
            .or_else(|| claims.org_id.clone())
    }

    /// Validate an API key
    fn validate_api_key(&self, key: &str) -> Result<AuthenticatedEntity, AuthError> {
        let api_config = self
            .config
            .api_keys
            .as_ref()
            .ok_or(AuthError::Configuration("API keys not configured".to_string()))?;

        let lookup_key = if api_config.hash_keys {
            hash_api_key(key)
        } else {
            key.to_string()
        };

        let metadata = api_config
            .keys
            .get(&lookup_key)
            .ok_or(AuthError::InvalidApiKey)?;

        // Check if key is enabled
        if !metadata.enabled {
            return Err(AuthError::InvalidApiKey);
        }

        // Check expiration
        if let Some(expires_at) = metadata.expires_at {
            if Utc::now() > expires_at {
                return Err(AuthError::ExpiredCredential);
            }
        }

        // Check required scopes
        if !self.config.required_scopes.is_empty() {
            let scope_set: HashSet<_> = metadata.scopes.iter().collect();
            for required_scope in &self.config.required_scopes {
                if !scope_set.contains(required_scope) {
                    return Err(AuthError::InsufficientScope(required_scope.clone()));
                }
            }
        }

        Ok(AuthenticatedEntity {
            id: metadata
                .user_id
                .clone()
                .unwrap_or_else(|| "api-key-user".to_string()),
            tenant_id: metadata.tenant_id.clone(),
            email: None,
            name: metadata.name.clone(),
            auth_method: AuthMethod::ApiKey,
            scopes: metadata.scopes.clone(),
            expires_at: metadata.expires_at,
            claims: metadata
                .metadata
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect(),
        })
    }

    /// Authenticate a request
    pub async fn authenticate(&self, request: &Request) -> Result<AuthenticatedEntity, AuthError> {
        // Check for Bearer token
        if let Some(auth_header) = request.headers().get(header::AUTHORIZATION) {
            if let Ok(auth_str) = auth_header.to_str() {
                if let Some(token) = auth_str.strip_prefix("Bearer ") {
                    return self.validate_jwt(token.trim()).await;
                }
            }
        }

        // Check for API key
        if let Some(api_config) = &self.config.api_keys {
            // Check header
            if let Some(key_header) = request.headers().get(&api_config.header_name) {
                if let Ok(key) = key_header.to_str() {
                    return self.validate_api_key(key);
                }
            }

            // Check Authorization header with Basic scheme (for API keys)
            if let Some(auth_header) = request.headers().get(header::AUTHORIZATION) {
                if let Ok(auth_str) = auth_header.to_str() {
                    if let Some(encoded) = auth_str.strip_prefix("Basic ") {
                        if let Ok(decoded) = URL_SAFE_NO_PAD.decode(encoded.trim()) {
                            if let Ok(credentials) = String::from_utf8(decoded) {
                                // Expect format "api-key:secret" or just ":secret"
                                if let Some(key) = credentials.strip_prefix(':') {
                                    return self.validate_api_key(key);
                                } else if let Some((_, key)) = credentials.split_once(':') {
                                    return self.validate_api_key(key);
                                }
                            }
                        }
                    }
                }
            }
        }

        // No authentication found
        if self.config.required {
            Err(AuthError::MissingCredentials)
        } else {
            // Return anonymous entity
            Ok(AuthenticatedEntity {
                id: "anonymous".to_string(),
                tenant_id: None,
                email: None,
                name: None,
                auth_method: AuthMethod::Anonymous,
                scopes: Vec::new(),
                expires_at: None,
                claims: HashMap::new(),
            })
        }
    }
}

/// Authentication error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum AuthError {
    /// Missing authentication credentials
    #[error("Authentication required")]
    MissingCredentials,

    /// Invalid JWT token
    #[error("Invalid token: {0}")]
    InvalidToken(String),

    /// Invalid API key
    #[error("Invalid API key")]
    InvalidApiKey,

    /// Expired credentials
    #[error("Credentials expired")]
    ExpiredCredential,

    /// Missing required claim
    #[error("Missing required claim: {0}")]
    MissingClaim(String),

    /// Insufficient scope/permissions
    #[error("Insufficient scope: {0}")]
    InsufficientScope(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),
}

impl AuthError {
    /// Get HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::MissingCredentials => StatusCode::UNAUTHORIZED,
            Self::InvalidToken(_) | Self::InvalidApiKey => StatusCode::UNAUTHORIZED,
            Self::ExpiredCredential => StatusCode::UNAUTHORIZED,
            Self::MissingClaim(_) => StatusCode::FORBIDDEN,
            Self::InsufficientScope(_) => StatusCode::FORBIDDEN,
            Self::Configuration(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get error code for API response
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::MissingCredentials => "authentication_required",
            Self::InvalidToken(_) => "invalid_token",
            Self::InvalidApiKey => "invalid_api_key",
            Self::ExpiredCredential => "expired_credentials",
            Self::MissingClaim(_) => "missing_claim",
            Self::InsufficientScope(_) => "insufficient_scope",
            Self::Configuration(_) => "configuration_error",
        }
    }
}

/// Authentication middleware
pub async fn auth_middleware(
    State(state): State<AuthState>,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();

    // Skip auth for public paths
    if state.config.is_public_path(&path) {
        return next.run(request).await;
    }

    // Skip auth if disabled
    if state.config.is_disabled() && !state.config.required {
        return next.run(request).await;
    }

    // Authenticate the request
    match state.authenticate(&request).await {
        Ok(entity) => {
            debug!(
                user_id = %entity.id,
                tenant_id = ?entity.tenant_id,
                method = ?entity.auth_method,
                "Request authenticated"
            );

            // Add authenticated entity to request extensions
            request.extensions_mut().insert(entity);

            next.run(request).await
        }
        Err(err) => {
            warn!(error = %err, path = %path, "Authentication failed");

            let status = err.status_code();
            let error_code = err.error_code();

            let body = if state.config.verbose_errors {
                serde_json::json!({
                    "error": {
                        "type": error_code,
                        "message": err.to_string(),
                    }
                })
            } else {
                serde_json::json!({
                    "error": {
                        "type": error_code,
                        "message": match err {
                            AuthError::MissingCredentials => "Authentication required",
                            AuthError::InvalidToken(_) | AuthError::InvalidApiKey => "Invalid credentials",
                            AuthError::ExpiredCredential => "Credentials expired",
                            AuthError::MissingClaim(_) | AuthError::InsufficientScope(_) => "Access denied",
                            AuthError::Configuration(_) => "Authentication service error",
                        },
                    }
                })
            };

            let mut response = (
                status,
                [(header::CONTENT_TYPE, "application/json")],
                serde_json::to_string(&body).unwrap_or_default(),
            )
                .into_response();

            // Add WWW-Authenticate header for 401 responses
            if status == StatusCode::UNAUTHORIZED {
                response.headers_mut().insert(
                    header::WWW_AUTHENTICATE,
                    "Bearer realm=\"api\", error=\"invalid_token\""
                        .parse()
                        .unwrap_or_else(|_| header::HeaderValue::from_static("Bearer")),
                );
            }

            response
        }
    }
}

/// Extension trait for extracting authenticated entity from request
pub trait AuthenticatedEntityExt {
    /// Get the authenticated entity if present
    fn authenticated_entity(&self) -> Option<&AuthenticatedEntity>;
}

impl AuthenticatedEntityExt for Request {
    fn authenticated_entity(&self) -> Option<&AuthenticatedEntity> {
        self.extensions().get::<AuthenticatedEntity>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http::Request as HttpRequest;

    fn make_request_with_header(uri: &str, header: Option<(&str, &str)>) -> Request {
        let mut builder = HttpRequest::builder().uri(uri);
        if let Some((name, value)) = header {
            builder = builder.header(name, value);
        }
        builder.body(Body::empty()).unwrap()
    }

    #[test]
    fn test_auth_config_builder() {
        let config = AuthConfig::builder()
            .required(true)
            .required_claims(vec!["sub".to_string(), "email".to_string()])
            .required_scopes(vec!["read".to_string()])
            .tenant_claim("org_id")
            .verbose_errors(true)
            .build();

        assert!(config.required);
        assert_eq!(config.required_claims.len(), 2);
        assert_eq!(config.required_scopes.len(), 1);
        assert_eq!(config.tenant_claim, Some("org_id".to_string()));
        assert!(config.verbose_errors);
    }

    #[test]
    fn test_jwt_config_oidc() {
        let config = JwtConfig::oidc("https://auth.example.com");

        match config.mode {
            JwtMode::Oidc { discovery_url } => {
                assert_eq!(
                    discovery_url,
                    "https://auth.example.com/.well-known/openid-configuration"
                );
            }
            _ => panic!("Expected OIDC mode"),
        }

        assert_eq!(config.issuers, vec!["https://auth.example.com"]);
    }

    #[test]
    fn test_jwt_config_secret() {
        let config = JwtConfig::secret("my-secret-key");

        match config.mode {
            JwtMode::Secret { secret } => {
                assert_eq!(secret, "my-secret-key");
            }
            _ => panic!("Expected Secret mode"),
        }

        assert!(config.algorithms.contains(&Algorithm::HS256));
    }

    #[test]
    fn test_api_key_config() {
        let config = ApiKeyConfig::new()
            .with_header("X-Custom-Key")
            .with_key(
                "test-key-123",
                ApiKeyMetadata::new()
                    .with_tenant("tenant-1")
                    .with_scopes(vec!["read".to_string(), "write".to_string()]),
            );

        assert_eq!(config.header_name, "X-Custom-Key");
        assert!(config.keys.contains_key("test-key-123"));

        let metadata = config.keys.get("test-key-123").unwrap();
        assert_eq!(metadata.tenant_id, Some("tenant-1".to_string()));
        assert_eq!(metadata.scopes.len(), 2);
    }

    #[test]
    fn test_api_key_hashing() {
        let config = ApiKeyConfig::new().with_hashing(true).with_key(
            "secret-key",
            ApiKeyMetadata::new(),
        );

        // Key should be hashed, not stored in plain text
        assert!(!config.keys.contains_key("secret-key"));
        // Hash should be stored
        let hash = hash_api_key("secret-key");
        assert!(config.keys.contains_key(&hash));
    }

    #[test]
    fn test_public_path_matching() {
        let config = AuthConfig::builder()
            .public_paths(vec![
                "/health".to_string(),
                "/health/*".to_string(),
                "/public/docs".to_string(),
            ])
            .build();

        assert!(config.is_public_path("/health"));
        assert!(config.is_public_path("/health/live"));
        assert!(config.is_public_path("/health/ready"));
        assert!(config.is_public_path("/public/docs"));
        assert!(!config.is_public_path("/api/v1/chat"));
        assert!(!config.is_public_path("/public/other"));
    }

    #[test]
    fn test_claim_value() {
        let single = ClaimValue::Single("value".to_string());
        assert!(single.contains("value"));
        assert!(!single.contains("other"));
        assert_eq!(single.as_vec(), vec!["value"]);

        let multiple = ClaimValue::Multiple(vec!["a".to_string(), "b".to_string()]);
        assert!(multiple.contains("a"));
        assert!(multiple.contains("b"));
        assert!(!multiple.contains("c"));
        assert_eq!(multiple.as_vec(), vec!["a", "b"]);
    }

    #[test]
    fn test_auth_error_status_codes() {
        assert_eq!(
            AuthError::MissingCredentials.status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AuthError::InvalidToken("test".to_string()).status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(AuthError::InvalidApiKey.status_code(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            AuthError::ExpiredCredential.status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AuthError::MissingClaim("sub".to_string()).status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            AuthError::InsufficientScope("read".to_string()).status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            AuthError::Configuration("test".to_string()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_auth_error_codes() {
        assert_eq!(AuthError::MissingCredentials.error_code(), "authentication_required");
        assert_eq!(
            AuthError::InvalidToken("test".to_string()).error_code(),
            "invalid_token"
        );
        assert_eq!(AuthError::InvalidApiKey.error_code(), "invalid_api_key");
        assert_eq!(AuthError::ExpiredCredential.error_code(), "expired_credentials");
        assert_eq!(
            AuthError::MissingClaim("sub".to_string()).error_code(),
            "missing_claim"
        );
        assert_eq!(
            AuthError::InsufficientScope("read".to_string()).error_code(),
            "insufficient_scope"
        );
    }

    #[test]
    fn test_authenticated_entity() {
        let entity = AuthenticatedEntity {
            id: "user-123".to_string(),
            tenant_id: Some("tenant-1".to_string()),
            email: Some("user@example.com".to_string()),
            name: Some("Test User".to_string()),
            auth_method: AuthMethod::Jwt,
            scopes: vec!["read".to_string(), "write".to_string()],
            expires_at: None,
            claims: HashMap::new(),
        };

        assert_eq!(entity.id, "user-123");
        assert_eq!(entity.tenant_id, Some("tenant-1".to_string()));
        assert_eq!(entity.auth_method, AuthMethod::Jwt);
    }

    #[test]
    fn test_api_key_metadata_builder() {
        let expires = Utc::now() + chrono::Duration::days(30);
        let metadata = ApiKeyMetadata::new()
            .with_tenant("tenant-1")
            .with_user("user-1")
            .with_scopes(vec!["read".to_string()])
            .with_expiration(expires);

        assert_eq!(metadata.tenant_id, Some("tenant-1".to_string()));
        assert_eq!(metadata.user_id, Some("user-1".to_string()));
        assert_eq!(metadata.scopes, vec!["read"]);
        assert!(metadata.expires_at.is_some());
        assert!(metadata.enabled);
    }

    #[tokio::test]
    async fn test_auth_state_disabled() {
        let state = AuthState::disabled();
        assert!(state.config.is_disabled());
        assert!(!state.config.required);
    }

    #[tokio::test]
    async fn test_authenticate_with_valid_api_key() {
        let config = AuthConfig::builder()
            .api_keys(
                ApiKeyConfig::new().with_key(
                    "valid-api-key",
                    ApiKeyMetadata::new()
                        .with_tenant("tenant-1")
                        .with_user("user-1"),
                ),
            )
            .required(true)
            .build();

        let state = AuthState {
            config: Arc::new(config),
            http_client: Client::new(),
            jwks_cache: Arc::new(DashMap::new()),
            oidc_cache: Arc::new(DashMap::new()),
            static_key: None,
        };

        let request = make_request_with_header("/api", Some(("X-API-Key", "valid-api-key")));
        let result = state.authenticate(&request).await;

        assert!(result.is_ok());
        let entity = result.unwrap();
        assert_eq!(entity.id, "user-1");
        assert_eq!(entity.tenant_id, Some("tenant-1".to_string()));
        assert_eq!(entity.auth_method, AuthMethod::ApiKey);
    }

    #[tokio::test]
    async fn test_authenticate_with_invalid_api_key() {
        let config = AuthConfig::builder()
            .api_keys(
                ApiKeyConfig::new().with_key(
                    "valid-api-key",
                    ApiKeyMetadata::new().with_tenant("tenant-1"),
                ),
            )
            .required(true)
            .build();

        let state = AuthState {
            config: Arc::new(config),
            http_client: Client::new(),
            jwks_cache: Arc::new(DashMap::new()),
            oidc_cache: Arc::new(DashMap::new()),
            static_key: None,
        };

        let request = make_request_with_header("/api", Some(("X-API-Key", "invalid-key")));
        let result = state.authenticate(&request).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::InvalidApiKey));
    }

    #[tokio::test]
    async fn test_authenticate_missing_credentials() {
        let config = AuthConfig::builder()
            .api_keys(ApiKeyConfig::new())
            .required(true)
            .build();

        let state = AuthState {
            config: Arc::new(config),
            http_client: Client::new(),
            jwks_cache: Arc::new(DashMap::new()),
            oidc_cache: Arc::new(DashMap::new()),
            static_key: None,
        };

        let request = make_request_with_header("/api", None);
        let result = state.authenticate(&request).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::MissingCredentials));
    }

    #[tokio::test]
    async fn test_authenticate_anonymous_when_not_required() {
        let config = AuthConfig::builder()
            .required(false)
            .build();

        let state = AuthState {
            config: Arc::new(config),
            http_client: Client::new(),
            jwks_cache: Arc::new(DashMap::new()),
            oidc_cache: Arc::new(DashMap::new()),
            static_key: None,
        };

        let request = make_request_with_header("/api", None);
        let result = state.authenticate(&request).await;

        assert!(result.is_ok());
        let entity = result.unwrap();
        assert_eq!(entity.id, "anonymous");
        assert_eq!(entity.auth_method, AuthMethod::Anonymous);
    }

    #[tokio::test]
    async fn test_authenticate_expired_api_key() {
        let expired = Utc::now() - chrono::Duration::days(1);
        let config = AuthConfig::builder()
            .api_keys(
                ApiKeyConfig::new().with_key(
                    "expired-key",
                    ApiKeyMetadata::new().with_expiration(expired),
                ),
            )
            .required(true)
            .build();

        let state = AuthState {
            config: Arc::new(config),
            http_client: Client::new(),
            jwks_cache: Arc::new(DashMap::new()),
            oidc_cache: Arc::new(DashMap::new()),
            static_key: None,
        };

        let request = make_request_with_header("/api", Some(("X-API-Key", "expired-key")));
        let result = state.authenticate(&request).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::ExpiredCredential));
    }

    #[tokio::test]
    async fn test_authenticate_disabled_api_key() {
        let mut metadata = ApiKeyMetadata::new();
        metadata.enabled = false;

        let config = AuthConfig::builder()
            .api_keys(
                ApiKeyConfig::new().with_key("disabled-key", metadata),
            )
            .required(true)
            .build();

        let state = AuthState {
            config: Arc::new(config),
            http_client: Client::new(),
            jwks_cache: Arc::new(DashMap::new()),
            oidc_cache: Arc::new(DashMap::new()),
            static_key: None,
        };

        let request = make_request_with_header("/api", Some(("X-API-Key", "disabled-key")));
        let result = state.authenticate(&request).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::InvalidApiKey));
    }

    #[tokio::test]
    async fn test_api_key_scope_validation() {
        let config = AuthConfig::builder()
            .api_keys(
                ApiKeyConfig::new().with_key(
                    "limited-key",
                    ApiKeyMetadata::new().with_scopes(vec!["read".to_string()]),
                ),
            )
            .required_scopes(vec!["write".to_string()])
            .required(true)
            .build();

        let state = AuthState {
            config: Arc::new(config),
            http_client: Client::new(),
            jwks_cache: Arc::new(DashMap::new()),
            oidc_cache: Arc::new(DashMap::new()),
            static_key: None,
        };

        let request = make_request_with_header("/api", Some(("X-API-Key", "limited-key")));
        let result = state.authenticate(&request).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::InsufficientScope(_)));
    }

    #[tokio::test]
    async fn test_api_key_hashed_validation() {
        let config = AuthConfig::builder()
            .api_keys(
                ApiKeyConfig::new()
                    .with_hashing(true)
                    .with_key("my-secret-key", ApiKeyMetadata::new().with_tenant("tenant-1")),
            )
            .required(true)
            .build();

        let state = AuthState {
            config: Arc::new(config),
            http_client: Client::new(),
            jwks_cache: Arc::new(DashMap::new()),
            oidc_cache: Arc::new(DashMap::new()),
            static_key: None,
        };

        // Original key should still work (it gets hashed during validation)
        let request = make_request_with_header("/api", Some(("X-API-Key", "my-secret-key")));
        let result = state.authenticate(&request).await;

        assert!(result.is_ok());
        let entity = result.unwrap();
        assert_eq!(entity.tenant_id, Some("tenant-1".to_string()));
    }

    #[test]
    fn test_jwt_config_with_options() {
        let config = JwtConfig::oidc("https://auth.example.com")
            .with_audiences(vec!["api".to_string()])
            .with_algorithms(vec![Algorithm::RS256])
            .with_leeway(Duration::from_secs(30))
            .with_cache_ttl(Duration::from_secs(7200));

        assert_eq!(config.audiences, vec!["api"]);
        assert_eq!(config.algorithms, vec![Algorithm::RS256]);
        assert_eq!(config.leeway, Duration::from_secs(30));
        assert_eq!(config.jwks_cache_ttl, Duration::from_secs(7200));
    }

    #[test]
    fn test_auth_method_serialization() {
        assert_eq!(
            serde_json::to_string(&AuthMethod::Jwt).unwrap(),
            "\"jwt\""
        );
        assert_eq!(
            serde_json::to_string(&AuthMethod::ApiKey).unwrap(),
            "\"api_key\""
        );
        assert_eq!(
            serde_json::to_string(&AuthMethod::Anonymous).unwrap(),
            "\"anonymous\""
        );
    }

    #[test]
    fn test_jwt_config_jwks() {
        let config = JwtConfig::jwks("https://example.com/.well-known/jwks.json");

        match config.mode {
            JwtMode::Jwks { url } => {
                assert_eq!(url, "https://example.com/.well-known/jwks.json");
            }
            _ => panic!("Expected JWKS mode"),
        }
    }

    #[test]
    fn test_jwt_config_public_key() {
        let pem = "-----BEGIN PUBLIC KEY-----\nMIIB...test...key\n-----END PUBLIC KEY-----";
        let config = JwtConfig::public_key(pem);

        match config.mode {
            JwtMode::PublicKey { pem: p } => {
                assert_eq!(p, pem);
            }
            _ => panic!("Expected PublicKey mode"),
        }
    }

    #[test]
    fn test_auth_config_is_disabled() {
        let config = AuthConfig::builder().build();
        assert!(config.is_disabled());

        let config_with_jwt = AuthConfig::builder()
            .jwt(JwtConfig::secret("test"))
            .build();
        assert!(!config_with_jwt.is_disabled());

        let config_with_api_keys = AuthConfig::builder()
            .api_keys(ApiKeyConfig::new())
            .build();
        assert!(!config_with_api_keys.is_disabled());
    }
}
