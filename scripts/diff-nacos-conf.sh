#!/usr/bin/env bash
#
# Diff Batata's runtime conf/ against a local Nacos distribution.
#
# Purpose: review upstream Nacos configuration changes without keeping copies of
# Nacos files inside conf/. See docs/compat/nacos/config-mapping.md for the
# key-by-key mapping.
#
# Usage:
#   scripts/diff-nacos-conf.sh
#   NACOS_HOME=/path/to/nacos scripts/diff-nacos-conf.sh
#
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BATATA_CONF="${REPO_ROOT}/conf"
NACOS_HOME="${NACOS_HOME:-${HOME}/work/nacos}"
NACOS_CONF="${NACOS_HOME}/conf"

# Files that exist in both Batata's runtime conf/ and Nacos' distribution conf/.
SHARED_FILES=(
  "announcement_en-US.conf"
  "announcement_zh-CN.conf"
  "console-guide.conf"
)

if [[ ! -d "${NACOS_CONF}" ]]; then
  echo "error: Nacos conf directory not found: ${NACOS_CONF}"
  echo "Set NACOS_HOME to your Nacos distribution root, e.g.:"
  echo "  NACOS_HOME=~/work/nacos scripts/diff-nacos-conf.sh"
  exit 1
fi

echo "Batata conf : ${BATATA_CONF}"
echo "Nacos  conf : ${NACOS_CONF}"
echo

status=0

for f in "${SHARED_FILES[@]}"; do
  echo "=============================================================="
  echo "== ${f}"
  echo "=============================================================="
  if [[ ! -f "${NACOS_CONF}/${f}" ]]; then
    echo "  (not present in Nacos distribution)"
    continue
  fi
  if ! diff -u "${NACOS_CONF}/${f}" "${BATATA_CONF}/${f}"; then
    status=1
  fi
  echo
done

# The upstream application.properties is kept as a reference file; diff it too.
REF_DIR="${REPO_ROOT}/docs/compat/nacos/conf"
echo "=============================================================="
echo "== application.properties (reference copy)"
echo "=============================================================="
if [[ -f "${NACOS_CONF}/application.properties" && -f "${REF_DIR}/application.properties" ]]; then
  diff -u "${NACOS_CONF}/application.properties" "${REF_DIR}/application.properties" || status=1
else
  echo "  (reference copy or Nacos file missing)"
fi
echo

if [[ ${status} -eq 0 ]]; then
  echo "No differences found."
else
  echo "Differences found (see above). Review whether Batata needs to follow upstream."
fi

exit "${status}"
