/**
 * LLM Inference Gateway - Stress Test
 *
 * Tests the gateway under increasing load to identify breaking points.
 * Run: k6 run stress-test.js
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Counter, Rate, Trend } from 'k6/metrics';
import {
  config,
  getHeaders,
  buildChatRequest,
  getRandomPrompt,
  getRandomModel,
} from './k6-config.js';

// Custom metrics
const requestsSuccess = new Counter('llm_requests_success');
const requestsError = new Counter('llm_requests_error');
const successRate = new Rate('llm_success_rate');
const responseTime = new Trend('llm_response_time');

export const options = {
  scenarios: {
    stress: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '2m', target: 20 },   // Ramp up to 20 VUs
        { duration: '3m', target: 20 },   // Stay at 20 VUs
        { duration: '2m', target: 50 },   // Ramp up to 50 VUs
        { duration: '3m', target: 50 },   // Stay at 50 VUs
        { duration: '2m', target: 100 },  // Ramp up to 100 VUs
        { duration: '5m', target: 100 },  // Stay at 100 VUs
        { duration: '2m', target: 150 },  // Ramp up to 150 VUs
        { duration: '3m', target: 150 },  // Stay at 150 VUs
        { duration: '3m', target: 0 },    // Ramp down to 0
      ],
      gracefulStop: '2m',
    },
  },
  thresholds: {
    // Relaxed thresholds for stress test
    http_req_failed: ['rate<0.15'],
    http_req_duration: ['p(95)<30000'],
    llm_success_rate: ['rate>0.85'],
  },
};

export default function () {
  const startTime = Date.now();
  const model = getRandomModel();

  const payload = buildChatRequest({
    model: model,
    prompt: getRandomPrompt(),
    maxTokens: 100,
  });

  const res = http.post(
    `${config.gateway.baseUrl}/v1/chat/completions`,
    JSON.stringify(payload),
    {
      headers: getHeaders(),
      timeout: '120s',
      tags: { model: model },
    }
  );

  const duration = Date.now() - startTime;
  responseTime.add(duration);

  const success = check(res, {
    'status is 200': (r) => r.status === 200,
    'has valid response': (r) => {
      if (r.status !== 200) return false;
      try {
        const body = JSON.parse(r.body);
        return body.choices && body.choices.length > 0;
      } catch {
        return false;
      }
    },
  });

  if (success) {
    requestsSuccess.add(1);
    successRate.add(1);
  } else {
    requestsError.add(1);
    successRate.add(0);

    // Log errors for analysis
    if (res.status === 429) {
      console.log(`Rate limited at VU=${__VU}`);
    } else if (res.status === 503) {
      console.log(`Service unavailable at VU=${__VU}`);
    } else if (res.status >= 500) {
      console.log(`Server error ${res.status} at VU=${__VU}`);
    }
  }

  // Minimal sleep to maximize load
  sleep(0.5);
}

export function handleSummary(data) {
  return {
    'stress-test-summary.json': JSON.stringify(data, null, 2),
    stdout: generateStressReport(data),
  };
}

function generateStressReport(data) {
  const metrics = data.metrics;

  let report = '\n';
  report += '╔══════════════════════════════════════════════════════════════════╗\n';
  report += '║              LLM Gateway Stress Test Report                      ║\n';
  report += '╠══════════════════════════════════════════════════════════════════╣\n';

  // Test summary
  report += '║ Test Summary                                                     ║\n';
  report += `║   Total Duration: 25 minutes                                     ║\n`;
  report += `║   Peak VUs: 150                                                  ║\n`;
  report += '╠══════════════════════════════════════════════════════════════════╣\n';

  // Request metrics
  report += '║ Request Metrics                                                  ║\n';
  if (metrics.http_reqs) {
    const total = metrics.http_reqs.values.count;
    const rps = metrics.http_reqs.values.rate.toFixed(2);
    report += `║   Total Requests: ${total.toString().padEnd(47)}║\n`;
    report += `║   Avg Requests/sec: ${rps.padEnd(45)}║\n`;
  }

  report += '╠══════════════════════════════════════════════════════════════════╣\n';

  // Latency metrics
  report += '║ Latency (Response Time)                                          ║\n';
  if (metrics.http_req_duration) {
    const dur = metrics.http_req_duration.values;
    report += `║   Min: ${dur.min.toFixed(0)}ms${' '.repeat(54 - dur.min.toFixed(0).length)}║\n`;
    report += `║   Avg: ${dur.avg.toFixed(0)}ms${' '.repeat(54 - dur.avg.toFixed(0).length)}║\n`;
    report += `║   P50: ${dur['p(50)'].toFixed(0)}ms${' '.repeat(54 - dur['p(50)'].toFixed(0).length)}║\n`;
    report += `║   P90: ${dur['p(90)'].toFixed(0)}ms${' '.repeat(54 - dur['p(90)'].toFixed(0).length)}║\n`;
    report += `║   P95: ${dur['p(95)'].toFixed(0)}ms${' '.repeat(54 - dur['p(95)'].toFixed(0).length)}║\n`;
    report += `║   P99: ${dur['p(99)'].toFixed(0)}ms${' '.repeat(54 - dur['p(99)'].toFixed(0).length)}║\n`;
    report += `║   Max: ${dur.max.toFixed(0)}ms${' '.repeat(54 - dur.max.toFixed(0).length)}║\n`;
  }

  report += '╠══════════════════════════════════════════════════════════════════╣\n';

  // Error metrics
  report += '║ Reliability                                                      ║\n';
  if (metrics.llm_success_rate) {
    const sr = (metrics.llm_success_rate.values.rate * 100).toFixed(2);
    report += `║   Success Rate: ${sr}%${' '.repeat(48 - sr.length)}║\n`;
  }
  if (metrics.http_req_failed) {
    const failRate = (metrics.http_req_failed.values.rate * 100).toFixed(2);
    report += `║   Error Rate: ${failRate}%${' '.repeat(50 - failRate.length)}║\n`;
  }
  if (metrics.llm_requests_success && metrics.llm_requests_error) {
    const success = metrics.llm_requests_success.values.count;
    const errors = metrics.llm_requests_error.values.count;
    report += `║   Successful: ${success.toString().padEnd(51)}║\n`;
    report += `║   Failed: ${errors.toString().padEnd(55)}║\n`;
  }

  report += '╠══════════════════════════════════════════════════════════════════╣\n';

  // Thresholds
  report += '║ Threshold Results                                                ║\n';
  let allPassed = true;
  for (const [name, metric] of Object.entries(metrics)) {
    if (metric.thresholds) {
      for (const [threshold, result] of Object.entries(metric.thresholds)) {
        const status = result.ok ? '✓' : '✗';
        if (!result.ok) allPassed = false;
        const line = `${status} ${name}: ${threshold}`;
        report += `║   ${line.padEnd(63)}║\n`;
      }
    }
  }

  report += '╠══════════════════════════════════════════════════════════════════╣\n';

  const status = allPassed ? '✓ PASSED' : '✗ FAILED';
  report += `║ Final Result: ${status}${' '.repeat(51 - status.length)}║\n`;

  report += '╚══════════════════════════════════════════════════════════════════╝\n';

  return report;
}
