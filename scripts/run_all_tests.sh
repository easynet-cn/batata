#!/usr/bin/env bash
# ==============================================================================
# Comprehensive Test Automation for Batata
#
# Orchestrates all test layers — from unit tests through SDK compatibility tests
# — with automatic server lifecycle management.
#
# Usage:
#   ./scripts/run_all_tests.sh [level] [options]
#
# Levels:
#   quick     (default) cargo fmt check + clippy + unit tests
#   standard  quick + standalone embedded E2E tests
#   full      standard + cluster embedded E2E + external DB E2E (requires Docker)
#   sdk       full + Nacos Java SDK + Consul Go SDK tests (requires Docker)
#
# Options:
#   --skip-build   Skip cargo build --release (reuse existing binary)
#   --help         Show this help message
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

source "${SCRIPT_DIR}/test_utils.sh"

# ==============================================================================
# Configuration
# ==============================================================================

LEVEL="${1:-quick}"
SKIP_BUILD=false
BINARY="${PROJECT_ROOT}/target/release/batata-server"
SERVER_PIDS=()
DOCKER_COMPOSE_STARTED=false
SDK_DOCKER_STARTED=false
CLUSTER_CONF_BACKED_UP=false
OVERALL_EXIT_CODE=0
STAGE_RESULTS=()

# Parse options
for arg in "$@"; do
    case "$arg" in
        --skip-build) SKIP_BUILD=true ;;
        --help|-h)
            head -20 "$0" | tail -17
            exit 0
            ;;
        quick|standard|full|sdk) LEVEL="$arg" ;;
        *)
            echo "Unknown argument: $arg"
            echo "Usage: $0 [quick|standard|full|sdk] [--skip-build] [--help]"
            exit 1
            ;;
    esac
done

# ==============================================================================
# Cleanup trap
# ==============================================================================

cleanup() {
    log_info "Cleaning up..."

    # Kill all tracked server PIDs
    for pid in "${SERVER_PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            log_info "Stopping server PID ${pid}..."
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
    done
    SERVER_PIDS=()

    # Stop Docker containers if started
    if [ "$DOCKER_COMPOSE_STARTED" = true ]; then
        log_info "Stopping test databases..."
        cd "$PROJECT_ROOT"
        docker-compose -f docker-compose.test.yml down -v 2>/dev/null || true
    fi

    if [ "$SDK_DOCKER_STARTED" = true ]; then
        log_info "Stopping SDK test stack..."
        cd "${PROJECT_ROOT}/sdk-tests"
        docker-compose down -v 2>/dev/null || true
        cd "$PROJECT_ROOT"
    fi

    # Remove temporary test data
    rm -rf "${PROJECT_ROOT}/data/test-node"* "${PROJECT_ROOT}/logs/test-node"* 2>/dev/null || true

    # Restore cluster.conf if backed up
    if [ "$CLUSTER_CONF_BACKED_UP" = true ] && [ -f "${PROJECT_ROOT}/conf/cluster.conf.bak" ]; then
        mv "${PROJECT_ROOT}/conf/cluster.conf.bak" "${PROJECT_ROOT}/conf/cluster.conf"
        log_info "Restored cluster.conf"
    fi
}
trap cleanup EXIT

# ==============================================================================
# Helper functions
# ==============================================================================

# Start a batata server process in background using release binary
# Usage: start_bg_server LOG_NAME [args...]
start_bg_server() {
    local log_name="$1"
    shift
    local log_file="/tmp/batata_test_${log_name}.log"

    log_info "Starting server (${log_name}): ${BINARY} $*"
    "$BINARY" "$@" > "$log_file" 2>&1 &
    local pid=$!
    SERVER_PIDS+=("$pid")
    log_info "Server started with PID ${pid} (log: ${log_file})"
    echo "$pid"
}

# Stop a specific server by PID
stop_bg_server() {
    local pid="$1"
    if kill -0 "$pid" 2>/dev/null; then
        log_info "Stopping server PID ${pid}..."
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    fi
    # Remove from tracked PIDs
    local new_pids=()
    for p in "${SERVER_PIDS[@]}"; do
        if [ "$p" != "$pid" ]; then
            new_pids+=("$p")
        fi
    done
    SERVER_PIDS=("${new_pids[@]+"${new_pids[@]}"}")
}

# Stop all tracked servers
stop_all_servers() {
    for pid in "${SERVER_PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            log_info "Stopping server PID ${pid}..."
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
    done
    SERVER_PIDS=()
}

# Initialize admin user on a server
# Usage: init_admin [server_url]
init_admin() {
    local server_url="${1:-http://127.0.0.1:8848}"
    local max_retries=3
    local retry=0

    while [ $retry -lt $max_retries ]; do
        local response
        response=$(curl -s -w "\n%{http_code}" -X POST \
            "${server_url}/nacos/v3/auth/user/admin" \
            -d "username=nacos&password=nacos" 2>/dev/null)
        local http_code
        http_code=$(echo "$response" | tail -1)

        if [ "$http_code" = "200" ] || [ "$http_code" = "409" ]; then
            log_info "Admin user initialized on ${server_url}"
            return 0
        fi
        retry=$((retry + 1))
        sleep 2
    done

    log_fail "Failed to initialize admin user on ${server_url}"
    return 1
}

# Wait for Docker service to be healthy
# Usage: wait_for_docker_healthy SERVICE_NAME [timeout_secs]
wait_for_docker_healthy() {
    local service="$1"
    local timeout="${2:-120}"
    local elapsed=0

    log_info "Waiting for Docker service '${service}' to be healthy (timeout=${timeout}s)..."

    while [ $elapsed -lt $timeout ]; do
        local status
        status=$(docker inspect --format='{{.State.Health.Status}}' "$service" 2>/dev/null || echo "not_found")
        if [ "$status" = "healthy" ]; then
            log_info "Docker service '${service}' is healthy (took ${elapsed}s)"
            return 0
        fi
        sleep 5
        elapsed=$((elapsed + 5))
    done

    log_fail "Docker service '${service}' did not become healthy within ${timeout}s"
    return 1
}

# Record stage result
record_stage() {
    local name="$1"
    local result="$2"
    STAGE_RESULTS+=("${result}:${name}")
}

# Check if a command exists
require_cmd() {
    local cmd="$1"
    if ! command -v "$cmd" &>/dev/null; then
        log_fail "Required command not found: ${cmd}"
        return 1
    fi
}

# ==============================================================================
# Stage: Quick (cargo fmt + clippy + unit tests)
# ==============================================================================

run_quick() {
    log_section "Stage: Quick (fmt + clippy + unit tests)"

    cd "$PROJECT_ROOT"

    # cargo fmt check
    log_info "Running cargo fmt --all --check..."
    if cargo fmt --all --check 2>&1; then
        log_pass "cargo fmt check"
    else
        log_fail "cargo fmt check — run 'cargo fmt --all' to fix"
        return 1
    fi

    # cargo clippy
    log_info "Running cargo clippy --workspace..."
    if cargo clippy --workspace 2>&1; then
        log_pass "cargo clippy"
    else
        log_fail "cargo clippy"
        return 1
    fi

    # cargo test
    log_info "Running cargo test --workspace..."
    if cargo test --workspace 2>&1; then
        log_pass "cargo test --workspace"
    else
        log_fail "cargo test --workspace"
        return 1
    fi

    return 0
}

# ==============================================================================
# Stage: Standard (standalone embedded E2E)
# ==============================================================================

run_standard() {
    log_section "Stage: Standard (standalone embedded E2E)"

    cd "$PROJECT_ROOT"

    # Build release binary
    if [ "$SKIP_BUILD" = true ] && [ -f "$BINARY" ]; then
        log_info "Skipping build (--skip-build, binary exists)"
    else
        log_info "Building release binary..."
        if cargo build --release -p batata-server 2>&1; then
            log_pass "cargo build --release"
        else
            log_fail "cargo build --release"
            return 1
        fi
    fi

    # Start server in standalone embedded mode
    local pid
    pid=$(start_bg_server "standalone_embedded" \
        --batata.sql.init.platform=embedded)

    # Wait for server ready
    if ! wait_for_server "http://127.0.0.1:8081" 90; then
        log_fail "Standalone embedded server failed to start"
        echo "--- Server log (last 50 lines) ---"
        tail -50 /tmp/batata_test_standalone_embedded.log 2>/dev/null || true
        echo "---"
        stop_bg_server "$pid"
        return 1
    fi

    # Initialize admin user
    init_admin "http://127.0.0.1:8848"

    # Run standalone embedded tests
    log_info "Running standalone embedded E2E tests..."
    local test_rc=0
    bash "${SCRIPT_DIR}/test_standalone_embedded.sh" || test_rc=$?

    # Stop server
    stop_bg_server "$pid"

    if [ $test_rc -ne 0 ]; then
        log_fail "Standalone embedded E2E tests failed (exit code: ${test_rc})"
        return 1
    fi

    log_pass "Standalone embedded E2E tests completed"
    return 0
}

# ==============================================================================
# Stage: Full — Cluster Embedded E2E
# ==============================================================================

run_cluster_embedded() {
    log_section "Stage: Full — Cluster Embedded E2E"

    cd "$PROJECT_ROOT"

    # Backup and write cluster.conf
    if [ -f "${PROJECT_ROOT}/conf/cluster.conf" ]; then
        cp "${PROJECT_ROOT}/conf/cluster.conf" "${PROJECT_ROOT}/conf/cluster.conf.bak"
        CLUSTER_CONF_BACKED_UP=true
    fi
    cat > "${PROJECT_ROOT}/conf/cluster.conf" <<'EOF'
127.0.0.1:8848
127.0.0.1:8858
127.0.0.1:8868
EOF

    # Start 3 nodes with separate data dirs
    local pid1 pid2 pid3

    pid1=$(start_bg_server "cluster_node1" \
        -m cluster \
        --batata.sql.init.platform=embedded \
        --batata.server.main.port=8848 --batata.console.port=8081 \
        --batata.persistence.embedded.data_dir=data/test-node1 \
        --batata.logs.path=logs/test-node1)

    pid2=$(start_bg_server "cluster_node2" \
        -m cluster \
        --batata.sql.init.platform=embedded \
        --batata.server.main.port=8858 --batata.console.port=8082 \
        --batata.persistence.embedded.data_dir=data/test-node2 \
        --batata.logs.path=logs/test-node2)

    pid3=$(start_bg_server "cluster_node3" \
        -m cluster \
        --batata.sql.init.platform=embedded \
        --batata.server.main.port=8868 --batata.console.port=8083 \
        --batata.persistence.embedded.data_dir=data/test-node3 \
        --batata.logs.path=logs/test-node3)

    # Wait for all nodes
    local all_ready=true
    for port in 8081 8082 8083; do
        if ! wait_for_server "http://127.0.0.1:${port}" 120; then
            log_fail "Cluster node on console port ${port} failed to start"
            all_ready=false
        fi
    done

    if [ "$all_ready" = false ]; then
        echo "--- Node 1 log (last 30 lines) ---"
        tail -30 /tmp/batata_test_cluster_node1.log 2>/dev/null || true
        echo "--- Node 2 log (last 30 lines) ---"
        tail -30 /tmp/batata_test_cluster_node2.log 2>/dev/null || true
        echo "--- Node 3 log (last 30 lines) ---"
        tail -30 /tmp/batata_test_cluster_node3.log 2>/dev/null || true
        stop_bg_server "$pid1"
        stop_bg_server "$pid2"
        stop_bg_server "$pid3"
        return 1
    fi

    # Initialize admin on node 1
    init_admin "http://127.0.0.1:8848"

    # Wait for Raft leader election and replication
    log_info "Waiting for Raft leader election (10s)..."
    sleep 10

    # Run cluster embedded tests
    log_info "Running cluster embedded E2E tests..."
    local test_rc=0
    bash "${SCRIPT_DIR}/test_cluster_embedded.sh" || test_rc=$?

    # Stop cluster
    stop_bg_server "$pid1"
    stop_bg_server "$pid2"
    stop_bg_server "$pid3"

    # Clean up data dirs
    rm -rf "${PROJECT_ROOT}/data/test-node"* "${PROJECT_ROOT}/logs/test-node"* 2>/dev/null || true

    # Restore cluster.conf
    if [ "$CLUSTER_CONF_BACKED_UP" = true ] && [ -f "${PROJECT_ROOT}/conf/cluster.conf.bak" ]; then
        mv "${PROJECT_ROOT}/conf/cluster.conf.bak" "${PROJECT_ROOT}/conf/cluster.conf"
        CLUSTER_CONF_BACKED_UP=false
    fi

    if [ $test_rc -ne 0 ]; then
        log_fail "Cluster embedded E2E tests failed (exit code: ${test_rc})"
        return 1
    fi

    log_pass "Cluster embedded E2E tests completed"
    return 0
}

# ==============================================================================
# Stage: Full — Standalone ExternalDb E2E
# ==============================================================================

run_standalone_externaldb() {
    log_section "Stage: Full — Standalone ExternalDb E2E (MySQL)"

    cd "$PROJECT_ROOT"

    # Start MySQL via docker-compose
    log_info "Starting MySQL test database..."
    docker-compose -f docker-compose.test.yml up -d mysql-test
    DOCKER_COMPOSE_STARTED=true

    # Wait for MySQL healthy
    if ! wait_for_docker_healthy "batata-mysql-test" 120; then
        log_fail "MySQL did not become healthy"
        return 1
    fi

    # Start server with MySQL
    local pid
    pid=$(start_bg_server "standalone_mysql" \
        --db-url "mysql://batata:batata@127.0.0.1:3307/batata_test")

    # Wait for server
    if ! wait_for_server "http://127.0.0.1:8081" 90; then
        log_fail "Standalone MySQL server failed to start"
        tail -50 /tmp/batata_test_standalone_mysql.log 2>/dev/null || true
        stop_bg_server "$pid"
        return 1
    fi

    # Initialize admin
    init_admin "http://127.0.0.1:8848"

    # Run tests
    log_info "Running standalone external DB E2E tests..."
    local test_rc=0
    bash "${SCRIPT_DIR}/test_standalone_externaldb.sh" || test_rc=$?

    # Stop server
    stop_bg_server "$pid"

    if [ $test_rc -ne 0 ]; then
        log_fail "Standalone ExternalDb E2E tests failed (exit code: ${test_rc})"
        return 1
    fi

    log_pass "Standalone ExternalDb E2E tests completed"
    return 0
}

# ==============================================================================
# Stage: Full — Cluster ExternalDb E2E
# ==============================================================================

run_cluster_externaldb() {
    log_section "Stage: Full — Cluster ExternalDb E2E (MySQL)"

    cd "$PROJECT_ROOT"

    # MySQL should already be running from standalone_externaldb stage
    # Ensure it's still healthy
    if ! docker inspect --format='{{.State.Health.Status}}' "batata-mysql-test" 2>/dev/null | grep -q "healthy"; then
        log_info "Restarting MySQL test database..."
        docker-compose -f docker-compose.test.yml up -d mysql-test
        DOCKER_COMPOSE_STARTED=true
        if ! wait_for_docker_healthy "batata-mysql-test" 120; then
            log_fail "MySQL did not become healthy"
            return 1
        fi
    fi

    # Write cluster.conf
    if [ -f "${PROJECT_ROOT}/conf/cluster.conf" ] && [ ! -f "${PROJECT_ROOT}/conf/cluster.conf.bak" ]; then
        cp "${PROJECT_ROOT}/conf/cluster.conf" "${PROJECT_ROOT}/conf/cluster.conf.bak"
        CLUSTER_CONF_BACKED_UP=true
    fi
    cat > "${PROJECT_ROOT}/conf/cluster.conf" <<'EOF'
127.0.0.1:8848
127.0.0.1:8858
127.0.0.1:8868
EOF

    local db_url="mysql://batata:batata@127.0.0.1:3307/batata_test"

    # Start 3 nodes
    local pid1 pid2 pid3

    pid1=$(start_bg_server "cluster_mysql_node1" \
        -m cluster --db-url "$db_url" \
        --batata.server.main.port=8848 --batata.console.port=8081 \
        --batata.logs.path=logs/test-node1)

    pid2=$(start_bg_server "cluster_mysql_node2" \
        -m cluster --db-url "$db_url" \
        --batata.server.main.port=8858 --batata.console.port=8082 \
        --batata.logs.path=logs/test-node2)

    pid3=$(start_bg_server "cluster_mysql_node3" \
        -m cluster --db-url "$db_url" \
        --batata.server.main.port=8868 --batata.console.port=8083 \
        --batata.logs.path=logs/test-node3)

    # Wait for all nodes
    local all_ready=true
    for port in 8081 8082 8083; do
        if ! wait_for_server "http://127.0.0.1:${port}" 120; then
            log_fail "Cluster MySQL node on console port ${port} failed to start"
            all_ready=false
        fi
    done

    if [ "$all_ready" = false ]; then
        stop_bg_server "$pid1"
        stop_bg_server "$pid2"
        stop_bg_server "$pid3"
        return 1
    fi

    # Initialize admin
    init_admin "http://127.0.0.1:8848"

    # Wait for cluster formation
    log_info "Waiting for cluster formation (5s)..."
    sleep 5

    # Run cluster externaldb tests
    log_info "Running cluster external DB E2E tests..."
    local test_rc=0
    bash "${SCRIPT_DIR}/test_cluster_externaldb.sh" || test_rc=$?

    # Stop cluster
    stop_bg_server "$pid1"
    stop_bg_server "$pid2"
    stop_bg_server "$pid3"

    # Clean up
    rm -rf "${PROJECT_ROOT}/logs/test-node"* 2>/dev/null || true

    # Restore cluster.conf
    if [ "$CLUSTER_CONF_BACKED_UP" = true ] && [ -f "${PROJECT_ROOT}/conf/cluster.conf.bak" ]; then
        mv "${PROJECT_ROOT}/conf/cluster.conf.bak" "${PROJECT_ROOT}/conf/cluster.conf"
        CLUSTER_CONF_BACKED_UP=false
    fi

    # Stop Docker databases
    log_info "Stopping test databases..."
    docker-compose -f docker-compose.test.yml down -v 2>/dev/null || true
    DOCKER_COMPOSE_STARTED=false

    if [ $test_rc -ne 0 ]; then
        log_fail "Cluster ExternalDb E2E tests failed (exit code: ${test_rc})"
        return 1
    fi

    log_pass "Cluster ExternalDb E2E tests completed"
    return 0
}

# ==============================================================================
# Stage: SDK (Nacos Java + Consul Go tests via Docker)
# ==============================================================================

run_sdk() {
    log_section "Stage: SDK (Nacos Java + Consul Go compatibility tests)"

    cd "$PROJECT_ROOT"

    # Check prerequisites
    require_cmd docker || return 1
    require_cmd docker-compose || return 1

    # Start full stack via sdk-tests/docker-compose.yml
    log_info "Starting SDK test stack (MySQL + Batata + test containers)..."
    cd "${PROJECT_ROOT}/sdk-tests"
    docker-compose up -d --build mysql batata
    SDK_DOCKER_STARTED=true
    cd "$PROJECT_ROOT"

    # Wait for batata service to be healthy
    if ! wait_for_docker_healthy "batata-test-server" 180; then
        log_fail "Batata server in SDK docker stack did not become healthy"
        cd "${PROJECT_ROOT}/sdk-tests"
        docker-compose logs batata 2>/dev/null | tail -50
        cd "$PROJECT_ROOT"
        return 1
    fi

    log_info "Batata server is healthy in Docker stack"

    # Initialize admin user inside the Docker stack
    local batata_url="http://127.0.0.1:8848"
    init_admin "$batata_url"

    # Run Nacos Java SDK tests
    log_info "Running Nacos Java SDK tests..."
    local nacos_rc=0
    cd "${PROJECT_ROOT}/sdk-tests"
    docker-compose run --rm nacos-tests || nacos_rc=$?
    cd "$PROJECT_ROOT"

    if [ $nacos_rc -eq 0 ]; then
        log_pass "Nacos Java SDK tests"
    else
        log_fail "Nacos Java SDK tests (exit code: ${nacos_rc})"
    fi

    # Run Consul Go SDK tests
    log_info "Running Consul Go SDK tests..."
    local consul_rc=0
    cd "${PROJECT_ROOT}/sdk-tests"
    docker-compose run --rm consul-tests || consul_rc=$?
    cd "$PROJECT_ROOT"

    if [ $consul_rc -eq 0 ]; then
        log_pass "Consul Go SDK tests"
    else
        log_fail "Consul Go SDK tests (exit code: ${consul_rc})"
    fi

    # Tear down SDK stack
    log_info "Stopping SDK test stack..."
    cd "${PROJECT_ROOT}/sdk-tests"
    docker-compose down -v 2>/dev/null || true
    cd "$PROJECT_ROOT"
    SDK_DOCKER_STARTED=false

    if [ $nacos_rc -ne 0 ] || [ $consul_rc -ne 0 ]; then
        return 1
    fi

    log_pass "All SDK compatibility tests completed"
    return 0
}

# ==============================================================================
# Main
# ==============================================================================

cd "$PROJECT_ROOT"

echo ""
echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║          Batata Comprehensive Test Suite                    ║${NC}"
echo -e "${CYAN}║                                                            ║${NC}"
echo -e "${CYAN}║  Level: $(printf '%-51s' "$LEVEL")  ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Determine which stages to run based on level
RUN_QUICK=true
RUN_STANDARD=false
RUN_FULL=false
RUN_SDK=false

case "$LEVEL" in
    quick)    ;;
    standard) RUN_STANDARD=true ;;
    full)     RUN_STANDARD=true; RUN_FULL=true ;;
    sdk)      RUN_STANDARD=true; RUN_FULL=true; RUN_SDK=true ;;
    *)
        echo "Unknown level: $LEVEL"
        echo "Valid levels: quick, standard, full, sdk"
        exit 1
        ;;
esac

# --- Quick ---
if [ "$RUN_QUICK" = true ]; then
    stage_rc=0
    run_quick || stage_rc=$?
    if [ $stage_rc -eq 0 ]; then
        record_stage "Quick (fmt + clippy + unit tests)" "PASS"
    else
        record_stage "Quick (fmt + clippy + unit tests)" "FAIL"
        OVERALL_EXIT_CODE=1
    fi
fi

# --- Standard ---
if [ "$RUN_STANDARD" = true ] && [ "$OVERALL_EXIT_CODE" -eq 0 ]; then
    stage_rc=0
    run_standard || stage_rc=$?
    if [ $stage_rc -eq 0 ]; then
        record_stage "Standard (standalone embedded E2E)" "PASS"
    else
        record_stage "Standard (standalone embedded E2E)" "FAIL"
        OVERALL_EXIT_CODE=1
    fi
fi

# --- Full: Cluster Embedded ---
if [ "$RUN_FULL" = true ] && [ "$OVERALL_EXIT_CODE" -eq 0 ]; then
    stage_rc=0
    run_cluster_embedded || stage_rc=$?
    if [ $stage_rc -eq 0 ]; then
        record_stage "Full — Cluster Embedded E2E" "PASS"
    else
        record_stage "Full — Cluster Embedded E2E" "FAIL"
        OVERALL_EXIT_CODE=1
    fi
fi

# --- Full: Standalone ExternalDb ---
if [ "$RUN_FULL" = true ] && [ "$OVERALL_EXIT_CODE" -eq 0 ]; then
    # Check Docker availability
    if command -v docker &>/dev/null && command -v docker-compose &>/dev/null; then
        stage_rc=0
        run_standalone_externaldb || stage_rc=$?
        if [ $stage_rc -eq 0 ]; then
            record_stage "Full — Standalone ExternalDb E2E" "PASS"
        else
            record_stage "Full — Standalone ExternalDb E2E" "FAIL"
            OVERALL_EXIT_CODE=1
        fi
    else
        record_stage "Full — Standalone ExternalDb E2E" "SKIP (Docker not available)"
        log_skip "Standalone ExternalDb E2E skipped — Docker not available"
    fi
fi

# --- Full: Cluster ExternalDb ---
if [ "$RUN_FULL" = true ] && [ "$OVERALL_EXIT_CODE" -eq 0 ]; then
    if command -v docker &>/dev/null && command -v docker-compose &>/dev/null; then
        stage_rc=0
        run_cluster_externaldb || stage_rc=$?
        if [ $stage_rc -eq 0 ]; then
            record_stage "Full — Cluster ExternalDb E2E" "PASS"
        else
            record_stage "Full — Cluster ExternalDb E2E" "FAIL"
            OVERALL_EXIT_CODE=1
        fi
    else
        record_stage "Full — Cluster ExternalDb E2E" "SKIP (Docker not available)"
        log_skip "Cluster ExternalDb E2E skipped — Docker not available"
    fi
fi

# --- SDK ---
if [ "$RUN_SDK" = true ] && [ "$OVERALL_EXIT_CODE" -eq 0 ]; then
    stage_rc=0
    run_sdk || stage_rc=$?
    if [ $stage_rc -eq 0 ]; then
        record_stage "SDK (Nacos Java + Consul Go)" "PASS"
    else
        record_stage "SDK (Nacos Java + Consul Go)" "FAIL"
        OVERALL_EXIT_CODE=1
    fi
fi

# ==============================================================================
# Final Summary
# ==============================================================================

echo ""
echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║              Test Suite Summary                             ║${NC}"
echo -e "${CYAN}╠══════════════════════════════════════════════════════════════╣${NC}"

for entry in "${STAGE_RESULTS[@]}"; do
    result="${entry%%:*}"
    name="${entry#*:}"
    case "$result" in
        PASS)
            echo -e "${CYAN}║${NC}  ${GREEN}PASS${NC}  ${name}"
            ;;
        FAIL)
            echo -e "${CYAN}║${NC}  ${RED}FAIL${NC}  ${name}"
            ;;
        SKIP*)
            echo -e "${CYAN}║${NC}  ${YELLOW}SKIP${NC}  ${name}"
            ;;
    esac
done

echo -e "${CYAN}╠══════════════════════════════════════════════════════════════╣${NC}"

if [ "$OVERALL_EXIT_CODE" -eq 0 ]; then
    echo -e "${CYAN}║${NC}  ${GREEN}ALL STAGES PASSED${NC}"
else
    echo -e "${CYAN}║${NC}  ${RED}SOME STAGES FAILED${NC}"
fi

echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

exit "$OVERALL_EXIT_CODE"
