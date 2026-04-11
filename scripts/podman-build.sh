#!/usr/bin/env bash
# ==============================================================================
# Build Batata container image with Podman
#
# Usage:
#   ./scripts/podman-build.sh                 # Build with default tag
#   ./scripts/podman-build.sh v0.1.0          # Build with version tag
#   TAG=myregistry/batata:dev ./scripts/podman-build.sh
# ==============================================================================

set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-latest}"
TAG="${TAG:-batata:${VERSION}}"

echo "==> Building Batata container image: ${TAG}"
echo "    Containerfile: Containerfile"
echo ""

podman build \
    -t "${TAG}" \
    -f Containerfile \
    --format oci \
    .

echo ""
echo "==> Build complete: ${TAG}"
echo ""
echo "Run with:"
echo "  podman run -d --name batata -p 8848:8848 -p 8081:8081 -p 9848:9848 ${TAG}"
