/**
 * LLM Inference Gateway - Streaming Load Test
 *
 * Tests streaming chat completions under load, measuring TTFT and throughput.
 * Run: k6 run streaming-test.js
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Counter, Rate, Trend } from 'k6/metrics';
import {
  config,
  getHeaders,
  buildChatRequest,
  getRandomPrompt,
  parseSSEEvents,
  countTokensFromEvents,
} from './k6-config.js';

// Custom metrics
const ttft = new Trend('llm_time_to_first_token');
const tokensPerSecond = new Trend('llm_tokens_per_second');
const totalTokens = new Counter('llm_total_tokens');
const streamingSuccess = new Rate('llm_streaming_success');
const streamingErrors = new Counter('llm_streaming_errors');

export const options = {
  scenarios: {
    streaming: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '1m', target: 10 },
        { duration: '3m', target: 10 },
        { duration: '1m', target: 25 },
        { duration: '3m', target: 25 },
        { duration: '1m', target: 50 },
        { duration: '3m', target: 50 },
        { duration: '2m', target: 0 },
      ],
    },
  },
  thresholds: {
    llm_time_to_first_token: ['p(95)<5000'],  // TTFT < 5s at p95
    llm_streaming_success: ['rate>0.95'],
    http_req_duration: ['p(95)<60000'],  // Allow longer for streaming
  },
};

export default function () {
  const payload = buildChatRequest({
    prompt: getRandomPrompt() + ' Please provide a detailed explanation.',
    maxTokens: 200,
    stream: true,
  });

  const startTime = Date.now();
  let firstTokenTime = null;
  let tokenCount = 0;

  // For k6, we need to make a regular request and parse the streamed response
  // Note: k6 doesn't natively support true streaming, so we measure total response
  const res = http.post(
    `${config.gateway.baseUrl}/v1/chat/completions`,
    JSON.stringify(payload),
    {
      headers: {
        ...getHeaders(),
        'Accept': 'text/event-stream',
      },
      timeout: '120s',
      responseType: 'text',
    }
  );

  const endTime = Date.now();

  const success = check(res, {
    'status is 200': (r) => r.status === 200,
    'is SSE response': (r) => {
      return r.headers['Content-Type']?.includes('text/event-stream') ||
             (r.body && r.body.includes('data: '));
    },
  });

  if (success && res.body) {
    streamingSuccess.add(1);

    // Parse SSE events
    const events = parseSSEEvents(res.body);

    if (events.length > 0) {
      // Estimate TTFT (time to receive first data)
      // In real streaming, this would be measured differently
      // Here we approximate based on response characteristics
      const estimatedTTFT = res.timings.waiting;
      ttft.add(estimatedTTFT);

      // Count tokens
      tokenCount = countTokensFromEvents(events);
      totalTokens.add(tokenCount);

      // Calculate tokens per second
      const totalTime = (endTime - startTime) / 1000;  // seconds
      if (totalTime > 0 && tokenCount > 0) {
        const tps = tokenCount / totalTime;
        tokensPerSecond.add(tps);
      }
    }
  } else {
    streamingSuccess.add(0);
    streamingErrors.add(1);

    if (res.status >= 400) {
      console.log(`Streaming error: ${res.status}`);
    }
  }

  // Wait between requests
  sleep(Math.random() * 2 + 1);
}

export function handleSummary(data) {
  return {
    'streaming-test-summary.json': JSON.stringify(data, null, 2),
    stdout: generateStreamingReport(data),
  };
}

function generateStreamingReport(data) {
  const metrics = data.metrics;

  let report = '\n';
  report += '╔═════════════════════════════════════════════════════════════════╗\n';
  report += '║            LLM Gateway Streaming Test Report                    ║\n';
  report += '╠═════════════════════════════════════════════════════════════════╣\n';

  // Test configuration
  report += '║ Test Configuration                                              ║\n';
  report += `║   Duration: ~14 minutes                                         ║\n`;
  report += `║   Peak VUs: 50                                                  ║\n`;
  report += `║   Mode: Streaming (SSE)                                         ║\n`;
  report += '╠═════════════════════════════════════════════════════════════════╣\n';

  // Streaming metrics
  report += '║ Streaming Metrics                                               ║\n';
  if (metrics.llm_time_to_first_token) {
    const ttftValues = metrics.llm_time_to_first_token.values;
    report += `║   Time to First Token (TTFT):                                   ║\n`;
    report += `║     Avg: ${ttftValues.avg.toFixed(0)}ms${' '.repeat(51 - ttftValues.avg.toFixed(0).length)}║\n`;
    report += `║     P50: ${ttftValues['p(50)'].toFixed(0)}ms${' '.repeat(51 - ttftValues['p(50)'].toFixed(0).length)}║\n`;
    report += `║     P95: ${ttftValues['p(95)'].toFixed(0)}ms${' '.repeat(51 - ttftValues['p(95)'].toFixed(0).length)}║\n`;
    report += `║     P99: ${ttftValues['p(99)'].toFixed(0)}ms${' '.repeat(51 - ttftValues['p(99)'].toFixed(0).length)}║\n`;
  }

  if (metrics.llm_tokens_per_second) {
    const tps = metrics.llm_tokens_per_second.values;
    report += `║   Token Generation Rate:                                        ║\n`;
    report += `║     Avg: ${tps.avg.toFixed(1)} tokens/sec${' '.repeat(43 - tps.avg.toFixed(1).length)}║\n`;
    report += `║     Max: ${tps.max.toFixed(1)} tokens/sec${' '.repeat(43 - tps.max.toFixed(1).length)}║\n`;
  }

  if (metrics.llm_total_tokens) {
    const total = metrics.llm_total_tokens.values.count;
    report += `║   Total Tokens Generated: ${total.toString().padEnd(35)}║\n`;
  }

  report += '╠═════════════════════════════════════════════════════════════════╣\n';

  // Reliability
  report += '║ Reliability                                                     ║\n';
  if (metrics.llm_streaming_success) {
    const sr = (metrics.llm_streaming_success.values.rate * 100).toFixed(2);
    report += `║   Streaming Success Rate: ${sr}%${' '.repeat(35 - sr.length)}║\n`;
  }
  if (metrics.llm_streaming_errors) {
    const errors = metrics.llm_streaming_errors.values.count;
    report += `║   Streaming Errors: ${errors.toString().padEnd(41)}║\n`;
  }

  report += '╠═════════════════════════════════════════════════════════════════╣\n';

  // Response time
  report += '║ Total Response Time                                             ║\n';
  if (metrics.http_req_duration) {
    const dur = metrics.http_req_duration.values;
    report += `║   Avg: ${dur.avg.toFixed(0)}ms${' '.repeat(53 - dur.avg.toFixed(0).length)}║\n`;
    report += `║   P95: ${dur['p(95)'].toFixed(0)}ms${' '.repeat(53 - dur['p(95)'].toFixed(0).length)}║\n`;
  }

  report += '╠═════════════════════════════════════════════════════════════════╣\n';

  // Pass/Fail
  const allPassed = Object.values(data.metrics)
    .filter((m) => m.thresholds)
    .every((m) => Object.values(m.thresholds).every((t) => t.ok));

  const status = allPassed ? '✓ PASSED' : '✗ FAILED';
  report += `║ Result: ${status}${' '.repeat(52 - status.length)}║\n`;

  report += '╚═════════════════════════════════════════════════════════════════╝\n';

  return report;
}
