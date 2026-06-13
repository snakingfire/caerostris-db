#!/usr/bin/env bash
# scripts/pr/land.sh — integrator helper: land a signed-off PR onto main
#
# Usage: land.sh <ID>
#   ID — board item ID (e.g. T-0142)
#
# ONLY the integrator runs this. Merges are serialized — land PRs one at a
# time in rubric-weight order (higher-weight gates first).
#
# What this does (in order):
#   1. Locate the worktree and its branch for <ID>.
#   2. Verify PR.md sign-offs (adversarial-reviewer + premortem-analyst).
#   3. Run ./format_code.sh inside the worktree (aborts on failure).
#   4. Run cargo test inside the worktree (aborts on failure).
#   5. Merge the branch into main (--no-ff, descriptive message).
#   6. Remove the worktree and delete the branch.
#
# REFUSES to land if sign-offs are unchecked or checks fail.
# Never uses: reset --hard, push --force, or deletes branches not owned by this PR.
#
# See: docs/process/simulated-pr-workflow.md

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
WORKTREES_ROOT="${REPO_ROOT}/.worktrees"

usage() {
  echo "Usage: $(basename "$0") <ID>" >&2
  echo ""
  echo "  NOTE: Only the integrator runs this. Merges onto main are serialized." >&2
  echo "  Land one PR at a time; prefer higher rubric-weight gate items first." >&2
  echo ""
  echo "  Examples:" >&2
  echo "    $(basename "$0") T-0142" >&2
  echo "    $(basename "$0") BUG-0007" >&2
  exit 1
}

die() {
  echo "ABORT: $*" >&2
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

echo "=== land.sh: integrator landing for ID=$ID ==="
echo "NOTE: Only the integrator runs this. Merges to main are serialized."
echo ""

# ---------------------------------------------------------------------------
# 1. Locate the worktree for this ID.
# ---------------------------------------------------------------------------
WORKTREE_PATH="${WORKTREES_ROOT}/${ID}"

if [ ! -d "$WORKTREE_PATH" ]; then
  die "Worktree not found: $WORKTREE_PATH
       Has scripts/pr/open.sh been run for '$ID'?"
fi

PR_FILE="${WORKTREE_PATH}/PR.md"
if [ ! -f "$PR_FILE" ]; then
  die "PR.md not found in worktree: $PR_FILE
       Cannot verify sign-offs. Was PR.md scaffolded by open.sh?"
fi

# ---------------------------------------------------------------------------
# 2. Determine the branch name from the worktree.
# ---------------------------------------------------------------------------
BRANCH="$(git -C "$WORKTREE_PATH" rev-parse --abbrev-ref HEAD 2>/dev/null || true)"
if [ -z "$BRANCH" ] || [ "$BRANCH" = "HEAD" ]; then
  die "Could not determine branch name from worktree at $WORKTREE_PATH"
fi

# Safety: refuse to land if the branch is 'main' itself.
if [ "$BRANCH" = "main" ]; then
  die "Worktree is already on 'main'. Something is wrong — abort."
fi

echo "Worktree: $WORKTREE_PATH"
echo "Branch:   $BRANCH"
echo ""

# ---------------------------------------------------------------------------
# 2. Verify PR.md sign-off checkboxes are checked.
# ---------------------------------------------------------------------------
echo "--- Checking PR.md sign-offs ---"

check_box() {
  local label="$1"
  local pattern="$2"
  if grep -qE "$pattern" "$PR_FILE"; then
    echo "  [x] $label"
  else
    echo "  [ ] $label  <-- UNCHECKED" >&2
    return 1
  fi
}

SIGNOFF_OK=true

# Look for checked reviewer sign-off: "- [x] adversarial-reviewer sign-off"
if ! grep -qiE '^\s*-\s*\[x\]\s*adversarial-reviewer sign-off' "$PR_FILE"; then
  echo "  FAIL: adversarial-reviewer sign-off is not checked in PR.md." >&2
  SIGNOFF_OK=false
else
  echo "  [x] adversarial-reviewer sign-off"
fi

# Look for checked pre-mortem sign-off.
if ! grep -qiE '^\s*-\s*\[x\]\s*premortem-analyst sign-off' "$PR_FILE"; then
  echo "  FAIL: premortem-analyst sign-off is not checked in PR.md." >&2
  SIGNOFF_OK=false
else
  echo "  [x] premortem-analyst sign-off"
fi

if [ "$SIGNOFF_OK" = false ]; then
  echo "" >&2
  die "Sign-offs are not complete.
     Both adversarial-reviewer and premortem-analyst must have verdict: approve
     in PR.md (checkboxes must be checked: [x]) before landing.
     See: docs/process/adversarial-review-loops.md"
fi

echo ""

# ---------------------------------------------------------------------------
# 3. Run ./format_code.sh inside the worktree.
# ---------------------------------------------------------------------------
echo "--- Running ./format_code.sh ---"
FORMAT_SCRIPT="${REPO_ROOT}/format_code.sh"

if [ ! -f "$FORMAT_SCRIPT" ]; then
  die "format_code.sh not found at: $FORMAT_SCRIPT"
fi

# Run format_code.sh from the worktree directory so cargo/taplo find the right files.
(cd "$WORKTREE_PATH" && bash "${FORMAT_SCRIPT}") \
  || die "./format_code.sh failed. Fix fmt/clippy issues in the worktree and try again."

echo "format_code.sh: OK"
echo ""

# ---------------------------------------------------------------------------
# 4. Run cargo test inside the worktree.
# ---------------------------------------------------------------------------
echo "--- Running cargo test ---"
(cd "$WORKTREE_PATH" && cargo test) \
  || die "'cargo test' failed. All tests must be green before landing."

echo "cargo test: OK"
echo ""

# ---------------------------------------------------------------------------
# 5. Merge branch into main (--no-ff).
# ---------------------------------------------------------------------------
echo "--- Merging $BRANCH into main ---"

# Get a short SHA for the merge message.
TIP_SHA="$(git -C "$WORKTREE_PATH" rev-parse --short HEAD)"

# Ensure main is checked out in the main worktree (repo root).
CURRENT_BRANCH="$(git -C "$REPO_ROOT" rev-parse --abbrev-ref HEAD 2>/dev/null || true)"
if [ "$CURRENT_BRANCH" != "main" ]; then
  die "The main worktree is not on 'main' (it is on '$CURRENT_BRANCH').
       The integrator must run land.sh from a checkout whose HEAD is main."
fi

MERGE_MSG="Merge work/${ID}: ${BRANCH}

Board item: ${ID}
Tip SHA:    ${TIP_SHA}
Landed by:  scripts/pr/land.sh

All checks passed: format_code.sh green, cargo test green.
Both review-gate sign-offs present in PR.md (adversarial-reviewer + premortem-analyst)."

git -C "$REPO_ROOT" merge --no-ff "$BRANCH" -m "$MERGE_MSG" \
  || die "Merge of '$BRANCH' into main failed.
         Likely a rebase conflict. Return the branch to the worker:
           the worker must: git fetch, rebase onto main, re-run checks,
           and re-request review (review-gate checkboxes reset to unchecked)."

MERGE_SHA="$(git -C "$REPO_ROOT" rev-parse --short HEAD)"
echo "Merged: $BRANCH -> main at $MERGE_SHA"
echo ""

# ---------------------------------------------------------------------------
# 6. Clean up worktree and branch.
# ---------------------------------------------------------------------------
echo "--- Cleaning up worktree and branch ---"

# Remove the worktree first (this makes the branch non-checked-out).
git -C "$REPO_ROOT" worktree remove "$WORKTREE_PATH" \
  || {
    echo "Warning: 'git worktree remove' failed; attempting --force." >&2
    git -C "$REPO_ROOT" worktree remove --force "$WORKTREE_PATH" \
      || echo "Warning: could not remove worktree at $WORKTREE_PATH — remove manually." >&2
  }

# Delete the branch (this is the PR's own branch — not others' branches).
git -C "$REPO_ROOT" branch -d "$BRANCH" \
  || {
    echo "Warning: 'git branch -d $BRANCH' failed (branch may not be fully merged)." >&2
    echo "         Investigate before deleting with -D; do not force-delete blindly." >&2
  }

echo "Worktree removed: $WORKTREE_PATH"
echo "Branch deleted:   $BRANCH"
echo ""

# ---------------------------------------------------------------------------
# Done.
# ---------------------------------------------------------------------------
echo "=== LANDED: ${ID} -> main @ ${MERGE_SHA} ==="
echo ""
echo "Board hygiene:"
echo "  - Update .project/board/tasks/<${ID}>-*.md:"
echo "      status: done"
echo "      updated: $(date -u '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || date -u)"
echo "    Append to Notes/log: 'Landed at ${MERGE_SHA}'"
echo "  - Commit the board update: git add .project/board && git commit -m 'board: close ${ID} (landed ${MERGE_SHA})'"
echo ""
echo "The integrator serializes all merges. Land the next ready PR in rubric-weight order."
