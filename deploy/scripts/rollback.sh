#!/bin/bash
# LLM-Inference-Gateway Rollback Script
# Safe rollback to previous revision without data loss

set -euo pipefail

PROJECT_ID="${GCP_PROJECT_ID:-agentics-dev}"
REGION="${GCP_REGION:-us-central1}"
SERVICE_NAME="llm-inference-gateway"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# List available revisions
list_revisions() {
    log_info "Available revisions:"
    gcloud run revisions list \
        --service="$SERVICE_NAME" \
        --region="$REGION" \
        --format="table(metadata.name,status.conditions[0].status,spec.containers[0].image,metadata.creationTimestamp)"
}

# Get current serving revision
get_current_revision() {
    gcloud run services describe "$SERVICE_NAME" \
        --region="$REGION" \
        --format='value(status.traffic[0].revisionName)'
}

# Get previous revision
get_previous_revision() {
    gcloud run revisions list \
        --service="$SERVICE_NAME" \
        --region="$REGION" \
        --format='value(metadata.name)' \
        --sort-by='~metadata.creationTimestamp' \
        --limit=2 | tail -1
}

# Rollback to specific revision
rollback_to_revision() {
    local target_revision="$1"

    log_info "Rolling back to revision: $target_revision"

    # Update traffic to target revision
    gcloud run services update-traffic "$SERVICE_NAME" \
        --region="$REGION" \
        --to-revisions="${target_revision}=100" \
        --quiet

    log_info "Traffic shifted to $target_revision"
}

# Verify rollback
verify_rollback() {
    local expected_revision="$1"

    log_info "Verifying rollback..."

    current=$(get_current_revision)

    if [ "$current" == "$expected_revision" ]; then
        log_info "✓ Rollback verified. Current revision: $current"

        # Health check
        SERVICE_URL=$(gcloud run services describe "$SERVICE_NAME" \
            --region="$REGION" \
            --format='value(status.url)')

        if curl -sf "${SERVICE_URL}/health" > /dev/null; then
            log_info "✓ Health check passed"
        else
            log_error "✗ Health check failed after rollback"
            return 1
        fi
    else
        log_error "✗ Rollback verification failed"
        log_error "Expected: $expected_revision, Got: $current"
        return 1
    fi
}

# Main rollback flow
main() {
    local target_revision="${1:-}"

    log_info "LLM-Inference-Gateway Rollback"
    log_info "Project: $PROJECT_ID"
    log_info "Region: $REGION"
    echo ""

    current_revision=$(get_current_revision)
    log_info "Current serving revision: $current_revision"
    echo ""

    if [ -z "$target_revision" ]; then
        # Auto-select previous revision
        target_revision=$(get_previous_revision)

        if [ -z "$target_revision" ] || [ "$target_revision" == "$current_revision" ]; then
            log_error "No previous revision available for rollback"
            echo ""
            list_revisions
            exit 1
        fi

        log_warn "Auto-selected previous revision: $target_revision"
        echo ""
        read -p "Proceed with rollback to $target_revision? (y/N) " confirm

        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            log_info "Rollback cancelled"
            exit 0
        fi
    fi

    echo ""
    rollback_to_revision "$target_revision"
    echo ""
    verify_rollback "$target_revision"

    echo ""
    log_info "=========================================="
    log_info "Rollback Complete"
    log_info "=========================================="
    log_info "Previous revision: $current_revision"
    log_info "Current revision:  $target_revision"
    log_info ""
    log_info "NOTE: DecisionEvents are preserved in ruvector-service."
    log_info "No routing data was lost during rollback."
    log_info "=========================================="
}

# Parse arguments
case "${1:-}" in
    --list)
        list_revisions
        ;;
    --help)
        echo "Usage: $0 [REVISION] [OPTIONS]"
        echo ""
        echo "Arguments:"
        echo "  REVISION    Target revision name (auto-selects previous if omitted)"
        echo ""
        echo "Options:"
        echo "  --list      List available revisions"
        echo "  --help      Show this help message"
        exit 0
        ;;
    *)
        main "${1:-}"
        ;;
esac
