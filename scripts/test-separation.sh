#!/usr/bin/env bash
# Test console/server route separation.
# Verifies that V2 console routes are only on Console Server (8081),
# and NOT on Main Server (8848).
#
# Prerequisites: Server running in merged or separated mode, admin user initialized.
#
# Usage: ./scripts/test-separation.sh [main_url] [console_url]

set -euo pipefail

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

# --- Console Server: V2 console routes should be available ---
echo ""
echo "[Console Server - V2 console routes (should be available)]"

CODE=$(curl -s -o /dev/null -w "%{http_code}" "${CONSOLE_URL}/v2/console/health/liveness")
check "V2 console health/liveness" "200" "$CODE"

CODE=$(curl -s -o /dev/null -w "%{http_code}" "${CONSOLE_URL}/v2/console/health/readiness")
check "V2 console health/readiness" "200" "$CODE"

CODE=$(curl -s -o /dev/null -w "%{http_code}" "${CONSOLE_URL}/v2/console/namespace/list")
# 403 = auth required (route exists), 200 = ok
if [ "$CODE" = "200" ] || [ "$CODE" = "403" ]; then
  echo "  PASS: V2 console namespace/list (HTTP ${CODE}, route exists)"
  PASS=$((PASS + 1))
else
  echo "  FAIL: V2 console namespace/list (expected 200 or 403, got HTTP ${CODE})"
  FAIL=$((FAIL + 1))
fi

# --- Console Server: V3 console routes should be available ---
echo ""
echo "[Console Server - V3 console routes (should be available)]"

CODE=$(curl -s -o /dev/null -w "%{http_code}" "${CONSOLE_URL}/v3/console/health/liveness")
check "V3 console health/liveness" "200" "$CODE"

# --- Console Server: V2 Open API routes should NOT be available ---
echo ""
echo "[Console Server - V2 Open API routes (should NOT be available)]"

CODE=$(curl -s -o /dev/null -w "%{http_code}" "${CONSOLE_URL}/v2/cs/config")
check "V2 cs/config on Console" "404" "$CODE"

CODE=$(curl -s -o /dev/null -w "%{http_code}" "${CONSOLE_URL}/v2/ns/instance/list")
check "V2 ns/instance/list on Console" "404" "$CODE"

# --- Console Server: V3 Admin routes should NOT be available ---
echo ""
echo "[Console Server - V3 Admin routes (should NOT be available)]"

CODE=$(curl -s -o /dev/null -w "%{http_code}" "${CONSOLE_URL}/v3/admin/core/cluster/node/self")
check "V3 admin cluster on Console" "404" "$CODE"

# --- Main Server: V2 console routes should NOT be available ---
echo ""
echo "[Main Server - V2 console routes (should NOT be available)]"

CODE=$(curl -s -o /dev/null -w "%{http_code}" "${MAIN_URL}/nacos/v2/console/namespace/list")
check "V2 console namespace/list on Main" "404" "$CODE"

CODE=$(curl -s -o /dev/null -w "%{http_code}" "${MAIN_URL}/nacos/v2/console/health/liveness")
check "V2 console health/liveness on Main" "404" "$CODE"

CODE=$(curl -s -o /dev/null -w "%{http_code}" "${MAIN_URL}/nacos/v2/console/health/readiness")
check "V2 console health/readiness on Main" "404" "$CODE"

# --- Main Server: V2 Open API routes should be available ---
echo ""
echo "[Main Server - V2 Open API routes (should be available)]"

CODE=$(curl -s -o /dev/null -w "%{http_code}" "${MAIN_URL}/nacos/v2/cs/config")
# 400 = missing params (route exists), 403 = auth required
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

# --- Auth endpoints should be on BOTH servers ---
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

# --- Summary ---
echo ""
echo "============================================="
echo "Results: ${PASS} passed, ${FAIL} failed"
echo "============================================="

if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
