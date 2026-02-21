'use strict';

const { describe, it, beforeEach } = require('node:test');
const assert = require('node:assert/strict');
const { handler } = require('../index');

// --- Helpers ---

function mockReq({ method = 'GET', path = '/', headers = {}, body = null } = {}) {
  return { method, path, headers, body };
}

function mockRes() {
  const res = {
    _status: null,
    _json: null,
    _headers: {},
    _sent: null,
    status(code) { res._status = code; return res; },
    json(data) { res._json = data; return res; },
    send(data) { res._sent = data; return res; },
    set(key, val) { res._headers[key] = val; return res; },
  };
  return res;
}

// --- Tests ---

describe('handler', () => {
  describe('CORS', () => {
    it('responds 204 to OPTIONS preflight', async () => {
      const res = mockRes();
      await handler(mockReq({ method: 'OPTIONS' }), res);
      assert.equal(res._status, 204);
      assert.equal(res._headers['Access-Control-Allow-Origin'], '*');
      assert.ok(res._headers['Access-Control-Allow-Methods'].includes('POST'));
      assert.ok(res._headers['Access-Control-Allow-Headers'].includes('X-Correlation-Id'));
    });

    it('sets CORS headers on every response', async () => {
      const res = mockRes();
      await handler(mockReq({ path: '/health' }), res);
      assert.equal(res._headers['Access-Control-Allow-Origin'], '*');
    });
  });

  describe('GET /health', () => {
    it('returns 200 with required envelope fields', async () => {
      const res = mockRes();
      await handler(mockReq({ path: '/health' }), res);

      assert.equal(res._status, 200);
      const body = res._json;

      // Top-level fields
      assert.equal(body.status, 'healthy');
      assert.equal(body.service, 'inference-gateway-agents');
      assert.deepEqual(body.agents, ['route']);

      // execution_metadata
      assert.ok(body.execution_metadata);
      assert.equal(body.execution_metadata.service, 'inference-gateway-agents');
      assert.ok(body.execution_metadata.trace_id);
      assert.ok(body.execution_metadata.execution_id);
      assert.ok(body.execution_metadata.timestamp);

      // layers_executed
      assert.ok(Array.isArray(body.layers_executed));
      assert.equal(body.layers_executed[0].layer, 'AGENT_ROUTING');
      assert.equal(body.layers_executed[0].status, 'completed');
    });

    it('aliases /healthz', async () => {
      const res = mockRes();
      await handler(mockReq({ path: '/healthz' }), res);
      assert.equal(res._status, 200);
      assert.equal(res._json.status, 'healthy');
    });

    it('uses X-Correlation-Id when provided', async () => {
      const res = mockRes();
      await handler(mockReq({ path: '/health', headers: { 'x-correlation-id': 'trace-abc' } }), res);
      assert.equal(res._json.execution_metadata.trace_id, 'trace-abc');
      assert.equal(res._headers['X-Correlation-Id'], 'trace-abc');
    });
  });

  describe('POST /v1/inference-gateway/route', () => {
    it('returns 502 with correct envelope when gateway unreachable', async () => {
      // With default GATEWAY_INTERNAL_URL (localhost:8080), gateway is not running
      const res = mockRes();
      const req = mockReq({
        method: 'POST',
        path: '/v1/inference-gateway/route',
        body: {
          request: { model: 'gpt-4', messages: [{ role: 'user', content: 'hello' }] },
        },
      });
      await handler(req, res);

      assert.equal(res._status, 502);
      const body = res._json;

      // Error body
      assert.equal(body.error.type, 'gateway_error');

      // execution_metadata present
      assert.ok(body.execution_metadata);
      assert.equal(body.execution_metadata.service, 'inference-gateway-agents');

      // layers_executed
      assert.equal(body.layers_executed.length, 2);
      assert.equal(body.layers_executed[0].layer, 'AGENT_ROUTING');
      assert.equal(body.layers_executed[0].status, 'completed');
      assert.equal(body.layers_executed[1].layer, 'INFERENCE_GATEWAY_ROUTE');
      assert.equal(body.layers_executed[1].status, 'failed');
      assert.ok(typeof body.layers_executed[1].duration_ms === 'number');
    });
  });

  describe('404 handling', () => {
    it('returns 404 with envelope for unknown routes', async () => {
      const res = mockRes();
      await handler(mockReq({ path: '/unknown' }), res);

      assert.equal(res._status, 404);
      const body = res._json;
      assert.equal(body.error.type, 'not_found');
      assert.ok(body.execution_metadata);
      assert.ok(Array.isArray(body.layers_executed));
    });
  });

  describe('execution_metadata', () => {
    it('generates unique execution_id per request', async () => {
      const res1 = mockRes();
      const res2 = mockRes();
      await handler(mockReq({ path: '/health' }), res1);
      await handler(mockReq({ path: '/health' }), res2);

      assert.notEqual(
        res1._json.execution_metadata.execution_id,
        res2._json.execution_metadata.execution_id,
      );
    });

    it('auto-generates trace_id when no header', async () => {
      const res = mockRes();
      await handler(mockReq({ path: '/health' }), res);

      const traceId = res._json.execution_metadata.trace_id;
      // UUID v4 format: 8-4-4-4-12
      assert.match(traceId, /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
    });
  });
});
