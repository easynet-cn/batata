#!/usr/bin/env bash
# Port-ownership helpers for the SDK matrix.
#
# The SDK suites talk to batata over loopback. A foreign listener bound to
# 127.0.0.1:<port> is more specific than batata's *:<port> bind, so a stray
# server can answer every request while the harness still reports "node1
# ready" on 8848. A left-over smoke server on 127.0.0.1:18080 did exactly
# that: all 36 Apollo openapi cases failed with its 401 `{"code":40100}`
# response instead of exercising batata at all.
#
# `clear` owns the SDK ports before startup; `verify` proves the Apollo
# endpoint really is batata before any suite is allowed to run.

SDK_PORT_LIST=(8848 8858 8868 8081 8082 8083 8080 8500 8510 8520 \
               18080 18090 18100 9848 9858 9868 9849 9859 9869 7848 7858 7868)

sdk_ports_clear() {
    local port
    for port in "${SDK_PORT_LIST[@]}"; do
        lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null | xargs kill -9 2>/dev/null || true
    done
}

# `verify [port]`: batata's unauthenticated Apollo admin surface answers
# `GET /openapi/v1/apps` with 200 and a JSON array. An auth-gated or unrelated
# listener answers 401/404, and an absent server gives 000. Any of those is a
# fixture failure, not a product regression.
sdk_ports_verify() {
    local port="${1:-18080}" code=000
    for _ in $(seq 1 15); do
        code=$(curl -s --noproxy '*' --max-time 3 -o /dev/null -w '%{http_code}' \
            "http://127.0.0.1:${port}/openapi/v1/apps" 2>/dev/null) || code=000
        [ "$code" = "200" ] && return 0
        sleep 1
    done
    echo "Apollo port ${port} is not served by batata (GET /openapi/v1/apps -> HTTP ${code}); check for a foreign listener" >&2
    return 1
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    case "${1:-}" in
        clear) sdk_ports_clear ;;
        verify) shift; sdk_ports_verify "$@" ;;
        *) echo "usage: $0 clear|verify [port]" >&2; exit 2 ;;
    esac
fi
