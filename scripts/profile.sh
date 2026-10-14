#!/usr/bin/env bash
# ==============================================================================
# Batata CPU Profiling Driver
#
# Captures a CPU flamegraph of a RUNNING Batata server while applying load,
# so you can see exactly where CPU time is spent (the "measure first" step for
# the high-CPU investigation).
#
# This complements scripts/benchmark.sh: it reuses the same load-generation
# approach (wrk / hey) and attaches an OS-native profiler to the server PID.
#
#   - Linux  : `perf record` + Brendan Gregg's FlameGraph (perf_events)
#   - macOS  : `sample` (built-in) + FlameGraph (no SIP changes needed)
#
# Prerequisites (auto-checked):
#   Linux : perf, and FlameGraph scripts (stackcollapse-perf.pl, flamegraph.pl)
#           git clone https://github.com/brendangregg/FlameGraph /opt/FlameGraph
#   macOS : sample (built-in); FlameGraph scripts for SVG rendering
#   Load  : wrk or hey (see benchmark.sh)
#
# Usage:
#   ./scripts/profile.sh                         # auto-detect PID, full benchmark load, 30s
#   ./scripts/profile.sh --pid 12345             # profile a specific server PID
#   ./scripts/profile.sh --mode naming --dur 60  # only naming load, profile 60s
#   ./scripts/profile.sh --out /tmp/cpu.svg      # custom output path
#   PID=... ./scripts/profile.sh                 # also accepts PID env
#
# Environment (shared with benchmark.sh):
#   SERVER=127.0.0.1:8848   Batata server address (for load generation)
#   MODE=full               Benchmark suite: full|config|naming|consul|quick
#   THREADS=4  CONNECTIONS=100  DURATION=30s
#   FLAMEGRAPH_DIR=/opt/FlameGraph   Where the FlameGraph perl scripts live
# ==============================================================================

set -uo pipefail
cd "$(dirname "$0")/.."

# ---- Configuration ----------------------------------------------------------
DURATION="${DURATION:-30}"
MODE="${MODE:-full}"
OUT="${OUT:-$(pwd)/batata-cpu-$(date +%Y%m%d-%H%M%S).svg}"
FLAMEGRAPH_DIR="${FLAMEGRAPH_DIR:-/opt/FlameGraph}"
ARG_PID="${PID:-}"
ARG_MODE=""
ARG_DUR=""

# ---- Colors / logging (mirrors benchmark.sh) --------------------------------
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; BLUE='\033[0;34m'; BOLD='\033[1m'; NC='\033[0m'
log_info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# ---- Parse args -------------------------------------------------------------
while [[ $# -gt 0 ]]; do
  case "$1" in
    --pid)  ARG_PID="$2"; shift 2 ;;
    --mode) ARG_MODE="$2"; shift 2 ;;
    --dur)  ARG_DUR="$2"; shift 2 ;;
    --out)  OUT="$2"; shift 2 ;;
    -h|--help) sed -n '3,40p' "$0"; exit 0 ;;
    *) log_warn "Unknown arg: $1"; shift ;;
  esac
done
[[ -n "$ARG_PID" ]]  && PID="$ARG_PID"
[[ -n "$ARG_MODE" ]] && MODE="$ARG_MODE"
[[ -n "$ARG_DUR"  ]] && DURATION="$ARG_DUR"

OS="$(uname -s)"
log_info "Platform: $OS | Mode: $MODE | Duration: ${DURATION}s | Output: $OUT"

# ---- Find server PID (if not given) ----------------------------------------
if [[ -z "${PID:-}" ]]; then
  # Prefer an explicit process name; fall back to common binary names.
  PID="$(pgrep -f 'batata' | head -n1)"
  if [[ -z "$PID" ]]; then
    log_error "No Batata server PID found. Pass --pid <PID> or start the server first."
    exit 1
  fi
  log_info "Auto-detected server PID: $PID"
fi
if ! kill -0 "$PID" 2>/dev/null; then
  log_error "PID $PID is not running."
  exit 1
fi

# ---- Prerequisite checks ----------------------------------------------------
if [[ "$OS" == "Linux" ]]; then
  if ! command -v perf &>/dev/null; then
    log_error "perf not found. Install: apt install linux-tools-common linux-tools-generic"
    exit 1
  fi
  for s in stackcollapse-perf.pl flamegraph.pl; do
    if [[ ! -x "$FLAMEGRAPH_DIR/$s" ]]; then
      log_error "Missing $s in $FLAMEGRAPH_DIR. Clone: git clone https://github.com/brendangregg/FlameGraph $FLAMEGRAPH_DIR"
      exit 1
    fi
  done
else
  if ! command -v sample &>/dev/null; then
    log_error "sample not found (required on macOS)."
    exit 1
  fi
  if [[ ! -x "$FLAMEGRAPH_DIR/flamegraph.pl" ]]; then
    log_warn "flamegraph.pl not found in $FLAMEGRAPH_DIR."
    log_warn "Falling back to plain 'sample' TEXT output (no SVG). For SVG flamegraph:"
    log_warn "  git clone https://github.com/brendangregg/FlameGraph $FLAMEGRAPH_DIR"
    FALLBACK_TEXT=1
  fi
fi

# ---- Start load generation in the background --------------------------------
# Reuse the existing benchmark harness so load patterns match production API usage.
if command -v wrk &>/dev/null || command -v hey &>/dev/null; then
  log_info "Starting benchmark load (mode=$MODE) in background..."
  SERVER="${SERVER:-127.0.0.1:8848}" DURATION="${DURATION}s" \
    ./scripts/benchmark.sh "$MODE" >/tmp/batata_profile_bench.log 2>&1 &
  BENCH_PID=$!
else
  log_warn "wrk/hey not installed — profiling with NO applied load (idle profile)."
fi

cleanup() {
  [[ -n "${BENCH_PID:-}" ]] && kill "$BENCH_PID" 2>/dev/null
}
trap cleanup EXIT

# ---- Capture profile --------------------------------------------------------
TMPDIR_PROF="$(mktemp -d /tmp/batata_profXXXXXX)"
if [[ "$OS" == "Linux" ]]; then
  log_info "Recording CPU profile with perf for ${DURATION}s (PID=$PID)..."
  perf record -F 99 -p "$PID" -g -o "$TMPDIR_PROF/perf.data" -- sleep "$DURATION" || {
    log_error "perf record failed (need CAP_SYS_ADMIN / --privileged in containers)."
    exit 1
  }
  perf script -i "$TMPDIR_PROF/perf.data" \
    | "$FLAMEGRAPH_DIR/stackcollapse-perf.pl" \
    | "$FLAMEGRAPH_DIR/flamegraph.pl" --title "Batata CPU ($MODE, ${DURATION}s)" > "$OUT"
else
  log_info "Recording CPU profile with sample for ${DURATION}s (PID=$PID)..."
  sample "$PID" "$DURATION" -f "$TMPDIR_PROF/sample.txt" -mayDie || {
    log_error "sample failed."
    exit 1
  }
  # Prefer SVG when FlameGraph is available; otherwise emit readable sample text
  # (look at the 'Self' / 'Total' columns to find the hottest functions).
  if [[ -n "${FALLBACK_TEXT:-}" ]]; then
    OUT_TXT="${OUT%.svg}.txt"
    cp "$TMPDIR_PROF/sample.txt" "$OUT_TXT"
    log_info "Plain sample text written: $OUT_TXT"
    log_info "No FlameGraph repo -> no SVG. For SVG: (a) clone FlameGraph, or"
    log_info "(b) run the server with --features pprof and curl /debug/pprof/flamegraph."
  else
    if [[ -x "$FLAMEGRAPH_DIR/stackcollapse-sample.pl" ]]; then
      COLLAPSE="$FLAMEGRAPH_DIR/stackcollapse-sample.pl"
    else
      COLLAPSE="cat"   # older FlameGraph: flamegraph.pl accepts sample output directly
    fi
    "$COLLAPSE" "$TMPDIR_PROF/sample.txt" \
      | "$FLAMEGRAPH_DIR/flamegraph.pl" --title "Batata CPU ($MODE, ${DURATION}s)" > "$OUT"
  fi
fi

if [[ -n "${FALLBACK_TEXT:-}" ]]; then
  if [[ -s "${OUT_TXT:-}" ]]; then
    log_info "Done. Plain sample text: $OUT_TXT"
    exit 0
  else
    log_error "sample text output missing. Inspect $TMPDIR_PROF."
    exit 1
  fi
fi
if [[ -s "$OUT" ]]; then
  log_info "Flamegraph written: $OUT"
  log_info "Open it in a browser to see where CPU is spent."
else
  log_error "Flamegraph generation failed. Inspect $TMPDIR_PROF."
  exit 1
fi
