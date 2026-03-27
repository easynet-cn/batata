#!/usr/bin/env bash
# ==============================================================================
# Batata Performance Benchmark Script
#
# Benchmarks the core APIs: Config Center, Service Registry (naming),
# and Consul compatibility endpoints.
#
# Prerequisites:
#   - Batata server running (single-node or cluster)
#   - wrk installed: brew install wrk  (or apt install wrk)
#   - hey installed: go install github.com/rakyll/hey@latest (optional)
#   - Admin user initialized (nacos/nacos)
#
# Usage:
#   ./scripts/benchmark.sh                  # Full benchmark (default)
#   ./scripts/benchmark.sh config           # Config center only
#   ./scripts/benchmark.sh naming           # Naming/registry only
#   ./scripts/benchmark.sh consul           # Consul API only
#   ./scripts/benchmark.sh quick            # Quick smoke test (5s each)
#
# Environment:
#   SERVER=127.0.0.1:8848   Nacos server address
#   CONSUL=127.0.0.1:8500   Consul server address
#   THREADS=4               Concurrent threads
#   DURATION=30s            Test duration
#   CONNECTIONS=100         Concurrent connections
# ==============================================================================

set -uo pipefail
cd "$(dirname "$0")/.."

# Configuration
SERVER="${SERVER:-127.0.0.1:8848}"
CONSUL="${CONSUL:-127.0.0.1:8500}"
THREADS="${THREADS:-4}"
DURATION="${DURATION:-30s}"
CONNECTIONS="${CONNECTIONS:-100}"
USERNAME="${USERNAME:-nacos}"
PASSWORD="${PASSWORD:-nacos}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

log_section() { echo -e "\n${BOLD}${BLUE}=== $1 ===${NC}"; }
log_info()    { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn()    { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_result()  { echo -e "  ${BOLD}$1${NC}: $2"; }

# Check prerequisites
check_tools() {
    if ! command -v wrk &>/dev/null && ! command -v hey &>/dev/null; then
        echo "Neither 'wrk' nor 'hey' found. Install one:"
        echo "  brew install wrk"
        echo "  go install github.com/rakyll/hey@latest"
        exit 1
    fi
}

# Get auth token
get_token() {
    local token
    token=$(curl -s -X POST "http://${SERVER}/nacos/v3/auth/user/login" \
        -d "username=${USERNAME}&password=${PASSWORD}" 2>/dev/null | \
        python3 -c "import sys,json;print(json.load(sys.stdin).get('accessToken',''))" 2>/dev/null)
    echo "$token"
}

# Run benchmark with wrk
run_wrk() {
    local name="$1"
    local method="$2"
    local url="$3"
    local body="${4:-}"
    local extra_headers="${5:-}"

    if ! command -v wrk &>/dev/null; then
        log_warn "wrk not installed, skipping: $name"
        return
    fi

    local lua_script=""
    if [ "$method" = "POST" ] || [ "$method" = "PUT" ]; then
        lua_script=$(mktemp /tmp/wrk_benchXXXXXX)
        mv "$lua_script" "${lua_script}.lua"
        lua_script="${lua_script}.lua"
        {
            echo "wrk.method = \"${method}\""
            echo 'wrk.headers["Content-Type"] = "application/x-www-form-urlencoded"'
            [ -n "$extra_headers" ] && echo "$extra_headers"
            echo "wrk.body = \"${body}\""
        } > "$lua_script"
    elif [ -n "$extra_headers" ]; then
        lua_script=$(mktemp /tmp/wrk_benchXXXXXX)
        mv "$lua_script" "${lua_script}.lua"
        lua_script="${lua_script}.lua"
        echo "$extra_headers" > "$lua_script"
    fi

    echo -e "  ${YELLOW}▶${NC} $name ($method $url)"

    local wrk_args="-t${THREADS} -c${CONNECTIONS} -d${DURATION}"
    if [ -n "$lua_script" ]; then
        wrk_args="${wrk_args} -s ${lua_script}"
    fi

    local result
    result=$(wrk ${wrk_args} "$url" 2>&1)

    # Extract key metrics (use || true to prevent set -e from aborting on missing lines)
    local rps latency_avg latency_p99
    rps=$(echo "$result" | grep "Requests/sec" | awk '{print $2}' || true)
    latency_avg=$(echo "$result" | grep "Latency" | head -1 | awk '{print $2}' || true)
    latency_p99=$(echo "$result" | grep "99%" | awk '{print $2}' || true)

    log_result "RPS" "${rps:-N/A}"
    log_result "Avg Latency" "${latency_avg:-N/A}"
    log_result "P99 Latency" "${latency_p99:-N/A}"

    [ -n "$lua_script" ] && rm -f "$lua_script"
}

# Run benchmark with hey
run_hey() {
    local name="$1"
    local method="$2"
    local url="$3"
    local body="${4:-}"

    if ! command -v hey &>/dev/null; then
        log_warn "hey not installed, skipping: $name"
        return
    fi

    local dur_secs="${DURATION%s}"
    local total=$((CONNECTIONS * dur_secs))

    echo -e "  ${YELLOW}▶${NC} $name ($method $url)"

    local hey_args="-n ${total} -c ${CONNECTIONS} -m ${method}"
    if [ -n "$body" ]; then
        hey_args="${hey_args} -T application/x-www-form-urlencoded -d ${body}"
    fi

    local result
    result=$(hey ${hey_args} "$url" 2>&1)

    local rps
    rps=$(echo "$result" | grep "Requests/sec" | awk '{print $2}')
    local latency_avg
    latency_avg=$(echo "$result" | grep "Average" | head -1 | awk '{print $2}')

    log_result "RPS" "${rps:-N/A}"
    log_result "Avg Latency" "${latency_avg:-N/A} secs"
}

# Setup test data
setup_data() {
    log_info "Setting up benchmark data..." >&2
    local token
    token=$(get_token)

    # Create test config
    curl -s -X POST "http://${SERVER}/nacos/v2/cs/config" \
        -H "Authorization: Bearer ${token}" \
        -d "dataId=bench-config&group=DEFAULT_GROUP&content=benchmark.key=value&type=text" > /dev/null 2>&1

    # Register test instance
    curl -s -X POST "http://${SERVER}/nacos/v2/ns/instance" \
        -H "Authorization: Bearer ${token}" \
        -d "serviceName=bench-service&ip=10.0.0.1&port=8080&ephemeral=true" > /dev/null 2>&1

    # Consul KV
    curl -s -X PUT "http://${CONSUL}/v1/kv/bench/key1" -d "benchmark-value" > /dev/null 2>&1

    # Consul service
    curl -s -X PUT "http://${CONSUL}/v1/agent/service/register" \
        -H "Content-Type: application/json" \
        -d '{"ID":"bench-svc","Name":"bench-service","Port":8080,"Address":"10.0.0.1"}' > /dev/null 2>&1

    sleep 1
    log_info "Data ready" >&2
    echo "$token"
}

# Cleanup
cleanup_data() {
    local token="$1"
    log_info "Cleaning up..."
    curl -s -X DELETE "http://${SERVER}/nacos/v2/cs/config?dataId=bench-config&group=DEFAULT_GROUP" \
        -H "Authorization: Bearer ${token}" > /dev/null 2>&1
    curl -s -X DELETE "http://${SERVER}/nacos/v2/ns/instance?serviceName=bench-service&ip=10.0.0.1&port=8080&ephemeral=true" \
        -H "Authorization: Bearer ${token}" > /dev/null 2>&1
    curl -s -X DELETE "http://${CONSUL}/v1/kv/bench/key1" > /dev/null 2>&1
    curl -s -X PUT "http://${CONSUL}/v1/agent/service/deregister/bench-svc" > /dev/null 2>&1
}

# ==================== Benchmark Suites ====================

bench_config() {
    local token="$1"
    log_section "Config Center Benchmark"

    # Config Write (POST) — uses a fixed dataId for repeated writes (upsert benchmark)
    run_wrk "Config Write" "POST" \
        "http://${SERVER}/nacos/v2/cs/config?accessToken=${token}" \
        "dataId=bench-write&group=DEFAULT_GROUP&content=benchmark-value&type=text"

    # Config Read (GET)
    run_wrk "Config Read" "GET" \
        "http://${SERVER}/nacos/v2/cs/config?dataId=bench-config&group=DEFAULT_GROUP&accessToken=${token}"
}

bench_naming() {
    local token="$1"
    log_section "Service Registry Benchmark"

    # Instance Register (POST)
    run_wrk "Instance Register" "POST" \
        "http://${SERVER}/nacos/v2/ns/instance?accessToken=${token}" \
        "serviceName=bench-reg&ip=10.0.0.1&port=8080&ephemeral=true&weight=1.0"

    # Instance Query (GET)
    run_wrk "Instance Query" "GET" \
        "http://${SERVER}/nacos/v2/ns/instance/list?serviceName=bench-service&accessToken=${token}"

    # Service List (GET)
    run_wrk "Service List" "GET" \
        "http://${SERVER}/nacos/v2/ns/service/list?pageNo=1&pageSize=10&accessToken=${token}"
}

bench_consul() {
    log_section "Consul API Benchmark"

    # KV Read
    run_wrk "Consul KV Read" "GET" \
        "http://${CONSUL}/v1/kv/bench/key1"

    # KV Write
    run_wrk "Consul KV Write" "PUT" \
        "http://${CONSUL}/v1/kv/bench/wrk-key" \
        "wrk-bench-value"

    # Service Catalog
    run_wrk "Consul Catalog Services" "GET" \
        "http://${CONSUL}/v1/catalog/services"

    # Health Check
    run_wrk "Consul Health Service" "GET" \
        "http://${CONSUL}/v1/health/service/bench-service"

    # Agent Self
    run_wrk "Consul Agent Self" "GET" \
        "http://${CONSUL}/v1/agent/self"
}

# ==================== Main ====================

check_tools

MODE="${1:-full}"

# Quick mode: 5s duration, fewer connections
if [ "$MODE" = "quick" ]; then
    DURATION="5s"
    CONNECTIONS=20
    THREADS=2
fi

echo -e "${BOLD}Batata Performance Benchmark${NC}"
echo "  Server:      ${SERVER}"
echo "  Consul:      ${CONSUL}"
echo "  Threads:     ${THREADS}"
echo "  Connections: ${CONNECTIONS}"
echo "  Duration:    ${DURATION}"
echo ""

TOKEN=$(setup_data)

case "$MODE" in
    config)  bench_config "$TOKEN" ;;
    naming)  bench_naming "$TOKEN" ;;
    consul)  bench_consul ;;
    quick|full)
        bench_config "$TOKEN"
        bench_naming "$TOKEN"
        bench_consul
        ;;
    *)
        echo "Usage: $0 {full|config|naming|consul|quick}"
        exit 1
        ;;
esac

cleanup_data "$TOKEN"
echo ""
log_info "Benchmark complete!"
