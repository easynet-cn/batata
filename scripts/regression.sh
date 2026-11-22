#!/usr/bin/env bash
# =============================================================================
# Unified regression gate for Batata.
#
# One command replaces the handful of manual invocations that used to make up
# "run everything": fmt + workspace check + crate unit tests, the live-server
# SDK suites, the full standalone/cluster x store matrix, and (optionally) the
# performance anchors.
#
# Every stage's failures are collected, not fatal: the gate runs to completion
# and then decides. A failure is either
#   - whitelisted  -> recorded as known (see scripts/regression_whitelist.txt)
#   - or a blocker -> exit code 1
# so "known bad" never silently becomes "everything is green", and a new
# failure is never absorbed into the existing noise.
#
# Usage:
#   ./scripts/regression.sh                 # quick (default): pre-commit check
#   ./scripts/regression.sh --standard      # + all core crates + java/go SDK
#   ./scripts/regression.sh --full          # + 6-combo SDK matrix
#   ./scripts/regression.sh --perf          # + performance anchors (advisory)
#   ./scripts/regression.sh --perf-gate     # ... and let a perf drop block
#   ./scripts/regression.sh --perf-capture  # record this machine's anchors
#   ./scripts/regression.sh --only unit     # single stage
#   ./scripts/regression.sh --list          # show stages per level
#
# Levels:
#   quick     fmt, check, unit(core), sim, integration(standalone, rust SDK)
#   standard  + unit(all crates), integration(java + go SDK)
#   full      + matrix (standalone/cluster x rock/mysql/postgres)
#
# The matrix stage additionally asserts Raft health: zero steady-state
# elections and zero apply/client-write timeouts (scripts/raft_health_check.py).
# A missing telemetry liveness marker fails closed rather than passing silently.
# The check stage runs six script gates' self-tests plus a static sim-stage
# wiring guard so the classifiers, the canonical SDK port list, the perf
# observability helpers, the leader attribution parser, the dual-path split
# and the protocol-level simulation stage cannot silently drift. The perf
# dispersion criterion (perf_dispersion.py) rejects a median whose recorded
# rounds do not describe one population, perf_leader.py says which Raft leader
# served each round, and perf_paths.py reports a median per write path.
#
# The perf stage is now attributable: host load/free-disk snapshots are recorded
# before and after each case, every (case, variant, round, node) keeps its own
# size-capped server log, and cluster perf records a Raft health summary as
# diagnostic context (the matrix stage stays the authoritative health gate).
# See perf_observability.py and perf_leader.py.
#
# Reports are written to reports/regression/<timestamp>/ (gitignored) with a
# stable symlink at reports/regression/latest. Never /tmp: it gets cleaned.
#
# Environment:
#   SKIP_FMT=1 SKIP_CHECK=1   skip a stage
#   REGRESSION_CRATES="a b"   run only these crates in the unit stage (overrides
#                             the level's crate set; useful to re-check one
#                             crate without paying for the whole workspace)
#   SDK_MODE=standalone       backend mode for the integration stage
#   REGRESSION_KEEP_SERVER=1  leave the server running after integration
#   PERF_MACHINE_ID=...       override the anchor key (default: hw.model+ncores)
#   PERF_STORAGE=rocks        anchor label: rocks = embedded (RocksDB)
#   PERF_STORAGE=mysql        anchor label + real MySQL backend (ExternalDb)
#   PERF_STORAGE=postgres     anchor label + real PostgreSQL backend (ExternalDb)
#                              (mysql/postgres start the server with --db-url and
#                               reset a dedicated perf DB each run)
#   PERF_MODE=standalone      standalone | cluster
#   PERF_WRITE_NODE=0         cluster node that receives all wrk load; 0 = auto
#                             (1 for standalone, 2 for cluster). The Raft
#                             bootstrap leader is deterministically node 1, so
#                             cluster load on node 2 keeps the forwarded-path
#                             Config Write anchor meaningful. Set 1 explicitly
#                             to measure the leader-local path instead.
#   PERF_ROUNDS=3 PERF_DURATION=20s PERF_WARMUP=10s
#   PERF_CONNECTIONS=20 PERF_THREADS=2 PERF_WORKERS=8
#   PERF_TOLERANCE=0.08       minimum tolerance when cv_pct is small
#   PERF_MAX_SPREAD_PCT=20    max (max-min)/median spread before a scenario is
#                             INCONCLUSIVE (limit also scales with anchor cv)
#   PERF_SPREAD_FACTOR=3      limit = max(PERF_MAX_SPREAD_PCT, factor*anchor cv)
#   PERF_SERVER_LOG_MAX_BYTES=4194304
#                             cap per server log (head + health signals + tail)
#   PERF_BASELINE=<file>      baseline to read/write (for experiments)
# =============================================================================

set -uo pipefail
cd "$(dirname "$0")/.."

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

WHITELIST="${SCRIPT_DIR}/regression_whitelist.txt"
PERF_BASELINE="${PERF_BASELINE:-${SCRIPT_DIR}/regression_perf_baseline.txt}"
MATRIX_BASELINE="${SCRIPT_DIR}/regression_matrix_baseline.txt"

# Minimum fraction of the anchor a scenario may regress before it is reported.
# The real tolerance is max(anchor cv_pct, PERF_TOLERANCE): cv_pct is the
# spread that machine showed when the anchor was captured, and on a busy
# developer box that is far larger than 8%. A machine that cannot resolve
# better than 40% must not fail a run for a 20% drop.
PERF_TOLERANCE="${PERF_TOLERANCE:-0.08}"
# A median over rounds that disagree by more than this is not a measurement of
# one code path. Cluster Config Write has two populations: when node 1 (the only
# node taking load) is the Raft leader the write is local, otherwise every write
# is forwarded over gRPC and the follower waits for its apply index to catch up.
# Never compare such a median to an anchor, and never capture it as one.
PERF_MAX_SPREAD_PCT="${PERF_MAX_SPREAD_PCT:-20}"
PERF_SPREAD_FACTOR="${PERF_SPREAD_FACTOR:-3}"
PERF_STORAGE="${PERF_STORAGE:-rocks}"
PERF_MODE="${PERF_MODE:-standalone}"
PERF_ROUNDS="${PERF_ROUNDS:-3}"
PERF_WARMUP_ROUNDS="${PERF_WARMUP_ROUNDS:-1}"
PERF_DURATION="${PERF_DURATION:-20s}"
PERF_WARMUP="${PERF_WARMUP:-10s}"
PERF_CONNECTIONS="${PERF_CONNECTIONS:-20}"
PERF_THREADS="${PERF_THREADS:-2}"
PERF_WORKERS="${PERF_WORKERS:-8}"
PERF_WRITE_NODE="${PERF_WRITE_NODE:-0}"
# Bound each retained server log. The default INFO level emits two lines per
# config write; an uncapped cluster node reached 66 MiB in one perf run.
PERF_SERVER_LOG_MAX_BYTES="${PERF_SERVER_LOG_MAX_BYTES:-4194304}"
PERF_MACHINE_ID="${PERF_MACHINE_ID:-}"
PERF_CASES=(naming config)

# Crates run at every level. These are the ones whose failure means something
# changed in a hot path; the rest are covered at --standard.
CORE_CRATES=(batata-core batata-client batata-config batata-naming batata-plugin-consul)
# `batata-server` owns the AI route tests (prompt, skill, MCP, agent, AgentSpec,
# resource search). They used to be `#[ignore]`d behind a live database, which
# also kept them out of the gate; they now run on the embedded backend, so the
# crate has to be here or 51 of them are nobody's responsibility.
STANDARD_CRATES=(batata-api batata-auth batata-persistence batata-consistency batata-server-common batata-console batata-ai batata-server)
FULL_CRATES=(batata-plugin-apollo)

LEVEL=quick
RUN_PERF=0
PERF_GATE=0
PERF_CAPTURE=0
ONLY=""
LIST_ONLY=0
while [ $# -gt 0 ]; do
    case "$1" in
        --quick)    LEVEL=quick ;;
        --standard) LEVEL=standard ;;
        --full)     LEVEL=full ;;
        --perf)     RUN_PERF=1 ;;
        --perf-gate) RUN_PERF=1; PERF_GATE=1 ;;
        --perf-capture) RUN_PERF=1; PERF_CAPTURE=1 ;;
        --only)     ONLY="${2:-}"; shift ;;
        --list)     LIST_ONLY=1 ;;
        -h|--help)  LIST_ONLY=1 ;;
        *)          echo "unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

if [ "$LIST_ONLY" = 1 ]; then
    cat <<'EOF'
stages per level
  quick     fmt check unit sim integration
  standard  fmt check unit sim integration
  full      fmt check unit sim integration matrix
  --perf    perf (additive, any level; advisory unless --perf-gate)
  --perf-capture  record this machine's anchors instead of comparing
  --only <stage> runs a single stage
EOF
    exit 0
fi

# ---------------------------------------------------------------------------
# Report plumbing
# ---------------------------------------------------------------------------
RUN_ID="$(date +%Y%m%d-%H%M%S)"
REPORT_DIR="${ROOT}/reports/regression/${RUN_ID}"
LOG_DIR="${REPORT_DIR}/logs"
mkdir -p "$LOG_DIR"
REPORT="${REPORT_DIR}/report.md"

BLOCKERS=()
WHITELISTED=()
PERF_ADVISORY=()
PERF_INCONCLUSIVE=0
STAGE_ROWS=()
TOTAL_PASSED=0
TOTAL_FAILED=0

GATE_START=$(date +%s)

report() { printf '%s\n' "$*" >> "$REPORT"; }

report_header() {
    local commit dirty
    commit=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
    dirty=""
    [ -n "$(git status --porcelain 2>/dev/null)" ] && dirty=" (dirty)"
    {
        echo "# Batata regression gate — ${RUN_ID}"
        echo
        echo "- Level: \`${LEVEL}\`$([ "$RUN_PERF" = 1 ] && echo ' + perf')"
        echo "- Commit: \`${commit}\`${dirty}"
        echo "- Host: $(uname -s) $(uname -m)"
        echo "- Started: $(date '+%Y-%m-%d %H:%M:%S')"
        echo
    } >> "$REPORT"
}

secs() { echo $(( $(date +%s) - $1 )); }
human() {
    local s=$1
    printf '%dm%02ds' $(( s / 60 )) $(( s % 60 ))
}

# whitelist_match <stage> <failure-item> -> echoes the reason, or nothing.
whitelist_match() {
    local stage="$1" item="$2"
    [ -f "$WHITELIST" ] || return 0
    local s p r
    while IFS='|' read -r s p r; do
        case "${s:-}" in ''|\#*) continue ;; esac
        [ "$s" = "$stage" ] || continue
        case "$item" in *"$p"*) echo "${r:-whitelisted}"; return 0 ;; esac
    done < "$WHITELIST"
    echo ""
}

record_failure() {
    local stage="$1" item="$2"
    local reason
    reason=$(whitelist_match "$stage" "$item")
    if [ -n "$reason" ]; then
        WHITELISTED+=("${stage}|${item}|${reason}")
    else
        BLOCKERS+=("${stage}|${item}")
    fi
}

# stage_end <name> <rc> <detail>
stage_end() {
    local name="$1" rc="$2" detail="$3" elapsed
    elapsed=$(secs "$STAGE_START")
    local status="PASS"
    [ "$rc" -ne 0 ] && status="FAIL"
    STAGE_ROWS+=("${name}|${status}|${detail}|$(human "$elapsed")")
    printf '  [%s] %-12s %s (%s)\n' "$status" "$name" "$detail" "$(human "$elapsed")"
}

should_run() { [ -z "$ONLY" ] || [ "$ONLY" = "$1" ]; }

# ---------------------------------------------------------------------------
# Stage: fmt
# ---------------------------------------------------------------------------
stage_fmt() {
    should_run fmt || return 0
    [ "${SKIP_FMT:-}" = "1" ] && { echo "  [SKIP] fmt"; return 0; }
    STAGE_START=$(date +%s)
    local log="$LOG_DIR/fmt.log"
    cargo fmt --all -- --check > "$log" 2>&1
    local rc=$?
    if [ $rc -ne 0 ]; then
        record_failure fmt "rustfmt drift"
    fi
    stage_end fmt $rc "$( [ $rc -eq 0 ] && echo 'clean' || echo 'rustfmt --check failed' )"
}

# ---------------------------------------------------------------------------
# Stage: check (workspace wide, all targets)
#
# sdk-tests/* are separate workspaces, so `cargo check --workspace` never
# compiles them — and a broken SDK test crate then shows up only as an
# integration suite that "fails" without ever running. Check them explicitly.
# ---------------------------------------------------------------------------
SDK_TEST_DIRS=(sdk-tests/batata-client-tests sdk-tests/batata-consul-client-tests sdk-tests/batata-maintainer-tests)

check_sdk_tests() {
    local dir log rc=0
    for dir in "${SDK_TEST_DIRS[@]}"; do
        [ -d "${ROOT}/${dir}" ] || continue
        log="$LOG_DIR/check-$(basename "$dir").log"
        (cd "${ROOT}/${dir}" && cargo check --all-targets) > "$log" 2>&1 || rc=1
        if [ -f "$log" ]; then
            while IFS= read -r line; do
                [ -n "$line" ] && record_failure check "${dir}: ${line}"
            done < <(grep -oE '^error(\[E[0-9]+\])?: .*' "$log" | head -10)
        fi
    done
    return $rc
}

stage_check() {
    should_run check || return 0
    [ "${SKIP_CHECK:-}" = "1" ] && { echo "  [SKIP] check"; return 0; }
    STAGE_START=$(date +%s)
    local log="$LOG_DIR/check.log"
    cargo check --workspace --all-targets > "$log" 2>&1
    local rc=$?
    # `grep -c` prints 0 *and* exits 1 when there is no match, so `|| echo 0`
    # would append a second 0. Default the empty/ unsuccessful case instead.
    local errors warnings detail
    errors=$(grep -c '^error' "$log" 2>/dev/null); errors=${errors:-0}
    warnings=$(grep -c '^warning' "$log" 2>/dev/null); warnings=${warnings:-0}
    if [ $rc -ne 0 ]; then
        # A broken build has no useful per-test attribution; attribute the crate.
        while IFS= read -r line; do
            record_failure check "$line"
        done < <(grep -oE '^error(\[E[0-9]+\])?: .*' "$log" | head -20)
        # If it failed without a parseable error line, still block.
        [ "${#BLOCKERS[@]}" -eq 0 ] && record_failure check "workspace check failed (rc=$rc)"
    fi
    local sdk_rc=0
    check_sdk_tests || sdk_rc=1
    [ "$sdk_rc" -ne 0 ] && rc=1
    detail="${errors} errors, ${warnings} warnings"
    [ "$sdk_rc" -ne 0 ] && detail="${detail}; sdk-tests failed to compile"

    # Script-level gates must be self-validating. A drifted SDK port list, a
    # weakened Raft anomaly classifier, a disabled perf dispersion criterion
    # or a lost leader attribution would otherwise stay invisible until a full
    # matrix run (or forever, if the matrix log happens to be clean).
    local guard_rc=0
    if ! bash "${SCRIPT_DIR}/sdk_port_guard.sh" self-test \
        > "$LOG_DIR/check-port-guard.log" 2>&1; then
        record_failure check "sdk_port_guard self-test failed"
        guard_rc=1
    fi
    if ! python3 "${SCRIPT_DIR}/raft_health_check.py" --self-test \
        > "$LOG_DIR/check-raft-health.log" 2>&1; then
        record_failure check "raft_health_check self-test failed"
        guard_rc=1
    fi
    if ! python3 "${SCRIPT_DIR}/perf_observability.py" --self-test \
        > "$LOG_DIR/check-perf-observability.log" 2>&1; then
        record_failure check "perf_observability self-test failed"
        guard_rc=1
    fi
    if ! python3 "${SCRIPT_DIR}/perf_dispersion.py" --self-test \
        > "$LOG_DIR/check-perf-dispersion.log" 2>&1; then
        record_failure check "perf_dispersion self-test failed"
        guard_rc=1
    fi
    if ! python3 "${SCRIPT_DIR}/perf_leader.py" --self-test \
        > "$LOG_DIR/check-perf-leader.log" 2>&1; then
        record_failure check "perf_leader self-test failed"
        guard_rc=1
    fi
    if ! python3 "${SCRIPT_DIR}/perf_paths.py" --self-test \
        > "$LOG_DIR/check-perf-paths.log" 2>&1; then
        record_failure check "perf_paths self-test failed"
        guard_rc=1
    fi
    # Guard the sim-stage wiring. Checking the declaration line (not the whole
    # file) keeps this guard's own text from satisfying the check.
    local sim_line sim_wiring_ok=1
    sim_line=$(grep '^SIM_TESTS=' "${SCRIPT_DIR}/regression.sh" || true)
    case "$sim_line" in
        *deterministic_raft_simulation*openraft_storage_conformance*config_cluster_ordered_pairs*) ;;
        *) sim_wiring_ok=0 ;;
    esac
    grep -q '^stage_sim() {' "${SCRIPT_DIR}/regression.sh" || sim_wiring_ok=0
    grep -q '^    stage_sim$' "${SCRIPT_DIR}/regression.sh" || sim_wiring_ok=0
    if [ "$sim_wiring_ok" -eq 1 ]; then
        echo "SELF-TEST PASS: sim stage wired with all three suites" \
            > "$LOG_DIR/check-sim-wiring.log"
    else
        echo "SELF-TEST FAIL: sim stage wiring incomplete" \
            > "$LOG_DIR/check-sim-wiring.log"
        record_failure check "sim stage wiring missing or incomplete"
        guard_rc=1
    fi
    # Guard the wiring itself: if a future edit drops one of the six
    # self-tests or the sim wiring guard, the stage must not pass merely
    # because nothing failed. Each prints an explicit PASS marker, so a
    # missing/empty artifact is a red gate. The perf helpers also check the
    # perf-stage flags statically.
    if ! grep -q 'PASS' "$LOG_DIR/check-port-guard.log" 2>/dev/null \
        || ! grep -q 'passed' "$LOG_DIR/check-raft-health.log" 2>/dev/null \
        || ! grep -q 'SELF-TEST PASS' "$LOG_DIR/check-perf-observability.log" 2>/dev/null \
        || ! grep -q 'SELF-TEST PASS' "$LOG_DIR/check-perf-dispersion.log" 2>/dev/null \
        || ! grep -q 'SELF-TEST PASS' "$LOG_DIR/check-perf-leader.log" 2>/dev/null \
        || ! grep -q 'SELF-TEST PASS' "$LOG_DIR/check-perf-paths.log" 2>/dev/null \
        || ! grep -q 'SELF-TEST PASS' "$LOG_DIR/check-sim-wiring.log" 2>/dev/null; then
        record_failure check "script self-test artifacts missing or incomplete"
        guard_rc=1
    fi
    [ "$guard_rc" -ne 0 ] && rc=1
    [ "$guard_rc" -ne 0 ] && detail="${detail}; script self-tests failed"
    stage_end check $rc "$detail"
}

# ---------------------------------------------------------------------------
# Stage: unit (per-crate cargo test)
#
# --no-fail-fast matters: without it cargo stops after the first failing test
# target, so one broken integration binary hides every suite behind it.
# ---------------------------------------------------------------------------
unit_crates() {
    if [ -n "${REGRESSION_CRATES:-}" ]; then
        echo "${REGRESSION_CRATES}"
        return
    fi
    echo "${CORE_CRATES[@]}"
    case "$LEVEL" in
        standard|full) printf ' %s' "${STANDARD_CRATES[@]}"; echo ;;
        *) : ;;
    esac
    [ "$LEVEL" = full ] && printf ' %s' "${FULL_CRATES[@]}"
    echo ""
}

run_unit_crate() {
    local crate="$1"
    local log="$LOG_DIR/unit-${crate}.log"
    cargo test -p "$crate" --no-fail-fast > "$log" 2>&1
    local rc=$?

    local passed failed
    passed=$(awk '/^test result:/{for(i=1;i<=NF;i++) if($i=="passed;") s+=$(i-1)} END{print s+0}' "$log")
    failed=$(awk '/^test result:/{for(i=1;i<=NF;i++) if($i=="failed;") s+=$(i-1)} END{print s+0}' "$log")
    TOTAL_PASSED=$(( TOTAL_PASSED + passed ))
    TOTAL_FAILED=$(( TOTAL_FAILED + failed ))

    if [ "$rc" -ne 0 ]; then
        local names
        names=$(awk '
            /^failures:$/        { inlist=1; next }
            /^test result:/      { inlist=0; next }
            inlist && /^    [A-Za-z0-9_:]+$/ { gsub(/^ +/,""); print }
        ' "$log" | sort -u)
        if [ -z "$names" ]; then
            # Compile error or a panic with no test attribution.
            record_failure unit "${crate}: did not run (rc=${rc})"
        else
            while IFS= read -r n; do
                [ -n "$n" ] && record_failure unit "${crate}::${n}"
            done <<< "$names"
        fi
    fi
    printf '    %-28s %5s passed %3s failed\n' "$crate" "$passed" "$failed"
    return $rc
}

stage_unit() {
    should_run unit || return 0
    STAGE_START=$(date +%s)

    # batata-console's deployment test spawns real core + console processes, so
    # the binary has to exist before the crate is exercised. `cargo build` is
    # incremental, so this is a no-op when the binary is already current.
    ensure_server_binary || record_failure unit "batata-server failed to build"

    local crates rc=0
    crates=$(unit_crates | tr ' ' '\n' | grep -v '^$' | sort -u)
    while IFS= read -r crate; do
        [ -n "$crate" ] || continue
        run_unit_crate "$crate" || rc=1
    done <<< "$crates"
    stage_end unit $rc "${TOTAL_PASSED} passed, ${TOTAL_FAILED} failed"
}

# ---------------------------------------------------------------------------
# Stage: sim (deterministic Raft simulation + storage conformance)
#
# Pure Rust test binaries: no server, no SDK, no ports. They check the consensus
# contract itself (openraft's own storage suite, deterministic fault injection,
# linearizability). A dedicated stage gives a protocol-level regression its own
# name and log instead of burying it in `unit|batata-consistency`.
# ---------------------------------------------------------------------------
SIM_TESTS=(deterministic_raft_simulation openraft_storage_conformance config_cluster_ordered_pairs)

stage_sim() {
    should_run sim || return 0
    STAGE_START=$(date +%s)
    local rc=0 test log test_rc passed failed
    for test in "${SIM_TESTS[@]}"; do
        log="$LOG_DIR/sim-${test}.log"
        cargo test -p batata-consistency --test "$test" --no-fail-fast > "$log" 2>&1
        test_rc=$?
        passed=$(awk '/^test result:/{for(i=1;i<=NF;i++) if($i=="passed;") s+=$(i-1)} END{print s+0}' "$log")
        failed=$(awk '/^test result:/{for(i=1;i<=NF;i++) if($i=="failed;") s+=$(i-1)} END{print s+0}' "$log")
        # Fail closed on an empty or truncated artifact: a green rc with no
        # executed tests is a false green, not a pass.
        if [ "$test_rc" -ne 0 ] || [ "$passed" -eq 0 ] || [ "$failed" -ne 0 ] \
            || ! grep -q '^test result: ok\.' "$log"; then
            record_failure sim "${test}: ${passed} passed, ${failed} failed (rc=${test_rc})"
            rc=1
        fi
        printf '    %-38s %5s passed %3s failed\n' "$test" "$passed" "$failed"
    done
    stage_end sim $rc "${#SIM_TESTS[@]} simulation/conformance suites"
}

# ---------------------------------------------------------------------------
# Stage: integration (live server + SDK suites)
#
# Delegates to test.sh, which owns server startup/teardown and the per-SDK
# runners. We only add: a log per suite, parsing, and whitelist routing.
# ---------------------------------------------------------------------------
# parse_sdk_failures <log> -> one failure name per line.
#
# Three runners feed the SDK stages, each with its own failure dialect:
#   go      "--- FAIL: TestName"
#   cargo   a "failures:" block of indented test paths, then "test result:"
#   maven   "[ERROR] Failures:" then "[ERROR]   Class.method:line msg"
# One parser for all three, so a failing suite always yields named items
# instead of a single opaque "suite failed" blocker.
parse_sdk_failures() {
    awk '
        # The matrix summary emits one of these per failing combination.
        /^\[MATRIX\] FAIL /          { print $3; next }
        /^--- FAIL: /                  { n=$3; sub(/\(.*/,"",n); print n; next }
        # The suite runners pipe cargo through `grep -E "^test result:|panicked"`,
        # so the `failures:` block is often NOT in the log — the panic line is
        # all we get. Take the test name from the  thread NAME (..) panicked
        # line instead.
        # Deliberately quote-free: this awk program lives inside a shell
        # single-quoted string, so an apostrophe would terminate it early. The
        # name is the token between "thread " and the next space.
        /^thread .* panicked at / {
            s=$0; sub(/^thread /,"",s); i=index(s," ");
            if (i > 2) print substr(s, 2, i - 3); next }
        /^failures:$/                  { inr=1; next }
        /^test result:/                { inr=0; next }
        inr && /^[[:space:]]+[A-Za-z0-9_:]+[[:space:]]*$/ {
            s=$0; gsub(/^[[:space:]]+|[[:space:]]+$/,"",s); print s; next }
        /^\[ERROR\] Failures:/         { inm=1; next }
        /^\[ERROR\] Tests run:/        { inm=0; next }
        inm && /^\[ERROR\][[:space:]]+[A-Za-z0-9_.:]+/ {
            s=$0; sub(/^\[ERROR\][[:space:]]+/,"",s); sub(/:.*/,"",s); print s; next }
    ' "$1" | grep -v '^$' | sort -u
}

run_sdk_suite() {
    local suite="$1"
    local log="$LOG_DIR/integration-${suite}.log"
    local t0
    t0=$(date +%s)
    SDK_MODE="${SDK_MODE:-standalone}" \
        bash "${SCRIPT_DIR}/test.sh" sdk --quick "$suite" > "$log" 2>&1
    local rc=$?

    if [ $rc -ne 0 ]; then
        local items
        items=$(parse_sdk_failures "$log")
        if [ -z "$items" ]; then
            record_failure integration "${suite}: suite failed (rc=${rc})"
        else
            while IFS= read -r item; do
                [ -n "$item" ] && record_failure integration "${suite}: ${item}"
            done <<< "$items"
        fi
    fi
    printf '    %-12s rc=%s (%s)\n' "$suite" "$rc" "$(human "$(secs "$t0")")"
    return $rc
}

# The integration stage needs a real server. `cargo check` (the previous
# stage) does not produce one, and a missing binary makes startup.sh fall back
# to a silent 25-minute release build — or, worse, run the suites against
# nothing, which shows up as a pile of login failures.
ensure_server_binary() {
    # The integration stage launches target/debug/batata-server directly.
    # `cargo check` never relinks that executable, so an early "file exists"
    # return would run every SDK suite against a stale binary after any source
    # change (and an existing release binary does not help: test.sh sdk
    # --quick uses the debug one unless RELEASE=1). Always let cargo decide;
    # the build is incremental and a no-op when nothing changed.
    echo "    building batata-server (debug)..."
    cargo build -p batata-server > "$LOG_DIR/build-server.log" 2>&1
}

stage_integration() {
    should_run integration || return 0
    STAGE_START=$(date +%s)
    local suites="rust"
    case "$LEVEL" in
        standard|full) suites="rust nacos consul" ;;
    esac

    ensure_server_binary || record_failure integration "batata-server failed to build"

    # Standalone defaults to ~/batata/data, whose macOS provenance attribute
    # makes RocksDB fail with EPERM on open. startup.sh forwards SDK_DATA_DIR
    # as a --batata.persistence.embedded.data_dir override (the BATATA_* env
    # convention cannot express that key: every `_` maps to a `.`).
    export SDK_DATA_DIR="${REPORT_DIR}/data"
    mkdir -p "${SDK_DATA_DIR}"

    local rc=0 s
    for s in $suites; do
        run_sdk_suite "$s" || rc=1
    done
    [ "${REGRESSION_KEEP_SERVER:-}" != "1" ] && pkill -f batata-server 2>/dev/null
    stage_end integration $rc "suites: ${suites}"
}

# ---------------------------------------------------------------------------
# Stage: matrix (standalone/cluster x rock/mysql/postgres), --full only
# ---------------------------------------------------------------------------
stage_matrix() {
    should_run matrix || return 0
    [ "$LEVEL" = full ] || return 0
    STAGE_START=$(date +%s)
    local log="$LOG_DIR/matrix.log"
    bash "${SCRIPT_DIR}/test.sh" sdk --matrix > "$log" 2>&1
    local rc=$?
    if [ $rc -ne 0 ]; then
        local items
        items=$(parse_sdk_failures "$log")
        [ -z "$items" ] && record_failure matrix "matrix failed (rc=${rc})"
        while IFS= read -r item; do
            [ -n "$item" ] && record_failure matrix "$item"
        done <<< "$items"
    fi
    pkill -f batata-server 2>/dev/null

    # Coverage check against scripts/regression_matrix_baseline.txt. rc only
    # tells us nothing crashed; it cannot tell that a suite silently ran 40
    # cases instead of 539 because Maven skipped a module. Anchoring the case
    # count per (combination, suite) is what makes "the DB backends are covered"
    # a claim the gate can actually verify.
    local tsv="${ROOT}/reports/sdk/latest/results.tsv"
    if [ ! -f "$MATRIX_BASELINE" ]; then
        echo "    missing $MATRIX_BASELINE — record anchors after a green run"
        stage_end matrix $rc "6 combinations (no coverage baseline)"
        return 0
    fi
    if [ ! -s "$tsv" ]; then
        record_failure matrix "no results.tsv at reports/sdk/latest — coverage unknown"
        stage_end matrix 1 "6 combinations (no results)"
        return 0
    fi
    local combo suite total passed have missing=0 shrunk=0
    while IFS='|' read -r combo suite total; do
        [ -n "${combo:-}" ] || continue
        case "$combo" in \#*) continue ;; esac
        have=$(awk -F'|' -v c="$combo" -v s="$suite" '$1==c && $2==s {print $3; exit}' "$tsv")
        if [ -z "$have" ]; then
            record_failure matrix "${combo}/${suite}: missing from the run (baseline ${total} cases)"
            missing=$((missing + 1))
            continue
        fi
        if [ "$have" -lt "$total" ]; then
            record_failure matrix "${combo}/${suite}: ${have} cases < baseline ${total}"
            shrunk=$((shrunk + 1))
        fi
    done < <(awk -F'|' '/^#/ {next} NF>=3 {print $1"|"$2"|"$3}' "$MATRIX_BASELINE")
    if [ "$missing" -gt 0 ] || [ "$shrunk" -gt 0 ]; then
        echo "    coverage: ${missing} missing, ${shrunk} shrunk"
        rc=1
    else
        echo "    coverage: every anchored (combination, suite) ran its full case count"
    fi

    # Raft health gate. matrix.log aggregates every node's WARN lines for all
    # six combinations (test.sh scan_logs), while the node stdout logs carry
    # the INFO-level telemetry liveness marker. A run where telemetry never
    # started is a false green, so the checker fails closed in that case.
    local health_args=("$log")
    local node_dir
    for node_dir in "${ROOT}/logs/sdk-node1" "${ROOT}/logs/sdk-node2" "${ROOT}/logs/sdk-node3"; do
        [ -f "${node_dir}/stdout.log" ] && health_args+=("${node_dir}/stdout.log")
    done
    local health_summary
    if health_summary=$(python3 "${SCRIPT_DIR}/raft_health_check.py" "${health_args[@]}" 2>&1); then
        echo "    raft health: ${health_summary}"
    else
        record_failure matrix "raft health: ${health_summary}"
        rc=1
    fi
    stage_end matrix $rc "6 combinations"
}

# ---------------------------------------------------------------------------
# Stage: perf (release; anchors in scripts/regression_perf_baseline.txt)
#
# Anchors are keyed by <machine_id>|<storage>|<mode>|<case>. An absolute RPS
# number only means something on the box that produced it, so a new machine
# has no anchors at all: the stage reports that and stops, instead of failing
# against another machine's numbers.
#
# The stage is advisory by default - it is the only stage whose result depends
# on how busy the host is. --perf-gate promotes a drop to a blocker.
# ---------------------------------------------------------------------------
# Raw-sample arithmetic (median, spread and the INCONCLUSIVE criterion) lives
# in perf_dispersion.py so it can be unit-tested and mutant-checked.
#
# Stable id for the machine that produced an anchor: lowercased hw.model with
# punctuation stripped plus the core count, e.g. `macbookpro161-16t`.
perf_machine_id() {
    if [ -n "$PERF_MACHINE_ID" ]; then printf '%s' "$PERF_MACHINE_ID"; return 0; fi
    local model ncpu
    model=$(sysctl -n hw.model 2>/dev/null || echo "unknown")
    ncpu=$(sysctl -n hw.ncpu 2>/dev/null || getconf _NPROCESSORS_ONLN 2>/dev/null || echo 0)
    model=$(printf '%s' "$model" | tr '[:upper:]' '[:lower:]' | tr -cd 'a-z0-9')
    printf '%s-%st' "${model:-unknown}" "$ncpu"
}

# perf_anchor <machine> <case> -> "median|cv_pct", empty when absent.
perf_anchor() {
    [ -f "$PERF_BASELINE" ] || return 0
    awk -F'|' -v m="$1" -v c="$2" -v s="$PERF_STORAGE" -v mo="$PERF_MODE" \
        '$1==m && $2==s && $3==mo && $4==c {print $5"|"$6; exit}' "$PERF_BASELINE"
}

# perf_capture_write <machine> <case> <median> <cv_pct>
perf_capture_write() {
    local machine="$1" case_name="$2" med="$3" cv="$4"
    [ -f "$PERF_BASELINE" ] || \
        printf '%s\n' "# machine_id|storage|mode|case|median_rps|cv_pct|captured|bench_params" > "$PERF_BASELINE"
    local tmp="${PERF_BASELINE}.tmp" params
    params="c${PERF_CONNECTIONS}-t${PERF_THREADS}-w${PERF_WORKERS}-d${PERF_DURATION}-r${PERF_ROUNDS}"
    awk -F'|' -v OFS='|' -v m="$machine" -v s="$PERF_STORAGE" -v mo="$PERF_MODE" -v c="$case_name" \
        '!($1==m && $2==s && $3==mo && $4==c)' "$PERF_BASELINE" > "$tmp"
    printf '%s|%s|%s|%s|%.0f|%.1f|%s|%s\n' \
        "$machine" "$PERF_STORAGE" "$PERF_MODE" "$case_name" "$med" "$cv" \
        "$(date +%Y-%m-%d)" "$params" >> "$tmp"
    mv "$tmp" "$PERF_BASELINE"
}

# A perf finding only blocks under --perf-gate; otherwise it is advisory.
perf_fail() {
    if [ "$PERF_GATE" = 1 ]; then
        record_failure perf "$1"
        echo "    [BLOCKER] $1"
        return 1
    fi
    PERF_ADVISORY+=("$1")
    echo "    [advisory] $1"
    return 0
}

stage_perf() {
    should_run perf || return 0
    [ "$RUN_PERF" = 1 ] || return 0
    STAGE_START=$(date +%s)

    local bin="${ROOT}/target/release/batata-server"
    if [ ! -x "$bin" ]; then
        echo "    release binary missing; build with 'cargo build --release -p batata-server'"
        stage_end perf 0 "SKIPPED (no release binary)"
        return 0
    fi

    local MACHINE
    MACHINE=$(perf_machine_id)

    # Resolve the load target. The cluster bootstrap leader is node 1, so
    # aiming wrk at node 1 would always measure the leader-local write path.
    # The `rocks|cluster|Config Write` anchor (2735) was captured on the
    # forwarded path; default cluster runs to a follower so the existing
    # anchor still describes the code path being compared. Override with
    # PERF_WRITE_NODE=1 to measure the local path deliberately.
    local write_node="$PERF_WRITE_NODE"
    if [ "$write_node" = "0" ]; then
        if [ "$PERF_MODE" = "cluster" ]; then
            write_node=2
        else
            write_node=1
        fi
    fi
    echo "    load target: node ${write_node} (PERF_WRITE_NODE=${PERF_WRITE_NODE})"

    # Cluster mode always uses embedded RocksDB for the raft/protocol layer, so
    # its anchor label and backend are "rocks"/embedded regardless of PERF_STORAGE.
    if [ "$PERF_MODE" = "cluster" ]; then
        PERF_STORAGE="rocks"
    fi

    # Map the anchor-storage label to the backend bench_ab.py actually starts.
    # "rocks" (the label) means embedded RocksDB; mysql/postgres make the driver
    # start the server against a real external database.
    local storage_arg="embedded"
    case "$PERF_STORAGE" in
        rocks|embedded) storage_arg="embedded" ;;
        mysql)          storage_arg="mysql" ;;
        postgres)       storage_arg="postgres" ;;
        *)              storage_arg="embedded" ;;
    esac
    echo "    machine=${MACHINE} storage=${PERF_STORAGE} mode=${PERF_MODE} backend=${storage_arg}"

    # Nothing anchored for this machine: say so instead of spending 20 minutes
    # benchmarking against a number that was never valid here.
    if [ "$PERF_CAPTURE" != 1 ]; then
        local anchored
        anchored=$(awk -F'|' -v m="$MACHINE" -v s="$PERF_STORAGE" -v mo="$PERF_MODE" \
            '$1==m && $2==s && $3==mo' "$PERF_BASELINE" 2>/dev/null | wc -l | tr -d ' ')
        if [ "${anchored:-0}" = "0" ]; then
            echo "    no anchors for machine '${MACHINE}' (${PERF_STORAGE}/${PERF_MODE})"
            echo "    record them with: ./scripts/regression.sh --perf-capture"
            PERF_ADVISORY+=("no anchors for machine '${MACHINE}' (${PERF_STORAGE}/${PERF_MODE}); nothing compared")
            stage_end perf 0 "no anchors for this machine"
            return 0
        fi
    fi

    local rc=0 case_name
    local host_state_log="$LOG_DIR/perf-host-state.log"
    for case_name in "${PERF_CASES[@]}"; do
        local csv="$LOG_DIR/perf-${case_name}.csv"
        local perf_log_dir="$LOG_DIR/perf-${case_name}"
        mkdir -p "$perf_log_dir"
        # Host context is what makes a slow run explainable after the fact.
        python3 "${SCRIPT_DIR}/perf_observability.py" --snapshot \
            "$host_state_log" "before-${case_name}-${PERF_MODE}" || \
            { perf_fail "${case_name}: host state snapshot failed" || rc=1; }
        python3 "${SCRIPT_DIR}/bench_ab.py" \
            --case "$case_name" \
            --variant "cur=BATATA_NOOP=1" \
            --storage "$storage_arg" \
            --rounds "$PERF_ROUNDS" --warmup-rounds "$PERF_WARMUP_ROUNDS" \
            --duration "$PERF_DURATION" --warmup "$PERF_WARMUP" \
            --connections "$PERF_CONNECTIONS" --threads "$PERF_THREADS" --workers "$PERF_WORKERS" \
            --mode "$PERF_MODE" --write-node "$write_node" --fresh-store \
            --log-dir "$perf_log_dir" \
            --host-state-log "$host_state_log" \
            --server-log-max-bytes "$PERF_SERVER_LOG_MAX_BYTES" \
            --out "$csv" > "$LOG_DIR/perf-${case_name}.log" 2>&1
        python3 "${SCRIPT_DIR}/perf_observability.py" --snapshot \
            "$host_state_log" "after-${case_name}-${PERF_MODE}" || \
            { perf_fail "${case_name}: host state snapshot failed" || rc=1; }

        # Cluster perf node logs are run through the same Raft health
        # classifier as the matrix stage, but the result is DIAGNOSTIC here:
        # the driver SIGTERMs all three nodes between rounds, and a peer
        # disappearing during shutdown can trigger an "election after a leader
        # was known" WARN 16 ms before the shutdown marker. That is a shutdown
        # artifact, not a measured-path anomaly. The matrix stage remains the
        # authoritative Raft health gate. The marker still makes the
        # classifier expect telemetry liveness, so truncated/empty logs show
        # up as "telemetry liveness missing" instead of a false green.
        if [ "$PERF_MODE" = "cluster" ]; then
            local marker="$LOG_DIR/perf-${case_name}-cluster.marker"
            local health_args=("$marker") f health_summary
            printf '[MATRIX] [cluster-rock] perf case=%s mode=cluster\n' "$case_name" > "$marker"
            for f in "$perf_log_dir"/*.node*.log; do
                [ -f "$f" ] && health_args+=("$f")
            done
            if [ "${#health_args[@]}" -eq 1 ]; then
                perf_fail "${case_name}: no node logs retained for Raft health check" || rc=1
            else
                python3 "${SCRIPT_DIR}/raft_health_check.py" "${health_args[@]}" \
                    > "$LOG_DIR/perf-${case_name}-raft-health.log" 2>&1
                health_summary=$(cat "$LOG_DIR/perf-${case_name}-raft-health.log")
                PERF_ADVISORY+=("${case_name}: Raft health (diagnostic; shutdown-window elections included): ${health_summary}")
                echo "    Raft health (diagnostic): ${health_summary}"
            fi
        fi

        [ -f "$csv" ] || { perf_fail "${case_name}: no results" || rc=1; continue; }

        # Leader attribution is diagnostic context, not a new gate. Historical
        # CSVs have no meta lines and must stay readable; every new CSV carries
        # one row per recorded round, so an INCONCLUSIVE verdict can name the
        # two code paths instead of leaving the reader to infer them.
        local leader_summary
        if ! python3 "${SCRIPT_DIR}/perf_leader.py" --check-csv "$csv" \
            > "$LOG_DIR/perf-${case_name}.leaders" \
            2> "$LOG_DIR/perf-${case_name}.leaders.err"; then
            perf_fail "${case_name}: leader attribution failed" || rc=1
            leader_summary="unavailable"
        else
            leader_summary=$(cat "$LOG_DIR/perf-${case_name}.leaders")
            PERF_ADVISORY+=("${case_name}: leader attribution: ${leader_summary}")
            echo "    Leader attribution: ${leader_summary}"
        fi

        # Split the recorded rounds into leader-local / forwarded populations.
        # The aggregate median is only a real code path when every round took
        # that path; otherwise perf_paths reports MIXED and prints the two
        # independent medians. Advisory only: perf_dispersion decides whether
        # the sample may be compared to an anchor.
        local path_summary
        if ! python3 "${SCRIPT_DIR}/perf_paths.py" --check-csv "$csv" \
            --case "Config Write" \
            > "$LOG_DIR/perf-${case_name}.paths" \
            2> "$LOG_DIR/perf-${case_name}.paths.err"; then
            perf_fail "${case_name}: path split failed" || rc=1
            path_summary="unavailable"
        else
            path_summary=$(cat "$LOG_DIR/perf-${case_name}.paths")
            PERF_ADVISORY+=("${case_name}: path split: ${path_summary}")
            echo "    Path split: ${path_summary}"
        fi

        # Classify every `# raw <variant> <case>: [v1, v2, v3]` line first.
        # A spread above the limit means the recorded rounds do not describe
        # one population: comparing their median to an anchor (or capturing it
        # as a new anchor) would produce a meaningless verdict. Cluster Config
        # Write is the known case: node 1 takes all load, while the cold-start
        # Raft election picks the leader at random. See perf_dispersion.py.
        local dispersion_out="$LOG_DIR/perf-${case_name}.dispersion"
        if ! python3 "${SCRIPT_DIR}/perf_dispersion.py" --check-csv "$csv" \
            --baseline "$PERF_BASELINE" --machine "$MACHINE" \
            --storage "$PERF_STORAGE" --mode "$PERF_MODE" \
            --max-spread "$PERF_MAX_SPREAD_PCT" \
            --spread-factor "$PERF_SPREAD_FACTOR" \
            > "$dispersion_out" 2> "${dispersion_out}.err"; then
            perf_fail "${case_name}: dispersion classifier failed" || rc=1
            continue
        fi

        while IFS='|' read -r name med spread limit label reason; do
            [ -n "$name" ] || continue
            if [ "$label" != "COMPARABLE" ]; then
                PERF_INCONCLUSIVE=$(( PERF_INCONCLUSIVE + 1 ))
                PERF_ADVISORY+=("${name}: INCONCLUSIVE leader context: ${leader_summary}; path split: ${path_summary}")
                if [ "$PERF_CAPTURE" = 1 ]; then
                    # dispersion-guard: never wash a bimodal median into an anchor.
                    echo "    [BLOCKER] ${name}: ${reason} - capture refused"
                    record_failure perf "${name}: ${reason} - capture refused"
                    rc=1
                else
                    perf_fail "${name}: INCONCLUSIVE - ${reason}" || rc=1
                fi
                continue
            fi

            if [ "$PERF_CAPTURE" = 1 ]; then
                perf_capture_write "$MACHINE" "$name" "$med" "$spread"
                echo "    ${name}: captured median ${med} (cv ${spread}%)"
                continue
            fi

            local anchor_line anchor acv tol floor
            anchor_line=$(perf_anchor "$MACHINE" "$name")
            if [ -z "$anchor_line" ]; then
                echo "    ${name}: no anchor for this machine (measured ${med})"
                PERF_ADVISORY+=("${name}: no anchor for machine '${MACHINE}' (measured ${med})")
                continue
            fi
            anchor=${anchor_line%%|*}
            acv=${anchor_line##*|}
            tol=$(awk -v c="$acv" -v t="$PERF_TOLERANCE" 'BEGIN{ v=c/100; if (v<t) v=t; printf "%.3f", v }')
            floor=$(awk -v a="$anchor" -v t="$tol" 'BEGIN{printf "%.0f", a*(1-t)}')
            if awk -v m="$med" -v f="$floor" 'BEGIN{exit !(m<f)}'; then
                perf_fail "${name}: ${med} RPS < floor ${floor} (anchor ${anchor}, tol ${tol}, cv ${spread}%)" || rc=1
            else
                echo "    ${name}: median ${med} (cv ${spread}%, spread limit ${limit}%) vs anchor ${anchor} (tol ${tol}) — ok"
            fi
        done < "$dispersion_out"
    done

    local detail="advisory"
    [ "$PERF_GATE" = 1 ] && detail="gating"
    [ "$PERF_CAPTURE" = 1 ] && detail="captured for ${MACHINE}"
    [ "$PERF_INCONCLUSIVE" -gt 0 ] && detail="${detail}; ${PERF_INCONCLUSIVE} inconclusive"
    stage_end perf $rc "$detail"
}

# ---------------------------------------------------------------------------
# Report assembly
# ---------------------------------------------------------------------------
write_report() {
    local elapsed
    elapsed=$(secs "$GATE_START")
    {
        echo "## Stages"
        echo
        echo "| Stage | Status | Detail | Time |"
        echo "|---|---|---|---|"
        local row name status detail time
        for row in "${STAGE_ROWS[@]:-}"; do
            [ -n "$row" ] || continue
            IFS='|' read -r name status detail time <<< "$row"
            echo "| ${name} | ${status} | ${detail} | ${time} |"
        done
        echo
        echo "Total: $(human "$elapsed")"
        echo
    } >> "$REPORT"

    {
        echo "## Blockers"
        echo
        if [ "${#BLOCKERS[@]}" -eq 0 ]; then
            echo "(none)"
        else
            echo "| Stage | Failure |"
            echo "|---|---|"
            local b
            for b in "${BLOCKERS[@]}"; do
                IFS='|' read -r s i <<< "$b"
                echo "| ${s} | \`${i}\` |"
            done
        fi
        echo
        echo "## Known failures (whitelisted)"
        echo
        if [ "${#WHITELISTED[@]}" -eq 0 ]; then
            echo "(none)"
        else
            echo "| Stage | Failure | Reason |"
            echo "|---|---|---|"
            local w
            for w in "${WHITELISTED[@]}"; do
                IFS='|' read -r s i r <<< "$w"
                echo "| ${s} | \`${i}\` | ${r} |"
            done
        fi
        echo
        if [ "${#PERF_ADVISORY[@]}" -gt 0 ]; then
            echo "## Perf (advisory)"
            echo
            echo "Not blockers. The perf stage is the only one whose result depends on how"
            echo "busy the host is; it blocks only under \`--perf-gate\`."
            echo
            local p
            for p in "${PERF_ADVISORY[@]}"; do echo "- ${p}"; done
            echo
        fi
        echo "## Artifacts"
        echo
        echo "- Logs: \`reports/regression/${RUN_ID}/logs/\`"
        echo "- Latest: \`reports/regression/latest\`"
        echo
    } >> "$REPORT"
}

finalize() {
    local rc=0
    [ "${#BLOCKERS[@]}" -gt 0 ] && rc=1

    ln -sfn "${RUN_ID}" "${ROOT}/reports/regression/latest"

    echo
    echo "============================================================="
    if [ "$rc" -eq 0 ]; then
        echo "GATE PASSED  (${LEVEL})   blockers=0  known=${#WHITELISTED[@]}"
    else
        echo "GATE FAILED  (${LEVEL})   blockers=${#BLOCKERS[@]}  known=${#WHITELISTED[@]}"
        local b
        for b in "${BLOCKERS[@]}"; do echo "  BLOCKER: ${b}"; done
    fi
    echo "Report: reports/regression/${RUN_ID}/report.md"
    echo "============================================================="
    return $rc
}

# ---------------------------------------------------------------------------
main() {
    local perf_label=""
    [ "$RUN_PERF" = 1 ] && perf_label=" + perf"
    [ "$PERF_CAPTURE" = 1 ] && perf_label=" + perf-capture"
    [ "$PERF_GATE" = 1 ] && perf_label=" + perf-gate"
    report_header
    echo "Batata regression gate — level=${LEVEL}${perf_label}"
    echo "Report dir: ${REPORT_DIR}"
    echo
    stage_fmt
    stage_check
    stage_unit
    stage_sim
    stage_integration
    stage_matrix
    stage_perf
    write_report
    finalize
}

main "$@"
