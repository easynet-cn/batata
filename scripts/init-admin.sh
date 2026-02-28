#!/usr/bin/env bash
# Initialize the admin user (first-time setup).
# This can only be called ONCE per server instance (returns 409 if admin already exists).
#
# Usage: ./scripts/init-admin.sh [username] [password] [server_url]
# Defaults: nacos / nacos / http://127.0.0.1:8848

set -euo pipefail

USERNAME="${1:-nacos}"
PASSWORD="${2:-nacos}"
SERVER_URL="${3:-http://127.0.0.1:8848}"

echo "Initializing admin user '${USERNAME}' on ${SERVER_URL} ..."

RESPONSE=$(curl -s -w "\n%{http_code}" -X POST \
  "${SERVER_URL}/nacos/v3/auth/user/admin" \
  -d "username=${USERNAME}&password=${PASSWORD}")

HTTP_CODE=$(echo "$RESPONSE" | tail -1)
BODY=$(echo "$RESPONSE" | sed '$d')

if [ "$HTTP_CODE" = "200" ]; then
  echo "Admin user created successfully."
  echo "Response: ${BODY}"
elif [ "$HTTP_CODE" = "409" ]; then
  echo "Admin user already exists (409 Conflict). Skipping."
else
  echo "Failed to create admin user. HTTP ${HTTP_CODE}"
  echo "Response: ${BODY}"
  exit 1
fi
