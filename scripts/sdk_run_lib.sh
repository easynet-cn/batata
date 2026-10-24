#!/usr/bin/env bash
# ==============================================================================
# Shared SDK compatibility test runners (Consul Go + Nacos Java + Apollo Java
# + the three batata Rust suites).
#
# Source this file to get sdk_run_consul / sdk_run_nacos / sdk_run_apollo /
# sdk_run_batata_client / sdk_run_batata_consul_client /
# sdk_run_batata_maintainer, used by both test.sh (sdk --quick manual entry
# and sdk --matrix full matrix). Centralizing the go/mvn/cargo invocations
# keeps the entry points in sync.
#
# Usage (in a sourcing script):
#   source "$(dirname "$0")/sdk_run_lib.sh"
#
#   sdk_run_consul  [test_name_filter]  # optional -run filter for `go test`
#   sdk_run_nacos   [class_filter]      # optional -Dtest=Nacos<filter>* filter
#   sdk_run_apollo  [class_filter]      # optional -Dtest=Apollo<filter>* filter
#   sdk_run_batata_client        [filter]  # optional cargo test-name filter
#   sdk_run_batata_consul_client [filter]  # optional cargo test-name filter
#   sdk_run_batata_maintainer    [filter]  # optional cargo test-name filter
#
# Environment (all optional):
#   CONSUL_HTTP_TOKEN   forwarded to the Consul Go SDK (e.g. "root")
#   MVN_QUIET           set to "1" to pass -q to mvn. Default off: -q hides
#                       Surefire's "Tests run:" summary, the only source the
#                       report parser has for a suite's case count.
#   CARGO_QUIET         set to 1 to pass --quiet to cargo
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
    # Only advertise the cluster peers that are actually listening.
    #
    # `cluster_test.go` skips itself unless CONSUL_CLUSTER_NODE1/2/3 are all
    # set, so passing them unconditionally makes a single-node deployment
    # *run* the cross-node tests instead of skipping them, which shows up as 6
    # failures that are really just "there is no node2/node3". Probing first
    # keeps standalone runs clean (skipped, not failed) without touching the
    # cluster runs. Same probe `sdk_run_nacos` already does for 8858/8868.
    local peers_up=0
    local code
    for peer in 8510 8520; do
        code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 \
            "http://127.0.0.1:${peer}/v1/status/leader" 2>/dev/null || echo 000)
        [ "${code}" = "200" ] && peers_up=$((peers_up + 1))
    done

    # `export` (not a bare `VAR=...` prefix): `${VAR:+VAR="$VAR"}` expands with
    # the quotes as literal characters, so bash treated it as a command name
    # and every run exited 127 without ever invoking `go test`.
    ( cd "${SDK_PROJECT_ROOT}/sdk-tests/consul-go-tests" && \
        export CONSUL_HTTP_ADDR=127.0.0.1:8500 && \
        if [ "${peers_up}" -eq 2 ]; then
            export CONSUL_CLUSTER_NODE1=127.0.0.1:8500 \
                   CONSUL_CLUSTER_NODE2=127.0.0.1:8510 \
                   CONSUL_CLUSTER_NODE3=127.0.0.1:8520
        else
            unset CONSUL_CLUSTER_NODE1 CONSUL_CLUSTER_NODE2 CONSUL_CLUSTER_NODE3
        fi && \
        { [ -z "${CONSUL_HTTP_TOKEN:-}" ] || export CONSUL_HTTP_TOKEN; } && \
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

    # `-q` hides Surefire's "Tests run:" summary, which is the only source
    # gen_sdk_report.sh has for the case count — with it on, every Maven suite
    # reports 0/0. So quiet has to be opt-in per value, not per non-emptiness.
    local mvn_q=""
    [ "${MVN_QUIET:-0}" = "1" ] && mvn_q="-q"
    ( cd "${SDK_PROJECT_ROOT}/sdk-tests/nacos-java-tests" && \
        mvn ${mvn_q} test \
            -Dnacos.server=127.0.0.1:8848 \
            -Dnacos.username=nacos -Dnacos.password=nacos \
            -Dnacos.cluster.node1=127.0.0.1:8848 \
            ${cluster_peers} \
            $test_flag )
}

# Run one of the Rust SDK suites under sdk-tests/.
#
# Each of these is its own Cargo workspace (it depends on batata-client by
# path), so they must be invoked from their own directory rather than from the
# repository root.
#
# Usage: sdk_run_rust_suite <dir> [test_name_filter]
sdk_run_rust_suite() {
    local dir="$1" filter="${2:-}"
    # --no-fail-fast: each suite is a dozen *test binaries*, and cargo stops at
    # the first failing one by default — one broken binary silently hid every
    # suite behind it (core_extended_test went unreported for a whole release
    # because config_extended_test failed first).
    local -a args=(test --no-fail-fast)
    [ -n "${CARGO_QUIET:-}" ] && args+=(--quiet)
    # `cargo test <filter>` filters by test *name*; passing an empty string
    # selects nothing, so only append the separator when a filter was given.
    [ -n "$filter" ] && args+=(-- "$filter")

    # A hung suite must not stall the matrix. Cargo buffers a test binary's
    # output until it exits, so a hang looks exactly like a slow run and would
    # otherwise block every remaining combination. `timeout` (GNU coreutils)
    # is not guaranteed to exist, so fall back to an unbounded run rather
    # than failing the whole matrix on a missing utility.
    local secs="${SDK_RUST_TIMEOUT:-900}"
    if command -v timeout >/dev/null 2>&1; then
        ( cd "${SDK_PROJECT_ROOT}/sdk-tests/${dir}" && timeout "$secs" cargo "${args[@]}" )
    else
        ( cd "${SDK_PROJECT_ROOT}/sdk-tests/${dir}" && cargo "${args[@]}" )
    fi
}

# Run the batata-client SDK suite (naming + config over gRPC and HTTP).
# Requires a server on 8848 and an initialized nacos/nacos user.
# Usage: sdk_run_batata_client [test_name_filter]
sdk_run_batata_client() {
    sdk_run_rust_suite batata-client-tests "${1:-}"
}

# Run the batata Consul-client SDK suite (Consul HTTP API on 8500).
# Requires the Consul plugin to be enabled.
# Usage: sdk_run_batata_consul_client [test_name_filter]
sdk_run_batata_consul_client() {
    sdk_run_rust_suite batata-consul-client-tests "${1:-}"
}

# Run the batata maintainer/admin SDK suite (admin + extended APIs on 8848).
# Requires a server on 8848 and an initialized nacos/nacos user.
# Usage: sdk_run_batata_maintainer [test_name_filter]
sdk_run_batata_maintainer() {
    sdk_run_rust_suite batata-maintainer-tests "${1:-}"
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
    local mvn_q=""
    [ "${MVN_QUIET:-0}" = "1" ] && mvn_q="-q"
    ( cd "${SDK_PROJECT_ROOT}/sdk-tests/apollo-java-tests" && \
        mvn ${mvn_q} test \
            -Dapollo.meta="$meta" \
            -Dapollo.openapi.url="$openapi_url" \
            -Dapollo.openapi.token="$token" \
            $test_flag )
}
