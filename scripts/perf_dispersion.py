#!/usr/bin/env python3
"""Reject perf samples whose recorded rounds do not describe one population.

Motivation (2026-11-21). ``bench_ab.py`` sends all load to cluster node 1, but
a cold-start Raft election can pick any of the three nodes as leader. When node
1 is the leader, ``RaftNode::write_with_index`` runs ``client_write`` locally
(~1.8 ms); otherwise it forwards the write over gRPC and then waits for the
follower state machine to catch up (~7.4 ms). Two recorded Config Write rounds
can therefore be 2701 and 14631 RPS. Their median (8666) describes no real code
path: it must not be compared with the 2026-11-15 anchor (2735, captured from
the forwarded population), and it must never be written back as a new anchor.

``regression.sh --perf`` used to compute ``(max-min)/median`` and only print it.
This module turns that number into a criterion: a run whose rounds disagree by
more than the limit is INCONCLUSIVE. Under ``--perf-gate`` that is a blocker;
under ``--perf-capture`` the anchor write is refused. The self-test also guards
the shell wiring, so deleting the call, weakening the branch, or dropping the
check-stage invocation turns the ``check`` stage red instead of silently
disabling the criterion.

Output of ``--check-csv`` is one pipe-delimited row per recorded scenario:
``name|median|spread_pct|limit_pct|label|reason``. The shell owns reporting and
gating; all arithmetic and classification lives here so it is unit-testable.
"""

from __future__ import annotations

import argparse
import math
import re
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
REGRESSION_SH = SCRIPT_DIR / "regression.sh"

RAW_LINE = re.compile(r"^# raw \S+ (.+?): \[(.+)\]\s*$")
COMPARABLE = "COMPARABLE"
INCONCLUSIVE = "INCONCLUSIVE"
DEFAULT_MAX_SPREAD_PCT = 20.0
DEFAULT_SPREAD_FACTOR = 3.0


def classify(
    values: list[float],
    anchor_cv_pct: float = 0.0,
    max_spread_pct: float = DEFAULT_MAX_SPREAD_PCT,
    spread_factor: float = DEFAULT_SPREAD_FACTOR,
) -> dict[str, object]:
    """Classify one scenario's recorded rounds.

    ``spread_pct`` is the project's existing ``(max-min)/median * 100`` metric
    (the baseline file calls it ``cv_pct``, though it is a range ratio, not a
    coefficient of variation). The limit is the larger of an absolute floor and
    a multiple of the machine's own anchor spread, so a box that cannot resolve
    better than 40% is not forced to meet a 20% criterion.
    """
    try:
        samples = [float(value) for value in values]
    except (TypeError, ValueError):
        return _inconclusive(0.0, 0.0, _limit(anchor_cv_pct, max_spread_pct, spread_factor),
                             "non-numeric sample")
    if len(samples) < 2:
        return _inconclusive(0.0, 0.0, _limit(anchor_cv_pct, max_spread_pct, spread_factor),
                             "fewer than 2 recorded rounds")
    if any((not math.isfinite(value)) or value <= 0 for value in samples):
        return _inconclusive(0.0, 0.0, _limit(anchor_cv_pct, max_spread_pct, spread_factor),
                             "non-positive or non-finite sample")

    median = statistics.median(samples)
    spread_pct = 100.0 * (max(samples) - min(samples)) / median
    limit_pct = _limit(anchor_cv_pct, max_spread_pct, spread_factor)
    label = COMPARABLE if spread_pct <= limit_pct else INCONCLUSIVE
    prefix = (
        f"spread {spread_pct:.1f}% <= limit {limit_pct:.6g}% "
        f"(anchor cv {anchor_cv_pct:.1f}%)"
        if label == COMPARABLE
        else f"spread {spread_pct:.1f}% > limit {limit_pct:.6g}% "
        f"(anchor cv {anchor_cv_pct:.1f}%); median {median:.0f} mixes distinct populations"
    )
    return {
        "median": median,
        "spread_pct": spread_pct,
        "limit_pct": limit_pct,
        "label": label,
        "reason": prefix,
    }


def _limit(anchor_cv_pct: float, max_spread_pct: float, spread_factor: float) -> float:
    return max(float(max_spread_pct), float(spread_factor) * float(anchor_cv_pct))


def _inconclusive(
    median: float, spread_pct: float, limit_pct: float, reason: str
) -> dict[str, object]:
    return {
        "median": median,
        "spread_pct": spread_pct,
        "limit_pct": limit_pct,
        "label": INCONCLUSIVE,
        "reason": reason,
    }


def load_anchor_cv(
    path: str, machine: str, storage: str, mode: str
) -> dict[str, float]:
    """Return ``{case: cv_pct}`` for one (machine, storage, mode) group."""
    result: dict[str, float] = {}
    try:
        lines = Path(path).read_text(encoding="utf-8").splitlines()
    except OSError:
        return result
    for line in lines:
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        fields = line.split("|")
        if len(fields) < 8:
            continue
        if fields[0] == machine and fields[1] == storage and fields[2] == mode:
            try:
                result[fields[3]] = float(fields[5])
            except ValueError:
                continue
    return result


def parse_raw(text: str) -> dict[str, list[float]]:
    """Extract ``# raw <variant> <case>: [v1, v2, ...]`` lines from a CSV."""
    result: dict[str, list[float]] = {}
    for line in text.splitlines():
        match = RAW_LINE.match(line.strip())
        if not match:
            continue
        try:
            result[match.group(1).strip()] = [
                float(item.strip()) for item in match.group(2).split(",")
            ]
        except ValueError:
            result[match.group(1).strip()] = []
    return result


def evaluate_csv(
    csv_path: str,
    baseline: str,
    machine: str,
    storage: str,
    mode: str,
    max_spread_pct: float,
    spread_factor: float,
) -> str:
    """Classify every recorded scenario in one perf CSV."""
    anchors = load_anchor_cv(baseline, machine, storage, mode)
    rows: list[str] = []
    for name, values in parse_raw(Path(csv_path).read_text(encoding="utf-8")).items():
        result = classify(
            values,
            anchors.get(name, 0.0),
            max_spread_pct,
            spread_factor,
        )
        median = f"{float(result['median']):.6g}"
        rows.append(
            f"{name}|{median}|{float(result['spread_pct']):.1f}|"
            f"{float(result['limit_pct']):.6g}|{result['label']}|{result['reason']}"
        )
    if not rows:
        rows.append(
            f"NO_SAMPLES|0|0.0|{_limit(0.0, max_spread_pct, spread_factor):.6g}|"
            f"{INCONCLUSIVE}|no raw samples in CSV"
        )
    return "\n".join(rows)


# ---------------------------------------------------------------------------
# Self-test: numeric fixtures, parser fixtures, CLI smoke and shell wiring.
# ---------------------------------------------------------------------------
def _stage_body(source: str, name: str) -> str:
    """Extract one bash function body (functions end with a bare `}`)."""
    match = re.search(rf"^{re.escape(name)}\(\) \{{(.*?)^\}}", source, re.S | re.M)
    return match.group(1) if match else ""


def self_test() -> int:
    cases: list[tuple[str, bool, str]] = []

    def record(label: str, ok: bool, detail: str = "") -> None:
        cases.append((label, bool(ok), detail))

    # 1. Historical bimodal samples from real cluster Config Write runs.
    bimodal_233043 = classify([2701.0, 14631.0], 2.9)
    bimodal_204500 = classify([2891.4, 17465.0], 2.9)
    record("233043 bimodal Config Write is INCONCLUSIVE",
           bimodal_233043["label"] == INCONCLUSIVE)
    record("233043 spread matches the reported 137.7%",
           abs(float(bimodal_233043["spread_pct"]) - 137.7) < 0.2,
           str(bimodal_233043["spread_pct"]))
    record("204500 bimodal Config Write is INCONCLUSIVE",
           bimodal_204500["label"] == INCONCLUSIVE)
    record("204500 spread matches the reported 143.2%",
           abs(float(bimodal_204500["spread_pct"]) - 143.2) < 0.2,
           str(bimodal_204500["spread_pct"]))
    record("INCONCLUSIVE reason names spread and limit",
           "spread" in str(bimodal_233043["reason"])
           and "limit" in str(bimodal_233043["reason"]))

    # 2. Healthy samples from the same runs stay comparable.
    healthy = [
        ("Config Read", [161477.2, 161502.6], 0.7),
        ("standalone Config Write", [62832.0, 61792.5], 0.7),
        ("Instance Register", [131551.1, 130951.1], 0.1),
        ("Service List", [168918.2, 174048.8], 0.9),
        ("tight cluster writes", [2884.2, 2875.9], 2.9),
    ]
    for name, values, cv in healthy:
        result = classify(values, cv)
        record(f"healthy {name} is COMPARABLE", result["label"] == COMPARABLE,
               str(result["spread_pct"]))
    record("a three-round outlier is INCONCLUSIVE",
           classify([2700.0, 2750.0, 14600.0], 2.9)["label"] == INCONCLUSIVE)
    record("three tight rounds are COMPARABLE",
           classify([100.0, 101.0, 99.0])["label"] == COMPARABLE)
    record("identical rounds have zero spread",
           float(classify([100.0, 100.0, 100.0])["spread_pct"]) == 0.0)

    # 3. Threshold is anchored to the machine's own captured spread.
    noisy = classify([100.0, 130.0], 40.0)
    strict = classify([100.0, 130.0], 0.0)
    record("noisy anchor raises the limit", noisy["label"] == COMPARABLE
           and float(noisy["limit_pct"]) == 120.0, str(noisy["limit_pct"]))
    record("same sample fails with a tight anchor", strict["label"] == INCONCLUSIVE
           and float(strict["limit_pct"]) == 20.0)
    record("absolute floor wins over a small anchor cv",
           float(classify([100.0, 101.0], 2.9)["limit_pct"]) == 20.0)
    record("custom max-spread is honoured",
           classify([100.0, 115.0], 0.0, 10.0)["label"] == INCONCLUSIVE
           and classify([100.0, 115.0], 0.0, 20.0)["label"] == COMPARABLE)
    record("custom spread-factor is honoured",
           float(classify([100.0, 130.0], 40.0, 20.0, 0.0)["limit_pct"]) == 20.0)

    # 4. Degenerate input fails closed rather than producing a fake median.
    # Assert the *reason* too: a zero/negative/NaN sample can also end up
    # INCONCLUSIVE through a huge/non-finite spread, which would let a broken
    # validation guard survive the mutant check.
    for label, values, expected in (
        ("single round", [1000.0], "fewer than 2 recorded rounds"),
        ("no rounds", [], "fewer than 2 recorded rounds"),
        ("zero sample", [0.0, 100.0], "non-positive or non-finite sample"),
        ("negative sample", [-5.0, 100.0], "non-positive or non-finite sample"),
        ("nan sample", [float("nan"), 100.0], "non-positive or non-finite sample"),
        ("non-numeric sample", ["oops", "100"], "non-numeric sample"),
    ):
        result = classify(values)
        record(f"{label} is INCONCLUSIVE", result["label"] == INCONCLUSIVE)
        record(f"{label} fails for the right reason",
               str(result["reason"]) == expected, str(result["reason"]))

    # 5. CSV parsing and anchor lookup use fixtures, never the live baseline.
    with tempfile.TemporaryDirectory(prefix="batata-perf-dispersion-") as tmp:
        root = Path(tmp)
        csv = root / "perf-config.csv"
        csv.write_text(
            "case,variant,median_rps,baseline_rps,delta_pct,verdict\n"
            "# raw cur Config Write: [2701.0, 14631.0]\n"
            "# raw cur Config Read: [161477.2, 161502.6]\n"
            "# unrelated comment\n",
            encoding="utf-8",
        )
        parsed = parse_raw(csv.read_text(encoding="utf-8"))
        record("parse_raw finds both scenarios", set(parsed) == {"Config Write", "Config Read"})
        record("parse_raw keeps the raw values",
               parsed.get("Config Write") == [2701.0, 14631.0], str(parsed))
        baseline = root / "baseline.txt"
        baseline.write_text(
            "# header\n"
            "mac1714-18t|rocks|cluster|Config Write|2735|2.9|2026-11-15|c20-t2-w8-d20s-r3\n"
            "mac1714-18t|rocks|cluster|Config Read|161160|0.7|2026-11-15|c20-t2-w8-d20s-r3\n"
            "mac1714-18t|rocks|standalone|Config Write|60826|0.7|2026-11-15|c20-t2-w8-d20s-r3\n"
            "malformed|row\n",
            encoding="utf-8",
        )
        anchors = load_anchor_cv(str(baseline), "mac1714-18t", "rocks", "cluster")
        record("anchor lookup selects the right group",
               anchors == {"Config Write": 2.9, "Config Read": 0.7}, str(anchors))
        record("missing baseline yields no anchors",
               load_anchor_cv(str(root / "missing"), "m", "rocks", "cluster") == {})
        output = evaluate_csv(str(csv), str(baseline), "mac1714-18t", "rocks",
                              "cluster", 20.0, 3.0)
        lines = output.splitlines()
        record("evaluate_csv emits one row per scenario", len(lines) == 2, str(len(lines)))
        record("evaluate_csv flags only the bimodal scenario",
               [line.split("|")[4] for line in lines] == [INCONCLUSIVE, COMPARABLE],
               output)
        record("evaluate_csv rows have six pipe fields",
               all(len(line.split("|")) == 6 for line in lines))
        empty = root / "empty.csv"
        empty.write_text("case,variant\n", encoding="utf-8")
        record("empty CSV fails closed",
               INCONCLUSIVE in evaluate_csv(str(empty), str(baseline), "m", "rocks",
                                            "cluster", 20.0, 3.0))

        # 6. CLI smoke: a missing CSV must be an error, a valid one must work.
        cli = [sys.executable, str(Path(__file__).resolve()), "--check-csv"]
        common = ["--baseline", str(baseline), "--machine", "mac1714-18t",
                  "--storage", "rocks", "--mode", "cluster"]
        missing = subprocess.run(cli + [str(root / "nope.csv")] + common,
                                 capture_output=True, text=True)
        record("CLI rejects a missing CSV", missing.returncode != 0)
        ok = subprocess.run(cli + [str(csv)] + common, capture_output=True, text=True)
        record("CLI prints INCONCLUSIVE for the fixture",
               ok.returncode == 0 and INCONCLUSIVE in ok.stdout, ok.stderr)

    # 7. Guard the shell wiring: a dropped call or weakened branch must be red.
    source = REGRESSION_SH.read_text(encoding="utf-8") if REGRESSION_SH.is_file() else ""
    perf_body = _stage_body(source, "stage_perf")
    check_body = _stage_body(source, "stage_check")
    for token in (
        "perf_dispersion.py",
        "--check-csv",
        "PERF_MAX_SPREAD_PCT",
        "PERF_SPREAD_FACTOR",
        "INCONCLUSIVE",
        "capture refused",
        '[ "$label" != "COMPARABLE" ]',
        'perf_fail "${name}: INCONCLUSIVE',
        "PERF_INCONCLUSIVE",
    ):
        record(f"stage_perf wires {token}", token in perf_body)
    for token in (
        "perf_dispersion.py",
        "--self-test",
        "check-perf-dispersion.log",
        "SELF-TEST PASS",
        "perf_dispersion self-test failed",
    ):
        record(f"stage_check wires {token}", token in check_body)
    record("stage_check artifact assertion covers dispersion",
           check_body.count("check-perf-dispersion.log") >= 2)
    record("absolute spread floor defaults to 20",
           'PERF_MAX_SPREAD_PCT="${PERF_MAX_SPREAD_PCT:-20}"' in source)
    record("anchor spread factor defaults to 3",
           'PERF_SPREAD_FACTOR="${PERF_SPREAD_FACTOR:-3}"' in source)

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
    print("SELF-TEST PASS: perf dispersion criterion and shell wiring intact")
    return 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--self-test", action="store_true",
                        help="run numeric fixtures and shell wiring guards")
    parser.add_argument("--check-csv", metavar="FILE",
                        help="classify every raw scenario in one perf CSV")
    parser.add_argument("--values", metavar="V1,V2,...",
                        help="classify one sample list instead of a CSV")
    parser.add_argument("--baseline", default=str(SCRIPT_DIR / "regression_perf_baseline.txt"))
    parser.add_argument("--machine", default="")
    parser.add_argument("--storage", default="rocks")
    parser.add_argument("--mode", default="standalone")
    parser.add_argument("--anchor-cv", type=float, default=0.0)
    parser.add_argument("--max-spread", type=float, default=DEFAULT_MAX_SPREAD_PCT)
    parser.add_argument("--spread-factor", type=float, default=DEFAULT_SPREAD_FACTOR)
    args = parser.parse_args(argv[1:])

    if args.self_test:
        return self_test()
    try:
        if args.check_csv:
            print(evaluate_csv(args.check_csv, args.baseline, args.machine,
                               args.storage, args.mode, args.max_spread,
                               args.spread_factor))
            return 0
        if args.values:
            result = classify([item.strip() for item in args.values.split(",")],
                              args.anchor_cv, args.max_spread, args.spread_factor)
            print(f"|{float(result['median']):.6g}|{float(result['spread_pct']):.1f}|"
                  f"{float(result['limit_pct']):.6g}|{result['label']}|{result['reason']}")
            return 0
    except (OSError, ValueError) as error:
        print(f"perf_dispersion: {error}", file=sys.stderr)
        return 1
    parser.print_help()
    return 3


if __name__ == "__main__":
    sys.exit(main(sys.argv))
