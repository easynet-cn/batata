#!/usr/bin/env bash
# ==============================================================================
# Test: Standalone + Embedded (RocksDB) Mode
#
# Storage: RocksDB (single node, no external DB)
# Start:   cargo run -p batata-server -- -m standalone
#          (with batata.sql.init.platform set to empty in config)
#
# Prerequisites:
#   - Server running in standalone embedded mode on default ports
#   - Or pass MAIN_PORT / CONSOLE_PORT env vars
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/test_utils.sh"

MODE_NAME="Standalone + Embedded (RocksDB)"

# ==============================================================================
# Main
# ==============================================================================

log_section "Testing Console Datasource: ${MODE_NAME}"
log_info "Main URL:    ${MAIN_URL}"
log_info "Console URL: ${CONSOLE_URL}"

# ------------------------------------------------------------------------------
# Step 0: Login
# ------------------------------------------------------------------------------
log_section "Authentication"

do_login "$CONSOLE_URL" || exit 1

# Also test login on main server port (main server uses /nacos context path)
do_login "$MAIN_URL/nacos" || log_fail "Login on main server port failed"

# Re-login on console for subsequent tests
do_login "$CONSOLE_URL"

# ------------------------------------------------------------------------------
# Step 1: Health & Server State
# ------------------------------------------------------------------------------
log_section "Health & Server State"

console_get "/v3/console/health/liveness"
assert_success "Health liveness check"

console_get "/v3/console/health/readiness"
assert_success "Health readiness check"

console_get "/v3/console/server/state"
if [ "$HTTP_CODE" = "200" ]; then
    log_pass "Server state endpoint"
    log_info "  Server state: $(echo "$HTTP_BODY" | jq -c '.' 2>/dev/null)"
else
    log_fail "Server state endpoint (HTTP=${HTTP_CODE})"
fi

# ------------------------------------------------------------------------------
# Step 2: Cluster Info (Standalone)
# ------------------------------------------------------------------------------
log_section "Cluster Info (Standalone)"

console_get "/v3/console/core/cluster/nodes"
assert_success "Get cluster nodes"

console_get "/v3/console/core/cluster/self"
assert_success "Get self node"

console_get "/v3/console/core/cluster/health"
assert_success "Get cluster health"

console_get "/v3/console/core/cluster/count"
assert_success "Get member count"

console_get "/v3/console/core/cluster/standalone"
assert_data_equals "true" "Standalone mode should be true"

console_get "/v3/console/core/cluster/leader"
assert_success "Get leader info"

# ------------------------------------------------------------------------------
# Step 3: Namespace CRUD
# ------------------------------------------------------------------------------
log_section "Namespace Operations"

NS_ID=$(unique_ns_id)
NS_NAME="Test Namespace ${TEST_ID}"

# List namespaces (embedded mode may not have initial "public" namespace)
console_get "/v3/console/core/namespace/list"
assert_success "List namespaces"

# Create namespace
console_post_form "/v3/console/core/namespace" \
    "customNamespaceId=${NS_ID}&namespaceName=${NS_NAME}&namespaceDesc=Test+namespace+for+standalone+embedded"
assert_success "Create namespace: ${NS_ID}"

# Check namespace exists
console_get "/v3/console/core/namespace/exist?customNamespaceId=${NS_ID}"
assert_data_equals "true" "Namespace ${NS_ID} should exist"

# Get namespace
console_get "/v3/console/core/namespace?namespaceId=${NS_ID}"
assert_success "Get namespace: ${NS_ID}"

# Update namespace
console_put_form "/v3/console/core/namespace" \
    "namespaceId=${NS_ID}&namespaceName=Updated+${NS_NAME}&namespaceDesc=Updated+description"
assert_success "Update namespace: ${NS_ID}"

# Verify update
console_get "/v3/console/core/namespace?namespaceId=${NS_ID}"
assert_success "Get updated namespace"

# List again (should have the newly created one)
console_get "/v3/console/core/namespace/list"
assert_success "List namespaces after create"
assert_data_length -ge 1 "At least 1 namespace after create"

# ------------------------------------------------------------------------------
# Step 4: Config CRUD
# ------------------------------------------------------------------------------
log_section "Config Operations"

DATA_ID=$(unique_data_id "app")
GROUP="DEFAULT_GROUP"
CONTENT="server:\n  port: 8080\n  name: test-${TEST_ID}"

# Note: In embedded mode, empty namespaceId is stored as "public" internally.
# Config get requires explicit namespaceId=public for embedded storage queries.
DEFAULT_NS="public"

# Publish config
console_post_form "/v3/console/cs/config" \
    "dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=&content=${CONTENT}&type=yaml&desc=Test+config"
assert_success "Publish config: ${DATA_ID}"

# Get config
console_get "/v3/console/cs/config?dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=${DEFAULT_NS}"
assert_success "Get config: ${DATA_ID}"
assert_data_not_empty "Config data not empty" ".data.content"

# Search configs
console_get "/v3/console/cs/config/list?pageNo=1&pageSize=10&dataId=${DATA_ID}&groupName=&namespaceId=${DEFAULT_NS}"
assert_success "Search configs by dataId"

# Update config
UPDATED_CONTENT="server:\n  port: 9090\n  name: updated-${TEST_ID}"
console_post_form "/v3/console/cs/config" \
    "dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=&content=${UPDATED_CONTENT}&type=yaml&desc=Updated+config"
assert_success "Update config: ${DATA_ID}"

# Verify update
console_get "/v3/console/cs/config?dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=${DEFAULT_NS}"
assert_success "Get updated config"

# Publish config in custom namespace
DATA_ID_NS=$(unique_data_id "ns-cfg")
console_post_form "/v3/console/cs/config" \
    "dataId=${DATA_ID_NS}&groupName=${GROUP}&namespaceId=${NS_ID}&content=key:+value&type=yaml"
assert_success "Publish config in namespace: ${NS_ID}"

# Search config in custom namespace
console_get "/v3/console/cs/config/list?pageNo=1&pageSize=10&dataId=&groupName=&namespaceId=${NS_ID}"
assert_success "Search configs in namespace: ${NS_ID}"

# ------------------------------------------------------------------------------
# Step 5: Config Gray/Beta
# ------------------------------------------------------------------------------
log_section "Config Gray/Beta Operations"

GRAY_DATA_ID=$(unique_data_id "gray")

# First publish base config
console_post_form "/v3/console/cs/config" \
    "dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId=&content=base+content&type=text"
assert_success "Publish base config for gray test"

# Publish beta config
console_post_form "/v3/console/cs/config/beta" \
    "dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId=&content=beta+content&betaIps=192.168.1.1,192.168.1.2&type=text"
assert_success "Publish beta config"

# Get beta config
console_get "/v3/console/cs/config/beta?dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId=${DEFAULT_NS}"
assert_success "Get beta config"

# Delete beta config
console_delete "/v3/console/cs/config/beta?dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId=${DEFAULT_NS}"
assert_success "Delete beta config"

# ------------------------------------------------------------------------------
# Step 6: Config History
# ------------------------------------------------------------------------------
log_section "Config History"

# Search history for the config we published
console_get "/v3/console/cs/history/list?pageNo=1&pageSize=10&dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=${DEFAULT_NS}"
assert_success "Search config history"

# Get history detail (if there are records)
HISTORY_ID=$(echo "$HTTP_BODY" | jq -r '.data.pageItems[0].id // .data[0].id // empty' 2>/dev/null) || true
if [ -n "$HISTORY_ID" ] && [ "$HISTORY_ID" != "null" ]; then
    console_get "/v3/console/cs/history?nid=${HISTORY_ID}&dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=${DEFAULT_NS}"
    assert_success "Get history detail: ${HISTORY_ID}"
else
    log_skip "No history records found to query detail"
fi

# ------------------------------------------------------------------------------
# Step 7: Config Listener
# ------------------------------------------------------------------------------
log_section "Config Listener"

console_get "/v3/console/cs/config/listener?dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=${DEFAULT_NS}"
assert_success "Get config listeners"

# ------------------------------------------------------------------------------
# Step 8: Service Discovery (via Main Server)
# ------------------------------------------------------------------------------
log_section "Service Discovery"

SVC_NAME=$(unique_service_name "test-svc")

# Register instance via Main Server V2 API
main_post_form "/nacos/v2/ns/instance" \
    "serviceName=${SVC_NAME}&groupName=${GROUP}&ip=192.168.1.100&port=8080&weight=1.0&healthy=true&enabled=true&ephemeral=true"
if [ "$HTTP_CODE" = "200" ]; then
    log_pass "Register instance via V2 API"
else
    log_fail "Register instance via V2 API (HTTP=${HTTP_CODE})"
    echo "  Response: ${HTTP_BODY:0:300}"
fi

# Wait a moment for registration to propagate
sleep 1

# List services via Console API
console_get "/v3/console/ns/service/list?pageNo=1&pageSize=20&namespaceId=&groupName=${GROUP}&serviceName=${SVC_NAME}"
assert_success "List services via console"

# Get service detail
console_get "/v3/console/ns/service?serviceName=${SVC_NAME}&namespaceId=&groupName=${GROUP}"
assert_success "Get service detail via console"

# List instances via Console API
console_get "/v3/console/ns/instance/list?serviceName=${SVC_NAME}&pageNo=1&pageSize=10&namespaceId=&groupName=${GROUP}"
assert_success "List instances via console"

# Get subscriber list
console_get "/v3/console/ns/service/subscribers?serviceName=${SVC_NAME}&pageNo=1&pageSize=10&namespaceId=&groupName=${GROUP}"
assert_success "List subscribers via console"

# Get selector types
console_get "/v3/console/ns/service/selector/types"
assert_success "Get service selector types"

# Deregister instance
main_delete "/nacos/v2/ns/instance?serviceName=${SVC_NAME}&groupName=${GROUP}&ip=192.168.1.100&port=8080&ephemeral=true"
if [ "$HTTP_CODE" = "200" ]; then
    log_pass "Deregister instance via V2 API"
else
    log_fail "Deregister instance via V2 API (HTTP=${HTTP_CODE})"
fi

# ------------------------------------------------------------------------------
# Step 9: Cleanup
# ------------------------------------------------------------------------------
log_section "Cleanup"

# Delete config in custom namespace
console_delete "/v3/console/cs/config?dataId=${DATA_ID_NS}&groupName=${GROUP}&namespaceId=${NS_ID}"
assert_success "Delete config in namespace: ${NS_ID}"

# Delete gray base config
console_delete "/v3/console/cs/config?dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId=${DEFAULT_NS}"
assert_success "Delete gray base config"

# Delete main config
console_delete "/v3/console/cs/config?dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=${DEFAULT_NS}"
assert_success "Delete config: ${DATA_ID}"

# Delete namespace
console_delete "/v3/console/core/namespace?namespaceId=${NS_ID}"
assert_success "Delete namespace: ${NS_ID}"

# Verify namespace deleted
console_get "/v3/console/core/namespace/exist?customNamespaceId=${NS_ID}"
assert_data_equals "false" "Namespace ${NS_ID} should not exist after delete"

# ------------------------------------------------------------------------------
# Summary
# ------------------------------------------------------------------------------
print_summary
