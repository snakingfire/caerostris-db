#!/usr/bin/env bash
# scripts/ci/grader-inputs.test.sh — tests for grader-inputs.sh
#
# A tiny, dependency-light test harness (no bats needed) that exercises the
# behaviours the CI relies on: parsing the TCK JSON, emitting the GRADER_INPUTS
# block in the exact format the rubric-grader's scraper expects, and enforcing
# the coverage threshold gate. Run: bash scripts/ci/grader-inputs.test.sh
#
# Written test-first (TDD) before grader-inputs.sh existed.

set -u

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
UNDER_TEST="${SCRIPT_DIR}/grader-inputs.sh"

PASS=0
FAIL=0

# ---------------------------------------------------------------------------
# Assertion helpers.
# ---------------------------------------------------------------------------
fail() {
  FAIL=$((FAIL + 1))
  echo "  FAIL: $1"
}

ok() {
  PASS=$((PASS + 1))
  echo "  ok:   $1"
}

assert_contains() {
  # assert_contains <haystack> <needle> <desc>
  if printf '%s' "$1" | grep -qF -- "$2"; then
    ok "$3"
  else
    fail "$3 (missing: '$2')"
    echo "      output was:"
    printf '%s\n' "$1" | sed 's/^/      | /'
  fi
}

assert_eq() {
  # assert_eq <actual> <expected> <desc>
  if [ "$1" = "$2" ]; then
    ok "$3"
  else
    fail "$3 (expected '$2', got '$1')"
  fi
}

TMPDIR_T="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_T"' EXIT

# A representative TCK results JSON (the schema CI archives).
TCK_JSON="${TMPDIR_T}/tck.json"
cat >"$TCK_JSON" <<'JSON'
{
  "total": 120,
  "pass": 0,
  "pending": 120,
  "fail": 0,
  "pass_rate": 0.0
}
JSON

# ---------------------------------------------------------------------------
# Test 1: emits a GRADER_INPUTS block with all three required fields.
# ---------------------------------------------------------------------------
echo "test: emits GRADER_INPUTS block with all three fields"
OUT="$(bash "$UNDER_TEST" \
  --coverage 0 --threshold 0 \
  --test-pass 1 --test-total 1 \
  --tck-json "$TCK_JSON" 2>&1)"
assert_contains "$OUT" "GRADER_INPUTS:" "header present"
assert_contains "$OUT" "coverage_pct: 0" "coverage_pct present"
assert_contains "$OUT" "test_pass: 1/1" "test_pass present"
assert_contains "$OUT" "tck_pass_rate: 0/120" "tck_pass_rate parsed from json"

# ---------------------------------------------------------------------------
# Test 2: coverage at/above threshold => exit 0 (gate passes).
# ---------------------------------------------------------------------------
echo "test: coverage at threshold passes the gate"
bash "$UNDER_TEST" --coverage 90 --threshold 90 \
  --test-pass 1 --test-total 1 --tck-json "$TCK_JSON" >/dev/null 2>&1
assert_eq "$?" "0" "exit 0 when coverage == threshold"

bash "$UNDER_TEST" --coverage 95.5 --threshold 90 \
  --test-pass 1 --test-total 1 --tck-json "$TCK_JSON" >/dev/null 2>&1
assert_eq "$?" "0" "exit 0 when coverage > threshold (fractional)"

# ---------------------------------------------------------------------------
# Test 3: coverage below threshold => non-zero exit (gate fails the build).
# ---------------------------------------------------------------------------
echo "test: coverage below threshold fails the gate"
OUT="$(bash "$UNDER_TEST" --coverage 42.0 --threshold 90 \
  --test-pass 1 --test-total 1 --tck-json "$TCK_JSON" 2>&1)"
RC=$?
assert_eq "$RC" "1" "exit 1 when coverage < threshold"
assert_contains "$OUT" "GRADER_INPUTS:" "still emits the block before failing"

# ---------------------------------------------------------------------------
# Test 4: missing TCK json degrades gracefully to N/A, not a crash.
# ---------------------------------------------------------------------------
echo "test: missing TCK json degrades to 0/0 without crashing"
OUT="$(bash "$UNDER_TEST" --coverage 0 --threshold 0 \
  --test-pass 0 --test-total 0 \
  --tck-json "${TMPDIR_T}/does-not-exist.json" 2>&1)"
RC=$?
assert_eq "$RC" "0" "exit 0 even when tck json is absent"
assert_contains "$OUT" "tck_pass_rate: 0/0" "tck falls back to 0/0"

# ---------------------------------------------------------------------------
# Summary.
# ---------------------------------------------------------------------------
echo ""
echo "grader-inputs.test.sh: ${PASS} passed, ${FAIL} failed"
[ "$FAIL" -eq 0 ]
