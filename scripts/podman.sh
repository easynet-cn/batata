#!/usr/bin/env bash
# ==============================================================================
# Podman one-click launcher for Batata (up / down / build)
#
# Uses `podman compose`, Podman's built-in compose subcommand, which delegates
# to an external compose provider. The default provider is docker-compose (the
# original Compose Spec implementation); override it with
# PODMAN_COMPOSE_PROVIDER, e.g. a standalone /usr/local/bin/docker-compose that
# does not depend on Docker Desktop.
#
# Usage:
#   ./scripts/podman.sh up [MODE] [extra profiles...]
#   ./scripts/podman.sh down [-v] [all|test|sdk]
#   ./scripts/podman.sh build [VERSION]
#
# up MODE (default: embedded):
#   embedded|default  Embedded RocksDB, merged mode
#   mysql             MySQL backend
#   postgres|pg       PostgreSQL backend
#   consul            Embedded + Consul API
#   cluster           3-node Raft cluster
#   split             Server + Console separated
#   test              Integration test databases
#   sdk               SDK test stack
#
# Examples:
#   ./scripts/podman.sh up
#   ./scripts/podman.sh up mysql
#   ./scripts/podman.sh up mysql monitoring
#   ./scripts/podman.sh up cluster
#   ./scripts/podman.sh down -v all
#   ./scripts/podman.sh build v0.1.0
#
# Environment variables:
#   BUILD=1          Force rebuild images (up)
#   DETACH=0         Run in foreground instead of background (up)
#   TAG=...          Image tag for build (default: batata:<VERSION>)
# ==============================================================================

set -euo pipefail
cd "$(dirname "$0")/.."

PROJECT_ROOT="$(pwd)"
COMPOSE_FILE="deploy/compose/podman-compose.yml"
TEST_COMPOSE_FILE="deploy/compose/podman-compose.test.yml"

cmd="${1:-}"
[ $# -gt 0 ] && shift

case "$cmd" in
    up)
        BUILD_FLAG=""
        DETACH_FLAG="-d"
        [ "${BUILD:-0}" = "1" ] && BUILD_FLAG="--build"
        [ "${DETACH:-1}" = "0" ] && DETACH_FLAG=""

        MODE="${1:-embedded}"
        shift 2>/dev/null || true

        # Collect additional profiles from remaining args
        EXTRA_PROFILES=()
        for arg in "$@"; do
            EXTRA_PROFILES+=("--profile" "$arg")
        done

        case "$MODE" in
            embedded|default)
                echo "==> Starting Batata (embedded RocksDB, merged mode)..."
                podman compose -f "$COMPOSE_FILE" ${BUILD_FLAG} up ${DETACH_FLAG} batata
                ;;
            mysql)
                echo "==> Starting Batata with MySQL..."
                podman compose -f "$COMPOSE_FILE" --profile mysql ${EXTRA_PROFILES[*]+"${EXTRA_PROFILES[@]}"} ${BUILD_FLAG} up ${DETACH_FLAG}
                ;;
            postgres|pg)
                echo "==> Starting Batata with PostgreSQL..."
                podman compose -f "$COMPOSE_FILE" --profile postgres ${EXTRA_PROFILES[*]+"${EXTRA_PROFILES[@]}"} ${BUILD_FLAG} up ${DETACH_FLAG}
                ;;
            consul)
                echo "==> Starting Batata with Consul compatibility..."
                podman compose -f "$COMPOSE_FILE" --profile consul ${BUILD_FLAG} up ${DETACH_FLAG}
                ;;
            cluster)
                echo "==> Starting 3-node Batata cluster..."
                podman compose -f "$COMPOSE_FILE" --profile cluster ${BUILD_FLAG} up ${DETACH_FLAG}
                echo ""
                echo "Cluster nodes:"
                echo "  Node 1: http://localhost:8848  (gRPC: 9848/9849, Consul: 8500)"
                echo "  Node 2: http://localhost:8858  (gRPC: 9858/9859, Consul: 8510)"
                echo "  Node 3: http://localhost:8868  (gRPC: 9868/9869, Consul: 8520)"
                ;;
            split)
                echo "==> Starting Batata (server + console separated)..."
                podman compose -f "$COMPOSE_FILE" --profile split ${BUILD_FLAG} up ${DETACH_FLAG}
                echo ""
                echo "Endpoints:"
                echo "  Server:  http://localhost:8848  (gRPC: 9848/9849)"
                echo "  Console: http://localhost:8081"
                ;;
            test)
                echo "==> Starting integration test databases..."
                podman compose -f "$TEST_COMPOSE_FILE" ${BUILD_FLAG} up ${DETACH_FLAG} mysql-test postgres-test
                echo ""
                echo "Test databases:"
                echo "  MySQL:      mysql://batata:batata@127.0.0.1:3307/batata_test"
                echo "  PostgreSQL: postgres://batata:batata@127.0.0.1:5433/batata_test"
                ;;
            sdk)
                echo "==> Starting SDK test stack..."
                cd sdk-tests
                podman compose ${BUILD_FLAG} up ${DETACH_FLAG} mysql batata
                echo ""
                echo "Run SDK tests with:"
                echo "  cd sdk-tests && podman compose --profile tests up"
                cd "$PROJECT_ROOT"
                ;;
            *)
                echo "Unknown mode: $MODE"
                echo "Available modes: embedded, mysql, postgres, consul, cluster, split, test, sdk"
                exit 1
                ;;
        esac

        if [ -n "$DETACH_FLAG" ] && [ "$MODE" != "test" ] && [ "$MODE" != "sdk" ]; then
            echo ""
            echo "Waiting for health check..."
            sleep 3
            echo "Status: podman compose -f $COMPOSE_FILE ps"
            echo "Logs:   podman compose -f $COMPOSE_FILE logs -f"
            echo "Stop:   ./scripts/podman.sh down"
        fi
        ;;

    down)
        VOLUME_FLAG=""
        MODE="${1:-default}"

        if [ "$MODE" = "-v" ] || [ "${2:-}" = "-v" ]; then
            VOLUME_FLAG="-v"
            echo "==> Stopping containers and removing volumes..."
        else
            echo "==> Stopping containers..."
        fi

        if [ "$MODE" = "all" ]; then
            # Stop all compose files
            podman compose -f "$COMPOSE_FILE" down $VOLUME_FLAG 2>/dev/null || true
            podman compose -f "$TEST_COMPOSE_FILE" down $VOLUME_FLAG 2>/dev/null || true
            (cd sdk-tests && podman compose down $VOLUME_FLAG 2>/dev/null) || true
            echo "All containers stopped."
        elif [ "$MODE" = "test" ]; then
            podman compose -f "$TEST_COMPOSE_FILE" down $VOLUME_FLAG
        elif [ "$MODE" = "sdk" ]; then
            (cd sdk-tests && podman compose down $VOLUME_FLAG)
        else
            podman compose -f "$COMPOSE_FILE" down $VOLUME_FLAG
        fi

        echo "Done."
        ;;

    build)
        VERSION="${1:-latest}"
        TAG="${TAG:-batata:${VERSION}}"

        echo "==> Building Batata container image: ${TAG}"
        echo "    Containerfile: Containerfile"
        echo ""

        podman build \
            -t "${TAG}" \
            -f Containerfile \
            --format oci \
            .

        echo ""
        echo "==> Build complete: ${TAG}"
        echo ""
        echo "Run with:"
        echo "  podman run -d --name batata -p 8848:8848 -p 8081:8081 -p 9848:9848 ${TAG}"
        ;;

    *)
        echo "Usage: $0 {up|down|build} ..."
        echo "  up    [MODE] [profiles...]   MODE: embedded|mysql|postgres|consul|cluster|split|test|sdk"
        echo "  down  [-v] [all|test|sdk]"
        echo "  build [VERSION]"
        exit 1
        ;;
esac
