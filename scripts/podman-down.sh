#!/usr/bin/env bash
# ==============================================================================
# Stop all Podman containers for Batata
#
# Usage:
#   ./scripts/podman-down.sh           # Stop and remove containers
#   ./scripts/podman-down.sh -v        # Also remove volumes (clean data)
#   ./scripts/podman-down.sh all       # Stop all compose files
# ==============================================================================

set -euo pipefail
cd "$(dirname "$0")/.."

VOLUME_FLAG=""
MODE="${1:-default}"

if [ "$MODE" = "-v" ] || [ "${2:-}" = "-v" ]; then
    VOLUME_FLAG="-v"
    echo "==> Stopping containers and removing volumes..."
else
    echo "==> Stopping containers..."
fi

if [ "$MODE" = "all" ]; then
    # Stop all compose files
    podman-compose down $VOLUME_FLAG 2>/dev/null || true
    podman-compose -f podman-compose.test.yml down $VOLUME_FLAG 2>/dev/null || true
    (cd sdk-tests && podman-compose down $VOLUME_FLAG 2>/dev/null) || true
    echo "All containers stopped."
elif [ "$MODE" = "test" ]; then
    podman-compose -f podman-compose.test.yml down $VOLUME_FLAG
elif [ "$MODE" = "sdk" ]; then
    (cd sdk-tests && podman-compose down $VOLUME_FLAG)
else
    podman-compose down $VOLUME_FLAG
fi

echo "Done."
