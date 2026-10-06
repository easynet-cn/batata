#!/bin/bash

# Copyright 2026 Batata Authors
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

#===========================================================================================
# Batata startup script (Nacos-compatible)
#
# Usage:
#   ./startup.sh                          # Cluster mode, merged deployment
#   ./startup.sh -m standalone            # Standalone mode (single node)
#   ./startup.sh -d server                # Server-only deployment
#   ./startup.sh -d console              # Console-only deployment
#   ./startup.sh -d serverWithMcp         # Server + MCP Registry
#   ./startup.sh -p embedded             # Use embedded RocksDB storage
#   ./startup.sh -c "ip1:port,ip2:port"  # Specify cluster members
#
# Options:
#   -m  Mode: standalone | cluster (default: cluster)
#   -d  Deployment: merged | server | console | serverWithMcp (default: merged)
#   -p  Storage platform: embedded | mysql | postgres (default: from config)
#   -c  Cluster member list (comma-separated ip:port)
#   -s  Server binary name (default: batata-server)
#   -C  Config file path (default: conf/application.yml)
#===========================================================================================

set -euo pipefail

error_exit() {
    echo "ERROR: $1 !!"
    exit 1
}

# Detect OS
darwin=false
linux=false
case "$(uname)" in
    Darwin*) darwin=true ;;
    Linux*)  linux=true ;;
esac

# ---------------------------------------------------------------------------
# Optional subcommand handling (e.g. `cluster`)
# ---------------------------------------------------------------------------
SUBCMD=""
case "${1:-}" in
    cluster) SUBCMD="cluster"; shift ;;
esac

if [ "$SUBCMD" = "cluster" ]; then
    BASE_DIR=$(cd "$(dirname "$0")/.."; pwd)
    SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
    source "${SCRIPT_DIR}/cluster_lib.sh"

    NODE1_PORT=${CLUSTER_NODE_PORTS[0]}; NODE2_PORT=${CLUSTER_NODE_PORTS[1]}; NODE3_PORT=${CLUSTER_NODE_PORTS[2]}
    CONSOLE_PORT=${CLUSTER_CONSOLE_PORTS[0]}
    NODE2_CONSOLE_PORT=${CLUSTER_CONSOLE_PORTS[1]}
    NODE3_CONSOLE_PORT=${CLUSTER_CONSOLE_PORTS[2]}
    NODE1_CONSUL_PORT=${CLUSTER_CONSUL_PORTS[0]}; NODE2_CONSUL_PORT=${CLUSTER_CONSUL_PORTS[1]}; NODE3_CONSUL_PORT=${CLUSTER_CONSUL_PORTS[2]}
    NODE1_APOLLO_PORT=${CLUSTER_APOLLO_PORTS[0]}; NODE2_APOLLO_PORT=${CLUSTER_APOLLO_PORTS[1]}; NODE3_APOLLO_PORT=${CLUSTER_APOLLO_PORTS[2]}

    RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; NC='\033[0m'
    log_info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
    log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
    log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

    CLUSTER_PID_DIR="${BASE_DIR}/data/cluster-pids"
    BINARY="${BINARY:-${BASE_DIR}/target/release/batata-server}"

    CONSUL_ENABLED="${CONSUL_ENABLED:-false}"
    APOLLO_ENABLED="${APOLLO_ENABLED:-false}"
    WITH_CONSOLE="${WITH_CONSOLE:-true}"
    BUILD="${BUILD:-false}"

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
        pkill -9 -f "batata-server" 2>/dev/null || true
        sleep 2
        cluster_kill_ports "${NODE1_PORT}" "${NODE2_PORT}" "${NODE3_PORT}" "${CONSOLE_PORT}" \
            "${NODE1_CONSUL_PORT}" "${NODE2_CONSUL_PORT}" "${NODE3_CONSUL_PORT}"
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
            for cport in $CONSOLE_PORT $NODE2_CONSOLE_PORT $NODE3_CONSOLE_PORT; do
                local code
                code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:${cport}/v3/auth/user/login" 2>/dev/null || echo "000")
                if [ "$code" != "000" ]; then
                    echo -e "  Console :${cport} ${GREEN}UP${NC}"
                else
                    echo -e "  Console :${cport} ${RED}DOWN${NC}"
                fi
            done
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
        if [ "$APOLLO_ENABLED" = "true" ]; then
            for aport in $NODE1_APOLLO_PORT $NODE2_APOLLO_PORT $NODE3_APOLLO_PORT; do
                local code
                code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:${aport}/services/config" 2>/dev/null || echo "000")
                if [ "$code" = "200" ]; then
                    echo -e "  Apollo :${aport} ${GREEN}UP${NC}"
                else
                    echo -e "  Apollo :${aport} ${RED}DOWN${NC}"
                fi
            done
        fi
    }

    start_cluster() {
        if [ "$BUILD" = "true" ]; then
            log_info "Building batata-server..."
            (cd "${BASE_DIR}" && cargo build -p batata-server)
        fi

        if [ ! -f "$BINARY" ]; then
            log_error "Binary not found: ${BINARY}. Run 'cargo build -p batata-server' first."
            exit 1
        fi
        # Normalize to absolute path for robustness
        BINARY="$(cd "$(dirname "$BINARY")" && pwd)/$(basename "$BINARY")"

        if [ ! -f "${BASE_DIR}/conf/cluster.conf" ]; then
            log_info "Creating ${BASE_DIR}/conf/cluster.conf..."
            cluster_write_conf "${BASE_DIR}"
        fi

        log_info "Cleaning old data and logs..."
        rm -rf "${BASE_DIR}/data/node1" "${BASE_DIR}/data/node2" "${BASE_DIR}/data/node3" \
              "${BASE_DIR}/data/consul-rocksdb" "${BASE_DIR}/data/rocksdb" "${BASE_DIR}/data/batata_rocksdb" "${BASE_DIR}/data/cluster-pids"
        rm -rf "${BASE_DIR}/logs/node1" "${BASE_DIR}/logs/node2" "${BASE_DIR}/logs/node3"
        mkdir -p "${BASE_DIR}/data/node1" "${BASE_DIR}/data/node2" "${BASE_DIR}/data/node3" \
                 "${CLUSTER_PID_DIR}" "${BASE_DIR}/logs/node1" "${BASE_DIR}/logs/node2" "${BASE_DIR}/logs/node3"

        local member_list="${CLUSTER_MEMBER_LIST}"
        local common_args="-m cluster --batata.sql.init.platform=embedded --batata.member.list=${member_list} --batata.server.http.access_log.enabled=false --batata.plugin.control.enabled=false"
        if [ "$CONSUL_ENABLED" = "true" ]; then
            common_args="${common_args} --batata.plugin.consul.enabled=true"
        fi
        if [ "$APOLLO_ENABLED" = "true" ]; then
            common_args="${common_args} --batata.plugin.apollo.enabled=true"
        fi

        local deploy_mode="server"
        if [ "$WITH_CONSOLE" = "true" ]; then
            deploy_mode="merged"
        fi

        log_info "Starting Node 1 (:${NODE1_PORT}, consul:${NODE1_CONSUL_PORT}, mode=${deploy_mode}, cluster)..."
        nohup "$BINARY" \
            -d "$deploy_mode" \
            --batata.server.main.port=${NODE1_PORT} \
            --batata.console.port=${CONSOLE_PORT} \
            --batata.persistence.embedded.data_dir="${BASE_DIR}/data/node1" \
            --batata.logs.path="${BASE_DIR}/logs/node1" \
            --batata.plugin.consul.port=${NODE1_CONSUL_PORT} \
            --batata.plugin.apollo.port=${NODE1_APOLLO_PORT} \
            ${common_args} \
            > "${BASE_DIR}/logs/node1/stdout.log" 2>&1 &
        echo $! > "${CLUSTER_PID_DIR}/node1.pid"

        log_info "Starting Node 2 (:${NODE2_PORT}, consul:${NODE2_CONSUL_PORT}, console:${NODE2_CONSOLE_PORT}, mode=merged, cluster)..."
        nohup "$BINARY" \
            -d merged \
            --batata.server.main.port=${NODE2_PORT} \
            --batata.console.port=${NODE2_CONSOLE_PORT} \
            --batata.persistence.embedded.data_dir="${BASE_DIR}/data/node2" \
            --batata.logs.path="${BASE_DIR}/logs/node2" \
            --batata.plugin.consul.port=${NODE2_CONSUL_PORT} \
            --batata.plugin.apollo.port=${NODE2_APOLLO_PORT} \
            ${common_args} \
            > "${BASE_DIR}/logs/node2/stdout.log" 2>&1 &
        echo $! > "${CLUSTER_PID_DIR}/node2.pid"

        log_info "Starting Node 3 (:${NODE3_PORT}, consul:${NODE3_CONSUL_PORT}, console:${NODE3_CONSOLE_PORT}, mode=merged, cluster)..."
        nohup "$BINARY" \
            -d merged \
            --batata.server.main.port=${NODE3_PORT} \
            --batata.console.port=${NODE3_CONSOLE_PORT} \
            --batata.persistence.embedded.data_dir="${BASE_DIR}/data/node3" \
            --batata.logs.path="${BASE_DIR}/logs/node3" \
            --batata.plugin.consul.port=${NODE3_CONSUL_PORT} \
            --batata.plugin.apollo.port=${NODE3_APOLLO_PORT} \
            ${common_args} \
            > "${BASE_DIR}/logs/node3/stdout.log" 2>&1 &
        echo $! > "${CLUSTER_PID_DIR}/node3.pid"

        log_info "Waiting for cluster to form..."
        local max_wait=90 waited=0 ready=0
        while [ $waited -lt $max_wait ]; do
            ready=0
            for port in $NODE1_PORT $NODE2_PORT $NODE3_PORT; do
                local code
                code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:${port}/nacos/v3/admin/core/state/liveness" 2>/dev/null || echo "000")
                [ "$code" = "200" ] && ready=$((ready + 1))
            done
            [ $ready -eq 3 ] && break
            sleep 2; waited=$((waited + 2)); echo -n "."
        done
        echo ""

        if [ $waited -ge $max_wait ]; then
            log_error "Cluster failed to start within ${max_wait}s"
            check_status
            exit 1
        fi

        log_info "All 3 nodes are UP"

        log_info "Initializing admin user..."
        sleep 3
        bash "${SCRIPT_DIR}/init-admin.sh" nacos nacos "http://127.0.0.1:${NODE1_PORT}" || true

        sleep 2
        check_status
        log_info "Cluster is ready!"
        echo ""
        echo "  Batata SDK endpoints:"
        echo "    Node 1: 127.0.0.1:${NODE1_PORT} (gRPC: $((NODE1_PORT + 1000)))"
        echo "    Node 2: 127.0.0.1:${NODE2_PORT} (gRPC: $((NODE2_PORT + 1000)))"
        echo "    Node 3: 127.0.0.1:${NODE3_PORT} (gRPC: $((NODE3_PORT + 1000)))"
        if [ "$CONSUL_ENABLED" = "true" ]; then
            echo "  Consul API:"
            echo "    Node 1: 127.0.0.1:${NODE1_CONSUL_PORT}"
            echo "    Node 2: 127.0.0.1:${NODE2_CONSUL_PORT}"
            echo "    Node 3: 127.0.0.1:${NODE3_CONSUL_PORT}"
        fi
        if [ "$APOLLO_ENABLED" = "true" ]; then
            echo "  Apollo API:"
            echo "    Node 1: 127.0.0.1:${NODE1_APOLLO_PORT}"
            echo "    Node 2: 127.0.0.1:${NODE2_APOLLO_PORT}"
            echo "    Node 3: 127.0.0.1:${NODE3_APOLLO_PORT}"
        fi
        echo ""
        echo "  Stop with: ./scripts/startup.sh cluster stop"
    }

    ACTION="${1:-start}"
    case "$ACTION" in
        start)  stop_cluster 2>/dev/null || true; start_cluster ;;
        stop)   stop_cluster ;;
        status) check_status ;;
        *)      echo "Usage: $0 cluster {start|stop|status}"; exit 1 ;;
    esac
    exit 0
fi

# Parse options (compatible with Nacos startup.sh flags for migration convenience)
export MODE="cluster"
export DEPLOYMENT="merged"
export MEMBER_LIST=""
export EMBEDDED_STORAGE=""
export SERVER="batata-server"
export CONFIG_FILE=""
export REMOTE_ADDR=""
export DB_URL=""

while getopts ":m:d:s:c:p:C:R:U:" opt; do
    case $opt in
        m) MODE=$OPTARG ;;
        d) DEPLOYMENT=$OPTARG ;;
        s) SERVER=$OPTARG ;;
        c) MEMBER_LIST=$OPTARG ;;
        p) EMBEDDED_STORAGE=$OPTARG ;;
        C) CONFIG_FILE=$OPTARG ;;
        R) REMOTE_ADDR=$OPTARG ;;
        U) DB_URL=$OPTARG ;;
        ?)
            echo "Unknown parameter: -$OPTARG"
            echo "Usage: $0 [-m standalone|cluster] [-d merged|server|console|serverWithMcp] [-p embedded|mysql|postgres] [-c member_list] [-R remote_console_addr] [-U db_url]"
            exit 1
            ;;
    esac
done

export BASE_DIR=$(cd "$(dirname "$0")/.."; pwd)

#===========================================================================================
# Locate the server binary
#===========================================================================================

BATATA_BIN=""

# Check multiple locations
if [ -x "${BASE_DIR}/target/release/${SERVER}" ]; then
    BATATA_BIN="${BASE_DIR}/target/release/${SERVER}"
elif [ -x "${BASE_DIR}/target/debug/${SERVER}" ]; then
    BATATA_BIN="${BASE_DIR}/target/debug/${SERVER}"
elif [ -x "${BASE_DIR}/bin/${SERVER}" ]; then
    BATATA_BIN="${BASE_DIR}/bin/${SERVER}"
elif [ -x "${BASE_DIR}/${SERVER}" ]; then
    BATATA_BIN="${BASE_DIR}/${SERVER}"
elif command -v "${SERVER}" &>/dev/null; then
    BATATA_BIN=$(command -v "${SERVER}")
else
    # Try to build
    echo "Server binary not found. Building..."
    if command -v cargo &>/dev/null; then
        (cd "${BASE_DIR}" && cargo build --release -p batata-server)
        BATATA_BIN="${BASE_DIR}/target/release/${SERVER}"
    else
        error_exit "Server binary '${SERVER}' not found and cargo is not available for building"
    fi
fi

if [ ! -x "${BATATA_BIN}" ]; then
    error_exit "Server binary '${BATATA_BIN}' is not executable"
fi

echo "Using binary: ${BATATA_BIN}"

#===========================================================================================
# Build command-line arguments
#===========================================================================================

RUN_ARGS=""

# Deployment type
RUN_ARGS="${RUN_ARGS} -d ${DEPLOYMENT}"

# Standalone mode
if [[ "${MODE}" == "standalone" ]]; then
    RUN_ARGS="${RUN_ARGS} -m standalone"
fi

# Cluster member list
if [ -n "${MEMBER_LIST}" ]; then
    RUN_ARGS="${RUN_ARGS} --batata.member.list=${MEMBER_LIST}"
fi

# Storage platform
if [ -n "${EMBEDDED_STORAGE}" ]; then
    if [[ "${EMBEDDED_STORAGE}" == "embedded" ]]; then
        RUN_ARGS="${RUN_ARGS} --batata.sql.init.platform=embedded"
    else
        RUN_ARGS="${RUN_ARGS} --batata.sql.init.platform=${EMBEDDED_STORAGE}"
    fi
fi

# Config file
if [ -n "${CONFIG_FILE}" ]; then
    RUN_ARGS="${RUN_ARGS} -c ${CONFIG_FILE}"
fi

# Remote console address (console-only deployment)
if [ -n "${REMOTE_ADDR}" ]; then
    RUN_ARGS="${RUN_ARGS} --batata.console.remote.server_addr=${REMOTE_ADDR}"
fi

# External database URL
if [ -n "${DB_URL}" ]; then
    RUN_ARGS="${RUN_ARGS} --db-url=${DB_URL}"
fi

#===========================================================================================
# Prepare directories
#===========================================================================================

if [ ! -d "${BASE_DIR}/logs" ]; then
    mkdir -p "${BASE_DIR}/logs"
fi

if [ ! -d "${BASE_DIR}/data" ]; then
    mkdir -p "${BASE_DIR}/data"
fi

#===========================================================================================
# Environment variables
#===========================================================================================

# Allow tuning via environment
export RUST_LOG="${RUST_LOG:-info}"
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"

#===========================================================================================
# Start application
#===========================================================================================

LOGFILE="${BASE_DIR}/logs/startup.log"

if [ ! -f "${LOGFILE}" ]; then
    touch "${LOGFILE}"
fi

echo "Batata is starting with ${MODE} mode, deployment: ${DEPLOYMENT}"
echo "CMD: ${BATATA_BIN} ${RUN_ARGS}" | tee "${LOGFILE}"

nohup "${BATATA_BIN}" ${RUN_ARGS} >> "${LOGFILE}" 2>&1 &
PID=$!

echo "Batata is starting (PID: ${PID}), you can check ${LOGFILE}"
echo "${PID}" > "${BASE_DIR}/logs/batata.pid"

# Wait for server to be ready (max 60 seconds).
# server-only deployments have no console port, so probe the main/admin liveness.
if [ "${DEPLOYMENT}" = "server" ]; then
    LIVENESS_URL="http://127.0.0.1:8848/nacos/v3/admin/core/state/liveness"
else
    LIVENESS_URL="http://127.0.0.1:8081/v3/console/health/liveness"
fi
echo -n "Waiting for server to start"
for i in $(seq 1 30); do
    if curl -s -o /dev/null -w "%{http_code}" "${LIVENESS_URL}" 2>/dev/null | grep -q 200; then
        echo ""
        echo "Batata started successfully (PID: ${PID})"
        exit 0
    fi
    # Check if process is still running
    if ! kill -0 "${PID}" 2>/dev/null; then
        echo ""
        echo "ERROR: Batata process exited unexpectedly. Check ${LOGFILE}"
        exit 1
    fi
    echo -n "."
    sleep 2
done

echo ""
echo "WARNING: Server may still be starting. Check ${LOGFILE}"
