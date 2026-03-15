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
# Batata shutdown script (Nacos-compatible)
#
# Sends SIGTERM to the Batata server process for graceful shutdown.
# The server will drain in-flight requests and clean up before exiting.
#
# Usage:
#   ./shutdown.sh
#===========================================================================================

BASE_DIR=$(cd "$(dirname "$0")/.."; pwd)
PID_FILE="${BASE_DIR}/logs/batata.pid"

# Try to read PID from file first
if [ -f "${PID_FILE}" ]; then
    pid=$(cat "${PID_FILE}" 2>/dev/null)
    if [ -n "${pid}" ] && kill -0 "${pid}" 2>/dev/null; then
        echo "The Batata server (PID: ${pid}) is running..."
        kill "${pid}"
        echo "Send shutdown request to Batata server (PID: ${pid}) OK"

        # Wait for graceful shutdown (max 30 seconds)
        echo -n "Waiting for shutdown"
        for i in $(seq 1 30); do
            if ! kill -0 "${pid}" 2>/dev/null; then
                echo ""
                echo "Batata server stopped successfully."
                rm -f "${PID_FILE}"
                exit 0
            fi
            echo -n "."
            sleep 1
        done

        echo ""
        echo "WARNING: Server did not stop within 30 seconds. Sending SIGKILL..."
        kill -9 "${pid}" 2>/dev/null
        rm -f "${PID_FILE}"
        echo "Batata server force killed."
        exit 0
    fi
fi

# Fallback: find process by name
pid=$(pgrep -f "batata-server" | head -1)

if [ -z "${pid}" ]; then
    echo "No Batata server running."
    exit 1
fi

echo "The Batata server (PID: ${pid}) is running..."
kill "${pid}"
echo "Send shutdown request to Batata server (PID: ${pid}) OK"

# Wait for graceful shutdown
echo -n "Waiting for shutdown"
for i in $(seq 1 30); do
    if ! kill -0 "${pid}" 2>/dev/null; then
        echo ""
        echo "Batata server stopped successfully."
        rm -f "${PID_FILE}" 2>/dev/null
        exit 0
    fi
    echo -n "."
    sleep 1
done

echo ""
echo "WARNING: Server did not stop within 30 seconds. Sending SIGKILL..."
kill -9 "${pid}" 2>/dev/null
rm -f "${PID_FILE}" 2>/dev/null
echo "Batata server force killed."
