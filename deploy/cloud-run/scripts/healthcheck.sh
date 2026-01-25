#!/bin/bash
# Phase 7 Health Check Script
# Used by Docker HEALTHCHECK to verify service health.
#
# Checks performed:
#   1. Gateway /health endpoint responds with HTTP 200
#   2. RuVector connectivity (when RUVECTOR_REQUIRED=true)
#
# Exit codes:
#   0 - Healthy
#   1 - Unhealthy

set -uo pipefail

# =============================================================================
# Configuration
# =============================================================================

GATEWAY_HOST="${GATEWAY_HOST:-localhost}"
GATEWAY_PORT="${GATEWAY_PORT:-8080}"
HEALTH_TIMEOUT="${HEALTH_TIMEOUT:-5}"
RUVECTOR_TIMEOUT="${RUVECTOR_TIMEOUT_MS:-5000}"

# =============================================================================
# Health Check Functions
# =============================================================================

check_gateway_health() {
    local url="http://${GATEWAY_HOST}:${GATEWAY_PORT}/health"
    local response

    response=$(curl -s -o /dev/null -w "%{http_code}" \
        --max-time "${HEALTH_TIMEOUT}" \
        "${url}" 2>/dev/null || echo "000")

    if [[ "${response}" == "200" ]]; then
        return 0
    fi

    echo "Gateway health check failed: HTTP ${response}"
    return 1
}

check_ruvector_health() {
    local url="${RUVECTOR_SERVICE_URL}"
    local timeout_sec=$((RUVECTOR_TIMEOUT / 1000))

    if [[ -z "${url}" ]]; then
        echo "RuVector URL not configured"
        return 1
    fi

    local response
    response=$(curl -s -o /dev/null -w "%{http_code}" \
        --max-time "${timeout_sec}" \
        -H "Authorization: Bearer ${RUVECTOR_API_KEY:-}" \
        "${url}/health" 2>/dev/null || echo "000")

    # Accept 200, 401, 403 as "reachable" (auth issues are not connectivity issues)
    if [[ "${response}" == "200" || "${response}" == "401" || "${response}" == "403" ]]; then
        return 0
    fi

    echo "RuVector health check failed: HTTP ${response}"
    return 1
}

# =============================================================================
# Main
# =============================================================================

main() {
    local healthy=true

    # Check 1: Gateway health
    if ! check_gateway_health; then
        healthy=false
    fi

    # Check 2: RuVector connectivity (if required)
    if [[ "${RUVECTOR_REQUIRED:-false}" == "true" ]]; then
        if ! check_ruvector_health; then
            healthy=false
        fi
    fi

    if [[ "${healthy}" == "true" ]]; then
        exit 0
    else
        exit 1
    fi
}

main "$@"
