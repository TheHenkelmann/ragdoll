#!/usr/bin/env bash
# Start a local Ragdoll dev session (Rust gateway + Python worker, no Docker).
#
# Usage:
#   ./scripts/dev-local.sh          # start
#   ./scripts/dev-local.sh stop     # stop background processes
#   ./scripts/dev-local.sh status   # show status
#   ./scripts/dev-local.sh logs     # tail gateway + worker logs
#
# Optional env:
#   RAGDOLL_DATA_DIR=/path/to/data   (default: <repo>/.data)
#   PORT=8080
#   SKIP_MODELS=1            skip models-ensure even if models missing
#   SKIP_FRONTEND=1          skip npm run build
#   PYTHON_VENV=/path/to/venv  override Python venv detection

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA_DIR="${RAGDOLL_DATA_DIR:-${DATA_DIR:-$ROOT/.data}}"
PORT="${RAGDOLL_PORT:-${PORT:-8080}}"
RUN_DIR="$DATA_DIR/run"
GATEWAY_PID="$RUN_DIR/gateway.pid"
WORKER_PID="$RUN_DIR/worker.pid"
GATEWAY_LOG="$RUN_DIR/gateway.log"
WORKER_LOG="$RUN_DIR/worker.log"

export RAGDOLL_DATA_DIR="$DATA_DIR"
export RAGDOLL_SECRET="${RAGDOLL_SECRET:-dev-local-secret-change-me}"
export RAGDOLL_PORT="$PORT"
export RAGDOLL_MIGRATIONS_DIR="$ROOT/migrations"
export RAGDOLL_STATIC_DIR="$ROOT/frontend/dist"
export RUST_LOG="${RUST_LOG:-info,ragdoll=debug}"

mkdir -p "$DATA_DIR" "$RUN_DIR"

die() {
  echo "error: $*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing command: $1"
}

is_running() {
  local pid_file=$1
  [[ -f $pid_file ]] || return 1
  local pid
  pid="$(cat "$pid_file")"
  kill -0 "$pid" 2>/dev/null
}

stop_pid_file() {
  local name=$1
  local pid_file=$2
  if is_running "$pid_file"; then
    local pid
    pid="$(cat "$pid_file")"
    echo "stopping $name (pid $pid)..."
    kill "$pid" 2>/dev/null || true
    for _ in $(seq 1 20); do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.25
    done
    if kill -0 "$pid" 2>/dev/null; then
      kill -9 "$pid" 2>/dev/null || true
    fi
  fi
  rm -f "$pid_file"
}

detect_python() {
  if [[ -n "${PYTHON_VENV:-}" ]]; then
    [[ -x "$PYTHON_VENV/bin/python" ]] || die "PYTHON_VENV has no bin/python: $PYTHON_VENV"
    echo "$PYTHON_VENV/bin/python"
    return
  fi
  if [[ -n "${VIRTUAL_ENV:-}" && -x "$VIRTUAL_ENV/bin/python" ]]; then
    echo "$VIRTUAL_ENV/bin/python"
    return
  fi
  for candidate in \
    "$ROOT/../.venv" \
    "$ROOT/python/.venv" \
    "$ROOT/.venv"; do
    if [[ -x "$candidate/bin/python" ]]; then
      echo "$candidate/bin/python"
      return
    fi
  done
  die "no Python venv found. Activate one or set PYTHON_VENV=/path/to/venv"
}

ensure_worker_installed() {
  local python_bin=$1
  "$python_bin" -c "import ragdoll_worker" 2>/dev/null && return 0
  echo "installing ragdoll-worker into $(dirname "$(dirname "$python_bin")")..."
  require_cmd uv
  (cd "$ROOT/python" && uv pip install --python "$python_bin" -e .)
}

models_present() {
  [[ -f "$DATA_DIR/models/BAAI__bge-m3/model.onnx" ]] \
    && [[ -f "$DATA_DIR/models/BAAI__bge-m3/tokenizer.json" ]]
}

ensure_frontend() {
  if [[ "${SKIP_FRONTEND:-}" == "1" ]]; then
    return 0
  fi
  local dist_index="$RAGDOLL_STATIC_DIR/index.html"
  if [[ -f "$dist_index" ]]; then
    local stale_src
    stale_src="$(find "$ROOT/frontend/src" -type f -newer "$dist_index" -print -quit 2>/dev/null || true)"
    if [[ -z "$stale_src" ]]; then
      return 0
    fi
    echo "frontend sources changed since last build — rebuilding..."
  else
    echo "building frontend (first run)..."
  fi
  require_cmd npm
  (cd "$ROOT/frontend" && npm install && npm run build)
}

port_in_use() {
  lsof -nP -iTCP:"$PORT" -sTCP:LISTEN >/dev/null 2>&1
}

stop_listeners_on_port() {
  local pids
  pids="$(lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t 2>/dev/null || true)"
  [[ -n "$pids" ]] || return 0
  echo "stopping listener(s) on port $PORT (pids: $pids)..."
  # shellcheck disable=SC2086
  kill $pids 2>/dev/null || true
  for _ in $(seq 1 20); do
    port_in_use || return 0
    sleep 0.25
  done
  pids="$(lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t 2>/dev/null || true)"
  if [[ -n "$pids" ]]; then
    # shellcheck disable=SC2086
    kill -9 $pids 2>/dev/null || true
  fi
}

stop_all() {
  stop_pid_file "gateway" "$GATEWAY_PID"
  stop_pid_file "worker" "$WORKER_PID"
  stop_listeners_on_port
  echo "stopped."
}

start_gateway() {
  if is_running "$GATEWAY_PID" && port_in_use; then
    echo "gateway already running (pid $(cat "$GATEWAY_PID"))"
    return 0
  fi
  rm -f "$GATEWAY_PID"
  if port_in_use; then
    echo "port $PORT held by stale process — stopping it..."
    stop_listeners_on_port
  fi

  echo "running migrations..."
  (cd "$ROOT" && cargo run --quiet -- migrate)

  if [[ "${SKIP_MODELS:-}" != "1" ]] && ! models_present; then
    echo "models missing — running models-ensure (first run, large download)..."
    (cd "$ROOT" && cargo run --quiet -- models-ensure)
  fi

  ensure_frontend

  local cargo_target="${CARGO_TARGET_DIR:-$ROOT/target}"
  local gateway_bin="$cargo_target/debug/ragdoll"

  echo "building gateway..."
  (cd "$ROOT" && cargo build --quiet)

  echo "starting gateway on http://127.0.0.1:$PORT ..."
  (cd "$ROOT" && nohup "$gateway_bin" serve >>"$GATEWAY_LOG" 2>&1 & echo $! >"$GATEWAY_PID")

  for _ in $(seq 1 120); do
    if curl -fsS "http://127.0.0.1:$PORT/api/v1/health" 2>/dev/null | grep -q '"ready":true'; then
      return 0
    fi
    sleep 1
  done
  echo "gateway did not become ready — last log lines:"
  tail -20 "$GATEWAY_LOG" >&2 || true
  die "gateway failed to start"
}

start_worker() {
  local python_bin
  python_bin="$(detect_python)"
  ensure_worker_installed "$python_bin"

  if is_running "$WORKER_PID"; then
    echo "worker already running (pid $(cat "$WORKER_PID"))"
    return 0
  fi

  echo "starting worker..."
  (cd "$ROOT/python" && nohup "$python_bin" -m ragdoll_worker >>"$WORKER_LOG" 2>&1 & echo $! >"$WORKER_PID")
  sleep 1
  is_running "$WORKER_PID" || die "worker failed to start — see $WORKER_LOG"
}

print_status() {
  echo "RAGDOLL_DATA_DIR=$DATA_DIR"
  echo "PORT=$PORT"
  if is_running "$GATEWAY_PID"; then
    echo "gateway: running (pid $(cat "$GATEWAY_PID"))"
  else
    echo "gateway: stopped"
  fi
  if is_running "$WORKER_PID"; then
    echo "worker:  running (pid $(cat "$WORKER_PID"))"
  else
    echo "worker:  stopped"
  fi
  if curl -fsS "http://127.0.0.1:$PORT/api/v1/health" 2>/dev/null; then
    echo
  else
    echo "health:  unreachable on port $PORT"
  fi
}

print_ready() {
  cat <<EOF

Ragdoll dev session is running.

  UI:      http://127.0.0.1:$PORT/
  Health:  http://127.0.0.1:$PORT/api/v1/health
  Swagger: http://127.0.0.1:$PORT/api/v1/swagger-ui

Logs:
  $GATEWAY_LOG
  $WORKER_LOG

Stop:
  $ROOT/scripts/dev-local.sh stop

Quick test:
  curl -X POST http://127.0.0.1:$PORT/api/v1/releases/first-release/sources \\
    -H 'Authorization: Bearer \$TOKEN' \\
    -H 'Content-Type: application/json' \\
    -d '[{"type":"text","name":"demo","content":"Ragdoll dev test."}]'

  curl -X POST http://127.0.0.1:$PORT/api/v1/releases/first-release/queries \\
    -H 'Authorization: Bearer \$TOKEN' \\
    -H 'Content-Type: application/json' \\
    -d '[{"text":"dev test"}]'

EOF
}

cmd="${1:-start}"

case "$cmd" in
  start)
    require_cmd cargo
    require_cmd curl
    require_cmd lsof
    start_gateway
    start_worker
    print_ready
    ;;
  stop)
    stop_all
    ;;
  status)
    print_status
    ;;
  logs)
    touch "$GATEWAY_LOG" "$WORKER_LOG"
    tail -f "$GATEWAY_LOG" "$WORKER_LOG"
    ;;
  *)
    die "unknown command: $cmd (use: start | stop | status | logs)"
    ;;
esac
