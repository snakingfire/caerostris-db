#!/usr/bin/env bash
# scripts/board/epoch-handoff.sh — serialise an epoch's in-flight context (T-0004).
#
# When the mainspring approaches the per-run agent cap it recycles: it writes a
# hand-off artifact capturing everything the NEXT epoch needs to resume at full
# throughput without re-doing completed work, then a fresh epoch is launched and
# reads the artifact. This script produces that artifact.
#
# The artifact is a lightweight, human-readable markdown file at
#   .project/epochs/epoch-<N>.md
# (markdown over binary so any agent can inspect it without tooling). It records:
#   * timestamp + epoch number;
#   * OPEN task IDs — every board item still resumable: ready / in_progress /
#     in_review / blocked. `done` / `dropped` are deliberately excluded so the
#     next epoch never re-executes finished work;
#   * blockers — items in `blocked` state, called out separately;
#   * the latest rubric snapshot (overall score + report name).
#
# Usage:
#   scripts/board/epoch-handoff.sh [N]            # N = epoch number (default: next)
#   CAERO_ROOT=/path scripts/board/epoch-handoff.sh N
#
# It is read-only on the board and on git (the only write is the one artifact
# file), so it is safe to call from a stand-down checkpoint.
#
# See: docs/process/epoch-recycling.md   (full relaunch procedure + schema)
#      .project/epochs/README.md         (schema reference)
#      scripts/board/checkpoint.sh        (the clean-state gate run alongside it)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="${CAERO_ROOT:-$(cd "${SCRIPT_DIR}/../.." && pwd)}"

TASKS_DIR="${ROOT}/.project/board/tasks"
REPORTS_DIR="${ROOT}/.project/reports"
EPOCHS_DIR="${ROOT}/.project/epochs"

if [ ! -d "$TASKS_DIR" ]; then
  echo "Error: board tasks directory not found: $TASKS_DIR" >&2
  exit 1
fi
mkdir -p "$EPOCHS_DIR"

# ---------------------------------------------------------------------------
# Epoch number: explicit arg wins; else next after the highest existing artifact.
# ---------------------------------------------------------------------------
EPOCH="${1:-}"
if [ -z "$EPOCH" ]; then
  LAST="$(find "$EPOCHS_DIR" -maxdepth 1 -name 'epoch-*.md' 2>/dev/null \
    | sed 's#.*/epoch-##; s#\.md$##' \
    | grep -E '^[0-9]+$' \
    | sort -n \
    | tail -1 || true)"
  if [ -z "$LAST" ]; then EPOCH=1; else EPOCH=$((LAST + 1)); fi
fi
case "$EPOCH" in
  ''|*[!0-9]*) echo "Error: epoch number must be a non-negative integer: '$EPOCH'" >&2; exit 1 ;;
esac

# ---------------------------------------------------------------------------
# Parse the board once: emit `id<TAB>status<TAB>title` per item (single awk).
# ---------------------------------------------------------------------------
PARSED="$(awk '
  FNR == 1 { in_fm = 0; fm_seen = 0; id = ""; status = ""; title = "" }
  {
    line = $0; sub(/\r$/, "", line)
    if (line == "---") {
      fm_seen++
      if (fm_seen == 1) { in_fm = 1; next }
      if (fm_seen == 2) {
        in_fm = 0
        if (id != "") { printf "%s\t%s\t%s\n", id, status, title }
        next
      }
    }
    if (in_fm) {
      if (line ~ /^id:/)         { v = line; sub(/^id:[[:space:]]*/, "", v);     id = v }
      else if (line ~ /^status:/) { v = line; sub(/^status:[[:space:]]*/, "", v); status = v }
      else if (line ~ /^title:/)  { v = line; sub(/^title:[[:space:]]*/, "", v);  title = v }
    }
  }
' "${TASKS_DIR}"/*.md 2>/dev/null || true)"

OPEN_ROWS=""      # ready / in_progress / in_review / blocked
BLOCKER_ROWS=""
OPEN_COUNT=0
BLOCKER_COUNT=0
while IFS=$'\t' read -r id status title; do
  [ -z "$id" ] && continue
  case "$status" in
    ready|in_progress|in_review|blocked)
      OPEN_ROWS="${OPEN_ROWS}| ${id} | ${status} | ${title} |"$'\n'
      OPEN_COUNT=$((OPEN_COUNT + 1))
      ;;
  esac
  if [ "$status" = "blocked" ]; then
    BLOCKER_ROWS="${BLOCKER_ROWS}| ${id} | ${title} |"$'\n'
    BLOCKER_COUNT=$((BLOCKER_COUNT + 1))
  fi
done <<< "$PARSED"

# ---------------------------------------------------------------------------
# Latest rubric snapshot (score accepts the live grader's `**~NN**` estimate).
# ---------------------------------------------------------------------------
LATEST_REPORT="$(find "$REPORTS_DIR" -maxdepth 1 -name 'rubric-*.md' 2>/dev/null | sort | tail -1 || true)"
RUBRIC_NAME="(none yet)"
RUBRIC_SCORE="n/a"
if [ -n "$LATEST_REPORT" ]; then
  RUBRIC_NAME="$(basename "$LATEST_REPORT")"
  RUBRIC_SCORE="$(grep -i 'OVERALL' "$LATEST_REPORT" \
    | grep -oE '\*\*~?[0-9]+\*\*' | sed 's/\*//g' | tail -1 || true)"
  [ -z "$RUBRIC_SCORE" ] && RUBRIC_SCORE="n/a"
fi

# ---------------------------------------------------------------------------
# Write the artifact.
# ---------------------------------------------------------------------------
NOW_ISO="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
ARTIFACT="${EPOCHS_DIR}/epoch-${EPOCH}.md"

{
  echo "# Epoch hand-off ${EPOCH} — ${NOW_ISO}"
  echo ""
  echo "> Serialised by \`scripts/board/epoch-handoff.sh\` when epoch ${EPOCH} recycled."
  echo "> The next epoch reads this to resume WITHOUT re-executing completed work."
  echo "> Schema: \`.project/epochs/README.md\`. Procedure: \`docs/process/epoch-recycling.md\`."
  echo ""
  echo "| field | value |"
  echo "|-------|-------|"
  echo "| epoch | ${EPOCH} |"
  echo "| timestamp | ${NOW_ISO} |"
  echo "| open items | ${OPEN_COUNT} |"
  echo "| blockers | ${BLOCKER_COUNT} |"
  echo "| rubric report | \`${RUBRIC_NAME}\` |"
  echo "| rubric overall | ${RUBRIC_SCORE} |"
  echo ""
  echo "## Open task IDs (resume these — \`done\`/\`dropped\` deliberately excluded)"
  echo ""
  if [ -n "$OPEN_ROWS" ]; then
    echo "| id | status | title |"
    echo "|----|--------|-------|"
    printf '%s' "$OPEN_ROWS"
  else
    echo "_None — the board is drained of open work._"
  fi
  echo ""
  echo "## Blockers (status: blocked — resolve or reland first)"
  echo ""
  if [ -n "$BLOCKER_ROWS" ]; then
    echo "| id | title |"
    echo "|----|-------|"
    printf '%s' "$BLOCKER_ROWS"
  else
    echo "_None — no items are blocked._"
  fi
  echo ""
  echo "## Resume checklist (see docs/process/epoch-recycling.md)"
  echo ""
  echo "1. Confirm \`scripts/board/checkpoint.sh\` exits 0 (clean, resumable tree)."
  echo "2. Run \`scripts/board/unblock.sh\` to re-open any newly-unblocked cascade."
  echo "3. Claim from the OPEN set above via \`scripts/board/claim.sh\` — the board's"
  echo "   committed \`status\` is the source of truth; this artifact is the summary."
  echo "4. Do NOT re-run items already \`done\` on the board."
} > "$ARTIFACT"

echo "$ARTIFACT"
