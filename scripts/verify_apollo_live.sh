#!/usr/bin/env bash
# ==============================================================================
# Live verification of Apollo plugin P0-P5 work against a running server.
# Usage: ./scripts/verify_apollo_live.sh [BASE_URL]   (default http://127.0.0.1:8080)
# ==============================================================================
set -uo pipefail
BASE="${1:-http://127.0.0.1:8080}"
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
RK=$(curl -s "$BASE/configs/$APP/default/$NS" | python3 -c 'import sys,json;print(json.load(sys.stdin)["releaseKey"])')

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
# new publish → poll with id_before must see newer id immediately
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
exit $([ "$FAIL" = "0" ] && echo 0 || echo 1)
