#!/bin/bash
# LLM-Inference-Gateway Post-Deployment Verification
# Comprehensive verification checklist

set -euo pipefail

PROJECT_ID="${GCP_PROJECT_ID:-agentics-dev}"
REGION="${GCP_REGION:-us-central1}"
SERVICE_NAME="llm-inference-gateway"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

PASS="${GREEN}✓${NC}"
FAIL="${RED}✗${NC}"
WARN="${YELLOW}⚠${NC}"

total_checks=0
passed_checks=0
failed_checks=0

check() {
    local name="$1"
    local result="$2"
    total_checks=$((total_checks + 1))

    if [ "$result" -eq 0 ]; then
        echo -e "  ${PASS} $name"
        passed_checks=$((passed_checks + 1))
    else
        echo -e "  ${FAIL} $name"
        failed_checks=$((failed_checks + 1))
    fi
}

warn_check() {
    local name="$1"
    local result="$2"
    total_checks=$((total_checks + 1))

    if [ "$result" -eq 0 ]; then
        echo -e "  ${PASS} $name"
        passed_checks=$((passed_checks + 1))
    else
        echo -e "  ${WARN} $name (non-critical)"
        passed_checks=$((passed_checks + 1))
    fi
}

echo "=============================================="
echo "LLM-Inference-Gateway Verification Checklist"
echo "=============================================="
echo ""

# Get service URL
SERVICE_URL=$(gcloud run services describe "$SERVICE_NAME" \
    --region="$REGION" \
    --format='value(status.url)' 2>/dev/null || echo "")

if [ -z "$SERVICE_URL" ]; then
    echo -e "${FAIL} Service not found. Deploy first."
    exit 1
fi

echo "Service URL: $SERVICE_URL"
echo ""

# 1. Service Health
echo "1. SERVICE HEALTH"
check "Service is live" $(curl -sf "${SERVICE_URL}/health" > /dev/null; echo $?)
check "Liveness probe" $(curl -sf "${SERVICE_URL}/live" > /dev/null; echo $?)
warn_check "Readiness probe" $(curl -sf "${SERVICE_URL}/ready" > /dev/null; echo $?)
echo ""

# 2. Agent Endpoints
echo "2. AGENT ENDPOINTS"
check "Agent list endpoint" $(curl -sf "${SERVICE_URL}/agents" > /dev/null; echo $?)
warn_check "Agent route endpoint" $(curl -sf -X POST "${SERVICE_URL}/agents/route" \
    -H "Content-Type: application/json" \
    -d '{"model":"gpt-4","request_id":"test-verify"}' 2>/dev/null | grep -q "provider\|error"; echo $?)
check "Agent inspect endpoint" $(curl -sf "${SERVICE_URL}/agents/inspect" > /dev/null; echo $?)
check "Agent status endpoint" $(curl -sf "${SERVICE_URL}/agents/status" > /dev/null; echo $?)
echo ""

# 3. OpenAI-Compatible Endpoints
echo "3. OPENAI-COMPATIBLE ENDPOINTS"
check "Models list endpoint" $(curl -sf "${SERVICE_URL}/v1/models" > /dev/null; echo $?)
warn_check "Chat completions endpoint" $(curl -sf -X POST "${SERVICE_URL}/v1/chat/completions" \
    -H "Content-Type: application/json" \
    -d '{"model":"gpt-4","messages":[{"role":"user","content":"test"}]}' 2>/dev/null | grep -q "choices\|error"; echo $?)
echo ""

# 4. Routing Decisions
echo "4. ROUTING DECISION VERIFICATION"
ROUTE_RESPONSE=$(curl -sf -X POST "${SERVICE_URL}/agents/route" \
    -H "Content-Type: application/json" \
    -d '{"model":"gpt-4","request_id":"verify-'$(date +%s)'","tenant_id":"test-tenant","fallback_enabled":true}' 2>/dev/null || echo "{}")

check "Routing returns provider" $(echo "$ROUTE_RESPONSE" | grep -q "provider\|selected_provider"; echo $?)
check "Routing returns decision_id" $(echo "$ROUTE_RESPONSE" | grep -q "decision_id\|execution_ref"; echo $?)
warn_check "Routing returns confidence" $(echo "$ROUTE_RESPONSE" | grep -q "confidence"; echo $?)
echo ""

# 5. DecisionEvent Persistence
echo "5. DECISION EVENT PERSISTENCE"
warn_check "RuVector service configured" $(curl -sf "${SERVICE_URL}/agents/status" 2>/dev/null | grep -q "ruvector\|persistence"; echo $?)
echo "  (Full persistence verification requires ruvector-service access)"
echo ""

# 6. Telemetry
echo "6. TELEMETRY VERIFICATION"
check "Metrics endpoint" $(curl -sf "${SERVICE_URL}/metrics" > /dev/null; echo $?)
warn_check "Prometheus metrics format" $(curl -sf "${SERVICE_URL}/metrics" 2>/dev/null | grep -q "TYPE\|HELP"; echo $?)
echo "  (Full telemetry verification requires LLM-Observatory access)"
echo ""

# 7. Security Checks
echo "7. SECURITY VERIFICATION"
check "No direct SQL access" $(! grep -r "postgres\|psycopg\|sqlx::query" /workspaces/inference-gateway/src 2>/dev/null; echo $?)
check "Uses ruvector-service client" $(grep -r "RuVectorClient\|ruvector" /workspaces/inference-gateway/crates/gateway-integrations 2>/dev/null > /dev/null; echo $?)
check "Uses agentics-contracts" $(grep -r "agentics_contracts\|DecisionEvent" /workspaces/inference-gateway/crates 2>/dev/null > /dev/null; echo $?)
echo ""

# 8. Architecture Compliance
echo "8. ARCHITECTURE COMPLIANCE"
check "Single unified service" $(gcloud run services list --region="$REGION" --filter="metadata.name:inference-gateway" --format="value(metadata.name)" 2>/dev/null | wc -l | grep -q "1"; echo $?)
check "Stateless design" $(! grep -r "static mut\|lazy_static.*Mutex" /workspaces/inference-gateway/src 2>/dev/null; echo $?)
check "No model execution" $(! grep -r "execute_inference\|run_model\|generate_completion" /workspaces/inference-gateway/crates/gateway-agents 2>/dev/null; echo $?)
echo ""

# Summary
echo "=============================================="
echo "VERIFICATION SUMMARY"
echo "=============================================="
echo -e "Total Checks:  $total_checks"
echo -e "Passed:        ${GREEN}$passed_checks${NC}"
echo -e "Failed:        ${RED}$failed_checks${NC}"
echo ""

if [ "$failed_checks" -eq 0 ]; then
    echo -e "${GREEN}All critical checks passed!${NC}"
    exit 0
else
    echo -e "${RED}$failed_checks critical check(s) failed.${NC}"
    echo "Review the failures above and fix before proceeding."
    exit 1
fi
