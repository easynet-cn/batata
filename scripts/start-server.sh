#!/usr/bin/env bash
# Start Batata in server-only mode with embedded RocksDB storage.
# Only the main server (8848) and gRPC servers (9848/9849) start.
# Console server does NOT start. Use with a separate console process (-d console).
#
# Usage: ./scripts/start-server.sh

set -euo pipefail
cd "$(dirname "$0")/.."

cargo run -p batata-server -- -d server --batata.sql.init.platform=embedded
