#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REGION="${AWS_REGION:-eu-central-1}"
STACK_NAME="${STACK_NAME:-ragdoll-ecs}"
IMAGE_URI="${IMAGE_URI:-ghcr.io/thehenkelmann/ragdoll:latest}"
TEMPLATE="${SCRIPT_DIR}/template.yaml"

if [[ -n "${RAGDOLL_JWT_SECRET:-}" ]]; then
  JWT_PARAM="JwtSecretOverride=${RAGDOLL_JWT_SECRET}"
  echo "Using RAGDOLL_JWT_SECRET from environment."
else
  JWT_PARAM="JwtSecretOverride="
  echo "CloudFormation will generate an ephemeral RAGDOLL_JWT_SECRET (not saved)." >&2
  echo "Set export RAGDOLL_JWT_SECRET=<your-secret> before deploy to use your own value." >&2
fi

aws cloudformation deploy \
  --region "${REGION}" \
  --stack-name "${STACK_NAME}" \
  --template-file "${TEMPLATE}" \
  --capabilities CAPABILITY_NAMED_IAM \
  --parameter-overrides "${JWT_PARAM}" ImageUri="${IMAGE_URI}"

aws cloudformation describe-stacks \
  --region "${REGION}" \
  --stack-name "${STACK_NAME}" \
  --query "Stacks[0].Outputs" \
  --output table
