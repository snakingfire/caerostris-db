#!/usr/bin/env bash
# scripts/release-hourly.sh — cut an hourly release of caerostris-db
#
# Usage: release-hourly.sh [N]
#   N — optional: explicit hourly release number (integer >= 1).
#       If omitted, auto-determines the next number from existing hourly-* tags.
#
# What this does:
#   1. Verify clean/green working tree (no uncommitted changes).
#   2. Run cargo test.
#   3. cargo build --release.
#   4. (Best-effort) build Python wheel if a bindings crate exists.
#   5. Determine the release number N (or use the provided N).
#   6. Create an annotated git tag 'hourly-<N>'.
#   7. Write releases/hourly-<N>.md with timestamp, SHA, board items landed
#      since the last hourly tag, and a pointer to the latest rubric report.
#   8. Commit releases/hourly-<N>.md.
#
# Binaries and wheels are NOT committed (they are build artifacts, gitignored).
#
# See: docs/process/autonomous-operating-model.md (every ~60 min cadence)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
  echo "Usage: $(basename "$0") [N]" >&2
  echo "  N — optional explicit hourly release number (integer >= 1)" >&2
  echo "  If omitted, auto-determines the next number from git tags." >&2
  exit 1
}

die() {
  echo "ABORT: $*" >&2
  exit 1
}

warn() {
  echo "WARNING: $*" >&2
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  usage
fi

if [ "$#" -gt 1 ]; then
  echo "Error: too many arguments." >&2
  usage
fi

EXPLICIT_N="${1:-}"
if [ -n "$EXPLICIT_N" ]; then
  case "$EXPLICIT_N" in
    ''|*[!0-9]*)
      echo "Error: N must be a positive integer, got: '$EXPLICIT_N'" >&2
      usage
      ;;
  esac
  if [ "$EXPLICIT_N" -lt 1 ]; then
    echo "Error: N must be >= 1." >&2
    usage
  fi
fi

cd "$REPO_ROOT"

echo "=== release-hourly.sh ==="
echo ""

# ---------------------------------------------------------------------------
# 1. Verify clean working tree (no uncommitted changes).
# ---------------------------------------------------------------------------
echo "--- Checking working tree ---"

# Untracked files don't block a release, but staged/modified do.
if ! git diff --quiet HEAD 2>/dev/null; then
  die "Working tree has uncommitted changes.
       Commit or stash all changes before cutting a release.
       (Hint: board commits, release notes, etc. must be committed first.)"
fi

# Check for staged changes.
if ! git diff --cached --quiet 2>/dev/null; then
  die "There are staged changes not yet committed.
       Commit them before cutting a release."
fi

echo "Working tree: clean"
echo ""

# ---------------------------------------------------------------------------
# 2. Run cargo test.
# ---------------------------------------------------------------------------
echo "--- Running cargo test ---"
cargo test || die "'cargo test' failed. All tests must be green before a release."
echo "cargo test: OK"
echo ""

# ---------------------------------------------------------------------------
# 3. cargo build --release.
# ---------------------------------------------------------------------------
echo "--- Building release binary ---"
cargo build --release || die "'cargo build --release' failed."
RELEASE_BINARY="$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
  | grep -o '"name":"[^"]*"' | head -1 | sed 's/"name":"//;s/"//' || true)"
echo "cargo build --release: OK"
[ -n "$RELEASE_BINARY" ] && echo "  Binary: target/release/${RELEASE_BINARY} (not committed)"
echo ""

# ---------------------------------------------------------------------------
# 4. Best-effort Python wheel build.
# ---------------------------------------------------------------------------
echo "--- Python wheel (best-effort) ---"
WHEEL_BUILT=false
WHEEL_SKIP_REASON=""

# Look for a bindings crate (common naming: caerostris-py, python, bindings).
BINDINGS_CRATE=""
for candidate in caerostris-py python bindings caerostris_py; do
  if [ -d "${REPO_ROOT}/${candidate}" ] && [ -f "${REPO_ROOT}/${candidate}/Cargo.toml" ]; then
    BINDINGS_CRATE="$candidate"
    break
  fi
done

if [ -z "$BINDINGS_CRATE" ]; then
  WHEEL_SKIP_REASON="no bindings crate found (looked for: caerostris-py, python, bindings)"
  warn "$WHEEL_SKIP_REASON"
else
  echo "  Found bindings crate: $BINDINGS_CRATE"
  if command -v maturin >/dev/null 2>&1; then
    (cd "${REPO_ROOT}/${BINDINGS_CRATE}" && maturin build --release 2>&1) \
      && WHEEL_BUILT=true \
      || { WHEEL_SKIP_REASON="maturin build failed (see output above)"; warn "$WHEEL_SKIP_REASON"; }
  else
    WHEEL_SKIP_REASON="maturin not found in PATH"
    warn "$WHEEL_SKIP_REASON"
  fi
fi

if [ "$WHEEL_BUILT" = true ]; then
  echo "  Wheel built (not committed — it is a build artifact)."
else
  echo "  Skipped: $WHEEL_SKIP_REASON"
fi
echo ""

# ---------------------------------------------------------------------------
# 5. Determine release number N.
# ---------------------------------------------------------------------------
echo "--- Determining release number ---"

if [ -n "$EXPLICIT_N" ]; then
  N="$EXPLICIT_N"
  echo "  Using explicit N=$N"
else
  # Find the highest existing hourly-* tag.
  LAST_N="$(git tag -l 'hourly-*' \
    | sed 's/hourly-//' \
    | grep -E '^[0-9]+$' \
    | sort -n \
    | tail -1 || true)"
  if [ -z "$LAST_N" ]; then
    N=1
    echo "  No previous hourly tags found. Starting at N=1."
  else
    N=$((LAST_N + 1))
    echo "  Previous: hourly-${LAST_N}. Next: N=${N}."
  fi
fi

TAG="hourly-${N}"

# Refuse to overwrite an existing tag.
if git rev-parse "$TAG" >/dev/null 2>&1; then
  die "Tag '$TAG' already exists. Use a different N or delete the tag first."
fi

echo ""

# ---------------------------------------------------------------------------
# 6. Determine git SHA and gather landed board items since last hourly tag.
# ---------------------------------------------------------------------------
CURRENT_SHA="$(git rev-parse HEAD)"
SHORT_SHA="$(git rev-parse --short HEAD)"
TIMESTAMP="$(date -u '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || date -u)"

# Board items landed since last hourly tag (git log commit messages).
LAST_TAG=""
LAST_TAG_SHA=""
if [ -n "${LAST_N:-}" ]; then
  LAST_TAG="hourly-${LAST_N}"
  LAST_TAG_SHA="$(git rev-parse "$LAST_TAG" 2>/dev/null || true)"
fi

if [ -n "$LAST_TAG_SHA" ]; then
  RANGE="${LAST_TAG_SHA}..HEAD"
  echo "--- Board items landed since $LAST_TAG ---"
else
  RANGE="HEAD"
  echo "--- All commits (no previous hourly tag) ---"
fi

# Collect board: prefixed commits and merge commits referencing board IDs.
LANDED_ITEMS="$(git log "$RANGE" --oneline --no-merges 2>/dev/null \
  | grep -iE '(board:|T-[0-9]+|EPIC-[0-9]+|BUG-[0-9]+|SPIKE-[0-9]+|STORY-[0-9]+|Merge work/)' \
  || true)"

MERGE_COMMITS="$(git log "$RANGE" --oneline --merges 2>/dev/null \
  | grep -iE '(work/|T-[0-9]+|EPIC-[0-9]+|BUG-[0-9]+|SPIKE-[0-9]+)' \
  || true)"

ALL_COMMITS="$(git log "$RANGE" --oneline 2>/dev/null || true)"

echo "$ALL_COMMITS" | head -20 || true
echo ""

# ---------------------------------------------------------------------------
# 7. Find latest rubric report.
# ---------------------------------------------------------------------------
REPORTS_DIR="${REPO_ROOT}/.project/reports"
LATEST_REPORT=""
if [ -d "$REPORTS_DIR" ]; then
  LATEST_REPORT="$(ls -t "${REPORTS_DIR}"/*.md 2>/dev/null | head -1 || true)"
fi

REPORT_LINK=""
if [ -n "$LATEST_REPORT" ]; then
  REPORT_LINK="${LATEST_REPORT#"${REPO_ROOT}/"}"
else
  REPORT_LINK="(no rubric report found in .project/reports/ yet)"
fi

# ---------------------------------------------------------------------------
# 8. Write releases/hourly-<N>.md.
# ---------------------------------------------------------------------------
RELEASES_DIR="${REPO_ROOT}/releases"
mkdir -p "$RELEASES_DIR"
RELEASE_NOTES="${RELEASES_DIR}/hourly-${N}.md"

if [ -e "$RELEASE_NOTES" ]; then
  die "Release notes file already exists: $RELEASE_NOTES"
fi

{
  echo "# Hourly Release ${N}"
  echo ""
  echo "| Field | Value |"
  echo "|-------|-------|"
  echo "| Tag | \`${TAG}\` |"
  echo "| Timestamp | ${TIMESTAMP} |"
  echo "| Git SHA | \`${CURRENT_SHA}\` |"
  echo "| Short SHA | \`${SHORT_SHA}\` |"
  if [ -n "$LAST_TAG" ]; then
    echo "| Previous release | \`${LAST_TAG}\` |"
  fi
  echo ""
  echo "## Board items landed since previous release"
  echo ""
  if [ -n "$MERGE_COMMITS" ]; then
    echo "### Merged branches"
    echo '```'
    echo "$MERGE_COMMITS"
    echo '```'
    echo ""
  fi
  if [ -n "$LANDED_ITEMS" ]; then
    echo "### Board-related commits"
    echo '```'
    echo "$LANDED_ITEMS"
    echo '```'
    echo ""
  fi
  echo "### All commits in range"
  echo '```'
  if [ -n "$ALL_COMMITS" ]; then
    echo "$ALL_COMMITS"
  else
    echo "(no commits in range)"
  fi
  echo '```'
  echo ""
  echo "## Build artifacts (not committed — build from source)"
  echo ""
  echo "- Release binary: \`cargo build --release\`"
  if [ "$WHEEL_BUILT" = true ]; then
    echo "- Python wheel: built successfully (see \`target/wheels/\`)"
  else
    echo "- Python wheel: skipped — ${WHEEL_SKIP_REASON}"
  fi
  echo ""
  echo "## Rubric grader report"
  echo ""
  echo "Latest report: \`${REPORT_LINK}\`"
  echo ""
  echo "> See [docs/requirements/master-rubric.md](../docs/requirements/master-rubric.md)"
  echo "> for scoring methodology and category definitions."
} > "$RELEASE_NOTES"

echo "Release notes written: $RELEASE_NOTES"
echo ""

# ---------------------------------------------------------------------------
# 9. Commit the release notes.
# ---------------------------------------------------------------------------
echo "--- Committing release notes ---"
git add "$RELEASE_NOTES"
git commit -m "release: cut hourly-${N} at ${SHORT_SHA}

Timestamp: ${TIMESTAMP}
Tag: ${TAG}

$([ -n "$LAST_TAG" ] && echo "Since: ${LAST_TAG}" || echo "First hourly release")
"
echo "Release notes committed."
echo ""

# ---------------------------------------------------------------------------
# 10. Create annotated git tag.
# ---------------------------------------------------------------------------
echo "--- Creating annotated tag: $TAG ---"
git tag -a "$TAG" -m "hourly release ${N}

Timestamp: ${TIMESTAMP}
SHA: ${CURRENT_SHA}
Release notes: releases/hourly-${N}.md
$([ -n "$LAST_TAG" ] && echo "Since: ${LAST_TAG}" || echo "First hourly release")

cargo test: PASS
cargo build --release: PASS
Python wheel: $([ "$WHEEL_BUILT" = true ] && echo "built" || echo "skipped — ${WHEEL_SKIP_REASON}")
"
echo "Tag created: $TAG -> $(git rev-parse --short "$TAG")"
echo ""

# ---------------------------------------------------------------------------
# Done.
# ---------------------------------------------------------------------------
echo "=== RELEASE COMPLETE: $TAG at $SHORT_SHA ==="
echo ""
echo "Release notes: $RELEASE_NOTES"
echo "Rubric report: $REPORT_LINK"
echo ""
echo "Reminder: binaries and wheels are NOT committed — they are build artifacts."
echo "          Push the tag when ready: git push origin $TAG"
