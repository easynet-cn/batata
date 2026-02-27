#!/usr/bin/env bash
# ==============================================================================
# Test: Cluster + Embedded (Raft + RocksDB) Mode
#
# Storage: Distributed RocksDB with Raft consensus
# Start 3 nodes (batata.sql.init.platform must be empty, each node needs its own data_dir and logs):
#   Node 1: cargo run -p batata-server -- -m cluster \
#           --batata.server.main.port=8848 --batata.console.port=8081 \
#           --batata.persistence.embedded.data_dir=data/node1 \
#           --batata.logs.path=logs/node1
#   Node 2: cargo run -p batata-server -- -m cluster \
#           --batata.server.main.port=8858 --batata.console.port=8082 \
#           --batata.persistence.embedded.data_dir=data/node2 \
#           --batata.logs.path=logs/node2
#   Node 3: cargo run -p batata-server -- -m cluster \
#           --batata.server.main.port=8868 --batata.console.port=8083 \
#           --batata.persistence.embedded.data_dir=data/node3 \
#           --batata.logs.path=logs/node3
#
# Prerequisites:
#   - 3-node cluster running with embedded storage (no batata.db.url)
#   - conf/cluster.conf or batata.member.list configured with all 3 nodes
#   - Each node MUST use a different data_dir and logs path to avoid conflicts
#   - Or pass NODE*_MAIN_PORT / NODE*_CONSOLE_PORT env vars
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/test_utils.sh"

MODE_NAME="Cluster + Embedded (Raft + RocksDB)"

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
    local sep="?"
    if [[ "$url" == *"?"* ]]; then sep="&"; fi
    local response
    response=$(curl -s -w "\n%{http_code}" -X GET "${url}${sep}accessToken=${token}" 2>/dev/null)
    HTTP_CODE=$(echo "$response" | tail -1)
    HTTP_BODY=$(echo "$response" | sed '$d')
}

# Helper: POST form with specific token (token in query string for console middleware)
node_post_form() {
    local url="$1"
    local data="$2"
    local token="$3"
    local response
    response=$(curl -s -w "\n%{http_code}" -X POST \
        -H "Content-Type: application/x-www-form-urlencoded" \
        -d "$data" \
        "${url}?accessToken=${token}" 2>/dev/null)
    HTTP_CODE=$(echo "$response" | tail -1)
    HTTP_BODY=$(echo "$response" | sed '$d')
}

# Helper: PUT form with specific token (token in query string for console middleware)
node_put_form() {
    local url="$1"
    local data="$2"
    local token="$3"
    local response
    response=$(curl -s -w "\n%{http_code}" -X PUT \
        -H "Content-Type: application/x-www-form-urlencoded" \
        -d "$data" \
        "${url}?accessToken=${token}" 2>/dev/null)
    HTTP_CODE=$(echo "$response" | tail -1)
    HTTP_BODY=$(echo "$response" | sed '$d')
}

# Helper: DELETE with specific token
node_delete() {
    local url="$1"
    local token="$2"
    local sep="&"
    # Use ? if URL doesn't already have query params
    if [[ "$url" != *"?"* ]]; then sep="?"; fi
    local response
    response=$(curl -s -w "\n%{http_code}" -X DELETE "${url}${sep}accessToken=${token}" 2>/dev/null)
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

TOKEN="$TOKEN1"

# ------------------------------------------------------------------------------
# Step 1: Cluster Formation & Raft Leader
# ------------------------------------------------------------------------------
log_section "Cluster Formation & Raft Leader"

# Verify each node sees all members
for i in 1 2 3; do
    eval "console_url=\${NODE${i}_CONSOLE}"
    eval "token=\${TOKEN${i}}"

    node_get "${console_url}/v3/console/core/cluster/nodes" "$token"
    assert_success "Node ${i}: Get cluster nodes"

    node_get "${console_url}/v3/console/core/cluster/count" "$token"
    local_count=$(echo "$HTTP_BODY" | jq -r '.data // empty' 2>/dev/null)
    if [ "$local_count" = "3" ]; then
        log_pass "Node ${i}: Sees 3 cluster members"
    else
        log_fail "Node ${i}: Expected 3 members, got ${local_count}"
    fi

    node_get "${console_url}/v3/console/core/cluster/standalone" "$token"
    assert_data_equals "false" "Node ${i}: Standalone should be false"
done

# Check leader election
LEADER_ADDR=""
for i in 1 2 3; do
    eval "console_url=\${NODE${i}_CONSOLE}"
    eval "token=\${TOKEN${i}}"

    node_get "${console_url}/v3/console/core/cluster/leader" "$token"
    assert_success "Node ${i}: Get leader info"

    is_leader=$(echo "$HTTP_BODY" | jq -r '.data.isLeader // empty' 2>/dev/null)
    leader_addr=$(echo "$HTTP_BODY" | jq -r '.data.leaderAddress // empty' 2>/dev/null)

    if [ "$is_leader" = "true" ]; then
        LEADER_ADDR="$leader_addr"
        log_pass "Node ${i}: Is the leader (${leader_addr})"
    else
        log_info "Node ${i}: Follower (leader=${leader_addr})"
    fi
done

if [ -n "$LEADER_ADDR" ]; then
    log_pass "Raft leader elected: ${LEADER_ADDR}"
else
    log_fail "No Raft leader found"
fi

# Cluster health
node_get "${NODE1_CONSOLE}/v3/console/core/cluster/health" "$TOKEN1"
assert_success "Cluster health check"

# Node states should all be UP
for i in 1 2 3; do
    eval "console_url=\${NODE${i}_CONSOLE}"
    eval "token=\${TOKEN${i}}"

    node_get "${console_url}/v3/console/core/cluster/self" "$token"
    node_state=$(echo "$HTTP_BODY" | jq -r '.data.state // empty' 2>/dev/null)
    if [ "$node_state" = "UP" ]; then
        log_pass "Node ${i}: State is UP"
    else
        log_fail "Node ${i}: Expected state UP, got ${node_state}"
    fi
done

# ------------------------------------------------------------------------------
# Step 2: Namespace via Raft
# ------------------------------------------------------------------------------
log_section "Namespace Operations (via Raft)"

NS_ID=$(unique_ns_id)
NS_NAME="Raft NS ${TEST_ID}"

# Create namespace on Node 1 (may need to forward to leader)
node_post_form "${NODE1_CONSOLE}/v3/console/core/namespace" \
    "customNamespaceId=${NS_ID}&namespaceName=${NS_NAME}&namespaceDesc=Raft+consensus+test" \
    "$TOKEN1"
assert_success "Create namespace via Raft"

# Wait for Raft replication
sleep 2

# Verify namespace replicated to all nodes
for i in 1 2 3; do
    eval "console_url=\${NODE${i}_CONSOLE}"
    eval "token=\${TOKEN${i}}"

    node_get "${console_url}/v3/console/core/namespace/exist?customNamespaceId=${NS_ID}" "$token"
    assert_data_equals "true" "Node ${i}: Namespace replicated"
done

# Update namespace on Node 2 (follower writes → forward to leader)
node_put_form "${NODE2_CONSOLE}/v3/console/core/namespace" \
    "namespaceId=${NS_ID}&namespaceName=Updated+${NS_NAME}&namespaceDesc=Updated+via+follower" \
    "$TOKEN2"
assert_success "Update namespace from follower"

sleep 1

# Verify update replicated
node_get "${NODE3_CONSOLE}/v3/console/core/namespace?namespaceId=${NS_ID}" "$TOKEN3"
assert_success "Node 3: See updated namespace"

# ------------------------------------------------------------------------------
# Step 3: Config via Raft Consensus
# ------------------------------------------------------------------------------
log_section "Config Operations (via Raft)"

DATA_ID=$(unique_data_id "raft-cfg")
GROUP="DEFAULT_GROUP"

# Write config on Node 1
node_post_form "${NODE1_CONSOLE}/v3/console/cs/config" \
    "dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=&content=raft-replicated-data&type=text" \
    "$TOKEN1"
assert_success "Node 1: Publish config"

# Wait for Raft replication
sleep 2

# Read config on all nodes (should be consistent via Raft)
for i in 1 2 3; do
    eval "console_url=\${NODE${i}_CONSOLE}"
    eval "token=\${TOKEN${i}}"

    node_get "${console_url}/v3/console/cs/config?dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=" "$token"
    assert_success "Node ${i}: Read config (Raft consistency)"
    assert_data_not_empty "Node ${i}: Config content" ".data.content"
done

# Write config from a follower node
DATA_ID_FOLLOWER=$(unique_data_id "follower-cfg")
node_post_form "${NODE2_CONSOLE}/v3/console/cs/config" \
    "dataId=${DATA_ID_FOLLOWER}&groupName=${GROUP}&namespaceId=&content=written-from-follower&type=text" \
    "$TOKEN2"
assert_success "Node 2 (follower): Publish config"

sleep 2

# Verify follower write replicated to all nodes
for i in 1 2 3; do
    eval "console_url=\${NODE${i}_CONSOLE}"
    eval "token=\${TOKEN${i}}"

    node_get "${console_url}/v3/console/cs/config?dataId=${DATA_ID_FOLLOWER}&groupName=${GROUP}&namespaceId=" "$token"
    assert_success "Node ${i}: Read follower-written config"
done

# Config search across nodes
node_get "${NODE3_CONSOLE}/v3/console/cs/config/list?pageNo=1&pageSize=10&dataId=&groupName=&namespaceId=" "$TOKEN3"
assert_success "Node 3: Search all configs"

# Config in custom namespace
DATA_ID_NS=$(unique_data_id "raft-ns-cfg")
node_post_form "${NODE3_CONSOLE}/v3/console/cs/config" \
    "dataId=${DATA_ID_NS}&groupName=${GROUP}&namespaceId=${NS_ID}&content=ns-raft-data&type=text" \
    "$TOKEN3"
assert_success "Node 3: Publish config in namespace"

sleep 2

node_get "${NODE1_CONSOLE}/v3/console/cs/config?dataId=${DATA_ID_NS}&groupName=${GROUP}&namespaceId=${NS_ID}" "$TOKEN1"
assert_success "Node 1: Read namespace config (cross-node)"

# ------------------------------------------------------------------------------
# Step 4: Config Gray/Beta via Raft
# ------------------------------------------------------------------------------
log_section "Config Gray/Beta (via Raft)"

GRAY_DATA_ID=$(unique_data_id "raft-gray")

# Publish base config on Node 1
node_post_form "${NODE1_CONSOLE}/v3/console/cs/config" \
    "dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId=&content=base&type=text" \
    "$TOKEN1"
assert_success "Publish base config for gray test"

sleep 1

# Publish beta on Node 2
node_post_form "${NODE2_CONSOLE}/v3/console/cs/config/beta" \
    "dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId=&content=beta+via+raft&betaIps=10.0.0.1&type=text" \
    "$TOKEN2"
assert_success "Publish beta config via follower"

sleep 2

# Read beta from Node 3
node_get "${NODE3_CONSOLE}/v3/console/cs/config/beta?dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId=" "$TOKEN3"
assert_success "Node 3: Read beta config (Raft replicated)"

# Delete beta
node_delete "${NODE1_CONSOLE}/v3/console/cs/config/beta?dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId=" "$TOKEN1"
assert_success "Delete beta config"

# ------------------------------------------------------------------------------
# Step 5: Service Discovery with Distro Sync
# ------------------------------------------------------------------------------
log_section "Service Discovery (Distro Sync)"

SVC_NAME=$(unique_service_name "raft-svc")

# Register instances on different nodes
node_post_form "${NODE1_MAIN}/nacos/v2/ns/instance" \
    "serviceName=${SVC_NAME}&groupName=${GROUP}&ip=10.0.2.1&port=9090&weight=1.0&healthy=true&enabled=true&ephemeral=true" \
    "$TOKEN1"
if [ "$HTTP_CODE" = "200" ]; then
    log_pass "Node 1: Register instance (10.0.2.1:9090)"
else
    log_fail "Node 1: Register instance (HTTP=${HTTP_CODE})"
fi

node_post_form "${NODE3_MAIN}/nacos/v2/ns/instance" \
    "serviceName=${SVC_NAME}&groupName=${GROUP}&ip=10.0.2.3&port=9090&weight=1.0&healthy=true&enabled=true&ephemeral=true" \
    "$TOKEN3"
if [ "$HTTP_CODE" = "200" ]; then
    log_pass "Node 3: Register instance (10.0.2.3:9090)"
else
    log_fail "Node 3: Register instance (HTTP=${HTTP_CODE})"
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

# Deregister
node_delete "${NODE1_MAIN}/nacos/v2/ns/instance?serviceName=${SVC_NAME}&groupName=${GROUP}&ip=10.0.2.1&port=9090&ephemeral=true" "$TOKEN1"
log_info "Deregistered instance from Node 1"

node_delete "${NODE3_MAIN}/nacos/v2/ns/instance?serviceName=${SVC_NAME}&groupName=${GROUP}&ip=10.0.2.3&port=9090&ephemeral=true" "$TOKEN3"
log_info "Deregistered instance from Node 3"

# ------------------------------------------------------------------------------
# Step 6: Config History (Embedded)
# ------------------------------------------------------------------------------
log_section "Config History (Embedded)"

node_get "${NODE2_CONSOLE}/v3/console/cs/history/list?pageNo=1&pageSize=10&dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=" "$TOKEN2"
assert_success "Node 2: Query config history"

# ------------------------------------------------------------------------------
# Step 7: Cleanup
# ------------------------------------------------------------------------------
log_section "Cleanup"

node_delete "${NODE1_CONSOLE}/v3/console/cs/config?dataId=${DATA_ID_NS}&groupName=${GROUP}&namespaceId=${NS_ID}" "$TOKEN1"
assert_success "Delete config in namespace"

node_delete "${NODE1_CONSOLE}/v3/console/cs/config?dataId=${GRAY_DATA_ID}&groupName=${GROUP}&namespaceId=" "$TOKEN1"
assert_success "Delete gray base config"

node_delete "${NODE1_CONSOLE}/v3/console/cs/config?dataId=${DATA_ID_FOLLOWER}&groupName=${GROUP}&namespaceId=" "$TOKEN1"
assert_success "Delete follower config"

node_delete "${NODE1_CONSOLE}/v3/console/cs/config?dataId=${DATA_ID}&groupName=${GROUP}&namespaceId=" "$TOKEN1"
assert_success "Delete config: ${DATA_ID}"

sleep 1

node_delete "${NODE1_CONSOLE}/v3/console/core/namespace?namespaceId=${NS_ID}" "$TOKEN1"
assert_success "Delete namespace"

sleep 1

# Verify cleanup across nodes
for i in 1 2 3; do
    eval "console_url=\${NODE${i}_CONSOLE}"
    eval "token=\${TOKEN${i}}"

    node_get "${console_url}/v3/console/core/namespace/exist?customNamespaceId=${NS_ID}" "$token"
    assert_data_equals "false" "Node ${i}: Namespace deleted"
done

# ------------------------------------------------------------------------------
# Summary
# ------------------------------------------------------------------------------
print_summary
