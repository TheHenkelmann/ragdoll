#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REGION="${AWS_REGION:-eu-central-1}"
STACK_NAME="${STACK_NAME:-ragdoll-app-runner}"
SOURCE_IMAGE="${SOURCE_IMAGE:-ghcr.io/thehenkelmann/ragdoll:latest}"
TEMPLATE="${SCRIPT_DIR}/template.yaml"

if [[ -n "${RAGDOLL_JWT_SECRET:-}" ]]; then
  JWT_PARAM="JwtSecretOverride=${RAGDOLL_JWT_SECRET}"
  echo "Using RAGDOLL_JWT_SECRET from environment."
else
  JWT_PARAM="JwtSecretOverride="
  echo "CloudFormation will generate an ephemeral RAGDOLL_JWT_SECRET (not saved)." >&2
  echo "Set export RAGDOLL_JWT_SECRET=<your-secret> before deploy to use your own value." >&2
fi

ACCOUNT_ID="$(aws sts get-caller-identity --query Account --output text)"
ECR_URI="${ACCOUNT_ID}.dkr.ecr.${REGION}.amazonaws.com/ragdoll:latest"

aws ecr get-login-password --region "${REGION}" | docker login --username AWS --password-stdin "${ACCOUNT_ID}.dkr.ecr.${REGION}.amazonaws.com"
docker pull "${SOURCE_IMAGE}"
docker tag "${SOURCE_IMAGE}" "${ECR_URI}"
docker push "${ECR_URI}"

aws cloudformation deploy \
  --region "${REGION}" \
  --stack-name "${STACK_NAME}" \
  --template-file "${TEMPLATE}" \
  --capabilities CAPABILITY_NAMED_IAM \
  --parameter-overrides "${JWT_PARAM}" SourceImage="${SOURCE_IMAGE}"

aws cloudformation describe-stacks \
  --region "${REGION}" \
  --stack-name "${STACK_NAME}" \
  --query "Stacks[0].Outputs" \
  --output table
