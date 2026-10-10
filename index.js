'use strict';

const crypto = require('crypto');

const SERVICE_NAME = 'inference-gateway-agents';
const HEALTH_AGENTS = ['route'];
const GATEWAY_INTERNAL_URL = process.env.GATEWAY_INTERNAL_URL || 'http://localhost:8080';
const DEFAULT_GATEWAY_TIMEOUT_MS = 30_000;
const DEFAULT_GATEWAY_MAX_RESPONSE_BYTES = 1_048_576;

function readGatewayTimeout() {
  const value = Number.parseInt(process.env.GATEWAY_REQUEST_TIMEOUT_MS || '', 10);
  return Number.isInteger(value) && value >= 100 && value <= 120_000
    ? value
    : DEFAULT_GATEWAY_TIMEOUT_MS;
}

function readGatewayMaxResponseBytes() {
  const value = Number.parseInt(process.env.GATEWAY_MAX_RESPONSE_BYTES || '', 10);
  return Number.isInteger(value) && value >= 1_024 && value <= 10_485_760
    ? value
    : DEFAULT_GATEWAY_MAX_RESPONSE_BYTES;
}

async function readBoundedResponse(resp) {
  const limit = readGatewayMaxResponseBytes();
  const declaredLength = Number.parseInt(resp.headers.get('content-length') || '', 10);
  if (Number.isFinite(declaredLength) && declaredLength > limit) {
    throw new Error('gateway response exceeds configured limit');
  }

  if (!resp.body) return '';

  const reader = resp.body.getReader();
  const decoder = new TextDecoder();
  let bytesRead = 0;
  let responseText = '';

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    bytesRead += value.byteLength;
    if (bytesRead > limit) {
      await reader.cancel();
      throw new Error('gateway response exceeds configured limit');
    }
    responseText += decoder.decode(value, { stream: true });
  }

  return responseText + decoder.decode();
}

function normalizeTraceId(value) {
  return typeof value === 'string' && /^[A-Za-z0-9._:-]{1,128}$/.test(value)
    ? value
    : crypto.randomUUID();
}

// --- CORS ---

function setCorsHeaders(res) {
  res.set('Access-Control-Allow-Origin', '*');
  res.set('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.set('Access-Control-Allow-Headers', 'Content-Type, Authorization, X-Correlation-Id, X-Parent-Span-Id');
  res.set('Access-Control-Expose-Headers', 'X-Correlation-Id, X-Request-Id, X-Response-Time');
  res.set('Access-Control-Max-Age', '3600');
}

// --- Response envelope ---

function buildExecutionMetadata(traceId, executionId) {
  return {
    trace_id: traceId,
    timestamp: new Date().toISOString(),
    service: SERVICE_NAME,
    execution_id: executionId,
  };
}

function envelop(data, traceId, executionId, layers) {
  return {
    ...data,
    execution_metadata: buildExecutionMetadata(traceId, executionId),
    layers_executed: layers,
  };
}

// --- Gateway proxy ---

async function forwardToGateway(path, method, headers, body) {
  const url = `${GATEWAY_INTERNAL_URL}${path}`;
  const opts = {
    method,
    headers: { 'Content-Type': 'application/json', ...headers },
    signal: AbortSignal.timeout(readGatewayTimeout()),
  };
  if (body && method !== 'GET') {
    opts.body = typeof body === 'string' ? body : JSON.stringify(body);
  }
  const resp = await fetch(url, opts);
  const contentType = resp.headers.get('content-type') || '';
  const responseText = await readBoundedResponse(resp);
  let respBody;

  if (contentType.toLowerCase().includes('json')) {
    try {
      respBody = JSON.parse(responseText);
    } catch {
      respBody = {
        error: {
          type: 'invalid_gateway_response',
          message: 'Inference gateway returned malformed JSON',
        },
      };
    }
  } else {
    respBody = {
      error: {
        type: 'invalid_gateway_response',
        message: 'Inference gateway returned a non-JSON response',
      },
    };
  }
  return { status: resp.status, body: respBody };
}

// --- Route handlers ---

function handleHealth(_req, res, traceId, executionId) {
  const payload = envelop(
    { status: 'healthy', service: SERVICE_NAME, agents: HEALTH_AGENTS },
    traceId,
    executionId,
    [{ layer: 'AGENT_ROUTING', status: 'completed' }],
  );
  res.status(200).json(payload);
}

async function handleRoute(req, res, traceId, executionId) {
  const start = Date.now();

  try {
    const fwdHeaders = { 'X-Correlation-Id': traceId };
    const parentSpan = req.headers['x-parent-span-id'];
    if (parentSpan) fwdHeaders['X-Parent-Span-Id'] = parentSpan;

    const gw = await forwardToGateway('/agents/route', 'POST', fwdHeaders, req.body);
    const elapsed = Date.now() - start;

    const payload = envelop(gw.body, traceId, executionId, [
      { layer: 'AGENT_ROUTING', status: 'completed' },
      { layer: 'INFERENCE_GATEWAY_ROUTE', status: 'completed', duration_ms: elapsed },
    ]);
    res.status(gw.status).json(payload);
  } catch {
    const elapsed = Date.now() - start;
    const payload = envelop(
      { error: { type: 'gateway_error', message: 'Failed to reach inference gateway' } },
      traceId,
      executionId,
      [
        { layer: 'AGENT_ROUTING', status: 'completed' },
        { layer: 'INFERENCE_GATEWAY_ROUTE', status: 'failed', duration_ms: elapsed },
      ],
    );
    res.status(502).json(payload);
  }
}

function handleNotFound(req, res, traceId, executionId) {
  const payload = envelop(
    {
      error: {
        type: 'not_found',
        message: `Route ${req.method} ${req.path} not found`,
        available_routes: [
          { method: 'POST', path: '/v1/inference-gateway/route', description: 'Inference Routing Agent' },
          { method: 'GET', path: '/health', description: 'Health check' },
        ],
      },
    },
    traceId,
    executionId,
    [{ layer: 'AGENT_ROUTING', status: 'completed' }],
  );
  res.status(404).json(payload);
}

// --- Entry point ---

exports.handler = async (req, res) => {
  // CORS preflight
  setCorsHeaders(res);
  if (req.method === 'OPTIONS') return res.status(204).send('');

  // Request context
  const traceId = normalizeTraceId(req.headers['x-correlation-id']);
  const executionId = crypto.randomUUID();
  res.set('X-Correlation-Id', traceId);

  // Routing
  const path = req.path;

  if ((path === '/health' || path === '/healthz') && req.method === 'GET') {
    return handleHealth(req, res, traceId, executionId);
  }

  if (path === '/v1/inference-gateway/route' && req.method === 'POST') {
    return handleRoute(req, res, traceId, executionId);
  }

  return handleNotFound(req, res, traceId, executionId);
};
