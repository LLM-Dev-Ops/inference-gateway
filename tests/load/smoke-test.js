/**
 * LLM Inference Gateway - Smoke Test
 *
 * Quick validation test to ensure the gateway is responding correctly.
 * Run: k6 run smoke-test.js
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { config, getHeaders, buildChatRequest } from './k6-config.js';

export const options = {
  vus: 1,
  duration: '30s',
  thresholds: {
    http_req_failed: ['rate<0.01'],
    http_req_duration: ['p(95)<15000'],
  },
};

export default function () {
  // Test health endpoint
  const healthRes = http.get(`${config.gateway.baseUrl}/health`);
  check(healthRes, {
    'health check returns 200': (r) => r.status === 200,
    'health check is healthy': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.status === 'healthy' || body.status === 'degraded';
      } catch {
        return false;
      }
    },
  });

  // Test models endpoint
  const modelsRes = http.get(`${config.gateway.baseUrl}/v1/models`, {
    headers: getHeaders(),
  });
  check(modelsRes, {
    'models returns 200': (r) => r.status === 200,
    'models has data': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.data && body.data.length > 0;
      } catch {
        return false;
      }
    },
  });

  // Test chat completion
  const chatRes = http.post(
    `${config.gateway.baseUrl}/v1/chat/completions`,
    JSON.stringify(buildChatRequest({ prompt: 'Say hello in one word.' })),
    { headers: getHeaders() }
  );
  check(chatRes, {
    'chat completion returns 200': (r) => r.status === 200,
    'chat completion has choices': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.choices && body.choices.length > 0;
      } catch {
        return false;
      }
    },
    'chat completion has content': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.choices[0].message && body.choices[0].message.content;
      } catch {
        return false;
      }
    },
  });

  sleep(1);
}

export function handleSummary(data) {
  return {
    'smoke-test-summary.json': JSON.stringify(data, null, 2),
    stdout: textSummary(data, { indent: ' ', enableColors: true }),
  };
}

function textSummary(data, opts) {
  const checks = data.root_group.checks;
  const metrics = data.metrics;

  let output = '\n';
  output += '=== Smoke Test Results ===\n\n';

  output += 'Checks:\n';
  for (const check of checks) {
    const passed = check.passes;
    const failed = check.fails;
    const status = failed === 0 ? '✓' : '✗';
    output += `  ${status} ${check.name}: ${passed}/${passed + failed}\n`;
  }

  output += '\nMetrics:\n';
  if (metrics.http_req_duration) {
    const dur = metrics.http_req_duration.values;
    output += `  HTTP Request Duration:\n`;
    output += `    - avg: ${dur.avg.toFixed(2)}ms\n`;
    output += `    - p95: ${dur['p(95)'].toFixed(2)}ms\n`;
    output += `    - p99: ${dur['p(99)'].toFixed(2)}ms\n`;
  }
  if (metrics.http_req_failed) {
    output += `  HTTP Request Failed Rate: ${(metrics.http_req_failed.values.rate * 100).toFixed(2)}%\n`;
  }

  return output;
}
