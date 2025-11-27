# LLM-Inference-Gateway API Design and Versioning Documentation

> **Version**: 1.0.0
> **Status**: Production Ready
> **Last Updated**: 2025-11-27
> **OpenAPI Version**: 3.0.3

---

## Table of Contents

1. [API Design Principles](#1-api-design-principles)
2. [API Endpoints Specification](#2-api-endpoints-specification)
3. [Request/Response Formats](#3-requestresponse-formats)
4. [API Versioning Strategy](#4-api-versioning-strategy)
5. [Rate Limiting Design](#5-rate-limiting-design)
6. [Authentication Headers](#6-authentication-headers)
7. [Pagination & Filtering](#7-pagination--filtering)
8. [OpenAPI Specification](#8-openapi-specification)
9. [SDK Design Guidelines](#9-sdk-design-guidelines)
10. [API Documentation](#10-api-documentation)

---

## 1. API Design Principles

### 1.1 RESTful Conventions

The LLM-Inference-Gateway strictly adheres to REST architectural principles:

**Resource-Oriented Design**
- Resources are identified by URIs (e.g., `/v1/models`, `/v1/chat/completions`)
- Standard HTTP methods map to CRUD operations:
  - `GET` - Retrieve resources (idempotent, cacheable)
  - `POST` - Create resources or execute actions
  - `PUT` - Update entire resources (idempotent)
  - `PATCH` - Partial resource updates (idempotent)
  - `DELETE` - Remove resources (idempotent)

**HTTP Status Codes**
```
2xx - Success
  200 OK              - Successful GET, PUT, PATCH, DELETE
  201 Created         - Successful POST creating new resource
  202 Accepted        - Request accepted for async processing
  204 No Content      - Successful DELETE with no response body

3xx - Redirection
  301 Moved Permanently  - Resource permanently relocated
  304 Not Modified       - Cached resource still valid

4xx - Client Errors
  400 Bad Request        - Invalid request syntax or parameters
  401 Unauthorized       - Missing or invalid authentication
  403 Forbidden          - Valid auth but insufficient permissions
  404 Not Found          - Resource does not exist
  409 Conflict           - Request conflicts with current state
  422 Unprocessable      - Validation failed
  429 Too Many Requests  - Rate limit exceeded

5xx - Server Errors
  500 Internal Server Error  - Unexpected server error
  502 Bad Gateway           - Invalid upstream response
  503 Service Unavailable   - Temporary overload or maintenance
  504 Gateway Timeout       - Upstream timeout
```

**Statelessness**
- Each request contains all information needed for processing
- No server-side session state
- Authentication via tokens/headers per request
- Enables horizontal scaling without sticky sessions

**Cacheability**
- `Cache-Control` headers indicate cacheability
- `ETag` headers for conditional requests
- `Last-Modified` for time-based validation
- Streaming responses are not cached

**Uniform Interface**
- Consistent URI patterns: `/v{version}/{resource}`
- Standard error response format across all endpoints
- HATEOAS links in responses where applicable

### 1.2 OpenAI API Compatibility

The gateway maintains **100% compatibility** with OpenAI API v1 specification:

**Endpoint Parity**
```
OpenAI API                    Gateway API                  Compatibility
-----------                   -----------                  -------------
POST /v1/chat/completions  →  POST /v1/chat/completions   ✓ Identical
GET  /v1/models            →  GET  /v1/models             ✓ Identical
GET  /v1/models/{id}       →  GET  /v1/models/{id}        ✓ Identical
```

**Request Schema Compatibility**
- All OpenAI request fields supported (required + optional)
- Additional gateway-specific fields are opt-in extensions
- Unknown fields are ignored (forward compatibility)

**Response Schema Compatibility**
- Standard OpenAI response structure maintained
- Additional metadata in `x-gateway-*` extension fields
- Streaming format follows Server-Sent Events (SSE) spec

**Drop-in Replacement**
```python
# OpenAI SDK works without modification
import openai

# Just change the base URL
openai.api_base = "https://gateway.example.com/v1"
openai.api_key = "gateway-api-key"

# All existing code works unchanged
response = openai.ChatCompletion.create(
    model="gpt-4",
    messages=[{"role": "user", "content": "Hello"}]
)
```

**Error Format Alignment**
```json
{
  "error": {
    "type": "invalid_request_error",
    "message": "Missing required field: messages",
    "code": "missing_field",
    "param": "messages"
  }
}
```

### 1.3 Consistent Error Responses

All errors follow a **standardized error envelope**:

**Error Structure**
```json
{
  "error": {
    "type": "string",           // Error category
    "message": "string",         // Human-readable description
    "code": "string",            // Machine-readable error code
    "param": "string | null",    // Problematic parameter (if applicable)
    "request_id": "string"       // Correlation ID for debugging
  }
}
```

**Error Types**
```typescript
type ErrorType =
  | "invalid_request_error"     // Malformed request
  | "authentication_error"      // Invalid/missing credentials
  | "permission_error"          // Insufficient permissions
  | "not_found_error"           // Resource does not exist
  | "rate_limit_error"          // Too many requests
  | "provider_error"            // Upstream provider failure
  | "internal_error"            // Gateway internal error
  | "timeout_error"             // Request timeout
  | "conflict_error";           // Resource conflict
```

**Error Code Examples**
```json
// 400 Bad Request - Invalid Parameter
{
  "error": {
    "type": "invalid_request_error",
    "message": "Invalid value for 'temperature': must be between 0 and 2",
    "code": "invalid_parameter_value",
    "param": "temperature",
    "request_id": "req_abc123def456"
  }
}

// 401 Unauthorized - Missing API Key
{
  "error": {
    "type": "authentication_error",
    "message": "Missing Authorization header",
    "code": "missing_authorization",
    "param": null,
    "request_id": "req_xyz789ghi012"
  }
}

// 429 Too Many Requests - Rate Limit
{
  "error": {
    "type": "rate_limit_error",
    "message": "Rate limit exceeded: 100 requests per minute",
    "code": "rate_limit_exceeded",
    "param": null,
    "request_id": "req_lmn345opq678"
  }
}

// 503 Service Unavailable - Circuit Breaker Open
{
  "error": {
    "type": "provider_error",
    "message": "All providers unavailable due to circuit breaker",
    "code": "circuit_breaker_open",
    "param": null,
    "request_id": "req_rst901uvw234"
  }
}
```

**Error Response Headers**
```http
HTTP/1.1 429 Too Many Requests
Content-Type: application/json
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1699564800
Retry-After: 60
X-Request-ID: req_lmn345opq678
```

### 1.4 Idempotency Support

The gateway implements **idempotency** for safe request retries:

**Idempotency Keys**
```http
POST /v1/chat/completions
Content-Type: application/json
Idempotency-Key: 550e8400-e29b-41d4-a716-446655440000

{
  "model": "gpt-4",
  "messages": [...]
}
```

**Idempotency Behavior**
```
First Request (Key: abc123)
  → Process request
  → Cache response for 24 hours
  → Return 200 OK with response

Duplicate Request (Key: abc123, within 24h)
  → Detect duplicate via key
  → Return cached response
  → Return 200 OK (not 201)
  → Add header: Idempotent-Replayed: true
```

**Idempotent Methods**
- `GET`, `HEAD`, `PUT`, `DELETE`, `OPTIONS` - Inherently idempotent
- `POST` - Idempotent only with `Idempotency-Key` header
- `PATCH` - Idempotent only with `Idempotency-Key` header

**Key Requirements**
- Must be UUID v4 or similar unique identifier
- Valid for 24 hours after first use
- Case-sensitive
- Max length: 255 characters

**Response Headers**
```http
HTTP/1.1 200 OK
Idempotent-Replayed: true
Idempotency-Key: 550e8400-e29b-41d4-a716-446655440000
```

---

## 2. API Endpoints Specification

### 2.1 Chat Completions (OpenAI Compatible)

**Endpoint**: `POST /v1/chat/completions`

**Description**: Generate chat completions using configured LLM providers with automatic routing, failover, and streaming support.

#### Request Schema

```json
{
  "model": "string",                    // Required: Model identifier
  "messages": [                         // Required: Conversation messages
    {
      "role": "system | user | assistant | function",
      "content": "string | array",      // Text or multimodal content
      "name": "string"                  // Optional: Speaker name
    }
  ],
  "temperature": 0.0-2.0,               // Optional: Sampling temperature (default: 1.0)
  "top_p": 0.0-1.0,                     // Optional: Nucleus sampling (default: 1.0)
  "n": 1-10,                            // Optional: Number of completions (default: 1)
  "stream": true | false,               // Optional: Enable SSE streaming (default: false)
  "stop": "string | array",             // Optional: Stop sequences
  "max_tokens": 1-128000,               // Optional: Maximum tokens to generate
  "presence_penalty": -2.0-2.0,         // Optional: Presence penalty (default: 0)
  "frequency_penalty": -2.0-2.0,        // Optional: Frequency penalty (default: 0)
  "logit_bias": { "token_id": -100-100 }, // Optional: Token likelihood bias
  "user": "string",                     // Optional: End-user identifier

  // Gateway-Specific Extensions (Optional)
  "x_gateway_routing": {
    "strategy": "lowest-latency | lowest-cost | round-robin | weighted",
    "provider_preference": ["openai", "anthropic", "google"],
    "fallback_enabled": true,
    "max_retries": 3
  },
  "x_gateway_metadata": {
    "tenant_id": "string",
    "project_id": "string",
    "cost_center": "string"
  }
}
```

#### Response Schema (Non-Streaming)

```json
{
  "id": "chatcmpl-abc123",              // Unique completion ID
  "object": "chat.completion",          // Object type
  "created": 1699564800,                // Unix timestamp
  "model": "gpt-4",                     // Model used (may differ from request)
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Generated response text"
      },
      "finish_reason": "stop | length | function_call | content_filter"
    }
  ],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 20,
    "total_tokens": 30
  },

  // Gateway-Specific Extensions
  "x_gateway_metadata": {
    "provider": "openai",
    "latency_ms": 342,
    "retries": 0,
    "cached": false
  }
}
```

#### Response Schema (Streaming)

**Content-Type**: `text/event-stream`

```
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1699564800,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1699564800,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1699564800,"model":"gpt-4","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1699564800,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
```

#### Error Responses

```json
// 400 Bad Request - Invalid Messages Format
{
  "error": {
    "type": "invalid_request_error",
    "message": "Messages must be a non-empty array",
    "code": "invalid_messages",
    "param": "messages",
    "request_id": "req_xyz789"
  }
}

// 404 Not Found - Unknown Model
{
  "error": {
    "type": "not_found_error",
    "message": "Model 'unknown-model' not found",
    "code": "model_not_found",
    "param": "model",
    "request_id": "req_abc123"
  }
}

// 503 Service Unavailable - All Providers Down
{
  "error": {
    "type": "provider_error",
    "message": "All configured providers are unavailable",
    "code": "providers_unavailable",
    "param": null,
    "request_id": "req_def456"
  }
}
```

#### cURL Example

```bash
curl -X POST https://gateway.example.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-gateway-abc123" \
  -d '{
    "model": "gpt-4",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "What is the capital of France?"}
    ],
    "temperature": 0.7,
    "max_tokens": 150
  }'
```

#### Streaming Example

```bash
curl -X POST https://gateway.example.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-gateway-abc123" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Count to 5"}],
    "stream": true
  }'
```

---

### 2.2 Models

#### 2.2.1 List Models

**Endpoint**: `GET /v1/models`

**Description**: Retrieve list of available models across all configured providers.

**Request Parameters**: None

**Response Schema**:
```json
{
  "object": "list",
  "data": [
    {
      "id": "gpt-4",
      "object": "model",
      "created": 1699564800,
      "owned_by": "openai",
      "permission": [],
      "root": "gpt-4",
      "parent": null,

      // Gateway Extensions
      "x_gateway_info": {
        "providers": ["openai", "azure-openai"],
        "capabilities": ["chat", "function_calling"],
        "context_window": 8192,
        "pricing": {
          "input_per_million": 30.0,
          "output_per_million": 60.0
        }
      }
    },
    {
      "id": "claude-3-opus",
      "object": "model",
      "created": 1699564800,
      "owned_by": "anthropic",
      "x_gateway_info": {
        "providers": ["anthropic"],
        "capabilities": ["chat", "vision"],
        "context_window": 200000,
        "pricing": {
          "input_per_million": 15.0,
          "output_per_million": 75.0
        }
      }
    }
  ]
}
```

**cURL Example**:
```bash
curl https://gateway.example.com/v1/models \
  -H "Authorization: Bearer sk-gateway-abc123"
```

#### 2.2.2 Retrieve Model

**Endpoint**: `GET /v1/models/{model_id}`

**Description**: Get detailed information about a specific model.

**Path Parameters**:
- `model_id` (string, required): Model identifier (e.g., "gpt-4", "claude-3-opus")

**Response Schema**:
```json
{
  "id": "gpt-4",
  "object": "model",
  "created": 1699564800,
  "owned_by": "openai",
  "x_gateway_info": {
    "providers": ["openai", "azure-openai"],
    "capabilities": ["chat", "function_calling"],
    "context_window": 8192,
    "max_output_tokens": 4096,
    "pricing": {
      "input_per_million": 30.0,
      "output_per_million": 60.0,
      "currency": "USD"
    },
    "performance": {
      "avg_latency_ms": 850,
      "p95_latency_ms": 1200,
      "success_rate": 0.998
    }
  }
}
```

**Error Response (404)**:
```json
{
  "error": {
    "type": "not_found_error",
    "message": "Model 'unknown-model' not found",
    "code": "model_not_found",
    "param": "model_id",
    "request_id": "req_abc123"
  }
}
```

**cURL Example**:
```bash
curl https://gateway.example.com/v1/models/gpt-4 \
  -H "Authorization: Bearer sk-gateway-abc123"
```

---

### 2.3 Health & Operations

#### 2.3.1 Liveness Probe

**Endpoint**: `GET /health/live`

**Description**: Kubernetes liveness probe - checks if the gateway process is running.

**Response (200 OK)**:
```json
{
  "status": "alive",
  "timestamp": "2025-11-27T10:30:00Z"
}
```

**Response (503 Service Unavailable)**:
```json
{
  "status": "dead",
  "timestamp": "2025-11-27T10:30:00Z",
  "reason": "Critical subsystem failure"
}
```

#### 2.3.2 Readiness Probe

**Endpoint**: `GET /health/ready`

**Description**: Kubernetes readiness probe - checks if gateway can accept traffic.

**Response (200 OK)**:
```json
{
  "status": "ready",
  "timestamp": "2025-11-27T10:30:00Z",
  "checks": {
    "provider_registry": "healthy",
    "routing_engine": "healthy",
    "telemetry": "healthy"
  }
}
```

**Response (503 Service Unavailable)**:
```json
{
  "status": "not_ready",
  "timestamp": "2025-11-27T10:30:00Z",
  "checks": {
    "provider_registry": "degraded",
    "routing_engine": "healthy",
    "telemetry": "healthy"
  },
  "reason": "Insufficient healthy providers"
}
```

#### 2.3.3 Provider Health

**Endpoint**: `GET /health/providers`

**Description**: Detailed health status for all configured providers.

**Response (200 OK)**:
```json
{
  "timestamp": "2025-11-27T10:30:00Z",
  "providers": [
    {
      "id": "openai-us-east",
      "name": "OpenAI US East",
      "type": "openai",
      "status": "healthy",
      "circuit_breaker": "closed",
      "metrics": {
        "success_rate": 0.998,
        "avg_latency_ms": 420,
        "requests_per_minute": 1250,
        "error_rate": 0.002
      },
      "last_check": "2025-11-27T10:29:55Z"
    },
    {
      "id": "anthropic-primary",
      "name": "Anthropic Primary",
      "type": "anthropic",
      "status": "degraded",
      "circuit_breaker": "half_open",
      "metrics": {
        "success_rate": 0.945,
        "avg_latency_ms": 1850,
        "requests_per_minute": 340,
        "error_rate": 0.055
      },
      "last_check": "2025-11-27T10:29:58Z",
      "warning": "Elevated error rate detected"
    },
    {
      "id": "google-vertex",
      "name": "Google Vertex AI",
      "type": "google",
      "status": "unhealthy",
      "circuit_breaker": "open",
      "metrics": {
        "success_rate": 0.0,
        "avg_latency_ms": 0,
        "requests_per_minute": 0,
        "error_rate": 1.0
      },
      "last_check": "2025-11-27T10:28:30Z",
      "error": "Connection timeout - circuit breaker open"
    }
  ]
}
```

#### 2.3.4 Metrics

**Endpoint**: `GET /metrics`

**Description**: Prometheus-compatible metrics endpoint.

**Response (200 OK, text/plain)**:
```prometheus
# HELP gateway_requests_total Total number of requests processed
# TYPE gateway_requests_total counter
gateway_requests_total{method="POST",endpoint="/v1/chat/completions",status="200"} 152847

# HELP gateway_request_duration_seconds Request duration in seconds
# TYPE gateway_request_duration_seconds histogram
gateway_request_duration_seconds_bucket{le="0.005"} 42150
gateway_request_duration_seconds_bucket{le="0.01"} 98234
gateway_request_duration_seconds_bucket{le="0.025"} 145678
gateway_request_duration_seconds_bucket{le="0.05"} 150123
gateway_request_duration_seconds_bucket{le="0.1"} 152340
gateway_request_duration_seconds_sum 3456.78
gateway_request_duration_seconds_count 152847

# HELP gateway_provider_requests_total Requests sent to each provider
# TYPE gateway_provider_requests_total counter
gateway_provider_requests_total{provider="openai",status="success"} 98234
gateway_provider_requests_total{provider="anthropic",status="success"} 45678
gateway_provider_requests_total{provider="openai",status="error"} 234

# HELP gateway_circuit_breaker_state Circuit breaker state (0=closed, 1=half_open, 2=open)
# TYPE gateway_circuit_breaker_state gauge
gateway_circuit_breaker_state{provider="openai"} 0
gateway_circuit_breaker_state{provider="anthropic"} 1
gateway_circuit_breaker_state{provider="google"} 2

# HELP gateway_active_connections Current number of active connections
# TYPE gateway_active_connections gauge
gateway_active_connections 247
```

---

### 2.4 Admin Endpoints

**Authentication**: Requires admin-level API key or mTLS certificate

#### 2.4.1 Get Configuration

**Endpoint**: `GET /admin/config`

**Description**: Retrieve current gateway configuration (sanitized).

**Response (200 OK)**:
```json
{
  "version": "1.0.0",
  "server": {
    "bind_address": "0.0.0.0:8080",
    "tls_enabled": true,
    "max_connections": 10000
  },
  "routing": {
    "default_strategy": "lowest-latency",
    "fallback_enabled": true,
    "max_retries": 3
  },
  "rate_limiting": {
    "enabled": true,
    "default_limit": 100,
    "default_window": "1m"
  },
  "providers": [
    {
      "id": "openai-us-east",
      "type": "openai",
      "enabled": true
      // Secrets redacted
    }
  ]
}
```

#### 2.4.2 Reload Configuration

**Endpoint**: `POST /admin/config/reload`

**Description**: Hot-reload configuration without restarting the gateway.

**Request Body**: None (reads from config file)

**Response (200 OK)**:
```json
{
  "status": "success",
  "message": "Configuration reloaded successfully",
  "timestamp": "2025-11-27T10:30:00Z",
  "changes": {
    "providers_added": 1,
    "providers_removed": 0,
    "providers_updated": 2,
    "routing_rules_updated": true
  }
}
```

**Response (400 Bad Request - Invalid Config)**:
```json
{
  "status": "error",
  "message": "Configuration validation failed",
  "errors": [
    {
      "field": "providers[2].api_key",
      "message": "Missing required field: api_key"
    }
  ]
}
```

#### 2.4.3 List Providers

**Endpoint**: `GET /admin/providers`

**Description**: Get detailed provider configuration and status.

**Response (200 OK)**:
```json
{
  "providers": [
    {
      "id": "openai-us-east",
      "name": "OpenAI US East",
      "type": "openai",
      "enabled": true,
      "config": {
        "base_url": "https://api.openai.com/v1",
        "timeout_ms": 30000,
        "max_retries": 3
      },
      "health": {
        "status": "healthy",
        "last_check": "2025-11-27T10:29:55Z"
      },
      "metrics": {
        "requests_total": 152847,
        "errors_total": 234,
        "avg_latency_ms": 420
      }
    }
  ]
}
```

#### 2.4.4 Add Provider

**Endpoint**: `POST /admin/providers`

**Description**: Dynamically register a new provider.

**Request Body**:
```json
{
  "id": "anthropic-backup",
  "name": "Anthropic Backup",
  "type": "anthropic",
  "enabled": true,
  "config": {
    "api_key": "sk-ant-...",
    "base_url": "https://api.anthropic.com/v1",
    "timeout_ms": 30000,
    "max_retries": 3
  }
}
```

**Response (201 Created)**:
```json
{
  "status": "success",
  "message": "Provider registered successfully",
  "provider_id": "anthropic-backup"
}
```

#### 2.4.5 Delete Provider

**Endpoint**: `DELETE /admin/providers/{provider_id}`

**Description**: Remove a provider from the registry.

**Path Parameters**:
- `provider_id` (string, required): Provider identifier

**Response (200 OK)**:
```json
{
  "status": "success",
  "message": "Provider removed successfully",
  "provider_id": "anthropic-backup"
}
```

**Response (409 Conflict - Provider In Use)**:
```json
{
  "error": {
    "type": "conflict_error",
    "message": "Cannot remove provider: currently serving active requests",
    "code": "provider_in_use",
    "param": "provider_id",
    "request_id": "req_abc123"
  }
}
```

---

## 3. Request/Response Formats

### 3.1 Standard Response Envelope

All successful API responses follow a consistent structure:

**Success Response Structure**:
```json
{
  "id": "string",                   // Unique resource/request identifier
  "object": "string",               // Resource type (e.g., "chat.completion", "model", "list")
  "created": 1699564800,            // Unix timestamp (seconds since epoch)
  "model": "string",                // Model identifier (for completions)

  // Resource-specific fields
  "choices": [...],                 // For chat completions
  "data": [...],                    // For list endpoints
  "usage": {...},                   // Token usage statistics

  // Gateway metadata (optional)
  "x_gateway_metadata": {
    "provider": "string",           // Provider that fulfilled request
    "latency_ms": 342,              // End-to-end latency
    "retries": 0,                   // Number of retries performed
    "cached": false,                // Whether response was cached
    "request_id": "req_abc123"      // Correlation ID
  }
}
```

**List Response Structure**:
```json
{
  "object": "list",
  "data": [
    {
      "id": "item-1",
      "object": "model",
      // Item fields...
    }
  ],
  "has_more": false,                // Pagination indicator
  "next_cursor": "cursor_xyz"       // Cursor for next page (if has_more)
}
```

### 3.2 Error Response Format

All error responses use a standardized envelope:

**Error Response Structure**:
```json
{
  "error": {
    "type": "string",               // Error category (see section 1.3)
    "message": "string",            // Human-readable error description
    "code": "string",               // Machine-readable error code
    "param": "string | null",       // Field that caused error (if applicable)
    "request_id": "string",         // Correlation ID for debugging
    "details": {                    // Additional error context (optional)
      "provider_error": "string",   // Upstream provider error
      "validation_errors": [...]    // Validation failures
    }
  }
}
```

**Error Response Examples**:

```json
// Validation Error (400)
{
  "error": {
    "type": "invalid_request_error",
    "message": "Invalid temperature value: must be between 0 and 2",
    "code": "invalid_parameter_value",
    "param": "temperature",
    "request_id": "req_abc123",
    "details": {
      "provided_value": 2.5,
      "allowed_range": [0, 2]
    }
  }
}

// Provider Error (502)
{
  "error": {
    "type": "provider_error",
    "message": "Upstream provider returned error",
    "code": "upstream_error",
    "param": null,
    "request_id": "req_xyz789",
    "details": {
      "provider": "openai",
      "provider_error": "Rate limit exceeded",
      "provider_code": "rate_limit_exceeded"
    }
  }
}

// Multiple Validation Errors (422)
{
  "error": {
    "type": "invalid_request_error",
    "message": "Request validation failed",
    "code": "validation_error",
    "param": null,
    "request_id": "req_def456",
    "details": {
      "validation_errors": [
        {
          "field": "messages",
          "message": "Must contain at least one message"
        },
        {
          "field": "max_tokens",
          "message": "Must be a positive integer"
        }
      ]
    }
  }
}
```

### 3.3 HTTP Headers

**Standard Request Headers**:
```http
Authorization: Bearer sk-gateway-abc123
Content-Type: application/json
Accept: application/json
User-Agent: gateway-sdk-python/1.0.0
Idempotency-Key: 550e8400-e29b-41d4-a716-446655440000
X-Request-ID: client-generated-id-123
X-Tenant-ID: tenant-456
```

**Standard Response Headers**:
```http
Content-Type: application/json
X-Request-ID: req_abc123
X-Gateway-Version: 1.0.0
X-Provider: openai
X-Latency-Ms: 342
Cache-Control: no-store
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1699564860
```

**Streaming Response Headers**:
```http
Content-Type: text/event-stream
Cache-Control: no-cache
Connection: keep-alive
X-Accel-Buffering: no
Transfer-Encoding: chunked
```

---

## 4. API Versioning Strategy

### 4.1 Version Scheme

The gateway uses **URI versioning** as the primary versioning mechanism:

**URI Versioning**:
```
https://gateway.example.com/v1/chat/completions  (Current: v1)
https://gateway.example.com/v2/chat/completions  (Future: v2)
```

**Version Format**: `/v{MAJOR}/` where MAJOR is an integer (1, 2, 3, ...)

**Header Versioning (Optional Fallback)**:
```http
GET /chat/completions
X-API-Version: v1
```

**Version Negotiation**:
- URI version takes precedence over header version
- Absence of version defaults to latest stable version
- Invalid version returns 404 with supported versions in error details

**Semantic Versioning for Releases**:
```
Gateway Release: v1.5.2
  1 = Major (breaking changes)
  5 = Minor (backwards-compatible features)
  2 = Patch (backwards-compatible fixes)

API Version: v1
  Only major version exposed in API URI
```

### 4.2 Compatibility Policy

**Breaking Changes** (require new major version):
- Removing endpoints or parameters
- Changing required parameters
- Modifying response structure in incompatible ways
- Changing authentication mechanisms
- Altering error response formats

**Non-Breaking Changes** (allowed in minor versions):
- Adding new endpoints
- Adding optional parameters
- Adding new fields to responses
- Adding new error codes
- Performance improvements
- Bug fixes

**Deprecation Timeline**:
```
T+0:  Deprecation announced (changelog, docs, blog)
      Warning header added: Sunset: Sat, 27 May 2026 00:00:00 GMT

T+3m: Deprecation warnings in API responses
      Header: Warning: 299 - "Deprecated API version, migrate to v2"

T+6m: Version sunset - endpoint returns 410 Gone
      Includes migration guide in error response
```

**Example Deprecation Response**:
```http
HTTP/1.1 200 OK
Warning: 299 - "API v1 deprecated, migrate to v2 by 2026-05-27"
Sunset: Sat, 27 May 2026 00:00:00 GMT
Link: <https://gateway.example.com/docs/migration-v1-v2>; rel="deprecation"

{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  ...
}
```

**Version Sunset Response**:
```http
HTTP/1.1 410 Gone
Content-Type: application/json

{
  "error": {
    "type": "invalid_request_error",
    "message": "API v1 has been sunset as of 2026-05-27",
    "code": "api_version_sunset",
    "param": null,
    "request_id": "req_abc123",
    "details": {
      "sunset_date": "2026-05-27T00:00:00Z",
      "current_version": "v2",
      "migration_guide": "https://gateway.example.com/docs/migration-v1-v2"
    }
  }
}
```

### 4.3 Migration Support

**Version Coexistence Period**: 6 months minimum

During coexistence:
- Both versions available simultaneously
- Independent release cadence
- Separate monitoring and SLOs
- Shared infrastructure (providers, routing)

**Automatic Request Upgrade** (opt-in):
```http
POST /v1/chat/completions
X-API-Version-Upgrade: auto

// Gateway may upgrade to v2 internally if safe
```

**Migration Guides**:

```markdown
# Migration Guide: v1 → v2

## Breaking Changes

### 1. Error Response Format
**v1**:
{
  "error": {
    "message": "Error occurred",
    "type": "invalid_request_error"
  }
}

**v2**:
{
  "error": {
    "type": "invalid_request_error",
    "message": "Error occurred",
    "code": "error_code",        // NEW: Machine-readable code
    "request_id": "req_abc123"   // NEW: Always included
  }
}

### 2. Streaming Format
**v1**: SSE with newline delimiters
**v2**: SSE with explicit event types

## New Features in v2
- Multi-modal support (images, audio)
- Function calling v2 (parallel functions)
- Streaming usage statistics
- Enhanced metadata

## Migration Steps
1. Update SDK to v2-compatible version
2. Update error handling for new format
3. Test streaming integration
4. Deploy to staging
5. Gradual rollout to production
```

---

## 5. Rate Limiting Design

### 5.1 Rate Limit Algorithm

**Token Bucket Algorithm**:
- Each client/tenant has a bucket with configurable capacity
- Tokens added at configured rate (e.g., 100/minute)
- Each request consumes tokens
- Request rejected if insufficient tokens

**Configurable Tiers**:
```yaml
rate_limits:
  tiers:
    - name: free
      requests_per_minute: 60
      requests_per_hour: 1000
      requests_per_day: 10000
      burst_size: 10

    - name: pro
      requests_per_minute: 500
      requests_per_hour: 20000
      requests_per_day: 200000
      burst_size: 50

    - name: enterprise
      requests_per_minute: 5000
      requests_per_hour: 500000
      burst_size: 500
      custom_limits: true
```

### 5.2 Rate Limit Headers

**Standard Headers** (returned on all requests):
```http
X-RateLimit-Limit: 100              // Requests allowed in window
X-RateLimit-Remaining: 45           // Requests remaining in current window
X-RateLimit-Reset: 1699564860       // Unix timestamp when limit resets
X-RateLimit-Window: 60              // Window size in seconds
```

**Rate Limit Exceeded (429)**:
```http
HTTP/1.1 429 Too Many Requests
Content-Type: application/json
Retry-After: 30
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1699564860

{
  "error": {
    "type": "rate_limit_error",
    "message": "Rate limit exceeded: 100 requests per minute",
    "code": "rate_limit_exceeded",
    "param": null,
    "request_id": "req_abc123",
    "details": {
      "limit": 100,
      "window": "1m",
      "retry_after_seconds": 30
    }
  }
}
```

### 5.3 Rate Limit Scope

**Per-Key Rate Limits**:
```http
Authorization: Bearer sk-gateway-abc123
// Rate limit applies to this API key
```

**Per-Tenant Rate Limits**:
```http
X-Tenant-ID: tenant-456
// Additional limit per tenant (shared across keys)
```

**Per-User Rate Limits**:
```json
{
  "user": "user-789",  // In request body
  // User-level limit (e.g., for end-user abuse prevention)
}
```

**Per-Endpoint Rate Limits**:
```
/v1/chat/completions → 100/min (compute-intensive)
/v1/models           → 1000/min (read-only)
/health/ready        → unlimited  (health checks)
```

### 5.4 Burst Allowance

**Burst Configuration**:
```yaml
rate_limit:
  sustained_rate: 100   # Tokens per minute
  burst_size: 20        # Extra tokens for bursts
```

**Burst Behavior**:
```
Bucket Capacity: 120 tokens (100 sustained + 20 burst)
Refill Rate: 100 tokens/minute (1.67 tokens/second)

Scenario: Client sends 50 requests instantly
  - Consumes 50 tokens from bucket
  - Remaining: 70 tokens
  - Client can send 70 more before rate limit
  - Bucket refills at 1.67 tokens/second
```

---

## 6. Authentication Headers

### 6.1 Bearer Token Authentication

**Header Format**:
```http
Authorization: Bearer sk-gateway-abc123def456
```

**Token Types**:
- `sk-gateway-*` - Standard API keys
- `sk-admin-*` - Admin-level keys (access to `/admin/*` endpoints)
- `sk-readonly-*` - Read-only keys (no POST/PUT/DELETE)

**Token Generation**:
```bash
# Generate new API key
curl -X POST https://gateway.example.com/admin/api-keys \
  -H "Authorization: Bearer sk-admin-master-key" \
  -d '{
    "name": "Production App Key",
    "scopes": ["chat:create", "models:read"],
    "rate_limit_tier": "pro",
    "expires_at": "2026-12-31T23:59:59Z"
  }'

# Response
{
  "api_key": "sk-gateway-new-key-xyz789",
  "name": "Production App Key",
  "created_at": "2025-11-27T10:30:00Z",
  "expires_at": "2026-12-31T23:59:59Z"
}
```

### 6.2 API Key Authentication

**Alternative Header**:
```http
X-API-Key: sk-gateway-abc123def456
```

**Use Case**: Environments where `Authorization` header is restricted

### 6.3 Custom Tenant Headers

**Multi-Tenancy Support**:
```http
X-Tenant-ID: org-acme-corp
X-Project-ID: project-customer-support
X-Environment: production
```

**Tenant-Based Routing**:
```yaml
routing:
  tenant_overrides:
    org-acme-corp:
      default_provider: openai-dedicated
      fallback_providers: [anthropic-backup]
      rate_limit_tier: enterprise
```

### 6.4 OAuth 2.0 / OIDC (Enterprise)

**Bearer Token (JWT)**:
```http
Authorization: Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9...
```

**Token Validation**:
- Signature verification against JWKS endpoint
- Expiration check (`exp` claim)
- Audience validation (`aud` claim)
- Issuer validation (`iss` claim)
- Scope verification (`scope` claim)

**Required JWT Claims**:
```json
{
  "iss": "https://auth.example.com",
  "sub": "user-123",
  "aud": "https://gateway.example.com",
  "exp": 1699564860,
  "iat": 1699564800,
  "scope": "chat:create models:read"
}
```

### 6.5 Mutual TLS (mTLS)

**Certificate-Based Authentication**:
- Client presents X.509 certificate during TLS handshake
- Gateway validates certificate against trusted CA
- Subject DN mapped to tenant/user identity

**Configuration**:
```yaml
authentication:
  mtls:
    enabled: true
    ca_certificate: /etc/gateway/ca.crt
    client_certificate_header: X-SSL-Client-Cert
    subject_dn_header: X-SSL-Client-DN
```

**Identity Extraction**:
```
Certificate Subject: CN=app.acme.com,O=Acme Corp,C=US
Mapped Identity:
  Tenant: acme-corp
  Application: app
  Environment: production
```

---

## 7. Pagination & Filtering

### 7.1 Cursor-Based Pagination

**Recommended for Large Datasets** (consistent results during data changes)

**Request**:
```http
GET /v1/models?limit=20&cursor=eyJpZCI6Im1vZGVsLTEwMCJ9
```

**Response**:
```json
{
  "object": "list",
  "data": [
    {"id": "model-101", ...},
    {"id": "model-102", ...}
  ],
  "has_more": true,
  "next_cursor": "eyJpZCI6Im1vZGVsLTEyMCJ9",
  "previous_cursor": "eyJpZCI6Im1vZGVsLTEwMCJ9"
}
```

**Cursor Format**: Base64-encoded JSON
```json
// Decoded cursor
{
  "id": "model-100",
  "created": 1699564800
}
```

### 7.2 Limit/Offset Pagination

**Simple but Inconsistent** (for small datasets or UI pagination)

**Request**:
```http
GET /v1/models?limit=20&offset=40
```

**Response**:
```json
{
  "object": "list",
  "data": [...],
  "total": 150,
  "limit": 20,
  "offset": 40,
  "has_more": true
}
```

**Constraints**:
- `limit`: 1-100 (default: 20)
- `offset`: 0+ (default: 0)

### 7.3 Filtering Syntax

**Query Parameters**:
```http
GET /v1/models?
  provider=openai&
  capability=function_calling&
  min_context=8000&
  sort=-created&
  limit=10
```

**Supported Operators**:
```
field=value           // Exact match
field__gt=value       // Greater than
field__gte=value      // Greater than or equal
field__lt=value       // Less than
field__lte=value      // Less than or equal
field__in=val1,val2   // In list
field__contains=text  // Substring match
field__startswith=pre // Prefix match
```

**Examples**:
```http
// Models with context window >= 100k
GET /v1/models?context_window__gte=100000

// OpenAI or Anthropic models
GET /v1/models?owned_by__in=openai,anthropic

// Models created after date
GET /v1/models?created__gt=1699564800

// Sort by price (ascending)
GET /v1/models?sort=pricing.input_per_million

// Sort by price (descending)
GET /v1/models?sort=-pricing.input_per_million
```

---

## 8. OpenAPI Specification

### 8.1 Complete OpenAPI 3.0 Spec

```yaml
openapi: 3.0.3
info:
  title: LLM-Inference-Gateway API
  description: |
    Unified edge-serving gateway for heterogeneous LLM inference backends.

    ## Features
    - OpenAI API compatibility
    - Multi-provider routing and failover
    - Real-time streaming (SSE)
    - Comprehensive observability

    ## Authentication
    All endpoints require authentication via Bearer token or API key.

    ## Rate Limiting
    Rate limits vary by tier. See headers for current limits.

  version: 1.0.0
  contact:
    name: API Support
    email: api@example.com
    url: https://gateway.example.com/support
  license:
    name: Proprietary
    url: https://gateway.example.com/license

servers:
  - url: https://gateway.example.com/v1
    description: Production Server
  - url: https://gateway-staging.example.com/v1
    description: Staging Server
  - url: http://localhost:8080/v1
    description: Local Development

tags:
  - name: chat
    description: Chat completion endpoints
  - name: models
    description: Model information endpoints
  - name: health
    description: Health check endpoints
  - name: admin
    description: Administrative endpoints (requires admin auth)

paths:
  /chat/completions:
    post:
      tags: [chat]
      summary: Create chat completion
      description: |
        Generate chat completion using configured LLM providers.
        Supports both streaming and non-streaming modes.
      operationId: createChatCompletion
      security:
        - BearerAuth: []
        - ApiKeyAuth: []
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/CreateChatCompletionRequest'
            examples:
              simple:
                summary: Simple chat completion
                value:
                  model: gpt-4
                  messages:
                    - role: user
                      content: What is the capital of France?
              streaming:
                summary: Streaming completion
                value:
                  model: gpt-4
                  messages:
                    - role: user
                      content: Count to 10
                  stream: true
      responses:
        '200':
          description: Successful completion
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ChatCompletionResponse'
            text/event-stream:
              schema:
                $ref: '#/components/schemas/ChatCompletionChunk'
        '400':
          $ref: '#/components/responses/BadRequest'
        '401':
          $ref: '#/components/responses/Unauthorized'
        '429':
          $ref: '#/components/responses/TooManyRequests'
        '503':
          $ref: '#/components/responses/ServiceUnavailable'

  /models:
    get:
      tags: [models]
      summary: List available models
      description: Retrieve list of all available models across configured providers
      operationId: listModels
      security:
        - BearerAuth: []
        - ApiKeyAuth: []
      parameters:
        - $ref: '#/components/parameters/LimitParam'
        - $ref: '#/components/parameters/CursorParam'
      responses:
        '200':
          description: List of models
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ListModelsResponse'
        '401':
          $ref: '#/components/responses/Unauthorized'

  /models/{model_id}:
    get:
      tags: [models]
      summary: Retrieve model details
      description: Get detailed information about a specific model
      operationId: retrieveModel
      security:
        - BearerAuth: []
        - ApiKeyAuth: []
      parameters:
        - name: model_id
          in: path
          required: true
          description: Model identifier
          schema:
            type: string
          example: gpt-4
      responses:
        '200':
          description: Model details
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Model'
        '404':
          $ref: '#/components/responses/NotFound'

  /health/live:
    get:
      tags: [health]
      summary: Liveness probe
      description: Check if gateway process is alive
      operationId: healthLive
      security: []
      responses:
        '200':
          description: Gateway is alive
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/HealthStatus'
        '503':
          description: Gateway is dead
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/HealthStatus'

  /health/ready:
    get:
      tags: [health]
      summary: Readiness probe
      description: Check if gateway is ready to accept traffic
      operationId: healthReady
      security: []
      responses:
        '200':
          description: Gateway is ready
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ReadinessStatus'
        '503':
          description: Gateway is not ready
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ReadinessStatus'

  /health/providers:
    get:
      tags: [health]
      summary: Provider health status
      description: Detailed health status for all configured providers
      operationId: healthProviders
      security:
        - BearerAuth: []
      responses:
        '200':
          description: Provider health status
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ProviderHealthStatus'

components:
  securitySchemes:
    BearerAuth:
      type: http
      scheme: bearer
      bearerFormat: JWT
      description: |
        API authentication using Bearer tokens.
        Format: `Authorization: Bearer sk-gateway-abc123`

    ApiKeyAuth:
      type: apiKey
      in: header
      name: X-API-Key
      description: |
        API authentication using custom header.
        Format: `X-API-Key: sk-gateway-abc123`

  parameters:
    LimitParam:
      name: limit
      in: query
      description: Maximum number of items to return
      schema:
        type: integer
        minimum: 1
        maximum: 100
        default: 20

    CursorParam:
      name: cursor
      in: query
      description: Pagination cursor from previous response
      schema:
        type: string

  schemas:
    CreateChatCompletionRequest:
      type: object
      required:
        - model
        - messages
      properties:
        model:
          type: string
          description: Model identifier to use for completion
          example: gpt-4
        messages:
          type: array
          description: List of messages in the conversation
          minItems: 1
          items:
            $ref: '#/components/schemas/ChatMessage'
        temperature:
          type: number
          minimum: 0
          maximum: 2
          default: 1
          description: Sampling temperature (0-2)
        top_p:
          type: number
          minimum: 0
          maximum: 1
          default: 1
          description: Nucleus sampling threshold
        n:
          type: integer
          minimum: 1
          maximum: 10
          default: 1
          description: Number of completions to generate
        stream:
          type: boolean
          default: false
          description: Enable Server-Sent Events streaming
        stop:
          oneOf:
            - type: string
            - type: array
              items:
                type: string
          description: Stop sequences
        max_tokens:
          type: integer
          minimum: 1
          description: Maximum tokens to generate
        presence_penalty:
          type: number
          minimum: -2
          maximum: 2
          default: 0
        frequency_penalty:
          type: number
          minimum: -2
          maximum: 2
          default: 0
        user:
          type: string
          description: End-user identifier for tracking

    ChatMessage:
      type: object
      required:
        - role
        - content
      properties:
        role:
          type: string
          enum: [system, user, assistant, function]
          description: Message sender role
        content:
          type: string
          description: Message content
        name:
          type: string
          description: Optional sender name

    ChatCompletionResponse:
      type: object
      properties:
        id:
          type: string
          example: chatcmpl-abc123
        object:
          type: string
          enum: [chat.completion]
        created:
          type: integer
          description: Unix timestamp
        model:
          type: string
          example: gpt-4
        choices:
          type: array
          items:
            $ref: '#/components/schemas/ChatCompletionChoice'
        usage:
          $ref: '#/components/schemas/Usage'
        x_gateway_metadata:
          $ref: '#/components/schemas/GatewayMetadata'

    ChatCompletionChoice:
      type: object
      properties:
        index:
          type: integer
        message:
          $ref: '#/components/schemas/ChatMessage'
        finish_reason:
          type: string
          enum: [stop, length, function_call, content_filter]

    ChatCompletionChunk:
      type: object
      description: Streaming response chunk (SSE format)
      properties:
        id:
          type: string
        object:
          type: string
          enum: [chat.completion.chunk]
        created:
          type: integer
        model:
          type: string
        choices:
          type: array
          items:
            type: object
            properties:
              index:
                type: integer
              delta:
                type: object
                properties:
                  role:
                    type: string
                  content:
                    type: string
              finish_reason:
                type: string
                nullable: true

    Usage:
      type: object
      properties:
        prompt_tokens:
          type: integer
        completion_tokens:
          type: integer
        total_tokens:
          type: integer

    GatewayMetadata:
      type: object
      description: Gateway-specific metadata (extension field)
      properties:
        provider:
          type: string
          example: openai
        latency_ms:
          type: integer
          example: 342
        retries:
          type: integer
          example: 0
        cached:
          type: boolean
          example: false
        request_id:
          type: string
          example: req_abc123

    ListModelsResponse:
      type: object
      properties:
        object:
          type: string
          enum: [list]
        data:
          type: array
          items:
            $ref: '#/components/schemas/Model'

    Model:
      type: object
      properties:
        id:
          type: string
          example: gpt-4
        object:
          type: string
          enum: [model]
        created:
          type: integer
        owned_by:
          type: string
          example: openai
        x_gateway_info:
          type: object
          properties:
            providers:
              type: array
              items:
                type: string
            capabilities:
              type: array
              items:
                type: string
            context_window:
              type: integer
            pricing:
              type: object
              properties:
                input_per_million:
                  type: number
                output_per_million:
                  type: number

    HealthStatus:
      type: object
      properties:
        status:
          type: string
          enum: [alive, dead]
        timestamp:
          type: string
          format: date-time

    ReadinessStatus:
      type: object
      properties:
        status:
          type: string
          enum: [ready, not_ready]
        timestamp:
          type: string
          format: date-time
        checks:
          type: object
          additionalProperties:
            type: string

    ProviderHealthStatus:
      type: object
      properties:
        timestamp:
          type: string
          format: date-time
        providers:
          type: array
          items:
            type: object
            properties:
              id:
                type: string
              name:
                type: string
              type:
                type: string
              status:
                type: string
                enum: [healthy, degraded, unhealthy]
              circuit_breaker:
                type: string
                enum: [closed, half_open, open]

    Error:
      type: object
      properties:
        error:
          type: object
          properties:
            type:
              type: string
              enum:
                - invalid_request_error
                - authentication_error
                - permission_error
                - not_found_error
                - rate_limit_error
                - provider_error
                - internal_error
                - timeout_error
                - conflict_error
            message:
              type: string
            code:
              type: string
            param:
              type: string
              nullable: true
            request_id:
              type: string

  responses:
    BadRequest:
      description: Invalid request
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/Error'

    Unauthorized:
      description: Missing or invalid authentication
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/Error'

    NotFound:
      description: Resource not found
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/Error'

    TooManyRequests:
      description: Rate limit exceeded
      headers:
        X-RateLimit-Limit:
          schema:
            type: integer
        X-RateLimit-Remaining:
          schema:
            type: integer
        X-RateLimit-Reset:
          schema:
            type: integer
        Retry-After:
          schema:
            type: integer
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/Error'

    ServiceUnavailable:
      description: Service temporarily unavailable
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/Error'
```

---

## 9. SDK Design Guidelines

### 9.1 Client Library Patterns

**Core SDK Structure**:
```python
# Python SDK Example
from llm_gateway import Gateway, ChatMessage

# Initialize client
client = Gateway(
    api_key="sk-gateway-abc123",
    base_url="https://gateway.example.com/v1",
    timeout=30.0
)

# Simple completion
response = client.chat.completions.create(
    model="gpt-4",
    messages=[
        ChatMessage(role="user", content="Hello")
    ]
)

# Streaming completion
for chunk in client.chat.completions.create(
    model="gpt-4",
    messages=[ChatMessage(role="user", content="Count to 5")],
    stream=True
):
    print(chunk.choices[0].delta.content, end="")
```

**TypeScript SDK Example**:
```typescript
import { Gateway, ChatMessage } from '@llm-gateway/sdk';

const client = new Gateway({
  apiKey: 'sk-gateway-abc123',
  baseURL: 'https://gateway.example.com/v1',
  timeout: 30000
});

// Simple completion
const response = await client.chat.completions.create({
  model: 'gpt-4',
  messages: [{ role: 'user', content: 'Hello' }]
});

// Streaming completion
const stream = await client.chat.completions.create({
  model: 'gpt-4',
  messages: [{ role: 'user', content: 'Count to 5' }],
  stream: true
});

for await (const chunk of stream) {
  process.stdout.write(chunk.choices[0]?.delta?.content || '');
}
```

### 9.2 Retry Logic Recommendations

**Exponential Backoff**:
```python
class GatewayClient:
    def __init__(self, max_retries=3, base_delay=0.1):
        self.max_retries = max_retries
        self.base_delay = base_delay

    def _should_retry(self, status_code: int) -> bool:
        """Determine if request should be retried"""
        return status_code in {
            408,  # Request Timeout
            429,  # Too Many Requests
            500,  # Internal Server Error
            502,  # Bad Gateway
            503,  # Service Unavailable
            504   # Gateway Timeout
        }

    def _calculate_delay(self, attempt: int) -> float:
        """Calculate retry delay with exponential backoff and jitter"""
        delay = self.base_delay * (2 ** attempt)
        jitter = random.uniform(0, delay * 0.25)
        return min(delay + jitter, 10.0)  # Max 10s delay

    def request(self, method, endpoint, **kwargs):
        for attempt in range(self.max_retries + 1):
            try:
                response = self._http_client.request(method, endpoint, **kwargs)

                if response.status_code < 400:
                    return response

                if not self._should_retry(response.status_code):
                    raise GatewayError(response)

                if attempt < self.max_retries:
                    delay = self._calculate_delay(attempt)
                    time.sleep(delay)
                else:
                    raise GatewayError(response)

            except (ConnectionError, TimeoutError) as e:
                if attempt < self.max_retries:
                    delay = self._calculate_delay(attempt)
                    time.sleep(delay)
                else:
                    raise GatewayConnectionError(e)
```

**Respect Retry-After Header**:
```python
def _get_retry_delay(self, response):
    """Extract retry delay from response headers"""
    if 'Retry-After' in response.headers:
        retry_after = response.headers['Retry-After']
        try:
            # Try parsing as seconds
            return float(retry_after)
        except ValueError:
            # Try parsing as HTTP date
            retry_date = email.utils.parsedate_to_datetime(retry_after)
            return (retry_date - datetime.now()).total_seconds()

    return None
```

### 9.3 Streaming Consumption

**Python Streaming**:
```python
def stream_chat_completion(client, messages):
    """Streaming with error handling and connection recovery"""
    stream = client.chat.completions.create(
        model="gpt-4",
        messages=messages,
        stream=True
    )

    buffer = []
    try:
        for chunk in stream:
            if chunk.choices[0].delta.content:
                content = chunk.choices[0].delta.content
                buffer.append(content)
                yield content

            # Check for finish
            if chunk.choices[0].finish_reason:
                break

    except StreamInterruptedError as e:
        # Attempt recovery with full response
        logger.warning(f"Stream interrupted: {e}")
        response = client.chat.completions.create(
            model="gpt-4",
            messages=messages,
            stream=False
        )
        # Yield remaining content
        full_content = response.choices[0].message.content
        buffered_content = ''.join(buffer)
        remaining = full_content[len(buffered_content):]
        yield remaining
```

**TypeScript Streaming**:
```typescript
async function* streamChatCompletion(
  client: Gateway,
  messages: ChatMessage[]
): AsyncGenerator<string> {
  const stream = await client.chat.completions.create({
    model: 'gpt-4',
    messages,
    stream: true
  });

  try {
    for await (const chunk of stream) {
      const content = chunk.choices[0]?.delta?.content;
      if (content) {
        yield content;
      }

      if (chunk.choices[0]?.finish_reason) {
        break;
      }
    }
  } catch (error) {
    if (error instanceof StreamInterruptedError) {
      // Fallback to non-streaming
      const response = await client.chat.completions.create({
        model: 'gpt-4',
        messages,
        stream: false
      });
      yield response.choices[0].message.content;
    } else {
      throw error;
    }
  }
}
```

### 9.4 Error Handling

**Typed Exceptions**:
```python
class GatewayError(Exception):
    """Base exception for all gateway errors"""
    def __init__(self, response):
        self.response = response
        self.status_code = response.status_code
        self.request_id = response.headers.get('X-Request-ID')

        error_data = response.json().get('error', {})
        self.type = error_data.get('type')
        self.message = error_data.get('message')
        self.code = error_data.get('code')
        self.param = error_data.get('param')

class InvalidRequestError(GatewayError):
    """400 Bad Request errors"""
    pass

class AuthenticationError(GatewayError):
    """401 Unauthorized errors"""
    pass

class RateLimitError(GatewayError):
    """429 Too Many Requests errors"""
    def __init__(self, response):
        super().__init__(response)
        self.retry_after = int(response.headers.get('Retry-After', 60))

class ProviderError(GatewayError):
    """Upstream provider errors (502, 503, 504)"""
    pass

# Usage
try:
    response = client.chat.completions.create(...)
except RateLimitError as e:
    print(f"Rate limited. Retry after {e.retry_after}s")
    time.sleep(e.retry_after)
except ProviderError as e:
    print(f"Provider unavailable: {e.message}")
    # Implement fallback logic
except AuthenticationError:
    print("Invalid API key")
    # Re-authenticate
```

**TypeScript Error Handling**:
```typescript
class GatewayError extends Error {
  constructor(
    public statusCode: number,
    public type: string,
    public code: string,
    public param: string | null,
    public requestId: string
  ) {
    super(`Gateway error: ${type} - ${code}`);
  }
}

class RateLimitError extends GatewayError {
  constructor(
    statusCode: number,
    type: string,
    code: string,
    param: string | null,
    requestId: string,
    public retryAfter: number
  ) {
    super(statusCode, type, code, param, requestId);
  }
}

// Usage
try {
  const response = await client.chat.completions.create({...});
} catch (error) {
  if (error instanceof RateLimitError) {
    console.log(`Rate limited. Retry after ${error.retryAfter}s`);
    await new Promise(resolve => setTimeout(resolve, error.retryAfter * 1000));
  } else if (error instanceof ProviderError) {
    console.log('Provider unavailable, implementing fallback...');
  }
}
```

---

## 10. API Documentation

### 10.1 Interactive Documentation (Swagger UI)

**Deployment**:
```yaml
# docker-compose.yml
services:
  swagger-ui:
    image: swaggerapi/swagger-ui:latest
    ports:
      - "8081:8080"
    environment:
      SWAGGER_JSON_URL: https://gateway.example.com/openapi.json
      VALIDATOR_URL: "null"
      DISPLAY_REQUEST_DURATION: "true"
      DEEP_LINKING: "true"
```

**Access**: `https://gateway.example.com/docs`

**Features**:
- Interactive API exploration
- Try-it-out functionality with authentication
- Request/response examples
- Schema validation
- Code generation

### 10.2 Code Examples Per Language

**Python**:
```python
# Installation
pip install llm-gateway-sdk

# Basic usage
from llm_gateway import Gateway

client = Gateway(api_key="sk-gateway-abc123")

response = client.chat.completions.create(
    model="gpt-4",
    messages=[
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "What is Python?"}
    ],
    temperature=0.7,
    max_tokens=150
)

print(response.choices[0].message.content)

# Streaming
for chunk in client.chat.completions.create(
    model="gpt-4",
    messages=[{"role": "user", "content": "Count to 10"}],
    stream=True
):
    print(chunk.choices[0].delta.content, end="", flush=True)
```

**TypeScript/JavaScript**:
```typescript
// Installation
npm install @llm-gateway/sdk

// Basic usage
import { Gateway } from '@llm-gateway/sdk';

const client = new Gateway({ apiKey: 'sk-gateway-abc123' });

const response = await client.chat.completions.create({
  model: 'gpt-4',
  messages: [
    { role: 'system', content: 'You are a helpful assistant.' },
    { role: 'user', content: 'What is TypeScript?' }
  ],
  temperature: 0.7,
  max_tokens: 150
});

console.log(response.choices[0].message.content);

// Streaming
const stream = await client.chat.completions.create({
  model: 'gpt-4',
  messages: [{ role: 'user', content: 'Count to 10' }],
  stream: true
});

for await (const chunk of stream) {
  process.stdout.write(chunk.choices[0]?.delta?.content || '');
}
```

**Go**:
```go
// Installation
go get github.com/llm-gateway/sdk-go

// Basic usage
package main

import (
    "context"
    "fmt"
    gateway "github.com/llm-gateway/sdk-go"
)

func main() {
    client := gateway.NewClient("sk-gateway-abc123")

    resp, err := client.Chat.Completions.Create(context.Background(), gateway.ChatCompletionRequest{
        Model: "gpt-4",
        Messages: []gateway.ChatMessage{
            {Role: "system", Content: "You are a helpful assistant."},
            {Role: "user", Content: "What is Go?"},
        },
        Temperature: 0.7,
        MaxTokens: 150,
    })

    if err != nil {
        panic(err)
    }

    fmt.Println(resp.Choices[0].Message.Content)
}
```

**Java**:
```java
// Installation (Maven)
<dependency>
    <groupId>com.llm-gateway</groupId>
    <artifactId>sdk-java</artifactId>
    <version>1.0.0</version>
</dependency>

// Basic usage
import com.llmgateway.Gateway;
import com.llmgateway.models.*;

public class Example {
    public static void main(String[] args) {
        Gateway client = new Gateway("sk-gateway-abc123");

        ChatCompletionRequest request = ChatCompletionRequest.builder()
            .model("gpt-4")
            .messages(List.of(
                new ChatMessage("system", "You are a helpful assistant."),
                new ChatMessage("user", "What is Java?")
            ))
            .temperature(0.7)
            .maxTokens(150)
            .build();

        ChatCompletionResponse response = client.chat().completions().create(request);

        System.out.println(response.getChoices().get(0).getMessage().getContent());
    }
}
```

**cURL**:
```bash
# Basic completion
curl -X POST https://gateway.example.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-gateway-abc123" \
  -d '{
    "model": "gpt-4",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "What is cURL?"}
    ],
    "temperature": 0.7,
    "max_tokens": 150
  }'

# Streaming completion
curl -X POST https://gateway.example.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-gateway-abc123" \
  -N \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Count to 10"}],
    "stream": true
  }'
```

### 10.3 Postman Collection

**Collection Structure**:
```json
{
  "info": {
    "name": "LLM-Inference-Gateway API",
    "description": "Complete API collection for LLM-Inference-Gateway",
    "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
  },
  "auth": {
    "type": "bearer",
    "bearer": [
      {
        "key": "token",
        "value": "{{api_key}}",
        "type": "string"
      }
    ]
  },
  "variable": [
    {
      "key": "base_url",
      "value": "https://gateway.example.com/v1",
      "type": "string"
    },
    {
      "key": "api_key",
      "value": "sk-gateway-abc123",
      "type": "string"
    }
  ],
  "item": [
    {
      "name": "Chat Completions",
      "item": [
        {
          "name": "Create Chat Completion",
          "request": {
            "method": "POST",
            "header": [
              {
                "key": "Content-Type",
                "value": "application/json"
              }
            ],
            "body": {
              "mode": "raw",
              "raw": "{\n  \"model\": \"gpt-4\",\n  \"messages\": [\n    {\"role\": \"user\", \"content\": \"Hello\"}\n  ]\n}"
            },
            "url": {
              "raw": "{{base_url}}/chat/completions",
              "host": ["{{base_url}}"],
              "path": ["chat", "completions"]
            }
          }
        },
        {
          "name": "Streaming Chat Completion",
          "request": {
            "method": "POST",
            "header": [
              {
                "key": "Content-Type",
                "value": "application/json"
              }
            ],
            "body": {
              "mode": "raw",
              "raw": "{\n  \"model\": \"gpt-4\",\n  \"messages\": [\n    {\"role\": \"user\", \"content\": \"Count to 5\"}\n  ],\n  \"stream\": true\n}"
            },
            "url": {
              "raw": "{{base_url}}/chat/completions",
              "host": ["{{base_url}}"],
              "path": ["chat", "completions"]
            }
          }
        }
      ]
    },
    {
      "name": "Models",
      "item": [
        {
          "name": "List Models",
          "request": {
            "method": "GET",
            "url": {
              "raw": "{{base_url}}/models",
              "host": ["{{base_url}}"],
              "path": ["models"]
            }
          }
        },
        {
          "name": "Retrieve Model",
          "request": {
            "method": "GET",
            "url": {
              "raw": "{{base_url}}/models/gpt-4",
              "host": ["{{base_url}}"],
              "path": ["models", "gpt-4"]
            }
          }
        }
      ]
    },
    {
      "name": "Health",
      "item": [
        {
          "name": "Liveness",
          "request": {
            "method": "GET",
            "url": {
              "raw": "{{base_url}}/health/live",
              "host": ["{{base_url}}"],
              "path": ["health", "live"]
            }
          }
        },
        {
          "name": "Readiness",
          "request": {
            "method": "GET",
            "url": {
              "raw": "{{base_url}}/health/ready",
              "host": ["{{base_url}}"],
              "path": ["health", "ready"]
            }
          }
        },
        {
          "name": "Provider Health",
          "request": {
            "method": "GET",
            "url": {
              "raw": "{{base_url}}/health/providers",
              "host": ["{{base_url}}"],
              "path": ["health", "providers"]
            }
          }
        }
      ]
    }
  ]
}
```

**Download Link**: `https://gateway.example.com/postman-collection.json`

**Import Instructions**:
1. Open Postman
2. Click "Import" → "Link"
3. Paste: `https://gateway.example.com/postman-collection.json`
4. Configure environment variables (`base_url`, `api_key`)
5. Start making requests

---

## Appendix

### A. Response Header Reference

| Header | Description | Example |
|--------|-------------|---------|
| `X-Request-ID` | Unique request identifier | `req_abc123def456` |
| `X-Gateway-Version` | Gateway software version | `1.0.0` |
| `X-Provider` | Provider that fulfilled request | `openai` |
| `X-Latency-Ms` | Request latency in milliseconds | `342` |
| `X-RateLimit-Limit` | Maximum requests in window | `100` |
| `X-RateLimit-Remaining` | Requests remaining | `95` |
| `X-RateLimit-Reset` | Unix timestamp when limit resets | `1699564860` |
| `Retry-After` | Seconds to wait before retry | `60` |
| `Idempotent-Replayed` | Request was deduplicated | `true` |

### B. HTTP Status Code Reference

| Code | Name | Usage |
|------|------|-------|
| 200 | OK | Successful request |
| 201 | Created | Resource created successfully |
| 400 | Bad Request | Invalid request syntax or parameters |
| 401 | Unauthorized | Missing or invalid authentication |
| 403 | Forbidden | Valid auth but insufficient permissions |
| 404 | Not Found | Resource does not exist |
| 409 | Conflict | Request conflicts with current state |
| 422 | Unprocessable Entity | Validation failed |
| 429 | Too Many Requests | Rate limit exceeded |
| 500 | Internal Server Error | Unexpected gateway error |
| 502 | Bad Gateway | Invalid upstream response |
| 503 | Service Unavailable | Temporary unavailability |
| 504 | Gateway Timeout | Upstream timeout |

### C. Migration Checklist (v1 → v2)

- [ ] Review breaking changes in migration guide
- [ ] Update SDK to v2-compatible version
- [ ] Update error handling for new error format
- [ ] Test streaming integration with new SSE format
- [ ] Update authentication if changed
- [ ] Review and update rate limiting logic
- [ ] Test in staging environment
- [ ] Monitor metrics during gradual rollout
- [ ] Update documentation and internal guides

### D. Support and Resources

- **Documentation**: https://gateway.example.com/docs
- **OpenAPI Spec**: https://gateway.example.com/openapi.json
- **Postman Collection**: https://gateway.example.com/postman-collection.json
- **SDK Repositories**:
  - Python: https://github.com/llm-gateway/sdk-python
  - TypeScript: https://github.com/llm-gateway/sdk-typescript
  - Go: https://github.com/llm-gateway/sdk-go
  - Java: https://github.com/llm-gateway/sdk-java
- **Support**: api-support@example.com
- **Status Page**: https://status.gateway.example.com

---

**Document Version**: 1.0.0
**Last Updated**: 2025-11-27
**Maintained By**: LLM DevOps Platform Team
