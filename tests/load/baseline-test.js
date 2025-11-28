/**
 * LLM Inference Gateway - Baseline Load Test
 *
 * Establishes baseline performance metrics under moderate load.
 * Run: k6 run baseline-test.js
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Counter, Rate, Trend } from 'k6/metrics';
import {
  config,
  getHeaders,
  buildChatRequest,
  getRandomPrompt,
} from './k6-config.js';

// Custom metrics
const requestsSuccess = new Counter('llm_requests_success');
const requestsError = new Counter('llm_requests_error');
const tokensTrend = new Trend('llm_tokens_generated');
const ttftTrend = new Trend('llm_time_to_first_response');
const successRate = new Rate('llm_success_rate');

export const options = {
  scenarios: {
    baseline: {
      executor: 'constant-vus',
      vus: 10,
      duration: '5m',
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.05'],
    http_req_duration: ['p(95)<15000', 'p(99)<30000'],
    llm_success_rate: ['rate>0.95'],
  },
};

export default function () {
  const startTime = Date.now();

  const payload = buildChatRequest({
    prompt: getRandomPrompt(),
    maxTokens: 150,
  });

  const res = http.post(
    `${config.gateway.baseUrl}/v1/chat/completions`,
    JSON.stringify(payload),
    {
      headers: getHeaders(),
      timeout: '60s',
    }
  );

  const duration = Date.now() - startTime;

  const success = check(res, {
    'status is 200': (r) => r.status === 200,
    'response has choices': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.choices && body.choices.length > 0;
      } catch {
        return false;
      }
    },
    'response has content': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.choices[0].message && body.choices[0].message.content;
      } catch {
        return false;
      }
    },
    'response time < 15s': (r) => r.timings.duration < 15000,
  });

  if (success) {
    requestsSuccess.add(1);
    successRate.add(1);

    try {
      const body = JSON.parse(res.body);
      if (body.usage && body.usage.completion_tokens) {
        tokensTrend.add(body.usage.completion_tokens);
      }
    } catch {
      // Ignore parse errors
    }

    // Record TTFT (approximated as time to receive full response for non-streaming)
    ttftTrend.add(res.timings.waiting);
  } else {
    requestsError.add(1);
    successRate.add(0);

    if (res.status >= 400) {
      console.log(`Error: ${res.status} - ${res.body}`);
    }
  }

  // Random sleep between 1-3 seconds to simulate realistic usage
  sleep(Math.random() * 2 + 1);
}

export function handleSummary(data) {
  return {
    'baseline-test-summary.json': JSON.stringify(data, null, 2),
    stdout: generateReport(data),
  };
}

function generateReport(data) {
  const metrics = data.metrics;

  let report = '\n';
  report += '╔══════════════════════════════════════════════════════════════╗\n';
  report += '║           LLM Gateway Baseline Load Test Report              ║\n';
  report += '╠══════════════════════════════════════════════════════════════╣\n';

  // Test configuration
  report += '║ Test Configuration                                           ║\n';
  report += `║   Duration: 5 minutes                                        ║\n`;
  report += `║   Virtual Users: 10                                          ║\n`;
  report += '╠══════════════════════════════════════════════════════════════╣\n';

  // HTTP metrics
  report += '║ HTTP Metrics                                                 ║\n';
  if (metrics.http_reqs) {
    const rps = metrics.http_reqs.values.rate.toFixed(2);
    report += `║   Requests/sec: ${rps.padEnd(45)}║\n`;
  }
  if (metrics.http_req_duration) {
    const dur = metrics.http_req_duration.values;
    report += `║   Avg Response Time: ${dur.avg.toFixed(0)}ms${' '.repeat(36 - dur.avg.toFixed(0).length)}║\n`;
    report += `║   P50 Response Time: ${dur['p(50)'].toFixed(0)}ms${' '.repeat(36 - dur['p(50)'].toFixed(0).length)}║\n`;
    report += `║   P95 Response Time: ${dur['p(95)'].toFixed(0)}ms${' '.repeat(36 - dur['p(95)'].toFixed(0).length)}║\n`;
    report += `║   P99 Response Time: ${dur['p(99)'].toFixed(0)}ms${' '.repeat(36 - dur['p(99)'].toFixed(0).length)}║\n`;
  }
  if (metrics.http_req_failed) {
    const failRate = (metrics.http_req_failed.values.rate * 100).toFixed(2);
    report += `║   Error Rate: ${failRate}%${' '.repeat(43 - failRate.length)}║\n`;
  }

  report += '╠══════════════════════════════════════════════════════════════╣\n';

  // LLM specific metrics
  report += '║ LLM Metrics                                                  ║\n';
  if (metrics.llm_success_rate) {
    const sr = (metrics.llm_success_rate.values.rate * 100).toFixed(2);
    report += `║   Success Rate: ${sr}%${' '.repeat(42 - sr.length)}║\n`;
  }
  if (metrics.llm_tokens_generated) {
    const avg = metrics.llm_tokens_generated.values.avg.toFixed(0);
    report += `║   Avg Tokens/Request: ${avg}${' '.repeat(36 - avg.length)}║\n`;
  }
  if (metrics.llm_time_to_first_response) {
    const ttft = metrics.llm_time_to_first_response.values.avg.toFixed(0);
    report += `║   Avg TTFT: ${ttft}ms${' '.repeat(43 - ttft.length)}║\n`;
  }

  report += '╠══════════════════════════════════════════════════════════════╣\n';

  // Pass/Fail
  const allPassed = Object.values(data.metrics)
    .filter((m) => m.thresholds)
    .every((m) => Object.values(m.thresholds).every((t) => t.ok));

  const status = allPassed ? '✓ PASSED' : '✗ FAILED';
  report += `║ Result: ${status}${' '.repeat(51 - status.length)}║\n`;

  report += '╚══════════════════════════════════════════════════════════════╝\n';

  return report;
}
