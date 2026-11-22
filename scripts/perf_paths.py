#!/usr/bin/env python3
"""Split perf rounds into ``leader-local`` / ``forwarded`` populations.

``bench_ab.py`` writes one ``# meta leader|...`` line per recorded round and
one ``# raw <variant> <case>: [v1, v2, ...]`` line per case. In cluster mode a
cold-start Raft election decides whether the load target is the leader; when
it is not, the write is forwarded over gRPC and waits for the local state
machine to catch up. Those two code paths differ by roughly 5x in throughput,
so their median describes neither of them (this is the Config Write double
peak seen on 2026-11-21).

This module joins the two artefacts by round order and reports a median *per
path* plus a verdict:

    SINGLE_PATH   all recorded rounds took one path; the median is attributable
    MIXED         two paths were recorded; the aggregate median is meaningless
    UNATTRIBUTED  no/mismatched meta, or the leader could not be resolved

It is observability only. No gate, anchor or Raft behaviour is changed:
``regression.sh`` prints the result as a perf advisory. The dispersion
criterion remains the authority on whether a mixed sample may be compared to
an anchor.

Self-test uses the real archived CSVs as ground truth (233043 mixed, 094242
forwarded-only) and statically guards the shell and bench wiring.
"""

from __future__ import annotations

import argparse
import re
import statistics
import subprocess
import sys
from pathlib import Path

import perf_leader

SCRIPT_DIR = Path(__file__).resolve().parent
REGRESSION_SH = SCRIPT_DIR / "regression.sh"
BENCH_AB = SCRIPT_DIR / "bench_ab.py"
README = SCRIPT_DIR / "README.md"

DEFAULT_CASE = "Config Write"
SINGLE_PATH = "SINGLE_PATH"
MIXED = "MIXED"
UNATTRIBUTED = "UNATTRIBUTED"

RAW_LINE = re.compile(r"^# raw (\S+) (.+?): \[(.*)\]\s*$")


def parse_raw(text: str) -> dict[tuple[str, str], list[float]]:
    """Read ``# raw <variant> <case>: [..]`` lines into ordered samples.

    Values are in recorded-round order. A malformed line is ignored rather
    than silently shifted: a shift would attribute a value to the wrong path.
    """
    out: dict[tuple[str, str], list[float]] = {}
    for line in text.splitlines():
        match = RAW_LINE.match(line)
        if not match:
            continue
        variant, case, body = match.groups()
        values: list[float] = []
        try:
            for token in body.split(","):
                token = token.strip()
                if token:
                    values.append(float(token))
        except ValueError:
            continue
        out[(variant, case.strip())] = values
    return out


def parse_paths(text: str) -> dict[str, list[dict[str, object]]]:
    """Group leader meta rows by variant, keeping CSV order."""
    grouped: dict[str, list[dict[str, object]]] = {}
    for row in perf_leader.parse_meta(text):
        grouped.setdefault(str(row["variant"]), []).append(row)
    return grouped


def _median(values: list[float]) -> float:
    return statistics.median(values) if values else float("nan")


def _spread_pct(values: list[float]) -> float:
    if len(values) < 2:
        return 0.0
    med = statistics.median(values)
    if med == 0:
        return 0.0
    return 100.0 * (max(values) - min(values)) / med


def classify(
    text: str, case_filter: str | None = DEFAULT_CASE
) -> list[dict[str, object]]:
    """Return one record per (variant, case) with per-path medians."""
    raw = parse_raw(text)
    paths = parse_paths(text)
    records: list[dict[str, object]] = []
    for (variant, case), values in raw.items():
        if case_filter is not None and case != case_filter:
            continue
        rows = paths.get(variant, [])
        groups: dict[str, list[float]] = {}
        reason = ""
        verdict = UNATTRIBUTED
        if not values:
            reason = "no raw samples"
        elif not rows:
            reason = "no leader meta (historical CSV)"
        elif len(rows) != len(values):
            reason = f"meta/raw count mismatch ({len(rows)} vs {len(values)})"
        else:
            for row, value in zip(rows, values):
                groups.setdefault(str(row["write_path"]), []).append(value)
            known = [p for p in (perf_leader.LEADER_LOCAL, perf_leader.FORWARDED)
                     if groups.get(p)]
            if len(known) >= 2:
                verdict = MIXED
                reason = ("aggregate median mixes " + " + ".join(known)
                          + "; use the per-path medians below")
            elif len(known) == 1:
                verdict = SINGLE_PATH
                write_nodes = sorted({str(row["write_node"]) for row in rows})
                reason = (f"median is attributable to {known[0]} "
                          f"(write-node {','.join(write_nodes)})")
            elif perf_leader.STANDALONE in groups:
                verdict = SINGLE_PATH
                reason = "standalone has no Raft write path"
            else:
                reason = "no round has a resolved write path"
        records.append({
            "variant": variant,
            "case": case,
            "verdict": verdict,
            "reason": reason,
            "values": values,
            "groups": groups,
            "aggregate_median": _median(values),
            "aggregate_spread_pct": _spread_pct(values),
            "meta_rounds": len(rows),
        })
    return records


def format_record(record: dict[str, object]) -> list[str]:
    """Human/machine readable lines; one verdict line plus one per path."""
    case = str(record["case"])
    variant = str(record["variant"])
    lines: list[str] = []
    for path in sorted(record["groups"]):  # type: ignore[union-attr]
        values = record["groups"][path]  # type: ignore[index]
        lines.append(
            f"{case}|{variant}|{path}|n={len(values)}|"
            f"median={_median(values):.2f}|spread_pct={_spread_pct(values):.2f}"
        )
    lines.append(
        f"{case}|{variant}|{record['verdict']}|"
        f"n={len(record['values'])}|"  # type: ignore[arg-type]
        f"aggregate_median={float(record['aggregate_median']):.2f}|"
        f"spread_pct={float(record['aggregate_spread_pct']):.2f}|"
        f"{record['reason']}"
    )
    return lines


def evaluate_csv(text: str, case_filter: str | None = DEFAULT_CASE) -> str:
    """Full report for one perf CSV."""
    records = classify(text, case_filter)
    if not records:
        return f"NO_PATH_SAMPLES|case={case_filter}"
    lines: list[str] = []
    for record in records:
        lines.extend(format_record(record))
    return "\n".join(lines)


def _stage_body(source: str, name: str) -> str:
    match = re.search(rf"^{re.escape(name)}\(\) \{{(.*?)^\}}", source, re.S | re.M)
    return match.group(1) if match else ""


def self_test() -> int:
    """Ground-truth CSV fixtures plus static wiring guards."""
    import tempfile

    cases: list[tuple[str, bool, str]] = []

    def record(label: str, ok: bool, detail: str = "") -> None:
        cases.append((label, bool(ok), detail))

    def meta(variant: str, round_no: int, leader: str, path: str,
             term: int, source: str, write_node: int = 1) -> str:
        return (f"# meta leader|{variant}|r{round_no}|{leader}|{path}|"
                f"{term}|{source}|{write_node}")

    with tempfile.TemporaryDirectory(prefix="batata-perf-paths-") as tmp:
        root = Path(tmp)
        # 1. 233043: the real Config Write double peak. r1 was forwarded to
        # node 3, r2 ran locally on node 1. The aggregate median 8666 is a
        # mixture and must never be presented as a single code path.
        double_peak = "\n".join([
            "case,variant,median_rps,baseline_rps,delta_pct,verdict",
            "# meta build|61086d17|clean|cluster|3|batata-server",
            meta("cur", 1, "node3", perf_leader.FORWARDED, 2, "leader_changed"),
            meta("cur", 2, "node1", perf_leader.LEADER_LOCAL, 2, "leader_changed"),
            "# raw cur Config Write: [2701.0, 14631.0]",
            "# raw cur Config Read: [100.0, 101.0]",
            "",
        ])
        records = classify(double_peak)
        config = [r for r in records if r["case"] == "Config Write"]
        record("only the requested case is classified",
               len(records) == 1 and records[0]["case"] == "Config Write",
               str(records))
        record("233043 is MIXED", config and config[0]["verdict"] == MIXED,
               str(config))
        record("233043 forwarded median is 2701",
               config and config[0]["groups"].get(perf_leader.FORWARDED) == [2701.0],
               str(config))
        record("233043 leader-local median is 14631",
               config and config[0]["groups"].get(perf_leader.LEADER_LOCAL) == [14631.0],
               str(config))
        record("233043 aggregate median is 8666",
               config and config[0]["aggregate_median"] == 8666.0, str(config))
        report = evaluate_csv(double_peak)
        record("report names both paths",
               "forwarded|n=1|median=2701.00" in report
               and "leader-local|n=1|median=14631.00" in report, report)
        record("report labels the aggregate MIXED", "Config Write|cur|MIXED|" in report)

        # 2. P0 short cluster run: both rounds forwarded to node 3. One
        # population, so the median is a legitimate single-path number.
        forwarded_only = "\n".join([
            meta("cur", 1, "node3", perf_leader.FORWARDED, 2, "leader_changed"),
            meta("cur", 2, "node3", perf_leader.FORWARDED, 2, "leader_changed"),
            "# raw cur Config Write: [2701.0, 3064.3]",
            "",
        ])
        single = classify(forwarded_only)
        record("094242 is SINGLE_PATH",
               len(single) == 1 and single[0]["verdict"] == SINGLE_PATH,
               str(single))
        record("094242 keeps both samples on one path",
               single and len(single[0]["groups"].get(perf_leader.FORWARDED, [])) == 2)
        record("094242 spread is computed",
               single and single[0]["aggregate_spread_pct"] > 0)

        # 3. Standalone has no Raft path and must not be called MIXED.
        standalone = "\n".join([
            meta("cur", 1, "unknown", perf_leader.STANDALONE, "unknown", "no-raft"),
            meta("cur", 2, "unknown", perf_leader.STANDALONE, "unknown", "no-raft"),
            "# raw cur Config Write: [100.0, 102.0]",
            "",
        ])
        record("standalone is a single path",
               classify(standalone)[0]["verdict"] == SINGLE_PATH)

        # 4. Backward compatibility: six-field meta lines from before the
        # dual-path split still attribute rounds (default write node 1).
        legacy = "\n".join([
            "# meta leader|cur|r1|node3|forwarded|2|leader_changed",
            "# meta leader|cur|r2|node3|forwarded|2|leader_changed",
            "# raw cur Config Write: [2701.0, 2882.65]",
            "",
        ])
        record("six-field legacy meta still classifies",
               classify(legacy)[0]["verdict"] == SINGLE_PATH)

        # 5. Fail closed: no meta, mismatched counts, unresolved leaders.
        no_meta = "# raw cur Config Write: [2701.0, 14631.0]\n"
        record("missing meta is UNATTRIBUTED",
               classify(no_meta)[0]["verdict"] == UNATTRIBUTED)
        mismatched = "\n".join([
            meta("cur", 1, "node3", perf_leader.FORWARDED, 2, "leader_changed"),
            "# raw cur Config Write: [2701.0, 14631.0]",
            "",
        ])
        mismatched_result = classify(mismatched)[0]
        record("meta/raw mismatch is UNATTRIBUTED",
               mismatched_result["verdict"] == UNATTRIBUTED
               and "mismatch" in str(mismatched_result["reason"]),
               str(mismatched_result))
        unknown = "\n".join([
            meta("cur", 1, "unknown", perf_leader.PATH_UNKNOWN, "unknown", "none"),
            meta("cur", 2, "unknown", perf_leader.PATH_UNKNOWN, "unknown", "none"),
            "# raw cur Config Write: [2701.0, 2882.65]",
            "",
        ])
        record("unresolved leaders are UNATTRIBUTED",
               classify(unknown)[0]["verdict"] == UNATTRIBUTED)

        # 6. Case names with spaces and malformed lines.
        odd = "\n".join([
            meta("cur", 1, "node1", perf_leader.LEADER_LOCAL, 2, "leader_changed"),
            "# raw cur Config Write: [1.0, bad, 3.0]",
            "# nothing to see here",
            "",
        ])
        record("malformed raw line is ignored", parse_raw(odd) == {})
        record("unbracketed raw line is ignored",
               parse_raw("# raw cur Config Write: 1.0, 2.0\n") == {})
        record("trailing junk after a raw line is ignored",
               parse_raw("# raw cur Config Write: [1.0] trailing\n") == {})
        record("no matching case yields NO_PATH_SAMPLES",
               evaluate_csv(double_peak, "Naming Write").startswith("NO_PATH_SAMPLES|"))

        # 7. CLI: valid CSV works, missing file fails, gating flag is strict.
        csv = root / "perf-config.csv"
        csv.write_text(double_peak, encoding="utf-8")
        cli = [sys.executable, str(Path(__file__).resolve()), "--check-csv"]
        ok = subprocess.run(cli + [str(csv)], capture_output=True, text=True)
        record("CLI prints both paths",
               ok.returncode == 0 and "leader-local" in ok.stdout
               and "forwarded" in ok.stdout, ok.stderr)
        missing = subprocess.run(cli + [str(root / "nope.csv")],
                                 capture_output=True, text=True)
        record("CLI rejects a missing CSV", missing.returncode != 0)
        gated = subprocess.run(cli + [str(csv), "--require-single-path"],
                               capture_output=True, text=True)
        record("--require-single-path rejects MIXED", gated.returncode == 1,
               gated.stderr)
        single_csv = root / "perf-single.csv"
        single_csv.write_text(forwarded_only, encoding="utf-8")
        gated_ok = subprocess.run(cli + [str(single_csv), "--require-single-path"],
                                  capture_output=True, text=True)
        record("--require-single-path accepts SINGLE_PATH",
               gated_ok.returncode == 0, gated_ok.stderr)

    # 8. Static wiring guards: removing any link turns the check stage red.
    shell = REGRESSION_SH.read_text(encoding="utf-8") if REGRESSION_SH.is_file() else ""
    bench = BENCH_AB.read_text(encoding="utf-8") if BENCH_AB.is_file() else ""
    readme = README.read_text(encoding="utf-8") if README.is_file() else ""
    perf_body = _stage_body(shell, "stage_perf")
    check_body = _stage_body(shell, "stage_check")
    for token in ("# raw ", "# meta leader", "--write-node",
                  '"--write-node",', "load_port("):
        record(f"bench_ab.py wires {token}", token in bench)
    for token in ("perf_paths.py", "--check-csv", 'perf_paths.py" --check-csv',
                  "perf-${case_name}.paths", "PERF_ADVISORY", "PERF_WRITE_NODE",
                  "--write-node", "path split: ${path_summary}",
                  "; path split: ${path_summary}",
                  "--write-node \"$write_node\"",
                  "write_node=2", "load target: node ${write_node}"):
        record(f"stage_perf wires {token}", token in perf_body)
    for token in ("perf_paths.py", "--self-test", "check-perf-paths.log",
                  "SELF-TEST PASS", "perf_paths self-test failed"):
        record(f"stage_check wires {token}", token in check_body)
    record("stage_check runs the path self-test",
           'perf_paths.py" --self-test' in check_body)
    record("stage_check artifact assertion covers paths",
           check_body.count("check-perf-paths.log") >= 2)
    record("README documents the path split",
           "├── perf_paths.py" in readme and "`perf_paths.py` completes" in readme
           and "leader-local" in readme)

    failed = 0
    for label, ok, detail in cases:
        if ok:
            print(f"[PASS] {label}")
        else:
            failed += 1
            print(f"[FAIL] {label}{(': ' + detail) if detail else ''}")
    print(f"self-test: {len(cases) - failed}/{len(cases)} passed")
    if failed:
        return 1
    print("SELF-TEST PASS: perf path split and shell wiring intact")
    return 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--self-test", action="store_true",
                        help="run CSV fixtures and shell wiring guards")
    parser.add_argument("--check-csv", metavar="FILE",
                        help="print per-path medians from one perf CSV")
    parser.add_argument("--case", default=DEFAULT_CASE,
                        help=f"case name to split (default: {DEFAULT_CASE}); "
                        "use 'ALL' for every case")
    parser.add_argument("--require-single-path", action="store_true",
                        help="with --check-csv, fail when a case is MIXED or "
                        "UNATTRIBUTED (fail-closed gating, off by default)")
    args = parser.parse_args(argv[1:])

    if args.self_test:
        return self_test()
    if args.check_csv:
        try:
            text = Path(args.check_csv).read_text(encoding="utf-8", errors="replace")
        except OSError as error:
            print(f"perf_paths: cannot read perf CSV: {error}", file=sys.stderr)
            return 1
        case_filter = None if args.case == "ALL" else args.case
        report = evaluate_csv(text, case_filter)
        print(report)
        if args.require_single_path:
            bad = [r for r in classify(text, case_filter)
                   if r["verdict"] != SINGLE_PATH]
            if bad:
                print(f"perf_paths: {len(bad)} case(s) not single-path",
                      file=sys.stderr)
                return 1
        return 0
    parser.print_help()
    return 3


if __name__ == "__main__":
    sys.exit(main(sys.argv))
