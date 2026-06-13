#!/usr/bin/env bash
# scripts/tck/fetch.sh — (re)vendor the openCypher TCK feature files at a pinned tag.
#
# The TCK `.feature` files are vendored into the repo at `tck/openCypher/features`
# so that CI can run the harness WITHOUT external network access. This script is
# the reproducible recipe that produced that vendored tree; run it to refresh the
# corpus or bump the pinned release.
#
# Usage:
#   scripts/tck/fetch.sh [TAG]
#     TAG — openCypher release tag to pin to (default: the TCK_TAG below).
#
# The openCypher TCK is licensed Apache-2.0 (license-clean; see
# docs/process/open-source-guardrails.md §5). The upstream LICENSE + NOTICE are
# vendored alongside the features for attribution.
#
# See: docs/process/testing-and-benchmarks.md §6, tck/openCypher/README.md

set -euo pipefail

# Pinned openCypher release. Bumping this is a deliberate, reviewed change.
TCK_TAG="${1:-2024.3}"
UPSTREAM="https://github.com/opencypher/openCypher.git"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DEST="${REPO_ROOT}/tck/openCypher"

echo "Fetching openCypher TCK at tag '${TCK_TAG}' from ${UPSTREAM}"

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT

# Shallow, blobless, sparse clone: we only need the TCK feature tree + license.
git clone --depth 1 --branch "${TCK_TAG}" --filter=blob:none --sparse \
  "${UPSTREAM}" "${TMP}/oc"
git -C "${TMP}/oc" sparse-checkout set tck/features LICENSE NOTICE LICENSE.txt

if [ ! -d "${TMP}/oc/tck/features" ]; then
  echo "Error: tck/features not found at tag '${TCK_TAG}'." >&2
  exit 1
fi

# Replace the vendored tree atomically-ish: stage in a sibling, then swap.
rm -rf "${DEST}/features"
mkdir -p "${DEST}"
cp -R "${TMP}/oc/tck/features" "${DEST}/features"

# Vendor the upstream license / notice for attribution.
for lf in LICENSE LICENSE.txt NOTICE; do
  if [ -f "${TMP}/oc/${lf}" ]; then
    cp "${TMP}/oc/${lf}" "${DEST}/${lf}"
  fi
done

# Record the exact tag + commit the corpus was pinned to.
printf '%s\n' "${TCK_TAG}" > "${DEST}/PINNED_TAG"
git -C "${TMP}/oc" rev-parse HEAD > "${DEST}/PINNED_COMMIT"

COUNT="$(find "${DEST}/features" -name '*.feature' | wc -l | tr -d ' ')"
echo "Vendored ${COUNT} .feature files to ${DEST}/features"
echo "Pinned tag:    ${TCK_TAG}"
echo "Pinned commit: $(cat "${DEST}/PINNED_COMMIT")"
echo "Done. Review the diff and commit the vendored corpus."
