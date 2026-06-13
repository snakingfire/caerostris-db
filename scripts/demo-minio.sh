#!/usr/bin/env bash
# caerostris-db — OBJECT-STORAGE-NATIVE graph database demo.
#
# The wow: a graph database whose durable state lives in plain S3 objects.
# This script proves it end to end against the local MinIO mock:
#
#   1. Show the S3 bucket EMPTY.
#   2. Insert a social graph (people, companies, relationships) and PERSIST it
#      as individual objects in the bucket.
#   3. Show the bucket now CONTAINS those objects (real S3 keys + sizes).
#   4. READ the graph back out of S3 and answer openCypher MATCH queries —
#      including multi-property filters, a one-hop traversal, and a WHERE clause.
#
# Screen-recording friendly: labelled sections, the query text, and the results.
# Run from anywhere:
#
#     ./scripts/demo-minio.sh
#
# Requirements (auto-provisioned by the swarm; see CLAUDE.md):
#   - the shared MinIO mock running (scripts/env/up.sh)
#   - the `aws` CLI on PATH (used here only to show the raw bucket listing)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

bar()  { printf '%s\n' "============================================================"; }
rule() { printf '%s\n' "------------------------------------------------------------"; }

# ---------------------------------------------------------------------------
# 0. Provision the environment + an isolated bucket/prefix for this demo run.
# ---------------------------------------------------------------------------
# Resolve the MAIN repo's env file (works from a worktree too).
GIT_COMMON="$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)"
if [[ -n "${GIT_COMMON}" ]]; then
  MAIN_ROOT="$(dirname "${GIT_COMMON}")"
else
  MAIN_ROOT="${REPO_ROOT}"
fi
ENV_FILE="${MAIN_ROOT}/.project/env/local.env"

bar
echo " caerostris-db — an object-storage-native graph database"
echo " insert a graph  ->  it persists as S3 objects  ->  query it with Cypher"
bar
echo

echo "[0/4] Provisioning the local S3 (MinIO) environment ..."
if [[ ! -f "${ENV_FILE}" ]]; then
  echo "      starting the local S3 mock (scripts/env/up.sh) ..."
  bash "${REPO_ROOT}/scripts/env/up.sh" >/dev/null 2>&1 || true
fi
# shellcheck source=/dev/null
source "${ENV_FILE}"
# Isolated bucket + per-run prefix so this demo never collides with other work.
eval "$(bash "${REPO_ROOT}/scripts/env/bucket.sh" demo 2>/dev/null)"
echo "      endpoint : ${CAEROSTRIS_S3_ENDPOINT}"
echo "      bucket   : ${CAEROSTRIS_S3_BUCKET}"
echo "      prefix   : ${CAEROSTRIS_S3_PREFIX}"
echo

echo "      Building the caero binary ..."
cargo build --quiet --bin caero
CAERO="${REPO_ROOT}/target/debug/caero"
echo "      build OK"
echo

# Helper: list the bucket contents straight from S3 with the raw aws CLI, so
# you can SEE these are genuine S3 objects, not an in-process illusion.
aws_ls() {
  aws --endpoint-url "${CAEROSTRIS_S3_ENDPOINT}" s3api list-objects-v2 \
    --bucket "${CAEROSTRIS_S3_BUCKET}" \
    --prefix "${CAEROSTRIS_S3_PREFIX}" \
    --query 'Contents[].{Key:Key,Bytes:Size}' \
    --output text 2>/dev/null || true
}

# ---------------------------------------------------------------------------
# 1. The bucket is EMPTY.
# ---------------------------------------------------------------------------
echo "[1/4] The S3 bucket starts EMPTY"
rule
echo "  \$ aws s3api list-objects-v2 --bucket ${CAEROSTRIS_S3_BUCKET} --prefix ${CAEROSTRIS_S3_PREFIX}"
EMPTY_LISTING="$(aws_ls)"
if [[ -z "${EMPTY_LISTING}" || "${EMPTY_LISTING}" == "None" ]]; then
  echo "  (no objects — the durable graph does not exist yet)"
else
  echo "${EMPTY_LISTING}"
fi
rule
echo

# ---------------------------------------------------------------------------
# 2 + 4. Insert + persist to S3, then read back & query (the caero binary
#        narrates this in labelled sections).
# ---------------------------------------------------------------------------
echo "[2/4] Insert a graph, PERSIST it to S3, read it back, and query it"
rule
"${CAERO}" minio-demo
rule
echo

# ---------------------------------------------------------------------------
# 3. The bucket now CONTAINS the persisted objects — straight from S3.
# ---------------------------------------------------------------------------
echo "[3/4] The same data, now durable as real S3 objects in the bucket"
rule
echo "  \$ aws s3api list-objects-v2 --bucket ${CAEROSTRIS_S3_BUCKET} --prefix ${CAEROSTRIS_S3_PREFIX}"
echo
printf '  %-40s %10s\n' "KEY" "BYTES"
aws_ls | while IFS=$'\t' read -r bytes key; do
  # `--query {Key,Bytes}` emits columns alphabetically: Bytes<TAB>Key.
  [[ -z "${key}" ]] && continue
  printf '  %-40s %10s\n' "${key}" "${bytes}"
done
rule
echo

# ---------------------------------------------------------------------------
# Done.
# ---------------------------------------------------------------------------
echo "[4/4] Done."
bar
echo " Every node and edge above is a real object in S3/MinIO."
echo " The MATCH queries READ those objects back — this is a graph database"
echo " whose source of truth is commodity object storage. That is the point."
bar
