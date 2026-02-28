#!/usr/bin/env bash
# Start Batata in merged mode with external MySQL database.
# Both console (8081) and main server (8848) run in the same process.
#
# Usage: ./scripts/start-mysql.sh [db_url]
# Example: ./scripts/start-mysql.sh "mysql://user:password@localhost:3306/batata"

set -euo pipefail
cd "$(dirname "$0")/.."

DB_URL="${1:-${DATABASE_URL:-}}"

if [ -z "$DB_URL" ]; then
  echo "Usage: $0 <mysql_url>"
  echo "  or set DATABASE_URL environment variable"
  echo "Example: $0 \"mysql://user:password@localhost:3306/batata\""
  exit 1
fi

cargo run -p batata-server -- --db-url "$DB_URL"
