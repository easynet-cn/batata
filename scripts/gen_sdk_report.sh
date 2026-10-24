#!/usr/bin/env bash
# ==============================================================================
# Normalise one SDK suite log into the shared TEST_REPORT.md format.
#
# Different SDK runners report results in incompatible ways:
#   Maven (nacos-java, apollo-java) -> "Tests run: N, Failures: F, Errors: E,
#                                      Skipped: S" + "[ERROR]   Class.method"
#   go test (consul-go)             -> "--- PASS: TestName" / "--- FAIL: ..."
#   cargo (batata-client,           -> "test result: ok. N passed; M failed;
#          consul-client,                K ignored" + a "failures:" block
#          maintainer)
#
# Every suite therefore needed its own way of being read, which is why only
# nacos-java ever had a TEST_REPORT.md: nobody could compare a Go count against
# a Maven count. This script is the single parser for all of them, so the
# matrix can emit one comparable report per suite.
#
# Usage:
#   gen_sdk_report.sh <suite> <logfile> <out.md> [combo] [duration]
#
# <suite> is one of: nacos-java | apollo-java | consul-go | batata-client |
#                    batata-consul-client | batata-maintainer
#
# It prints one machine-readable line on stdout so callers can aggregate
# without re-parsing:  <combo>|<suite>|<total>|<passed>|<failed>|<errors>|<skipped>
# ==============================================================================

set -uo pipefail

SUITE="${1:?usage: gen_sdk_report.sh <suite> <logfile> <out.md> [combo] [duration]}"
LOG="${2:?}"
OUT="${3:?}"
COMBO="${4:-standalone-rock}"
DURATION="${5:-}"

case "$SUITE" in
    nacos-java)            KIND=maven; TITLE="Nacos Java SDK";           CLIENT="Nacos Java SDK (Maven Surefire)" ;;
    apollo-java)           KIND=maven; TITLE="Apollo Java SDK";          CLIENT="Apollo Java SDK (Maven Surefire)" ;;
    consul-go)             KIND=go;    TITLE="Consul Go SDK";            CLIENT="Consul Go SDK (go test)" ;;
    batata-client)         KIND=cargo; TITLE="batata-client SDK";        CLIENT="batata-client (cargo test)" ;;
    batata-consul-client)  KIND=cargo; TITLE="batata-consul-client SDK"; CLIENT="batata-consul-client (cargo test)" ;;
    batata-maintainer)     KIND=cargo; TITLE="batata-maintainer SDK";    CLIENT="batata-maintainer (cargo test)" ;;
    *) echo "unknown suite: $SUITE" >&2; exit 2 ;;
esac

# Maven colours its output when attached to a TTY and go/cargo do not; strip
# escapes so a piped run and a terminal run parse identically.
strip_ansi() { sed -e 's/\x1b\[[0-9;]*[A-Za-z]//g' -e 's/\x1b\][^\x07]*\x07//g'; }

total=0 passed=0 failed=0 errors=0 skipped=0
fail_list=""

case "$KIND" in
maven)
    # Surefire prints one "Tests run:" line per class plus a grand total. The
    # grand total is last and is the only one without " -- in <class>".
    #
    # It is prefixed by whatever severity Maven assigns the run: [INFO] when
    # everything passed, [ERROR] when something did not, and — the case that
    # broke this parser — [WARNING] when tests were skipped. All three have to
    # be accepted, or a suite whose only oddity is a skipped test is reported
    # as having run 0 cases.
    last=$(grep -aE "^(Tests run:|\[[A-Z]+\] Tests run:)" "$LOG" \
            | strip_ansi | grep -av " -- in " | tail -1)
    if [ -n "$last" ]; then
        t=$(sed -n 's/.*Tests run: \([0-9]*\), Failures: \([0-9]*\), Errors: \([0-9]*\), Skipped: \([0-9]*\).*/\1 \2 \3 \4/p' <<<"$last")
        if [ -n "$t" ]; then
            # shellcheck disable=SC2086
            set -- $t
            total=$1; failed=$2; errors=$3; skipped=$4
        fi
    fi
    passed=$(( total - failed - errors - skipped ))
    [ "$passed" -lt 0 ] && passed=0
    # Failure entries look like "  NacosAuthRbacTest.testX:42 expected ...".
    # The bare "Failures:"/"Errors:" headers share the same indent, so drop
    # them explicitly.
    fail_list=$(grep -aE "^\[ERROR\]   " "$LOG" | strip_ansi \
                | sed -e 's/^\[ERROR\]   //' \
                | grep -avE "^(Failures|Errors|Tests run)" \
                | sed -e 's/:[0-9][0-9]*//' | sort -u)
    ;;
go)
    # `go test -v` emits one --- PASS/FAIL/SKIP line per test. Sub-tests are
    # indented ("    --- PASS:"), and counting them would double-count, so
    # only column-0 results are counted.
    passed=$(grep -ac "^--- PASS" "$LOG" 2>/dev/null || true)
    failed=$(grep -ac "^--- FAIL" "$LOG" 2>/dev/null || true)
    skipped=$(grep -ac "^--- SKIP" "$LOG" 2>/dev/null || true)
    passed=${passed:-0}; failed=${failed:-0}; skipped=${skipped:-0}
    total=$(( passed + failed + skipped ))
    fail_list=$(grep -aE "^--- FAIL" "$LOG" | strip_ansi \
                | sed -e 's/^--- FAIL: //' -e 's/ .*//' | sort -u)
    ;;
cargo)
    # A crate emits one "test result:" line per test binary, so they accumulate.
    while IFS= read -r line; do
        p=$(sed -n 's/.* \([0-9]*\) passed;.*/\1/p' <<<"${line}")
        f=$(sed -n 's/.* \([0-9]*\) failed;.*/\1/p' <<<"${line}")
        s=$(sed -n 's/.* \([0-9]*\) ignored;.*/\1/p' <<<"${line}")
        passed=$(( passed + ${p:-0} ))
        failed=$(( failed + ${f:-0} ))
        skipped=$(( skipped + ${s:-0} ))
    done < <(grep -aE "^test result:" "$LOG" | strip_ansi)
    total=$(( passed + failed + skipped ))
    fail_list=$(awk '
        /^failures:$/     { inlist=1; next }
        /^test result:/   { inlist=0; next }
        inlist && /^    [A-Za-z0-9_:]+$/ { gsub(/^ +/,""); print }
    ' "$LOG" | sort -u)
    ;;
esac

pct() { awk -v n="$1" -v t="$total" 'BEGIN{printf "%.1f", (t ? n*100/t : 0)}'; }

mkdir -p "$(dirname "$OUT")"
{
    echo "# ${TITLE} Compatibility Test Report"
    echo
    echo "- **Generated:** $(date '+%Y-%m-%d %H:%M:%S %z')"
    echo "- **Combination:** \`${COMBO}\`"
    echo "- **Client:** ${CLIENT}"
    echo "- **Raw log:** \`${LOG}\`"
    [ -n "$DURATION" ] && echo "- **Duration:** ${DURATION}"
    echo
    echo "> Auto-generated by \`scripts/gen_sdk_report.sh\` from the SDK matrix run."
    echo "> Do not hand-edit; re-run the matrix to refresh."
    echo
    echo "## Summary"
    echo
    echo "| Metric | Count | Percentage |"
    echo "|--------|-------|------------|"
    echo "| Total tests | ${total} | 100% |"
    echo "| **Passed** | **${passed}** | **$(pct "$passed")%** |"
    echo "| Failed | ${failed} | $(pct "$failed")% |"
    echo "| Errors | ${errors} | $(pct "$errors")% |"
    echo "| Skipped | ${skipped} | $(pct "$skipped")% |"
    echo
    echo "## Failure List"
    echo
    if [ -n "$fail_list" ]; then
        echo '```'
        echo "$fail_list"
        echo '```'
    else
        echo "(none)"
    fi
} > "$OUT"

echo "${COMBO}|${SUITE}|${total}|${passed}|${failed}|${errors}|${skipped}"
