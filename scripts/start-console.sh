#!/usr/bin/env bash
# Start Batata in console-only mode (remote console).
# Only the console server (8081) starts, connecting to a remote main server.
# Requires a running server instance (started with -d server or merged mode).
#
# Usage: ./scripts/start-console.sh [server_addr]
# Example: ./scripts/start-console.sh http://127.0.0.1:8848

set -euo pipefail
cd "$(dirname "$0")/.."

SERVER_ADDR="${1:-http://127.0.0.1:8848}"

cargo run -p batata-server -- -d console \
  --nacos.console.remote.server_addr="$SERVER_ADDR"
