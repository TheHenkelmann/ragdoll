#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Resolve RAGDOLL_JWT_SECRET for deploy scripts.
# Source this file: source "$(dirname "$0")/../_jwt_secret.sh"

if [[ -n "${RAGDOLL_JWT_SECRET:-}" ]]; then
  JWT_SECRET="${RAGDOLL_JWT_SECRET}"
  echo "Using RAGDOLL_JWT_SECRET from environment."
else
  JWT_SECRET="$(openssl rand -hex 32)"
  echo "Generated ephemeral RAGDOLL_JWT_SECRET (random, not saved)." >&2
  echo "Set export RAGDOLL_JWT_SECRET=<your-secret> before deploy to use your own value." >&2
fi

export JWT_SECRET
