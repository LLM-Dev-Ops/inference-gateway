#!/bin/bash

# ============================================================================
# LLM Inference Gateway - Smoke Tests
# ============================================================================

set -e

ENVIRONMENT=${1:-staging}

if [ "$ENVIRONMENT" == "staging" ]; then
    API_URL="https://staging-api.llmgateway.example.com"
elif [ "$ENVIRONMENT" == "production" ]; then
    API_URL="https://api.llmgateway.example.com"
else
    API_URL="http://localhost:8080"
fi

echo "=========================================="
echo "LLM Gateway Smoke Tests - $ENVIRONMENT"
echo "=========================================="
echo ""

FAILED_TESTS=0
TOTAL_TESTS=0

# ============================================================================
# Helper Functions
# ============================================================================

test_passed() {
    echo "   ✓ $1"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
}

test_failed() {
    echo "   ✗ $1"
    FAILED_TESTS=$((FAILED_TESTS + 1))
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
}

# ============================================================================
# Test 1: Health Endpoint
# ============================================================================

echo "Test 1: Health Endpoint"
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" $API_URL/health)

if [ "$HTTP_CODE" == "200" ]; then
    test_passed "Health endpoint returns 200"
else
    test_failed "Health endpoint returned $HTTP_CODE"
fi

# ============================================================================
# Test 2: Metrics Endpoint (internal)
# ============================================================================

echo ""
echo "Test 2: Metrics Endpoint"

# This test only works for local/internal access
if [ "$ENVIRONMENT" == "local" ]; then
    METRICS=$(curl -s http://localhost:9090/metrics)
    if echo "$METRICS" | grep -q "http_requests_total"; then
        test_passed "Metrics endpoint is accessible"
    else
        test_failed "Metrics endpoint not accessible"
    fi
else
    echo "   ⊘ Skipped (internal only)"
fi

# ============================================================================
# Test 3: Chat Completion (Basic)
# ============================================================================

echo ""
echo "Test 3: Basic Chat Completion"

if [ -z "$API_KEY" ]; then
    echo "   ⊘ Skipped (requires API_KEY environment variable)"
else
    RESPONSE=$(curl -s -X POST $API_URL/v1/chat/completions \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $API_KEY" \
        -d '{
          "model": "gpt-3.5-turbo",
          "messages": [{"role": "user", "content": "Say hello"}],
          "max_tokens": 10
        }')

    if echo "$RESPONSE" | jq -e '.choices[0].message.content' > /dev/null 2>&1; then
        test_passed "Basic chat completion works"
    else
        test_failed "Chat completion failed: $RESPONSE"
    fi
fi

# ============================================================================
# Test 4: Error Handling (Invalid Request)
# ============================================================================

echo ""
echo "Test 4: Error Handling"

ERROR_RESPONSE=$(curl -s -X POST $API_URL/v1/chat/completions \
    -H "Content-Type: application/json" \
    -d '{"invalid": "request"}')

if echo "$ERROR_RESPONSE" | grep -q "error"; then
    test_passed "Error handling works correctly"
else
    test_failed "Error handling incorrect: $ERROR_RESPONSE"
fi

# ============================================================================
# Test 5: CORS Headers
# ============================================================================

echo ""
echo "Test 5: CORS Headers"

CORS_RESPONSE=$(curl -s -I -X OPTIONS $API_URL/v1/chat/completions \
    -H "Origin: https://example.com" \
    -H "Access-Control-Request-Method: POST")

if echo "$CORS_RESPONSE" | grep -q "Access-Control-Allow-Origin"; then
    test_passed "CORS headers present"
else
    test_failed "CORS headers missing"
fi

# ============================================================================
# Test 6: Rate Limiting
# ============================================================================

echo ""
echo "Test 6: Rate Limiting"

RATE_LIMIT_COUNT=0
for i in {1..10}; do
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" $API_URL/health)
    if [ "$HTTP_CODE" == "429" ]; then
        RATE_LIMIT_COUNT=$((RATE_LIMIT_COUNT + 1))
    fi
done

if [ $RATE_LIMIT_COUNT -gt 0 ]; then
    echo "   ⚠ Rate limiting triggered (expected for high load)"
else
    test_passed "No rate limiting at low request volume"
fi

# ============================================================================
# Test 7: Latency Check
# ============================================================================

echo ""
echo "Test 7: Latency Check"

LATENCY=$(curl -s -w "%{time_total}" -o /dev/null $API_URL/health)
LATENCY_MS=$(echo "$LATENCY * 1000" | bc | cut -d. -f1)

if [ $LATENCY_MS -lt 1000 ]; then
    test_passed "Health endpoint latency: ${LATENCY_MS}ms"
else
    test_failed "Health endpoint latency too high: ${LATENCY_MS}ms"
fi

# ============================================================================
# Summary
# ============================================================================

echo ""
echo "=========================================="
echo "Smoke Test Summary"
echo "=========================================="
echo "Total Tests: $TOTAL_TESTS"
echo "Passed: $((TOTAL_TESTS - FAILED_TESTS))"
echo "Failed: $FAILED_TESTS"
echo "=========================================="

if [ $FAILED_TESTS -gt 0 ]; then
    echo "SMOKE TESTS FAILED"
    exit 1
else
    echo "ALL SMOKE TESTS PASSED"
    exit 0
fi
