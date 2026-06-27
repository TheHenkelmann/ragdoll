#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=../../_secret.sh
source "${SCRIPT_DIR}/../../_secret.sh"

PROJECT_ID="${PROJECT_ID:-$(gcloud config get-value project)}"
ZONE="${ZONE:-europe-west1-b}"
INSTANCE_NAME="${INSTANCE_NAME:-ragdoll}"
DISK_SIZE_GB="${DISK_SIZE_GB:-50}"
IMAGE="${IMAGE:-ghcr.io/thehenkelmann/ragdoll:latest}"

if [[ -z "${PROJECT_ID}" ]]; then
  echo "Set PROJECT_ID or configure gcloud default project" >&2
  exit 1
fi

gcloud compute instances create-with-container "${INSTANCE_NAME}" \
  --project="${PROJECT_ID}" \
  --zone="${ZONE}" \
  --machine-type=e2-standard-4 \
  --boot-disk-size="${DISK_SIZE_GB}GB" \
  --container-image="${IMAGE}" \
  --container-restart-policy=always \
  --container-env="RAGDOLL_DATA_DIR=/data,RAGDOLL_SECRET=${SECRET}" \
  --container-mount-host-path=mount-path=/data,host-path=/mnt/disks/ragdoll-data,mode=rw \
  --metadata=google-logging-enabled=true

echo "Deployed GCE container VM ${INSTANCE_NAME} in ${ZONE}"
