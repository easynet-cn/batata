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
#   quick     fmt, check, unit(core), integration(standalone, rust SDK)
#   standard  + unit(all crates), integration(java + go SDK)
#   full      + matrix (standalone/cluster x rock/mysql/postgres)
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
#   PERF_ROUNDS=3 PERF_DURATION=20s PERF_WARMUP=10s
#   PERF_CONNECTIONS=20 PERF_THREADS=2 PERF_WORKERS=8
#   PERF_TOLERANCE=0.08       minimum tolerance when cv_pct is small
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
PERF_STORAGE="${PERF_STORAGE:-rocks}"
PERF_MODE="${PERF_MODE:-standalone}"
PERF_ROUNDS="${PERF_ROUNDS:-3}"
PERF_WARMUP_ROUNDS="${PERF_WARMUP_ROUNDS:-1}"
PERF_DURATION="${PERF_DURATION:-20s}"
PERF_WARMUP="${PERF_WARMUP:-10s}"
PERF_CONNECTIONS="${PERF_CONNECTIONS:-20}"
PERF_THREADS="${PERF_THREADS:-2}"
PERF_WORKERS="${PERF_WORKERS:-8}"
PERF_MACHINE_ID="${PERF_MACHINE_ID:-}"
PERF_CASES=(naming config)

# Crates run at every level. These are the ones whose failure means something
# changed in a hot path; the rest are covered at --standard.
CORE_CRATES=(batata-core batata-client batata-config batata-naming batata-plugin-consul)
STANDARD_CRATES=(batata-api batata-auth batata-persistence batata-consistency batata-server-common batata-console)
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
  quick     fmt check unit integration
  standard  fmt check unit integration
  full      fmt check unit integration matrix
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
    [ -x "${ROOT}/target/release/batata-server" ] && return 0
    [ -x "${ROOT}/target/debug/batata-server" ] && return 0
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
median_of() {
    printf '%s\n' "$1" | tr ',' '\n' | sort -n | awk '{v[NR]=$1} END{ if (NR%2) print v[(NR+1)/2]; else print (v[NR/2]+v[NR/2+1])/2 }'
}

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
    for case_name in "${PERF_CASES[@]}"; do
        local csv="$LOG_DIR/perf-${case_name}.csv"
        python3 "${SCRIPT_DIR}/bench_ab.py" \
            --case "$case_name" \
            --variant "cur=BATATA_NOOP=1" \
            --storage "$storage_arg" \
            --rounds "$PERF_ROUNDS" --warmup-rounds "$PERF_WARMUP_ROUNDS" \
            --duration "$PERF_DURATION" --warmup "$PERF_WARMUP" \
            --connections "$PERF_CONNECTIONS" --threads "$PERF_THREADS" --workers "$PERF_WORKERS" \
            --mode "$PERF_MODE" --fresh-store \
            --out "$csv" > "$LOG_DIR/perf-${case_name}.log" 2>&1
        [ -f "$csv" ] || { perf_fail "${case_name}: no results" || rc=1; continue; }

        # `# raw <variant> <case>: [v1, v2, v3]`
        while IFS='|' read -r name vals; do
            [ -n "$name" ] || continue
            local med mn mx cv
            med=$(median_of "$vals")
            mn=$(printf '%s\n' "$vals" | tr ',' '\n' | sort -n | head -1)
            mx=$(printf '%s\n' "$vals" | tr ',' '\n' | sort -n | tail -1)
            cv=$(awk -v a="$mn" -v b="$mx" -v m="$med" 'BEGIN{ if (m<=0) print "0"; else printf "%.1f", 100*(b-a)/m }')

            if [ "$PERF_CAPTURE" = 1 ]; then
                perf_capture_write "$MACHINE" "$name" "$med" "$cv"
                echo "    ${name}: captured median ${med} (cv ${cv}%)"
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
                perf_fail "${name}: ${med} RPS < floor ${floor} (anchor ${anchor}, tol ${tol}, cv ${cv}%)" || rc=1
            else
                echo "    ${name}: median ${med} (cv ${cv}%) vs anchor ${anchor} (tol ${tol}) — ok"
            fi
        done < <(awk -F':' '/^# raw /{ nm=$1; sub(/^# raw [^ ]+ /,"",nm); v=$2; gsub(/[][ ]/,"",v); print nm"|"v }' "$csv")
    done

    local detail="advisory"
    [ "$PERF_GATE" = 1 ] && detail="gating"
    [ "$PERF_CAPTURE" = 1 ] && detail="captured for ${MACHINE}"
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
    stage_integration
    stage_matrix
    stage_perf
    write_report
    finalize
}

main "$@"
