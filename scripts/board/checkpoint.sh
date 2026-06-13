#!/usr/bin/env bash
# scripts/board/checkpoint.sh — verify a clean, RESUMABLE checkpoint (T-0004).
#
# Run this when a STOP sentinel (`.project/STOP`) is observed, or at any point an
# epoch is about to stand down. It asserts the working tree is in a state the
# NEXT epoch can resume from without losing or duplicating work:
#
#   1. Git is clean      — no uncommitted/staged/partial changes. A half-written
#                          file or an un-committed board edit is unrecoverable
#                          context for the next epoch, so it fails the checkpoint.
#   2. in_progress noted — every board item still `status: in_progress` carries at
#                          least one bullet in its `## Notes / log` section, so the
#                          relaunch knows what was in flight and where to pick up.
#   3. STOP reported     — if `.project/STOP` exists, say so (this is the final,
#                          standing-down checkpoint); otherwise report "running".
#
# Exit code: 0 if the checkpoint is clean (resumable), non-zero otherwise with a
# diagnostic naming exactly what is wrong. A relaunch / pace-marshal can gate on
# the exit code.
#
# Usage:
#   scripts/board/checkpoint.sh                  # verify the repo's own tree
#   CAERO_ROOT=/path scripts/board/checkpoint.sh # verify an alternate tree (tests)
#
# See: docs/process/epoch-recycling.md           (STOP handling + relaunch)
#      .claude/workflows/mainspring.js           (the loop that observes STOP)
#      .project/board/tasks/T-0004-*.md          (acceptance criteria)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="${CAERO_ROOT:-$(cd "${SCRIPT_DIR}/../.." && pwd)}"

TASKS_DIR="${ROOT}/.project/board/tasks"
STOP_FILE="${ROOT}/.project/STOP"

FAIL=0

echo "== caerostris-db checkpoint =="
echo "root: ${ROOT}"

# ---------------------------------------------------------------------------
# 1. STOP sentinel status (informational; does not by itself fail).
# ---------------------------------------------------------------------------
if [ -f "$STOP_FILE" ]; then
  echo "STOP: sentinel present at ${STOP_FILE} — standing down (final checkpoint)."
else
  echo "STOP: no STOP sentinel — epoch is running."
fi

# ---------------------------------------------------------------------------
# 2. Git cleanliness — no uncommitted/partial state.
# ---------------------------------------------------------------------------
if git -C "$ROOT" rev-parse --git-dir >/dev/null 2>&1; then
  DIRTY="$(git -C "$ROOT" status --porcelain 2>/dev/null || true)"
  if [ -n "$DIRTY" ]; then
    echo "GIT: NOT CLEAN — uncommitted/partial changes present:" >&2
    echo "$DIRTY" | sed 's/^/  /' >&2
    echo "  → commit (or discard) before standing down; partial state is not resumable." >&2
    FAIL=1
  else
    echo "GIT: clean working tree."
  fi
else
  echo "GIT: not a git repository (${ROOT}) — skipping cleanliness check." >&2
fi

# ---------------------------------------------------------------------------
# 3. Every in_progress board item must carry a note for the next epoch.
# ---------------------------------------------------------------------------
extract_field() {
  awk '
    /^---/ { count++; if (count == 2) exit; next }
    count == 1 { print }
  ' "$1" | grep -m1 "^${2}:" | sed "s/^${2}:[[:space:]]*//" | tr -d '\r'
}

# Does the body's "## Notes / log" section contain at least one non-blank line?
has_note() {
  awk '
    /^##[[:space:]]+Notes/ { in_notes = 1; next }
    in_notes && /^##[[:space:]]/ { in_notes = 0 }
    in_notes {
      line = $0
      gsub(/[[:space:]]/, "", line)
      if (line != "") { found = 1 }
    }
    END { exit(found ? 0 : 1) }
  ' "$1"
}

UNNOTED=""
INPROGRESS_COUNT=0
if [ -d "$TASKS_DIR" ]; then
  for file in "${TASKS_DIR}"/*.md; do
    [ -f "$file" ] || continue
    status="$(extract_field "$file" status)"
    [ "$status" = "in_progress" ] || continue
    id="$(extract_field "$file" id)"
    INPROGRESS_COUNT=$((INPROGRESS_COUNT + 1))
    if ! has_note "$file"; then
      UNNOTED="${UNNOTED} ${id:-$(basename "$file")}"
    fi
  done
fi

if [ -n "$UNNOTED" ]; then
  echo "BOARD: in_progress items WITHOUT a handoff note:${UNNOTED}" >&2
  echo "  → add a Notes/log bullet recording what is in flight before standing down." >&2
  FAIL=1
else
  echo "BOARD: ${INPROGRESS_COUNT} in_progress item(s), all noted."
fi

# ---------------------------------------------------------------------------
# Verdict.
# ---------------------------------------------------------------------------
if [ "$FAIL" -eq 0 ]; then
  echo "CHECKPOINT: OK — tree is resumable by the next epoch."
  exit 0
fi
echo "CHECKPOINT: FAILED — resolve the items above, then re-run." >&2
exit 1
