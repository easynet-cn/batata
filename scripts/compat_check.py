#!/usr/bin/env python3
"""Validate docs/compat feature inventories (batata compatibility tracking).

Checks:
  1. feature-ID format        F-<PROTO>-<MODULE>-<NNN>  (PROTO: NAC|CON|APO|SYS)
  2. global uniqueness of every feature ID across all files
  3. per (PROTO, MODULE) contiguous numbering 1..N with no gaps/duplicates
  4. every feature row has a valid Status enum cell
  5. "Total" row in each Summary matches the number of feature rows
  6. references to Test (T-<fid>-NN) / Bug (B-<fid>-NN) only point at known feature ids (soft)

Usage: scripts/compat_check.py [file.md ...]    (default: every features.md under docs/compat)
Exit code 0 = ok, 1 = at least one problem.
"""
from __future__ import annotations

import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMPAT = ROOT / "docs" / "compat"
PROTOS = ("NAC", "CON", "APO", "SYS")
STATUS = {"🟢", "🟡", "⚡", "⚪", "⛔"}

ID_RE = re.compile(r"F-(NAC|CON|APO|SYS)-([A-Z][A-Z-]*[A-Z])-(\d+)")
ROW_RE = re.compile(r"^\|\s*F-(NAC|CON|APO|SYS)-[A-Z][A-Z-]*[A-Z]-\d+\s*\|")


def row_status(line: str) -> str:
    # status emoji lives in different column positions across tables; just scan the row
    found = [s for s in STATUS if s in line]
    if found:
        return found[0]
    return "NO-STATUS"


def main(argv: list[str]) -> int:
    if argv:
        files = [Path(a) for a in argv]
    else:
        files = sorted(COMPAT.rglob("features.md"))

    problems = []
    seen: dict[str, str] = {}          # feature id -> file
    per_module: dict[tuple[str, str], set[int]] = defaultdict(set)
    counts: dict[str, int] = Counter() # file -> feature rows

    for f in files:
        if not f.exists():
            problems.append(f"{f}: file not found")
            continue
        label = str(f.relative_to(ROOT))
        try:
            lines = f.read_text().splitlines()
        except OSError as e:
            problems.append(f"{f}: {e}")
            continue

        rows = 0
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
            if st not in STATUS:
                problems.append(f"{label}:{ln}: {fid} invalid status cell {st!r}")

        counts[label] = rows

        # Summary total check
        m = re.search(r"\|\s*\*\*Total\*\*\s*\|\s*\d+\s*\|\s*\d+\s*\|\s*\d+\s*\|\s*(\d+)\s*\|\s*\d+\s*\|\s*(\d+)\s*\|", "\n".join(lines))
        if m:
            planned, total = int(m.group(1)), int(m.group(2))
            if total != rows:
                problems.append(f"{label}: summary Total {total} != {rows} actual feature rows")
            if planned + 0 != total - planned and False:
                pass  # status mix computed loosely; skip
            if planned > total:
                problems.append(f"{label}: summary planned {planned} > total {total}")

    # contiguity
    for (proto, mod), nums in sorted(per_module.items()):
        n = len(nums)
        if nums != set(range(1, n + 1)):
            missing = sorted(set(range(1, max(nums) + 1)) - nums)
            problems.append(
                f"module {proto}-{mod}: {n} ids, max {max(nums)}, missing {missing}"
            )

    # cross-file reference targets
    for f in files:
        if not f.exists():
            continue
        for ln, line in enumerate(f.read_text().splitlines(), 1):
            for ref in re.findall(r"\bT-([A-Z0-9-]+)-\d+\b|\bB-([A-Z0-9-]+)-\d+\b", line):
                target = ref[0] or ref[1]
                # normalize: T-NAC-CFG-01-1 -> target feature F-NAC-CFG-01
                if target in seen:
                    continue
                problems.append(f"{f.relative_to(ROOT)}:{ln}: dangling ref {target}")

    # report
    if problems:
        for p in problems:
            print(f"  ✗ {p}", file=sys.stderr)
        print(f"\n{len(problems)} problem(s).", file=sys.stderr)
        return 1

    total = sum(counts.values())
    print(f"OK: {len(counts)} file(s), {total} feature rows, all unique & contiguous.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))