#!/usr/bin/env bash
# scripts/board/dashboard.sh — regenerate the board/pace dashboard (T-0004).
#
# Produces a human-readable markdown snapshot of the project's live state:
#   (a) item counts by status (backlog/ready/in_progress/in_review/blocked/done)
#       and by epic;
#   (b) a pace metric: items completed / elapsed time since T0, with a projected
#       drain of the remaining board at the current rate;
#   (c) the latest rubric overall score (read from .project/reports/rubric-*.md);
#   (d) the current blockers (items in `blocked` state).
#
# Usage:
#   scripts/board/dashboard.sh                     # write dashboard, print its path
#   CAERO_ROOT=/path scripts/board/dashboard.sh    # operate on an alternate tree
#                                                  #   (used by the test harness)
#
# Design notes:
#   * Output goes to $ROOT/.project/reports/dashboard-<UTC-timestamp>.md.
#   * The ONLY filesystem side effect is writing that one file — the script never
#     mutates the board, the pace ledger, or git — so it is safe to run on every
#     grader cycle and repeatedly without surprises.
#   * Uses only coreutils + awk/grep/sed, no network and no cargo, so it finishes
#     in well under a second on a normal board.
#
# See: docs/process/task-board-protocol.md (board format)
#      docs/process/epoch-recycling.md     (when/why this runs each grader cycle)
#      .project/board/tasks/T-0004-*.md    (acceptance criteria)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# CAERO_ROOT lets tests (and alternate checkouts) point the generator at a
# fixture tree; default is the repository root two levels above this script.
ROOT="${CAERO_ROOT:-$(cd "${SCRIPT_DIR}/../.." && pwd)}"

TASKS_DIR="${ROOT}/.project/board/tasks"
REPORTS_DIR="${ROOT}/.project/reports"
PACE_FILE="${ROOT}/.project/pace/deadline.md"

if [ ! -d "$TASKS_DIR" ]; then
  echo "Error: board tasks directory not found: $TASKS_DIR" >&2
  exit 1
fi
mkdir -p "$REPORTS_DIR"

# ---------------------------------------------------------------------------
# Walk the board once: tally status + epic and collect blocker rows.
#
# Performance: parse every file's frontmatter in a SINGLE awk pass (not one
# subprocess per field per file) and emit a tab-delimited
# `id<TAB>status<TAB>epic<TAB>title` row per item. With ~100 board items the old
# 4-subprocess-per-file approach spawned ~400 pipelines and ran for >5 s under a
# busy parallel swarm; one awk over the whole directory keeps us well under 1 s.
# ---------------------------------------------------------------------------
declare -A STATUS_COUNT
declare -A EPIC_COUNT
BLOCKERS=""
TOTAL=0

# Canonical status order for stable output.
STATUSES="backlog ready in_progress in_review blocked done dropped"
for s in $STATUSES; do STATUS_COUNT[$s]=0; done

# One awk pass over all board files. FNR==1 resets per-file state; we read only
# the frontmatter (between the first two `---` lines) and capture the four
# fields we report on. Strip a trailing CR for CRLF-safety.
PARSED="$(awk '
  FNR == 1 { in_fm = 0; fm_seen = 0; id = ""; status = ""; epic = ""; title = "" }
  {
    line = $0
    sub(/\r$/, "", line)
    if (line == "---") {
      fm_seen++
      if (fm_seen == 1) { in_fm = 1; next }
      if (fm_seen == 2) {
        in_fm = 0
        if (id != "") {
          if (status == "") status = "(none)"
          if (epic == "")   epic = "(none)"
          printf "%s\t%s\t%s\t%s\n", id, status, epic, title
        }
        next
      }
    }
    if (in_fm) {
      if (line ~ /^id:/)     { v = line; sub(/^id:[[:space:]]*/, "", v);     id = v }
      else if (line ~ /^status:/) { v = line; sub(/^status:[[:space:]]*/, "", v); status = v }
      else if (line ~ /^epic:/)   { v = line; sub(/^epic:[[:space:]]*/, "", v);   epic = v }
      else if (line ~ /^title:/)  { v = line; sub(/^title:[[:space:]]*/, "", v);  title = v }
    }
  }
' "${TASKS_DIR}"/*.md 2>/dev/null || true)"

while IFS=$'\t' read -r id status epic title; do
  [ -z "$id" ] && continue
  TOTAL=$((TOTAL + 1))
  STATUS_COUNT[$status]=$(( ${STATUS_COUNT[$status]:-0} + 1 ))
  EPIC_COUNT[$epic]=$(( ${EPIC_COUNT[$epic]:-0} + 1 ))
  if [ "$status" = "blocked" ]; then
    BLOCKERS="${BLOCKERS}| ${id} | ${title} | ${epic} |"$'\n'
  fi
done <<< "$PARSED"

DONE_COUNT="${STATUS_COUNT[done]:-0}"

# ---------------------------------------------------------------------------
# Latest rubric report + its overall score.
# ---------------------------------------------------------------------------
LATEST_REPORT=""
LATEST_SCORE="n/a"
if [ -d "$REPORTS_DIR" ]; then
  # Sort rubric-*.md by name (embedded T+ markers ⇒ lexical == chronological)
  # and take the last; never consider dashboard-*.md so we don't cite ourselves.
  LATEST_REPORT="$(find "$REPORTS_DIR" -maxdepth 1 -name 'rubric-*.md' 2>/dev/null | sort | tail -1 || true)"
  if [ -n "$LATEST_REPORT" ]; then
    # The OVERALL score lives on the table row containing "OVERALL", wrapped in
    # **bold**. The live grader writes it as an estimate, e.g. `**~25**`, so we
    # accept an optional leading `~` and keep it for fidelity (the score is
    # approximate). Take the LAST such match on the row so the bold weight cell
    # (`**100**`) never wins over the score cell that follows it.
    LATEST_SCORE="$(grep -i 'OVERALL' "$LATEST_REPORT" \
      | grep -oE '\*\*~?[0-9]+\*\*' \
      | sed 's/\*//g' \
      | tail -1 || true)"
    [ -z "$LATEST_SCORE" ] && LATEST_SCORE="n/a"
  fi
fi
LATEST_REPORT_NAME="(none yet)"
[ -n "$LATEST_REPORT" ] && LATEST_REPORT_NAME="$(basename "$LATEST_REPORT")"

# ---------------------------------------------------------------------------
# Pace metric: elapsed since T0, completion rate, projected drain.
# ---------------------------------------------------------------------------
NOW_EPOCH="$(date -u +%s)"
NOW_ISO="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"

T0_ISO=""
if [ -f "$PACE_FILE" ]; then
  # Match the T0 line: "... T0 ...: `2026-06-13T18:24:00Z` ..."
  T0_ISO="$(grep -iE 'T0 .*run start' "$PACE_FILE" \
    | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z' \
    | head -1 || true)"
fi

# Convert an ISO-8601 Z timestamp to epoch seconds (GNU date or BSD date).
iso_to_epoch() {
  date -u -d "$1" +%s 2>/dev/null || date -u -jf '%Y-%m-%dT%H:%M:%SZ' "$1" +%s 2>/dev/null || echo ""
}

REMAINING=$((TOTAL - DONE_COUNT))
PACE_LINE="Pace: T0 not parseable from \`${PACE_FILE}\` — elapsed unknown (**${DONE_COUNT} / ${TOTAL}** items done)."
PROJECTION="Projected drain: n/a (no T0)."
if [ -n "$T0_ISO" ]; then
  T0_EPOCH="$(iso_to_epoch "$T0_ISO")"
  if [ -n "$T0_EPOCH" ]; then
    ELAPSED_S=$((NOW_EPOCH - T0_EPOCH))
    [ "$ELAPSED_S" -lt 0 ] && ELAPSED_S=0
    ELAPSED_MIN=$((ELAPSED_S / 60))
    PACE_LINE="Pace: **${DONE_COUNT} / ${TOTAL}** items done in **${ELAPSED_MIN} min** elapsed since T0 (\`${T0_ISO}\`)."
    if [ "$DONE_COUNT" -gt 0 ] && [ "$ELAPSED_MIN" -gt 0 ]; then
      # Minutes-per-done-item; project minutes to drain the remaining backlog.
      MIN_PER_ITEM=$((ELAPSED_MIN / DONE_COUNT))
      [ "$MIN_PER_ITEM" -lt 1 ] && MIN_PER_ITEM=1
      PROJ_MIN=$((REMAINING * MIN_PER_ITEM))
      PROJECTION="Projected drain: at the current rate (~${MIN_PER_ITEM} min/item) the remaining ${REMAINING} items finish in ~${PROJ_MIN} min."
    else
      PROJECTION="Projected drain: n/a (no completed items yet)."
    fi
  fi
fi

# ---------------------------------------------------------------------------
# Compose the dashboard markdown.
# ---------------------------------------------------------------------------
STAMP="$(date -u '+%Y%m%dT%H%M%SZ')"
DASH_FILE="${REPORTS_DIR}/dashboard-${STAMP}.md"

{
  echo "# Board / Pace Dashboard — ${NOW_ISO}"
  echo ""
  echo "> Auto-generated by \`scripts/board/dashboard.sh\`. Read-only snapshot; the"
  echo "> board files are the source of truth. Regenerated each grader cycle"
  echo "> (see docs/process/epoch-recycling.md)."
  echo ""
  echo "## Item counts by status"
  echo ""
  echo "| status | count |"
  echo "|--------|------:|"
  for s in $STATUSES; do
    c="${STATUS_COUNT[$s]:-0}"
    # Skip dropped if zero to keep the table tight; always show the core states.
    if [ "$s" = "dropped" ] && [ "$c" -eq 0 ]; then continue; fi
    echo "| ${s} | ${c} |"
  done
  echo "| **total** | **${TOTAL}** |"
  echo ""
  echo "## Item counts by epic"
  echo ""
  echo "| epic | count |"
  echo "|------|------:|"
  for e in $(printf '%s\n' "${!EPIC_COUNT[@]}" | sort); do
    echo "| ${e} | ${EPIC_COUNT[$e]} |"
  done
  echo ""
  echo "## Pace"
  echo ""
  echo "${PACE_LINE}"
  echo ""
  echo "${PROJECTION}"
  echo ""
  echo "## Latest rubric score"
  echo ""
  echo "| latest report | overall |"
  echo "|---------------|--------:|"
  echo "| \`${LATEST_REPORT_NAME}\` | ${LATEST_SCORE} |"
  echo ""
  echo "## Blockers (status: blocked)"
  echo ""
  if [ -n "$BLOCKERS" ]; then
    echo "| id | title | epic |"
    echo "|----|-------|------|"
    printf '%s' "$BLOCKERS"
  else
    echo "_None — no items are blocked._"
  fi
  echo ""
} > "$DASH_FILE"

# Print the path so callers (and tests) can locate the generated file.
echo "$DASH_FILE"
