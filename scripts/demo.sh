#!/usr/bin/env bash
# caerostris-db end-to-end demo: insert graph data, then query it back with
# openCypher MATCH and see the inserted data returned.
#
# Copy-paste friendly and screen-recording friendly: it prints labelled section
# headers, shows the query text, and shows the results. Run from anywhere:
#
#     ./scripts/demo.sh
#
# It builds the `caero` binary (release-free debug build for speed) and runs the
# `caero demo` subcommand, whose logic lives in src/demo/ and is unit-tested.
set -euo pipefail

# Resolve the repo root from this script's location so the demo runs from any cwd.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

bar() { printf '%s\n' "------------------------------------------------------------"; }

bar
echo " caerostris-db — end-to-end demo"
echo " insert graph data  ->  run a Cypher MATCH  ->  see it returned"
bar
echo

echo "[1/2] Building the caero binary (cargo build) ..."
cargo build --quiet --bin caero
echo "      build OK"
echo

echo "[2/2] Running: caero demo"
bar
# The binary prints its own labelled sections (insert / query 1 / query 2).
./target/debug/caero demo
bar
echo
echo "Done. The two MATCH queries above returned the exact nodes/edges that were"
echo "inserted at the top — proving an end-to-end insert -> query round trip."
