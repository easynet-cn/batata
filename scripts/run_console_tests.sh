#!/usr/bin/env bash
# ==============================================================================
# Batata Console Datasource Test Runner
#
# Run all or specific console datasource mode tests.
#
# Usage:
#   ./scripts/run_console_tests.sh [mode]
#
# Modes:
#   standalone-embedded    Standalone + RocksDB (default ports 8848/8081)
#   standalone-externaldb  Standalone + MySQL/PostgreSQL (default ports 8848/8081)
#   cluster-externaldb     Cluster + MySQL/PostgreSQL (3-node: 8848-8868/8081-8083)
#   cluster-embedded       Cluster + Raft+RocksDB (3-node: 8848-8868/8081-8083)
#   all                    Run all modes sequentially
#
# Environment Variables:
#   MAIN_PORT              Main server port (standalone modes, default: 8848)
#   CONSOLE_PORT           Console server port (standalone modes, default: 8081)
#   NODE1_MAIN_PORT        Node 1 main port (cluster modes, default: 8848)
#   NODE1_CONSOLE_PORT     Node 1 console port (cluster modes, default: 8081)
#   NODE2_MAIN_PORT        Node 2 main port (cluster modes, default: 8858)
#   NODE2_CONSOLE_PORT     Node 2 console port (cluster modes, default: 8082)
#   NODE3_MAIN_PORT        Node 3 main port (cluster modes, default: 8868)
#   NODE3_CONSOLE_PORT     Node 3 console port (cluster modes, default: 8083)
#   USERNAME               Auth username (default: nacos)
#   PASSWORD               Auth password (default: nacos)
#
# Examples:
#   # Test standalone embedded (start server first with embedded mode)
#   cargo run -p batata-server -- -m standalone &
#   ./scripts/run_console_tests.sh standalone-embedded
#
#   # Test standalone with MySQL
#   cargo run -p batata-server -- -m standalone --db-url "mysql://user:pass@localhost/batata" &
#   ./scripts/run_console_tests.sh standalone-externaldb
#
#   # Test 3-node cluster with external DB
#   cargo run -p batata-server -- -m cluster --db-url "mysql://..." --main-port 8848 --console-port 8081 &
#   cargo run -p batata-server -- -m cluster --db-url "mysql://..." --main-port 8858 --console-port 8082 &
#   cargo run -p batata-server -- -m cluster --db-url "mysql://..." --main-port 8868 --console-port 8083 &
#   ./scripts/run_console_tests.sh cluster-externaldb
#
#   # Test with custom ports
#   MAIN_PORT=9848 CONSOLE_PORT=9081 ./scripts/run_console_tests.sh standalone-embedded
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

MODE="${1:-}"

usage() {
    echo "Usage: $0 <mode>"
    echo ""
    echo "Available modes:"
    echo "  standalone-embedded    Standalone + RocksDB"
    echo "  standalone-externaldb  Standalone + MySQL/PostgreSQL"
    echo "  cluster-externaldb     Cluster + MySQL/PostgreSQL (3-node)"
    echo "  cluster-embedded       Cluster + Raft+RocksDB (3-node)"
    echo "  all                    Run all modes"
    echo ""
    echo "Run '$0 <mode>' after starting the server in the corresponding mode."
    exit 1
}

check_dependency() {
    if ! command -v "$1" &> /dev/null; then
        echo -e "${RED}Error: '$1' is required but not installed.${NC}"
        exit 1
    fi
}

run_test() {
    local script="$1"
    local name="$2"

    echo ""
    echo -e "${CYAN}================================================================${NC}"
    echo -e "${CYAN}  Running: ${name}${NC}"
    echo -e "${CYAN}================================================================${NC}"

    if bash "$script"; then
        echo -e "${GREEN}>>> ${name}: PASSED${NC}"
        return 0
    else
        echo -e "${RED}>>> ${name}: FAILED${NC}"
        return 1
    fi
}

# Check dependencies
check_dependency curl
check_dependency jq

if [ -z "$MODE" ]; then
    usage
fi

OVERALL_RESULT=0

case "$MODE" in
    standalone-embedded)
        run_test "${SCRIPT_DIR}/test_standalone_embedded.sh" "Standalone + Embedded" || OVERALL_RESULT=1
        ;;
    standalone-externaldb)
        run_test "${SCRIPT_DIR}/test_standalone_externaldb.sh" "Standalone + ExternalDb" || OVERALL_RESULT=1
        ;;
    cluster-externaldb)
        run_test "${SCRIPT_DIR}/test_cluster_externaldb.sh" "Cluster + ExternalDb" || OVERALL_RESULT=1
        ;;
    cluster-embedded)
        run_test "${SCRIPT_DIR}/test_cluster_embedded.sh" "Cluster + Embedded" || OVERALL_RESULT=1
        ;;
    all)
        echo -e "${YELLOW}NOTE: 'all' mode requires servers running for each mode.${NC}"
        echo -e "${YELLOW}Usually you test one mode at a time.${NC}"
        echo ""

        # Test each mode
        run_test "${SCRIPT_DIR}/test_standalone_embedded.sh" "Standalone + Embedded" || OVERALL_RESULT=1
        run_test "${SCRIPT_DIR}/test_standalone_externaldb.sh" "Standalone + ExternalDb" || OVERALL_RESULT=1
        run_test "${SCRIPT_DIR}/test_cluster_externaldb.sh" "Cluster + ExternalDb" || OVERALL_RESULT=1
        run_test "${SCRIPT_DIR}/test_cluster_embedded.sh" "Cluster + Embedded" || OVERALL_RESULT=1
        ;;
    *)
        echo -e "${RED}Unknown mode: ${MODE}${NC}"
        usage
        ;;
esac

echo ""
if [ $OVERALL_RESULT -eq 0 ]; then
    echo -e "${GREEN}All test suites passed!${NC}"
else
    echo -e "${RED}Some test suites failed!${NC}"
fi

exit $OVERALL_RESULT
