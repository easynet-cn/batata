#!/usr/bin/env bash
# ==============================================================================
# Test: Cluster + ExternalDb (MySQL/PostgreSQL) Mode
#
# Storage: External database (shared across all nodes)
# Start 3 nodes (each node should use its own log directory):
#   Node 1: cargo run -p batata-server -- -m cluster --db-url "mysql://..." \
#           --batata.server.main.port=8848 --batata.console.port=8081 \
#           --batata.logs.path=logs/node1
#   Node 2: cargo run -p batata-server -- -m cluster --db-url "mysql://..." \
#           --batata.server.main.port=8858 --batata.console.port=8082 \
#           --batata.logs.path=logs/node2
#   Node 3: cargo run -p batata-server -- -m cluster --db-url "mysql://..." \
#           --batata.server.main.port=8868 --batata.console.port=8083 \
#           --batata.logs.path=logs/node3
#
# Cluster member discovery via conf/cluster.conf or batata.member.list config
#
# Prerequisites:
#   - 3-node cluster running with shared external database
#   - Or pass NODE1_MAIN_PORT, NODE1_CONSOLE_PORT, etc. env vars
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/test_utils.sh"

MODE_NAME="Cluster + ExternalDb"

# Node configuration
NODE1_MAIN_PORT="${NODE1_MAIN_PORT:-8848}"
NODE1_CONSOLE_PORT="${NODE1_CONSOLE_PORT:-8081}"
NODE2_MAIN_PORT="${NODE2_MAIN_PORT:-8858}"
NODE2_CONSOLE_PORT="${NODE2_CONSOLE_PORT:-8082}"
NODE3_MAIN_PORT="${NODE3_MAIN_PORT:-8868}"
NODE3_CONSOLE_PORT="${NODE3_CONSOLE_PORT:-8083}"

NODE1_MAIN="http://127.0.0.1:${NODE1_MAIN_PORT}"
NODE1_CONSOLE="http://127.0.0.1:${NODE1_CONSOLE_PORT}"
NODE2_MAIN="http://127.0.0.1:${NODE2_MAIN_PORT}"
NODE2_CONSOLE="http://127.0.0.1:${NODE2_CONSOLE_PORT}"
NODE3_MAIN="http://127.0.0.1:${NODE3_MAIN_PORT}"
NODE3_CONSOLE="http://127.0.0.1:${NODE3_CONSOLE_PORT}"

# Tokens per node
TOKEN1=""
TOKEN2=""
TOKEN3=""

# Helper: login to a specific node's console
login_node() {
    local node_url="$1"
    local response
    response=$(curl -s -w "\n%{http_code}" -X POST \
        "${node_url}/v3/auth/user/login" \
        -d "username=${USERNAME}&password=${PASSWORD}" 2>/dev/null)
    local http_code
    http_code=$(echo "$response" | tail -1)
    local body
    body=$(echo "$response" | sed '$d')

    if [ "$http_code" = "200" ]; then
        echo "$body" | jq -r '.data.accessToken // .accessToken // empty'
    else
        echo ""
    fi
}

# Helper: GET with specific token
node_get() {
    local url="$1"
    local token="$2"
    local response
    response=$(curl -s -w "\n%{http_code}" -X GET "${url}?accessToken=${token}" 2>/dev/null)
    HTTP_CODE=$(echo "$response" | tail -1)
    HTTP_BODY=$(echo "$response" | sed '$d')
}

# Helper: POST form with specific token
node_post_form() {
    local url="$1"
    local data="$2"
    local token="$3"
    local response
    response=$(curl -s -w "\n%{http_code}" -X POST \
        -H "Content-Type: application/x-www-form-urlencoded" \
        -d "${data}&accessToken=${token}" \
        "$url" 2>/dev/null)
    HTTP_CODE=$(echo "$response" | tail -1)
    HTTP_BODY=$(echo "$response" | sed '$d')
}

# Helper: DELETE with specific token
node_delete() {
    local url="$1"
    local token="$2"
    local response
    response=$(curl -s -w "\n%{http_code}" -X DELETE "${url}&accessToken=${token}" 2>/dev/null)
    HTTP_CODE=$(echo "$response" | tail -1)
    HTTP_BODY=$(echo "$response" | sed '$d')
}

# ==============================================================================
# Main
# ==============================================================================

log_section "Testing Console Datasource: ${MODE_NAME}"
log_info "Node 1: Main=${NODE1_MAIN}, Console=${NODE1_CONSOLE}"
log_info "Node 2: Main=${NODE2_MAIN}, Console=${NODE2_CONSOLE}"
log_info "Node 3: Main=${NODE3_MAIN}, Console=${NODE3_CONSOLE}"

# ------------------------------------------------------------------------------
# Step 0: Login to all nodes
# ------------------------------------------------------------------------------
log_section "Authentication (All Nodes)"

TOKEN1=$(login_node "$NODE1_CONSOLE")
if [ -n "$TOKEN1" ]; then
    log_pass "Login to Node 1"
else
    log_fail "Login to Node 1"
    exit 1
fi

TOKEN2=$(login_node "$NODE2_CONSOLE")
if [ -n "$TOKEN2" ]; then
    log_pass "Login to Node 2"
else
    log_fail "Login to Node 2"
    exit 1
fi

TOKEN3=$(login_node "$NODE3_CONSOLE")
if [ -n "$TOKEN3" ]; then
    log_pass "Login to Node 3"
else
    log_fail "Login to Node 3"
    exit 1
fi

# Also set default TOKEN for utility functions
TOKEN="$TOKEN1"

# ------------------------------------------------------------------------------
# Step 1: Cluster Formation
# ------------------------------------------------------------------------------
log_section "Cluster Formation"

# Each node should see all 3 members
for i in 1 2 3; do
    eval "console_url=\${NODE${i}_CONSOLE}"
    eval "token=\${TOKEN${i}}"

    node_get "${console_url}/v3/console/core/cluster/nodes" "$token"
    assert_success "Node ${i}: Get cluster nodes"

    node_get "${console_url}/v3/console/core/cluster/count" "$token"
    assert_success "Node ${i}: Get member count"
    local_count=$(echo "$HTTP_BODY" | jq -r '.data // empty' 2>/dev/null)
    if [ "$local_count" = "3" ]; then
        log_pass "Node ${i}: Sees 3 cluster members"
    else
        log_fail "Node ${i}: Expected 3 members, got ${local_count}"
    fi

    node_get "${console_url}/v3/console/core/cluster/standalone" "$token"
    assert_data_equals "false" "Node ${i}: Standalone should be false"
done

# Check cluster health
node_get "${NODE1_CONSOLE}/v3/console/core/cluster/health" "$TOKEN1"
assert_success "Cluster health check"

# Check self info on each node
for i in 1 2 3; do
    eval "console_url=\${NODE${i}_CONSOLE}"
    eval "token=\${TOKEN${i}}"

    node_get "${console_url}/v3/console/core/cluster/self" "$token"
    assert_success "Node ${i}: Get self info"
    node_state=$(echo "$HTTP_BODY" | jq -r '.data.state // empty' 2>/dev/null)
    if [ "$node_state" = "UP" ]; then
        log_pass "Node ${i}: State is UP"
    else
        log_fail "Node ${i}: Expected state UP, got ${node_state}"
    fi
done

# ------------------------------------------------------------------------------
# Step 2: Namespace Consistency
# ------------------------------------------------------------------------------
log_section "Namespace Consistency Across Nodes"

NS_ID=$(unique_ns_id)
NS_NAME="Cluster NS ${TEST_ID}"

# Create namespace on Node 1
node_post_form "${NODE1_CONSOLE}/v3/console/core/namespace" \
    "customNamespaceId=${NS_ID}&namespaceName=${NS_NAME}&namespaceDesc=Cluster+test" \
    "$TOKEN1"
assert_success "Node 1: Create namespace"

sleep 1

# Verify namespace visible on Node 2 and Node 3
node_get "${NODE2_CONSOLE}/v3/console/core/namespace?namespaceId=${NS_ID}" "$TOKEN2"
assert_success "Node 2: Namespace visible"

node_get "${NODE3_CONSOLE}/v3/console/core/namespace?namespaceId=${NS_ID}" "$TOKEN3"
assert_success "Node 3: Namespace visible"

# Verify namespace exists on all nodes
for i in 1 2 3; do
    eval "console_url=\${NODE${i}_CONSOLE}"
    eval "token=\${TOKEN${i}}"

    node_get "${console_url}/v3/console/core/namespace/exist?customNamespaceId=${NS_ID}" "$token"
    assert_data_equals "true" "Node ${i}: Namespace ${NS_ID} exists"
done

# ------------------------------------------------------------------------------
# Step 3: Config Consistency
# ------------------------------------------------------------------------------
log_section "Config Consistency Across Nodes"

DATA_ID=$(unique_data_id "cluster-cfg")
GROUP="DEFAULT_GROUP"
CONTENT="cluster-key:+cluster-value"

# Write config on Node 1
node_post_form "${NODE1_CONSOLE}/v3/console/cs/config" \
    "dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=&content=${CONTENT}&type=yaml" \
    "$TOKEN1"
assert_success "Node 1: Publish config"

sleep 1

# Read config on Node 2 (should be consistent - same DB)
node_get "${NODE2_CONSOLE}/v3/console/cs/config?dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=" "$TOKEN2"
assert_success "Node 2: Read config (consistency)"
assert_data_not_empty "Node 2: Config content not empty" ".data.content"

# Read config on Node 3
node_get "${NODE3_CONSOLE}/v3/console/cs/config?dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=" "$TOKEN3"
assert_success "Node 3: Read config (consistency)"
assert_data_not_empty "Node 3: Config content not empty" ".data.content"

# Update config on Node 2
node_post_form "${NODE2_CONSOLE}/v3/console/cs/config" \
    "dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=&content=updated-by-node2&type=yaml" \
    "$TOKEN2"
assert_success "Node 2: Update config"

sleep 1

# Verify update visible on Node 1 and Node 3
node_get "${NODE1_CONSOLE}/v3/console/cs/config?dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=" "$TOKEN1"
assert_success "Node 1: See updated config from Node 2"

node_get "${NODE3_CONSOLE}/v3/console/cs/config?dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=" "$TOKEN3"
assert_success "Node 3: See updated config from Node 2"

# ------------------------------------------------------------------------------
# Step 4: Config in Custom Namespace (Cross-Node)
# ------------------------------------------------------------------------------
log_section "Config in Custom Namespace (Cross-Node)"

DATA_ID_NS=$(unique_data_id "cluster-ns-cfg")

# Write in custom namespace on Node 3
node_post_form "${NODE3_CONSOLE}/v3/console/cs/config" \
    "dataId=${DATA_ID_NS}&groupName=${GROUP}&namespaceId=${NS_ID}&content=ns-cluster-data&type=text" \
    "$TOKEN3"
assert_success "Node 3: Publish config in namespace"

sleep 1

# Verify on Node 1
node_get "${NODE1_CONSOLE}/v3/console/cs/config?dataId=${DATA_ID_NS}&groupName=${GROUP}&namespaceId=${NS_ID}" "$TOKEN1"
assert_success "Node 1: Read config in namespace (cross-node)"

# Search on Node 2
node_get "${NODE2_CONSOLE}/v3/console/cs/config/list?pageNo=1&pageSize=10&dataId=&groupName=&namespaceId=${NS_ID}" "$TOKEN2"
assert_success "Node 2: Search configs in namespace (cross-node)"

# ------------------------------------------------------------------------------
# Step 5: Service Discovery with Distro Sync
# ------------------------------------------------------------------------------
log_section "Service Discovery (Distro Sync)"

SVC_NAME=$(unique_service_name "cluster-svc")

# Register instance on Node 1's main server
node_post_form "${NODE1_MAIN}/nacos/v2/ns/instance" \
    "serviceName=${SVC_NAME}&groupName=${GROUP}&ip=10.0.1.1&port=8080&weight=1.0&healthy=true&enabled=true&ephemeral=true" \
    "$TOKEN1"
if [ "$HTTP_CODE" = "200" ]; then
    log_pass "Node 1: Register instance (10.0.1.1:8080)"
else
    log_fail "Node 1: Register instance (HTTP=${HTTP_CODE})"
fi

# Register another instance on Node 2's main server
node_post_form "${NODE2_MAIN}/nacos/v2/ns/instance" \
    "serviceName=${SVC_NAME}&groupName=${GROUP}&ip=10.0.1.2&port=8080&weight=1.0&healthy=true&enabled=true&ephemeral=true" \
    "$TOKEN2"
if [ "$HTTP_CODE" = "200" ]; then
    log_pass "Node 2: Register instance (10.0.1.2:8080)"
else
    log_fail "Node 2: Register instance (HTTP=${HTTP_CODE})"
fi

# Wait for distro sync
log_info "Waiting for distro sync..."
sleep 3

# Verify all instances visible from all console nodes
for i in 1 2 3; do
    eval "console_url=\${NODE${i}_CONSOLE}"
    eval "token=\${TOKEN${i}}"

    node_get "${console_url}/v3/console/ns/instance/list?serviceName=${SVC_NAME}&pageNo=1&pageSize=20&namespaceId=&groupName=${GROUP}" "$token"
    assert_success "Node ${i}: List instances"

    instance_count=$(echo "$HTTP_BODY" | jq '.data.totalCount // .data.list | length // 0' 2>/dev/null)
    if [ "$instance_count" = "2" ]; then
        log_pass "Node ${i}: Sees 2 instances (distro sync OK)"
    else
        log_fail "Node ${i}: Expected 2 instances, got ${instance_count}"
    fi
done

# Verify services visible from all nodes
for i in 1 2 3; do
    eval "console_url=\${NODE${i}_CONSOLE}"
    eval "token=\${TOKEN${i}}"

    node_get "${console_url}/v3/console/ns/service/list?pageNo=1&pageSize=20&namespaceId=&groupName=${GROUP}&serviceName=${SVC_NAME}" "$token"
    assert_success "Node ${i}: List services"
done

# Deregister instances
node_delete "${NODE1_MAIN}/nacos/v2/ns/instance?serviceName=${SVC_NAME}&groupName=${GROUP}&ip=10.0.1.1&port=8080&ephemeral=true" "$TOKEN1"
if [ "$HTTP_CODE" = "200" ]; then
    log_pass "Deregister instance from Node 1"
else
    log_fail "Deregister instance from Node 1 (HTTP=${HTTP_CODE})"
fi

node_delete "${NODE2_MAIN}/nacos/v2/ns/instance?serviceName=${SVC_NAME}&groupName=${GROUP}&ip=10.0.1.2&port=8080&ephemeral=true" "$TOKEN2"
if [ "$HTTP_CODE" = "200" ]; then
    log_pass "Deregister instance from Node 2"
else
    log_fail "Deregister instance from Node 2 (HTTP=${HTTP_CODE})"
fi

# ------------------------------------------------------------------------------
# Step 6: Config History (Cross-Node)
# ------------------------------------------------------------------------------
log_section "Config History (Cross-Node)"

# Query history on Node 3 for config created on Node 1
node_get "${NODE3_CONSOLE}/v3/console/cs/history/list?pageNo=1&pageSize=10&dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=" "$TOKEN3"
assert_success "Node 3: Query history for config from Node 1"

# ------------------------------------------------------------------------------
# Step 7: Cleanup
# ------------------------------------------------------------------------------
log_section "Cleanup"

# Delete config in namespace (from any node - all share same DB)
node_delete "${NODE1_CONSOLE}/v3/console/cs/config?dataId=${DATA_ID_NS}&groupName=${GROUP}&namespaceId=${NS_ID}" "$TOKEN1"
assert_success "Delete config in namespace"

node_delete "${NODE1_CONSOLE}/v3/console/cs/config?dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=" "$TOKEN1"
assert_success "Delete config: ${DATA_ID}"

node_delete "${NODE1_CONSOLE}/v3/console/core/namespace?namespaceId=${NS_ID}" "$TOKEN1"
assert_success "Delete namespace: ${NS_ID}"

# Verify cleanup across nodes
node_get "${NODE2_CONSOLE}/v3/console/core/namespace/exist?customNamespaceId=${NS_ID}" "$TOKEN2"
assert_data_equals "false" "Node 2: Namespace deleted"

node_get "${NODE3_CONSOLE}/v3/console/core/namespace/exist?customNamespaceId=${NS_ID}" "$TOKEN3"
assert_data_equals "false" "Node 3: Namespace deleted"

# ------------------------------------------------------------------------------
# Summary
# ------------------------------------------------------------------------------
print_summary
