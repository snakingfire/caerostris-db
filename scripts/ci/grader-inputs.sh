#!/usr/bin/env bash
# scripts/ci/grader-inputs.sh — emit the GRADER_INPUTS block + enforce the
# coverage threshold gate.
#
# The rubric-grader's evidence-scraper parses a structured block from CI logs:
#
#   GRADER_INPUTS:
#     coverage_pct: <N>
#     test_pass: <pass>/<total>
#     tck_pass_rate: <X>/<Y>
#
# This script assembles that block from the numbers CI already computed
# (coverage% from cargo-llvm-cov, test pass/total from cargo nextest, TCK
# pass/total from the TCK results JSON) and prints it to stdout. It then fails
# the build (exit 1) if coverage% is below the configured threshold.
#
# The threshold starts at 0 (so the empty/early crate is not blocked) and is
# ratcheted up toward 90 as real tests land — see docs/process/ci-grader-inputs.md.
#
# Usage:
#   grader-inputs.sh --coverage <pct> --threshold <pct> \
#                    --test-pass <n> --test-total <n> \
#                    --tck-json <path>
#
# All flags are optional; sensible defaults keep the block well-formed even when
# a signal is not yet available (e.g. before the TCK harness lands).

set -euo pipefail

COVERAGE="0"
THRESHOLD="0"
TEST_PASS="0"
TEST_TOTAL="0"
TCK_JSON=""

usage() {
  sed -n '2,30p' "$0" >&2
  exit "${1:-1}"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --coverage) COVERAGE="${2:?--coverage needs a value}"; shift 2 ;;
    --threshold) THRESHOLD="${2:?--threshold needs a value}"; shift 2 ;;
    --test-pass) TEST_PASS="${2:?--test-pass needs a value}"; shift 2 ;;
    --test-total) TEST_TOTAL="${2:?--test-total needs a value}"; shift 2 ;;
    --tck-json) TCK_JSON="${2:?--tck-json needs a value}"; shift 2 ;;
    -h|--help) usage 0 ;;
    *) echo "Error: unknown argument '$1'" >&2; usage 1 ;;
  esac
done

# ---------------------------------------------------------------------------
# Resolve the TCK pass/total from the JSON, degrading to 0/0 when absent or
# unparseable so a missing harness never crashes the gate.
# ---------------------------------------------------------------------------
TCK_PASS="0"
TCK_TOTAL="0"
if [ -n "$TCK_JSON" ] && [ -f "$TCK_JSON" ] && command -v jq >/dev/null 2>&1; then
  TCK_PASS="$(jq -r '(.pass // 0) | floor' "$TCK_JSON" 2>/dev/null || echo 0)"
  TCK_TOTAL="$(jq -r '(.total // 0) | floor' "$TCK_JSON" 2>/dev/null || echo 0)"
  # Guard against jq emitting "null" or empty on a malformed file.
  case "$TCK_PASS" in ''|*[!0-9]*) TCK_PASS="0" ;; esac
  case "$TCK_TOTAL" in ''|*[!0-9]*) TCK_TOTAL="0" ;; esac
fi

# ---------------------------------------------------------------------------
# Emit the block. This is the exact shape the grader scraper matches on.
# ---------------------------------------------------------------------------
cat <<EOF
GRADER_INPUTS:
  coverage_pct: ${COVERAGE}
  test_pass: ${TEST_PASS}/${TEST_TOTAL}
  tck_pass_rate: ${TCK_PASS}/${TCK_TOTAL}
EOF

# ---------------------------------------------------------------------------
# Coverage gate: fail the build when coverage < threshold. Uses awk for a
# float-safe comparison (coverage% and threshold may be fractional).
# ---------------------------------------------------------------------------
if awk -v c="$COVERAGE" -v t="$THRESHOLD" 'BEGIN { exit !(c + 0 < t + 0) }'; then
  echo "ERROR: line coverage ${COVERAGE}% is below the threshold ${THRESHOLD}%." >&2
  exit 1
fi

echo "OK: line coverage ${COVERAGE}% meets the threshold ${THRESHOLD}%." >&2
