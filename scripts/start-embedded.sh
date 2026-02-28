#!/usr/bin/env bash
# Start Batata in merged mode with embedded RocksDB storage (no database required).
# Both console (8081) and main server (8848) run in the same process.
#
# Usage: ./scripts/start-embedded.sh

set -euo pipefail
cd "$(dirname "$0")/.."

cargo run -p batata-server -- --batata.sql.init.platform=embedded
