#!/usr/bin/env bash
# scripts/env/down.sh — tear down the shared local S3 mock and remove
# run-state from ENV_DIR.
#
# Safe to call when nothing is running (no-ops gracefully).
# Never touches git state or any committed files.
#
# Usage: scripts/env/down.sh

set -euo pipefail

# ---------------------------------------------------------------------------
# Locate the shared ENV_DIR in the MAIN repo (works from any worktree)
# ---------------------------------------------------------------------------
GIT_COMMON=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null \
  || { echo "ERROR: not inside a git repository" >&2; exit 1; })
MAIN_ROOT=$(dirname "$GIT_COMMON")
ENV_DIR="$MAIN_ROOT/.project/env"

DID_SOMETHING=false

# ---------------------------------------------------------------------------
# Helper: source local.env if present, ignoring errors (it may be absent or
# partially written if up.sh failed mid-run).
# ---------------------------------------------------------------------------
PROVIDER=""
if [[ -f "$ENV_DIR/local.env" ]]; then
  # shellcheck source=/dev/null
  source "$ENV_DIR/local.env" 2>/dev/null || true
  PROVIDER="${CAEROSTRIS_S3_PROVIDER:-}"
fi

# Also check the memory-only marker written by up.sh ladder step (d)
if [[ -z "$PROVIDER" && -f "$ENV_DIR/PROVIDER" ]]; then
  PROVIDER=$(cat "$ENV_DIR/PROVIDER" 2>/dev/null || true)
fi

# ---------------------------------------------------------------------------
# (a) Docker / MinIO teardown
# ---------------------------------------------------------------------------
if command -v docker &>/dev/null && docker info &>/dev/null 2>&1; then
  if docker ps -a --format '{{.Names}}' 2>/dev/null | grep -q '^caerostris-minio$'; then
    echo "Stopping and removing Docker container caerostris-minio…"
    docker rm -f caerostris-minio >/dev/null
    echo "  Container removed."
    DID_SOMETHING=true
  fi
fi

# ---------------------------------------------------------------------------
# (b) moto_server teardown — kill via pidfile
# ---------------------------------------------------------------------------
PIDFILE="$ENV_DIR/moto.pid"
if [[ -f "$PIDFILE" ]]; then
  MOTO_PID=$(cat "$PIDFILE" 2>/dev/null || true)
  if [[ -n "$MOTO_PID" ]] && kill -0 "$MOTO_PID" 2>/dev/null; then
    echo "Stopping moto_server (pid $MOTO_PID)…"
    kill "$MOTO_PID" 2>/dev/null || true
    # Wait briefly for clean exit
    local_deadline=$(( $(date +%s) + 5 ))
    while kill -0 "$MOTO_PID" 2>/dev/null; do
      if (( $(date +%s) >= local_deadline )); then
        echo "  moto_server did not exit cleanly; sending SIGKILL."
        kill -9 "$MOTO_PID" 2>/dev/null || true
        break
      fi
      sleep 0.3
    done
    echo "  moto_server stopped."
    DID_SOMETHING=true
  else
    echo "  moto.pid found but process $MOTO_PID is not running (already gone)."
    DID_SOMETHING=true
  fi
fi

# ---------------------------------------------------------------------------
# Remove run-state files from ENV_DIR
# Removes: local.env, moto.pid, PROVIDER marker, and any stale lock dir.
# Does NOT remove the ENV_DIR itself (it may hold unrelated project files).
# Never removes committed files or anything outside ENV_DIR.
# ---------------------------------------------------------------------------
REMOVED=()
for f in local.env moto.pid PROVIDER; do
  target="$ENV_DIR/$f"
  if [[ -f "$target" ]]; then
    rm -f "$target"
    REMOVED+=("$f")
    DID_SOMETHING=true
  fi
done

# Clean up any stale lock (should not normally be present, but safe to remove)
if [[ -d "$ENV_DIR/.up.lock" ]]; then
  rmdir "$ENV_DIR/.up.lock" 2>/dev/null || true
  REMOVED+=(".up.lock")
  DID_SOMETHING=true
fi

if [[ ${#REMOVED[@]} -gt 0 ]]; then
  echo "Removed run-state files: ${REMOVED[*]}"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
if [[ "$DID_SOMETHING" == "false" ]]; then
  echo "Nothing to tear down — S3 mock was not running."
else
  echo "Teardown complete."
fi
