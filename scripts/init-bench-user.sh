#!/usr/bin/env bash
# Create a non-admin user for permission-path benchmarking.
#
# The stock benchmark runs as `nacos`/`nacos`, which is a global admin: the
# authorization path short-circuits on `is_global_admin` and never loads the
# user's role and permission bindings. This script provisions a plain user with
# a single role and a wildcard permission so `validate_authority` runs its full
# RBAC lookup on every request, which is what benchmarks the RBAC cache.
#
# Usage: ./scripts/init-bench-user.sh [username] [password] [role] [server_url]
# Defaults: benchuser / benchpass / BENCH_ROLE / http://127.0.0.1:8848
#
# Environment:
#   ADMIN_USER=nacos   Admin used to provision the account
#   ADMIN_PASS=nacos   Admin password
#
# The script is idempotent: a user that already exists is reused.

set -uo pipefail

USERNAME="${1:-benchuser}"
PASSWORD="${2:-benchpass}"
ROLE="${3:-BENCH_ROLE}"
SERVER_URL="${4:-http://127.0.0.1:8848}"
ADMIN_USER="${ADMIN_USER:-nacos}"
ADMIN_PASS="${ADMIN_PASS:-nacos}"

json_field() {
    python3 -c "import sys,json;print(json.load(sys.stdin).get('$1',''))" 2>/dev/null || echo ""
}

echo "Logging in as admin '${ADMIN_USER}' on ${SERVER_URL} ..."
TOKEN=$(curl -s -X POST "${SERVER_URL}/nacos/v3/auth/user/login" \
    -d "username=${ADMIN_USER}&password=${ADMIN_PASS}" | json_field accessToken)

if [ -z "${TOKEN}" ]; then
    echo "Failed to obtain admin access token. Is the admin user initialized?"
    echo "Run: ./scripts/init-admin.sh"
    exit 1
fi

AUTH="accessToken=${TOKEN}"

echo "Creating user '${USERNAME}' ..."
CREATE_USER=$(curl -s -w "\n%{http_code}" -X POST "${SERVER_URL}/nacos/v3/auth/user?${AUTH}" \
    -d "username=${USERNAME}&password=${PASSWORD}")
CREATE_CODE=$(echo "${CREATE_USER}" | tail -1)

case "${CREATE_CODE}" in
    200) echo "User created." ;;
    409|400) echo "User already exists (HTTP ${CREATE_CODE}). Reusing." ;;
    *) echo "Failed to create user. HTTP ${CREATE_CODE}"; echo "${CREATE_USER}"; exit 1 ;;
esac

echo "Binding role '${ROLE}' to '${USERNAME}' ..."
BIND=$(curl -s -w "\n%{http_code}" -X POST "${SERVER_URL}/nacos/v3/auth/role?${AUTH}" \
    -d "role=${ROLE}&username=${USERNAME}")
BIND_CODE=$(echo "${BIND}" | tail -1)
BIND_BODY=$(echo "${BIND}" | sed '$d')

case "${BIND_CODE}" in
    200) echo "Role bound." ;;
    *) echo "Role binding returned HTTP ${BIND_CODE}: ${BIND_BODY}" ;;
esac

echo "Granting wildcard permission to role '${ROLE}' ..."
GRANT=$(curl -s -w "\n%{http_code}" -X POST "${SERVER_URL}/nacos/v3/auth/permission?${AUTH}" \
    -d "role=${ROLE}&resource=*&action=rw")
GRANT_CODE=$(echo "${GRANT}" | tail -1)
GRANT_BODY=$(echo "${GRANT}" | sed '$d')

case "${GRANT_CODE}" in
    200) echo "Permission granted." ;;
    *) echo "Permission grant returned HTTP ${GRANT_CODE}: ${GRANT_BODY}" ;;
esac

echo "Verifying login for '${USERNAME}' ..."
USER_TOKEN=$(curl -s -X POST "${SERVER_URL}/nacos/v3/auth/user/login" \
    -d "username=${USERNAME}&password=${PASSWORD}" | json_field accessToken)

if [ -z "${USER_TOKEN}" ]; then
    echo "Benchmark user cannot log in. Aborting."
    exit 1
fi

echo
echo "Benchmark user ready:"
echo "  USERNAME=${USERNAME} PASSWORD=${PASSWORD} ./scripts/benchmark.sh naming"
