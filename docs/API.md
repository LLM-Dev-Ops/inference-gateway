# LLM Inference Gateway - API Reference

Complete API documentation for the LLM Inference Gateway.

## Base URL

```
http://localhost:8080
```

## Authentication

The gateway supports multiple authentication methods:

### API Key Authentication

```bash
curl -H "X-API-Key: your-api-key" http://localhost:8080/v1/models
```

### Bearer Token (JWT)

```bash
curl -H "Authorization: Bearer your-jwt-token" http://localhost:8080/v1/models
```

### No Authentication (Development)

By default, authentication is optional for development. Enable it via configuration for production.

---

## Endpoints

### Health & Status

#### Health Check

Check the overall health of the gateway.

```
GET /health
```

**Response:**

```json
{
  "status": "healthy",
  "timestamp": "2024-11-28T12:00:00Z",
  "version": "0.1.0"
}
```

**Status Codes:**
- `200 OK` - Gateway is healthy
- `503 Service Unavailable` - Gateway is unhealthy

---

#### Readiness Check

Kubernetes readiness probe endpoint.

```
GET /ready
```

**Response:**

```json
{
  "status": "ready",
  "providers": {
    "openai": true,
    "anthropic": true
  }
}
```

**Status Codes:**
- `200 OK` - Gateway is ready to accept traffic
- `503 Service Unavailable` - Gateway is not ready

---

#### Liveness Check

Kubernetes liveness probe endpoint.

```
GET /live
```

**Response:**

```json
{
  "status": "alive"
}
```

**Status Codes:**
- `200 OK` - Gateway is alive

---

### Models

#### List Models

List all available models from configured providers.

```
GET /v1/models
```

**Response:**

```json
{
  "object": "list",
  "data": [
    {
      "id": "gpt-4o",
      "object": "model",
      "created": 1698959748,
      "owned_by": "openai"
    },
    {
      "id": "gpt-4o-mini",
      "object": "model",
      "created": 1698959748,
      "owned_by": "openai"
    },
    {
      "id": "claude-3-5-sonnet-latest",
      "object": "model",
      "created": 1698959748,
      "owned_by": "anthropic"
    }
  ]
}
```

---

#### Get Model

Get details for a specific model.

```
GET /v1/models/{model_id}
```

**Parameters:**
- `model_id` (path) - The model identifier

**Response:**

```json
{
  "id": "gpt-4o",
  "object": "model",
  "created": 1698959748,
  "owned_by": "openai"
}
```

---

### Chat Completions

#### Create Chat Completion

Create a chat completion using the specified model.

```
POST /v1/chat/completions
```

**Request Body:**

```json
{
  "model": "gpt-4o-mini",
  "messages": [
    {
      "role": "system",
      "content": "You are a helpful assistant."
    },
    {
      "role": "user",
      "content": "Hello, how are you?"
    }
  ],
  "temperature": 0.7,
  "max_tokens": 1000,
  "top_p": 1.0,
  "stream": false,
  "stop": null,
  "presence_penalty": 0,
  "frequency_penalty": 0,
  "user": "user-123"
}
```

**Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `model` | string | Yes | - | Model identifier |
| `messages` | array | Yes | - | Array of message objects |
| `temperature` | number | No | 1.0 | Sampling temperature (0-2) |
| `max_tokens` | integer | No | - | Maximum tokens to generate |
| `top_p` | number | No | 1.0 | Nucleus sampling parameter |
| `stream` | boolean | No | false | Enable streaming responses |
| `stop` | string/array | No | null | Stop sequences |
| `presence_penalty` | number | No | 0 | Presence penalty (-2 to 2) |
| `frequency_penalty` | number | No | 0 | Frequency penalty (-2 to 2) |
| `user` | string | No | - | User identifier for tracking |

**Message Object:**

```json
{
  "role": "user",
  "content": "Hello!"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `role` | string | Yes | One of: `system`, `user`, `assistant` |
| `content` | string/array | Yes | Message content |
| `name` | string | No | Optional name for the participant |

**Response (Non-streaming):**

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1698959748,
  "model": "gpt-4o-mini",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! I'm doing well, thank you for asking. How can I help you today?"
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 25,
    "completion_tokens": 18,
    "total_tokens": 43
  }
}
```

**Response (Streaming):**

When `stream: true`, the response is sent as Server-Sent Events (SSE):

```
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1698959748,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1698959748,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1698959748,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{"content":"!"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1698959748,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
```

---

### Vision (Multi-modal)

Send images along with text messages.

```
POST /v1/chat/completions
```

**Request Body:**

```json
{
  "model": "gpt-4o",
  "messages": [
    {
      "role": "user",
      "content": [
        {
          "type": "text",
          "text": "What's in this image?"
        },
        {
          "type": "image_url",
          "image_url": {
            "url": "https://example.com/image.jpg",
            "detail": "high"
          }
        }
      ]
    }
  ]
}
```

**Image URL Options:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `url` | string | Yes | Image URL or base64 data URI |
| `detail` | string | No | `low`, `high`, or `auto` (default) |

**Base64 Image:**

```json
{
  "type": "image_url",
  "image_url": {
    "url": "data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQABAAD..."
  }
}
```

---

### Admin Endpoints

#### List Providers

Get status of all configured providers.

```
GET /admin/providers
```

**Response:**

```json
{
  "providers": [
    {
      "id": "openai",
      "name": "OpenAI",
      "enabled": true,
      "healthy": true,
      "models": ["gpt-4o", "gpt-4o-mini", "gpt-4-turbo"],
      "rate_limit": {
        "requests_per_minute": 1000,
        "tokens_per_minute": 100000
      }
    },
    {
      "id": "anthropic",
      "name": "Anthropic",
      "enabled": true,
      "healthy": true,
      "models": ["claude-3-5-sonnet-latest", "claude-3-opus-latest"],
      "rate_limit": {
        "requests_per_minute": 500,
        "tokens_per_minute": 80000
      }
    }
  ]
}
```

---

#### Gateway Statistics

Get gateway operational statistics.

```
GET /admin/stats
```

**Response:**

```json
{
  "requests": {
    "total": 1000000,
    "success": 998500,
    "failed": 1500,
    "rate_limited": 250
  },
  "cache": {
    "hits": 450000,
    "misses": 548500,
    "hit_ratio": 0.45
  },
  "latency": {
    "p50_ms": 150,
    "p95_ms": 450,
    "p99_ms": 850
  },
  "tokens": {
    "input": 50000000,
    "output": 25000000,
    "total": 75000000
  },
  "uptime_seconds": 86400
}
```

---

### Metrics

#### Prometheus Metrics

Get Prometheus-formatted metrics.

```
GET /metrics
```

**Response:**

```
# HELP llm_gateway_requests_total Total number of requests
# TYPE llm_gateway_requests_total counter
llm_gateway_requests_total{provider="openai",model="gpt-4o-mini",status="success"} 10000
llm_gateway_requests_total{provider="openai",model="gpt-4o-mini",status="error"} 15

# HELP llm_gateway_request_duration_seconds Request duration in seconds
# TYPE llm_gateway_request_duration_seconds histogram
llm_gateway_request_duration_seconds_bucket{provider="openai",le="0.1"} 5000
llm_gateway_request_duration_seconds_bucket{provider="openai",le="0.5"} 9500
llm_gateway_request_duration_seconds_bucket{provider="openai",le="1.0"} 9900
llm_gateway_request_duration_seconds_bucket{provider="openai",le="+Inf"} 10000

# HELP llm_gateway_tokens_total Total tokens processed
# TYPE llm_gateway_tokens_total counter
llm_gateway_tokens_total{provider="openai",type="input"} 500000
llm_gateway_tokens_total{provider="openai",type="output"} 250000
```

---

## Error Handling

### Error Response Format

All errors follow a consistent format:

```json
{
  "error": {
    "type": "invalid_request_error",
    "message": "The model 'invalid-model' does not exist",
    "code": "model_not_found",
    "param": "model"
  }
}
```

### Error Types

| Type | Description |
|------|-------------|
| `invalid_request_error` | Invalid parameters or request format |
| `authentication_error` | Authentication failed |
| `permission_error` | Insufficient permissions |
| `not_found_error` | Resource not found |
| `rate_limit_error` | Rate limit exceeded |
| `server_error` | Internal server error |
| `provider_error` | Upstream provider error |

### HTTP Status Codes

| Status | Description |
|--------|-------------|
| `200` | Success |
| `400` | Bad Request - Invalid parameters |
| `401` | Unauthorized - Authentication required |
| `403` | Forbidden - Insufficient permissions |
| `404` | Not Found - Resource not found |
| `429` | Too Many Requests - Rate limit exceeded |
| `500` | Internal Server Error |
| `502` | Bad Gateway - Provider error |
| `503` | Service Unavailable - Gateway unhealthy |
| `504` | Gateway Timeout - Request timed out |

---

## Rate Limiting

### Rate Limit Headers

Rate limit information is included in response headers:

```
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 950
X-RateLimit-Reset: 1698959800
X-RateLimit-Limit-Tokens: 100000
X-RateLimit-Remaining-Tokens: 95000
```

### Rate Limit Response

When rate limited, the API returns:

```json
{
  "error": {
    "type": "rate_limit_error",
    "message": "Rate limit exceeded. Please retry after 60 seconds.",
    "code": "rate_limit_exceeded"
  }
}
```

---

## Caching

### Cache Control

Control caching behavior with request headers:

```bash
# Bypass cache and force fresh response
curl -H "Cache-Control: no-cache" ...

# Use cached response if available
curl -H "Cache-Control: max-age=3600" ...
```

### Cache Headers

Response includes cache information:

```
X-Cache: HIT
X-Cache-TTL: 3540
```

---

## Request ID

Every request is assigned a unique ID for tracing:

```
X-Request-ID: req_abc123def456
```

Include this ID when reporting issues.

---

## Examples

### Python

```python
import openai

client = openai.OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="your-api-key"
)

response = client.chat.completions.create(
    model="gpt-4o-mini",
    messages=[
        {"role": "user", "content": "Hello!"}
    ]
)

print(response.choices[0].message.content)
```

### JavaScript/Node.js

```javascript
import OpenAI from 'openai';

const client = new OpenAI({
  baseURL: 'http://localhost:8080/v1',
  apiKey: 'your-api-key',
});

const response = await client.chat.completions.create({
  model: 'gpt-4o-mini',
  messages: [{ role: 'user', content: 'Hello!' }],
});

console.log(response.choices[0].message.content);
```

### cURL

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

### Streaming with cURL

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [{"role": "user", "content": "Tell me a story"}],
    "stream": true
  }' --no-buffer
```

---

## SDK Compatibility

The gateway is compatible with official OpenAI SDKs:

| SDK | Version | Compatibility |
|-----|---------|---------------|
| openai-python | 1.0+ | Full |
| openai-node | 4.0+ | Full |
| openai-go | 1.0+ | Full |
| langchain | 0.1+ | Full |
| llama-index | 0.9+ | Full |

Simply change the `base_url` to point to your gateway instance.
