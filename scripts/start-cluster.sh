#!/usr/bin/env bash
# Start a 3-node Batata cluster in embedded mode for SDK testing.
# Each node runs as server-only (no console) to keep it lightweight.
# One console is optionally started on Node 1 for admin operations.
#
# Usage:
#   ./scripts/start-cluster.sh          # Start cluster
#   ./scripts/start-cluster.sh stop     # Stop cluster
#   ./scripts/start-cluster.sh status   # Check cluster status
#
# Environment variables:
#   CONSUL_ENABLED=true    Enable Consul compatibility (default: false)
#   WITH_CONSOLE=true      Start console on Node 1 (default: true)
#   BUILD=true             Build before starting (default: false)

set -euo pipefail
cd "$(dirname "$0")/.."

# Node configuration
NODE1_PORT=8848
NODE2_PORT=8858
NODE3_PORT=8868
CONSOLE_PORT=8081

# Consul ports (one per node to avoid conflicts)
NODE1_CONSUL_PORT=8500
NODE2_CONSUL_PORT=8510
NODE3_CONSUL_PORT=8520

# Consul Raft ports (consul_port - 1000)

CONSUL_ENABLED="${CONSUL_ENABLED:-false}"
WITH_CONSOLE="${WITH_CONSOLE:-true}"
BUILD="${BUILD:-false}"

CLUSTER_PID_DIR="data/cluster-pids"
BINARY="${BINARY:-./target/release/batata-server}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

stop_cluster() {
    log_info "Stopping cluster..."
    for node in 1 2 3; do
        local pidfile="${CLUSTER_PID_DIR}/node${node}.pid"
        if [ -f "$pidfile" ]; then
            local pid
            pid=$(cat "$pidfile")
            if kill -0 "$pid" 2>/dev/null; then
                kill "$pid" 2>/dev/null || true
                log_info "Stopped node ${node} (PID ${pid})"
            fi
            rm -f "$pidfile"
        fi
    done
    # Kill any leftover batata-server processes
    pkill -9 -f "batata-server" 2>/dev/null || true
    sleep 2
    # Release ports
    for port in $NODE1_PORT $NODE2_PORT $NODE3_PORT $CONSOLE_PORT $NODE1_CONSUL_PORT $NODE2_CONSUL_PORT $NODE3_CONSUL_PORT; do
        lsof -ti:${port} 2>/dev/null | xargs kill -9 2>/dev/null || true
    done
    # Also release gRPC/Raft ports
    for port in $((NODE1_PORT+1000)) $((NODE1_PORT+1001)) $((NODE1_PORT-1000)) \
                $((NODE2_PORT+1000)) $((NODE2_PORT+1001)) $((NODE2_PORT-1000)) \
                $((NODE3_PORT+1000)) $((NODE3_PORT+1001)) $((NODE3_PORT-1000)); do
        lsof -ti:${port} 2>/dev/null | xargs kill -9 2>/dev/null || true
    done
    sleep 1
    log_info "Cluster stopped (data/logs preserved for debugging, cleaned on next start)"
}

check_status() {
    echo "=== Cluster Status ==="
    for port in $NODE1_PORT $NODE2_PORT $NODE3_PORT; do
        local code
        code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:${port}/nacos/v3/admin/core/state/liveness" 2>/dev/null || echo "000")
        if [ "$code" = "200" ]; then
            echo -e "  Node :${port} ${GREEN}UP${NC}"
        else
            echo -e "  Node :${port} ${RED}DOWN${NC}"
        fi
    done
    if [ "$WITH_CONSOLE" = "true" ]; then
        local code
        code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:${CONSOLE_PORT}/v3/auth/user/login" 2>/dev/null || echo "000")
        if [ "$code" != "000" ]; then
            echo -e "  Console :${CONSOLE_PORT} ${GREEN}UP${NC}"
        else
            echo -e "  Console :${CONSOLE_PORT} ${RED}DOWN${NC}"
        fi
    fi
    if [ "$CONSUL_ENABLED" = "true" ]; then
        for cport in $NODE1_CONSUL_PORT $NODE2_CONSUL_PORT $NODE3_CONSUL_PORT; do
            local code
            code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:${cport}/v1/agent/self" 2>/dev/null || echo "000")
            if [ "$code" = "200" ]; then
                echo -e "  Consul :${cport} ${GREEN}UP${NC}"
            else
                echo -e "  Consul :${cport} ${RED}DOWN${NC}"
            fi
        done
    fi
}

start_cluster() {
    # Build if requested
    if [ "$BUILD" = "true" ]; then
        log_info "Building batata-server..."
        cargo build -p batata-server
    fi

    if [ ! -f "$BINARY" ]; then
        log_error "Binary not found: ${BINARY}. Run 'cargo build -p batata-server' first."
        exit 1
    fi

    # Ensure cluster.conf exists
    if [ ! -f conf/cluster.conf ]; then
        log_info "Creating conf/cluster.conf..."
        cat > conf/cluster.conf <<EOF
127.0.0.1:${NODE1_PORT}
127.0.0.1:${NODE2_PORT}
127.0.0.1:${NODE3_PORT}
EOF
    fi

    # Clean old data and logs
    log_info "Cleaning old data and logs..."
    rm -rf data/node1 data/node2 data/node3 data/consul-rocksdb data/rocksdb data/batata_rocksdb data/cluster-pids
    rm -rf logs/node1 logs/node2 logs/node3
    mkdir -p data/node1 data/node2 data/node3 "${CLUSTER_PID_DIR}" logs/node1 logs/node2 logs/node3

    # Member list for cluster discovery
    local member_list="127.0.0.1:${NODE1_PORT},127.0.0.1:${NODE2_PORT},127.0.0.1:${NODE3_PORT}"

    # Common args: cluster mode + embedded storage + member list
    local common_args="-m cluster --batata.sql.init.platform=embedded --batata.member.list=${member_list} --batata.server.http.access_log.enabled=false --batata.plugin.control.enabled=false"
    if [ "$CONSUL_ENABLED" = "true" ]; then
        common_args="${common_args} --batata.plugin.consul.enabled=true"
    fi

    # Determine deployment mode for Node 1
    local deploy_mode="server"
    if [ "$WITH_CONSOLE" = "true" ]; then
        deploy_mode="merged"
    fi

    # Start Node 1 (with console if WITH_CONSOLE=true)
    log_info "Starting Node 1 (:${NODE1_PORT}, consul:${NODE1_CONSUL_PORT}, mode=${deploy_mode}, cluster)..."
    nohup $BINARY \
        -d "$deploy_mode" \
        --batata.server.main.port=${NODE1_PORT} \
        --batata.console.port=${CONSOLE_PORT} \
        --batata.persistence.embedded.data_dir=data/node1 \
        --batata.logs.path=logs/node1 \
        --batata.plugin.consul.port=${NODE1_CONSUL_PORT} \
        ${common_args} \
        > logs/node1/stdout.log 2>&1 &
    echo $! > "${CLUSTER_PID_DIR}/node1.pid"

    # Start Node 2 (server only)
    log_info "Starting Node 2 (:${NODE2_PORT}, consul:${NODE2_CONSUL_PORT}, mode=server, cluster)..."
    nohup $BINARY \
        -d server \
        --batata.server.main.port=${NODE2_PORT} \
        --batata.persistence.embedded.data_dir=data/node2 \
        --batata.logs.path=logs/node2 \
        --batata.plugin.consul.port=${NODE2_CONSUL_PORT} \
        ${common_args} \
        > logs/node2/stdout.log 2>&1 &
    echo $! > "${CLUSTER_PID_DIR}/node2.pid"

    # Start Node 3 (server only)
    log_info "Starting Node 3 (:${NODE3_PORT}, consul:${NODE3_CONSUL_PORT}, mode=server, cluster)..."
    nohup $BINARY \
        -d server \
        --batata.server.main.port=${NODE3_PORT} \
        --batata.persistence.embedded.data_dir=data/node3 \
        --batata.logs.path=logs/node3 \
        --batata.plugin.consul.port=${NODE3_CONSUL_PORT} \
        ${common_args} \
        > logs/node3/stdout.log 2>&1 &
    echo $! > "${CLUSTER_PID_DIR}/node3.pid"

    # Wait for all nodes to be ready
    log_info "Waiting for cluster to form..."
    local max_wait=90
    local waited=0
    while [ $waited -lt $max_wait ]; do
        local ready=0
        for port in $NODE1_PORT $NODE2_PORT $NODE3_PORT; do
            local code
            code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:${port}/nacos/v3/admin/core/state/liveness" 2>/dev/null || echo "000")
            if [ "$code" = "200" ]; then
                ready=$((ready + 1))
            fi
        done
        if [ $ready -eq 3 ]; then
            break
        fi
        sleep 2
        waited=$((waited + 2))
        echo -n "."
    done
    echo ""

    if [ $waited -ge $max_wait ]; then
        log_error "Cluster failed to start within ${max_wait}s"
        check_status
        exit 1
    fi

    log_info "All 3 nodes are UP"

    # Initialize admin user on Node 1
    log_info "Initializing admin user..."
    sleep 3
    curl -s -X POST "http://127.0.0.1:${NODE1_PORT}/nacos/v3/auth/user/admin" \
        -d "username=nacos&password=nacos" > /dev/null 2>&1 || true

    # Wait for Raft replication
    sleep 2

    check_status
    log_info "Cluster is ready!"
    echo ""
    echo "  Nacos SDK endpoints:"
    echo "    Node 1: 127.0.0.1:${NODE1_PORT} (gRPC: $((NODE1_PORT + 1000)))"
    echo "    Node 2: 127.0.0.1:${NODE2_PORT} (gRPC: $((NODE2_PORT + 1000)))"
    echo "    Node 3: 127.0.0.1:${NODE3_PORT} (gRPC: $((NODE3_PORT + 1000)))"
    if [ "$CONSUL_ENABLED" = "true" ]; then
        echo "  Consul API:"
        echo "    Node 1: 127.0.0.1:${NODE1_CONSUL_PORT}"
        echo "    Node 2: 127.0.0.1:${NODE2_CONSUL_PORT}"
        echo "    Node 3: 127.0.0.1:${NODE3_CONSUL_PORT}"
    fi
    echo ""
    echo "  Stop with: ./scripts/start-cluster.sh stop"
}

# Main
case "${1:-start}" in
    start)  stop_cluster 2>/dev/null || true; start_cluster ;;
    stop)   stop_cluster ;;
    status) check_status ;;
    *)      echo "Usage: $0 {start|stop|status}"; exit 1 ;;
esac
