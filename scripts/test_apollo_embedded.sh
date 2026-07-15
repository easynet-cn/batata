#!/usr/bin/env bash
# ==============================================================================
# Test: Apollo Plugin + Embedded (RocksDB) Mode
#
# Storage: RocksDB (single node, no external DB)
# Plugin:  Apollo compatibility plugin enabled
#
# Usage: ./scripts/test_apollo_embedded.sh
#
# Environment Variables:
#   APOLLO_PORT              - Apollo plugin port (default: 8080)
#   MAIN_PORT                - Main server port (default: 8848)
#   APOLLO_EXTERNAL_SERVER   - Set to "true" if server is already running
# ==============================================================================

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=test_utils.sh
source "${SCRIPT_DIR}/test_utils.sh"

APOLLO_PORT="${APOLLO_PORT:-8080}"
MAIN_PORT="${MAIN_PORT:-8848}"
APOLLO_URL="http://localhost:${APOLLO_PORT}"
TEST_ID="$(date +%s)"
APOLLO_EXTERNAL_SERVER="${APOLLO_EXTERNAL_SERVER:-false}"

APP_ID="test-app-${TEST_ID}"
CLUSTER_NAME="default"
NAMESPACE_NAME="application"
ITEM_KEY="test.key"
ITEM_VALUE="test.value.${TEST_ID}"

# ── Setup ────────────────────────────────────────────────────────────────────

cleanup() {
    if [ "$APOLLO_EXTERNAL_SERVER" != "true" ]; then
        stop_server
    fi
}
trap cleanup EXIT

log_section "Apollo Plugin Embedded Mode E2E Test"
log_info "Test ID: ${TEST_ID}"
log_info "Apollo URL: ${APOLLO_URL}"
log_info ""

# ── Start server (or use external) ───────────────────────────────────────────

if [ "$APOLLO_EXTERNAL_SERVER" != "true" ]; then
    start_server \
        -m standalone \
        --batata.sql.init.platform=embedded \
        --batata.plugin.apollo.enabled=true \
        --batata.plugin.apollo.port="${APOLLO_PORT}" \
        --batata.server.main.port="${MAIN_PORT}"

    wait_for_server "http://localhost:${MAIN_PORT}" 90

    log_info "Waiting for Apollo plugin to be ready..."
    for i in $(seq 1 30); do
        if curl -s -o /dev/null -w "%{http_code}" "${APOLLO_URL}/health" 2>/dev/null | grep -q "200"; then
            log_info "Apollo plugin is ready"
            break
        fi
        sleep 2
    done
else
    log_info "Using external server at ${APOLLO_URL}"
fi

# ── HTTP helpers for Apollo ──────────────────────────────────────────────────

apollo_get() {
    http_request GET "${APOLLO_URL}$1"
}

apollo_post_json() {
    http_request POST "${APOLLO_URL}$1" "$2" "application/json"
}

apollo_put_json() {
    http_request PUT "${APOLLO_URL}$1" "$2" "application/json"
}

apollo_delete() {
    http_request DELETE "${APOLLO_URL}$1"
}

# ── Tests ────────────────────────────────────────────────────────────────────

log_section "1. Health Check"
apollo_get "/health"
assert_http_code 200 "Health check"

log_section "2. App CRUD Tests"

# List apps (empty initially)
apollo_get "/apps"
assert_http_code 200 "List apps"

# Create app
apollo_post_json "/apps" "{\"appId\":\"${APP_ID}\",\"name\":\"Test App ${TEST_ID}\",\"orgId\":\"TEST\",\"orgName\":\"Test Org\",\"ownerName\":\"admin\",\"ownerEmail\":\"admin@test.com\"}"
assert_http_code 200 "Create app"
assert_data_equals "${APP_ID}" "Verify appId created" ".appId"

# Get app
apollo_get "/apps/${APP_ID}"
assert_http_code 200 "Get app"
assert_data_equals "${APP_ID}" "Verify appId" ".appId"

# Update app
apollo_put_json "/apps/${APP_ID}" "{\"appId\":\"${APP_ID}\",\"name\":\"Updated App ${TEST_ID}\",\"orgId\":\"TEST\",\"orgName\":\"Test Org\",\"ownerName\":\"admin\",\"ownerEmail\":\"admin@test.com\"}"
assert_http_code 200 "Update app"

log_section "3. Cluster Tests"

# List clusters
apollo_get "/apps/${APP_ID}/clusters"
assert_http_code 200 "List clusters"

# Create cluster
apollo_post_json "/apps/${APP_ID}/clusters" "{\"name\":\"${CLUSTER_NAME}\",\"appId\":\"${APP_ID}\"}"
assert_http_code 200 "Create cluster"

# Get cluster
apollo_get "/apps/${APP_ID}/clusters/${CLUSTER_NAME}"
assert_http_code 200 "Get cluster"

log_section "4. Namespace CRUD Tests"

# Create namespace
apollo_post_json "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces" "{\"appId\":\"${APP_ID}\",\"clusterName\":\"${CLUSTER_NAME}\",\"namespaceName\":\"${NAMESPACE_NAME}\",\"format\":\"properties\",\"isPublic\":false,\"comment\":\"Test namespace\"}"
assert_http_code 200 "Create namespace"

# Get namespace
apollo_get "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces/${NAMESPACE_NAME}"
assert_http_code 200 "Get namespace"

# List namespaces
apollo_get "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces"
assert_http_code 200 "List namespaces"
assert_data_length -ge 1 "Namespace list not empty" "."

log_section "5. Item CRUD Tests"

# Create item
apollo_post_json "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces/${NAMESPACE_NAME}/items" "{\"key\":\"${ITEM_KEY}\",\"value\":\"${ITEM_VALUE}\",\"comment\":\"Test item\"}"
assert_http_code 200 "Create item"

# Get item
apollo_get "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces/${NAMESPACE_NAME}/items/${ITEM_KEY}"
assert_http_code 200 "Get item"
assert_data_equals "${ITEM_VALUE}" "Verify item value" ".value"

# List items
apollo_get "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces/${NAMESPACE_NAME}/items"
assert_http_code 200 "List items"
assert_data_length -ge 1 "Item list not empty" "."

# Get item id for update
apollo_put_json "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces/${NAMESPACE_NAME}/items/${ITEM_KEY}" "{\"key\":\"${ITEM_KEY}\",\"value\":\"updated.value.${TEST_ID}\",\"comment\":\"Updated test item\"}"
assert_http_code 200 "Update item"

log_section "6. Release Tests"

# Publish release
apollo_post_json "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces/${NAMESPACE_NAME}/releases?name=Release+${TEST_ID}&comment=Test+release&operator=admin" ""
assert_http_code 200 "Publish release"

# List releases
apollo_get "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces/${NAMESPACE_NAME}/releases"
assert_http_code 200 "List releases"
assert_data_length -ge 1 "Release list not empty" ".content"

log_section "7. Config Service Tests"

# Get config via Config Service (JSON format)
apollo_get "/configfiles/json/${APP_ID}/${CLUSTER_NAME}/${NAMESPACE_NAME}"
assert_http_code 200 "Get config (JSON format)"

# Get config as properties
apollo_get "/configfiles/${APP_ID}/${CLUSTER_NAME}/${NAMESPACE_NAME}"
assert_http_code 200 "Get config (properties format)"

# Get config via /configs endpoint
apollo_get "/configs/${APP_ID}/${CLUSTER_NAME}/${NAMESPACE_NAME}"
assert_http_code 200 "Get config (configs endpoint)"

log_section "8. Commit Tests"

# List commits
apollo_get "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces/${NAMESPACE_NAME}/commits"
assert_http_code 200 "List commits"

log_section "9. Instance Tests"

# Register instance (simulate SDK heartbeat)
apollo_post_json "/instances" "{\"ip\":\"127.0.0.1\",\"appId\":\"${APP_ID}\",\"clusterName\":\"${CLUSTER_NAME}\",\"dataCenter\":\"default\",\"hostname\":\"localhost\"}"
assert_http_code 200 "Register instance"

log_section "10. ItemSet Batch Tests"

ITEMSET_BODY="{
  \"createItems\": [
    {\"key\":\"batch.key1\",\"value\":\"batch.value1\",\"comment\":\"Batch create 1\"},
    {\"key\":\"batch.key2\",\"value\":\"batch.value2\",\"comment\":\"Batch create 2\"}
  ],
  \"updateItems\": [
    {\"key\":\"${ITEM_KEY}\",\"value\":\"batch.updated.value\",\"comment\":\"Batch update\"}
  ],
  \"deleteItems\": []
}"

apollo_post_json "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces/${NAMESPACE_NAME}/itemset" "${ITEMSET_BODY}"
assert_http_code 200 "ItemSet batch update"

log_section "11. Namespace Lock Tests"

# Lock namespace
apollo_post_json "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces/${NAMESPACE_NAME}/lock" "{\"operator\":\"admin\"}"
assert_http_code 200 "Lock namespace"

# Get namespace lock
apollo_get "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces/${NAMESPACE_NAME}/lock"
assert_http_code 200 "Get namespace lock"

# Unlock namespace
apollo_delete "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces/${NAMESPACE_NAME}/lock?operator=admin"
assert_http_code 200 "Unlock namespace"

log_section "12. OpenAPI Tests"

# OpenAPI list apps
apollo_get "/openapi/v1/apps"
assert_http_code 200 "OpenAPI list apps"

# OpenAPI get app
apollo_get "/openapi/v1/apps/${APP_ID}"
assert_http_code 200 "OpenAPI get app"

log_section "13. Delete Tests"

# Delete item by key
apollo_delete "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces/${NAMESPACE_NAME}/items/${ITEM_KEY}?operator=admin"
assert_http_code 200 "Delete item"

# Delete namespace
apollo_delete "/apps/${APP_ID}/clusters/${CLUSTER_NAME}/namespaces/${NAMESPACE_NAME}?operator=admin"
assert_http_code 200 "Delete namespace"

# Delete cluster
apollo_delete "/apps/${APP_ID}/clusters/${CLUSTER_NAME}?operator=admin"
assert_http_code 200 "Delete cluster"

# Delete app
apollo_delete "/apps/${APP_ID}?operator=admin"
assert_http_code 200 "Delete app"

# ── Summary ──────────────────────────────────────────────────────────────────

print_summary
