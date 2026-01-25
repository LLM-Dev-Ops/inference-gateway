#!/bin/bash
# Phase 7 Startup Script
# Validates required environment variables and RuVector connectivity
# before starting the inference gateway service.
#
# Exit codes:
#   0 - Success (gateway started)
#   1 - Missing required environment variable
#   2 - RuVector connectivity failure (when RUVECTOR_REQUIRED=true)
#   3 - Gateway binary not found

set -euo pipefail

# =============================================================================
# Configuration
# =============================================================================

GATEWAY_BINARY="/app/llm-inference-gateway"
STARTUP_TIMEOUT="${STARTUP_TIMEOUT:-30}"
RUVECTOR_RETRY_COUNT="${RUVECTOR_RETRY_COUNT:-3}"
RUVECTOR_RETRY_DELAY="${RUVECTOR_RETRY_DELAY_MS:-1000}"

# =============================================================================
# Logging Functions
# =============================================================================

log_info() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [INFO] $*"
}

log_warn() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [WARN] $*" >&2
}

log_error() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [ERROR] $*" >&2
}

# =============================================================================
# Validation Functions
# =============================================================================

validate_required_env() {
    local var_name="$1"
    local var_value="${!var_name:-}"

    if [[ -z "${var_value}" ]]; then
        log_error "Missing required environment variable: ${var_name}"
        return 1
    fi

    log_info "Validated: ${var_name}=<set>"
    return 0
}

validate_ruvector_connectivity() {
    local url="${RUVECTOR_SERVICE_URL}"
    local timeout_ms="${RUVECTOR_TIMEOUT_MS:-5000}"
    local timeout_sec=$((timeout_ms / 1000))

    log_info "Checking RuVector connectivity at ${url}..."

    for i in $(seq 1 "${RUVECTOR_RETRY_COUNT}"); do
        log_info "Attempt ${i}/${RUVECTOR_RETRY_COUNT}..."

        # Try health endpoint first, fallback to root
        local response
        response=$(curl -s -o /dev/null -w "%{http_code}" \
            --max-time "${timeout_sec}" \
            -H "Authorization: Bearer ${RUVECTOR_API_KEY}" \
            "${url}/health" 2>/dev/null || echo "000")

        if [[ "${response}" == "200" ]]; then
            log_info "RuVector connectivity: OK (HTTP 200)"
            return 0
        fi

        # Try root endpoint if health fails
        response=$(curl -s -o /dev/null -w "%{http_code}" \
            --max-time "${timeout_sec}" \
            -H "Authorization: Bearer ${RUVECTOR_API_KEY}" \
            "${url}/" 2>/dev/null || echo "000")

        if [[ "${response}" == "200" || "${response}" == "401" || "${response}" == "403" ]]; then
            # 401/403 means server is reachable, just auth issue (which is expected without full auth)
            log_info "RuVector connectivity: OK (server reachable, HTTP ${response})"
            return 0
        fi

        log_warn "RuVector not reachable (HTTP ${response}), retrying in ${RUVECTOR_RETRY_DELAY}ms..."
        sleep "$(echo "scale=2; ${RUVECTOR_RETRY_DELAY} / 1000" | bc)"
    done

    log_error "RuVector connectivity failed after ${RUVECTOR_RETRY_COUNT} attempts"
    return 1
}

# =============================================================================
# Main Startup Sequence
# =============================================================================

main() {
    log_info "=============================================="
    log_info "Phase 7 Inference Gateway - Startup"
    log_info "=============================================="
    log_info ""

    # Display agent identity
    log_info "Agent Configuration:"
    log_info "  Name:    ${AGENT_NAME:-unset}"
    log_info "  Domain:  ${AGENT_DOMAIN:-unset}"
    log_info "  Phase:   ${AGENT_PHASE:-unset}"
    log_info "  Layer:   ${AGENT_LAYER:-unset}"
    log_info "  Version: ${AGENT_VERSION:-unset}"
    log_info ""

    # Check gateway binary exists
    if [[ ! -x "${GATEWAY_BINARY}" ]]; then
        log_error "Gateway binary not found or not executable: ${GATEWAY_BINARY}"
        exit 3
    fi
    log_info "Gateway binary: OK"

    # Validate required environment variables
    log_info ""
    log_info "Validating environment variables..."

    local validation_failed=0

    # Agent identity (required)
    validate_required_env "AGENT_NAME" || validation_failed=1
    validate_required_env "AGENT_DOMAIN" || validation_failed=1
    validate_required_env "AGENT_PHASE" || validation_failed=1

    # RuVector configuration
    if [[ "${RUVECTOR_REQUIRED:-false}" == "true" ]]; then
        log_info ""
        log_info "RuVector is REQUIRED - validating configuration..."

        validate_required_env "RUVECTOR_SERVICE_URL" || validation_failed=1
        validate_required_env "RUVECTOR_API_KEY" || validation_failed=1

        if [[ ${validation_failed} -eq 0 ]]; then
            validate_ruvector_connectivity || {
                log_error ""
                log_error "FATAL: RuVector connectivity check failed!"
                log_error "The service cannot start without RuVector."
                log_error ""
                log_error "Troubleshooting:"
                log_error "  1. Verify RUVECTOR_SERVICE_URL is correct"
                log_error "  2. Verify RUVECTOR_API_KEY is valid"
                log_error "  3. Check RuVector service is running"
                log_error "  4. Check network connectivity"
                log_error ""
                exit 2
            }
        fi
    else
        log_info ""
        log_info "RuVector is optional - skipping connectivity check"
    fi

    if [[ ${validation_failed} -ne 0 ]]; then
        log_error ""
        log_error "FATAL: Environment validation failed!"
        log_error "Please set the required environment variables."
        exit 1
    fi

    # Display performance budgets
    log_info ""
    log_info "Performance Budgets:"
    log_info "  Max Tokens:       ${MAX_TOKENS:-unset}"
    log_info "  Max Latency (ms): ${MAX_LATENCY_MS:-unset}"
    log_info "  Max Calls/Run:    ${MAX_CALLS_PER_RUN:-unset}"
    log_info ""

    # Display Phase 7 features
    log_info "Phase 7 Features:"
    log_info "  Routing:          ${PHASE7_ROUTING_ENABLED:-false}"
    log_info "  Model Selection:  ${PHASE7_MODEL_SELECTION:-false}"
    log_info "  Cost Optimization: ${PHASE7_COST_OPTIMIZATION:-false}"
    log_info "  Semantic Cache:   ${PHASE7_SEMANTIC_CACHE:-false}"
    log_info ""

    log_info "=============================================="
    log_info "Starting Inference Gateway..."
    log_info "=============================================="
    log_info ""

    # Execute the gateway
    exec "${GATEWAY_BINARY}"
}

# Run main
main "$@"
