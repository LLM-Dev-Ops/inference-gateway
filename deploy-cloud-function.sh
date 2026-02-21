#!/usr/bin/env bash
set -euo pipefail

# Deploy inference-gateway-agents Cloud Function
# Usage: ./deploy-cloud-function.sh

gcloud functions deploy inference-gateway-agents \
  --runtime nodejs20 \
  --trigger-http \
  --region us-central1 \
  --project agentics-dev \
  --entry-point handler \
  --memory 512MB \
  --timeout 120s \
  --no-allow-unauthenticated \
  --set-env-vars "GATEWAY_INTERNAL_URL=${GATEWAY_INTERNAL_URL:-http://localhost:8080}"
