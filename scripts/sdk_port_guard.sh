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
# endpoint really is batata before any suite is allowed to run; `self-test`
# proves no second, drifting port list has been re-introduced in test.sh.

SDK_PORT_LIST=(8848 8858 8868 8081 8082 8083 8080 8500 8502 8510 8520 \
               18080 18090 18100 9848 9858 9868 9849 9859 9869 7848 7858 7868)
# 8502 is the Consul Connect xDS gRPC listener (wired by the SDS/CA work).
# A stale xDS listener here makes a fresh batata bind fail with
# "Address already in use", so it must be cleared like the HTTP ports.

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

# Structural guard against a second, drifting port list.
#
# `sdk_ports_clear` is the only owner of SDK port cleanup. A literal
# `for port in 8848 ...` sweep inside test.sh silently goes stale whenever a
# new listener is added to SDK_PORT_LIST (8502 was exactly that case). The
# self-test fails if test.sh contains a direct port-kill sweep or a hardcoded
# numeric port loop, if it stops delegating to `sdk_ports_clear`, or if a
# historically-missing port disappears from the canonical list again.
# Run it in the static gate, like `raft_health_check.py --self-test`.
sdk_ports_guard_selftest() {
    local guard_dir test_sh failures=0 port p found calls=0
    guard_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    test_sh="${guard_dir}/test.sh"

    if [ ! -f "$test_sh" ]; then
        echo "FAIL: ${test_sh} not found" >&2
        return 1
    fi

    # Anchored to a real `source`/`.` command: a prose comment mentioning
    # "source of truth ... sdk_port_guard.sh" must not satisfy this check.
    if ! grep -Eq '^[[:space:]]*(source|\.)[[:space:]].*sdk_port_guard\.sh' "$test_sh"; then
        echo "FAIL: test.sh no longer sources sdk_port_guard.sh" >&2
        failures=$((failures + 1))
    fi

    # Any inline `lsof ... | xargs kill` in test.sh is a hand-rolled sweep
    # that bypasses SDK_PORT_LIST, regardless of how its ports are spelled.
    if grep -Eq 'lsof[[:space:]]+-ti.*\|[[:space:]]*xargs[[:space:]]+kill' "$test_sh"; then
        echo "FAIL: test.sh contains a direct port-kill sweep; use sdk_ports_clear" >&2
        failures=$((failures + 1))
    fi

    if grep -Eq '^[[:space:]]*for[[:space:]]+port[[:space:]]+in[[:space:]]+[0-9]' "$test_sh"; then
        echo "FAIL: test.sh contains a hardcoded numeric port loop" >&2
        failures=$((failures + 1))
    fi

    calls=$(grep -Ec '^[[:space:]]*sdk_ports_clear([[:space:]]|$)' "$test_sh" || true)
    if [ "${calls:-0}" -lt 1 ]; then
        echo "FAIL: test.sh never calls sdk_ports_clear" >&2
        failures=$((failures + 1))
    fi

    # 8502 (Consul Connect xDS), 8082 and 8083 were missing from the copied
    # quick-path list. Keep them anchored to the canonical list.
    for port in 8502 8082 8083; do
        found=0
        for p in "${SDK_PORT_LIST[@]}"; do
            if [ "$p" = "$port" ]; then found=1; break; fi
        done
        if [ "$found" -ne 1 ]; then
            echo "FAIL: SDK_PORT_LIST is missing ${port}" >&2
            failures=$((failures + 1))
        fi
    done

    if [ "$failures" -ne 0 ]; then
        echo "sdk_ports_guard_selftest: ${failures} failure(s)" >&2
        return 1
    fi
    echo "sdk_ports_guard_selftest: PASS (${#SDK_PORT_LIST[@]} ports, ${calls} delegation sites, no inline sweeps)"
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    case "${1:-}" in
        clear) sdk_ports_clear ;;
        verify) shift; sdk_ports_verify "$@" ;;
        self-test) sdk_ports_guard_selftest ;;
        *) echo "usage: $0 clear|verify [port]|self-test" >&2; exit 2 ;;
    esac
fi
