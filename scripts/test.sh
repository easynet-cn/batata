#!/usr/bin/env bash
# ==============================================================================
# Unified test entry point for Batata.
#
# Consolidates the previous standalone test launchers (test-cluster.sh,
# test-separation.sh, run_console_tests.sh, run_sdk_matrix.sh,
# verify_apollo_live.sh, run_all_tests.sh) and the Apollo embedded E2E
# (test_apollo_embedded.sh, now inline in `apollo`) behind a
# single Nacos-style subcommand interface.
#
# Subcommands:
#   separation [main_url] [console_url]   Console/Server route separation test
#   apollo [--live]                       Apollo plugin live verification
#                                          (--live: against container :8080;
#                                           default: embedded plugin :18080)
#   sdk [--quick|--matrix]                SDK (Consul Go + Nacos Java + Apollo Java) tests
#                                          default --quick
#   console [mode]                        Console datasource E2E (delegates to
#                                          test_standalone_embedded.sh etc.)
#                                          mode: standalone-embedded |
#                                                standalone-externaldb |
#                                                cluster-externaldb |
#                                                cluster-embedded | all
#   all [level] [--skip-build]            Full suite (level: quick|standard|
#                                          full|sdk, default quick)
#
# Environment variables consumed by the underlying scripts are unchanged.
# ==============================================================================

set -euo pipefail
cd "$(dirname "$0")/.."

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PROJECT_ROOT="$ROOT"

source "${SCRIPT_DIR}/test_utils.sh"
source "${SCRIPT_DIR}/cluster_lib.sh"
source "${SCRIPT_DIR}/sdk_run_lib.sh"

# ---------------------------------------------------------------------------
# separation: Console/Server route separation test
# ---------------------------------------------------------------------------
cmd_separation() {
    MAIN_URL="${1:-http://127.0.0.1:8848}"
    CONSOLE_URL="${2:-http://127.0.0.1:8081}"

    PASS=0
    FAIL=0

    check() {
        local label="$1"
        local expected="$2"
        local actual="$3"

        if [ "$actual" = "$expected" ]; then
            echo "  PASS: ${label} (HTTP ${actual})"
            PASS=$((PASS + 1))
        else
            echo "  FAIL: ${label} (expected HTTP ${expected}, got HTTP ${actual})"
            FAIL=$((FAIL + 1))
        fi
    }

    echo "============================================="
    echo "Console/Server Route Separation Test"
    echo "Main: ${MAIN_URL}  Console: ${CONSOLE_URL}"
    echo "============================================="

    echo ""
    echo "[Console Server - V2 console routes (should be available)]"
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "${CONSOLE_URL}/v2/console/health/liveness")
    check "V2 console health/liveness" "200" "$CODE"
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "${CONSOLE_URL}/v2/console/health/readiness")
    check "V2 console health/readiness" "200" "$CODE"
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "${CONSOLE_URL}/v2/console/namespace/list")
    if [ "$CODE" = "200" ] || [ "$CODE" = "403" ]; then
        echo "  PASS: V2 console namespace/list (HTTP ${CODE}, route exists)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: V2 console namespace/list (expected 200 or 403, got HTTP ${CODE})"
        FAIL=$((FAIL + 1))
    fi

    echo ""
    echo "[Console Server - V3 console routes (should be available)]"
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "${CONSOLE_URL}/v3/console/health/liveness")
    check "V3 console health/liveness" "200" "$CODE"

    echo ""
    echo "[Console Server - V2 Open API routes (should NOT be available)]"
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "${CONSOLE_URL}/v2/cs/config")
    check "V2 cs/config on Console" "404" "$CODE"
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "${CONSOLE_URL}/v2/ns/instance/list")
    check "V2 ns/instance/list on Console" "404" "$CODE"

    echo ""
    echo "[Console Server - V3 Admin routes (should NOT be available)]"
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "${CONSOLE_URL}/v3/admin/core/cluster/node/self")
    check "V3 admin cluster on Console" "404" "$CODE"

    echo ""
    echo "[Main Server - V2 console routes (should NOT be available)]"
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "${MAIN_URL}/nacos/v2/console/namespace/list")
    check "V2 console namespace/list on Main" "404" "$CODE"
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "${MAIN_URL}/nacos/v2/console/health/liveness")
    check "V2 console health/liveness on Main" "404" "$CODE"
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "${MAIN_URL}/nacos/v2/console/health/readiness")
    check "V2 console health/readiness on Main" "404" "$CODE"

    echo ""
    echo "[Main Server - V2 Open API routes (should be available)]"
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "${MAIN_URL}/nacos/v2/cs/config")
    if [ "$CODE" = "400" ] || [ "$CODE" = "403" ]; then
        echo "  PASS: V2 cs/config on Main (HTTP ${CODE}, route exists)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: V2 cs/config on Main (expected 400 or 403, got HTTP ${CODE})"
        FAIL=$((FAIL + 1))
    fi
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "${MAIN_URL}/nacos/v2/core/cluster/node/self")
    if [ "$CODE" = "200" ] || [ "$CODE" = "403" ]; then
        echo "  PASS: V2 core/cluster on Main (HTTP ${CODE}, route exists)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: V2 core/cluster on Main (expected 200 or 403, got HTTP ${CODE})"
        FAIL=$((FAIL + 1))
    fi

    echo ""
    echo "[Auth endpoints (should be on BOTH servers)]"
    CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST "${MAIN_URL}/nacos/v3/auth/user/login" -d "username=x&password=x")
    if [ "$CODE" != "404" ]; then
        echo "  PASS: Auth login on Main (HTTP ${CODE}, route exists)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: Auth login on Main (got 404, route missing)"
        FAIL=$((FAIL + 1))
    fi
    CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST "${CONSOLE_URL}/v3/auth/user/login" -d "username=x&password=x")
    if [ "$CODE" != "404" ]; then
        echo "  PASS: Auth login on Console (HTTP ${CODE}, route exists)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: Auth login on Console (got 404, route missing)"
        FAIL=$((FAIL + 1))
    fi

    echo ""
    echo "============================================="
    echo "Results: ${PASS} passed, ${FAIL} failed"
    echo "============================================="
    [ "$FAIL" -gt 0 ] && return 1 || return 0
}

# ---------------------------------------------------------------------------
# apollo: Apollo plugin live verification (parameterized live/embedded)
# ---------------------------------------------------------------------------
cmd_apollo() {
    local LIVE=0
    for a in "$@"; do
        case "$a" in
            --live) LIVE=1 ;;
        esac
    done

    local BASE DEFAULT_SKIP
    if [ "$LIVE" = 1 ]; then
        BASE="${APOLLO_BASE:-http://127.0.0.1:8080}"
        DEFAULT_SKIP=0
    else
        BASE="${APOLLO_BASE:-http://127.0.0.1:${APOLLO_PORT:-18080}}"
        DEFAULT_SKIP=1
    fi
    SKIP_RESTART="${SKIP_RESTART:-$DEFAULT_SKIP}"

    PASS=0; FAIL=0
    ok()   { PASS=$((PASS+1)); echo "  ✓ $1"; }
    bad()  { FAIL=$((FAIL+1)); echo "  ✗ $1"; }
    check(){ if [ "$1" = "$2" ]; then ok "$3"; else bad "$3 (expected=$1 got=$2)"; fi }

    APP="live-app-$(date +%s)"
    NS="live-ns"

    echo "==> setup: app / namespace / item / release"
    curl -s -o /dev/null -X POST "$BASE/openapi/v1/apps" -H 'Content-Type: application/json' \
        -d "{\"app\":{\"appId\":\"$APP\",\"name\":\"Live-$APP\",\"orgId\":\"TEST_ORG\",\"orgName\":\"Test Org\",\"ownerName\":\"t\",\"ownerEmail\":\"t@t\",\"dataChangeCreatedBy\":\"t\"},\"admins\":[\"t\"],\"assignAppRoleToSelf\":true}"

    CODE=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/apps/$APP/appnamespaces" -H 'Content-Type: application/json' \
        -d "{\"name\":\"$NS\",\"appId\":\"$APP\",\"format\":\"properties\",\"isPublic\":false,\"comment\":\"live\",\"dataChangeCreatedBy\":\"t\"}")
    check 200 "$CODE" "create namespace"

    CODE=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/apps/$APP/clusters/default/namespaces/$NS/items" -H 'Content-Type: application/json' \
        -d "{\"key\":\"k1\",\"value\":\"v1\",\"comment\":\"c\",\"dataChangeCreatedBy\":\"t\"}")
    check 200 "$CODE" "create item"

    CODE=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/apps/$APP/clusters/default/namespaces/$NS/releases?operator=t" -H 'Content-Type: application/json' \
        -d "{\"releaseTitle\":\"r1\",\"releaseComment\":\"\",\"releasedBy\":\"t\",\"emergencyPublish\":false}")
    check 200 "$CODE" "publish r1"

    echo "==> B1 metaservice (database discovery + self fallback)"
    BODY=$(curl -s "$BASE/services/config")
    echo "$BODY" | grep -q "apollo-configservice" && ok "services/config lists configservice" || bad "services/config: $BODY"

    echo "==> CFGSVC-001 fetch"
    V=$(curl -s "$BASE/configs/$APP/default/$NS" | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d["configurations"]["k1"])')
    check v1 "$V" "config fetch returns k1=v1"
    R value=$(curl -s "$BASE/configs/$APP/default/$NS" | python3 -c 'import sys,json;print(json.load(sys.stdin)["releaseKey"])')

    echo "==> LONGPOL-001: hold then wake on publish"
    ( sleep 1; curl -s -o /dev/null -X POST "$BASE/apps/$APP/clusters/default/namespaces/$NS/items" -H 'Content-Type: application/json' \
        -d "{\"key\":\"k1\",\"value\":\"v2\",\"dataChangeCreatedBy\":\"t\",\"dataChangeLastModifiedBy\":\"t\"}";
      curl -s -o /dev/null -X POST "$BASE/apps/$APP/clusters/default/namespaces/$NS/releases?operator=t" -H 'Content-Type: application/json' \
        -d "{\"releaseTitle\":\"r2\",\"releasedBy\":\"t\",\"emergencyPublish\":false}" ) &
    START=$(date +%s)
    NOTIF=$(curl -s --max-time 70 "$BASE/notifications/v2?appId=$APP&cluster=default&notifications=%5B%7B%22namespaceName%22%3A%22$NS%22%2C%22notificationId%22%3A0%7D%5D")
    ELAPSED=$(( $(date +%s) - START ))
    wait %2 2>/dev/null
    echo "$NOTIF" | grep -q "$NS" && ok "long-poll woken on publish (${ELAPSED}s)" || bad "no notification: $NOTIF"

    echo "==> restart-recovery: id must keep growing across restart"
    ID_BEFORE=$(echo "$NOTIF" | python3 -c 'import sys,json;print(json.load(sys.stdin)[0]["notificationId"])' 2>/dev/null || echo 0)
    if [ "${SKIP_RESTART:-0}" = "1" ]; then
        echo "  (skip: SKIP_RESTART=1)"
    else
        podman restart batata >/dev/null 2>&1 || true
        for i in $(seq 1 30); do curl -sf -o /dev/null "$BASE/services/config" && break; sleep 2; done
    fi
    curl -s -o /dev/null -X POST "$BASE/apps/$APP/clusters/default/namespaces/$NS/items" -H 'Content-Type: application/json' \
        -d "{\"key\":\"k1\",\"value\":\"v3\",\"dataChangeCreatedBy\":\"t\",\"dataChangeLastModifiedBy\":\"t\"}"
    curl -s -o /dev/null -X POST "$BASE/apps/$APP/clusters/default/namespaces/$NS/releases?operator=t" -H 'Content-Type: application/json' \
        -d "{\"releaseTitle\":\"r3\",\"releasedBy\":\"t\",\"emergencyPublish\":false}"
    sleep 2
    ID_AFTER=$(curl -s --max-time 10 "$BASE/notifications/v2?appId=$APP&cluster=default&notifications=%5B%7B%22namespaceName%22%3A%22$NS%22%2C%22notificationId%22%3A$ID_BEFORE%7D%5D" \
        | python3 -c 'import sys,json;print(json.load(sys.stdin)[0]["notificationId"])' 2>/dev/null || echo 0)
    [ "$ID_AFTER" -gt "$ID_BEFORE" ] && ok "notificationId monotonic across restart ($ID_BEFORE → $ID_AFTER)" || bad "id not monotonic: before=$ID_BEFORE after=$ID_AFTER"

    echo "==> CFGSVC-007 AccessKey enforcement"
    SECRET=$(curl -s -X POST "$BASE/openapi/v1/apps/$APP/accesskeys" -H 'Content-Type: application/json' -d '{"createdBy":"t"}' \
        | python3 -c 'import sys,json;print(json.load(sys.stdin)["secret"])')
    TS=$(($(date +%s)*1000))
    PATHQ="/configs/$APP/default/$NS"
    SIG=$(python3 -c "
import hmac,hashlib,base64,sys
secret=sys.argv[1]; ts=sys.argv[2]; p=sys.argv[3]
mac=hmac.new(secret.encode(), f'{ts}\n{p}'.encode(), hashlib.sha1).digest()
print(base64.b64encode(mac).decode())" "$SECRET" "$TS" "$PATHQ")

    CODE=$(curl -s -o /dev/null -w '%{http_code}' "$BASE$PATHQ")
    check 401 "$CODE" "unsigned request REJECTED once key enabled"
    CODE=$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Apollo $APP:$SIG" -H "Timestamp: $TS" "$BASE$PATHQ")
    check 200 "$CODE" "signed request accepted"

    echo ""
    echo "==> RESULT: PASS=$PASS FAIL=$FAIL"
    [ "$FAIL" = "0" ] && return 0 || return 1
}

# ---------------------------------------------------------------------------
# sdk --quick: quick SDK test against a locally started cluster
# ---------------------------------------------------------------------------
cmd_sdk_quick() {
    # Configuration
    if [ "${RELEASE:-}" = "1" ]; then
        BINARY="./target/release/batata-server"
        [ "${BUILD:-}" = "1" ] && cargo build --release -p batata-server
    else
        BINARY="./target/debug/batata-server"
        [ "${BUILD:-}" = "1" ] && cargo build -p batata-server
    fi

    RED='\033[0;31m'; GREEN='\033[0;32m'; BOLD='\033[1m'; NC='\033[0m'

    SUITE="${1:-all}"
    FILTER="${2:-}"

    # --- Targeted tracing -------------------------------------------------
    # init_logging() uses EnvFilter::try_from_default_env(), so RUST_LOG is a
    # ready-made per-module switch. For consul (and all) we bump the consul
    # plugin + healthcheck reactor to debug, so the check registration ->
    # reactor execution -> query path is fully visible without an OTEL backend.
    if [ "${SUITE}" = "consul" ] || [ "${SUITE}" = "all" ]; then
        export RUST_LOG="${RUST_LOG:-info},batata_plugin_consul=debug,batata_naming::healthcheck=debug,batata_naming=debug"
    fi

    # --- Start backend ----------------------------------------------------
    # SDK_MODE=standalone -> single node (fast verify); default cluster (full
    # suite, incl. peering/raft cases). Both enable consul + apollo.
    MODE="${SDK_MODE:-cluster}"
    if [ "${NO_RESTART:-}" != "1" ]; then
        # Kill any leftover batata-server so we don't collide on shared ports
        # (e.g. a previous cluster run still holding 8848/8081/8500).
        pkill -f batata-server 2>/dev/null || true
        sleep 2
        for port in 8848 8858 8868 8081 8500 8510 8520 8080 18080 18090 18100 \
                    9848 9858 9868 9849 9859 9869 7848 7858 7868; do
            lsof -ti:"$port" 2>/dev/null | xargs kill -9 2>/dev/null || true
        done
        sleep 1
        if [ "${MODE}" = "standalone" ]; then
            echo -e "${BOLD}Starting standalone node ($(basename $BINARY)) with consul+apollo...${NC}"
            # startup.sh's standalone branch does NOT read CONSUL_ENABLED/
            # APOLLO_ENABLED; rely on BATATA_PLUGIN_*_ENABLED env overrides.
            BATATA_PLUGIN_CONSUL_ENABLED=true BATATA_PLUGIN_APOLLO_ENABLED=true \
                BINARY="$BINARY" "${SCRIPT_DIR}/startup.sh" -m standalone 2>&1 | grep "ready\|ERROR" || true
            LOG_SOURCES=("${ROOT}/logs/startup.log")
        else
            echo -e "${BOLD}Starting cluster ($(basename $BINARY))...${NC}"
            BINARY="$BINARY" CONSUL_ENABLED=true APOLLO_ENABLED=true "${SCRIPT_DIR}/startup.sh" cluster 2>&1 | grep "ready\|ERROR" || true
            LOG_SOURCES=("${ROOT}/logs/node1" "${ROOT}/logs/node2" "${ROOT}/logs/node3")
        fi
    fi

    run_consul() {
        echo -e "\n${BOLD}=== Consul Go Tests ===${NC}"
        sdk_run_consul "$FILTER" 2>&1 | grep -v "^{" | tee /tmp/test-${MODE}-consul.txt | grep -E "^---|PASS|FAIL"
        local pass=$(grep -c '^--- PASS' /tmp/test-${MODE}-consul.txt 2>/dev/null || echo 0)
        local fail=$(grep -c '^--- FAIL' /tmp/test-${MODE}-consul.txt 2>/dev/null || echo 0)
        echo -e "\n${BOLD}Consul: ${GREEN}${pass} PASS${NC} ${RED}${fail} FAIL${NC}"
    }

    run_batata() {
        echo -e "\n${BOLD}=== Nacos Java SDK Tests ===${NC}"
        sdk_run_nacos "$FILTER" 2>&1 | tee /tmp/test-${MODE}-nacos.txt | grep "Tests run:" | grep -v " -- in " | tail -1
    }

    run_apollo() {
        echo -e "\n${BOLD}=== Apollo Java SDK Tests ===${NC}"
        APOLLO_META="http://127.0.0.1:18080" sdk_run_apollo "$FILTER" 2>&1 | tee /tmp/test-${MODE}-apollo.txt | grep "Tests run:" | grep -v " -- in " | tail -1
    }

    # Run the suite without errexit: a test failure must not abort before we
    # archive the server logs (and the standalone/cluster start above already
    # backgrounded the server regardless).
    local suite_rc=0
    set +e
    case "$SUITE" in
        consul)  run_consul ;;
        nacos)   run_batata ;;
        apollo)  run_apollo ;;
        all)     run_consul; run_batata; run_apollo ;;
        *)       echo "Usage: $0 sdk --quick {all|consul|nacos|apollo} [filter]"; suite_rc=1 ;;
    esac
    suite_rc=$?
    set -e

    # --- Archive server logs (avoid per-run overwrite, ease post-mortem) --
    local ts=$(date +%Y%m%d-%H%M%S)
    local arc="/tmp/sdk-${MODE}-${SUITE}-server-${ts}.log"
    : > "$arc"
    for src in "${LOG_SOURCES[@]:-}"; do
        [ -e "$src" ] || continue
        if [ -f "$src" ]; then
            { echo "===== $src ====="; cat "$src"; } >> "$arc"
        elif [ -d "$src" ]; then
            for f in "$src"/*.log "$src"/stdout.log; do
                [ -f "$f" ] || continue
                { echo "===== $f ====="; cat "$f"; } >> "$arc"
            done
        fi
    done
    echo -e "${BOLD}Server logs archived to: ${arc}${NC}"

    echo -e "\n${BOLD}Done.${NC}"
    return $suite_rc
}

# ---------------------------------------------------------------------------
# sdk --matrix: full SDK compatibility matrix (2 modes x 3 stores)
# ---------------------------------------------------------------------------
cmd_sdk_matrix() {
    # Auto-detect JAVA_HOME if not set (required for Nacos Java SDK tests)
    if [ -z "${JAVA_HOME:-}" ]; then
        if [ -x /usr/libexec/java_home ]; then
            export JAVA_HOME="$(/usr/libexec/java_home 2>/dev/null)" || true
        elif command -v javac &>/dev/null; then
            export JAVA_HOME="$(dirname "$(dirname "$(readlink -f "$(which javac)")")")"
        fi
    fi
    if [ -n "${JAVA_HOME:-}" ]; then
        export PATH="${JAVA_HOME}/bin:${PATH}"
    fi

    BINARY="${ROOT}/target/release/batata-server"
    BUILD=true
    ONLY=()
    SKIP=()
    RUN_STANDALONE=false
    RUN_CLUSTER=false
    DEFAULT_RUN=false

    for arg in "$@"; do
        case "$arg" in
            --no-build) BUILD=false ;;
            --help|-h)
                sed -n '5,24p' "$0" | grep -E '^#   ' | sed 's/^#   //'
                echo
                echo "组合名: standalone-rock standalone-mysql standalone-postgres cluster-rock cluster-mysql cluster-postgres"
                return 0 ;;
            --standalone) RUN_STANDALONE=true ;;
            --cluster) RUN_CLUSTER=true ;;
            --only=*)  ONLY+=("${arg#*=}") ;;
            --skip=*)  SKIP+=("${arg#*=}") ;;
            *) echo "未知参数: $arg"; return 1 ;;
        esac
    done
    [ "$RUN_STANDALONE" = false ] && [ "$RUN_CLUSTER" = false ] && DEFAULT_RUN=true

    declare -a COMBOS=( )
    if [ "$RUN_STANDALONE" = true ] || [ "$DEFAULT_RUN" = true ]; then
        COMBOS+=(standalone-rock standalone-mysql standalone-postgres)
    fi
    if [ "$RUN_CLUSTER" = true ] || [ "$DEFAULT_RUN" = true ]; then
        COMBOS+=(cluster-rock cluster-mysql cluster-postgres)
    fi
    [ "${#ONLY[@]}" -gt 0 ] && COMBOS=("${ONLY[@]}")

    RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'
    log()  { echo -e "${CYAN}[MATRIX]${NC} $*"; }
    ok()   { echo -e "${GREEN}[OK]${NC} $*"; }
    fail() { echo -e "${RED}[FAIL]${NC} $*"; }
    warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }

    sdk_wait_for_server() { # url timeout_secs
        local url="$1" timeout="${2:-120}" waited=0
        while [ "$waited" -lt "$timeout" ]; do
            code=$(curl -s -o /dev/null -w '%{http_code}' "$url" 2>/dev/null || echo 000)
            [ "$code" = "200" ] && return 0
            sleep 2; waited=$((waited+2))
        done
        return 1
    }

    stop_servers() {
        for pid in "${SERVER_PIDS[@]:-}"; do
            kill "$pid" 2>/dev/null || true
        done
        sleep 2
        for port in 8848 8858 8868 8081 8082 8083 8500 8510 8520 \
                    9848 9858 9868 9849 9859 9869 7848 7858 7868; do
            lsof -ti:"$port" 2>/dev/null | xargs kill -9 2>/dev/null || true
        done
        SERVER_PIDS=()
    }

    clean_dirs() {
        rm -rf "${ROOT}/data/sdk-node1" "${ROOT}/data/sdk-node2" "${ROOT}/data/sdk-node3" \
               "${ROOT}/logs/sdk-node1" "${ROOT}/logs/sdk-node2" "${ROOT}/logs/sdk-node3" \
               "${ROOT}/data/rock-n1" "${ROOT}/data/rock-n2" "${ROOT}/data/rock-n3" \
               "${ROOT}/data/batata_rocksdb" 2>/dev/null || true
        rm -f "${ROOT}/conf/cluster.conf" 2>/dev/null || true
        mkdir -p "${ROOT}/logs/sdk-node1" "${ROOT}/logs/sdk-node2" "${ROOT}/logs/sdk-node3"
    }

    recreate_db() { # platform
        local platform="$1"
        case "$platform" in
            mysql)
                podman exec mysql mysql -uroot -pdevterry \
                    -e "DROP DATABASE IF EXISTS batata_sdk_test; CREATE DATABASE batata_sdk_test CHARACTER SET utf8mb4;" \
                    || { fail "MySQL 重建库失败"; return 1; }
                ok "MySQL 库 batata_sdk_test 已重建" ;;
            postgres)
                podman exec postgres psql -U postgres -c "DROP DATABASE IF EXISTS batata_sdk_test;" >/dev/null 2>&1
                podman exec postgres psql -U postgres -c "CREATE DATABASE batata_sdk_test OWNER postgres;" >/dev/null 2>&1 \
                    || { fail "Postgres 重建库失败"; return 1; }
                ok "Postgres 库 batata_sdk_test 已重建" ;;
        esac
        return 0
    }

    scan_logs() {
        local dirs=("${ROOT}/logs/sdk-node1" "${ROOT}/logs/sdk-node2" "${ROOT}/logs/sdk-node3")
        local pat=' WARN| ERROR|E  *[0-9]* *//|panicked at|stack backtrace|Error: |response 500|\[FATAL\]'
        for d in "${dirs[@]}"; do
            [ -d "$d" ] || continue
            find "$d" -maxdepth 1 -type f \( -name '*.log' -o -name 'stdout.log' \) -print0 2>/dev/null \
                | while IFS= read -r -d '' f; do
                    grep -nE "$pat" "$f" 2>/dev/null | sed "s|^|$(basename "$d")/$(basename "$f"):|"
                done
        done
    }

    start_standalone() { # platform_flag..., db_args...
        local args=("$@")
        "${BINARY}" -m standalone -d merged \
            --batata.server.main.port=8848 --batata.console.port=8081 \
            --batata.plugin.consul.enabled=true --batata.plugin.consul.port=8500 \
            --batata.plugin.apollo.enabled=true --batata.plugin.apollo.port=18080 \
            "${args[@]}" \
            > "${ROOT}/logs/sdk-node1/stdout.log" 2>&1 &
        SERVER_PIDS+=("$!")
    }

    start_cluster() {
        local args=("$@")
        local is_embedded=false
        case " ${args[*]} " in *"embedded"*) is_embedded=true ;; esac
        local n_dirs=()
        if [ "$is_embedded" = true ]; then
            n_dirs=("${ROOT}/data/rock-n1" "${ROOT}/data/rock-n2" "${ROOT}/data/rock-n3")
        fi
        cluster_write_conf "$ROOT"
        local common=(-m cluster --batata.member.list="${CLUSTER_MEMBER_LIST}" --batata.plugin.consul.enabled=true --batata.plugin.apollo.enabled=true "${args[@]}")
        for i in 0 1 2; do
            local deploy=server
            [ "$i" -eq 0 ] && deploy=merged
            local data_arg=()
            [ "$is_embedded" = true ] && data_arg=(--batata.persistence.embedded.data_dir="${n_dirs[$i]}")
            "${BINARY}" "${common[@]}" -d "$deploy" \
                --batata.server.main.port="${CLUSTER_NODE_PORTS[$i]}" \
                --batata.console.port="${CLUSTER_CONSOLE_PORTS[$i]}" \
                --batata.plugin.consul.port="${CLUSTER_CONSUL_PORTS[$i]}" \
                --batata.plugin.apollo.port="${CLUSTER_APOLLO_PORTS[$i]}" \
                "${data_arg[@]}" \
                > "${ROOT}/logs/sdk-node$((i+1))/stdout.log" 2>&1 &
            SERVER_PIDS+=("$!")
        done
    }

    run_combo() {
        local combo="$1" platform="$2" mode="$3"
        log "=============== [${combo}] 开始 (mode=${mode}, platform=${platform}) ==============="

        if [ "$platform" != "embedded" ]; then
            recreate_db "$platform" || { stop_servers; return 1; }
        fi
        clean_dirs

        local args=()
        case "$platform" in
            embedded) args+=(--batata.sql.init.platform=embedded) ;;
            mysql)    args+=(--batata.sql.init.platform=mysql "--db-url=mysql://root:devterry@127.0.0.1:3306/batata_sdk_test") ;;
            postgres) args+=(--batata.sql.init.platform=postgresql "--db-url=postgres://postgres:devterry@127.0.0.1:5432/batata_sdk_test") ;;
        esac
        if [ "$mode" = "standalone" ]; then
            start_standalone "${args[@]}"
        else
            start_cluster "${args[@]}"
        fi

        if ! sdk_wait_for_server "http://127.0.0.1:8848${LIVENESS_PATH}" 150; then
            fail "${combo}: 尚未就绪，日志如下"
            tail -40 "${ROOT}/logs/sdk-node1/stdout.log"
            stop_servers
            return 1
        fi
        ok "${combo}: node1 (8848) 就绪"

        if [ "$mode" = "cluster" ]; then
            sleep 8
            ok "${combo}: cluster 形成"
        fi

        log ">>> 初始化管理用户 nacos/nacos"
        "${SCRIPT_DIR}/init-admin.sh" nacos nacos http://127.0.0.1:8848 \
            | sed 's/^/    [init-admin] /'
        sleep 1

        log ">>> 运行 Nacos Java SDK (server=127.0.0.1:8848)"
        local nacos_rc=0
        MVN_QUIET=1 sdk_run_nacos || nacos_rc=$?
        [ "$nacos_rc" -eq 0 ] && ok "Nacos SDK 通过" || fail "Nacos SDK 退出码 $nacos_rc"

        log ">>> 运行 Consul Go SDK (addr=127.0.0.1:8500)"
        local consul_rc=0
        CONSUL_HTTP_TOKEN=root sdk_run_consul || consul_rc=$?
        [ "$consul_rc" -eq 0 ] && ok "Consul SDK 通过" || fail "Consul SDK 退出码 $consul_rc"

        log ">>> 运行 Apollo Java SDK (meta=127.0.0.1:18080)"
        local apollo_rc=0
        APOLLO_META="http://127.0.0.1:18080" sdk_run_apollo || apollo_rc=$?
        [ "$apollo_rc" -eq 0 ] && ok "Apollo SDK 通过" || fail "Apollo SDK 退出码 $apollo_rc"

        log ">>> 检查日志目录异常日志"
        local danger_log_scan=0
        while IFS= read -r line; do
            danger_log_scan=1
            warn "  发现异常日志: $line"
        done < <(scan_logs)
        [ "$danger_log_scan" -eq 0 ] && ok "日志目录无异常"

        stop_servers

        echo -e "${CYAN}──── ${combo}: Nacos=$([ $nacos_rc -eq 0 ] && echo PASS || echo FAIL) Consul=$([ $consul_rc -eq 0 ] && echo PASS || echo FAIL) Apollo=$([ $apollo_rc -eq 0 ] && echo PASS || echo FAIL) ────${NC}"
        [ "$nacos_rc" -eq 0 ] && [ "$consul_rc" -eq 0 ] && [ "$apollo_rc" -eq 0 ] && return 0 || return 1
    }

    SERVER_PIDS=()
    LIVENESS_PATH="/nacos/v3/admin/core/state/liveness"

    if [ "$BUILD" = true ]; then
        log "构建 release 二进制..."
        ( cd "${ROOT}" && cargo build --release -p batata-server ) || { fail "构建失败"; return 1; }
    fi
    [ -x "$BINARY" ] || { fail "找不到二进制 $BINARY，请先 --no-build 前的构建或去掉 --no-build"; return 1; }

    log "将执行 ${#COMBOS[@]} 个组合: ${COMBOS[*]}"
    RESULTS=()
    for combo in "${COMBOS[@]}"; do
        if [[ " ${SKIP[*]} " == *" ${combo} "* ]]; then warn "跳过 ${combo}"; continue; fi
        rc=0
        case "$combo" in
            standalone-rock)     run_combo "$combo" embedded  standalone || rc=$? ;;
            standalone-mysql)    run_combo "$combo" mysql     standalone || rc=$? ;;
            standalone-postgres) run_combo "$combo" postgres  standalone || rc=$? ;;
            cluster-rock)        run_combo "$combo" embedded  cluster || rc=$? ;;
            cluster-mysql)       run_combo "$combo" mysql     cluster || rc=$? ;;
            cluster-postgres)    run_combo "$combo" postgres  cluster || rc=$? ;;
            *) warn "未知组合: $combo"; continue ;;
        esac
        RESULTS+=( "${combo}:$( [ "$rc" -eq 0 ] && echo PASS || echo FAIL )" )
    done

    echo
    log "=========== 测试矩阵结果摘要 ==========="
    for r in "${RESULTS[@]}"; do
        name="${r%%:*}"; res="${r##*:}"
        [ "$res" = "PASS" ] && c="$GREEN" || c="$RED"
        echo -e "${c}${res}${NC}  ${name}"
    done
    return 0
}

# ---------------------------------------------------------------------------
# sdk: dispatcher between --quick and --matrix
# ---------------------------------------------------------------------------
cmd_sdk() {
    local mode="quick"
    local rest=()
    for a in "$@"; do
        case "$a" in
            --quick)  mode="quick" ;;
            --matrix) mode="matrix" ;;
            *)        rest+=("$a") ;;
        esac
    done
    if [ "$mode" = "matrix" ]; then
        cmd_sdk_matrix "${rest[@]}"
    else
        cmd_sdk_quick "${rest[@]}"
    fi
}

# ---------------------------------------------------------------------------
# console: console datasource E2E (delegates to the kept test_* scripts)
# ---------------------------------------------------------------------------
cmd_console() {
    MODE="${1:-}"

    check_dependency() {
        if ! command -v "$1" &> /dev/null; then
            echo -e "${RED}Error: '$1' is required but not installed.${NC}"
            exit 1
        fi
    }
    RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'

    check_dependency curl
    check_dependency jq

    if [ -z "$MODE" ]; then
        echo "Usage: $0 console <mode>"
        echo ""
        echo "Available modes:"
        echo "  standalone-embedded    Standalone + RocksDB"
        echo "  standalone-externaldb  Standalone + MySQL/PostgreSQL"
        echo "  cluster-externaldb     Cluster + MySQL/PostgreSQL (3-node)"
        echo "  cluster-embedded       Cluster + Raft+RocksDB (3-node)"
        echo "  all                    Run all modes"
        return 1
    fi

    OVERALL_RESULT=0
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

    case "$MODE" in
        standalone-embedded)
            run_test "${SCRIPT_DIR}/test_standalone_embedded.sh" "Standalone + Embedded" || OVERALL_RESULT=1 ;;
        standalone-externaldb)
            run_test "${SCRIPT_DIR}/test_standalone_externaldb.sh" "Standalone + ExternalDb" || OVERALL_RESULT=1 ;;
        cluster-externaldb)
            run_test "${SCRIPT_DIR}/test_cluster_externaldb.sh" "Cluster + ExternalDb" || OVERALL_RESULT=1 ;;
        cluster-embedded)
            run_test "${SCRIPT_DIR}/test_cluster_embedded.sh" "Cluster + Embedded" || OVERALL_RESULT=1 ;;
        all)
            echo -e "${YELLOW}NOTE: 'all' mode requires servers running for each mode.${NC}"
            run_test "${SCRIPT_DIR}/test_standalone_embedded.sh" "Standalone + Embedded" || OVERALL_RESULT=1
            run_test "${SCRIPT_DIR}/test_standalone_externaldb.sh" "Standalone + ExternalDb" || OVERALL_RESULT=1
            run_test "${SCRIPT_DIR}/test_cluster_externaldb.sh" "Cluster + ExternalDb" || OVERALL_RESULT=1
            run_test "${SCRIPT_DIR}/test_cluster_embedded.sh" "Cluster + Embedded" || OVERALL_RESULT=1 ;;
        *)
            echo -e "${RED}Unknown mode: ${MODE}${NC}"
            return 1 ;;
    esac

    echo ""
    if [ $OVERALL_RESULT -eq 0 ]; then
        echo -e "${GREEN}All test suites passed!${NC}"
    else
        echo -e "${RED}Some test suites failed!${NC}"
    fi
    return $OVERALL_RESULT
}

# ---------------------------------------------------------------------------
# all: full test suite (level: quick|standard|full|sdk)
# ---------------------------------------------------------------------------
cmd_all() {
    LEVEL="${1:-quick}"
    SKIP_BUILD=false
    BINARY="${PROJECT_ROOT}/target/release/batata-server"
    SERVER_PIDS=()
    DOCKER_COMPOSE_STARTED=false
    SDK_DOCKER_STARTED=false
    CLUSTER_CONF_BACKED_UP=false
    OVERALL_EXIT_CODE=0
    STAGE_RESULTS=()

    for arg in "$@"; do
        case "$arg" in
            --skip-build) SKIP_BUILD=true ;;
            --help|-h)
                head -20 "$0" | tail -17
                return 0 ;;
            quick|standard|full|sdk) LEVEL="$arg" ;;
            *)
                echo "Unknown argument: $arg"
                echo "Usage: $0 all [quick|standard|full|sdk] [--skip-build] [--help]"
                return 1 ;;
        esac
    done

    cleanup() {
        log_info "Cleaning up..."
        for pid in "${SERVER_PIDS[@]}"; do
            if kill -0 "$pid" 2>/dev/null; then
                log_info "Stopping server PID ${pid}..."
                kill "$pid" 2>/dev/null || true
                wait "$pid" 2>/dev/null || true
            fi
        done
        SERVER_PIDS=()
        if [ "$DOCKER_COMPOSE_STARTED" = true ]; then
            log_info "Stopping test databases..."
            cd "$PROJECT_ROOT"
            podman compose -f deploy/compose/podman-compose.test.yml down -v 2>/dev/null || true
        fi
        if [ "$SDK_DOCKER_STARTED" = true ]; then
            log_info "Stopping SDK test stack..."
            cd "${PROJECT_ROOT}/sdk-tests"
            podman compose -f deploy/compose/podman-compose.yml down -v 2>/dev/null || true
            cd "$PROJECT_ROOT"
        fi
        rm -rf "${PROJECT_ROOT}/data/test-node"* "${PROJECT_ROOT}/logs/test-node"* 2>/dev/null || true
        if [ "$CLUSTER_CONF_BACKED_UP" = true ] && [ -f "${PROJECT_ROOT}/conf/cluster.conf.bak" ]; then
            mv "${PROJECT_ROOT}/conf/cluster.conf.bak" "${PROJECT_ROOT}/conf/cluster.conf"
            log_info "Restored cluster.conf"
        fi
    }
    trap cleanup EXIT

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

    stop_bg_server() {
        local pid="$1"
        if kill -0 "$pid" 2>/dev/null; then
            log_info "Stopping server PID ${pid}..."
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
        local new_pids=()
        for p in "${SERVER_PIDS[@]}"; do
            [ "$p" != "$pid" ] && new_pids+=("$p")
        done
        SERVER_PIDS=("${new_pids[@]+"${new_pids[@]}"}")
    }

    init_admin() {
        local server_url="${1:-http://127.0.0.1:8848}"
        if bash "${SCRIPT_DIR}/init-admin.sh" nacos nacos "${server_url}" >/dev/null 2>&1; then
            log_info "Admin user initialized on ${server_url}"
            return 0
        fi
        log_fail "Failed to initialize admin user on ${server_url}"
        return 1
    }

    wait_for_podman_healthy() {
        local service="$1"
        local timeout="${2:-120}"
        local elapsed=0
        log_info "Waiting for Podman container '${service}' to be healthy (timeout=${timeout}s)..."
        while [ $elapsed -lt $timeout ]; do
            local status
            status=$(podman inspect --format='{{.State.Health.Status}}' "$service" 2>/dev/null || echo "not_found")
            if [ "$status" = "healthy" ]; then
                log_info "Podman container '${service}' is healthy (took ${elapsed}s)"
                return 0
            fi
            sleep 5
            elapsed=$((elapsed + 5))
        done
        log_fail "Podman container '${service}' did not become healthy within ${timeout}s"
        return 1
    }

    record_stage() {
        local name="$1"
        local result="$2"
        STAGE_RESULTS+=("${result}:${name}")
    }

    require_cmd() {
        local cmd="$1"
        if ! command -v "$cmd" &>/dev/null; then
            log_fail "Required command not found: ${cmd}"
            return 1
        fi
    }

    run_quick() {
        log_section "Stage: Quick (fmt + clippy + unit tests)"
        cd "$PROJECT_ROOT"
        log_info "Running cargo fmt --all --check..."
        cargo fmt --all --check 2>&1 && log_pass "cargo fmt check" || { log_fail "cargo fmt check — run 'cargo fmt --all' to fix"; return 1; }
        log_info "Running cargo clippy --workspace..."
        cargo clippy --workspace 2>&1 && log_pass "cargo clippy" || { log_fail "cargo clippy"; return 1; }
        log_info "Running cargo test --workspace..."
        cargo test --workspace 2>&1 && log_pass "cargo test --workspace" || { log_fail "cargo test --workspace"; return 1; }
        return 0
    }

    run_standard() {
        log_section "Stage: Standard (standalone embedded E2E)"
        cd "$PROJECT_ROOT"
        if [ "$SKIP_BUILD" = true ] && [ -f "$BINARY" ]; then
            log_info "Skipping build (--skip-build, binary exists)"
        else
            log_info "Building release binary..."
            cargo build --release -p batata-server 2>&1 && log_pass "cargo build --release" || { log_fail "cargo build --release"; return 1; }
        fi
        local pid
        pid=$(start_bg_server "standalone_embedded" --batata.sql.init.platform=embedded)
        if ! wait_for_server "http://127.0.0.1:8081" 90; then
            log_fail "Standalone embedded server failed to start"
            tail -50 /tmp/batata_test_standalone_embedded.log 2>/dev/null || true
            stop_bg_server "$pid"
            return 1
        fi
        init_admin "http://127.0.0.1:8848"
        log_info "Running standalone embedded E2E tests..."
        local test_rc=0
        bash "${SCRIPT_DIR}/test_standalone_embedded.sh" || test_rc=$?
        if [ $test_rc -ne 0 ]; then
            log_fail "Standalone embedded E2E tests failed (exit code: ${test_rc})"
            stop_bg_server "$pid"
            return 1
        fi
        log_pass "Standalone embedded E2E tests completed"
        stop_bg_server "$pid"

        log_info "Starting server with Apollo plugin (embedded mode)..."
        local apollo_pid
        apollo_pid=$(start_bg_server "standalone_apollo_embedded" \
            --batata.sql.init.platform=embedded \
            --batata.plugin.apollo.enabled=true \
            --batata.plugin.apollo.port=18080 \
            --batata.server.main.port=18848 \
            --batata.console.port=18081)
        if ! wait_for_server "http://127.0.0.1:18081" 90; then
            log_fail "Apollo embedded server failed to start"
            tail -50 /tmp/batata_test_standalone_apollo_embedded.log 2>/dev/null || true
            stop_bg_server "$apollo_pid"
            return 1
        fi
        log_info "Waiting for Apollo plugin..."
        for i in $(seq 1 30); do
            curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:18080/health" 2>/dev/null | grep -q "200" && break
            sleep 2
        done
        log_info "Running Apollo embedded E2E tests..."
        local apollo_test_rc=0
        APOLLO_PORT=18080 MAIN_PORT=18848 cmd_apollo || apollo_test_rc=$?
        stop_bg_server "$apollo_pid"
        if [ $apollo_test_rc -ne 0 ]; then
            log_fail "Apollo embedded E2E tests failed (exit code: ${apollo_test_rc})"
            return 1
        fi
        log_pass "Apollo embedded E2E tests completed"
        return 0
    }

    run_cluster_embedded() {
        log_section "Stage: Full — Cluster Embedded E2E"
        cd "$PROJECT_ROOT"
        if [ -f "${PROJECT_ROOT}/conf/cluster.conf" ]; then
            cp "${PROJECT_ROOT}/conf/cluster.conf" "${PROJECT_ROOT}/conf/cluster.conf.bak"
            CLUSTER_CONF_BACKED_UP=true
        fi
        cluster_write_conf "${PROJECT_ROOT}"
        local pid1 pid2 pid3
        pid1=$(start_bg_server "cluster_node1" -m cluster --batata.sql.init.platform=embedded \
            --batata.server.main.port=${CLUSTER_NODE_PORTS[0]} --batata.console.port=${CLUSTER_CONSOLE_PORTS[0]} \
            --batata.persistence.embedded.data_dir=data/test-node1 --batata.logs.path=logs/test-node1)
        pid2=$(start_bg_server "cluster_node2" -m cluster --batata.sql.init.platform=embedded \
            --batata.server.main.port=${CLUSTER_NODE_PORTS[1]} --batata.console.port=${CLUSTER_CONSOLE_PORTS[1]} \
            --batata.persistence.embedded.data_dir=data/test-node2 --batata.logs.path=logs/test-node2)
        pid3=$(start_bg_server "cluster_node3" -m cluster --batata.sql.init.platform=embedded \
            --batata.server.main.port=${CLUSTER_NODE_PORTS[2]} --batata.console.port=${CLUSTER_CONSOLE_PORTS[2]} \
            --batata.persistence.embedded.data_dir=data/test-node3 --batata.logs.path=logs/test-node3)
        local all_ready=true
        for port in "${CLUSTER_CONSOLE_PORTS[@]}"; do
            if ! wait_for_server "http://127.0.0.1:${port}" 120; then
                log_fail "Cluster node on console port ${port} failed to start"
                all_ready=false
            fi
        done
        if [ "$all_ready" = false ]; then
            tail -30 /tmp/batata_test_cluster_node1.log 2>/dev/null || true
            stop_bg_server "$pid1"; stop_bg_server "$pid2"; stop_bg_server "$pid3"
            return 1
        fi
        init_admin "http://127.0.0.1:8848"
        log_info "Waiting for Raft leader election (10s)..."
        sleep 10
        log_info "Running cluster embedded E2E tests..."
        local test_rc=0
        bash "${SCRIPT_DIR}/test_cluster_embedded.sh" || test_rc=$?
        stop_bg_server "$pid1"; stop_bg_server "$pid2"; stop_bg_server "$pid3"
        rm -rf "${PROJECT_ROOT}/data/test-node"* "${PROJECT_ROOT}/logs/test-node"* 2>/dev/null || true
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

    run_standalone_externaldb() {
        log_section "Stage: Full — Standalone ExternalDb E2E (MySQL)"
        cd "$PROJECT_ROOT"
        log_info "Starting MySQL test database..."
        podman compose -f deploy/compose/podman-compose.test.yml up -d mysql-test
        DOCKER_COMPOSE_STARTED=true
        if ! wait_for_podman_healthy "batata-mysql-test" 120; then
            log_fail "MySQL did not become healthy"
            return 1
        fi
        local pid
        pid=$(start_bg_server "standalone_mysql" --db-url "mysql://batata:batata@127.0.0.1:3307/batata_test")
        if ! wait_for_server "http://127.0.0.1:8081" 90; then
            log_fail "Standalone MySQL server failed to start"
            tail -50 /tmp/batata_test_standalone_mysql.log 2>/dev/null || true
            stop_bg_server "$pid"
            return 1
        fi
        init_admin "http://127.0.0.1:8848"
        log_info "Running standalone external DB E2E tests..."
        local test_rc=0
        bash "${SCRIPT_DIR}/test_standalone_externaldb.sh" || test_rc=$?
        stop_bg_server "$pid"
        if [ $test_rc -ne 0 ]; then
            log_fail "Standalone ExternalDb E2E tests failed (exit code: ${test_rc})"
            return 1
        fi
        log_pass "Standalone ExternalDb E2E tests completed"
        return 0
    }

    run_cluster_externaldb() {
        log_section "Stage: Full — Cluster ExternalDb E2E (MySQL)"
        cd "$PROJECT_ROOT"
        if ! podman inspect --format='{{.State.Health.Status}}' "batata-mysql-test" 2>/dev/null | grep -q "healthy"; then
            log_info "Restarting MySQL test database..."
            podman compose -f deploy/compose/podman-compose.test.yml up -d mysql-test
            DOCKER_COMPOSE_STARTED=true
            if ! wait_for_podman_healthy "batata-mysql-test" 120; then
                log_fail "MySQL did not become healthy"
                return 1
            fi
        fi
        if [ -f "${PROJECT_ROOT}/conf/cluster.conf" ] && [ ! -f "${PROJECT_ROOT}/conf/cluster.conf.bak" ]; then
            cp "${PROJECT_ROOT}/conf/cluster.conf" "${PROJECT_ROOT}/conf/cluster.conf.bak"
            CLUSTER_CONF_BACKED_UP=true
        fi
        cluster_write_conf "${PROJECT_ROOT}"
        local db_url="mysql://batata:batata@127.0.0.1:3307/batata_test"
        local pid1 pid2 pid3
        pid1=$(start_bg_server "cluster_mysql_node1" -m cluster --db-url "$db_url" \
            --batata.server.main.port=${CLUSTER_NODE_PORTS[0]} --batata.console.port=${CLUSTER_CONSOLE_PORTS[0]} --batata.logs.path=logs/test-node1)
        pid2=$(start_bg_server "cluster_mysql_node2" -m cluster --db-url "$db_url" \
            --batata.server.main.port=${CLUSTER_NODE_PORTS[1]} --batata.console.port=${CLUSTER_CONSOLE_PORTS[1]} --batata.logs.path=logs/test-node2)
        pid3=$(start_bg_server "cluster_mysql_node3" -m cluster --db-url "$db_url" \
            --batata.server.main.port=${CLUSTER_NODE_PORTS[2]} --batata.console.port=${CLUSTER_CONSOLE_PORTS[2]} --batata.logs.path=logs/test-node3)
        local all_ready=true
        for port in "${CLUSTER_CONSOLE_PORTS[@]}"; do
            if ! wait_for_server "http://127.0.0.1:${port}" 120; then
                log_fail "Cluster MySQL node on console port ${port} failed to start"
                all_ready=false
            fi
        done
        if [ "$all_ready" = false ]; then
            stop_bg_server "$pid1"; stop_bg_server "$pid2"; stop_bg_server "$pid3"
            return 1
        fi
        init_admin "http://127.0.0.1:8848"
        log_info "Waiting for cluster formation (5s)..."
        sleep 5
        log_info "Running cluster external DB E2E tests..."
        local test_rc=0
        bash "${SCRIPT_DIR}/test_cluster_externaldb.sh" || test_rc=$?
        stop_bg_server "$pid1"; stop_bg_server "$pid2"; stop_bg_server "$pid3"
        rm -rf "${PROJECT_ROOT}/logs/test-node"* 2>/dev/null || true
        if [ "$CLUSTER_CONF_BACKED_UP" = true ] && [ -f "${PROJECT_ROOT}/conf/cluster.conf.bak" ]; then
            mv "${PROJECT_ROOT}/conf/cluster.conf.bak" "${PROJECT_ROOT}/conf/cluster.conf"
            CLUSTER_CONF_BACKED_UP=false
        fi
        log_info "Stopping test databases..."
        podman compose -f deploy/compose/podman-compose.test.yml down -v 2>/dev/null || true
        DOCKER_COMPOSE_STARTED=false
        if [ $test_rc -ne 0 ]; then
            log_fail "Cluster ExternalDb E2E tests failed (exit code: ${test_rc})"
            return 1
        fi
        log_pass "Cluster ExternalDb E2E tests completed"
        return 0
    }

    run_sdk() {
        log_section "Stage: SDK (Nacos Java + Consul Go compatibility tests)"
        cd "$PROJECT_ROOT"
        require_cmd podman || return 1
        log_info "Starting SDK test stack (MySQL + Batata + test containers)..."
        cd "${PROJECT_ROOT}/sdk-tests"
        podman compose up -d --build mysql batata
        SDK_DOCKER_STARTED=true
        cd "$PROJECT_ROOT"
        if ! wait_for_podman_healthy "batata-test-server" 180; then
            log_fail "Batata server in SDK podman stack did not become healthy"
            cd "${PROJECT_ROOT}/sdk-tests"
            podman compose logs batata 2>/dev/null | tail -50
            cd "$PROJECT_ROOT"
            return 1
        fi
        log_info "Batata server is healthy in Docker stack"
        local batata_url="http://127.0.0.1:8848"
        init_admin "$batata_url"
        log_info "Running Nacos Java SDK tests..."
        local nacos_rc=0
        cd "${PROJECT_ROOT}/sdk-tests"
        podman compose run --rm nacos-tests || nacos_rc=$?
        cd "$PROJECT_ROOT"
        if [ $nacos_rc -eq 0 ]; then log_pass "Nacos Java SDK tests"; else log_fail "Nacos Java SDK tests (exit code: ${nacos_rc})"; fi
        log_info "Running Consul Go SDK tests..."
        local consul_rc=0
        cd "${PROJECT_ROOT}/sdk-tests"
        podman compose run --rm consul-tests || consul_rc=$?
        cd "$PROJECT_ROOT"
        if [ $consul_rc -eq 0 ]; then log_pass "Consul Go SDK tests"; else log_fail "Consul Go SDK tests (exit code: ${consul_rc})"; fi
        log_info "Running Apollo Java SDK tests..."
        local apollo_rc=0
        cd "${PROJECT_ROOT}/sdk-tests"
        podman compose run --rm apollo-tests || apollo_rc=$?
        cd "$PROJECT_ROOT"
        if [ $apollo_rc -eq 0 ]; then log_pass "Apollo Java SDK tests"; else log_fail "Apollo Java SDK tests (exit code: ${apollo_rc})"; fi
        log_info "Stopping SDK test stack..."
        cd "${PROJECT_ROOT}/sdk-tests"
        podman compose down -v 2>/dev/null || true
        cd "$PROJECT_ROOT"
        SDK_DOCKER_STARTED=false
        if [ $nacos_rc -ne 0 ] || [ $consul_rc -ne 0 ] || [ $apollo_rc -ne 0 ]; then
            return 1
        fi
        log_pass "All SDK compatibility tests completed"
        return 0
    }

    cd "$PROJECT_ROOT"
    RUN_QUICK=true
    RUN_STANDARD=false
    RUN_FULL=false
    RUN_SDK=false
    case "$LEVEL" in
        quick)    ;;
        standard) RUN_STANDARD=true ;;
        full)     RUN_STANDALONE=true; RUN_FULL=true ;;
        sdk)      RUN_STANDARD=true; RUN_FULL=true; RUN_SDK=true ;;
        *)
            echo "Unknown level: $LEVEL"
            echo "Valid levels: quick, standard, full, sdk"
            return 1 ;;
    esac

    if [ "$RUN_QUICK" = true ]; then
        stage_rc=0; run_quick || stage_rc=$?
        if [ $stage_rc -eq 0 ]; then record_stage "Quick (fmt + clippy + unit tests)" "PASS"; else record_stage "Quick (fmt + clippy + unit tests)" "FAIL"; OVERALL_EXIT_CODE=1; fi
    fi
    if [ "$RUN_STANDARD" = true ] && [ "$OVERALL_EXIT_CODE" -eq 0 ]; then
        stage_rc=0; run_standard || stage_rc=$?
        if [ $stage_rc -eq 0 ]; then record_stage "Standard (standalone embedded E2E)" "PASS"; else record_stage "Standard (standalone embedded E2E)" "FAIL"; OVERALL_EXIT_CODE=1; fi
    fi
    if [ "$RUN_FULL" = true ] && [ "$OVERALL_EXIT_CODE" -eq 0 ]; then
        stage_rc=0; run_cluster_embedded || stage_rc=$?
        if [ $stage_rc -eq 0 ]; then record_stage "Full — Cluster Embedded E2E" "PASS"; else record_stage "Full — Cluster Embedded E2E" "FAIL"; OVERALL_EXIT_CODE=1; fi
    fi
    if [ "$RUN_FULL" = true ] && [ "$OVERALL_EXIT_CODE" -eq 0 ]; then
        if command -v podman &>/dev/null; then
            stage_rc=0; run_standalone_externaldb || stage_rc=$?
            if [ $stage_rc -eq 0 ]; then record_stage "Full — Standalone ExternalDb E2E" "PASS"; else record_stage "Full — Standalone ExternalDb E2E" "FAIL"; OVERALL_EXIT_CODE=1; fi
        else
            record_stage "Full — Standalone ExternalDb E2E" "SKIP (Docker not available)"
            log_skip "Standalone ExternalDb E2E skipped — Docker not available"
        fi
    fi
    if [ "$RUN_FULL" = true ] && [ "$OVERALL_EXIT_CODE" -eq 0 ]; then
        if command -v podman &>/dev/null; then
            stage_rc=0; run_cluster_externaldb || stage_rc=$?
            if [ $stage_rc -eq 0 ]; then record_stage "Full — Cluster ExternalDb E2E" "PASS"; else record_stage "Full — Cluster ExternalDb E2E" "FAIL"; OVERALL_EXIT_CODE=1; fi
        else
            record_stage "Full — Cluster ExternalDb E2E" "SKIP (Docker not available)"
            log_skip "Cluster ExternalDb E2E skipped — Docker not available"
        fi
    fi
    if [ "$RUN_SDK" = true ] && [ "$OVERALL_EXIT_CODE" -eq 0 ]; then
        stage_rc=0; run_sdk || stage_rc=$?
        if [ $stage_rc -eq 0 ]; then record_stage "SDK (Nacos Java + Consul Go + Apollo Java)" "PASS"; else record_stage "SDK (Nacos Java + Consul Go + Apollo Java)" "FAIL"; OVERALL_EXIT_CODE=1; fi
    fi

    echo ""
    echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║              Test Suite Summary                             ║${NC}"
    echo -e "${CYAN}╠══════════════════════════════════════════════════════════════╣${NC}"
    for entry in "${STAGE_RESULTS[@]}"; do
        result="${entry%%:*}"; name="${entry#*:}"
        case "$result" in
            PASS) echo -e "${CYAN}║${NC}  ${GREEN}PASS${NC}  ${name}" ;;
            FAIL) echo -e "${CYAN}║${NC}  ${RED}FAIL${NC}  ${name}" ;;
            SKIP*) echo -e "${CYAN}║${NC}  ${YELLOW}SKIP${NC}  ${name}" ;;
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
    return "$OVERALL_EXIT_CODE"
}

# ---------------------------------------------------------------------------
# Dispatch
# ---------------------------------------------------------------------------
usage() {
    cat <<EOF
Usage: $0 <subcommand> [args]

Subcommands:
  separation [main_url] [console_url]   Console/Server route separation test
  apollo [--live]                       Apollo plugin live verification
  sdk [--quick|--matrix]                SDK (Consul Go + Nacos Java + Apollo Java) tests
  console [mode]                        Console datasource E2E
                                        mode: standalone-embedded |
                                              standalone-externaldb |
                                              cluster-externaldb |
                                              cluster-embedded | all
  all [level] [--skip-build]            Full suite (quick|standard|full|sdk)

Examples:
  $0 separation
  $0 apollo --live
  $0 sdk --quick
  $0 sdk --matrix --only cluster-rock
  $0 console cluster-embedded
  $0 all full
EOF
}

CMD="${1:-help}"
shift 2>/dev/null || true
case "$CMD" in
    separation) cmd_separation "$@" ;;
    apollo)     cmd_apollo "$@" ;;
    sdk)        cmd_sdk "$@" ;;
    console)    cmd_console "$@" ;;
    all)        cmd_all "$@" ;;
    help|-h|--help) usage; exit 0 ;;
    *) echo "Unknown subcommand: $CMD"; usage; exit 1 ;;
esac
