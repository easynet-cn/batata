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

# Parse options (compatible with Nacos startup.sh)
export MODE="cluster"
export DEPLOYMENT="merged"
export MEMBER_LIST=""
export EMBEDDED_STORAGE=""
export SERVER="batata-server"
export CONFIG_FILE=""

while getopts ":m:d:s:c:p:C:" opt; do
    case $opt in
        m) MODE=$OPTARG ;;
        d) DEPLOYMENT=$OPTARG ;;
        s) SERVER=$OPTARG ;;
        c) MEMBER_LIST=$OPTARG ;;
        p) EMBEDDED_STORAGE=$OPTARG ;;
        C) CONFIG_FILE=$OPTARG ;;
        ?)
            echo "Unknown parameter: -$OPTARG"
            echo "Usage: $0 [-m standalone|cluster] [-d merged|server|console|serverWithMcp] [-p embedded|mysql|postgres] [-c member_list]"
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

# Wait for server to be ready (max 60 seconds)
echo -n "Waiting for server to start"
for i in $(seq 1 30); do
    if curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:8081/v3/console/health/liveness" 2>/dev/null | grep -q 200; then
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
