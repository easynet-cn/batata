#!/usr/bin/env bash
# ==============================================================================
# Shared SDK compatibility test runners (Consul Go + Nacos Java + Apollo Java).
#
# Source this file to get sdk_run_consul / sdk_run_nacos / sdk_run_apollo,
# used by both test.sh (sdk --quick manual entry and sdk --matrix full matrix).
# Centralizing the go/mvn invocations keeps the entry points in sync.
#
# Usage (in a sourcing script):
#   source "$(dirname "$0")/sdk_run_lib.sh"
#
#   sdk_run_consul [test_name_filter]   # optional -run filter for `go test`
#   sdk_run_nacos  [test_class_filter]  # optional -Dtest=Nacos<filter>* filter
#   sdk_run_apollo  [test_class_filter] # optional -Dtest=Apollo<filter>* filter
#
# Environment (all optional):
#   CONSUL_HTTP_TOKEN   forwarded to the Consul Go SDK (e.g. "root")
#   MVN_QUIET           set to 1 to pass -q to mvn
#   APOLLO_META         Apollo meta/config-service URL (default http://127.0.0.1:18080)
#   APOLLO_OPENAPI_URL  Apollo OpenAPI portal URL (default = APOLLO_META)
#   APOLLO_OPENAPI_TOKEN Apollo OpenAPI token (default "admin"; the plugin does
#                        not enforce it unless APOLLO_OPENAPI_AUTH_ENABLED is set)
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
        go test ./... -v -count=1 -timeout 600s $run_flag )
}

# Run the Nacos Java SDK test suite.
# Usage: sdk_run_nacos [class_filter]
sdk_run_nacos() {
    local filter="${1:-}"
    local test_flag=""
    [ -n "$filter" ] && test_flag="-Dtest=Nacos${filter}*"

    # Only advertise the cluster peers that are actually listening.
    #
    # NacosClusterTest skips itself unless -Dnacos.cluster.node2/node3 are set, so
    # these properties are what distinguish "cluster" from "standalone" for the
    # suite. Passing them unconditionally makes a single-node deployment *run* the
    # cross-node tests instead of skipping them, which shows up as 8 failures that
    # are really just "there is no node2/node3". Probing first keeps standalone
    # runs clean (skipped, not failed) without touching the cluster runs.
    local cluster_peers=""
    local peers_up=0
    local code
    for peer in 8858 8868; do
        code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 \
            "http://127.0.0.1:${peer}/nacos/v3/admin/core/state/liveness" 2>/dev/null || echo 000)
        [ "${code}" = "200" ] && peers_up=$((peers_up + 1))
    done
    if [ "${peers_up}" -eq 2 ]; then
        cluster_peers="-Dnacos.cluster.node2=127.0.0.1:8858 -Dnacos.cluster.node3=127.0.0.1:8868"
    fi

    ( cd "${SDK_PROJECT_ROOT}/sdk-tests/nacos-java-tests" && \
        mvn ${MVN_QUIET:+-q} test \
            -Dnacos.server=127.0.0.1:8848 \
            -Dnacos.username=nacos -Dnacos.password=nacos \
            -Dnacos.cluster.node1=127.0.0.1:8848 \
            ${cluster_peers} \
            $test_flag )
}

# Run the Apollo Java SDK test suite.
# Usage: sdk_run_apollo [class_filter]
#   sdk_run_apollo  [test_class_filter]  # optional -Dtest=Apollo<filter>*
#
# The Apollo compatibility plugin must be enabled on the Batata server
# (--batata.plugin.apollo.enabled=true --batata.plugin.apollo.port=<port>),
# and APOLLO_META must point at that port. By default the plugin does not
# enforce OpenAPI tokens, so APOLLO_OPENAPI_TOKEN is accepted as-is.
sdk_run_apollo() {
    local filter="${1:-}"
    local test_flag=""
    [ -n "$filter" ] && test_flag="-Dtest=Apollo${filter}*"
    local meta="${APOLLO_META:-http://127.0.0.1:18080}"
    local openapi_url="${APOLLO_OPENAPI_URL:-$meta}"
    local token="${APOLLO_OPENAPI_TOKEN:-admin}"
    ( cd "${SDK_PROJECT_ROOT}/sdk-tests/apollo-java-tests" && \
        mvn ${MVN_QUIET:+-q} test \
            -Dapollo.meta="$meta" \
            -Dapollo.openapi.url="$openapi_url" \
            -Dapollo.openapi.token="$token" \
            $test_flag )
}
