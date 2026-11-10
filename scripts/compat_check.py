#!/usr/bin/env python3
"""Validate docs/compat inventories (batata compatibility tracking).

Checks:
  1. feature-ID format           F-<PROTO>-<MODULE>-<NNN>  (PROTO: NAC|CON|APO|SYS)
  2. global uniqueness of every feature ID across all inventories
  3. per (PROTO, MODULE) contiguous numbering 1..N with no gaps/duplicates
  4. every feature row carries a valid Status enum cell
  5. each features.md Summary **Total** row matches recomputed rows/status/rate
  6. README global summary matches recomputed features + T-ID data-row counts
  7. 🟢 rows in anchor-bearing inventories carry a resolvable crate anchor
  8. F-ID references anywhere under docs/compat resolve to a known feature
  9. backticked file links in README resolve to an existing path
 10. the declared Nacos alignment target matches every executable/config
     version declaration (Java SDK pom, server compat version, inventory headings)

Rationale: the old checker only validated ID syntax and one number in the
Summary row, so hand-maintained inventories could drift (false 🟢 rows, stale
global totals, dead links) while the gate stayed green. Check 10 prevents the
3.2.4 / 3.3.0-SNAPSHOT / 3.3.0-RC baseline split from coming back.

Usage: scripts/compat_check.py [features.md ...]   (default: every features.md under docs/compat)
Exit code 0 = ok, 1 = at least one problem.
"""
from __future__ import annotations

import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMPAT = ROOT / "docs" / "compat"
README = COMPAT / "README.md"
PROTOS = ("NAC", "CON", "APO", "SYS")
PROTO_NAMES = {"nacos": "Nacos", "consul": "Consul", "apollo": "Apollo"}
STATUS = ("🟢", "🟡", "⚡", "⚪", "⛔")  # ordered: deterministic status detection
STATUS_SET = set(STATUS)

ID_RE = re.compile(r"F-(NAC|CON|APO|SYS)-([A-Z][A-Z-]*[A-Z])-(\d+)")
ROW_RE = re.compile(r"^\|\s*F-(?:NAC|CON|APO|SYS)-[A-Z][A-Z-]*[A-Z]-\d+\s*\|")
TROW_RE = re.compile(r"^\|\s*T-(?:NAC|CON|APO|SYS)-")
FID_REF_RE = re.compile(r"\bF-(?:NAC|CON|APO|SYS)-[A-Z][A-Z-]*[A-Z]-\d+\b")
CRATE_RE = re.compile(r"\bbatata-[a-z0-9-]+\b")
TOTAL_RE = re.compile(r"^\|\s*\*\*Total\*\*\s*\|")
GS_ROW_RE = re.compile(r"^\|\s*(Nacos|Consul|Apollo|\*\*Total\*\*)\s*\|")
PATH_REF_RE = re.compile(r"`((?:docs/compat|scripts|reports|crates|sdk-tests)/[^`]+)`")
TARGET_RE = re.compile(r"Alignment target: Nacos (\d+\.\d+\.\d+) GA")
# (relative path, regex capturing the declared version, whether the file is required)
VERSION_DECLARATIONS = (
    (
        "sdk-tests/nacos-java-tests/pom.xml",
        re.compile(r"<nacos\.version>\s*(\d+\.\d+\.\d+)\s*</nacos\.version>"),
        True,
    ),
    (
        "conf/application.yml",
        re.compile(r"^nacos\.version:\s*(\d+\.\d+\.\d+)\s*$", re.M),
        True,
    ),
    (
        "deploy/k8s/configmap.yaml",
        re.compile(r"^[ \t]*nacos\.version:\s*(\d+\.\d+\.\d+)\s*$", re.M),
        True,
    ),
    (
        "docs/compat/nacos/features.md",
        re.compile(r"Upstream baseline: \*\*Nacos (\d+\.\d+\.\d+) GA"),
        True,
    ),
    (
        "docs/compat/nacos/tests.md",
        re.compile(r"Upstream baseline: \*\*Nacos (\d+\.\d+\.\d+) GA"),
        True,
    ),
    (
        "docs/compat/review-report.md",
        re.compile(r"align with Nacos (\d+\.\d+\.\d+) GA"),
        True,
    ),
)


def cells(line: str) -> list[str]:
    return [c.strip() for c in line.strip().strip("|").split("|")]


def row_status(line: str) -> str | None:
    # Status emoji lives in different column positions across tables; scan the row.
    # STATUS is ordered so a row containing several markers is classified deterministically.
    return next((s for s in STATUS if s in line), None)


def number(cell: str) -> int | None:
    return int(re.sub(r"[^0-9]", "", cell)) if re.search(r"\d", cell) else None


def pct(green: int, total: int) -> int:
    return int(100 * green / total + 0.5) if total else 0


def crate_anchored(line: str) -> bool:
    """A row is anchored when it names at least one existing crate under crates/."""
    return any((ROOT / "crates" / name).is_dir() for name in set(CRATE_RE.findall(line)))


def check_baseline_versions(problems: list[str]) -> str | None:
    """Check 10: every executable/heading version must equal the declared target.

    The README paragraph is the single source of truth. This catches the
    historical split where the E2E pom said 3.2.4, the server config said
    3.2.3 and the inventory headings said 3.3.0-SNAPSHOT / 3.3.0-RC.
    """
    if not README.exists():
        return None
    target_match = TARGET_RE.search(README.read_text())
    if not target_match:
        problems.append(
            f"{README.relative_to(ROOT)}: missing 'Alignment target: Nacos X.Y.Z GA' declaration"
        )
        return None
    target = target_match.group(1)
    for rel, pattern, required in VERSION_DECLARATIONS:
        path = ROOT / rel
        if not path.exists():
            if required:
                problems.append(f"baseline version: missing declaration file {rel}")
            continue
        m = pattern.search(path.read_text())
        if not m:
            problems.append(f"baseline version: no version declaration found in {rel}")
        elif m.group(1) != target:
            problems.append(
                f"baseline version: {rel} declares {m.group(1)}, alignment target is {target}"
            )
    return target


def main(argv: list[str]) -> int:
    feature_files = [Path(a) for a in argv] if argv else sorted(COMPAT.rglob("features.md"))
    problems: list[str] = []
    seen: dict[str, str] = {}
    per_module: dict[tuple[str, str], set[int]] = defaultdict(set)
    counts: Counter[str] = Counter()
    tallies: dict[str, tuple[int, Counter[str]]] = {}
    anchor_notes: list[str] = []
    declared_target: str | None = None

    for f in feature_files:
        if not f.exists():
            problems.append(f"{f}: file not found")
            continue
        label = str(f.relative_to(ROOT)) if f.is_relative_to(ROOT) else str(f)
        try:
            lines = f.read_text().splitlines()
        except OSError as e:
            problems.append(f"{f}: {e}")
            continue

        rows = 0
        tally: Counter[str] = Counter()
        green = 0
        green_anchored = 0
        missing_anchors: list[str] = []

        for ln, line in enumerate(lines, 1):
            if not ROW_RE.match(line):
                continue
            rows += 1
            m = ID_RE.search(line)
            assert m, line
            fid = m.group(0)
            proto, mod, num = m.group(1), m.group(2), int(m.group(3))

            if fid in seen:
                problems.append(f"{label}:{ln}: duplicate feature id {fid} (also in {seen[fid]})")
            else:
                seen[fid] = label
            per_module[(proto, mod)].add(num)

            st = row_status(line)
            if st not in STATUS_SET:
                problems.append(f"{label}:{ln}: {fid} invalid status cell {st!r}")
                continue
            tally[st] += 1
            if st == "🟢":
                green += 1
                if crate_anchored(line):
                    green_anchored += 1
                else:
                    missing_anchors.append(f"{fid}@{ln}")

        counts[label] = rows
        tallies[f.parent.name] = (rows, tally)

        # Check 7: only enforce anchors in inventories that actually maintain them.
        # Nacos fills `batata impl` for every 🟢 row; Consul/Apollo leave it empty
        # by convention, so requiring it there would produce ~350 false positives.
        if green and green_anchored / green >= 0.8:
            if missing_anchors:
                problems.append(
                    f"{label}: {len(missing_anchors)} 🟢 row(s) without a resolvable crate anchor: "
                    + ", ".join(missing_anchors[:8])
                )
            anchor_notes.append(f"{f.parent.name} {green_anchored}/{green} anchored")
        else:
            anchor_notes.append(f"{f.parent.name} anchors not maintained (skipped)")

        # Check 5: Summary **Total** row must match the recomputed values.
        total_lines = [(ln, line) for ln, line in enumerate(lines, 1) if TOTAL_RE.match(line)]
        if not total_lines:
            problems.append(f"{label}: missing Summary **Total** row")
        else:
            ln, line = total_lines[-1]
            c = cells(line)
            if len(c) < 8:
                problems.append(f"{label}:{ln}: Summary **Total** row has too few cells")
            else:
                got = [number(x) for x in c[1:7]]
                expected = [tally[s] for s in STATUS] + [rows]
                if got != expected:
                    problems.append(
                        f"{label}:{ln}: Summary statuses/total {got} != recomputed {expected}"
                    )
                rate = number(c[7])
                if rate != pct(tally["🟢"], rows):
                    problems.append(
                        f"{label}:{ln}: Summary impl rate {rate}% != "
                        f"{pct(tally['🟢'], rows)}%"
                    )

    # Check 3: contiguity.
    for (proto, mod), nums in sorted(per_module.items()):
        n = len(nums)
        if nums != set(range(1, n + 1)):
            missing = sorted(set(range(1, max(nums) + 1)) - nums)
            problems.append(f"module {proto}-{mod}: {n} ids, max {max(nums)}, missing {missing}")

    # Known feature ids from *all* inventories, independent of argv.
    known_ids: set[str] = set()
    for f in sorted(COMPAT.rglob("features.md")):
        for line in f.read_text().splitlines():
            if ROW_RE.match(line):
                m = ID_RE.search(line)
                if m:
                    known_ids.add(m.group(0))

    # Check 8: every F-... reference must resolve to a registered feature.
    for f in sorted(COMPAT.rglob("*.md")):
        label = str(f.relative_to(ROOT))
        for ln, line in enumerate(f.read_text().splitlines(), 1):
            for ref in FID_REF_RE.findall(line):
                if ref not in known_ids:
                    problems.append(f"{label}:{ln}: dangling F-ID reference {ref}")

    # Check 6: README global summary.
    if README.exists():
        readme_lines = README.read_text().splitlines()
        expected: dict[str, list[int]] = {}
        for proto, name in PROTO_NAMES.items():
            rows, tally = tallies.get(proto, (0, Counter()))
            tests_md = COMPAT / proto / "tests.md"
            t_rows = (
                sum(1 for line in tests_md.read_text().splitlines() if TROW_RE.match(line))
                if tests_md.exists()
                else 0
            )
            expected[name.lower()] = (
                [rows] + [tally[s] for s in STATUS] + [pct(tally["🟢"], rows), t_rows]
            )
        if expected:
            # v = [features, 5 status counts, impl rate, T-ID rows]
            totals = [sum(v[i] for v in expected.values()) for i in range(8)]
            expected["total"] = totals[:6] + [pct(totals[1], totals[0]), totals[7]]

        found: set[str] = set()
        for ln, line in enumerate(readme_lines, 1):
            m = GS_ROW_RE.match(line)
            if not m:
                continue
            key = m.group(1).strip("*").lower()
            if key not in expected:
                continue
            found.add(key)
            c = cells(line)
            if len(c) < 9:
                problems.append(f"{README.relative_to(ROOT)}:{ln}: global summary row has too few cells")
                continue
            got = [number(x) for x in c[1:9]]
            if got != expected[key]:
                problems.append(
                    f"{README.relative_to(ROOT)}:{ln}: global summary {key} {got} "
                    f"!= recomputed {expected[key]}"
                )
        for key in expected:
            if key not in found:
                problems.append(f"{README.relative_to(ROOT)}: missing global summary row for {key}")

        # Check 10: one declared alignment target across pom/config/headings.
        declared_target = check_baseline_versions(problems)

        # Check 9: backticked file links must resolve (placeholders and commands excluded).
        for ln, line in enumerate(readme_lines, 1):
            for m in PATH_REF_RE.finditer(line):
                raw = m.group(1)
                token = raw.split()[0] if raw.split() else raw
                token = re.sub(r":\d+(?:-\d+)?$", "", token)  # strip :line suffix
                if any(ch in token for ch in "<>{}"):
                    continue  # placeholder such as <timestamp>
                if "*" in token:
                    if not list(ROOT.glob(token)):
                        problems.append(f"{README.relative_to(ROOT)}:{ln}: dead path glob {token}")
                elif not (ROOT / token).exists():
                    problems.append(f"{README.relative_to(ROOT)}:{ln}: dead path link {token}")
    else:
        problems.append(f"{README}: file not found")

    if problems:
        for p in problems:
            print(f"  ✗ {p}", file=sys.stderr)
        print(f"\n{len(problems)} problem(s).", file=sys.stderr)
        return 1

    total_features = sum(counts.values())
    total_tests = sum(
        sum(1 for line in (COMPAT / proto / "tests.md").read_text().splitlines() if TROW_RE.match(line))
        for proto in PROTO_NAMES
        if (COMPAT / proto / "tests.md").exists()
    )
    print(
        f"OK: {len(counts)} inventory file(s), {total_features} feature rows, "
        f"{total_tests} T-ID rows; summaries, anchors and references consistent."
    )
    print("  anchor coverage: " + "; ".join(anchor_notes))
    print(f"  Nacos alignment target: {declared_target or '?'} GA (pom/config/headings agree)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
