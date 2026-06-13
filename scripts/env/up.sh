#!/usr/bin/env bash
# scripts/env/up.sh — bring up the SHARED local S3 mock for caerostris-db.
#
# Design: run-state lives in the MAIN worktree's .project/env/ so all git
# worktrees under .worktrees/<ID> share ONE endpoint.  Many agents may call
# this concurrently; an atomic mkdir-lock serialises the provision step.
#
# Usage: scripts/env/up.sh
#   Override default port: CAEROSTRIS_S3_PORT=9000 scripts/env/up.sh

set -euo pipefail

# ---------------------------------------------------------------------------
# Locate the shared ENV_DIR in the MAIN repo (works from any worktree)
# ---------------------------------------------------------------------------
GIT_COMMON=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null \
  || { echo "ERROR: not inside a git repository" >&2; exit 1; })
MAIN_ROOT=$(dirname "$GIT_COMMON")
ENV_DIR="$MAIN_ROOT/.project/env"

# ---------------------------------------------------------------------------
# Helper: pick a free TCP port starting from $1
# ---------------------------------------------------------------------------
find_free_port() {
  local port="$1"
  while lsof -iTCP:"$port" -sTCP:LISTEN -t &>/dev/null; do
    port=$((port + 1))
  done
  echo "$port"
}

# ---------------------------------------------------------------------------
# Helper: health-check an already-written local.env
# ---------------------------------------------------------------------------
is_endpoint_alive() {
  local env_file="$ENV_DIR/local.env"
  [[ -f "$env_file" ]] || return 1
  # shellcheck source=/dev/null
  source "$env_file"
  local endpoint="${CAEROSTRIS_S3_ENDPOINT:-}"
  [[ -n "$endpoint" ]] || return 1
  local provider="${CAEROSTRIS_S3_PROVIDER:-}"
  if [[ "$provider" == "minio" ]]; then
    curl -fsS --max-time 3 "$endpoint/minio/health/live" &>/dev/null
  else
    # moto: a simple list-buckets against the endpoint is sufficient
    curl -fsS --max-time 3 "$endpoint" &>/dev/null
  fi
}

# ---------------------------------------------------------------------------
# Atomic lock: try mkdir (atomic on POSIX) up to 30 s, then fail
# ---------------------------------------------------------------------------
LOCK_DIR="$ENV_DIR/.up.lock"
LOCK_HELD=false

acquire_lock() {
  mkdir -p "$ENV_DIR"
  local deadline=$(( $(date +%s) + 30 ))
  while ! mkdir "$LOCK_DIR" 2>/dev/null; do
    if (( $(date +%s) >= deadline )); then
      echo "ERROR: timed out waiting for env lock ($LOCK_DIR). \
Remove it manually if stale." >&2
      exit 1
    fi
    sleep 0.5
  done
  LOCK_HELD=true
}

release_lock() {
  if [[ "$LOCK_HELD" == "true" ]]; then
    rmdir "$LOCK_DIR" 2>/dev/null || true
    LOCK_HELD=false
  fi
}

trap release_lock EXIT

# ---------------------------------------------------------------------------
# Fast path: already up and healthy — skip locking entirely
# ---------------------------------------------------------------------------
if is_endpoint_alive; then
  echo "already up — $(grep CAEROSTRIS_S3_ENDPOINT "$ENV_DIR/local.env")"
  echo "  source $ENV_DIR/local.env"
  exit 0
fi

# ---------------------------------------------------------------------------
# Serialise concurrent provisioners
# ---------------------------------------------------------------------------
acquire_lock

# Re-check inside the lock (another agent may have provisioned while we waited)
if is_endpoint_alive; then
  echo "already up (raced) — $(grep CAEROSTRIS_S3_ENDPOINT "$ENV_DIR/local.env")"
  echo "  source $ENV_DIR/local.env"
  exit 0
fi

# ---------------------------------------------------------------------------
# Determine port
# ---------------------------------------------------------------------------
DESIRED_PORT="${CAEROSTRIS_S3_PORT:-9000}"

# If the port is occupied by a process OTHER than our own minio container,
# find the next free one.
if lsof -iTCP:"$DESIRED_PORT" -sTCP:LISTEN -t &>/dev/null; then
  # Check whether it is our own container
  if ! docker ps --format '{{.Names}}' 2>/dev/null | grep -q '^caerostris-minio$'; then
    DESIRED_PORT=$(find_free_port "$DESIRED_PORT")
    echo "Port in use by another process; using port $DESIRED_PORT instead."
  fi
fi
S3_PORT="$DESIRED_PORT"

PROVIDER=""
ENDPOINT=""
ACCESS_KEY=""
SECRET_KEY=""

# ---------------------------------------------------------------------------
# Provision ladder
# ---------------------------------------------------------------------------

if command -v docker &>/dev/null && docker info &>/dev/null 2>&1; then
  # ── (a) Docker / MinIO ──────────────────────────────────────────────────
  PROVIDER="minio"
  ACCESS_KEY="minioadmin"
  SECRET_KEY="minioadmin"
  # NOTE: "minioadmin" is MinIO's documented PUBLIC DEFAULT for LOCAL MOCKS
  # ONLY — it is not a real secret.  Real AWS credentials come from the
  # environment at runtime and are never stored in this repo.

  ENDPOINT="http://127.0.0.1:${S3_PORT}"

  # Reuse a running container if its published port matches; otherwise (re)start.
  EXISTING=$(docker ps --filter "name=^caerostris-minio$" \
    --format '{{.Names}}' 2>/dev/null || true)
  if [[ -n "$EXISTING" ]]; then
    echo "Reusing running caerostris-minio container."
  else
    # Remove a stopped container with the same name if present
    docker rm -f caerostris-minio &>/dev/null 2>&1 || true

    echo "Starting MinIO container on port ${S3_PORT}…"
    docker run -d \
      --name caerostris-minio \
      -p "${S3_PORT}:9000" \
      -p "$((S3_PORT + 1)):9001" \
      -e MINIO_ROOT_USER="$ACCESS_KEY" \
      -e MINIO_ROOT_PASSWORD="$SECRET_KEY" \
      quay.io/minio/minio server /data --console-address ":9001" \
      >/dev/null
  fi

  # Wait for the health endpoint (up to 30 s)
  echo "Waiting for MinIO health endpoint…"
  local_deadline=$(( $(date +%s) + 30 ))
  until curl -fsS --max-time 2 "$ENDPOINT/minio/health/live" &>/dev/null; do
    if (( $(date +%s) >= local_deadline )); then
      echo "ERROR: MinIO did not become healthy within 30 s." >&2
      exit 1
    fi
    sleep 1
  done
  echo "MinIO is healthy."

elif command -v moto_server &>/dev/null; then
  # ── (b) moto_server already on PATH ────────────────────────────────────
  PROVIDER="moto"
  ACCESS_KEY="test"
  SECRET_KEY="test"
  ENDPOINT="http://127.0.0.1:${S3_PORT}"

  echo "Starting moto_server on port ${S3_PORT}…"
  moto_server -p "${S3_PORT}" &>/tmp/caerostris-moto.log &
  MOTO_PID=$!
  echo "$MOTO_PID" > "$ENV_DIR/moto.pid"

  # Wait for moto to accept connections (up to 15 s)
  local_deadline=$(( $(date +%s) + 15 ))
  until curl -fsS --max-time 2 "$ENDPOINT" &>/dev/null; do
    if (( $(date +%s) >= local_deadline )); then
      echo "ERROR: moto_server did not start within 15 s." >&2
      kill "$MOTO_PID" 2>/dev/null || true
      exit 1
    fi
    sleep 0.5
  done
  echo "moto_server is up (pid $MOTO_PID)."

elif command -v pip &>/dev/null || command -v pip3 &>/dev/null; then
  # ── (c) pip present — install moto[server] then start ──────────────────
  PIP_CMD="pip"
  command -v pip3 &>/dev/null && PIP_CMD="pip3"

  echo "moto_server not found; installing moto[server] via $PIP_CMD…"
  "$PIP_CMD" install --quiet 'moto[server]' >/dev/null

  if ! command -v moto_server &>/dev/null; then
    echo "ERROR: moto_server still not on PATH after pip install." >&2
    echo "       You may need to add the Python scripts directory to \$PATH." >&2
    exit 1
  fi

  PROVIDER="moto"
  ACCESS_KEY="test"
  SECRET_KEY="test"
  ENDPOINT="http://127.0.0.1:${S3_PORT}"

  echo "Starting moto_server on port ${S3_PORT}…"
  moto_server -p "${S3_PORT}" &>/tmp/caerostris-moto.log &
  MOTO_PID=$!
  mkdir -p "$ENV_DIR"
  echo "$MOTO_PID" > "$ENV_DIR/moto.pid"

  local_deadline=$(( $(date +%s) + 15 ))
  until curl -fsS --max-time 2 "$ENDPOINT" &>/dev/null; do
    if (( $(date +%s) >= local_deadline )); then
      echo "ERROR: moto_server did not start within 15 s." >&2
      kill "$MOTO_PID" 2>/dev/null || true
      exit 1
    fi
    sleep 0.5
  done
  echo "moto_server is up (pid $MOTO_PID)."

else
  # ── (d) No Docker, no moto, no pip — in-process memory backend only ────
  echo "PROVIDER=memory" > "$ENV_DIR/PROVIDER"
  echo ""
  echo "WARNING: Neither Docker nor Python/pip was found." >&2
  echo "  Only the in-process object_store memory backend is available." >&2
  echo "  Unit tests will work; integration tests require Docker or moto." >&2
  echo "  Install Docker (preferred) or Python + pip, then re-run this script." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Write the shared local.env
# ---------------------------------------------------------------------------
cat > "$ENV_DIR/local.env" <<EOF
# Generated by scripts/env/up.sh — do not edit by hand.
CAEROSTRIS_S3_ENDPOINT=${ENDPOINT}
CAEROSTRIS_S3_PORT=${S3_PORT}
CAEROSTRIS_S3_PROVIDER=${PROVIDER}
CAEROSTRIS_S3_BUCKET_BASE=caerostris-it
CAEROSTRIS_S3_REGION=us-east-1
CAEROSTRIS_S3_FORCE_PATH_STYLE=true
AWS_ACCESS_KEY_ID=${ACCESS_KEY}
AWS_SECRET_ACCESS_KEY=${SECRET_KEY}
AWS_REGION=us-east-1
EOF

echo ""
echo "S3 mock is up:"
echo "  endpoint : $ENDPOINT"
echo "  provider : $PROVIDER"
echo "  env file : $ENV_DIR/local.env"
echo ""
echo "  source $ENV_DIR/local.env"
