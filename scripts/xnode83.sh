#!/usr/bin/env bash
# Cross-node experiment: is Consul health state cluster-consistent?
set -uo pipefail
ROOT="/Users/liangtianyu/work/github/easynet-cn/batata"
BIN="${BIN:-${ROOT}/target/release/batata-server}"
cd "$ROOT"
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
COMMON=(-m cluster
  --batata.member.list=127.0.0.1:8848,127.0.0.1:8858,127.0.0.1:8868
  --batata.sql.init.platform=embedded
  --batata.plugin.consul.enabled=true
  --batata.server.http.access_log.enabled=false)
for i in 0 1 2; do
  deploy=server; [ "$i" -eq 0 ] && deploy=merged
  "$BIN" "${COMMON[@]}" -d "$deploy" \
    --batata.server.main.port=$((8848 + i*10)) \
    --batata.console.port=$((8081 + i)) \
    --batata.plugin.consul.port=$((8500 + i*10)) \
    --batata.plugin.consul.node_name=c$((i+1)) \
    --batata.persistence.embedded.data_dir="${ROOT}/data/rock-n$((i+1))" \
    > "${ROOT}/logs/repro-node$((i+1))/stdout.log" 2>&1 &
  PIDS+=("$!")
done
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
sleep 6
echo "ready=$ready"

SVC="xnode-svc"
echo "== register on node1 (8500) with TTL check, initial critical =="
curl -s -X PUT "http://127.0.0.1:8500/v1/agent/service/register" -H 'Content-Type: application/json' -d "{\"ID\":\"$SVC\",\"Name\":\"$SVC\",\"Port\":8080,\"Check\":{\"TTL\":\"30s\",\"Status\":\"critical\"}}"
echo
sleep 2
for p in 8500 8510 8520; do
  echo "node $p: catalog entries=$(curl -s "http://127.0.0.1:$p/v1/health/service/$SVC?passing=false" | python3 -c 'import sys,json;d=json.load(sys.stdin);print(len(d))' 2>/dev/null)"
  echo "node $p: passing entries=$(curl -s "http://127.0.0.1:$p/v1/health/service/$SVC?passing=true" | python3 -c 'import sys,json;d=json.load(sys.stdin);print(len(d))' 2>/dev/null)"
  echo "node $p: checks=$(curl -s "http://127.0.0.1:$p/v1/agent/checks" | python3 -c 'import sys,json;d=json.load(sys.stdin);print({k:v["Status"] for k,v in d.items() if "xnode" in k or "serf" in k})' 2>/dev/null)"
done
echo "== pass TTL on node1 =="
curl -s -X PUT "http://127.0.0.1:8500/v1/agent/check/pass/service:$SVC" -H 'Content-Type: application/json' -d '{"Output":"ok"}'
echo
sleep 3
for p in 8500 8510 8520; do
  echo "node $p: passing entries=$(curl -s "http://127.0.0.1:$p/v1/health/service/$SVC?passing=true" | python3 -c 'import sys,json;d=json.load(sys.stdin);print(len(d))' 2>/dev/null)"
done
echo "== fail serfHealth on node2 only =="
curl -s -X PUT "http://127.0.0.1:8510/v1/agent/check/fail/serfHealth" -H 'Content-Type: application/json' -d '{"Output":"node2 down"}'
echo
sleep 3
for p in 8500 8510 8520; do
  echo "node $p: serfHealth=$(curl -s "http://127.0.0.1:$p/v1/agent/checks" | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d.get("serfHealth",{}).get("Status"))' 2>/dev/null)"
  echo "node $p: passing entries=$(curl -s "http://127.0.0.1:$p/v1/health/service/$SVC?passing=true" | python3 -c 'import sys,json;d=json.load(sys.stdin);print(len(d))' 2>/dev/null)"
done
