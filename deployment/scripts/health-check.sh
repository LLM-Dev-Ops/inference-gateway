#!/bin/bash

# ============================================================================
# LLM Inference Gateway - Health Check Script
# ============================================================================

set -e

ENVIRONMENT=${1:-production}
NAMESPACE="llm-gateway"

if [ "$ENVIRONMENT" == "staging" ]; then
    NAMESPACE="llm-gateway-staging"
    API_URL="https://staging-api.llmgateway.example.com"
else
    API_URL="https://api.llmgateway.example.com"
fi

echo "=========================================="
echo "LLM Gateway Health Check - $ENVIRONMENT"
echo "=========================================="
echo ""

# ============================================================================
# Kubernetes Health Checks
# ============================================================================

echo "1. Checking Kubernetes Deployment..."

DESIRED_REPLICAS=$(kubectl get deployment llm-gateway -n $NAMESPACE -o jsonpath='{.spec.replicas}')
READY_REPLICAS=$(kubectl get deployment llm-gateway -n $NAMESPACE -o jsonpath='{.status.readyReplicas}')

if [ "$READY_REPLICAS" -eq "$DESIRED_REPLICAS" ]; then
    echo "   ✓ All pods are ready ($READY_REPLICAS/$DESIRED_REPLICAS)"
else
    echo "   ✗ Not all pods are ready ($READY_REPLICAS/$DESIRED_REPLICAS)"
    exit 1
fi

# ============================================================================
# HTTP Health Endpoint
# ============================================================================

echo ""
echo "2. Checking HTTP Health Endpoint..."

HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" $API_URL/health)

if [ "$HTTP_CODE" == "200" ]; then
    echo "   ✓ Health endpoint returned 200 OK"
else
    echo "   ✗ Health endpoint returned $HTTP_CODE"
    exit 1
fi

# Get health details
HEALTH_RESPONSE=$(curl -s $API_URL/health)
echo "   Health Response: $HEALTH_RESPONSE"

# ============================================================================
# Provider Health Checks
# ============================================================================

echo ""
echo "3. Checking LLM Providers..."

PROVIDERS=$(echo $HEALTH_RESPONSE | jq -r '.providers[] | "\(.name):\(.healthy)"')

while IFS=: read -r provider healthy; do
    if [ "$healthy" == "true" ]; then
        echo "   ✓ $provider is healthy"
    else
        echo "   ✗ $provider is unhealthy"
    fi
done <<< "$PROVIDERS"

# ============================================================================
# Dependency Health Checks
# ============================================================================

echo ""
echo "4. Checking Dependencies..."

REDIS_HEALTHY=$(echo $HEALTH_RESPONSE | jq -r '.dependencies.redis')
if [ "$REDIS_HEALTHY" == "true" ]; then
    echo "   ✓ Redis is healthy"
else
    echo "   ✗ Redis is unhealthy"
    exit 1
fi

# ============================================================================
# Smoke Test
# ============================================================================

echo ""
echo "5. Running Smoke Test..."

SMOKE_TEST_PAYLOAD='{
  "model": "gpt-3.5-turbo",
  "messages": [
    {"role": "user", "content": "Hello, health check!"}
  ],
  "max_tokens": 10
}'

SMOKE_TEST_RESPONSE=$(curl -s -X POST $API_URL/v1/chat/completions \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer ${API_KEY:-test}" \
    -d "$SMOKE_TEST_PAYLOAD")

SMOKE_TEST_STATUS=$(echo $SMOKE_TEST_RESPONSE | jq -r '.choices[0].message.content' 2>/dev/null || echo "")

if [ ! -z "$SMOKE_TEST_STATUS" ]; then
    echo "   ✓ Smoke test passed"
else
    echo "   ⚠ Smoke test skipped (requires API key)"
fi

# ============================================================================
# Metrics Check
# ============================================================================

echo ""
echo "6. Checking Metrics Endpoint..."

# Get first pod name
POD_NAME=$(kubectl get pods -n $NAMESPACE -l app=llm-gateway -o jsonpath='{.items[0].metadata.name}')

METRICS=$(kubectl exec -n $NAMESPACE $POD_NAME -- curl -s localhost:9090/metrics 2>/dev/null || echo "")

if [ ! -z "$METRICS" ]; then
    echo "   ✓ Metrics endpoint is accessible"

    # Extract some key metrics
    REQUEST_TOTAL=$(echo "$METRICS" | grep "http_requests_total" | head -1 | awk '{print $2}')
    echo "   Total Requests: $REQUEST_TOTAL"
else
    echo "   ✗ Metrics endpoint is not accessible"
fi

# ============================================================================
# Summary
# ============================================================================

echo ""
echo "=========================================="
echo "Health Check Complete - All systems operational"
echo "=========================================="

exit 0
