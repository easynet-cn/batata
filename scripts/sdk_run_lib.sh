#!/usr/bin/env bash
# ==============================================================================
# Shared SDK compatibility test runners (Consul Go + Nacos Java).
#
# Source this file to get sdk_run_consul / sdk_run_nacos, used by both
# test.sh (sdk --quick manual entry and sdk --matrix full matrix).
# Centralizing the go/mvn invocations keeps the two entry points in sync.
#
# Usage (in a sourcing script):
#   source "$(dirname "$0")/sdk_run_lib.sh"
#
#   sdk_run_consul [test_name_filter]   # optional -run filter for `go test`
#   sdk_run_nacos  [test_class_filter]  # optional -Dtest=Nacos<filter>* filter
#
# Environment (all optional):
#   CONSUL_HTTP_TOKEN   forwarded to the Consul Go SDK (e.g. "root")
#   MVN_QUIET           set to 1 to pass -q to mvn
# ==============================================================================

SDK_PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Run the Consul Go SDK test suite.
# Usage: sdk_run_consul [run_filter]
sdk_run_consul() {
    local filter="${1:-}"
    local run_flag=""
    [ -n "$filter" ] && run_flag="-run $filter"
    ( cd "${SDK_PROJECT_ROOT}/sdk-tests/consul-go-tests" && \
        CONSUL_HTTP_ADDR=127.0.0.1:8500 \
        CONSUL_CLUSTER_NODE1=127.0.0.1:8500 \
        CONSUL_CLUSTER_NODE2=127.0.0.1:8510 \
        CONSUL_CLUSTER_NODE3=127.0.0.1:8520 \
        ${CONSUL_HTTP_TOKEN:+CONSUL_HTTP_TOKEN="$CONSUL_HTTP_TOKEN"} \
        go test ./... -v -count=1 -timeout 120s $run_flag )
}

# Run the Nacos Java SDK test suite.
# Usage: sdk_run_nacos [class_filter]
sdk_run_nacos() {
    local filter="${1:-}"
    local test_flag=""
    [ -n "$filter" ] && test_flag="-Dtest=Nacos${filter}*"
    ( cd "${SDK_PROJECT_ROOT}/sdk-tests/nacos-java-tests" && \
        mvn ${MVN_QUIET:+-q} test \
            -Dnacos.server=127.0.0.1:8848 \
            -Dnacos.username=nacos -Dnacos.password=nacos \
            -Dnacos.cluster.node1=127.0.0.1:8848 \
            -Dnacos.cluster.node2=127.0.0.1:8858 \
            -Dnacos.cluster.node3=127.0.0.1:8868 \
            $test_flag )
}
