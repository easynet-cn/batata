#!/usr/bin/env bash
# ==============================================================================
# Common test utilities for Batata console datasource testing
# ==============================================================================

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Counters
PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
TOTAL_COUNT=0

# Default credentials
USERNAME="${USERNAME:-nacos}"
PASSWORD="${PASSWORD:-nacos}"

# Default ports
MAIN_PORT="${MAIN_PORT:-8848}"
CONSOLE_PORT="${CONSOLE_PORT:-8081}"

# Derived URLs
MAIN_URL="http://127.0.0.1:${MAIN_PORT}"
CONSOLE_URL="http://127.0.0.1:${CONSOLE_PORT}"

# Token (populated by login)
TOKEN=""

# Unique test ID suffix
TEST_ID="$(date +%s%N)"

# ==============================================================================
# Logging helpers
# ==============================================================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $*"
}

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $*"
    ((PASS_COUNT++))
    ((TOTAL_COUNT++))
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $*"
    ((FAIL_COUNT++))
    ((TOTAL_COUNT++))
}

log_skip() {
    echo -e "${YELLOW}[SKIP]${NC} $*"
    ((SKIP_COUNT++))
    ((TOTAL_COUNT++))
}

log_section() {
    echo ""
    echo -e "${CYAN}========================================${NC}"
    echo -e "${CYAN} $*${NC}"
    echo -e "${CYAN}========================================${NC}"
}

# ==============================================================================
# HTTP helpers
# ==============================================================================

# Login and set TOKEN
# Usage: do_login [url] [username] [password]
do_login() {
    local url="${1:-$CONSOLE_URL}"
    local user="${2:-$USERNAME}"
    local pass="${3:-$PASSWORD}"

    local response
    response=$(curl -s -w "\n%{http_code}" -X POST \
        "${url}/v3/auth/user/login" \
        -d "username=${user}&password=${pass}" 2>/dev/null)

    local http_code
    http_code=$(echo "$response" | tail -1)
    local body
    body=$(echo "$response" | sed '$d')

    if [ "$http_code" = "200" ]; then
        TOKEN=$(echo "$body" | jq -r '.data.accessToken // .accessToken // empty')
        if [ -n "$TOKEN" ]; then
            log_info "Login successful (token=${TOKEN:0:20}...)"
            return 0
        fi
    fi

    log_fail "Login failed (HTTP ${http_code}): ${body}"
    return 1
}

# Generic HTTP request
# Usage: http_request METHOD URL [data] [content_type]
# Returns: Sets HTTP_CODE, HTTP_BODY global variables
HTTP_CODE=""
HTTP_BODY=""

http_request() {
    local method="$1"
    local url="$2"
    local data="$3"
    local content_type="${4:-application/x-www-form-urlencoded}"

    local auth_param=""
    if [ -n "$TOKEN" ]; then
        auth_param="accessToken=${TOKEN}"
    fi

    # Append token to URL
    if [[ "$url" == *"?"* ]]; then
        url="${url}&${auth_param}"
    else
        url="${url}?${auth_param}"
    fi

    local response
    if [ -n "$data" ]; then
        response=$(curl -s -w "\n%{http_code}" -X "$method" \
            -H "Content-Type: ${content_type}" \
            -d "$data" \
            "$url" 2>/dev/null)
    else
        response=$(curl -s -w "\n%{http_code}" -X "$method" \
            "$url" 2>/dev/null)
    fi

    HTTP_CODE=$(echo "$response" | tail -1)
    HTTP_BODY=$(echo "$response" | sed '$d')
}

# Convenience wrappers
console_get() {
    http_request GET "${CONSOLE_URL}$1"
}

console_post_form() {
    http_request POST "${CONSOLE_URL}$1" "$2"
}

console_post_json() {
    http_request POST "${CONSOLE_URL}$1" "$2" "application/json"
}

console_put_form() {
    http_request PUT "${CONSOLE_URL}$1" "$2"
}

console_put_json() {
    http_request PUT "${CONSOLE_URL}$1" "$2" "application/json"
}

console_delete() {
    http_request DELETE "${CONSOLE_URL}$1"
}

main_get() {
    http_request GET "${MAIN_URL}$1"
}

main_post_form() {
    http_request POST "${MAIN_URL}$1" "$2"
}

main_delete() {
    http_request DELETE "${MAIN_URL}$1"
}

# ==============================================================================
# Assertion helpers
# ==============================================================================

# Assert HTTP status code
# Usage: assert_http_code EXPECTED TEST_NAME
assert_http_code() {
    local expected="$1"
    local test_name="$2"
    if [ "$HTTP_CODE" = "$expected" ]; then
        return 0
    else
        log_fail "${test_name} - Expected HTTP ${expected}, got ${HTTP_CODE}"
        echo "  Response: ${HTTP_BODY:0:200}"
        return 1
    fi
}

# Assert response code field equals 0 (success)
# Usage: assert_success TEST_NAME
assert_success() {
    local test_name="$1"
    local code
    code=$(echo "$HTTP_BODY" | jq -r '.code // empty' 2>/dev/null)

    if [ "$HTTP_CODE" = "200" ] && [ "$code" = "0" ]; then
        log_pass "$test_name"
        return 0
    else
        log_fail "${test_name} (HTTP=${HTTP_CODE}, code=${code})"
        echo "  Response: ${HTTP_BODY:0:300}"
        return 1
    fi
}

# Assert response data field equals expected value
# Usage: assert_data_equals EXPECTED TEST_NAME [jq_path]
assert_data_equals() {
    local expected="$1"
    local test_name="$2"
    local jq_path="${3:-.data}"

    local actual
    actual=$(echo "$HTTP_BODY" | jq -r "${jq_path} // empty" 2>/dev/null)

    if [ "$actual" = "$expected" ]; then
        log_pass "$test_name"
        return 0
    else
        log_fail "${test_name} - Expected '${expected}', got '${actual}'"
        return 1
    fi
}

# Assert response data is not empty/null
# Usage: assert_data_not_empty TEST_NAME [jq_path]
assert_data_not_empty() {
    local test_name="$1"
    local jq_path="${2:-.data}"

    local actual
    actual=$(echo "$HTTP_BODY" | jq -r "${jq_path} // empty" 2>/dev/null)

    if [ -n "$actual" ] && [ "$actual" != "null" ]; then
        log_pass "$test_name"
        return 0
    else
        log_fail "${test_name} - Data is empty or null"
        echo "  Response: ${HTTP_BODY:0:300}"
        return 1
    fi
}

# Assert response data array length
# Usage: assert_data_length EXPECTED_OP EXPECTED_VAL TEST_NAME [jq_path]
# EXPECTED_OP: -eq, -ge, -gt, -le, -lt
assert_data_length() {
    local op="$1"
    local expected="$2"
    local test_name="$3"
    local jq_path="${4:-.data}"

    local length
    length=$(echo "$HTTP_BODY" | jq "${jq_path} | length" 2>/dev/null)

    if [ -z "$length" ] || [ "$length" = "null" ]; then
        log_fail "${test_name} - Cannot get array length"
        return 1
    fi

    if [ "$length" "$op" "$expected" ]; then
        log_pass "${test_name} (length=${length})"
        return 0
    else
        log_fail "${test_name} - Expected length ${op} ${expected}, got ${length}"
        return 1
    fi
}

# ==============================================================================
# Server lifecycle helpers
# ==============================================================================

# Wait for server to be ready
# Usage: wait_for_server URL [timeout_secs]
wait_for_server() {
    local url="$1"
    local timeout="${2:-60}"
    local interval=2
    local elapsed=0

    log_info "Waiting for server at ${url} (timeout=${timeout}s)..."

    while [ $elapsed -lt $timeout ]; do
        local http_code
        http_code=$(curl -s -o /dev/null -w "%{http_code}" "${url}/v3/console/health/liveness" 2>/dev/null)
        if [ "$http_code" = "200" ]; then
            log_info "Server is ready (took ${elapsed}s)"
            return 0
        fi
        sleep $interval
        elapsed=$((elapsed + interval))
    done

    log_fail "Server at ${url} did not become ready within ${timeout}s"
    return 1
}

# Start batata server in background
# Usage: start_server [args...]
# Returns: Sets SERVER_PID global variable
SERVER_PID=""

start_server() {
    local args=("$@")
    local project_root
    project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

    log_info "Starting server: cargo run -p batata-server -- ${args[*]}"

    cd "$project_root" || return 1
    cargo run -p batata-server -- "${args[@]}" > /tmp/batata_test_server.log 2>&1 &
    SERVER_PID=$!

    log_info "Server started with PID ${SERVER_PID}"
}

# Stop server by PID
# Usage: stop_server [pid]
stop_server() {
    local pid="${1:-$SERVER_PID}"
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
        log_info "Stopping server (PID=${pid})..."
        kill "$pid" 2>/dev/null
        wait "$pid" 2>/dev/null
        log_info "Server stopped"
    fi
}

# ==============================================================================
# Cleanup helpers
# ==============================================================================

# Generate unique test resource names
unique_ns_id() {
    echo "test-ns-${TEST_ID}"
}

unique_data_id() {
    local prefix="${1:-cfg}"
    echo "${prefix}-${TEST_ID}"
}

unique_service_name() {
    local prefix="${1:-svc}"
    echo "${prefix}-${TEST_ID}"
}

# ==============================================================================
# Summary
# ==============================================================================

print_summary() {
    echo ""
    echo -e "${CYAN}========================================${NC}"
    echo -e "${CYAN} Test Summary${NC}"
    echo -e "${CYAN}========================================${NC}"
    echo -e " Total:   ${TOTAL_COUNT}"
    echo -e " ${GREEN}Passed:  ${PASS_COUNT}${NC}"
    echo -e " ${RED}Failed:  ${FAIL_COUNT}${NC}"
    echo -e " ${YELLOW}Skipped: ${SKIP_COUNT}${NC}"
    echo ""

    if [ "$FAIL_COUNT" -gt 0 ]; then
        echo -e "${RED}SOME TESTS FAILED${NC}"
        return 1
    else
        echo -e "${GREEN}ALL TESTS PASSED${NC}"
        return 0
    fi
}
