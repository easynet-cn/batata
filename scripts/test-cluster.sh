#!/usr/bin/env bash
# ==============================================================================
# Quick cluster test script (uses debug build by default for fast iteration)
#
# Usage:
#   ./scripts/test-cluster.sh                      # All tests
#   ./scripts/test-cluster.sh consul               # Consul Go tests only
#   ./scripts/test-cluster.sh nacos                # Nacos Java tests only
#   ./scripts/test-cluster.sh consul "TestKV"      # Filter by test name
#   ./scripts/test-cluster.sh nacos "Cluster"      # Filter by test class
#
# Options:
#   RELEASE=1    Use release build
#   NO_RESTART=1 Skip cluster restart (reuse running cluster)
#   BUILD=1      Rebuild before testing
# ==============================================================================

set -euo pipefail
cd "$(dirname "$0")/.."

# Configuration
if [ "${RELEASE:-}" = "1" ]; then
    BINARY="./target/release/batata-server"
    [ "${BUILD:-}" = "1" ] && cargo build --release -p batata-server
else
    BINARY="./target/debug/batata-server"
    [ "${BUILD:-}" = "1" ] && cargo build -p batata-server
fi

RED='\033[0;31m'
GREEN='\033[0;32m'
BOLD='\033[1m'
NC='\033[0m'

SUITE="${1:-all}"
FILTER="${2:-}"

# Start cluster unless NO_RESTART
if [ "${NO_RESTART:-}" != "1" ]; then
    echo -e "${BOLD}Starting cluster ($(basename $BINARY))...${NC}"
    BINARY="$BINARY" CONSUL_ENABLED=true ./scripts/start-cluster.sh 2>&1 | grep "ready\|ERROR"
fi

run_consul() {
    echo -e "\n${BOLD}=== Consul Go Tests ===${NC}"
    local run_flag=""
    [ -n "$FILTER" ] && run_flag="-run $FILTER"

    cd sdk-tests/consul-go-tests
    CONSUL_HTTP_ADDR=127.0.0.1:8500 \
    CONSUL_CLUSTER_NODE1=127.0.0.1:8500 \
    CONSUL_CLUSTER_NODE2=127.0.0.1:8510 \
    CONSUL_CLUSTER_NODE3=127.0.0.1:8520 \
    go test ./... $run_flag -v -count=1 -timeout 120s 2>&1 | grep -v "^{" | tee /tmp/test-cluster-consul.txt | grep -E "^---|PASS|FAIL"
    cd ../..

    local pass=$(grep -c '^--- PASS' /tmp/test-cluster-consul.txt 2>/dev/null || echo 0)
    local fail=$(grep -c '^--- FAIL' /tmp/test-cluster-consul.txt 2>/dev/null || echo 0)
    echo -e "\n${BOLD}Consul: ${GREEN}${pass} PASS${NC} ${RED}${fail} FAIL${NC}"
}

run_nacos() {
    echo -e "\n${BOLD}=== Nacos Java Tests ===${NC}"
    local test_flag=""
    [ -n "$FILTER" ] && test_flag="-Dtest=Nacos${FILTER}*"

    cd sdk-tests/nacos-java-tests
    mvn test \
        -Dnacos.server=127.0.0.1:8848 \
        -Dnacos.username=nacos -Dnacos.password=nacos \
        -Dnacos.cluster.node1=127.0.0.1:8848 \
        -Dnacos.cluster.node2=127.0.0.1:8858 \
        -Dnacos.cluster.node3=127.0.0.1:8868 \
        $test_flag 2>&1 | tee /tmp/test-cluster-nacos.txt | grep "Tests run:" | grep -v " -- in " | tail -1
    cd ../..
}

case "$SUITE" in
    consul)  run_consul ;;
    nacos)   run_nacos ;;
    all)     run_consul; run_nacos ;;
    *)       echo "Usage: $0 {all|consul|nacos} [filter]"; exit 1 ;;
esac

echo -e "\n${BOLD}Done.${NC}"
