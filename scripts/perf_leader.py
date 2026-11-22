#!/usr/bin/env python3
"""Attribute every perf round to the Raft leader that actually served it.

``bench_ab.py`` sends all load to one cluster node (``--write-node``, default
node 1). A cold-start Raft election can pick any node as leader, so a write
either runs ``client_write`` locally on the load target or is forwarded over
gRPC to the leader and then waits for the local state machine to catch up.
Those are two different code paths with roughly 5x different throughput; a
median over both describes neither of them. The path is therefore derived
from ``(leader, write_node)``, never from the load target alone.

This module reads the per-(case, variant, round, node) server logs that
``perf_observability.py`` already archives and resolves the steady-state leader
for each round from the telemetry emitted by ``raft/telemetry.rs``:

    raft_event="leader_changed" ... node_id=N term=T ... to_leader_id=L
    raft_event="election_settled" ... node_id=N term=T role="leader"

The result is written into the perf CSV as ``# meta`` lines and into a JSONL
artifact, so ``regression.sh`` can print the attribution next to any
INCONCLUSIVE verdict instead of leaving the reader to guess. This is
observability only: it does not change any gate, anchor or Raft behaviour.

Self-test validates the parser against the exact log format captured on
2026-11-21 (including ANSI colour codes) and guards the shell/bench wiring.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
REGRESSION_SH = SCRIPT_DIR / "regression.sh"
BENCH_AB = SCRIPT_DIR / "bench_ab.py"
README = SCRIPT_DIR / "README.md"

UNKNOWN = "unknown"
STANDALONE = "standalone"
LEADER_LOCAL = "leader-local"
FORWARDED = "forwarded"
PATH_UNKNOWN = "path-unknown"
INVALID_WRITE_NODE = "invalid-write-node"
DEFAULT_WRITE_NODE = 1
LEADER_LOG_NAME = "leader-attribution.log"

ANSI = re.compile(r"\x1b\[[0-9;]*m")
TIMESTAMP = re.compile(r"^(\d{4}-\d{2}-\d{2}T[0-9:.]+Z)")
LEADER_CHANGED = re.compile(
    r'raft_event="leader_changed".*?node_id=(\d+).*?term=(\d+).*?to_leader_id=(-?\d+)'
)
ELECTION_SETTLED = re.compile(
    r'raft_event="election_settled".*?node_id=(\d+).*?term=(\d+).*?role="(\w+)"'
)
# The trailing write-node field was added by the dual-path split. Older CSVs
# have only six fields and default to node 1, which was the only load target
# those runs could have used. Never reinterpret historical numbers.
META_LEADER = re.compile(
    r"^# meta leader\|([^|]+)\|r(\d+)\|([^|]+)\|([^|]+)\|([^|]+)\|([^|]+)"
    r"(?:\|([^|\s]+))?\s*$"
)
META_BUILD = re.compile(
    r"^# meta build\|([^|]+)\|([^|]+)\|([^|]+)\|([^|]+)\|([^|]+)\s*$"
)


def strip_ansi(text: str) -> str:
    """Remove terminal colour codes so the field regexes see plain text."""
    return ANSI.sub("", text)


def parse_log(path: str | Path) -> list[dict[str, object]]:
    """Return leader-related events from one server log, in file order.

    A missing or unreadable log contributes no evidence; callers must treat
    that as ``unknown`` rather than guessing node 1.
    """
    target = Path(path)
    try:
        lines = target.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return []
    events: list[dict[str, object]] = []
    for raw in lines:
        line = strip_ansi(raw)
        stamp = TIMESTAMP.match(line)
        changed = LEADER_CHANGED.search(line)
        if changed:
            events.append({
                "ts": stamp.group(1) if stamp else "",
                "kind": "leader_changed",
                "node_id": int(changed.group(1)),
                "term": int(changed.group(2)),
                "leader_id": int(changed.group(3)),
                "source_log": target.name,
            })
            continue
        settled = ELECTION_SETTLED.search(line)
        if settled:
            events.append({
                "ts": stamp.group(1) if stamp else "",
                "kind": "election_settled",
                "node_id": int(settled.group(1)),
                "term": int(settled.group(2)),
                "role": settled.group(3),
                "source_log": target.name,
            })
    return events


def _validate_leader(leader: int | None, nodes: int) -> int | None:
    """A leader outside the configured cluster is not usable attribution."""
    if leader is None or leader < 1 or leader > nodes:
        return None
    return leader


def resolve_leader(
    paths: list[str] | list[Path],
    nodes: int,
    mode: str = "cluster",
    write_node: int = DEFAULT_WRITE_NODE,
) -> dict[str, object]:
    """Resolve the steady-state leader from a round's archived server logs.

    Priority: the last non-negative ``to_leader_id`` (a settled leadership
    change), then the last ``election_settled`` with ``role="leader"``.
    ``to_leader_id=-1`` means the group lost its leader, so it is never used
    as the answer. No evidence yields ``unknown``; attribution must not infer
    a leader from the load target.

    ``write_node`` is the node that actually received the load. A write is
    ``leader-local`` only when that node *is* the leader; otherwise it goes
    through gRPC forwarding. Hardcoding node 1 here would mislabel every run
    that aims wrk at another node (the dual-path split).
    """
    if mode != "cluster":
        present = sum(1 for path in paths if Path(path).is_file())
        return {
            "mode": mode,
            "leader_node": None,
            "term": None,
            "write_path": STANDALONE,
            "source": "no-raft",
            "events": 0,
            "logs_present": present,
            "write_node": int(write_node),
        }

    events: list[dict[str, object]] = []
    present = 0
    for path in paths:
        if Path(path).is_file():
            present += 1
        events.extend(parse_log(path))
    events.sort(key=lambda item: (str(item.get("ts", "")), str(item.get("source_log", ""))))

    leader = None
    term = None
    source = "none"
    for event in reversed(events):
        if event["kind"] == "leader_changed" and int(event["leader_id"]) >= 1:
            leader = int(event["leader_id"])
            term = int(event["term"])
            source = "leader_changed"
            break
    if leader is None:
        for event in reversed(events):
            if event["kind"] == "election_settled" and event["role"] == "leader":
                leader = int(event["node_id"])
                term = int(event["term"])
                source = "election_settled"
                break

    leader = _validate_leader(leader, nodes)
    valid_target = 1 <= int(write_node) <= int(nodes)
    if leader is None or not valid_target:
        write_path = PATH_UNKNOWN
        if not valid_target:
            source = INVALID_WRITE_NODE
        else:
            source = "none" if source == "none" else f"invalid:{source}"
    elif leader == int(write_node):
        write_path = LEADER_LOCAL
    else:
        write_path = FORWARDED
    return {
        "mode": mode,
        "leader_node": leader,
        "term": term,
        "write_path": write_path,
        "source": source,
        "events": len(events),
        "logs_present": present,
        "write_node": int(write_node),
    }


def capture_round(
    log_dir: str | Path,
    case: str,
    variant: str,
    round_index: int,
    nodes: int,
    mode: str,
    paths: list[str] | None = None,
    write_node: int = DEFAULT_WRITE_NODE,
) -> dict[str, object]:
    """Resolve one round and attach its identity for machine consumption."""
    if paths is None:
        from perf_observability import server_log_paths
        paths = server_log_paths(log_dir, case, variant, round_index, nodes, mode)
    result = resolve_leader(paths, nodes, mode, write_node)
    result.update({
        "case": case,
        "variant": variant,
        "round": int(round_index),
        "nodes": int(nodes),
    })
    return result


def build_identity(mode: str, nodes: int) -> dict[str, object]:
    """Record the exact source revision that produced a perf number."""
    sha = "unknown"
    dirty = "unknown"
    try:
        sha = subprocess.run(
            ["git", "-C", str(REPO_ROOT), "rev-parse", "--short", "HEAD"],
            capture_output=True, text=True, timeout=5, check=False,
        ).stdout.strip() or "unknown"
    except (OSError, subprocess.SubprocessError):
        pass
    try:
        status = subprocess.run(
            ["git", "-C", str(REPO_ROOT), "status", "--porcelain"],
            capture_output=True, text=True, timeout=5, check=False,
        )
        dirty = "dirty" if status.stdout.strip() else "clean"
    except (OSError, subprocess.SubprocessError):
        pass
    return {"sha": sha, "dirty": dirty, "mode": mode, "nodes": int(nodes)}


def format_build_line(identity: dict[str, object]) -> str:
    """One CSV meta line describing the binary's source revision."""
    return (f"# meta build|{identity['sha']}|{identity['dirty']}|"
            f"{identity['mode']}|{identity['nodes']}|batata-server")


def format_meta_line(record: dict[str, object]) -> str:
    """One CSV meta line describing which leader served a recorded round.

    The trailing ``write_node`` makes the path label self-describing: a
    ``leader-local`` verdict means the load target and the leader are the same
    node, not merely "node 1 happened to win the election".
    """
    leader = record["leader_node"]
    label = UNKNOWN if leader is None else f"node{leader}"
    term = record["term"] if record["term"] is not None else UNKNOWN
    write_node = record.get("write_node", DEFAULT_WRITE_NODE)
    return (f"# meta leader|{record['variant']}|r{record['round']}|"
            f"{label}|{record['write_path']}|{term}|{record['source']}|"
            f"{write_node}")


def write_jsonl(path: str | Path, record: dict[str, object]) -> None:
    """Append one structured attribution record."""
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    with target.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(record, sort_keys=True) + "\n")


def parse_meta(text: str) -> list[dict[str, object]]:
    """Read back ``# meta leader`` lines from a perf CSV."""
    rows: list[dict[str, object]] = []
    for line in text.splitlines():
        match = META_LEADER.match(line)
        if not match:
            continue
        variant, round_no, leader, write_path, term, source, write_node = match.groups()
        rows.append({
            "variant": variant,
            "round": int(round_no),
            "leader": leader,
            "write_path": write_path,
            "term": term,
            "source": source,
            "write_node": int(write_node) if write_node else DEFAULT_WRITE_NODE,
        })
    return rows


def evaluate_csv(path: str | Path) -> str:
    """Pipe-delimited attribution rows; ``NO_LEADER_META`` for old CSVs."""
    try:
        text = Path(path).read_text(encoding="utf-8", errors="replace")
    except OSError as error:
        raise ValueError(f"cannot read perf CSV: {error}") from error
    rows = parse_meta(text)
    if not rows:
        return f"NO_LEADER_META|{Path(path).name}"
    return "\n".join(
        f"{row['variant']}|r{row['round']}|{row['leader']}|"
        f"{row['write_path']}|{row['term']}|{row['source']}|w{row['write_node']}"
        for row in rows
    )


def _stage_body(source: str, name: str) -> str:
    """Extract one bash function body (functions end with a bare ``}``)."""
    match = re.search(rf"^{re.escape(name)}\(\) \{{(.*?)^\}}", source, re.S | re.M)
    return match.group(1) if match else ""


def _log(root: Path, name: str, lines: list[str]) -> Path:
    path = root / name
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


def self_test() -> int:
    """Numeric/format fixtures plus static guards on the wiring."""
    cases: list[tuple[str, bool, str]] = []

    def record(label: str, ok: bool, detail: str = "") -> None:
        cases.append((label, bool(ok), detail))

    ts = "2026-11-21T15:38:06.848770Z"
    prefix = " INFO main batata_raft_health: crates/batata-consistency/src/raft/telemetry.rs:492: "

    # 1. Parser against the exact captured log format, ANSI codes included.
    with tempfile.TemporaryDirectory(prefix="batata-perf-leader-") as tmp:
        root = Path(tmp)
        # This mirrors the real tracing-subscriber output: colour codes are
        # interleaved *inside* the field syntax (raft_event<esc>=<esc>"...").
        # A parser that forgets to strip them cannot match any field.
        real_line = (
            f"\x1b[2m{ts}\x1b[0m \x1b[32m INFO\x1b[0m main "
            f"\x1b[2mbatata_raft_health\x1b[0m\x1b[2m:\x1b[0m "
            f"\x1b[2mcrates/batata-consistency/src/raft/telemetry.rs\x1b[0m"
            f"\x1b[2m:\x1b[0m\x1b[2m492:\x1b[0m Raft leader changed "
            f"\x1b[3mraft_event\x1b[0m\x1b[2m=\x1b[0m\"leader_changed\" "
            f"\x1b[3mnode_id\x1b[0m\x1b[2m=\x1b[0m1 "
            f"\x1b[3mterm\x1b[0m\x1b[2m=\x1b[0m2 "
            f"\x1b[3mfrom_leader_id\x1b[0m\x1b[2m=\x1b[0m-1 "
            f"\x1b[3mto_leader_id\x1b[0m\x1b[2m=\x1b[0m3"
        )
        changed = _log(root, "node1.log", [real_line])
        events = parse_log(changed)
        record("ANSI colour codes are stripped",
               len(events) == 1 and events[0]["leader_id"] == 3, str(events))
        record("ANSI interleaved fields still parse",
               events and events[0]["kind"] == "leader_changed", str(events))
        record("leader_changed fields are parsed",
               events and events[0]["term"] == 2 and events[0]["node_id"] == 1)
        record("missing log contributes no evidence", parse_log(root / "nope.log") == [])

        # 2. Ground truth: 233043 r2 leader=node3 (forwarded), r3 leader=node1.
        n1 = _log(root, "r2.node1.log", [
            f"{ts}{prefix}Raft leader changed raft_event=\"leader_changed\" "
            "node_id=1 term=2 from_leader_id=3 to_leader_id=-1",
            f"{ts}6Z{prefix}Raft leader changed raft_event=\"leader_changed\" "
            "node_id=1 term=2 from_leader_id=-1 to_leader_id=3",
        ])
        n2 = _log(root, "r2.node2.log", [
            f"{ts}{prefix}Raft leader changed raft_event=\"leader_changed\" "
            "node_id=2 term=2 from_leader_id=-1 to_leader_id=3",
        ])
        n3 = _log(root, "r2.node3.log", [
            f"{ts}{prefix}Raft election settled raft_event=\"election_settled\" "
            'node_id=3 term=2 role="leader" elapsed_ms=2',
        ])
        r2 = resolve_leader([str(n1), str(n2), str(n3)], 3, "cluster")
        record("233043 r2 resolves to node 3", r2["leader_node"] == 3, str(r2))
        record("233043 r2 is the forwarded path", r2["write_path"] == FORWARDED)
        record("233043 r2 prefers leader_changed", r2["source"] == "leader_changed")
        record("233043 r2 keeps the term", r2["term"] == 2)
        record("233043 r2 counts every log", r2["logs_present"] == 3)

        r3n1 = _log(root, "r3.node1.log", [
            f"{ts}{prefix}Raft election settled raft_event=\"election_settled\" "
            'node_id=1 term=3 role="leader" elapsed_ms=2',
        ])
        r3 = resolve_leader([str(r3n1)], 3, "cluster")
        record("233043 r3 resolves to node 1", r3["leader_node"] == 1, str(r3))
        record("233043 r3 is the leader-local path", r3["write_path"] == LEADER_LOCAL)
        record("election_settled is the fallback source",
               r3["source"] == "election_settled")

        # 2b. Dual-path split: the path follows (leader, write_node). A
        # hardcoded "leader == 1" would mislabel both of the runs below.
        on_leader = resolve_leader([str(n1), str(n2), str(n3)], 3,
                                   "cluster", write_node=3)
        record("load on the leader is leader-local",
               on_leader["write_path"] == LEADER_LOCAL, str(on_leader))
        record("attribution keeps the load target", on_leader["write_node"] == 3)
        on_follower = resolve_leader([str(n1), str(n2), str(n3)], 3,
                                     "cluster", write_node=2)
        record("load on a follower is forwarded",
               on_follower["write_path"] == FORWARDED, str(on_follower))
        r3_follower = resolve_leader([str(r3n1)], 3, "cluster", write_node=2)
        record("leader=node1 but load on node2 is forwarded",
               r3_follower["write_path"] == FORWARDED, str(r3_follower))
        bad_target = resolve_leader([str(n1)], 3, "cluster", write_node=9)
        record("out-of-range load target is not attributed",
               bad_target["write_path"] == PATH_UNKNOWN
               and bad_target["source"] == INVALID_WRITE_NODE, str(bad_target))
        zero_target = resolve_leader([str(n1)], 3, "cluster", write_node=0)
        record("zero load target is not attributed",
               zero_target["write_path"] == PATH_UNKNOWN)
        standalone_target = resolve_leader([str(changed)], 1,
                                           "standalone", write_node=1)
        record("standalone keeps its load target",
               standalone_target["write_node"] == 1)

        # 3. 204500 r1/r2: only election_settled, different leaders.
        for round_no, node, expected_path in ((1, 3, FORWARDED), (2, 1, LEADER_LOCAL)):
            solo = _log(root, f"a{round_no}.node{node}.log", [
                f"{ts}{prefix}Raft election settled raft_event=\"election_settled\" "
                f'node_id={node} term={round_no + 3} role="leader" elapsed_ms=2',
            ])
            got = resolve_leader([str(solo)], 3, "cluster")
            record(f"204500 r{round_no} resolves to node {node}",
                   got["leader_node"] == node, str(got))
            record(f"204500 r{round_no} path is {expected_path}",
                   got["write_path"] == expected_path)

        # 4. Fail closed: no evidence, lost leader, out-of-range ids.
        empty = resolve_leader([str(_log(root, "plain.log", ["nothing to see"]))], 3)
        record("no telemetry is unknown", empty["leader_node"] is None, str(empty))
        record("no telemetry has no guessed path", empty["write_path"] == PATH_UNKNOWN)
        lost = _log(root, "lost.log", [
            f"{ts}{prefix}Raft leader changed raft_event=\"leader_changed\" "
            "node_id=1 term=2 from_leader_id=3 to_leader_id=-1",
        ])
        record("to_leader_id=-1 is not a leader",
               resolve_leader([str(lost)], 3)["leader_node"] is None)
        gain_then_loss = _log(root, "gain-loss.log", [
            f"{ts}{prefix}Raft leader changed raft_event=\"leader_changed\" "
            "node_id=1 term=2 from_leader_id=-1 to_leader_id=3",
            f"{ts}9Z{prefix}Raft leader changed raft_event=\"leader_changed\" "
            "node_id=1 term=3 from_leader_id=3 to_leader_id=-1",
        ])
        trailing_loss = resolve_leader([str(gain_then_loss)], 3)
        record("a trailing leader loss keeps the last known leader",
               trailing_loss["leader_node"] == 3
               and trailing_loss["write_path"] == FORWARDED, str(trailing_loss))
        bad = _log(root, "bad.log", [
            f"{ts}{prefix}Raft leader changed raft_event=\"leader_changed\" "
            "node_id=1 term=2 from_leader_id=-1 to_leader_id=9",
        ])
        record("leader outside the cluster is rejected",
               resolve_leader([str(bad)], 3)["leader_node"] is None)
        follow = _log(root, "follow.log", [
            f"{ts}{prefix}Raft election settled raft_event=\"election_settled\" "
            'node_id=2 term=2 role="follower" elapsed_ms=2',
        ])
        record("a settled follower is not a leader",
               resolve_leader([str(follow)], 3)["leader_node"] is None)
        standalone = resolve_leader([str(changed)], 1, "standalone")
        record("standalone has no Raft leader", standalone["leader_node"] is None)
        record("standalone path is standalone", standalone["write_path"] == STANDALONE)

        # 5. Later evidence wins over an earlier election.
        old = f"2026-11-21T15:00:00.000000Z"
        new = f"2026-11-21T15:59:00.000000Z"
        first = _log(root, "first.log", [
            f"{old}{prefix}Raft leader changed raft_event=\"leader_changed\" "
            "node_id=1 term=1 from_leader_id=-1 to_leader_id=2",
        ])
        last = _log(root, "last.log", [
            f"{new}{prefix}Raft leader changed raft_event=\"leader_changed\" "
            "node_id=1 term=2 from_leader_id=2 to_leader_id=1",
        ])
        record("the steady leader is the last change",
               resolve_leader([str(last), str(first)], 3)["leader_node"] == 1)

        # 6. Meta lines round-trip and never invent data.
        identity = {"sha": "61086d17", "dirty": "clean", "mode": "cluster", "nodes": 3}
        build_line = format_build_line(identity)
        record("build line parses", META_BUILD.match(build_line) is not None)
        record("build line records the sha", "|61086d17|clean|cluster|3|" in build_line)
        live_identity = build_identity("cluster", 3)
        record("build identity resolves a real sha",
               str(live_identity["sha"]) not in ("", "unknown"))
        record("build identity reports clean/dirty",
               live_identity["dirty"] in ("clean", "dirty"))
        worktree = subprocess.run(
            ["git", "-C", str(REPO_ROOT), "status", "--porcelain"],
            capture_output=True, text=True, timeout=5, check=False,
        )
        record("build identity matches the worktree",
               live_identity["dirty"] == ("dirty" if worktree.stdout.strip() else "clean"))
        snap = capture_round(root, "config", "cur", 2, 3, "cluster",
                             [str(n1), str(n2), str(n3)])
        meta_line = format_meta_line(snap)
        record("meta line parses", META_LEADER.match(meta_line) is not None, meta_line)
        record("meta line names the leader", "|node3|forwarded|2|" in meta_line, meta_line)
        record("meta line records the load target",
               meta_line.endswith("|1"), meta_line)
        target_snap = dict(snap, write_node=3, write_path=LEADER_LOCAL)
        target_line = format_meta_line(target_snap)
        record("meta line records a non-default load target",
               target_line.endswith("|3"), target_line)
        legacy_line = (
            f"# meta leader|{snap['variant']}|r{snap['round']}|node3|"
            f"forwarded|2|leader_changed"
        )
        record("six-field historical meta line still parses",
               META_LEADER.match(legacy_line) is not None)
        legacy_rows = parse_meta(legacy_line + "\n")
        record("six-field historical line defaults to node 1",
               bool(legacy_rows) and legacy_rows[0]["write_node"] == 1,
               str(legacy_rows))
        csv = root / "perf-config.csv"
        csv.write_text(
            "case,variant,median_rps,baseline_rps,delta_pct,verdict\n"
            + build_line + "\n" + meta_line + "\n"
            "# raw cur Config Write: [2701.0, 14631.0]\n",
            encoding="utf-8",
        )
        parsed = parse_meta(csv.read_text(encoding="utf-8"))
        record("CSV meta round-trips",
               len(parsed) == 1 and parsed[0]["leader"] == "node3"
               and parsed[0]["write_path"] == FORWARDED, str(parsed))
        rows = evaluate_csv(csv)
        record("evaluate_csv emits one row", rows.count("\n") == 0 and "cur|r2|node3" in rows)
        record("evaluate_csv includes the load target", rows.endswith("|w1"), rows)
        target_csv = root / "perf-target.csv"
        target_csv.write_text(target_line + "\n", encoding="utf-8")
        record("evaluate_csv preserves a non-default load target",
               evaluate_csv(target_csv).endswith("|w3"),
               evaluate_csv(target_csv))

        # The load target must select a real port: a driver that still points
        # wrk at node 1 while claiming write-node 3 would mislabel the path.
        import importlib
        import types
        bench_module = importlib.import_module("bench_ab")
        loader = getattr(bench_module, "load_port", None)
        namespace = types.SimpleNamespace(
            mode="cluster", base_port=8848, write_node=3, port=8848,
        )
        record("load target selects the matching HTTP port",
               callable(loader) and loader(namespace) == 8868,
               str(loader))
        namespace.write_node = 1
        record("default load target stays on node 1",
               callable(loader) and loader(namespace) == 8848)
        namespace.mode = "standalone"
        record("standalone load target stays on its only port",
               callable(loader) and loader(namespace) == 8848)
        old_csv = root / "old.csv"
        old_csv.write_text("# raw cur Config Write: [2701.0, 14631.0]\n", encoding="utf-8")
        record("historical CSV reports missing meta",
               evaluate_csv(old_csv).startswith("NO_LEADER_META|"))
        record("unrelated comments are ignored",
               parse_meta("# unrelated\n" + meta_line + "\n") == parsed)

        jsonl = root / LEADER_LOG_NAME
        write_jsonl(jsonl, snap)
        loaded = json.loads(jsonl.read_text(encoding="utf-8").strip())
        record("JSONL record is structured", loaded["leader_node"] == 3
               and loaded["round"] == 2 and loaded["case"] == "config")

        # 7. CLI smoke: missing file is an error, valid file works.
        cli = [sys.executable, str(Path(__file__).resolve()), "--check-csv"]
        missing = subprocess.run(cli + [str(root / "nope.csv")],
                                 capture_output=True, text=True)
        record("CLI rejects a missing CSV", missing.returncode != 0)
        ok = subprocess.run(cli + [str(csv)], capture_output=True, text=True)
        record("CLI prints the attribution",
               ok.returncode == 0 and "forwarded" in ok.stdout, ok.stderr)
        record("CLI includes the load target", ok.stdout.strip().endswith("|w1"),
               ok.stdout)
        replay_log = _log(root, "dual-cur-r1.node1.log", [
            f"{ts}{prefix}Raft leader changed raft_event=\"leader_changed\" "
            "node_id=1 term=2 from_leader_id=-1 to_leader_id=3",
        ])
        logs_cli = [sys.executable, str(Path(__file__).resolve()),
                    "--check-logs", "--log-dir", str(root), "--case", "dual",
                    "--variant", "cur", "--round", "1", "--nodes", "3"]
        on_follower_cli = subprocess.run(logs_cli, capture_output=True, text=True)
        record("CLI replay defaults to the forwarded path",
               on_follower_cli.returncode == 0
               and "path=forwarded" in on_follower_cli.stdout,
               on_follower_cli.stdout + on_follower_cli.stderr)
        on_leader_cli = subprocess.run(logs_cli + ["--write-node", "3"],
                                       capture_output=True, text=True)
        record("CLI replay honours --write-node",
               on_leader_cli.returncode == 0
               and "path=leader-local" in on_leader_cli.stdout
               and "write_node=3" in on_leader_cli.stdout,
               on_leader_cli.stdout + on_leader_cli.stderr)
        record("replay fixture exists", replay_log.is_file())

    # 8. Static wiring guards: removing any link turns the check stage red.
    shell = REGRESSION_SH.read_text(encoding="utf-8") if REGRESSION_SH.is_file() else ""
    bench = BENCH_AB.read_text(encoding="utf-8") if BENCH_AB.is_file() else ""
    readme = README.read_text(encoding="utf-8") if README.is_file() else ""
    perf_body = _stage_body(shell, "stage_perf")
    check_body = _stage_body(shell, "stage_check")
    for token in (
        "import perf_leader",
        "perf_leader.capture_round(",
        "perf_leader.format_meta_line(",
        "perf_leader.format_build_line(",
        "perf_leader.write_jsonl(",
        "# meta leader",
        "perf_leader.LEADER_LOG_NAME",
        "--write-node",
        '"--write-node",',
        "def load_port(",
        "write_node=args.write_node",
        '"SERVER": f"127.0.0.1:{load_port(args)}"',
        "business_ok(load_port(args)",
        "if index != self.args.write_node:",
        "1 <= args.write_node <= args.nodes",
    ):
        record(f"bench_ab.py wires {token}", token in bench)
    for token in (
        "perf_leader.py",
        "--check-csv",
        "perf_leader.py\" --check-csv",
        "perf-${case_name}.leaders",
        "PERF_ADVISORY",
        "INCONCLUSIVE leader context",
        "PERF_WRITE_NODE",
        "--write-node",
    ):
        record(f"stage_perf wires {token}", token in perf_body)
    for token in (
        "perf_leader.py",
        "--self-test",
        "check-perf-leader.log",
        "SELF-TEST PASS",
        "perf_leader self-test failed",
    ):
        record(f"stage_check wires {token}", token in check_body)
    record("stage_check runs the leader self-test",
           'perf_leader.py" --self-test' in check_body)
    record("stage_check artifact assertion covers leader",
           check_body.count("check-perf-leader.log") >= 2)
    record("README documents leader attribution",
           "perf_leader.py" in readme and "leader" in readme.lower())

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
    print("SELF-TEST PASS: perf leader attribution and shell wiring intact")
    return 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--self-test", action="store_true",
                        help="run parser fixtures and shell wiring guards")
    parser.add_argument("--check-csv", metavar="FILE",
                        help="print leader attribution rows from one perf CSV")
    parser.add_argument("--require-meta", action="store_true",
                        help="with --check-csv, fail when attribution is missing")
    parser.add_argument("--check-logs", action="store_true",
                        help="resolve one archived round from its server logs")
    parser.add_argument("--log-dir", default=".")
    parser.add_argument("--case", default="config")
    parser.add_argument("--variant", default="cur")
    parser.add_argument("--round", type=int, default=1)
    parser.add_argument("--nodes", type=int, default=3)
    parser.add_argument("--write-node", type=int, default=DEFAULT_WRITE_NODE,
                        help="cluster node that received the load (default: 1)")
    parser.add_argument("--mode", default="cluster",
                        choices=("standalone", "cluster"))
    args = parser.parse_args(argv[1:])

    if args.self_test:
        return self_test()
    try:
        if args.check_csv:
            output = evaluate_csv(args.check_csv)
            print(output)
            if args.require_meta and output.startswith("NO_LEADER_META|"):
                print("perf_leader: missing leader attribution", file=sys.stderr)
                return 1
            return 0
        if args.check_logs:
            snap = capture_round(args.log_dir, args.case, args.variant,
                                 args.round, args.nodes, args.mode,
                                 write_node=args.write_node)
            label = (UNKNOWN if snap["leader_node"] is None
                     else f"node{snap['leader_node']}")
            print(f"case={args.case} variant={args.variant} r{args.round} "
                  f"leader={label} path={snap['write_path']} "
                  f"term={snap['term']} source={snap['source']} "
                  f"write_node={snap['write_node']} "
                  f"events={snap['events']} logs={snap['logs_present']}")
            return 0
    except (OSError, ValueError) as error:
        print(f"perf_leader: {error}", file=sys.stderr)
        return 1
    parser.print_help()
    return 3


if __name__ == "__main__":
    sys.exit(main(sys.argv))
