#!/bin/bash
# LLM-Inference-Gateway Deployment Script
# Deploys unified service to Google Cloud Run

set -euo pipefail

# Configuration
PROJECT_ID="${GCP_PROJECT_ID:-agentics-dev}"
REGION="${GCP_REGION:-us-central1}"
SERVICE_NAME="llm-inference-gateway"
PLATFORM_ENV="${PLATFORM_ENV:-dev}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Verify prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    # Check gcloud
    if ! command -v gcloud &> /dev/null; then
        log_error "gcloud CLI not found. Please install Google Cloud SDK."
        exit 1
    fi

    # Check docker
    if ! command -v docker &> /dev/null; then
        log_error "Docker not found. Please install Docker."
        exit 1
    fi

    # Check authentication
    if ! gcloud auth print-identity-token &> /dev/null; then
        log_error "Not authenticated. Run 'gcloud auth login' first."
        exit 1
    fi

    # Verify project
    gcloud config set project "$PROJECT_ID"
    log_info "Using project: $PROJECT_ID"
}

# Enable required APIs
enable_apis() {
    log_info "Enabling required GCP APIs..."

    gcloud services enable \
        run.googleapis.com \
        cloudbuild.googleapis.com \
        secretmanager.googleapis.com \
        artifactregistry.googleapis.com \
        containerregistry.googleapis.com \
        --quiet

    log_info "APIs enabled successfully"
}

# Create service account with least privilege
setup_service_account() {
    log_info "Setting up service account..."

    SA_NAME="inference-gateway"
    SA_EMAIL="${SA_NAME}@${PROJECT_ID}.iam.gserviceaccount.com"

    # Create service account if not exists
    if ! gcloud iam service-accounts describe "$SA_EMAIL" &> /dev/null; then
        gcloud iam service-accounts create "$SA_NAME" \
            --display-name="LLM Inference Gateway Service Account" \
            --description="Service account for LLM Inference Gateway Cloud Run service"
    fi

    # Grant required roles (least privilege)
    ROLES=(
        "roles/secretmanager.secretAccessor"  # Access secrets
        "roles/run.invoker"                   # Invoke other Cloud Run services
        "roles/logging.logWriter"             # Write logs
        "roles/monitoring.metricWriter"       # Write metrics
        "roles/cloudtrace.agent"              # Send traces
    )

    for role in "${ROLES[@]}"; do
        gcloud projects add-iam-policy-binding "$PROJECT_ID" \
            --member="serviceAccount:${SA_EMAIL}" \
            --role="$role" \
            --quiet 2>/dev/null || true
    done

    log_info "Service account configured: $SA_EMAIL"
}

# Setup secrets in Secret Manager
setup_secrets() {
    log_info "Setting up secrets..."

    SECRETS=(
        "ruvector-api-key"
        "openai-api-key"
        "anthropic-api-key"
        "google-api-key"
    )

    for secret in "${SECRETS[@]}"; do
        if ! gcloud secrets describe "$secret" &> /dev/null 2>&1; then
            log_warn "Secret '$secret' not found. Creating placeholder..."
            echo -n "PLACEHOLDER_REPLACE_ME" | gcloud secrets create "$secret" \
                --data-file=- \
                --replication-policy="automatic" \
                --quiet
            log_warn "Please update secret '$secret' with actual value"
        else
            log_info "Secret '$secret' exists"
        fi
    done
}

# Build and push container image
build_image() {
    log_info "Building container image..."

    IMAGE_TAG="gcr.io/${PROJECT_ID}/${SERVICE_NAME}:${PLATFORM_ENV}"

    # Build using Cloud Build (faster, no local Docker needed)
    gcloud builds submit \
        --config=deploy/cloud-run/cloudbuild.yaml \
        --substitutions="_PLATFORM_ENV=${PLATFORM_ENV}" \
        --quiet

    log_info "Image built and pushed: $IMAGE_TAG"
}

# Deploy to Cloud Run
deploy_service() {
    log_info "Deploying to Cloud Run..."

    # Substitute PROJECT_ID in service.yaml
    sed "s/PROJECT_ID/${PROJECT_ID}/g" deploy/cloud-run/service.yaml > /tmp/service-substituted.yaml

    # Deploy using gcloud
    gcloud run services replace /tmp/service-substituted.yaml \
        --region="$REGION" \
        --quiet

    # Allow unauthenticated access (for public API)
    gcloud run services add-iam-policy-binding "$SERVICE_NAME" \
        --region="$REGION" \
        --member="allUsers" \
        --role="roles/run.invoker" \
        --quiet

    # Get service URL
    SERVICE_URL=$(gcloud run services describe "$SERVICE_NAME" \
        --region="$REGION" \
        --format='value(status.url)')

    log_info "Service deployed: $SERVICE_URL"
    echo "$SERVICE_URL" > /tmp/service-url.txt
}

# Verify deployment
verify_deployment() {
    log_info "Verifying deployment..."

    SERVICE_URL=$(cat /tmp/service-url.txt)

    # Health check
    log_info "Checking health endpoint..."
    if curl -sf "${SERVICE_URL}/health" > /dev/null; then
        log_info "✓ Health check passed"
    else
        log_error "✗ Health check failed"
        exit 1
    fi

    # Readiness check
    log_info "Checking readiness endpoint..."
    if curl -sf "${SERVICE_URL}/ready" > /dev/null; then
        log_info "✓ Readiness check passed"
    else
        log_warn "✗ Readiness check failed (providers may not be configured)"
    fi

    # Agent endpoint check
    log_info "Checking agent endpoints..."
    if curl -sf "${SERVICE_URL}/agents" > /dev/null; then
        log_info "✓ Agent list endpoint accessible"
    else
        log_warn "✗ Agent endpoint not accessible"
    fi

    # Models endpoint check
    log_info "Checking models endpoint..."
    if curl -sf "${SERVICE_URL}/v1/models" > /dev/null; then
        log_info "✓ Models endpoint accessible"
    else
        log_warn "✗ Models endpoint not accessible (expected if no providers configured)"
    fi

    echo ""
    log_info "=========================================="
    log_info "Deployment Complete!"
    log_info "=========================================="
    log_info "Service URL: $SERVICE_URL"
    log_info "Environment: $PLATFORM_ENV"
    log_info ""
    log_info "Agent Endpoints:"
    log_info "  POST ${SERVICE_URL}/agents/route    - Route inference request"
    log_info "  GET  ${SERVICE_URL}/agents/inspect  - Inspect routing config"
    log_info "  GET  ${SERVICE_URL}/agents/status   - Agent status"
    log_info "  GET  ${SERVICE_URL}/agents          - List agents"
    log_info ""
    log_info "OpenAI-Compatible Endpoints:"
    log_info "  POST ${SERVICE_URL}/v1/chat/completions"
    log_info "  GET  ${SERVICE_URL}/v1/models"
    log_info "=========================================="
}

# Main execution
main() {
    log_info "Starting LLM-Inference-Gateway deployment..."
    log_info "Project: $PROJECT_ID"
    log_info "Region: $REGION"
    log_info "Environment: $PLATFORM_ENV"
    echo ""

    check_prerequisites
    enable_apis
    setup_service_account
    setup_secrets
    build_image
    deploy_service
    verify_deployment
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --project)
            PROJECT_ID="$2"
            shift 2
            ;;
        --region)
            REGION="$2"
            shift 2
            ;;
        --env)
            PLATFORM_ENV="$2"
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --project PROJECT_ID    GCP project ID (default: agentics-dev)"
            echo "  --region REGION         GCP region (default: us-central1)"
            echo "  --env ENV               Environment: dev|staging|prod (default: dev)"
            echo "  --skip-build            Skip image build, use existing"
            echo "  --help                  Show this help message"
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

main
