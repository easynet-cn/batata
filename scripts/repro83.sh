#!/usr/bin/env bash
# Repro for #83: Consul ?passing filter returns 0 in a 3-node cluster.
# Starts a 3-node cluster, runs the failing tests, then shuts everything down.
set -uo pipefail
ROOT="/Users/liangtianyu/work/github/easynet-cn/batata"
BIN="${ROOT}/target/release/batata-server"
cd "$ROOT"

FILTER="${1:-TestHealthServicePassingFilter|TestDiscoveryFailover}"
PIDS=()

cleanup() {
  for pid in "${PIDS[@]:-}"; do kill -9 "$pid" 2>/dev/null || true; done
  for p in 8848 8858 8868 8500 8510 8520 8081 8082 8083; do
    lsof -ti:"$p" 2>/dev/null | xargs kill -9 2>/dev/null || true
  done
}
trap cleanup EXIT

for p in 8848 8858 8868 8500 8510 8520 8081 8082 8083; do
  lsof -ti:"$p" 2>/dev/null | xargs kill -9 2>/dev/null || true
done
sleep 1

rm -rf data/rock-n1 data/rock-n2 data/rock-n3 logs/repro-node1 logs/repro-node2 logs/repro-node3
mkdir -p logs/repro-node1 logs/repro-node2 logs/repro-node3

cat > conf/cluster.conf <<'EOF'
127.0.0.1:8848
127.0.0.1:8858
127.0.0.1:8868
EOF

export RUST_LOG="info,batata_plugin_consul=debug,batata_naming::healthcheck=debug"

COMMON=(-m cluster
  --batata.member.list=127.0.0.1:8848,127.0.0.1:8858,127.0.0.1:8868
  --batata.sql.init.platform=embedded
  --batata.plugin.consul.enabled=true
  --batata.plugin.apollo.enabled=true
  --batata.server.http.access_log.enabled=false)

if [ "${MODE:-cluster}" = "standalone" ]; then
  rm -rf data/standalone-store
  "$BIN" -m standalone -d merged \
    --batata.server.main.port=8848 --batata.console.port=8081 \
    --batata.plugin.consul.enabled=true --batata.plugin.consul.port=8500 \
    --batata.sql.init.platform=embedded \
    --batata.persistence.embedded.data_dir="${ROOT}/data/standalone-store" \
    > "${ROOT}/logs/repro-node1/stdout.log" 2>&1 &
  PIDS+=("$!")
else
for i in 0 1 2; do
  deploy=server; [ "$i" -eq 0 ] && deploy=merged
  "$BIN" "${COMMON[@]}" -d "$deploy" \
    --batata.server.main.port=$((8848 + i*10)) \
    --batata.console.port=$((8081 + i)) \
    --batata.plugin.consul.port=$((8500 + i*10)) \
    --batata.plugin.consul.node_name=c$((i+1)) \
    --batata.plugin.apollo.port=$((18080 + i*100)) \
    --batata.persistence.embedded.data_dir="${ROOT}/data/rock-n$((i+1))" \
    > "${ROOT}/logs/repro-node$((i+1))/stdout.log" 2>&1 &
  PIDS+=("$!")
done
fi

echo "waiting for cluster..."
ready=0
for i in $(seq 1 90); do
  ready=0
  for p in 8848 8858 8868; do
    code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:${p}/nacos/v3/admin/core/state/liveness" || echo 000)
    [ "$code" = "200" ] && ready=$((ready+1))
  done
  [ "$ready" -eq 3 ] && break
  sleep 2
done
sleep 8
echo "ready nodes: $ready"
for p in 8500 8510 8520; do echo -n "consul $p leader: "; curl -s "http://127.0.0.1:${p}/v1/status/leader"; echo; done

echo "=== running go tests: $FILTER ==="
( cd "${ROOT}/sdk-tests/consul-go-tests" && \
  CONSUL_HTTP_ADDR=127.0.0.1:8500 CONSUL_HTTP_TOKEN=root \
    go test ./... -v -count=1 -timeout 600s -run "$FILTER" ) > "${ROOT}/logs/repro-consul.txt" 2>&1
echo "=== go test summary ==="
grep -E "^--- FAIL|^ok|^FAIL|^\s+--- FAIL" "${ROOT}/logs/repro-consul.txt" | head -40
echo "PASS count: $(grep -c '^--- PASS' "${ROOT}/logs/repro-consul.txt")"
echo "--- leader after suite ---"
for p in 8500 8510 8520; do echo -n "$p: "; curl -s "http://127.0.0.1:${p}/v1/status/leader"; echo; done
