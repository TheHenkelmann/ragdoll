#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=../../_secret.sh
source "${SCRIPT_DIR}/../../_secret.sh"

PROJECT_ID="${PROJECT_ID:-$(gcloud config get-value project)}"
REGION="${REGION:-europe-west1}"
SERVICE_NAME="${SERVICE_NAME:-ragdoll}"
IMAGE="${IMAGE:-ghcr.io/thehenkelmann/ragdoll:latest}"
BUCKET="${BUCKET:-${PROJECT_ID}-ragdoll-data}"

if [[ -z "${PROJECT_ID}" ]]; then
  echo "Set PROJECT_ID or configure gcloud default project" >&2
  exit 1
fi

gcloud services enable run.googleapis.com artifactregistry.googleapis.com storage.googleapis.com

if ! gcloud artifacts repositories describe ragdoll --location="${REGION}" >/dev/null 2>&1; then
  gcloud artifacts repositories create ragdoll \
    --repository-format=docker \
    --location="${REGION}" \
    --description="Ragdoll container images"
fi

AR_IMAGE="${REGION}-docker.pkg.dev/${PROJECT_ID}/ragdoll/ragdoll:latest"
echo "Mirroring ${IMAGE} -> ${AR_IMAGE}"
docker pull "${IMAGE}"
docker tag "${IMAGE}" "${AR_IMAGE}"
docker push "${AR_IMAGE}"

gsutil mb -l "${REGION}" "gs://${BUCKET}" 2>/dev/null || true

gcloud run deploy "${SERVICE_NAME}" \
  --image="${AR_IMAGE}" \
  --region="${REGION}" \
  --platform=managed \
  --allow-unauthenticated \
  --port=8080 \
  --min-instances=0 \
  --max-instances=1 \
  --cpu=2 \
  --memory=4Gi \
  --timeout=3600 \
  --set-env-vars="RAGDOLL_DATA_DIR=/data,RAGDOLL_SECRET=${SECRET}" \
  --add-volume=name=data,type=cloud-storage,bucket="${BUCKET}" \
  --add-volume-mount=volume=data,mount-path=/data

echo "Deployed Cloud Run service ${SERVICE_NAME} in ${REGION}"
