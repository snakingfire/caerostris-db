#!/usr/bin/env bash
# check.sh — model-check commit_protocol.tla with TLC (Apalache fallback).
#
# Cat. 11 evidence reproducer. Apalache is preferred (see README.md) but is not
# yet in the Nix shell; this script uses TLC (tla2tools, EPL-2.0). It locates a
# JRE + tla2tools.jar, or tells you how to get them (both open-source,
# license-clean; never committed to the repo).
#
# Usage: ./check.sh            (runs SANY parse + safety + liveness + probes)
#        TLA_TOOLS=/path/to/tla2tools.jar JAVA=/path/to/java ./check.sh
#
# Exit non-zero if any safety/liveness check fails OR if a non-vacuity probe is
# NOT refuted (a probe that holds would mean the modelled behaviour is
# unreachable and the safety result vacuous).

set -euo pipefail
cd "$(dirname "$0")"

JAVA="${JAVA:-$(command -v java || true)}"
JAR="${TLA_TOOLS:-tla2tools.jar}"

if [ -z "${JAVA}" ] || ! "${JAVA}" -version >/dev/null 2>&1; then
  echo "No working JRE found. Install one (e.g. Temurin 21, GPLv2+CE) and set JAVA." >&2
  echo "  e.g. JAVA=/path/to/jdk/Contents/Home/bin/java ./check.sh" >&2
  exit 2
fi
if [ ! -f "${JAR}" ]; then
  echo "tla2tools.jar (EPL-2.0) not found at '${JAR}'." >&2
  echo "  Download: https://github.com/tlaplus/tlaplus/releases (tla2tools.jar)" >&2
  echo "  then set TLA_TOOLS=/path/to/tla2tools.jar" >&2
  exit 2
fi

TLC() { "${JAVA}" -XX:+UseParallelGC -cp "${JAR}" tlc2.TLC "$@"; }

echo "== SANY parse =="
"${JAVA}" -cp "${JAR}" tla2sany.SANY commit_protocol.tla

echo "== Safety (all invariants, incl. OrphansNeverReferenced + NoOverwriteOfReferenced) =="
TLC -config commit_protocol.cfg -deadlock -workers auto commit_protocol.tla

echo "== Liveness (WriterEventuallyCommits) =="
TLC -config commit_protocol_liveness.cfg -workers auto commit_protocol.tla

# --- Non-vacuity probes. Each is EXPECTED to be REFUTED (TLC exits non-zero).
# We run each by swapping the single uncommented INVARIANT line in the probes
# config, then assert TLC reported a violation (== the behaviour is reachable).
echo "== Non-vacuity probes (each must be REFUTED) =="
run_probe() {
  local inv="$1"
  echo "-- probe: ${inv} (expect: VIOLATED == reachable) --"
  local out
  out="$(sed "s/^INVARIANT .*/INVARIANT ${inv}/" commit_protocol_probes.cfg \
          > /tmp/_probe_${inv}.cfg; \
        TLC -config /tmp/_probe_${inv}.cfg -workers auto commit_protocol.tla \
          2>&1 || true)"
  echo "${out}" | grep -E "Invariant ${inv} is violated" >/dev/null && {
    echo "   OK: ${inv} refuted -> behaviour reachable (non-vacuous)."
  } || {
    echo "   FAIL: ${inv} was NOT refuted -> behaviour UNREACHABLE; safety may be vacuous." >&2
    exit 3
  }
}
run_probe NoRaceProbe
run_probe DistinctIdsProbe
run_probe ZombieWroteProbe

echo "== All checks passed (safety + liveness hold; all probes refuted). =="
