#!/usr/bin/env python3
"""Observability helpers for the regression gate's performance stage.

A perf number without its host context is not attributable: five metrics can
all drop together because the machine was busy, and a run that only kept
``perf-*.log`` cannot be diagnosed afterwards. This module provides three
things, all stdlib-only so the check stage can exercise them without a server:

1. ``capture_host_state`` appends a JSON line with load average, CPU count and
   free disk space before/after each measured round.
2. ``server_log_paths`` gives every (case, variant, round, node) its own log
   file instead of overwriting one fixed ``/tmp/bench_ab_cur.log.nodeN`` per
   round.
3. ``cap_log_file`` bounds each server log. The default INFO level emits two
   lines per config write; a single cluster node reached 66 MiB in one run.
   Truncation keeps the head (startup/telemetry liveness), the tail (shutdown)
   and every health-signal line from the middle, so ``raft_health_check.py``
   still has the evidence it needs.

Usage:
    python3 scripts/perf_observability.py --snapshot <file> <label>
    python3 scripts/perf_observability.py --self-test

The self-test also guards the shell wiring: dropping ``--log-dir`` from
``regression.sh`` or the self-test invocation from ``stage_check`` turns the
check stage red instead of silently disabling perf observability.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
DEFAULT_MAX_BYTES = 4 * 1024 * 1024

_SAFE = re.compile(r"[^A-Za-z0-9._-]+")


def _safe(value: str) -> str:
    """Make a case/variant name usable as a path component."""
    return _SAFE.sub("-", value).strip("-") or "unnamed"


def _memory_available_mb() -> int | None:
    """Best-effort host memory reading; ``None`` when not available."""
    proc_meminfo = Path("/proc/meminfo")
    if proc_meminfo.is_file():
        try:
            for line in proc_meminfo.read_text(encoding="utf-8").splitlines():
                if line.startswith("MemAvailable:"):
                    return int(line.split()[1]) // 1024
        except (OSError, ValueError, IndexError):
            pass
    if sys.platform == "darwin":
        try:
            out = subprocess.run(
                ["vm_stat"], capture_output=True, text=True, timeout=3
            ).stdout
            page_size_out = subprocess.run(
                ["sysctl", "-n", "hw.pagesize"],
                capture_output=True,
                text=True,
                timeout=3,
            ).stdout.strip()
            page_size = int(page_size_out)
            free = 0
            for line in out.splitlines():
                if line.startswith("Pages free:") or line.startswith(
                    "Pages inactive:"
                ):
                    free += int(line.split(":")[1].strip().rstrip("."))
            return free * page_size // (1024 * 1024)
        except (OSError, ValueError, subprocess.SubprocessError):
            pass
    return None


def host_state(label: str) -> dict[str, object]:
    """Build one host-state record. Never raises on an unsupported platform."""
    try:
        load = [round(value, 3) for value in os.getloadavg()]
    except (AttributeError, OSError):  # pragma: no cover - non-Unix fallback
        load = []
    disk = shutil.disk_usage(REPO_ROOT)
    return {
        "ts": round(time.time(), 3),
        "iso": time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime()),
        "label": label,
        "host": platform.node(),
        "os": f"{platform.system()} {platform.release()}",
        "cpu_count": os.cpu_count(),
        "loadavg_1_5_15": load,
        "disk_total_gb": round(disk.total / 2**30, 2),
        "disk_free_gb": round(disk.free / 2**30, 2),
        "mem_available_mb": _memory_available_mb(),
    }


def capture_host_state(path: str | Path, label: str) -> dict[str, object]:
    """Append one host-state record as a JSON line."""
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    record = host_state(label)
    with target.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(record, sort_keys=True) + "\n")
    return record


def server_log_paths(
    log_dir: str | Path,
    case: str,
    variant: str,
    round_index: int,
    nodes: int,
    mode: str,
) -> list[str]:
    """Unique, sorted-friendly log path per (case, variant, round, node)."""
    base = Path(log_dir) / f"{_safe(case)}-{_safe(variant)}-r{int(round_index)}"
    if mode == "cluster":
        return [f"{base}.node{index}.log" for index in range(1, max(1, int(nodes)) + 1)]
    return [f"{base}.log"]


INTERESTING_RE = re.compile(rb"raft_event|WARN|ERROR|resource_pressure")


def cap_log_file(path: str | Path, max_bytes: int = DEFAULT_MAX_BYTES) -> dict[str, object]:
    """Bound a finished server log while preserving health-check evidence.

    Missing files and small files are left untouched. Oversized files are
    rewritten as head + health-signal lines + tail with explicit markers;
    ``raft_event`` lines take priority over other WARN/ERROR lines when the
    budget runs out.
    """
    target = Path(path)
    if max_bytes <= 0 or not target.is_file():
        return {"path": str(target), "capped": False, "original": None, "final": None}
    original = target.stat().st_size
    if original <= max_bytes:
        return {
            "path": str(target),
            "capped": False,
            "original": original,
            "final": original,
        }
    raw = target.read_bytes()
    third = max_bytes // 3
    head = raw[:third]
    tail = raw[-third:] if third else b""
    middle = raw[third : len(raw) - third] if third else raw
    marker = (
        f"\n[perf_observability] truncated {original} bytes to <= {max_bytes}; "
        "head/tail and health-signal lines retained\n"
    ).encode("ascii")
    budget = max(0, max_bytes - len(head) - len(tail) - 2 * len(marker) - 32)

    primary: list[bytes] = []
    secondary: list[bytes] = []
    for line in middle.splitlines(keepends=True):
        if not INTERESTING_RE.search(line):
            continue
        (primary if b"raft_event" in line else secondary).append(line)

    kept: list[bytes] = []
    used = 0
    for line in [*primary, *secondary]:
        if used + len(line) > budget:
            break
        kept.append(line)
        used += len(line)

    out = head + marker + b"".join(kept) + marker + tail
    if len(out) > max_bytes:
        overflow = len(out) - max_bytes
        tail = tail[overflow:] if overflow < len(tail) else b""
        out = head + marker + b"".join(kept) + marker + tail
    target.write_bytes(out)
    return {
        "path": str(target),
        "capped": True,
        "original": original,
        "final": len(out),
        "signal_lines": len(kept),
    }


# ---------------------------------------------------------------------------
# Self-test: fixtures plus a static guard on the shell wiring.
# ---------------------------------------------------------------------------
def _stage_body(source: str, name: str) -> str:
    """Extract one bash function body (functions end with a bare `}`)."""
    match = re.search(rf"^{re.escape(name)}\(\) \{{(.*?)^\}}", source, re.S | re.M)
    return match.group(1) if match else ""


def self_test() -> int:
    cases: list[tuple[str, bool, str]] = []

    def record(label: str, ok: bool, detail: str = "") -> None:
        cases.append((label, bool(ok), detail))

    with tempfile.TemporaryDirectory(prefix="batata-perf-obs-") as tmp:
        root = Path(tmp)

        # 1. Host state snapshots are append-only, complete and ordered.
        state_log = root / "host-state.log"
        first = capture_host_state(state_log, "before-naming")
        second = capture_host_state(state_log, "after-naming")
        lines = [
            json.loads(line)
            for line in state_log.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        required = {
            "ts",
            "iso",
            "label",
            "host",
            "os",
            "cpu_count",
            "loadavg_1_5_15",
            "disk_total_gb",
            "disk_free_gb",
            "mem_available_mb",
        }
        record("host state writes two JSON lines", len(lines) == 2)
        record(
            "host state records all fields",
            all(required.issubset(line) for line in lines),
        )
        record(
            "host state labels are distinct and ordered",
            [line["label"] for line in lines] == ["before-naming", "after-naming"]
            and first["ts"] <= second["ts"],
        )
        record(
            "host state has load average and disk space",
            isinstance(lines[0]["loadavg_1_5_15"], list)
            and lines[0]["disk_free_gb"] > 0
            and lines[0]["cpu_count"] > 0,
        )

        # 2. Log paths are unique per round and node, and stay in the log dir.
        paths_r1 = server_log_paths(root, "config", "cur", 1, 3, "cluster")
        paths_r2 = server_log_paths(root, "config", "cur", 2, 3, "cluster")
        standalone = server_log_paths(root, "naming", "cur", 1, 1, "standalone")
        record(
            "cluster log paths are per-node and inside log dir",
            len(paths_r1) == 3
            and all(Path(p).parent == root for p in paths_r1)
            and all(".node" in p for p in paths_r1),
        )
        record(
            "log paths never overwrite across rounds",
            len(set(paths_r1) & set(paths_r2)) == 0
            and "-r1" in paths_r1[0]
            and "-r2" in paths_r2[0],
        )
        record(
            "standalone has exactly one log without a node suffix",
            len(standalone) == 1 and ".node" not in standalone[0],
        )
        record(
            "case/variant names are path-safe",
            server_log_paths(root, "a/b c", "v:1", 1, 1, "standalone")[0].startswith(
                str(root / "a-b-c-v-1-r1")
            ),
        )

        # 3. Capping keeps head, tail and health signals, and stays bounded.
        big = root / "oversized.log"
        with big.open("wb") as handle:
            handle.write(b"HEAD telemetry marker\n")
            handle.write(b'INFO batata_raft_health: raft_event="telemetry_started" node_id=1\n')
            handle.write(b"filler line\n" * 40000)
            handle.write(
                b'WARN batata_raft_health: raft_event="apply_wait_timeout" cpu_pct=91.5\n'
            )
            handle.write(b"filler line\n" * 40000)
            handle.write(b"ordinary WARN without health fields\n")
            handle.write(b"TAIL shutdown marker\n")
        original = big.stat().st_size
        info = cap_log_file(big, 8192)
        body = big.read_bytes()
        record("oversized log is capped", info["capped"] and original > 8192)
        record("capped log stays within budget", big.stat().st_size <= 8192)
        record(
            "capped log keeps head and tail markers",
            b"HEAD telemetry marker" in body and b"TAIL shutdown marker" in body,
        )
        record(
            "capped log keeps telemetry liveness",
            b"telemetry_started" in body,
        )
        record(
            "capped log keeps anomaly signals from the middle",
            b"apply_wait_timeout" in body and b"cpu_pct=91.5" in body,
        )
        record("capped log is explicitly marked", b"[perf_observability] truncated" in body)

        small = root / "small.log"
        small.write_bytes(b"small\n")
        unchanged = cap_log_file(small, 8192)
        record(
            "small log is left byte-identical",
            not unchanged["capped"] and small.read_bytes() == b"small\n",
        )
        record(
            "missing log is a no-op",
            not cap_log_file(root / "missing.log", 8192)["capped"],
        )

    # 4. Guard the shell wiring: a dropped flag must fail the check stage.
    regression = SCRIPT_DIR / "regression.sh"
    source = regression.read_text(encoding="utf-8") if regression.is_file() else ""
    perf_body = _stage_body(source, "stage_perf")
    check_body = _stage_body(source, "stage_check")
    for token in (
        "perf_observability.py",
        "--snapshot",
        "--log-dir",
        "--host-state-log",
        "--server-log-max-bytes",
        "raft_health_check.py",
        "perf-host-state.log",
    ):
        record(f"stage_perf wires {token}", token in perf_body)
    for token in (
        "perf_observability.py",
        "--self-test",
        "check-perf-observability.log",
        "SELF-TEST PASS",
    ):
        record(f"stage_check wires {token}", token in check_body)

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
    print("SELF-TEST PASS: perf observability helpers and shell wiring intact")
    return 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--snapshot",
        nargs=2,
        metavar=("FILE", "LABEL"),
        help="append one host-state record to FILE",
    )
    parser.add_argument(
        "--self-test", action="store_true", help="run fixtures and wiring guards"
    )
    args = parser.parse_args(argv[1:])
    if args.self_test:
        return self_test()
    if args.snapshot:
        capture_host_state(args.snapshot[0], args.snapshot[1])
        return 0
    parser.print_help()
    return 3


if __name__ == "__main__":
    sys.exit(main(sys.argv))
