#!/usr/bin/env bash
# scripts/pr/open.sh — open a simulated PR for a board item
#
# Usage: open.sh <ID>
#   ID — board item ID (e.g. T-0142, BUG-0007, SPIKE-0003)
#
# Creates a git worktree at .worktrees/<ID> on a new branch work/<ID>-<slug>
# based on the current tip of main, and scaffolds a PR.md there.
# The slug is derived from the board item's title if found, otherwise from ID.
#
# See: docs/process/simulated-pr-workflow.md
#      docs/process/adversarial-review-loops.md

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TASKS_DIR="${REPO_ROOT}/.project/board/tasks"
WORKTREES_ROOT="${REPO_ROOT}/.worktrees"

usage() {
  echo "Usage: $(basename "$0") <ID>" >&2
  echo "  Examples:" >&2
  echo "    $(basename "$0") T-0142" >&2
  echo "    $(basename "$0") SPIKE-0003" >&2
  exit 1
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  usage
fi

if [ "$#" -ne 1 ]; then
  echo "Error: expected exactly 1 argument, got $#." >&2
  usage
fi

ID="$1"

# Validate ID (no whitespace or path separators).
case "$ID" in
  *' '*|*'/'*|*$'\t'*)
    echo "Error: ID must not contain spaces, tabs, or slashes: '$ID'" >&2
    exit 1
    ;;
esac

# ---------------------------------------------------------------------------
# Locate the board item file (glob: <ID>-*.md or exactly <ID>.md)
# ---------------------------------------------------------------------------
BOARD_FILE=""
for f in "${TASKS_DIR}/${ID}"-*.md "${TASKS_DIR}/${ID}.md"; do
  [ -f "$f" ] && BOARD_FILE="$f" && break
done

# Extract a YAML frontmatter field from a file.
extract_field() {
  local file="$1"
  local field="$2"
  awk '
    /^---/ { count++; if (count == 2) exit; next }
    count == 1 { print }
  ' "$file" | grep -m1 "^${field}:" | sed "s/^${field}:[[:space:]]*//" | tr -d '\r'
}

# Extract all acceptance-criteria lines (unchecked checkboxes) from body.
extract_criteria() {
  local file="$1"
  awk '
    in_ac && /^- \[/ { print; next }
    /^## Acceptance criteria/ { in_ac=1; next }
    in_ac && /^## / { in_ac=0 }
  ' "$file"
}

TITLE=""
RUBRIC_REFS=""
CRITERIA_LINES=""

if [ -n "$BOARD_FILE" ]; then
  TITLE="$(extract_field "$BOARD_FILE" title)"
  RUBRIC_REFS="$(extract_field "$BOARD_FILE" rubric_refs)"
  CRITERIA_LINES="$(extract_criteria "$BOARD_FILE")"
  echo "Board item: $BOARD_FILE"
else
  echo "Warning: no board item file found for ID '$ID' in $TASKS_DIR" >&2
  echo "         Scaffolding a stub PR.md without board data." >&2
fi

# ---------------------------------------------------------------------------
# Derive slug from title (or ID if no title).
# ---------------------------------------------------------------------------
if [ -n "$TITLE" ]; then
  SLUG="$(printf '%s' "$TITLE" \
    | tr '[:upper:]' '[:lower:]' \
    | sed 's/[^a-z0-9]\{1,\}/-/g' \
    | sed 's/^-//; s/-$//' \
    | cut -c1-50)"
else
  SLUG="$(printf '%s' "$ID" \
    | tr '[:upper:]' '[:lower:]' \
    | sed 's/[^a-z0-9]\{1,\}/-/g' \
    | sed 's/^-//; s/-$//')"
fi

BRANCH="work/${ID}-${SLUG}"
WORKTREE_PATH="${WORKTREES_ROOT}/${ID}"

# ---------------------------------------------------------------------------
# Safety checks before mutating anything.
# ---------------------------------------------------------------------------
if [ -e "$WORKTREE_PATH" ]; then
  echo "Error: worktree path already exists: $WORKTREE_PATH" >&2
  echo "       Remove it manually or choose a different ID." >&2
  exit 1
fi

# Check the branch doesn't already exist.
if git -C "$REPO_ROOT" rev-parse --verify "refs/heads/$BRANCH" >/dev/null 2>&1; then
  echo "Error: branch '$BRANCH' already exists." >&2
  echo "       Delete it or rename to avoid collision." >&2
  exit 1
fi

# Ensure main exists.
if ! git -C "$REPO_ROOT" rev-parse --verify "refs/heads/main" >/dev/null 2>&1; then
  echo "Error: 'main' branch not found in this repository." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Create the worktree + branch.
# ---------------------------------------------------------------------------
mkdir -p "$WORKTREES_ROOT"

echo "Creating worktree at: $WORKTREE_PATH"
echo "Branch: $BRANCH (based on main)"
git -C "$REPO_ROOT" worktree add -b "$BRANCH" "$WORKTREE_PATH" main

# ---------------------------------------------------------------------------
# Scaffold PR.md inside the worktree.
# ---------------------------------------------------------------------------
PR_FILE="${WORKTREE_PATH}/PR.md"

# Build acceptance-criteria section: use board criteria or a placeholder.
if [ -n "$CRITERIA_LINES" ]; then
  AC_SECTION="$CRITERIA_LINES"
else
  AC_SECTION="- [ ] (fill in acceptance criteria from the board item)"
fi

# Rubric refs display.
RUBRIC_DISPLAY="${RUBRIC_REFS:-"(fill in rubric category numbers, e.g. Cat 5, Cat 3)"}"

# Board item link.
if [ -n "$BOARD_FILE" ]; then
  # Relative path from worktree root back to repo root tasks dir.
  BOARD_LINK="$(realpath --relative-to="$WORKTREE_PATH" "$BOARD_FILE" 2>/dev/null \
    || echo "${BOARD_FILE#"${REPO_ROOT}/"}")"
  BOARD_ITEM_LINE="[$BOARD_FILE]($BOARD_LINK)"
else
  BOARD_ITEM_LINE="(board item not found — update this link manually)"
fi

cat > "$PR_FILE" <<EOF
# PR: ${ID} — ${TITLE:-"(no title — fill in)"}

## Board item

${BOARD_ITEM_LINE}

## Rubric refs

<!-- Cat numbers from docs/requirements/master-rubric.md this change advances. -->
${RUBRIC_DISPLAY}

## Acceptance criteria (from board item)

${AC_SECTION}

## Summary of change

<!-- What changed and why — 3–8 sentences. Reference the design/ADR if one exists. -->
(fill in)

## Test evidence

<!-- Paste or link the output of: cargo nextest run, cargo llvm-cov, ./format_code.sh -->
<!-- At minimum: test count, coverage %, any benchmark numbers relevant to the change. -->
(fill in before requesting review)

## Review gate

- [ ] adversarial-reviewer sign-off (see docs/process/adversarial-review-loops.md)
- [ ] premortem-analyst sign-off (see docs/process/adversarial-review-loops.md)
- [ ] \`./format_code.sh\` green
- [ ] \`cargo nextest run\` green (or \`cargo test\` outside Nix shell)
- [ ] coverage not regressed
- [ ] board item updated to \`in_review\`

<!-- Reviewers: append your verdict block below this line per adversarial-review-loops.md -->
EOF

echo ""
echo "PR.md scaffolded at: $PR_FILE"
echo ""
echo "Next steps:"
echo "  1.  cd $WORKTREE_PATH"
echo "  2.  Implement the task TDD-first; commit in logical slices."
echo "  3.  Fill in PR.md (Summary, Test evidence, Acceptance criteria)."
echo "  4.  Update the board item to status: in_review."
echo "  5.  Dispatch adversarial-reviewer and premortem-analyst concurrently."
echo "  6.  Address blocking findings; get both verdicts to 'approve'."
echo "  7.  Call: scripts/pr/land.sh $ID"
