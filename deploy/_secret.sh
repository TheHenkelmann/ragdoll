#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Resolve RAGDOLL_SECRET for deploy scripts.
# Source this file: source "$(dirname "$0")/../_secret.sh"

if [[ -n "${RAGDOLL_SECRET:-}" ]]; then
  SECRET="${RAGDOLL_SECRET}"
  echo "Using RAGDOLL_SECRET from environment."
else
  SECRET="$(openssl rand -hex 32)"
  echo "Generated ephemeral RAGDOLL_SECRET (random, not saved)." >&2
  echo "Set export RAGDOLL_SECRET=<your-secret> before deploy to use your own value." >&2
fi

export SECRET
