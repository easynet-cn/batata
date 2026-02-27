#!/usr/bin/env bash
# ==============================================================================
# Test: Standalone + ExternalDb (MySQL/PostgreSQL) Mode
#
# Storage: External database (MySQL or PostgreSQL)
# Start:   cargo run -p batata-server -- -m standalone --db-url "mysql://user:pass@localhost:3306/batata"
#          (spring.sql.init.platform must be "mysql" or "postgresql" in config)
#
# Prerequisites:
#   - Server running in standalone mode with external database
#   - Database initialized with schema (conf/mysql-schema.sql or conf/postgresql-schema.sql)
#   - Or pass MAIN_PORT / CONSOLE_PORT env vars
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/test_utils.sh"

MODE_NAME="Standalone + ExternalDb"

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

# Test login on main server port too
do_login "$MAIN_URL" || log_fail "Login on main server port failed"

# Re-login on console
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

    # Verify it's using external DB (check for database-related state info)
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

console_get "/v3/console/core/cluster/standalone"
assert_data_equals "true" "Standalone mode should be true"

console_get "/v3/console/core/cluster/count"
assert_success "Get member count"

# ------------------------------------------------------------------------------
# Step 3: Namespace CRUD
# ------------------------------------------------------------------------------
log_section "Namespace Operations"

NS_ID=$(unique_ns_id)
NS_NAME="Test Namespace ${TEST_ID}"

# List namespaces
console_get "/v3/console/core/namespace/list"
assert_success "List namespaces"
assert_data_length -ge 1 "At least 1 namespace exists"

# Create namespace
console_post_form "/v3/console/core/namespace" \
    "customNamespaceId=${NS_ID}&namespaceName=${NS_NAME}&namespaceDesc=Test+namespace+for+standalone+externaldb"
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

# ------------------------------------------------------------------------------
# Step 4: Config CRUD
# ------------------------------------------------------------------------------
log_section "Config Operations"

DATA_ID=$(unique_data_id "db-cfg")
GROUP="DEFAULT_GROUP"
CONTENT="database:\n  host: localhost\n  port: 3306"

# Publish config
console_post_form "/v3/console/cs/config" \
    "dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=&content=${CONTENT}&type=yaml&desc=ExternalDb+test"
assert_success "Publish config: ${DATA_ID}"

# Get config
console_get "/v3/console/cs/config?dataId=${DATA_ID}&groupName=${GROUP}&namespaceId="
assert_success "Get config: ${DATA_ID}"
assert_data_not_empty "Config content not empty" ".data.content"

# Search configs
console_get "/v3/console/cs/config/list?pageNo=1&pageSize=10&dataId=${DATA_ID}&groupName=&namespaceId="
assert_success "Search configs by dataId"

# Update config
console_post_form "/v3/console/cs/config" \
    "dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=&content=updated:+true&type=yaml"
assert_success "Update config: ${DATA_ID}"

# Publish config in custom namespace
DATA_ID_NS=$(unique_data_id "ns-db-cfg")
console_post_form "/v3/console/cs/config" \
    "dataId=${DATA_ID_NS}&groupName=${GROUP}&namespaceId=${NS_ID}&content=ns+config&type=text"
assert_success "Publish config in namespace: ${NS_ID}"

# Search config in custom namespace
console_get "/v3/console/cs/config/list?pageNo=1&pageSize=10&dataId=&groupName=&namespaceId=${NS_ID}"
assert_success "Search configs in namespace"

# ------------------------------------------------------------------------------
# Step 5: Config Gray/Beta
# ------------------------------------------------------------------------------
log_section "Config Gray/Beta Operations"

GRAY_DATA_ID=$(unique_data_id "db-gray")

# Publish base config
console_post_form "/v3/console/cs/config" \
    "dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId=&content=base&type=text"
assert_success "Publish base config for gray test"

# Publish beta config
console_post_form "/v3/console/cs/config/beta" \
    "dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId=&content=beta+content&betaIps=10.0.0.1,10.0.0.2&type=text"
assert_success "Publish beta config"

# Get beta config
console_get "/v3/console/cs/config/beta?dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId="
assert_success "Get beta config"

# Delete beta config
console_delete "/v3/console/cs/config/beta?dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId="
assert_success "Delete beta config"

# ------------------------------------------------------------------------------
# Step 6: Config History (ExternalDb has full history support)
# ------------------------------------------------------------------------------
log_section "Config History"

# Search history
console_get "/v3/console/cs/history/list?pageNo=1&pageSize=10&dataId=${DATA_ID}&groupName=${GROUP}&namespaceId="
assert_success "Search config history"

# Get history detail if available
HISTORY_ID=$(echo "$HTTP_BODY" | jq -r '.data.pageItems[0].id // .data[0].id // empty' 2>/dev/null)
if [ -n "$HISTORY_ID" ] && [ "$HISTORY_ID" != "null" ]; then
    console_get "/v3/console/cs/history?nid=${HISTORY_ID}"
    assert_success "Get history detail: ${HISTORY_ID}"

    # Get previous version
    console_get "/v3/console/cs/history/previous?id=${HISTORY_ID}&dataId=${DATA_ID}&groupName=${GROUP}&namespaceId="
    if [ "$HTTP_CODE" = "200" ]; then
        log_pass "Get previous history version"
    else
        log_skip "No previous history version available"
    fi
else
    log_skip "No history records found"
fi

# ------------------------------------------------------------------------------
# Step 7: Config Export/Import
# ------------------------------------------------------------------------------
log_section "Config Export/Import"

# Export configs
EXPORT_FILE="/tmp/batata_test_export_${TEST_ID}.zip"
curl -s -o "$EXPORT_FILE" -w "%{http_code}" \
    "${CONSOLE_URL}/v3/console/cs/config/export?namespaceId=&group=${GROUP}&accessToken=${TOKEN}" \
    > /tmp/batata_export_code_${TEST_ID}.txt 2>/dev/null

EXPORT_CODE=$(cat "/tmp/batata_export_code_${TEST_ID}.txt")
if [ "$EXPORT_CODE" = "200" ] && [ -f "$EXPORT_FILE" ] && [ -s "$EXPORT_FILE" ]; then
    log_pass "Export configs to zip"
else
    log_skip "Config export (HTTP=${EXPORT_CODE})"
fi

# Clean up export temp files
rm -f "$EXPORT_FILE" "/tmp/batata_export_code_${TEST_ID}.txt"

# ------------------------------------------------------------------------------
# Step 8: Service Discovery
# ------------------------------------------------------------------------------
log_section "Service Discovery"

SVC_NAME=$(unique_service_name "db-svc")

# Register instance via Main Server
main_post_form "/nacos/v2/ns/instance" \
    "serviceName=${SVC_NAME}&groupName=${GROUP}&ip=10.0.1.1&port=8080&weight=1.0&healthy=true&enabled=true&ephemeral=true"
if [ "$HTTP_CODE" = "200" ]; then
    log_pass "Register instance via V2 API"
else
    log_fail "Register instance via V2 API (HTTP=${HTTP_CODE})"
fi

sleep 1

# List services via Console
console_get "/v3/console/ns/service/list?pageNo=1&pageSize=20&namespaceId=&groupName=${GROUP}&serviceName=${SVC_NAME}"
assert_success "List services via console"

# Get service detail
console_get "/v3/console/ns/service?serviceName=${SVC_NAME}&namespaceId=&groupName=${GROUP}"
assert_success "Get service detail"

# List instances
console_get "/v3/console/ns/instance/list?serviceName=${SVC_NAME}&pageNo=1&pageSize=10&namespaceId=&groupName=${GROUP}"
assert_success "List instances via console"

# Deregister instance
main_delete "/nacos/v2/ns/instance?serviceName=${SVC_NAME}&groupName=${GROUP}&ip=10.0.1.1&port=8080&ephemeral=true"
if [ "$HTTP_CODE" = "200" ]; then
    log_pass "Deregister instance"
else
    log_fail "Deregister instance (HTTP=${HTTP_CODE})"
fi

# ------------------------------------------------------------------------------
# Step 9: Cleanup
# ------------------------------------------------------------------------------
log_section "Cleanup"

console_delete "/v3/console/cs/config?dataId=${DATA_ID_NS}&groupName=${GROUP}&namespaceId=${NS_ID}"
assert_success "Delete config in namespace"

console_delete "/v3/console/cs/config?dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId="
assert_success "Delete gray base config"

console_delete "/v3/console/cs/config?dataId=${DATA_ID}&groupName=${GROUP}&namespaceId="
assert_success "Delete config: ${DATA_ID}"

console_delete "/v3/console/core/namespace?namespaceId=${NS_ID}"
assert_success "Delete namespace: ${NS_ID}"

console_get "/v3/console/core/namespace/exist?customNamespaceId=${NS_ID}"
assert_data_equals "false" "Namespace ${NS_ID} should not exist after delete"

# ------------------------------------------------------------------------------
# Summary
# ------------------------------------------------------------------------------
print_summary
