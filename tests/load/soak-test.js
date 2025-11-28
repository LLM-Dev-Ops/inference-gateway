/**
 * LLM Inference Gateway - Soak Test
 *
 * Long-running test to identify memory leaks, connection issues, and degradation over time.
 * Run: k6 run soak-test.js
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
const successRate = new Rate('llm_success_rate');
const hourlyTrend = new Trend('llm_hourly_latency');

export const options = {
  scenarios: {
    soak: {
      executor: 'constant-vus',
      vus: 20,
      duration: '2h',  // 2 hour soak test
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.02'],
    http_req_duration: ['p(95)<15000'],
    llm_success_rate: ['rate>0.98'],
  },
};

// Track metrics per time window
let windowStartTime = Date.now();
let windowRequests = 0;
let windowErrors = 0;
let windowLatencySum = 0;
const WINDOW_SIZE = 300000; // 5 minutes

export default function () {
  const startTime = Date.now();

  const payload = buildChatRequest({
    prompt: getRandomPrompt(),
    maxTokens: 100,
  });

  const res = http.post(
    `${config.gateway.baseUrl}/v1/chat/completions`,
    JSON.stringify(payload),
    {
      headers: getHeaders(),
      timeout: '60s',
    }
  );

  const latency = Date.now() - startTime;

  const success = check(res, {
    'status is 200': (r) => r.status === 200,
    'valid response': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.choices && body.choices.length > 0;
      } catch {
        return false;
      }
    },
  });

  // Track window metrics
  windowRequests++;
  windowLatencySum += latency;

  if (success) {
    requestsSuccess.add(1);
    successRate.add(1);
  } else {
    requestsError.add(1);
    successRate.add(0);
    windowErrors++;
  }

  // Log window stats every 5 minutes
  const now = Date.now();
  if (now - windowStartTime >= WINDOW_SIZE) {
    const windowAvgLatency = windowLatencySum / windowRequests;
    const windowErrorRate = (windowErrors / windowRequests) * 100;
    const elapsed = Math.floor((now - __ENV.TEST_START) / 60000);

    console.log(
      `[${elapsed}m] Window Stats: ${windowRequests} reqs, ` +
      `${windowAvgLatency.toFixed(0)}ms avg latency, ` +
      `${windowErrorRate.toFixed(2)}% errors`
    );

    hourlyTrend.add(windowAvgLatency);

    // Reset window
    windowStartTime = now;
    windowRequests = 0;
    windowErrors = 0;
    windowLatencySum = 0;
  }

  // Moderate request rate
  sleep(Math.random() * 2 + 1);
}

export function setup() {
  // Store test start time
  return { startTime: Date.now() };
}

export function handleSummary(data) {
  return {
    'soak-test-summary.json': JSON.stringify(data, null, 2),
    stdout: generateSoakReport(data),
  };
}

function generateSoakReport(data) {
  const metrics = data.metrics;

  let report = '\n';
  report += '╔════════════════════════════════════════════════════════════════════╗\n';
  report += '║                 LLM Gateway Soak Test Report                       ║\n';
  report += '╠════════════════════════════════════════════════════════════════════╣\n';

  // Test summary
  report += '║ Test Configuration                                                 ║\n';
  report += `║   Duration: 2 hours                                                ║\n`;
  report += `║   Constant VUs: 20                                                 ║\n`;
  report += '╠════════════════════════════════════════════════════════════════════╣\n';

  // Volume metrics
  report += '║ Volume                                                             ║\n';
  if (metrics.http_reqs) {
    const total = metrics.http_reqs.values.count;
    const rps = metrics.http_reqs.values.rate.toFixed(2);
    report += `║   Total Requests: ${total.toString().padEnd(49)}║\n`;
    report += `║   Avg Requests/sec: ${rps.padEnd(47)}║\n`;
  }

  report += '╠════════════════════════════════════════════════════════════════════╣\n';

  // Latency stability
  report += '║ Latency Stability                                                  ║\n';
  if (metrics.http_req_duration) {
    const dur = metrics.http_req_duration.values;
    report += `║   Avg: ${dur.avg.toFixed(0)}ms${' '.repeat(58 - dur.avg.toFixed(0).length)}║\n`;
    report += `║   P50: ${dur['p(50)'].toFixed(0)}ms${' '.repeat(58 - dur['p(50)'].toFixed(0).length)}║\n`;
    report += `║   P95: ${dur['p(95)'].toFixed(0)}ms${' '.repeat(58 - dur['p(95)'].toFixed(0).length)}║\n`;
    report += `║   P99: ${dur['p(99)'].toFixed(0)}ms${' '.repeat(58 - dur['p(99)'].toFixed(0).length)}║\n`;
  }

  // Hourly trend (if available)
  if (metrics.llm_hourly_latency) {
    const trend = metrics.llm_hourly_latency.values;
    report += `║   First Window Avg: ~${trend.min.toFixed(0)}ms${' '.repeat(46 - trend.min.toFixed(0).length)}║\n`;
    report += `║   Last Window Avg: ~${trend.max.toFixed(0)}ms${' '.repeat(47 - trend.max.toFixed(0).length)}║\n`;

    // Check for degradation
    const degradation = ((trend.max - trend.min) / trend.min) * 100;
    if (degradation > 20) {
      report += `║   ⚠ Latency increased by ${degradation.toFixed(0)}% over test duration${' '.repeat(25 - degradation.toFixed(0).length)}║\n`;
    }
  }

  report += '╠════════════════════════════════════════════════════════════════════╣\n';

  // Reliability
  report += '║ Reliability                                                        ║\n';
  if (metrics.llm_success_rate) {
    const sr = (metrics.llm_success_rate.values.rate * 100).toFixed(4);
    report += `║   Success Rate: ${sr}%${' '.repeat(50 - sr.length)}║\n`;
  }
  if (metrics.http_req_failed) {
    const failRate = (metrics.http_req_failed.values.rate * 100).toFixed(4);
    report += `║   Error Rate: ${failRate}%${' '.repeat(52 - failRate.length)}║\n`;
  }

  report += '╠════════════════════════════════════════════════════════════════════╣\n';

  // Analysis
  report += '║ Analysis                                                           ║\n';

  // Check for memory leak indicators (latency trend)
  if (metrics.llm_hourly_latency) {
    const trend = metrics.llm_hourly_latency.values;
    const degradation = ((trend.max - trend.min) / trend.min) * 100;

    if (degradation < 10) {
      report += `║   ✓ Stable performance - no significant degradation               ║\n`;
    } else if (degradation < 25) {
      report += `║   ⚠ Minor performance degradation detected                        ║\n`;
    } else {
      report += `║   ✗ Significant degradation - possible memory leak                ║\n`;
    }
  }

  // Check error consistency
  if (metrics.http_req_failed && metrics.http_req_failed.values.rate < 0.01) {
    report += `║   ✓ Error rate remained consistently low                          ║\n`;
  }

  report += '╠════════════════════════════════════════════════════════════════════╣\n';

  // Pass/Fail
  const allPassed = Object.values(data.metrics)
    .filter((m) => m.thresholds)
    .every((m) => Object.values(m.thresholds).every((t) => t.ok));

  const status = allPassed ? '✓ PASSED' : '✗ FAILED';
  report += `║ Result: ${status}${' '.repeat(57 - status.length)}║\n`;

  report += '╚════════════════════════════════════════════════════════════════════╝\n';

  return report;
}
