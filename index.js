'use strict';

const crypto = require('crypto');

const SERVICE_NAME = 'inference-gateway-agents';
const HEALTH_AGENTS = ['route'];
const GATEWAY_INTERNAL_URL = process.env.GATEWAY_INTERNAL_URL || 'http://localhost:8080';

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
  };
  if (body && method !== 'GET') {
    opts.body = typeof body === 'string' ? body : JSON.stringify(body);
  }
  const resp = await fetch(url, opts);
  const respBody = await resp.json();
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
  } catch (err) {
    const elapsed = Date.now() - start;
    const payload = envelop(
      { error: { type: 'gateway_error', message: err.message || 'Failed to reach inference gateway' } },
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
  const traceId = req.headers['x-correlation-id'] || crypto.randomUUID();
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
