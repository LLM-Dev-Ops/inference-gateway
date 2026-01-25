#!/bin/bash
# Phase 7 Cloud Run Deployment Script
# Requires: gcloud CLI authenticated, Secret Manager configured
#
# Usage:
#   PROJECT_ID=my-project REGION=us-central1 ./deploy.sh
#   PROJECT_ID=my-project ENVIRONMENT=staging ./deploy.sh
#
# Prerequisites:
#   1. gcloud CLI installed and authenticated
#   2. Secret Manager secrets created:
#      - ruvector-api-key (required)
#      - openai-api-key (optional)
#      - anthropic-api-key (optional)
#   3. Docker image built or source available

set -euo pipefail

# =============================================================================
# Configuration
# =============================================================================

# Required configuration
PROJECT_ID="${PROJECT_ID:-}"
REGION="${REGION:-us-central1}"
ENVIRONMENT="${ENVIRONMENT:-dev}"

# Service configuration
SERVICE_NAME="${SERVICE_NAME:-inference-gateway-phase7}"
SECRET_NAME="${SECRET_NAME:-ruvector-api-key}"

# RuVector URL per environment
case "${ENVIRONMENT}" in
  dev)
    RUVECTOR_URL="${RUVECTOR_SERVICE_URL:-https://ruvector-service-agentics-dev.run.app}"
    MIN_INSTANCES="0"
    MAX_INSTANCES="3"
    MEMORY="512Mi"
    CPU="1"
    LOG_LEVEL="debug,phase7=trace"
    ;;
  staging)
    RUVECTOR_URL="${RUVECTOR_SERVICE_URL:-https://ruvector-service-agentics-staging.run.app}"
    MIN_INSTANCES="1"
    MAX_INSTANCES="10"
    MEMORY="1Gi"
    CPU="1"
    LOG_LEVEL="info,phase7=debug"
    ;;
  prod)
    RUVECTOR_URL="${RUVECTOR_SERVICE_URL:-https://ruvector-service-agentics-prod.run.app}"
    MIN_INSTANCES="2"
    MAX_INSTANCES="100"
    MEMORY="2Gi"
    CPU="2"
    LOG_LEVEL="info,phase7=info"
    ;;
  *)
    echo "ERROR: Unknown environment '${ENVIRONMENT}'. Use: dev, staging, or prod"
    exit 1
    ;;
esac

# =============================================================================
# Validation
# =============================================================================

echo "=============================================="
echo "Phase 7 Cloud Run Deployment"
echo "=============================================="
echo ""

# Check required configuration
if [[ -z "${PROJECT_ID}" ]]; then
    echo "ERROR: PROJECT_ID must be set"
    echo "Usage: PROJECT_ID=my-project ./deploy.sh"
    exit 1
fi

# Check gcloud is installed
if ! command -v gcloud &> /dev/null; then
    echo "ERROR: gcloud CLI is not installed"
    echo "Install: https://cloud.google.com/sdk/docs/install"
    exit 1
fi

# Check gcloud is authenticated
if ! gcloud auth list --filter=status:ACTIVE --format="value(account)" | head -n1 > /dev/null 2>&1; then
    echo "ERROR: gcloud is not authenticated"
    echo "Run: gcloud auth login"
    exit 1
fi

# Check secret exists
echo "Checking Secret Manager for '${SECRET_NAME}'..."
if ! gcloud secrets describe "${SECRET_NAME}" --project="${PROJECT_ID}" > /dev/null 2>&1; then
    echo "ERROR: Secret '${SECRET_NAME}' not found in Secret Manager"
    echo ""
    echo "Create the secret with:"
    echo "  echo -n 'your-api-key' | gcloud secrets create ${SECRET_NAME} --data-file=- --project=${PROJECT_ID}"
    exit 1
fi
echo "Secret '${SECRET_NAME}' found."

# =============================================================================
# Deployment
# =============================================================================

echo ""
echo "Configuration:"
echo "  Project:     ${PROJECT_ID}"
echo "  Region:      ${REGION}"
echo "  Environment: ${ENVIRONMENT}"
echo "  Service:     ${SERVICE_NAME}"
echo "  RuVector:    ${RUVECTOR_URL}"
echo "  Instances:   ${MIN_INSTANCES}-${MAX_INSTANCES}"
echo "  Resources:   ${CPU} CPU, ${MEMORY} memory"
echo ""

# Confirm deployment for production
if [[ "${ENVIRONMENT}" == "prod" ]]; then
    read -p "Deploying to PRODUCTION. Continue? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Deployment cancelled."
        exit 0
    fi
fi

echo "Deploying ${SERVICE_NAME} to Cloud Run..."
echo ""

# Build the deploy command
DEPLOY_CMD=(
    gcloud run deploy "${SERVICE_NAME}"
    --project="${PROJECT_ID}"
    --region="${REGION}"
    --source=../..
    --set-secrets="RUVECTOR_API_KEY=${SECRET_NAME}:latest"
    --set-env-vars="RUVECTOR_SERVICE_URL=${RUVECTOR_URL}"
    --set-env-vars="RUVECTOR_REQUIRED=true"
    --set-env-vars="RUVECTOR_TIMEOUT_MS=5000"
    --set-env-vars="RUVECTOR_RETRY_COUNT=3"
    --set-env-vars="AGENT_NAME=inference-routing-agent"
    --set-env-vars="AGENT_DOMAIN=inference-gateway"
    --set-env-vars="AGENT_PHASE=phase7"
    --set-env-vars="AGENT_LAYER=layer2"
    --set-env-vars="AGENT_VERSION=1.0.0"
    --set-env-vars="MAX_TOKENS=2500"
    --set-env-vars="MAX_LATENCY_MS=5000"
    --set-env-vars="MAX_CALLS_PER_RUN=5"
    --set-env-vars="PHASE7_ROUTING_ENABLED=true"
    --set-env-vars="PHASE7_MODEL_SELECTION=true"
    --set-env-vars="PHASE7_COST_OPTIMIZATION=true"
    --set-env-vars="PHASE7_SEMANTIC_CACHE=true"
    --set-env-vars="RUST_LOG=${LOG_LEVEL}"
    --set-env-vars="PLATFORM_ENV=${ENVIRONMENT}"
    --min-instances="${MIN_INSTANCES}"
    --max-instances="${MAX_INSTANCES}"
    --cpu="${CPU}"
    --memory="${MEMORY}"
    --timeout=60s
    --concurrency=80
    --cpu-throttling
    --execution-environment=gen2
)

# Add authentication based on environment
if [[ "${ENVIRONMENT}" == "dev" ]]; then
    DEPLOY_CMD+=(--allow-unauthenticated)
else
    DEPLOY_CMD+=(--no-allow-unauthenticated)
fi

# Execute deployment
"${DEPLOY_CMD[@]}"

# =============================================================================
# Post-Deployment Verification
# =============================================================================

echo ""
echo "=============================================="
echo "Deployment Complete!"
echo "=============================================="
echo ""

# Get service URL
SERVICE_URL=$(gcloud run services describe "${SERVICE_NAME}" \
    --project="${PROJECT_ID}" \
    --region="${REGION}" \
    --format='value(status.url)')

echo "Service URL: ${SERVICE_URL}"
echo ""

# Verify health endpoint
echo "Verifying health endpoint..."
HEALTH_RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" "${SERVICE_URL}/health" || echo "000")

if [[ "${HEALTH_RESPONSE}" == "200" ]]; then
    echo "Health check: PASSED (HTTP 200)"
elif [[ "${HEALTH_RESPONSE}" == "401" || "${HEALTH_RESPONSE}" == "403" ]]; then
    echo "Health check: Authentication required (expected for non-dev)"
else
    echo "Health check: WARNING - HTTP ${HEALTH_RESPONSE}"
    echo "The service may still be starting up. Check logs with:"
    echo "  gcloud run services logs read ${SERVICE_NAME} --project=${PROJECT_ID} --region=${REGION}"
fi

# Verify RuVector connectivity (if public)
if [[ "${ENVIRONMENT}" == "dev" ]]; then
    echo ""
    echo "Verifying RuVector connectivity..."
    READY_RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" "${SERVICE_URL}/ready" || echo "000")
    if [[ "${READY_RESPONSE}" == "200" ]]; then
        echo "RuVector connectivity: PASSED"
    else
        echo "RuVector connectivity: CHECK LOGS (HTTP ${READY_RESPONSE})"
    fi
fi

echo ""
echo "Useful commands:"
echo "  View logs:    gcloud run services logs read ${SERVICE_NAME} --project=${PROJECT_ID} --region=${REGION}"
echo "  Describe:     gcloud run services describe ${SERVICE_NAME} --project=${PROJECT_ID} --region=${REGION}"
echo "  Revisions:    gcloud run revisions list --service=${SERVICE_NAME} --project=${PROJECT_ID} --region=${REGION}"
echo ""
