#!/usr/bin/env bash
# scripts/board/new-task.sh — scaffold a new board task from the template
#
# Usage: new-task.sh <ID> "<title>"
#   ID    — board item ID (e.g. T-0142, BUG-0007, SPIKE-0003)
#   title — one-line imperative title (quoted)
#
# Copies .project/board/_templates/task.md to
#   .project/board/tasks/<ID>-<slug>.md
# where <slug> is the title lowercased with non-alphanumerics replaced by '-'.
# Refuses to overwrite an existing file.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TEMPLATE="${REPO_ROOT}/.project/board/_templates/task.md"
TASKS_DIR="${REPO_ROOT}/.project/board/tasks"

usage() {
  echo "Usage: $(basename "$0") <ID> \"<title>\"" >&2
  echo "  Examples:" >&2
  echo "    $(basename "$0") T-0142 \"Add B-tree secondary index on text properties\"" >&2
  echo "    $(basename "$0") BUG-0007 \"Fix manifest race on concurrent writes\"" >&2
  exit 1
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  usage
fi

if [ "$#" -ne 2 ]; then
  echo "Error: expected exactly 2 arguments, got $#." >&2
  usage
fi

ID="$1"
TITLE="$2"

# Validate ID format loosely (non-empty, no whitespace, no path separators).
case "$ID" in
  *' '*|*'/'*|*$'\t'*)
    echo "Error: ID must not contain spaces, tabs, or slashes: '$ID'" >&2
    exit 1
    ;;
esac

if [ -z "$TITLE" ]; then
  echo "Error: title must not be empty." >&2
  exit 1
fi

# Build slug: lowercase, replace runs of non-alphanumeric chars with '-',
# strip leading/trailing '-'.
SLUG="$(printf '%s' "$TITLE" \
  | tr '[:upper:]' '[:lower:]' \
  | sed 's/[^a-z0-9]\{1,\}/-/g' \
  | sed 's/^-//; s/-$//')"

if [ -z "$SLUG" ]; then
  echo "Error: title produced an empty slug after sanitisation." >&2
  exit 1
fi

DEST="${TASKS_DIR}/${ID}-${SLUG}.md"

# Guard: refuse to overwrite.
if [ -e "$DEST" ]; then
  echo "Error: file already exists: $DEST" >&2
  echo "Choose a different ID or title, or edit the existing file." >&2
  exit 1
fi

# Guard: template must exist.
if [ ! -f "$TEMPLATE" ]; then
  echo "Error: template not found: $TEMPLATE" >&2
  exit 1
fi

# Ensure tasks directory exists.
mkdir -p "$TASKS_DIR"

# Determine creation timestamp (ISO date).
CREATED="$(date -u '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || date -u)"

# Copy template and substitute id + title + created/updated placeholders.
# Uses sed with a delimiter that won't appear in the values.
sed \
  -e "s|^id:.*|id: ${ID}|" \
  -e "s|^title:.*|title: ${TITLE}|" \
  -e "s|^created:.*|created: ${CREATED}|" \
  -e "s|^updated:.*|updated: ${CREATED}|" \
  "$TEMPLATE" > "$DEST"

echo "Created: $DEST"
