#!/usr/bin/env bash
# ==============================================================================
# Batata SDK 兼容性测试矩阵编排脚本
#
# 覆盖 2 种模式 (standalone/cluster) × 3 种存储 (RocksDB/mysql/postgres) 组合，
# 每组依次：重建数据库 -> 启动 server -> 跑 Nacos Java SDK -> 跑 Consul Go SDK -> 停机。
#
# 依赖：
#   - Docker 中 mysql(3306)/postgres(5432) 已运行（root/devterry、postgres/devterry）
#   - Java + Maven、Go、Rust/Cargo 工具链
#   - release 二进制 target/release/batata-server（默认会自动构建）
#
# 用法：
#   ./scripts/run_sdk_matrix.sh [--no-build] [--only <组合,>] [--skip <组合,>]
#   ./scripts/run_sdk_matrix.sh --standalone     # 仅 standalone 三组
#   ./scripts/run_sdk_matrix.sh --cluster        # 仅 cluster 三组
#
# 组合名：
#   standalone-rock / standalone-mysql / standalone-postgres
#   cluster-rock   / cluster-mysql   / cluster-postgres
# ==============================================================================

set -o pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Auto-detect JAVA_HOME if not set (required for Nacos Java SDK tests)
if [ -z "${JAVA_HOME:-}" ]; then
  if [ -x /usr/libexec/java_home ]; then
    # macOS: use java_home helper
    export JAVA_HOME="$(/usr/libexec/java_home 2>/dev/null)" || true
  elif command -v javac &>/dev/null; then
    # Linux/generic: derive from javac path
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

# ---------------------------------------------------------------------------
# 解析参数
# ---------------------------------------------------------------------------
for arg in "$@"; do
  case "$arg" in
    --no-build) BUILD=false ;;
    --help|-h)
      sed -n '5,24p' "$0" | grep -E '^#   ' | sed 's/^#   //'
      echo
      echo "组合名: standalone-rock standalone-mysql standalone-postgres cluster-rock cluster-mysql cluster-postgres"
      exit 0 ;;
    --standalone) RUN_STANDALONE=true ;;
    --cluster) RUN_CLUSTER=true ;;
    --only=*)  ONLY+=("${arg#*=}") ;;
    --skip=*)  SKIP+=("${arg#*=}") ;;
    *) echo "未知参数: $arg"; exit 1 ;;
  esac
done
[ "$RUN_STANDALONE" = false ] && [ "$RUN_CLUSTER" = false ] && DEFAULT_RUN=true

# 组合定义与顺序（默认全部；--standalone/--cluster 只选对应组；--only 精确指定）
declare -a COMBOS=( )
if [ "$RUN_STANDALONE" = true ] || [ "$DEFAULT_RUN" = true ]; then
  COMBOS+=(standalone-rock standalone-mysql standalone-postgres)
fi
if [ "$RUN_CLUSTER" = true ] || [ "$DEFAULT_RUN" = true ]; then
  COMBOS+=(cluster-rock cluster-mysql cluster-postgres)
fi
[ "${#ONLY[@]}" -gt 0 ] && COMBOS=("${ONLY[@]}")

# ---------------------------------------------------------------------------
# 端口常量
# ---------------------------------------------------------------------------
# ---------------------------------------------------------------------------
# 端口常量（启动函数中硬编码，见 start_standalone / start_cluster）
# standalone: main 8848 / console 8081 / consul 8500
# cluster:    node1 8848/8081/8500, node2 8858/8082/8510, node3 8868/8083/8520
# ---------------------------------------------------------------------------
LIVENESS_PATH="/nacos/v3/admin/core/state/liveness"

# ---------------------------------------------------------------------------
# 输出 & 工具
# ---------------------------------------------------------------------------
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'
log()  { echo -e "${CYAN}[MATRIX]${NC} $*"; }
ok()   { echo -e "${GREEN}[OK]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }

wait_for_server() { # url timeout_secs
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
      docker exec mysql mysql -uroot -pdevterry \
        -e "DROP DATABASE IF EXISTS batata_sdk_test; CREATE DATABASE batata_sdk_test CHARACTER SET utf8mb4;" \
        || { fail "MySQL 重建库失败"; return 1; }
      ok "MySQL 库 batata_sdk_test 已重建" ;;
    postgres)
      docker exec postgres psql -U postgres -c "DROP DATABASE IF EXISTS batata_sdk_test;" >/dev/null 2>&1
      docker exec postgres psql -U postgres -c "CREATE DATABASE batata_sdk_test OWNER postgres;" >/dev/null 2>&1 \
        || { fail "Postgres 重建库失败"; return 1; }
      ok "Postgres 库 batata_sdk_test 已重建" ;;
  esac
  return 0
}

# ---------------------------------------------------------------------------
# 扫描日志目录，输出异常日志（WARN/ERROR/panic 等关键错误模式）
# 输出：匹配行（含文件与行号前缀），无异常则无输出
# ---------------------------------------------------------------------------
scan_logs() {
  local dirs=("${ROOT}/logs/sdk-node1" "${ROOT}/logs/sdk-node2" "${ROOT}/logs/sdk-node3")
  local pat=' WARN| ERROR|E  *[0-9]* *//|panicked at|stack backtrace|Error: |response 500|\[FATAL\]'
  for d in "${dirs[@]}"; do
    [ -d "$d" ] || continue
    # stdout.log 与日志文件名（*.log）都扫描
    find "$d" -maxdepth 1 -type f \( -name '*.log' -o -name 'stdout.log' \) -print0 2>/dev/null \
      | while IFS= read -r -d '' f; do
          grep -nE "$pat" "$f" 2>/dev/null | sed "s|^|$(basename "$d")/$(basename "$f"):|"
        done
  done
}

# ---------------------------------------------------------------------------
# 启动 standalone（单进程 merged）
# ---------------------------------------------------------------------------
start_standalone() { # platform_flag..., db_args...
  local args=("$@")
  "${BINARY}" -m standalone -d merged \
    --batata.server.main.port=8848 --batata.console.port=8081 \
    --batata.plugin.consul.enabled=true --batata.plugin.consul.port=8500 \
    "${args[@]}" \
    > "${ROOT}/logs/sdk-node1/stdout.log" 2>&1 &
  SERVER_PIDS+=("$!")
}

# ---------------------------------------------------------------------------
# 启动集群（3 节点：node1 merged + node2/3 server）
# ---------------------------------------------------------------------------
start_cluster() {
  local args=("$@")
  local is_embedded=false
  case " ${args[*]} " in *"embedded"*) is_embedded=true ;; esac

  local n1_dir=() n2_dir=() n3_dir=()
  if [ "$is_embedded" = true ]; then
    n1_dir=(--batata.persistence.embedded.data_dir="${ROOT}/data/rock-n1")
    n2_dir=(--batata.persistence.embedded.data_dir="${ROOT}/data/rock-n2")
    n3_dir=(--batata.persistence.embedded.data_dir="${ROOT}/data/rock-n3")
  fi

  cat > "${ROOT}/conf/cluster.conf" <<EOF
127.0.0.1:8848
127.0.0.1:8858
127.0.0.1:8868
EOF
  local common=(-m cluster --batata.member.list=127.0.0.1:8848,127.0.0.1:8858,127.0.0.1:8868 --batata.plugin.consul.enabled=true "${args[@]}")

  "${BINARY}" "${common[@]}" -d merged \
    --batata.server.main.port=8848 --batata.console.port=8081 \
    --batata.plugin.consul.port=8500 "${n1_dir[@]}" \
    > "${ROOT}/logs/sdk-node1/stdout.log" 2>&1 &
  SERVER_PIDS+=("$!")

  "${BINARY}" "${common[@]}" -d server \
    --batata.server.main.port=8858 --batata.console.port=8082 \
    --batata.plugin.consul.port=8510 "${n2_dir[@]}" \
    > "${ROOT}/logs/sdk-node2/stdout.log" 2>&1 &
  SERVER_PIDS+=("$!")

  "${BINARY}" "${common[@]}" -d server \
    --batata.server.main.port=8868 --batata.console.port=8083 \
    --batata.plugin.consul.port=8520 "${n3_dir[@]}" \
    > "${ROOT}/logs/sdk-node3/stdout.log" 2>&1 &
  SERVER_PIDS+=("$!")
}

# ---------------------------------------------------------------------------
# 执行组合
# ---------------------------------------------------------------------------
run_combo() {
  local combo="$1" platform="$2" mode="$3"
  log "=============== [${combo}] 开始 (mode=${mode}, platform=${platform}) ==============="

  # ① 环境准备：重建数据库（外部库）+ 清空目录
  if [ "$platform" != "embedded" ]; then
    recreate_db "$platform" || { stop_servers; return 1; }
  fi
  clean_dirs

  # ② 启动（构造存储参数）
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

  # ③ 等待 node1 就绪
  if ! wait_for_server "http://127.0.0.1:8848${LIVENESS_PATH}" 150; then
    fail "${combo}: 尚未就绪，日志如下"
    tail -40 "${ROOT}/logs/sdk-node1/stdout.log"
    stop_servers
    return 1
  fi
  ok "${combo}: node1 (8848) 就绪"

  if [ "$mode" = "cluster" ]; then
    sleep 8   # 等待 Raft 选主与复制
    ok "${combo}: cluster 形成"
  fi

  # ④ 初始化管理用户（auth 开启时必须；一次性，重复返回 409 会被忽略）
  log ">>> 初始化管理用户 nacos/nacos"
  "${SCRIPT_DIR}/init-admin.sh" nacos nacos http://127.0.0.1:8848 \
    | sed 's/^/    [init-admin] /'
  sleep 1

  # ⑤ Nacos Java SDK
  log ">>> 运行 Nacos Java SDK (server=127.0.0.1:8848)"
  local nacos_rc=0
  ( cd "${ROOT}/sdk-tests/nacos-java-tests" && \
    mvn -q test -Dnacos.server=127.0.0.1:8848 -Dnacos.username=nacos -Dnacos.password=nacos ) || nacos_rc=$?
  [ "$nacos_rc" -eq 0 ] && ok "Nacos SDK 通过" || fail "Nacos SDK 退出码 $nacos_rc"

  # ⑥ Consul Go SDK
  log ">>> 运行 Consul Go SDK (addr=127.0.0.1:8500)"
  local consul_rc=0
  ( cd "${ROOT}/sdk-tests/consul-go-tests" && \
      CONSUL_HTTP_ADDR=127.0.0.1:8500 CONSUL_HTTP_TOKEN=root go test -v -count=1 ./... ) || consul_rc=$?
  [ "$consul_rc" -eq 0 ] && ok "Consul SDK 通过" || fail "Consul SDK 退出码 $consul_rc"

  # ⑦ 检查日志目录中的异常
  log ">>> 检查日志目录异常日志"
  local danger_log_scan=0
  while IFS= read -r line; do
    danger_log_scan=1
    warn "  发现异常日志: $line"
  done < <(scan_logs)
  [ "$danger_log_scan" -eq 0 ] && ok "日志目录无异常"

  # ⑧ 停机
  stop_servers

  echo -e "${CYAN}──── ${combo}: Nacos=$([ $nacos_rc -eq 0 ] && echo PASS || echo FAIL) Consul=$([ $consul_rc -eq 0 ] && echo PASS || echo FAIL) ────${NC}"
  [ "$nacos_rc" -eq 0 ] && [ "$consul_rc" -eq 0 ] && return 0 || return 1
}

# ---------------------------------------------------------------------------
# 主流程
# ---------------------------------------------------------------------------
if [ "$BUILD" = true ]; then
  log "构建 release 二进制..."
  ( cd "${ROOT}" && cargo build --release -p batata-server ) || { fail "构建失败"; exit 1; }
fi
[ -x "$BINARY" ] || { fail "找不到二进制 $BINARY，请先 --no-build 前的构建或去掉 --no-build"; exit 1; }

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

# 摘要
echo
log "=========== 测试矩阵结果摘要 ==========="
for r in "${RESULTS[@]}"; do
  name="${r%%:*}"; res="${r##*:}"
  [ "$res" = "PASS" ] && c="$GREEN" || c="$RED"
  echo -e "${c}${res}${NC}  ${name}"
done
exit 0