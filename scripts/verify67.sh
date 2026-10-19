#!/usr/bin/env bash
# Targeted SDK verification for #67 (service listing / cluster filter / delete).
# Boots a standalone server on a project-local data dir, seeds the admin user
# (a project dir has no pre-seeded users, unlike the long-lived ~/batata store),
# then runs the Nacos Java maintainer suites — the ones exercising
# create / list / delete service.
set -uo pipefail
ROOT="/Users/liangtianyu/work/github/easynet-cn/batata"
BIN="${ROOT}/target/release/batata-server"
cd "$ROOT"

PIDS=()
cleanup() {
  for pid in "${PIDS[@]:-}"; do kill -9 "$pid" 2>/dev/null || true; done
  for p in 8848 8081; do lsof -ti:"$p" 2>/dev/null | xargs kill -9 2>/dev/null || true; done
}
trap cleanup EXIT

for p in 8848 8081; do lsof -ti:"$p" 2>/dev/null | xargs kill -9 2>/dev/null || true; done
sleep 1
rm -rf data/verify67 logs/verify67
mkdir -p logs/verify67

"$BIN" -m standalone -d merged \
  --batata.server.main.port=8848 --batata.console.port=8081 \
  --batata.sql.init.platform=embedded \
  --batata.persistence.embedded.data_dir="${ROOT}/data/verify67" \
  > "${ROOT}/logs/verify67/stdout.log" 2>&1 &
PIDS+=("$!")

echo "waiting for server..."
for _ in $(seq 1 60); do
  code=$(curl -s --noproxy '*' -o /dev/null -w '%{http_code}' \
    "http://127.0.0.1:8848/nacos/v3/admin/core/state/liveness" || echo 000)
  [ "$code" = "200" ] && break
  sleep 2
done
echo "liveness: $(curl -s --noproxy '*' -o /dev/null -w '%{http_code}' http://127.0.0.1:8848/nacos/v3/admin/core/state/liveness)"

"${ROOT}/scripts/init-admin.sh" nacos nacos http://127.0.0.1:8848

TESTS="${TESTS:-NacosMaintainServiceTest,NacosNamingMaintainerServiceTest}"
echo "=== running java tests: $TESTS ==="
( cd "${ROOT}/sdk-tests/nacos-java-tests" && \
  mvn -q test \
    -Dnacos.server=127.0.0.1:8848 \
    -Dnacos.username=nacos -Dnacos.password=nacos \
    -Dnacos.cluster.node1=127.0.0.1:8848 \
    -Dtest="$TESTS" ) 2>&1 | tee "${ROOT}/logs/verify67/java.txt" | tail -40

echo "=== summary ==="
grep -E "Tests run:|BUILD" "${ROOT}/logs/verify67/java.txt" | head -20
