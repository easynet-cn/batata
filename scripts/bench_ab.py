#!/usr/bin/env python3
"""A/B benchmark driver for Batata.

Runs the same benchmark suite against two or more server configurations and
reports whether the difference is larger than the run-to-run noise.

Why this exists: a single `benchmark.sh` run on a developer machine is not
reproducible enough to judge changes of 10-20%. Measured spread on a 16-core
macOS box with a colocated load generator reached 43% on Instance Register and
49% on Service List, so a one-off before/after comparison says nothing.

This driver controls the two dominant error sources:

1. Colocated client and server oversubscribing the CPU. The server is started
   with `-d server` (no console server) and a fixed worker count, leaving cores
   for `wrk` and the OS.
2. Monotonic drift within a session (RocksDB compaction, thermal, background
   load). Variants are measured in alternating order per round - AB, then BA -
   so a linear drift cancels out of the median difference.

Each variant is measured once per round after a discarded warmup run, and the
per-case median is reported together with the observed min/max. A difference is
only called "resolved" when it exceeds the spread of both variants.

Usage:
    ./scripts/bench_ab.py --case naming --rounds 3 \
        --variant off=BATATA_CORE_AUTH_CACHING_ENABLED=false \
        --variant on=

    # more rounds and a shorter duration for a quick sanity check
    ./scripts/bench_ab.py --rounds 2 --duration 20s --case naming \
        --variant off=BATATA_CORE_AUTH_CACHING_ENABLED=false --variant on=

    # 3-node raft cluster: node 1 (:8848) is measured, peers (:8858/:8868)
    # only absorb raft + distro traffic
    ./scripts/bench_ab.py --mode cluster --nodes 3 --rounds 3 --case naming \
        --variant off=BATATA_CORE_AUTH_CACHING_ENABLED=false --variant on=

Environment passed to the benchmark (wrk) comes from THREADS / CONNECTIONS /
DURATION / USERNAME / PASSWORD, same as benchmark.sh.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import shutil
import signal
import socket
import statistics
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

# The driver is normally launched as `python3 scripts/bench_ab.py`, so the
# script directory is already on sys.path. Add it explicitly as well so the
# helpers still resolve when this module is imported from another directory.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from perf_observability import (  # noqa: E402
    DEFAULT_MAX_BYTES,
    cap_log_file,
    capture_host_state,
    server_log_paths,
)
import perf_leader  # noqa: E402

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BENCH = os.path.join(REPO_ROOT, "scripts", "benchmark.sh")
SERVER_BIN = os.path.join(REPO_ROOT, "target", "release", "batata-server")
ANSI = re.compile(r"\x1b\[[0-9;]*m")
CASE_LINE = re.compile(r"^\s*(?:▶|>\s*)\s*(.+?)\s*\(")
RPS_LINE = re.compile(r"^\s*RPS:\s*([0-9.]+)")
LATENCY_LINE = re.compile(r"^\s*Avg Latency:\s*([0-9.]+)(ms|s|us)?")
ERROR_LINE = re.compile(r"^\s*Errors:\s*(.+)$")

# A fresh store per run: the suites write hundreds of thousands of
# registrations, so a shared store keeps growing and compacting and throughput
# drifts downwards across a session.
#
# This must be a `--dotted.key=value` CLI override, not a BATATA_* env var: the
# env mapping rewrites every underscore to a dot, so `data_dir` would arrive as
# `data.dir` and be ignored.
DATA_DIR_KEY = "batata.persistence.embedded.data_dir"

# Applied to every node in every mode so standalone and cluster runs measure the
# same code paths. The access log costs several milliseconds per request (it is
# enabled by default), which swamps the differences this driver is meant to
# resolve and makes the two modes incomparable.
COMMON_ARGS = ["--batata.server.http.access_log.enabled=false"]

# External DB (MySQL/PostgreSQL) backing for `--storage mysql|postgres`.
# The driver benchmarks a *clean* external DB exactly like it benchmarks a
# fresh embedded store: each run DROPs and CREATEs this dedicated database so the
# SQL-backed write path is measured against an empty schema rather than one that
# has grown across the session. A dedicated `batata_perf` DB is used so a perf
# capture never clobbers the SDK test database (`batata_sdk_test`).
#
# ExternalDb + Standalone opens no RocksDB at all (see startup/persistence.rs:
# `rocks_db: None, raft_node: None`), so no embedded data_dir is passed for it.
EXTERNAL_DB = {
    "mysql": {
        "platform": "mysql",
        "url": "mysql://root:devterry@127.0.0.1:3306/batata_perf",
        "db": "batata_perf",
        "client": "mysql",
    },
    "postgres": {
        "platform": "postgresql",
        "url": "postgres://postgres:devterry@127.0.0.1:5432/batata_perf",
        "db": "batata_perf",
        "client": "psql",
    },
}


def find_client(name: str) -> str:
    """Locate a DB client binary across Homebrew prefixes and PATH."""
    candidates: dict[str, list[str]] = {
        "mysql": [
            "/opt/homebrew/opt/mysql-client@8.4/bin/mysql",
            "/usr/local/mysql/bin/mysql",
            "/opt/homebrew/bin/mysql",
            "mysql",
        ],
        "psql": [
            "/opt/homebrew/opt/libpq/bin/psql",
            "/opt/homebrew/bin/psql",
            "/usr/local/bin/psql",
            "psql",
        ],
    }
    for path in candidates.get(name, []):
        if os.path.exists(path):
            return path
    found = shutil.which(name)
    if found:
        return found
    raise SystemExit(f"DB client '{name}' not found; install mysql-client/libpq or put it on PATH")


def reset_external_db(storage: str) -> None:
    """DROP + CREATE the dedicated perf database for a clean store."""
    cfg = EXTERNAL_DB[storage]
    client = find_client(cfg["client"])
    db = cfg["db"]
    if cfg["client"] == "mysql":
        cmd = [
            client, "-h", "127.0.0.1", "-P", "3306", "-u", "root", "-pdevterry",
            "-e", f"DROP DATABASE IF EXISTS {db}; CREATE DATABASE {db};",
        ]
        subprocess.run(cmd, check=False, capture_output=True, text=True)
    else:
        env = dict(os.environ)
        env["PGPASSWORD"] = "devterry"
        for statement in (
            f"SELECT pg_terminate_backend(pid) FROM pg_stat_activity "
            f"WHERE datname='{db}' AND pid<>pg_backend_pid();",
            f"DROP DATABASE IF EXISTS {db};",
            f"CREATE DATABASE {db};",
        ):
            subprocess.run(
                [client, "-h", "127.0.0.1", "-p", "5432", "-U", "postgres", "-d", "postgres",
                 "-c", statement],
                check=False, env=env, capture_output=True, text=True,
            )
    print(f"  reset external db: {storage}/{db}")


class Sample:
    """One measured case: throughput, average latency and client-side errors."""

    def __init__(self, rps: float, latency_ms: float, errors: str = ""):
        self.rps = rps
        self.latency_ms = latency_ms
        self.errors = errors


def strip_ansi(text: str) -> str:
    return ANSI.sub("", text)


def parse_benchmark(output: str) -> dict[str, Sample]:
    """Extract `case -> Sample` from a benchmark.sh run.

    Latency and client errors are captured alongside RPS because a variant can
    look "faster" simply by failing requests earlier.
    """
    cases: dict[str, Sample] = {}
    current: str | None = None
    rps: float | None = None
    latency: float | None = None
    errors = ""

    def flush() -> None:
        if current and rps is not None:
            cases[current] = Sample(rps, latency if latency is not None else float("nan"), errors)

    for raw in strip_ansi(output).splitlines():
        line = raw.strip()
        match = CASE_LINE.match(line)
        if match:
            flush()
            current, rps, latency, errors = match.group(1).strip(), None, None, ""
            continue
        match = RPS_LINE.match(line)
        if match and current:
            rps = float(match.group(1))
            continue
        match = LATENCY_LINE.match(line)
        if match and current:
            value = float(match.group(1))
            unit = match.group(2) or "ms"
            latency = value * {"ms": 1.0, "s": 1000.0, "us": 0.001}[unit]
            continue
        match = ERROR_LINE.match(line)
        if match and current:
            errors = match.group(1).strip()

    flush()
    return cases


def port_in_use(port: int, host: str = "127.0.0.1") -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.settimeout(0.5)
        return sock.connect_ex((host, port)) == 0


def wait_until(predicate, timeout: float, interval: float = 0.5) -> bool:
    deadline = time.time() + timeout
    while time.time() < deadline:
        if predicate():
            return True
        time.sleep(interval)
    return False


def health_ok(port: int) -> bool:
    url = f"http://127.0.0.1:{port}/nacos/v3/console/health/liveness"
    try:
        with urllib.request.urlopen(url, timeout=2) as response:
            return response.status == 200
    except (urllib.error.URLError, OSError):
        return False


def business_ok(port: int, user: str, password: str) -> tuple[bool, str]:
    """Run one real request and check the payload says success.

    A health probe is not enough: Batata answers most failures with HTTP 200
    and a `code` inside the body, so a variant that rejects every request
    looks *faster* (sub-millisecond) while doing nothing. This logs in and
    registers one instance, and reports the body code.
    """
    body = urllib.parse.urlencode({"username": user, "password": password}).encode()
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}/nacos/v3/auth/user/login", data=body, method="POST"
    )
    try:
        with urllib.request.urlopen(request, timeout=5) as response:
            token = json.load(response).get("accessToken", "")
    except (urllib.error.URLError, OSError, ValueError) as error:
        return False, f"login failed: {error}"

    if not token:
        return False, "login returned no accessToken"

    body = urllib.parse.urlencode(
        {"serviceName": "probe-service", "ip": "10.0.0.1", "port": "8080"}
    ).encode()
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}/nacos/v2/ns/instance?accessToken={token}",
        data=body,
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=5) as response:
            payload = json.load(response)
    except (urllib.error.URLError, OSError, ValueError) as error:
        return False, f"register failed: {error}"

    if payload.get("code") != 0:
        return False, f"register returned {payload}"

    return True, "ok"


def run(cmd: list[str], env: dict[str, str]) -> subprocess.CompletedProcess:
    return subprocess.run(
        cmd,
        cwd=REPO_ROOT,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )


def strip_mode_flag(cmd: list[str]) -> list[str]:
    """Drop an existing `-m <mode>` pair so the caller can set the mode."""
    out: list[str] = []
    index = 0
    while index < len(cmd):
        if cmd[index] == "-m":
            index += 2
            continue
        out.append(cmd[index])
        index += 1
    return out


def load_port(args: argparse.Namespace) -> int:
    """HTTP port of the single node that receives the measured load.

    Keeping this in one place matters: the benchmark, the readiness probe,
    the init scripts and the leader attribution all have to agree on which
    node was written to, otherwise ``write_path`` describes a different
    request path than the measured RPS.
    """
    if args.mode != "cluster":
        return args.port
    return args.base_port + 10 * (args.write_node - 1)


class Server:
    """One batata-server process, or a whole cluster, for one variant."""

    def __init__(
        self,
        args: argparse.Namespace,
        variant_env: dict[str, str],
        log_paths: list[str],
        store_dir: str | None = None,
        variant_cli: list[str] | None = None,
    ):
        self.args = args
        self.store_dir = store_dir
        self.env = dict(os.environ)
        self.env.update(variant_env)
        self.env["BATATA_SERVER_HTTP_WORKERS"] = str(args.workers)
        self.log_paths = log_paths
        self.variant_cli = variant_cli or []
        self.procs: list[subprocess.Popen] = []
        self.nodes = self._build_nodes()

    def _build_nodes(self) -> list[tuple[list[str], int, str]]:
        """Return `(command, port, log path)` per process to start."""
        base = strip_mode_flag(shlex.split(self.args.server_cmd))

        if self.args.mode == "standalone":
            cmd = [*base, "-m", "standalone", *COMMON_ARGS, *self.variant_cli]
            if self.args.storage == "embedded":
                if self.store_dir is not None:
                    cmd.append(f"--{DATA_DIR_KEY}={self.store_dir}")
                    print(f"  fresh store: {self.store_dir}")
            else:
                cfg = EXTERNAL_DB[self.args.storage]
                cmd.append(f"--batata.sql.init.platform={cfg['platform']}")
                cmd.append(f"--db-url={cfg['url']}")
                print(f"  external store: {self.args.storage} ({cfg['url']})")
            return [(cmd, self.args.port, self.log_paths[0])]

        ports = [self.args.base_port + 10 * i for i in range(self.args.nodes)]
        member_list = ",".join(
            f"127.0.0.1:{port}?node_id={index}"
            for index, port in enumerate(ports, start=1)
        )
        nodes: list[tuple[list[str], int, str]] = []

        for index, port in enumerate(ports, start=1):
            cmd = [
                *base,
                "-m",
                "cluster",
                *COMMON_ARGS,
                *self.variant_cli,
                f"--batata.server.main.port={port}",
                f"--batata.member.list={member_list}",
                "--batata.sql.init.platform=embedded",
            ]
            # Only the load target serves wrk; every other node just answers
            # distro/raft traffic and gets a small worker pool so the measured
            # node keeps the cores. This must follow --write-node, not node 1.
            if index != self.args.write_node:
                cmd.append(f"--batata.server.http.workers={self.args.peer_workers}")

            if self.store_dir is not None:
                node_dir = os.path.join(self.store_dir, f"node{index}")
                os.makedirs(node_dir, exist_ok=True)
                cmd.append(f"--{DATA_DIR_KEY}={node_dir}")

            nodes.append((cmd, port, self.log_paths[index - 1]))

        if self.store_dir is not None:
            print(f"  fresh store: {self.store_dir} (one directory per node)")

        return nodes

    def start(self) -> bool:
        # A clean external DB each run, exactly like a fresh embedded store.
        if self.args.storage != "embedded" and self.store_dir is not None:
            reset_external_db(self.args.storage)

        for cmd, _port, log_path in self.nodes:
            log = open(log_path, "w", encoding="utf-8", errors="replace")
            self.procs.append(
                subprocess.Popen(
                    cmd,
                    cwd=REPO_ROOT,
                    env=self.env,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                )
            )

        ready = wait_until(
            lambda: all(health_ok(port) for _cmd, port, _log in self.nodes),
            self.args.ready_timeout,
        )
        if not ready:
            return False

        if self.args.mode == "cluster":
            # Raft elects a leader after the HTTP ports open; writing before
            # that fails and would be measured as a fast error.
            time.sleep(self.args.cluster_settle)

        return True

    def stop(self) -> None:
        for proc in self.procs:
            proc.send_signal(signal.SIGTERM)
        for proc in self.procs:
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.send_signal(signal.SIGKILL)
                proc.wait(timeout=10)
        self.procs = []
        for _cmd, port, _log in self.nodes:
            wait_until(lambda p=port: not port_in_use(p), 15)


def wait_business_ready(args: argparse.Namespace, timeout: float) -> tuple[bool, str]:
    """Poll until one authenticated write comes back with `code: 0`."""
    deadline = time.time() + timeout
    detail = "timeout"
    while time.time() < deadline:
        ok, detail = business_ok(load_port(args), args.user, args.password)
        if ok:
            return True, detail
        time.sleep(1.0)
    return False, detail


def parse_variants(raw: list[str]) -> list[tuple[str, dict[str, str]]]:
    """Parse `NAME=K=V;K2=V2` style variant definitions."""
    variants: list[tuple[str, dict[str, str]]] = []
    for item in raw:
        name, _, spec = item.partition("=")
        name = name.strip()
        if not name:
            raise SystemExit(f"invalid variant (empty name): {item!r}")
        env: dict[str, str] = {}
        for pair in spec.split(";"):
            pair = pair.strip()
            if not pair:
                continue
            key, sep, value = pair.partition("=")
            if not sep:
                raise SystemExit(f"invalid env assignment in variant {name}: {pair!r}")
            env[key.strip()] = value.strip()
        variants.append((name, env))
    return variants


def parse_variant_cli(raw: list[str]) -> list[tuple[str, list[str]]]:
    """Parse `NAME=--flag;--flag2=val` style variant CLI-arg definitions.

    Unlike `--variant` (which injects environment variables and cannot express
    config keys containing an underscore, e.g. `access_log`, due to the BATATA_*
    env underscore->dot rewriting), this attaches raw CLI arguments to a variant.
    They are appended after COMMON_ARGS so the last write wins.
    """
    cli: list[tuple[str, list[str]]] = []
    for item in raw:
        name, _, spec = item.partition("=")
        name = name.strip()
        if not name:
            raise SystemExit(f"invalid variant-cli (empty name): {item!r}")
        tokens = [tok.strip() for tok in spec.split(";") if tok.strip()]
        cli.append((name, tokens))
    return cli


def main() -> int:
    parser = argparse.ArgumentParser(
        description="A/B benchmark driver with alternating rounds and median aggregation",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--case", default="naming", help="benchmark.sh suite (default: naming)")
    parser.add_argument("--rounds", type=int, default=3, help="rounds per variant (default: 3)")
    parser.add_argument(
        "--warmup-rounds",
        type=int,
        default=0,
        help="leading rounds that are run but not recorded (default: 0)",
    )
    parser.add_argument(
        "--variant",
        action="append",
        required=True,
        metavar="NAME=ENV",
        help="variant definition, repeatable; env pairs separated by ';'",
    )
    parser.add_argument(
        "--variant-cli",
        action="append",
        default=[],
        metavar="NAME=--FLAG;--FLAG2=VAL",
        help="per-variant CLI args appended AFTER COMMON_ARGS (so they override the "
        "hardcoded defaults, e.g. --batata.server.http.access_log.enabled=true). "
        "Repeatable; tokens separated by ';'. Needed because --variant only injects "
        "ENV vars, which cannot express keys with an underscore (access_log).",
    )
    parser.add_argument("--workers", type=int, default=8, help="batata.server.http.workers (default: 8)")
    parser.add_argument("--duration", default="30s", help="measured run duration (default: 30s)")
    parser.add_argument("--warmup", default="10s", help="discarded warmup duration (default: 10s)")
    parser.add_argument("--connections", type=int, default=100)
    parser.add_argument("--threads", type=int, default=2)
    parser.add_argument("--port", type=int, default=8848)
    parser.add_argument(
        "--ready-timeout",
        type=float,
        default=40.0,
        help="seconds to wait for the server health probe (default: 40)",
    )
    parser.add_argument("--settle", type=float, default=5.0, help="idle seconds between variants")
    parser.add_argument("--user", default="benchuser")
    parser.add_argument("--password", default="benchpass")
    parser.add_argument(
        "--mode",
        choices=("standalone", "cluster"),
        default="standalone",
        help="standalone: one node; cluster: --nodes nodes forming a raft cluster",
    )
    parser.add_argument("--nodes", type=int, default=3, help="cluster size (default: 3)")
    parser.add_argument(
        "--write-node",
        type=int,
        default=1,
        help="cluster node that receives all wrk load (default: 1). Changing it "
        "changes whether the write runs locally on the leader or is forwarded; "
        "attribution records the resulting path per round. Standalone always "
        "normalises this to 1.",
    )
    parser.add_argument(
        "--base-port",
        type=int,
        default=8848,
        help="main port of node 1; node i uses base-port + 10*i (default: 8848)",
    )
    parser.add_argument(
        "--peer-workers",
        type=int,
        default=2,
        help="HTTP workers on cluster nodes 2..N (default: 2)",
    )
    parser.add_argument(
        "--cluster-settle",
        type=float,
        default=10.0,
        help="extra seconds after the cluster is healthy, for raft election (default: 10)",
    )
    parser.add_argument(
        "--server-cmd",
        default="./target/release/batata-server -d server -m standalone",
        help="server start command (default: -d server, i.e. no console server)",
    )
    parser.add_argument(
        "--storage",
        choices=("embedded", "mysql", "postgres"),
        default="embedded",
        help="storage backend the server runs against (default: embedded/RocksDB). "
        "mysql|postgres start the server with --db-url + --batata.sql.init.platform "
        "and reset a dedicated perf database each run. Cluster mode always uses "
        "embedded RocksDB for the raft/protocol layer, so --storage is ignored there.",
    )
    parser.add_argument("--init", action="store_true", help="run init-admin.sh / init-bench-user.sh per variant")
    parser.add_argument(
        "--fresh-store",
        action="store_true",
        help="give every run its own empty embedded store under --store-root",
    )
    parser.add_argument(
        "--store-root",
        default="/tmp/bench_ab_stores",
        help="parent directory for --fresh-store data dirs (default: /tmp/bench_ab_stores)",
    )
    parser.add_argument(
        "--log-dir",
        default="/tmp/bench_ab_logs",
        help="directory for per-(case, variant, round, node) server logs "
        "(default: /tmp/bench_ab_logs). The regression gate overrides this with "
        "its report log dir so the logs survive the run.",
    )
    parser.add_argument(
        "--host-state-log",
        default=None,
        help="JSON-lines file for before/after host load and disk snapshots "
        "(default: <log-dir>/host-state.log)",
    )
    parser.add_argument(
        "--server-log-max-bytes",
        type=int,
        default=DEFAULT_MAX_BYTES,
        help="cap each server log after the process stops; head, tail and "
        "health-signal lines are retained (default: 4 MiB)",
    )
    parser.add_argument("--out", default="/tmp/bench_ab_results.csv")
    args = parser.parse_args()

    if args.rounds < 2:
        raise SystemExit("--rounds must be >= 2: a single round cannot separate drift from effect")
    if args.rounds - args.warmup_rounds < 2:
        raise SystemExit(
            f"--rounds {args.rounds} minus --warmup-rounds {args.warmup_rounds} leaves "
            "fewer than 2 recorded rounds: the median and spread would be meaningless"
        )
    if not os.path.exists(SERVER_BIN):
        raise SystemExit(f"server binary not found: {SERVER_BIN} (build with --release first)")
    if args.mode == "cluster":
        args.port = args.base_port
        args.nodes = max(args.nodes, 1)
        if not 1 <= args.write_node <= args.nodes:
            raise SystemExit(
                f"--write-node {args.write_node} outside cluster 1..{args.nodes}"
            )
        ports = [args.base_port + 10 * i for i in range(args.nodes)]
        if args.storage != "embedded":
            print(
                f"--storage {args.storage} ignored for --mode cluster "
                "(cluster always uses embedded RocksDB for the raft/protocol layer); "
                "benchmarking embedded cluster"
            )
            args.storage = "embedded"
    else:
        ports = [args.port]
        # Standalone is one process with no Raft group. Normalise the node
        # count and the load target so the build/leader metadata does not
        # claim three nodes or a non-existent write target.
        args.nodes = 1
        args.write_node = 1

    busy = [
        port + offset
        for port in ports
        for offset in (0, 1000, -1000)  # main, gRPC, raft
    ]
    for port in busy:
        if port_in_use(port):
            raise SystemExit(f"port {port} already in use; stop the running server/cluster first")

    variants = parse_variants(args.variant)
    variant_cli_map = {name: cli for name, cli in parse_variant_cli(args.variant_cli)}

    # Persist the context a perf number needs to be attributable: host load and
    # free disk before/after each round, and one server log per round/node.
    os.makedirs(args.log_dir, exist_ok=True)
    host_state_log = args.host_state_log or os.path.join(args.log_dir, "host-state.log")
    capture_host_state(host_state_log, f"start case={args.case} mode={args.mode}")
    # A perf number is only meaningful together with the revision that produced
    # it and, in cluster mode, the Raft leader that served each round.
    leader_log_path = os.path.join(args.log_dir, perf_leader.LEADER_LOG_NAME)
    build_identity = perf_leader.build_identity(args.mode, args.nodes)
    leader_meta: list[dict[str, object]] = []
    if args.fresh_store and not args.init:
        # A fresh store contains no users, so the benchmark would run anonymously.
        args.init = True
        print("--fresh-store implies --init (a new store has no users)")
    results: dict[str, dict[str, list[Sample]]] = {name: {} for name, _ in variants}
    errors_seen: dict[str, set[str]] = {name: set() for name, _ in variants}

    bench_env = dict(os.environ)
    bench_env.update(
        {
            "DURATION": args.duration,
            "CONNECTIONS": str(args.connections),
            "THREADS": str(args.threads),
            "USERNAME": args.user,
            "PASSWORD": args.password,
            # All wrk load goes to one node. --write-node decides whether that
            # node is the Raft leader (local client_write) or a follower
            # (gRPC forward + wait_for_applied); peers only see raft/distro.
            "SERVER": f"127.0.0.1:{load_port(args)}",
        }
    )
    # Never route local traffic through a proxy: urllib (used by
    # `business_ok()`) honours HTTP_PROXY, so an exported proxy makes every
    # variant look "not ready" and the run is skipped. The same applies to
    # benchmark.sh's curl probes, which pass `--noproxy '*'` themselves.
    for key in ("HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy"):
        bench_env.pop(key, None)
    bench_env["no_proxy"] = "*"

    print(
        f"bench_ab: mode={args.mode} case={args.case} rounds={args.rounds} "
        f"duration={args.duration} workers={args.workers} wrk=-t{args.threads} -c{args.connections} "
        f"write-node={args.write_node} load-port={load_port(args)}"
    )
    print(f"variants: {', '.join(name for name, _ in variants)}\n")

    for round_index in range(1, args.rounds + 1):
        # Alternate the order per round so a linear drift cancels out.
        order = variants if round_index % 2 == 1 else list(reversed(variants))

        for name, env in order:
            recorded = round_index > args.warmup_rounds
            suffix = "" if recorded else " (warmup round, discarded)"
            print(f"[round {round_index}] variant '{name}'{suffix}")
            store_dir = None
            if args.fresh_store:
                store_dir = os.path.join(args.store_root, f"{name}-r{round_index}")
            paths = server_log_paths(
                args.log_dir, args.case, name, round_index, args.nodes, args.mode
            )
            capture_host_state(host_state_log, f"before {name} r{round_index}")
            server = Server(
                args,
                env,
                paths,
                store_dir,
                variant_cli=variant_cli_map.get(name, []),
            )

            try:
                if not server.start():
                    print("  server failed to become ready; skipping", file=sys.stderr)
                    continue

                if args.init:
                    for script in ("init-admin.sh", "init-bench-user.sh"):
                        run(["bash", os.path.join("scripts", script)], bench_env)

                # Never measure a server that answers with in-body error codes:
                # those responses are sub-millisecond and look like a win.
                ok, detail = wait_business_ready(args, args.ready_timeout)
                if not ok:
                    print(f"  business request never succeeded: {detail}", file=sys.stderr)
                    continue

                # Warmup: fills caches and warms the process; result discarded.
                run([BENCH, args.case], {**bench_env, "DURATION": args.warmup})

                if not recorded:
                    # Still run the load: the point is to let the freshly
                    # started node reach steady state before recording.
                    run([BENCH, args.case], bench_env)
                    continue

                measured = run([BENCH, args.case], bench_env)
                cases = parse_benchmark(measured.stdout)
                if not cases:
                    print("  no results parsed; see output below", file=sys.stderr)
                    print(measured.stdout[-2000:], file=sys.stderr)

                for case_name, sample in cases.items():
                    results[name].setdefault(case_name, []).append(sample)
                    if sample.errors:
                        errors_seen[name].add(f"{case_name}: {sample.errors}")
                    print(
                        f"  {case_name}: {sample.rps:.0f} RPS, "
                        f"avg {sample.latency_ms:.2f} ms"
                    )
            finally:
                # Always stop, otherwise a crash leaves a server holding the port.
                server.stop()
                capture_host_state(host_state_log, f"after {name} r{round_index}")
                # The default INFO level emits two lines per config write; bound
                # the artifacts without losing Raft health evidence.
                for path in server.log_paths:
                    cap_log_file(path, args.server_log_max_bytes)
                # The cap keeps raft_event lines, so the election/leader
                # evidence survives the rewrite. Resolve the steady leader
                # only after stop(): a round that is still serving can change
                # leader between the measurement and the parse.
                snapshot = perf_leader.capture_round(
                    args.log_dir, args.case, name, round_index,
                    args.nodes, args.mode, paths=server.log_paths,
                    write_node=args.write_node,
                )
                snapshot["recorded"] = bool(recorded)
                leader_meta.append(snapshot)
                perf_leader.write_jsonl(leader_log_path, snapshot)
                leader_label = (
                    "unknown" if snapshot["leader_node"] is None
                    else f"node{snapshot['leader_node']}"
                )
                print(
                    f"  leader={leader_label} path={snapshot['write_path']} "
                    f"write-node={snapshot['write_node']} "
                    f"source={snapshot['source']}"
                )
            time.sleep(args.settle)

    print("\n" + "=" * 78)
    print("Results (RPS, median over rounds; spread = observed min..max)")
    print("=" * 78)

    baseline = variants[0][0]
    rows: list[str] = []

    for case_name in results[baseline]:
        print(f"\n{case_name}")
        medians: dict[str, float] = {}
        spreads: dict[str, float] = {}

        for name, _ in variants:
            samples = results[name].get(case_name, [])
            if not samples:
                print(f"  {name:<10} no data")
                continue
            values = [s.rps for s in samples]
            median = statistics.median(values)
            spread = max(values) - min(values)
            latency = statistics.median([s.latency_ms for s in samples])
            medians[name] = median
            spreads[name] = spread
            print(
                f"  {name:<10} median {median:>9.0f}   min {min(values):>9.0f}   "
                f"max {max(values):>9.0f}   spread {spread:>8.0f} "
                f"({100 * spread / median:>4.1f}%)   avg latency {latency:>6.2f} ms"
            )

        if baseline not in medians:
            continue

        base = medians[baseline]
        for name, _ in variants[1:]:
            if name not in medians:
                continue
            delta = medians[name] - base
            pct = 100 * delta / base
            noise = max(spreads.get(baseline, 0.0), spreads.get(name, 0.0))
            resolved = abs(delta) > noise
            verdict = "RESOLVED" if resolved else "within noise"
            print(
                f"  -> {name} vs {baseline}: {delta:+.0f} RPS ({pct:+.1f}%), "
                f"noise floor {noise:.0f} => {verdict}"
            )
            rows.append(f"{case_name},{name},{medians[name]:.1f},{base:.1f},{pct:.1f},{verdict}")

    with open(args.out, "w", encoding="utf-8") as handle:
        handle.write("case,variant,median_rps,baseline_rps,delta_pct,verdict\n")
        for row in rows:
            handle.write(row + "\n")
        # `# meta leader|<variant>|r<N>|node<X>|<path>|<term>|<source>|<write_node>`
        # lines make every recorded round attributable without changing the CSV
        # columns consumed by existing tools. The trailing load target lets
        # perf_paths.py split rounds into the leader-local and forwarded
        # populations. Warmup rounds are discarded from the results, so they
        # are discarded from the attribution too.
        handle.write(perf_leader.format_build_line(build_identity) + "\n")
        for snapshot in leader_meta:
            if snapshot.get("recorded"):
                handle.write(perf_leader.format_meta_line(snapshot) + "\n")
        for name, _ in variants:
            for case_name, samples in results[name].items():
                values = [round(s.rps, 1) for s in samples]
                handle.write(f"# raw {name} {case_name}: {values}\n")

    if any(snapshot.get("recorded") for snapshot in leader_meta):
        print("\nLeader attribution (recorded rounds):")
        for snapshot in leader_meta:
            if not snapshot.get("recorded"):
                continue
            label = (
                "unknown" if snapshot["leader_node"] is None
                else f"node{snapshot['leader_node']}"
            )
            print(
                f"  {snapshot['variant']} r{snapshot['round']}: leader={label} "
                f"path={snapshot['write_path']} write-node={snapshot['write_node']} "
                f"term={snapshot['term']} source={snapshot['source']}"
            )

    for name, _ in variants:
        if errors_seen[name]:
            print(f"\nWARNING: client errors reported for variant '{name}':")
            for item in sorted(errors_seen[name]):
                print(f"  {item}")
            print("A variant that fails requests earlier can look faster; check these before quoting any delta.")

    capture_host_state(host_state_log, f"end case={args.case} mode={args.mode}")
    print(f"\nCSV written to {args.out}")
    print(f"Server logs: {args.log_dir}")
    print(f"Host state:  {host_state_log}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
