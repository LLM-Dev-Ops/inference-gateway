# LLM Inference Gateway - Type Safety Rules

**Zero Runtime Errors Through Compile-Time Guarantees**

---

## 1. Newtype Pattern Specifications

### 1.1 Domain Value Types

```rust
// Temperature: Valid range [0.0, 2.0]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Temperature(f32);

impl Temperature {
    pub fn new(value: f32) -> Result<Self, ValidationError> {
        if !(0.0..=2.0).contains(&value) {
            return Err(ValidationError::InvalidTemperature {
                value,
                min: 0.0,
                max: 2.0
            });
        }
        Ok(Self(value))
    }

    pub fn value(&self) -> f32 { self.0 }
}

impl Default for Temperature {
    fn default() -> Self { Self(1.0) }
}

// MaxTokens: Valid range [1, 128000]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaxTokens(NonZeroU32);

impl MaxTokens {
    pub const MIN: u32 = 1;
    pub const MAX: u32 = 128_000;

    pub fn new(value: u32) -> Result<Self, ValidationError> {
        if value == 0 || value > Self::MAX {
            return Err(ValidationError::InvalidMaxTokens {
                value,
                min: Self::MIN,
                max: Self::MAX
            });
        }
        Ok(Self(NonZeroU32::new(value).unwrap()))
    }

    pub fn value(&self) -> u32 { self.0.get() }
}

// TopP: Valid range (0.0, 1.0]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct TopP(f32);

impl TopP {
    pub fn new(value: f32) -> Result<Self, ValidationError> {
        if value <= 0.0 || value > 1.0 {
            return Err(ValidationError::InvalidTopP {
                value,
                min_exclusive: 0.0,
                max_inclusive: 1.0
            });
        }
        Ok(Self(value))
    }

    pub fn value(&self) -> f32 { self.0 }
}

// TopK: Valid range [1, ∞)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TopK(NonZeroU32);

impl TopK {
    pub fn new(value: u32) -> Result<Self, ValidationError> {
        NonZeroU32::new(value)
            .map(Self)
            .ok_or(ValidationError::InvalidTopK { value })
    }

    pub fn value(&self) -> u32 { self.0.get() }
}

// ModelId: Non-empty, validated format
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelId(String);

impl ModelId {
    pub fn new(value: String) -> Result<Self, ValidationError> {
        if value.is_empty() {
            return Err(ValidationError::EmptyModelId);
        }
        if value.len() > 256 {
            return Err(ValidationError::ModelIdTooLong {
                length: value.len(),
                max: 256
            });
        }
        // Validate format: alphanumeric, hyphens, underscores, slashes, dots
        if !value.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '/' | '.' | ':')) {
            return Err(ValidationError::InvalidModelIdFormat { value });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

impl FromStr for ModelId {
    type Err = ValidationError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

// RequestId: Non-empty, unique identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RequestId(String);

impl RequestId {
    pub fn new(value: String) -> Result<Self, ValidationError> {
        if value.is_empty() {
            return Err(ValidationError::EmptyRequestId);
        }
        if value.len() > 128 {
            return Err(ValidationError::RequestIdTooLong {
                length: value.len(),
                max: 128
            });
        }
        Ok(Self(value))
    }

    pub fn generate() -> Self {
        Self(format!("req_{}", uuid::Uuid::new_v4()))
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

// ApiKey: Secret, never logged
#[derive(Clone)]
pub struct ApiKey(SecretString);

impl ApiKey {
    pub fn new(value: String) -> Result<Self, ValidationError> {
        if value.is_empty() {
            return Err(ValidationError::EmptyApiKey);
        }
        Ok(Self(SecretString::new(value)))
    }

    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

impl Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey([REDACTED])")
    }
}

// TenantId: Non-empty identifier for multi-tenancy
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantId(String);

impl TenantId {
    pub fn new(value: String) -> Result<Self, ValidationError> {
        if value.is_empty() {
            return Err(ValidationError::EmptyTenantId);
        }
        if !value.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(ValidationError::InvalidTenantIdFormat { value });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

// ProviderId: Validated provider identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProviderId {
    OpenAI,
    Anthropic,
    Google,
    AzureOpenAI,
    Bedrock,
    Ollama,
    VLLM,
    Together,
    Custom(String),
}

impl ProviderId {
    pub fn as_str(&self) -> &str {
        match self {
            Self::OpenAI => "openai",
            Self::Anthropic => "anthropic",
            Self::Google => "google",
            Self::AzureOpenAI => "azure-openai",
            Self::Bedrock => "bedrock",
            Self::Ollama => "ollama",
            Self::VLLM => "vllm",
            Self::Together => "together",
            Self::Custom(s) => s,
        }
    }
}

impl FromStr for ProviderId {
    type Err = ValidationError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "openai" => Self::OpenAI,
            "anthropic" => Self::Anthropic,
            "google" => Self::Google,
            "azure-openai" => Self::AzureOpenAI,
            "bedrock" => Self::Bedrock,
            "ollama" => Self::Ollama,
            "vllm" => Self::VLLM,
            "together" => Self::Together,
            custom => {
                if custom.is_empty() {
                    return Err(ValidationError::EmptyProviderId);
                }
                Self::Custom(custom.to_string())
            }
        })
    }
}

// NonEmptyString: Validated non-empty string
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyString(String);

impl NonEmptyString {
    pub fn new(value: String) -> Result<Self, ValidationError> {
        if value.is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

// NonEmptyVec: Validated non-empty vector
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyVec<T>(Vec<T>);

impl<T> NonEmptyVec<T> {
    pub fn new(vec: Vec<T>) -> Result<Self, ValidationError> {
        if vec.is_empty() {
            return Err(ValidationError::EmptyVec);
        }
        Ok(Self(vec))
    }

    pub fn from_single(item: T) -> Self {
        Self(vec![item])
    }

    pub fn as_slice(&self) -> &[T] { &self.0 }

    pub fn first(&self) -> &T {
        // Safe because we guarantee non-empty
        &self.0[0]
    }

    pub fn len(&self) -> usize { self.0.len() }
}
```

---

## 2. Validation Rules Table

| Field | Type | Constraints | Default | Error Code |
|-------|------|-------------|---------|------------|
| temperature | Temperature | 0.0 ≤ x ≤ 2.0 | 1.0 | `invalid_temperature` |
| max_tokens | MaxTokens | 1 ≤ x ≤ 128000 | None | `invalid_max_tokens` |
| top_p | TopP | 0.0 < x ≤ 1.0 | None | `invalid_top_p` |
| top_k | TopK | x ≥ 1 | None | `invalid_top_k` |
| model | ModelId | Non-empty, ≤256 chars, valid format | Required | `invalid_model_id` |
| request_id | RequestId | Non-empty, ≤128 chars | Auto-generated | `invalid_request_id` |
| api_key | ApiKey | Non-empty | Required | `empty_api_key` |
| tenant_id | TenantId | Non-empty, alphanumeric + `-_` | None | `invalid_tenant_id` |
| messages | NonEmptyVec\<Message\> | At least 1 message | Required | `empty_messages` |
| stop_sequences | Vec\<NonEmptyString\> | Each non-empty | None | `empty_stop_sequence` |
| timeout | Duration | > 0, ≤ 600s | 120s | `invalid_timeout` |
| system_prompt | Option\<NonEmptyString\> | If present, non-empty | None | `empty_system_prompt` |
| provider_id | ProviderId | Valid provider | Required | `invalid_provider_id` |

---

## 3. Compile-Time Guarantees

### 3.1 Builder Pattern with Required Fields

```rust
// Typestate pattern: ensures all required fields are set at compile time
pub struct GatewayRequestBuilder<M, Ms> {
    model: M,
    messages: Ms,
    temperature: Option<Temperature>,
    max_tokens: Option<MaxTokens>,
    top_p: Option<TopP>,
    top_k: Option<TopK>,
    stop_sequences: Vec<NonEmptyString>,
    stream: bool,
    system: Option<NonEmptyString>,
    tools: Option<Vec<Tool>>,
    tool_choice: Option<ToolChoice>,
    metadata: HashMap<String, serde_json::Value>,
    timeout: Option<Duration>,
}

// Marker types for compile-time state tracking
pub struct ModelNotSet;
pub struct ModelSet(ModelId);
pub struct MessagesNotSet;
pub struct MessagesSet(NonEmptyVec<Message>);

impl GatewayRequestBuilder<ModelNotSet, MessagesNotSet> {
    pub fn new() -> Self {
        Self {
            model: ModelNotSet,
            messages: MessagesNotSet,
            temperature: None,
            max_tokens: None,
            top_p: None,
            top_k: None,
            stop_sequences: Vec::new(),
            stream: false,
            system: None,
            tools: None,
            tool_choice: None,
            metadata: HashMap::new(),
            timeout: None,
        }
    }
}

impl<Ms> GatewayRequestBuilder<ModelNotSet, Ms> {
    pub fn model(self, model: ModelId) -> GatewayRequestBuilder<ModelSet, Ms> {
        GatewayRequestBuilder {
            model: ModelSet(model),
            messages: self.messages,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            top_p: self.top_p,
            top_k: self.top_k,
            stop_sequences: self.stop_sequences,
            stream: self.stream,
            system: self.system,
            tools: self.tools,
            tool_choice: self.tool_choice,
            metadata: self.metadata,
            timeout: self.timeout,
        }
    }
}

impl<M> GatewayRequestBuilder<M, MessagesNotSet> {
    pub fn messages(self, messages: NonEmptyVec<Message>)
        -> GatewayRequestBuilder<M, MessagesSet>
    {
        GatewayRequestBuilder {
            model: self.model,
            messages: MessagesSet(messages),
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            top_p: self.top_p,
            top_k: self.top_k,
            stop_sequences: self.stop_sequences,
            stream: self.stream,
            system: self.system,
            tools: self.tools,
            tool_choice: self.tool_choice,
            metadata: self.metadata,
            timeout: self.timeout,
        }
    }
}

// Only allow build() when all required fields are set
impl GatewayRequestBuilder<ModelSet, MessagesSet> {
    pub fn temperature(mut self, temp: Temperature) -> Self {
        self.temperature = Some(temp);
        self
    }

    pub fn max_tokens(mut self, tokens: MaxTokens) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    pub fn top_p(mut self, top_p: TopP) -> Self {
        self.top_p = Some(top_p);
        self
    }

    pub fn top_k(mut self, top_k: TopK) -> Self {
        self.top_k = Some(top_k);
        self
    }

    pub fn system(mut self, system: NonEmptyString) -> Self {
        self.system = Some(system);
        self
    }

    pub fn stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    pub fn build(self) -> ValidatedRequest {
        ValidatedRequest {
            request_id: RequestId::generate(),
            model: self.model.0,
            messages: self.messages.0,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            top_p: self.top_p,
            top_k: self.top_k,
            stop_sequences: self.stop_sequences,
            stream: self.stream,
            system: self.system,
            tools: self.tools,
            tool_choice: self.tool_choice,
            metadata: self.metadata,
            timeout: self.timeout.unwrap_or(Duration::from_secs(120)),
        }
    }
}
```

### 3.2 Phantom Types for Provider-Specific Requests

```rust
// Marker types for provider-specific constraints
pub struct OpenAIRequest;
pub struct AnthropicRequest;
pub struct GoogleRequest;

pub struct ProviderRequest<P> {
    base: ValidatedRequest,
    _provider: PhantomData<P>,
}

impl ProviderRequest<OpenAIRequest> {
    pub fn from_validated(req: ValidatedRequest) -> Result<Self, ValidationError> {
        // OpenAI-specific validation
        if let Some(tools) = &req.tools {
            if tools.len() > 128 {
                return Err(ValidationError::TooManyTools {
                    count: tools.len(),
                    max: 128
                });
            }
        }
        Ok(Self {
            base: req,
            _provider: PhantomData,
        })
    }
}

impl ProviderRequest<AnthropicRequest> {
    pub fn from_validated(req: ValidatedRequest) -> Result<Self, ValidationError> {
        // Anthropic requires max_tokens
        if req.max_tokens.is_none() {
            return Err(ValidationError::MissingMaxTokens);
        }

        // Anthropic doesn't support image URLs (only base64)
        for msg in req.messages.as_slice() {
            if let MessageContent::MultiModal(parts) = &msg.content {
                for part in parts {
                    if let ContentPart::Image { source: ImageSource::Url { .. }, .. } = part {
                        return Err(ValidationError::ImageUrlNotSupported);
                    }
                }
            }
        }

        Ok(Self {
            base: req,
            _provider: PhantomData,
        })
    }
}
```

### 3.3 State Machine Types (Circuit Breaker)

```rust
// Typestate pattern for circuit breaker
pub struct Closed;
pub struct Open;
pub struct HalfOpen;

pub struct CircuitBreaker<S> {
    failure_count: u32,
    success_count: u32,
    last_failure: Option<Instant>,
    config: CircuitBreakerConfig,
    _state: PhantomData<S>,
}

impl CircuitBreaker<Closed> {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            failure_count: 0,
            success_count: 0,
            last_failure: None,
            config,
            _state: PhantomData,
        }
    }

    pub fn record_failure(mut self) -> Either<Self, CircuitBreaker<Open>> {
        self.failure_count += 1;
        if self.failure_count >= self.config.failure_threshold {
            Either::Right(CircuitBreaker {
                failure_count: 0,
                success_count: 0,
                last_failure: Some(Instant::now()),
                config: self.config,
                _state: PhantomData,
            })
        } else {
            Either::Left(self)
        }
    }

    pub fn record_success(mut self) -> Self {
        self.failure_count = 0;
        self
    }
}

impl CircuitBreaker<Open> {
    pub fn try_half_open(self) -> Result<CircuitBreaker<HalfOpen>, Self> {
        let elapsed = self.last_failure
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO);

        if elapsed >= self.config.timeout {
            Ok(CircuitBreaker {
                failure_count: 0,
                success_count: 0,
                last_failure: None,
                config: self.config,
                _state: PhantomData,
            })
        } else {
            Err(self)
        }
    }
}

impl CircuitBreaker<HalfOpen> {
    pub fn record_success(mut self) -> Either<Self, CircuitBreaker<Closed>> {
        self.success_count += 1;
        if self.success_count >= self.config.success_threshold {
            Either::Right(CircuitBreaker {
                failure_count: 0,
                success_count: 0,
                last_failure: None,
                config: self.config,
                _state: PhantomData,
            })
        } else {
            Either::Left(self)
        }
    }

    pub fn record_failure(self) -> CircuitBreaker<Open> {
        CircuitBreaker {
            failure_count: 0,
            success_count: 0,
            last_failure: Some(Instant::now()),
            config: self.config,
            _state: PhantomData,
        }
    }
}
```

---

## 4. Runtime Validation Pipeline

```rust
pub struct ValidatedRequest {
    pub request_id: RequestId,
    pub model: ModelId,
    pub messages: NonEmptyVec<Message>,
    pub temperature: Option<Temperature>,
    pub max_tokens: Option<MaxTokens>,
    pub top_p: Option<TopP>,
    pub top_k: Option<TopK>,
    pub stop_sequences: Vec<NonEmptyString>,
    pub stream: bool,
    pub system: Option<NonEmptyString>,
    pub tools: Option<Vec<Tool>>,
    pub tool_choice: Option<ToolChoice>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub timeout: Duration,
}

impl ValidatedRequest {
    /// Validate cross-field constraints
    pub fn validate_constraints(&self) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();

        // Temperature and top_p should not both be set (provider-specific)
        if self.temperature.is_some() && self.top_p.is_some() {
            errors.push(ValidationError::ConflictingParameters {
                params: vec!["temperature".to_string(), "top_p".to_string()],
                reason: "Most providers recommend setting only one".to_string(),
            });
        }

        // Tool choice requires tools
        if self.tool_choice.is_some() && self.tools.is_none() {
            errors.push(ValidationError::MissingDependency {
                field: "tool_choice".to_string(),
                requires: "tools".to_string(),
            });
        }

        // Timeout bounds
        if self.timeout > Duration::from_secs(600) {
            errors.push(ValidationError::TimeoutTooLarge {
                value: self.timeout,
                max: Duration::from_secs(600),
            });
        }

        // Message alternation check (user/assistant pattern)
        if !self.validate_message_alternation() {
            errors.push(ValidationError::InvalidMessageSequence);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationErrors { errors })
        }
    }

    fn validate_message_alternation(&self) -> bool {
        let messages = self.messages.as_slice();

        // First message should be user or system
        if let Some(first) = messages.first() {
            if !matches!(first.role, MessageRole::User | MessageRole::System) {
                return false;
            }
        }

        // Check for assistant messages not preceded by user messages
        let mut last_was_user = false;
        for msg in messages {
            match msg.role {
                MessageRole::User => last_was_user = true,
                MessageRole::Assistant => {
                    if !last_was_user {
                        return false;
                    }
                    last_was_user = false;
                }
                _ => {}
            }
        }

        true
    }

    /// Validate against provider capabilities
    pub fn validate_for_provider(
        &self,
        capabilities: &ProviderCapabilities,
    ) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();

        // Check streaming support
        if self.stream && !capabilities.supports_streaming {
            errors.push(ValidationError::UnsupportedCapability {
                capability: "streaming".to_string(),
            });
        }

        // Check tools support
        if self.tools.is_some() && !capabilities.supports_tools {
            errors.push(ValidationError::UnsupportedCapability {
                capability: "tools".to_string(),
            });
        }

        // Check multimodal support
        if self.has_multimodal_content() && !capabilities.supports_multimodal {
            errors.push(ValidationError::UnsupportedCapability {
                capability: "multimodal".to_string(),
            });
        }

        // Check model availability
        if !capabilities.models.contains(&self.model.as_str().to_string()) {
            errors.push(ValidationError::UnsupportedModel {
                model: self.model.as_str().to_string(),
                available: capabilities.models.clone(),
            });
        }

        // Check max tokens
        if let Some(max_tokens) = self.max_tokens {
            if max_tokens.value() > capabilities.max_output_tokens {
                errors.push(ValidationError::MaxTokensExceedsLimit {
                    requested: max_tokens.value(),
                    max: capabilities.max_output_tokens,
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationErrors { errors })
        }
    }

    fn has_multimodal_content(&self) -> bool {
        self.messages.as_slice().iter().any(|msg| {
            matches!(msg.content, MessageContent::MultiModal(_))
        })
    }

    /// Estimate token count for rate limiting
    pub fn estimate_tokens(&self) -> u32 {
        let mut tokens = 0u32;

        // System prompt
        if let Some(system) = &self.system {
            tokens += (system.as_str().len() / 4) as u32;
        }

        // Messages
        for msg in self.messages.as_slice() {
            tokens += match &msg.content {
                MessageContent::Text(text) => (text.len() / 4) as u32,
                MessageContent::MultiModal(parts) => {
                    parts.iter().map(|p| match p {
                        ContentPart::Text { text } => (text.len() / 4) as u32,
                        ContentPart::Image { .. } => 765, // Approx for images
                    }).sum()
                }
            };
        }

        // Add overhead for formatting
        tokens + 10
    }
}
```

---

## 5. Error Type Hierarchy

```rust
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    // Temperature errors
    #[error("Invalid temperature: {value}. Must be between {min} and {max}")]
    InvalidTemperature { value: f32, min: f32, max: f32 },

    // Token errors
    #[error("Invalid max_tokens: {value}. Must be between {min} and {max}")]
    InvalidMaxTokens { value: u32, min: u32, max: u32 },

    #[error("Max tokens {requested} exceeds provider limit of {max}")]
    MaxTokensExceedsLimit { requested: u32, max: u32 },

    // TopP errors
    #[error("Invalid top_p: {value}. Must be between {min_exclusive} (exclusive) and {max_inclusive} (inclusive)")]
    InvalidTopP { value: f32, min_exclusive: f32, max_inclusive: f32 },

    // TopK errors
    #[error("Invalid top_k: {value}. Must be at least 1")]
    InvalidTopK { value: u32 },

    // Model ID errors
    #[error("Model ID cannot be empty")]
    EmptyModelId,

    #[error("Model ID too long: {length} characters (max {max})")]
    ModelIdTooLong { length: usize, max: usize },

    #[error("Invalid model ID format: {value}")]
    InvalidModelIdFormat { value: String },

    #[error("Unsupported model: {model}. Available: {available:?}")]
    UnsupportedModel { model: String, available: Vec<String> },

    // Request ID errors
    #[error("Request ID cannot be empty")]
    EmptyRequestId,

    #[error("Request ID too long: {length} characters (max {max})")]
    RequestIdTooLong { length: usize, max: usize },

    // API Key errors
    #[error("API key cannot be empty")]
    EmptyApiKey,

    // Tenant ID errors
    #[error("Tenant ID cannot be empty")]
    EmptyTenantId,

    #[error("Invalid tenant ID format: {value}")]
    InvalidTenantIdFormat { value: String },

    // Provider ID errors
    #[error("Provider ID cannot be empty")]
    EmptyProviderId,

    // String/Vec errors
    #[error("String cannot be empty")]
    EmptyString,

    #[error("Vector cannot be empty")]
    EmptyVec,

    // Message errors
    #[error("Messages cannot be empty")]
    EmptyMessages,

    #[error("Invalid message sequence: messages must alternate between user and assistant")]
    InvalidMessageSequence,

    // Parameter conflicts
    #[error("Conflicting parameters: {params:?}. Reason: {reason}")]
    ConflictingParameters { params: Vec<String>, reason: String },

    #[error("Missing required field: {field} (required when {requires} is set)")]
    MissingDependency { field: String, requires: String },

    #[error("Missing required field: max_tokens")]
    MissingMaxTokens,

    // Timeout errors
    #[error("Timeout {value:?} exceeds maximum of {max:?}")]
    TimeoutTooLarge { value: Duration, max: Duration },

    // Capability errors
    #[error("Provider does not support capability: {capability}")]
    UnsupportedCapability { capability: String },

    #[error("Image URLs not supported, use base64 encoding")]
    ImageUrlNotSupported,

    #[error("Too many tools: {count} (max {max})")]
    TooManyTools { count: usize, max: usize },
}

#[derive(Debug, thiserror::Error)]
#[error("Validation failed with {0} errors")]
pub struct ValidationErrors {
    pub errors: Vec<ValidationError>,
}

impl ValidationErrors {
    pub fn error_codes(&self) -> Vec<&'static str> {
        self.errors.iter().map(|e| e.error_code()).collect()
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "errors": self.errors.iter().map(|e| {
                serde_json::json!({
                    "code": e.error_code(),
                    "message": e.to_string(),
                })
            }).collect::<Vec<_>>()
        })
    }
}

impl ValidationError {
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::InvalidTemperature { .. } => "invalid_temperature",
            Self::InvalidMaxTokens { .. } => "invalid_max_tokens",
            Self::MaxTokensExceedsLimit { .. } => "max_tokens_exceeds_limit",
            Self::InvalidTopP { .. } => "invalid_top_p",
            Self::InvalidTopK { .. } => "invalid_top_k",
            Self::EmptyModelId => "empty_model_id",
            Self::ModelIdTooLong { .. } => "model_id_too_long",
            Self::InvalidModelIdFormat { .. } => "invalid_model_id_format",
            Self::UnsupportedModel { .. } => "unsupported_model",
            Self::EmptyRequestId => "empty_request_id",
            Self::RequestIdTooLong { .. } => "request_id_too_long",
            Self::EmptyApiKey => "empty_api_key",
            Self::EmptyTenantId => "empty_tenant_id",
            Self::InvalidTenantIdFormat { .. } => "invalid_tenant_id_format",
            Self::EmptyProviderId => "empty_provider_id",
            Self::EmptyString => "empty_string",
            Self::EmptyVec => "empty_vec",
            Self::EmptyMessages => "empty_messages",
            Self::InvalidMessageSequence => "invalid_message_sequence",
            Self::ConflictingParameters { .. } => "conflicting_parameters",
            Self::MissingDependency { .. } => "missing_dependency",
            Self::MissingMaxTokens => "missing_max_tokens",
            Self::TimeoutTooLarge { .. } => "timeout_too_large",
            Self::UnsupportedCapability { .. } => "unsupported_capability",
            Self::ImageUrlNotSupported => "image_url_not_supported",
            Self::TooManyTools { .. } => "too_many_tools",
        }
    }

    pub fn http_status(&self) -> http::StatusCode {
        match self {
            Self::UnsupportedModel { .. } |
            Self::UnsupportedCapability { .. } |
            Self::ImageUrlNotSupported => http::StatusCode::NOT_IMPLEMENTED,

            Self::EmptyApiKey => http::StatusCode::UNAUTHORIZED,

            _ => http::StatusCode::BAD_REQUEST,
        }
    }

    pub fn is_retryable(&self) -> bool {
        // None of the validation errors are retryable
        false
    }
}
```

---

## 6. Serialization Safety

```rust
// Custom Serialize/Deserialize with validation
impl Serialize for Temperature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_f32(self.0)
    }
}

impl<'de> Deserialize<'de> for Temperature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = f32::deserialize(deserializer)?;
        Temperature::new(value).map_err(serde::de::Error::custom)
    }
}

// Similar for all validated types...

// Never serialize ApiKey
impl Serialize for ApiKey {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom("Cannot serialize ApiKey"))
    }
}
```

---

## 7. Usage Example

```rust
// Type-safe request construction
let request = GatewayRequestBuilder::new()
    .model(ModelId::new("gpt-4".to_string())?)
    .messages(NonEmptyVec::from_single(Message {
        role: MessageRole::User,
        content: MessageContent::Text("Hello".to_string()),
        name: None,
    }))
    .temperature(Temperature::new(0.7)?)
    .max_tokens(MaxTokens::new(1000)?)
    .stream(false)
    .build();  // Returns ValidatedRequest

// Cross-field validation
request.validate_constraints()?;

// Provider-specific validation
request.validate_for_provider(&provider.capabilities())?;

// Convert to provider-specific request
let openai_request = ProviderRequest::<OpenAIRequest>::from_validated(request)?;
```

This architecture ensures **zero runtime validation errors** for well-formed requests through:
1. Compile-time type checking
2. Newtype wrappers with validated constructors
3. Typestate pattern for required fields
4. Phantom types for provider-specific constraints
5. Comprehensive error types with HTTP status mapping
