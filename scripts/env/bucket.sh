#!/usr/bin/env bash
# scripts/env/bucket.sh — print (and create if possible) an isolated S3
# bucket + prefix for one work item so parallel agents never collide.
#
# Usage: scripts/env/bucket.sh <ID>
#   e.g.  scripts/env/bucket.sh T-0042
#
# Output: exportable shell lines that a test can eval:
#   eval "$(scripts/env/bucket.sh T-0042)"
#
# The bucket name is derived deterministically from the ID; the prefix is
# per-invocation (timestamp + PID) so repeated runs of the same item don't
# clash inside the bucket.

set -euo pipefail

# ---------------------------------------------------------------------------
# Usage guard
# ---------------------------------------------------------------------------
if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <ID>" >&2
  echo "  e.g. $0 T-0042" >&2
  exit 1
fi

ID="$1"

# ---------------------------------------------------------------------------
# Locate the shared ENV_DIR in the MAIN repo (works from any worktree)
# ---------------------------------------------------------------------------
GIT_COMMON=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null \
  || { echo "ERROR: not inside a git repository" >&2; exit 1; })
MAIN_ROOT=$(dirname "$GIT_COMMON")
ENV_DIR="$MAIN_ROOT/.project/env"

ENV_FILE="$ENV_DIR/local.env"
if [[ ! -f "$ENV_FILE" ]]; then
  echo "ERROR: $ENV_FILE not found." >&2
  echo "  Run scripts/env/up.sh first to start the S3 mock." >&2
  exit 1
fi

# shellcheck source=/dev/null
source "$ENV_FILE"

# Verify required variables were sourced
for var in CAEROSTRIS_S3_ENDPOINT CAEROSTRIS_S3_BUCKET_BASE AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY; do
  if [[ -z "${!var:-}" ]]; then
    echo "ERROR: $var is missing from $ENV_FILE." >&2
    echo "  Try re-running scripts/env/up.sh." >&2
    exit 1
  fi
done

# ---------------------------------------------------------------------------
# Derive deterministic bucket name from the ID
# Valid S3 bucket names: 3-63 chars, lowercase letters/digits/hyphens, no
# leading/trailing hyphen, no consecutive hyphens.
# ---------------------------------------------------------------------------
ID_SLUG=$(echo "$ID" \
  | tr 'A-Z' 'a-z' \
  | tr -c 'a-z0-9-' '-' \
  | sed 's/^-*//' \
  | sed 's/-*$//')

BUCKET="${CAEROSTRIS_S3_BUCKET_BASE}-${ID_SLUG}"

# Truncate to 63 chars (S3 limit) if the combination is long
if [[ ${#BUCKET} -gt 63 ]]; then
  BUCKET="${BUCKET:0:63}"
  # Strip any trailing hyphen introduced by truncation
  BUCKET="${BUCKET%-}"
fi

# ---------------------------------------------------------------------------
# Per-invocation prefix: timestamp + PID avoids collisions across reruns
# ---------------------------------------------------------------------------
PREFIX="run/$(date -u +%Y%m%dT%H%M%SZ)-$$/";

# ---------------------------------------------------------------------------
# Create the bucket if a suitable CLI is available (idempotent)
# ---------------------------------------------------------------------------
REGION="${CAEROSTRIS_S3_REGION:-us-east-1}"

if command -v mc &>/dev/null; then
  # MinIO Client — configure a transient alias and create the bucket
  MC_ALIAS="caerostris-local-$$"
  mc alias set "$MC_ALIAS" \
    "$CAEROSTRIS_S3_ENDPOINT" \
    "$AWS_ACCESS_KEY_ID" \
    "$AWS_SECRET_ACCESS_KEY" \
    --api S3v4 \
    &>/dev/null
  # mb returns 0 even if bucket exists when --ignore-existing is passed
  mc mb --ignore-existing "${MC_ALIAS}/${BUCKET}" &>/dev/null || true
  mc alias remove "$MC_ALIAS" &>/dev/null || true

elif command -v aws &>/dev/null; then
  # AWS CLI — works against MinIO and moto with path-style + explicit endpoint
  aws s3api create-bucket \
    --bucket "$BUCKET" \
    --region "$REGION" \
    --endpoint-url "$CAEROSTRIS_S3_ENDPOINT" \
    &>/dev/null 2>&1 || true
  # "BucketAlreadyOwnedByYou" and "BucketAlreadyExists" are both acceptable;
  # suppress them silently via || true.

else
  # No CLI available — print a notice but don't fail; the test harness can
  # create the bucket itself via the object_store SDK.
  echo "# NOTE: neither 'mc' nor 'aws' CLI found; bucket not pre-created." >&2
  echo "#       The test harness must create it via the object_store API." >&2
fi

# ---------------------------------------------------------------------------
# Print exportable lines for eval
# ---------------------------------------------------------------------------
echo "export CAEROSTRIS_S3_BUCKET=${BUCKET}"
echo "export CAEROSTRIS_S3_PREFIX=${PREFIX}"
echo "export CAEROSTRIS_S3_ENDPOINT=${CAEROSTRIS_S3_ENDPOINT}"
echo "export AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID}"
echo "export AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY}"
echo "export CAEROSTRIS_S3_REGION=${REGION}"
echo "export CAEROSTRIS_S3_FORCE_PATH_STYLE=true"
