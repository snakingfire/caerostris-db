#!/usr/bin/env bash
# scripts/board/ls.sh — list board tasks from .project/board/tasks/
#
# Usage: ls.sh [status] [priority]
#   status   — filter by status field (e.g. ready, in_progress, done)
#   priority — filter by priority field (e.g. P0, P1, P2, P3)
#
# Prints a compact aligned table: ID | type | status | prio | assignee | title
# Sorted by priority (P0 first) then ID.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TASKS_DIR="${REPO_ROOT}/.project/board/tasks"

usage() {
  echo "Usage: $(basename "$0") [status] [priority]" >&2
  echo "  Examples:" >&2
  echo "    $(basename "$0")                    # list all items" >&2
  echo "    $(basename "$0") ready              # only ready items" >&2
  echo "    $(basename "$0") ready P1           # ready P1 items" >&2
  echo "    $(basename "$0") in_progress        # everything in flight" >&2
  exit 1
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  usage
fi

if [ "$#" -gt 2 ]; then
  echo "Error: too many arguments." >&2
  usage
fi

FILTER_STATUS="${1:-}"
FILTER_PRIORITY="${2:-}"

if [ ! -d "$TASKS_DIR" ]; then
  echo "Error: tasks directory not found: $TASKS_DIR" >&2
  exit 1
fi

# Extract a single YAML frontmatter field from a file.
# Uses only the first occurrence (between the first pair of --- delimiters).
# extract_field <file> <field>
extract_field() {
  local file="$1"
  local field="$2"
  # Read lines between first and second --- markers; grep for the field.
  awk '
    /^---/ { count++; if (count == 2) exit; next }
    count == 1 { print }
  ' "$file" | grep -m1 "^${field}:" | sed "s/^${field}:[[:space:]]*//" | tr -d '\r'
}

# Priority sort key: P0→0, P1→1, P2→2, P3→3, else 9
prio_key() {
  case "$1" in
    P0) echo 0 ;;
    P1) echo 1 ;;
    P2) echo 2 ;;
    P3) echo 3 ;;
    *)  echo 9 ;;
  esac
}

# Collect rows into a temp file for sorting.
TMPFILE="$(mktemp)"
trap 'rm -f "$TMPFILE"' EXIT

for file in "${TASKS_DIR}"/*.md; do
  [ -f "$file" ] || continue

  id="$(extract_field "$file" id)"
  title="$(extract_field "$file" title)"
  type="$(extract_field "$file" type)"
  status="$(extract_field "$file" status)"
  priority="$(extract_field "$file" priority)"
  assignee="$(extract_field "$file" assignee)"

  # Skip items with missing id (malformed files).
  [ -z "$id" ] && continue

  # Apply filters.
  if [ -n "$FILTER_STATUS" ] && [ "$status" != "$FILTER_STATUS" ]; then
    continue
  fi
  if [ -n "$FILTER_PRIORITY" ] && [ "$priority" != "$FILTER_PRIORITY" ]; then
    continue
  fi

  pk="$(prio_key "$priority")"
  # Sanitise fields for display (replace empty with '-').
  id="${id:--}"
  type="${type:--}"
  status="${status:--}"
  priority="${priority:--}"
  assignee="${assignee:--}"
  title="${title:--}"

  # Write sort key + tab-delimited row.
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "${pk}${id}" "$id" "$type" "$status" "$priority" "$assignee" "$title" \
    >> "$TMPFILE"
done

if [ ! -s "$TMPFILE" ]; then
  echo "(no matching board items)"
  exit 0
fi

# Sort by priority key then ID; strip sort key; format with awk column alignment.
sort "$TMPFILE" | cut -f2- | awk -F'\t' '
BEGIN {
  hdr_id       = "ID"
  hdr_type     = "TYPE"
  hdr_status   = "STATUS"
  hdr_prio     = "PRIO"
  hdr_assignee = "ASSIGNEE"
  hdr_title    = "TITLE"
  n = 0
}
{
  id[n]       = $1
  typ[n]      = $2
  stat[n]     = $3
  prio[n]     = $4
  assignee[n] = $5
  title[n]    = $6
  n++

  # Track max column widths.
  if (length($1) > w_id)       w_id       = length($1)
  if (length($2) > w_type)     w_type     = length($2)
  if (length($3) > w_stat)     w_stat     = length($3)
  if (length($4) > w_prio)     w_prio     = length($4)
  if (length($5) > w_assign)   w_assign   = length($5)
}
END {
  # Ensure minimums for header widths.
  if (length(hdr_id)       > w_id)       w_id       = length(hdr_id)
  if (length(hdr_type)     > w_type)     w_type     = length(hdr_type)
  if (length(hdr_status)   > w_stat)     w_stat     = length(hdr_status)
  if (length(hdr_prio)     > w_prio)     w_prio     = length(hdr_prio)
  if (length(hdr_assignee) > w_assign)   w_assign   = length(hdr_assignee)

  fmt = "%-" w_id "s  %-" w_type "s  %-" w_stat "s  %-" w_prio "s  %-" w_assign "s  %s\n"

  printf fmt, hdr_id, hdr_type, hdr_status, hdr_prio, hdr_assignee, hdr_title

  # Separator line.
  sep = ""
  total = w_id + 2 + w_type + 2 + w_stat + 2 + w_prio + 2 + w_assign + 2 + 40
  for (i = 0; i < total; i++) sep = sep "-"
  print sep

  for (i = 0; i < n; i++) {
    printf fmt, id[i], typ[i], stat[i], prio[i], assignee[i], title[i]
  }
}
'
