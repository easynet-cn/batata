#!/usr/bin/env python3
"""Fail the regression gate on Raft health anomalies (task T5).

The Raft telemetry module emits structured events with a stable
``raft_event=<name>`` field. Anomalies are WARN-level and therefore already
appear in ``matrix.log``: ``scripts/test.sh`` calls ``scan_logs`` at the end of
every matrix combination and prints every WARN/ERROR line from the three node
logs. INFO-level events only appear in the node stdout logs, which are used
here as a liveness signal -- if telemetry never started, "zero anomalies"
would be a false green, so the check fails closed.

Usage:
    python3 scripts/raft_health_check.py <matrix.log> [<node stdout.log> ...]
    python3 scripts/raft_health_check.py --self-test

Exit codes: 0 = healthy, 1 = anomaly found, 2 = telemetry liveness missing,
3 = usage/IO error.
"""

from __future__ import annotations

import re
import sys
from collections import Counter
from pathlib import Path

# Any of these turning up in a full gate run is a blocker, not a known flake.
ANOMALY_EVENTS = (
    "steady_state_election",
    "apply_wait_timeout",
    "client_write_timeout",
    "leaderless_give_up",
)
LIVENESS_EVENT = "telemetry_started"

# tracing's fmt layer renders string fields unquoted (foo=bar); accept quoted
# and colon-separated variants so a formatter change cannot silence the gate.
EVENT_RE = re.compile(r"""raft_event\s*[=:]\s*["']?([a-z_]+)["']?""")
# The console formatter colours field names and the '=' sign separately
# (ESC[3mraft_event ESC[0m ESC[2m= ESC[0m"value"). Strip SGR sequences first
# or a coloured run would look perfectly healthy to this gate.
ANSI_RE = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
# matrix.log names every combination it ran. Standalone deployments have no
# Raft node, so telemetry is not expected there.
CLUSTER_COMBO_RE = re.compile(r"\[cluster-(?:rock|mysql|postgres)\]")


def count_events(text: str) -> Counter:
    """Count structured Raft health events in one log body."""
    return Counter(EVENT_RE.findall(text))


def read_logs(paths: list[str]) -> tuple[str, str]:
    """Return (aggregated matrix log, concatenated node logs)."""
    matrix_parts: list[str] = []
    node_parts: list[str] = []
    for index, raw in enumerate(paths):
        path = Path(raw)
        if not path.is_file():
            raise OSError(f"missing log file: {path}")
        body = path.read_text(encoding="utf-8", errors="replace")
        (matrix_parts if index == 0 else node_parts).append(body)
    return "\n".join(matrix_parts), "\n".join(node_parts)


def check(matrix_text: str, node_text: str) -> tuple[int, str]:
    """Return (exit code, human-readable summary)."""
    matrix_text = ANSI_RE.sub("", matrix_text)
    node_text = ANSI_RE.sub("", node_text)
    # matrix.log aggregates WARN lines from all six combinations, so it is the
    # authoritative anomaly source. Node logs are checked too, to cover the
    # last combination whose WARNs may not have been flushed into matrix.log.
    anomalies = count_events(matrix_text) + count_events(node_text)
    found = {name: anomalies.get(name, 0) for name in ANOMALY_EVENTS}
    total = sum(found.values())

    healthy = count_events(matrix_text).get(LIVENESS_EVENT, 0) + count_events(
        node_text
    ).get(LIVENESS_EVENT, 0)
    cluster_expected = bool(CLUSTER_COMBO_RE.search(matrix_text))
    if healthy == 0 and cluster_expected:
        return 2, (
            "telemetry liveness missing (no raft_event=telemetry_started found) "
            "although cluster combinations ran; cannot conclude the cluster was healthy"
        )

    detail = ", ".join(f"{name}={count}" for name, count in found.items())
    if total > 0:
        return 1, f"{total} Raft health anomaly event(s): {detail}"
    scope = "telemetry active" if healthy else "standalone-only run, no Raft node"
    return 0, f"0 Raft health anomalies ({detail}), {scope}"


def self_test() -> int:
    """Fixture-based tests; a mutant that ignores an event must fail here."""
    clean_nodes = "INFO batata_raft_health: raft_event=telemetry_started node_id=1\n"
    clean_matrix = "[MATRIX] ===== [cluster-rock] 开始 (mode=cluster) =====\n"
    standalone_matrix = "[MATRIX] ===== [standalone-rock] 开始 (mode=standalone) =====\n"
    cases = [
        ("clean run passes", clean_matrix, clean_nodes, 0),
        (
            "steady-state election is a blocker",
            clean_matrix + "WARN raft_event=steady_state_election node_id=1\n",
            clean_nodes,
            1,
        ),
        (
            "apply wait timeout is a blocker",
            clean_matrix + "WARN raft_event=apply_wait_timeout elapsed_ms=7000\n",
            clean_nodes,
            1,
        ),
        (
            "client write timeout is a blocker",
            clean_matrix + "WARN raft_event=client_write_timeout elapsed_ms=14000\n",
            clean_nodes,
            1,
        ),
        (
            "leaderless give-up is a blocker",
            clean_matrix + "WARN raft_event=leaderless_give_up waited_ms=7000\n",
            clean_nodes,
            1,
        ),
        (
            "missing liveness fails closed",
            clean_matrix,
            "no telemetry here\n",
            2,
        ),
        (
            "quoted field is still detected",
            clean_matrix + 'WARN raft_event="steady_state_election"\n',
            clean_nodes,
            1,
        ),
        (
            "colon-separated field is still detected",
            clean_matrix + "WARN raft_event: apply_wait_timeout\n",
            clean_nodes,
            1,
        ),
        (
            "unrelated warnings are not anomalies",
            clean_matrix + "[WARNING] Tests run: 531, Failures: 0\n",
            clean_nodes,
            0,
        ),
        (
            "standalone-only run needs no telemetry",
            standalone_matrix,
            "no telemetry here\n",
            0,
        ),
        (
            "cluster run without telemetry fails closed",
            "[MATRIX] ===== [cluster-postgres] 开始 =====\n",
            "no telemetry here\n",
            2,
        ),
        (
            "ANSI-coloured fields are still detected",
            clean_matrix
            + 'WARN \x1b[3mraft_event\x1b[0m\x1b[2m=\x1b[0m"steady_state_election"\n',
            clean_nodes,
            1,
        ),
        (
            "ANSI-coloured liveness is still detected",
            clean_matrix,
            '\x1b[3mraft_event\x1b[0m\x1b[2m=\x1b[0m"telemetry_started"\n',
            0,
        ),
    ]
    failed = 0
    for label, matrix, nodes, expected in cases:
        code, summary = check(matrix, nodes)
        if code != expected:
            failed += 1
            print(f"[FAIL] {label}: expected {expected}, got {code} ({summary})")
        else:
            print(f"[PASS] {label}")
    print(f"self-test: {len(cases) - failed}/{len(cases)} passed")
    return 0 if failed == 0 else 1


def main(argv: list[str]) -> int:
    if len(argv) == 2 and argv[1] == "--self-test":
        return self_test()
    if len(argv) < 2:
        print(__doc__.strip(), file=sys.stderr)
        return 3
    try:
        matrix_text, node_text = read_logs(argv[1:])
    except OSError as exc:
        print(f"raft health check: {exc}", file=sys.stderr)
        return 3
    code, summary = check(matrix_text, node_text)
    print(summary)
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv))
