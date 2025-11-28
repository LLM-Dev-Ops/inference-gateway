/**
 * LLM Inference Gateway - Load Testing Configuration
 *
 * This file contains shared configuration and utilities for k6 load tests.
 */

// Environment configuration
export const config = {
  // Gateway settings
  gateway: {
    baseUrl: __ENV.GATEWAY_URL || 'http://localhost:8080',
    apiKey: __ENV.API_KEY || 'test-api-key',
  },

  // Test defaults
  defaults: {
    model: __ENV.MODEL || 'gpt-3.5-turbo',
    maxTokens: parseInt(__ENV.MAX_TOKENS) || 100,
    temperature: parseFloat(__ENV.TEMPERATURE) || 0.7,
  },

  // Thresholds for pass/fail
  thresholds: {
    http_req_duration: ['p(95)<10000', 'p(99)<30000'], // 10s p95, 30s p99
    http_req_failed: ['rate<0.05'], // Less than 5% error rate
    http_reqs: ['rate>10'], // At least 10 RPS
  },
};

// Standard request headers
export function getHeaders() {
  return {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${config.gateway.apiKey}`,
    'X-Request-ID': `load-test-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
  };
}

// Build chat completion request payload
export function buildChatRequest(options = {}) {
  return {
    model: options.model || config.defaults.model,
    messages: options.messages || [
      {
        role: 'user',
        content: options.prompt || 'Hello, how are you?',
      },
    ],
    max_tokens: options.maxTokens || config.defaults.maxTokens,
    temperature: options.temperature || config.defaults.temperature,
    stream: options.stream || false,
  };
}

// Sample prompts for realistic testing
export const samplePrompts = [
  'What is the capital of France?',
  'Explain quantum computing in simple terms.',
  'Write a haiku about programming.',
  'What are the benefits of exercise?',
  'How does photosynthesis work?',
  'Describe the water cycle.',
  'What is machine learning?',
  'Explain the theory of relativity.',
  'How do airplanes fly?',
  'What causes earthquakes?',
  'Describe the solar system.',
  'How does the internet work?',
  'What is artificial intelligence?',
  'Explain blockchain technology.',
  'How do vaccines work?',
];

// Get a random prompt
export function getRandomPrompt() {
  return samplePrompts[Math.floor(Math.random() * samplePrompts.length)];
}

// Models for multi-model testing
export const testModels = [
  'gpt-3.5-turbo',
  'gpt-4',
  'claude-3-sonnet-20240229',
  'claude-3-haiku-20240307',
];

// Get a random model
export function getRandomModel() {
  return testModels[Math.floor(Math.random() * testModels.length)];
}

// Custom metrics names
export const customMetrics = {
  timeToFirstToken: 'llm_ttft',
  tokensPerSecond: 'llm_tokens_per_second',
  totalTokens: 'llm_total_tokens',
};

// Parse SSE events from streaming response
export function parseSSEEvents(body) {
  const events = [];
  const lines = body.split('\n');

  for (const line of lines) {
    if (line.startsWith('data: ')) {
      const data = line.substring(6);
      if (data === '[DONE]') {
        events.push({ done: true });
      } else {
        try {
          events.push(JSON.parse(data));
        } catch (e) {
          // Ignore parse errors
        }
      }
    }
  }

  return events;
}

// Calculate tokens from events
export function countTokensFromEvents(events) {
  let tokens = 0;
  for (const event of events) {
    if (event.choices && event.choices[0] && event.choices[0].delta) {
      const content = event.choices[0].delta.content;
      if (content) {
        // Rough token estimation (1 token ~= 4 chars)
        tokens += Math.ceil(content.length / 4);
      }
    }
  }
  return tokens;
}
