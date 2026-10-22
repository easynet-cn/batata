# Console Datasource Test Scripts

Automated test scripts for verifying the Batata console datasource across all deployment modes.

## Prerequisites

- `curl` and `jq` installed
- Batata server built: `cargo build -p batata-server`
- For ExternalDb modes: MySQL or PostgreSQL initialized with schema

## Test Modes

Batata console datasource has 4 deployment combinations:

| Mode | Storage | Nodes | Config Requirement |
|------|---------|-------|--------------------|
| **Standalone + Embedded** | RocksDB | 1 | `batata.sql.init.platform` = empty |
| **Standalone + ExternalDb** | MySQL/PostgreSQL | 1 | `batata.sql.init.platform` = mysql/postgresql |
| **Cluster + ExternalDb** | MySQL/PostgreSQL (shared) | 3 | `batata.sql.init.platform` = mysql/postgresql |
| **Cluster + Embedded** | Raft + RocksDB | 3 | `batata.sql.init.platform` = empty |

The storage mode is determined by `batata.sql.init.platform` in `conf/application.yml`, **independent** of standalone/cluster mode. See [Storage Mode Decision Logic](#storage-mode-decision-logic) for details.

## Quick Start

### 1. Standalone + Embedded (RocksDB)

No external database required. Simplest mode to test.

```bash
# Ensure batata.sql.init.platform is empty in conf/application.yml
# Start server
cargo run -p batata-server -- -m standalone &

# Wait for server to be ready, then run tests
./scripts/test.sh console standalone-embedded
```

### 2. Standalone + ExternalDb (MySQL/PostgreSQL)

```bash
# Create an empty database — Batata creates the schema via SeaORM migrations

# Ensure batata.sql.init.platform: mysql in conf/application.yml
# Start server
cargo run -p batata-server -- -m standalone --db-url "mysql://user:pass@localhost:3306/batata" &

# Run tests
./scripts/test.sh console standalone-externaldb
```

### 3. Cluster + ExternalDb (3-node)

All nodes share the same external database for config storage. Naming service uses distro protocol for ephemeral instance sync.

```bash
# Create an empty database — Batata creates the schema via SeaORM migrations

# Create conf/cluster.conf
echo "127.0.0.1:8848
127.0.0.1:8858
127.0.0.1:8868" > conf/cluster.conf

# Start 3 nodes
cargo run -p batata-server -- -m cluster \
  --db-url "mysql://user:pass@localhost:3306/batata" \
  --main-port 8848 --console-port 8081 &

cargo run -p batata-server -- -m cluster \
  --db-url "mysql://user:pass@localhost:3306/batata" \
  --main-port 8858 --console-port 8082 &

cargo run -p batata-server -- -m cluster \
  --db-url "mysql://user:pass@localhost:3306/batata" \
  --main-port 8868 --console-port 8083 &

# Run tests
./scripts/test.sh console cluster-externaldb
```

### 4. Cluster + Embedded (Raft + RocksDB, 3-node)

No external database. Config storage uses Raft consensus + RocksDB.

```bash
# Create conf/cluster.conf
echo "127.0.0.1:8848
127.0.0.1:8858
127.0.0.1:8868" > conf/cluster.conf

# Ensure batata.sql.init.platform is empty in conf/application.yml
# Start 3 nodes (each needs separate data dir)
cargo run -p batata-server -- -m cluster \
  --main-port 8848 --console-port 8081 &

cargo run -p batata-server -- -m cluster \
  --main-port 8858 --console-port 8082 &

cargo run -p batata-server -- -m cluster \
  --main-port 8868 --console-port 8083 &

# Run tests
./scripts/test.sh console cluster-embedded
```

## File Structure

```
scripts/
├── README.md                       # This file
├── test.sh                         # Unified test entry point (console / separation / apollo / sdk / all)
├── test_utils.sh                   # Shared utilities (login, HTTP, assertions)
├── test_standalone_embedded.sh     # Standalone + RocksDB tests
├── test_standalone_externaldb.sh   # Standalone + MySQL/PostgreSQL tests
├── test_cluster_externaldb.sh      # 3-node Cluster + ExternalDb tests
├── test_cluster_embedded.sh        # 3-node Cluster + Raft+RocksDB tests
├── sdk_run_lib.sh                  # SDK runner helpers (nacos / consul / apollo)
├── cluster_lib.sh                  # Cluster startup/teardown helpers for SDK matrix
├── startup.sh                      # Local multi-node startup helper (used by SDK matrix)
├── init-admin.sh                   # One-time admin bootstrap (nacos/nacos)
├── init-bench-user.sh              # Non-admin user for permission-path benchmarks
├── benchmark.sh                    # wrk/hey throughput benchmark (config / naming / consul)
├── bench_ab.py                     # A/B driver: alternating rounds, median aggregation
├── regression.sh                   # Unified regression gate (see "Regression Gate")
├── regression_whitelist.txt        # Known-failure baseline for the gate
├── regression_perf_baseline.txt    # Performance anchors for `regression.sh --perf`
├── bench_sync_load.py              # Cluster sync load generator (distinct instances)
└── profile.sh                      # CPU profiler harness (perf / sample)
```

### test_utils.sh

Shared library sourced by all test scripts. Provides:

| Category | Functions |
|----------|-----------|
| **Logging** | `log_info`, `log_pass`, `log_fail`, `log_skip`, `log_section` |
| **Auth** | `do_login(url, username, password)` |
| **HTTP** | `console_get`, `console_post_form`, `console_post_json`, `console_put_form`, `console_put_json`, `console_delete`, `main_get`, `main_post_form`, `main_delete` |
| **Assertions** | `assert_http_code`, `assert_success`, `assert_data_equals`, `assert_data_not_empty`, `assert_data_length` |
| **Server** | `wait_for_server`, `start_server`, `stop_server` |
| **ID Gen** | `unique_ns_id`, `unique_data_id`, `unique_service_name` |
| **Summary** | `print_summary` |

## Environment Variables

### Standalone Modes

| Variable | Default | Description |
|----------|---------|-------------|
| `MAIN_PORT` | `8848` | Main HTTP server port |
| `CONSOLE_PORT` | `8081` | Console HTTP server port |
| `USERNAME` | `nacos` | Auth username |
| `PASSWORD` | `nacos` | Auth password |

### Cluster Modes

| Variable | Default | Description |
|----------|---------|-------------|
| `NODE1_MAIN_PORT` | `8848` | Node 1 main port |
| `NODE1_CONSOLE_PORT` | `8081` | Node 1 console port |
| `NODE2_MAIN_PORT` | `8858` | Node 2 main port |
| `NODE2_CONSOLE_PORT` | `8082` | Node 2 console port |
| `NODE3_MAIN_PORT` | `8868` | Node 3 main port |
| `NODE3_CONSOLE_PORT` | `8083` | Node 3 console port |

Example with custom ports:

```bash
MAIN_PORT=9848 CONSOLE_PORT=9081 ./scripts/test.sh console standalone-embedded
```

## Test Coverage

Each test script covers the following areas in order:

| # | Area | Endpoints Tested |
|---|------|-----------------|
| 1 | **Authentication** | `POST /v3/auth/user/login` (both console and main ports) |
| 2 | **Health** | `GET /v3/console/health/liveness`, `/readiness` |
| 3 | **Server State** | `GET /v3/console/server/state` |
| 4 | **Cluster Info** | `/core/cluster/nodes`, `/self`, `/health`, `/count`, `/standalone`, `/leader` |
| 5 | **Namespace CRUD** | `/core/namespace` - list, create, get, update, delete, exist check |
| 6 | **Config CRUD** | `/cs/config` - publish, get, search, update, delete, cross-namespace |
| 7 | **Config Gray/Beta** | `/cs/config/beta` - publish, get, delete |
| 8 | **Config History** | `/cs/history/list`, `/cs/history?nid=` |
| 9 | **Service Discovery** | Register instance (V2 API), list services/instances via console |
| 10 | **Cleanup** | Delete all test resources, verify deletion |

### Cluster-Specific Tests

Cluster scripts additionally test:

| Area | What is Verified |
|------|-----------------|
| **Cluster Formation** | All 3 nodes see each other, state=UP |
| **Raft Leader** | Leader elected, leader address consistent (embedded only) |
| **Data Consistency** | Config written on Node 1 readable on Node 2/3 |
| **Write Forwarding** | Follower writes succeed (forwarded to leader, embedded only) |
| **Distro Sync** | Instances registered on different nodes visible from all nodes |
| **Cross-Node Cleanup** | Resources deleted on one node confirmed gone on others |

## Storage Mode Decision Logic

The storage mode is determined by `batata.sql.init.platform` configuration, with priority **higher** than the standalone/cluster flag:

```
batata.sql.init.platform = "mysql" or "postgresql"
    → ExternalDb (regardless of standalone/cluster)

batata.sql.init.platform = empty + standalone mode
    → StandaloneEmbedded (single-node RocksDB)

batata.sql.init.platform = empty + cluster mode
    → DistributedEmbedded (Raft + RocksDB)
```

This maps to console datasource implementations:

| Storage Mode | Console DataSource | Description |
|-------------|-------------------|-------------|
| ExternalDb | `LocalDataSource` | Direct SeaORM database queries |
| StandaloneEmbedded | `EmbeddedLocalDataSource` | Direct PersistenceService (RocksDB) |
| DistributedEmbedded | `EmbeddedLocalDataSource` | PersistenceService via Raft consensus |

## Port Architecture

Each Batata server node uses 4 ports:

| Port | Offset | Role |
|------|--------|------|
| Main HTTP | base (8848) | V2/V3 API for SDK clients |
| Console HTTP | configurable (8081) | Console admin API |
| SDK gRPC | base + 1000 (9848) | Client SDK communication |
| Cluster gRPC | base + 1001 (9849) | Inter-node cluster communication |

## SDK Compatibility Tests

`scripts/test.sh sdk` runs the **official SDK client test suites** against a real Batata
server with the corresponding compatibility plugin enabled. The suites live under
[`sdk-tests/`](../../sdk-tests) and are executed as Podman (or Docker) Compose test
containers so they do not require a local JVM/Go toolchain to be installed on the host.

| Suite | Language | Batata plugin | Default endpoint |
|-------|----------|---------------|------------------|
| `nacos-java-tests`  | Java (Maven)  | Nacos (`BATATA_PLUGIN_CONSUL_ENABLED` not needed) | `nacos.server=batata:8848` |
| `consul-go-tests`   | Go (`go test`)| Consul (`BATATA_PLUGIN_CONSUL_ENABLED=true`)       | `CONSUL_HTTP_ADDR=batata:8500` |
| `apollo-java-tests` | Java (Maven)  | Apollo (`BATATA_PLUGIN_APOLLO_ENABLED=true`)       | `apollo.meta=batata:18080` |

> The Apollo plugin does not verify the OpenAPI token by default (`openapi_auth_enabled=false`),
> so the test token `admin` works out of the box. In the `all sdk` stack the batata service
> additionally sets `BATATA_PLUGIN_CONSUL_ENABLED=true` and `BATATA_PLUGIN_APOLLO_ENABLED=true`.

### Prerequisites

- `podman` + `podman-compose` (or `docker` + `docker compose`) available on `PATH`
- Network access to pull base images and (for the Java suites) download Maven dependencies

### Run via test.sh

```bash
# Quick: standalone + embedded, runs nacos + consul + apollo against a local cluster
./scripts/test.sh sdk --quick all

# Matrix: embedded + MySQL + 3-node cluster variants, each with nacos/consul/apollo
./scripts/test.sh sdk --matrix all

# Full: build & run the whole SDK stack with Podman Compose (MySQL + batata + test containers)
./scripts/test.sh all sdk
```

Filter by suite or test class (apollo only shows as a single aggregate here):

```bash
./scripts/test.sh sdk --quick nacos
./scripts/test.sh sdk --matrix apollo
```

### Run a suite manually against a local Batata

Start Batata with the required plugin(s) enabled, then run the suite directly:

```bash
# Nacos (Java)
cd sdk-tests/nacos-java-tests
mvn test -Dnacos.server=127.0.0.1:8848 -Dnacos.username=nacos -Dnacos.password=nacos

# Consul (Go)
cd sdk-tests/consul-go-tests
CONSUL_HTTP_ADDR=http://127.0.0.1:8500 CONSUL_HTTP_TOKEN=root go test ./... -v -count=1

# Apollo (Java)
cd sdk-tests/apollo-java-tests
mvn test -Dapollo.meta=http://127.0.0.1:18080 \
         -Dapollo.openapi.url=http://127.0.0.1:18080 \
         -Dapollo.openapi.token=admin
```

To run only a subset of the Apollo test classes, pass `-Dtest=ApolloConfig*` (or any
prefix of the class name, e.g. `ApolloGray`, `ApolloCluster`).

### Standalone debug (local cluster with Apollo enabled)

Use `startup.sh cluster` to spin up a 3-node local cluster with the Apollo plugin on:

```bash
APOLLO_ENABLED=true ./scripts/startup.sh cluster
```

Each node exposes Apollo on its own port (`18080` / `18090` / `18100` for nodes 1–3 by
default); override with `NODE1_APOLLO_PORT` / `NODE2_APOLLO_PORT` / `NODE3_APOLLO_PORT`.

## Troubleshooting

### Server not ready

If tests fail immediately, the server may not be fully started. Wait for the log message:

```
INFO  Batata server started successfully
```

Or use the liveness endpoint:

```bash
curl -s http://127.0.0.1:8081/v3/console/health/liveness
```

### Authentication failures

Verify the default credentials are correct:

```bash
curl -s -X POST http://127.0.0.1:8081/v3/auth/user/login \
  -d "username=nacos&password=nacos"
```

### Cluster nodes not discovering each other

Ensure `conf/cluster.conf` contains all node addresses, or set `batata.member.list` in `conf/application.yml`:

```yaml
batata.member.list: "127.0.0.1:8848,127.0.0.1:8858,127.0.0.1:8868"
```

### Embedded mode but using external DB

If `batata.sql.init.platform` is set to `mysql` or `postgresql` in `conf/application.yml`, the server will use ExternalDb mode even if started with `-m standalone`. Clear the platform value for embedded mode:

```yaml
batata.sql.init.platform:   # empty = embedded mode
```

## Regression Gate

`scripts/regression.sh` is the single entry point that replaces the handful of
manual commands "run everything" used to mean. It runs the stages, collects
**every** failure, and then decides — a stage failing does not stop the run.

```bash
./scripts/regression.sh                # quick (default): pre-commit self-check
./scripts/regression.sh --standard     # + all core crates + Java/Go SDK suites
./scripts/regression.sh --full         # + 6-combination SDK matrix
./scripts/regression.sh --full --perf  # + performance anchors
./scripts/regression.sh --only unit    # a single stage
```

| Level | Stages |
|-------|--------|
| `quick` | `fmt`, `check`, `unit` (core crates), `integration` (standalone + Rust SDK) |
| `standard` | + `unit` (all core crates), `integration` (Java + Go SDK) |
| `full` | + `matrix` (standalone/cluster x rock/mysql/postgres), `unit` incl. apollo |

Exit code is `0` only when nothing outside the whitelist failed.

### Reports

Written to `reports/regression/<timestamp>/report.md` (gitignored), with
`reports/regression/latest` symlinked to the most recent run. Per-stage logs
are under `reports/regression/<timestamp>/logs/`. Nothing is written to `/tmp`.

```
## Stages        | Stage | Status | Detail | Time |
## Blockers      failures that are NOT whitelisted — these fail the gate
## Known failures (whitelisted)  failure -> reason, from regression_whitelist.txt
```

### The whitelist

`scripts/regression_whitelist.txt` holds the known-failure baseline:

```
<stage>|<pattern>|<reason>
```

`<pattern>` is matched as a substring of the failing item (`crate::test::path`
for `unit`, `<suite>: <name>` for `integration`/`matrix`).

Rules:

- Every entry needs a reason **and** the date it was observed. An entry without
  a reason is a suppressed bug, not a baseline.
- Narrow the pattern to the specific test; never whitelist a whole crate.
- **Delete the entry in the same commit that fixes it.** Otherwise the gate
  silently passes a test that no longer runs.

To check the gate actually bites, remove an entry and re-run: the failure must
reappear as a blocker.

### Performance gate

`--perf` runs `bench_ab.py` for the `naming` and `config` cases and compares
each median against `scripts/regression_perf_baseline.txt`. A scenario blocks
when its median falls below `anchor * (1 - 0.116)`; the 11.6% tolerance is the
observed run-to-run spread on this machine (Register/Query are ~1%, Config
Write/Read ~12%), so a tighter band would fire on noise.

It needs `target/release/batata-server`; without it the stage is skipped with a
warning rather than failing (a release build is ~25 min and should not be a
surprise side effect of a gate run).

Refresh the anchors with the command recorded in the baseline file's header,
and replace them in the same commit as the change that moved them.

### Notes

- Unit tests run with `--no-fail-fast`: without it cargo stops at the first
  failing test target and hides every suite behind it.
- `SKIP_FMT=1` / `SKIP_CHECK=1` skip a stage; `SDK_MODE=cluster` changes the
  integration backend; `REGRESSION_KEEP_SERVER=1` leaves the server up.
- `REGRESSION_CRATES="batata-core batata-naming"` overrides the level's crate
  set and runs only those crates in the unit stage — use it to re-check one
  crate without paying for the whole workspace (a cold full-level unit run
  exceeds an hour).
- The integration stage delegates to `test.sh sdk --quick`, which owns server
  startup/teardown — do not start a server by hand for it.
- SDK failure names are parsed from all three runner dialects (go `--- FAIL:`,
  cargo's `failures:` block, maven's `[ERROR] Failures:`), so a red suite
  reports named tests instead of one opaque blocker.

## Benchmark & Profiling

Scripts used to measure throughput and to locate CPU hot spots.

| Script | Purpose |
|--------|---------|
| `benchmark.sh` | wrk/hey throughput for config / naming / consul endpoints |
| `init-admin.sh` | One-time admin bootstrap, required before any authenticated run |
| `init-bench-user.sh` | Creates a **non-admin** user so the authorization path is exercised |
| `bench_sync_load.py` | Registers distinct instances to drive cross-node distro sync |
| `bench_ab.py` | A/B driver: alternating rounds, median aggregation, noise-aware verdict |
| `profile.sh` | Captures CPU profiles (perf on Linux, `sample` on macOS) |

### Running the naming benchmark

```bash
./scripts/startup.sh -m standalone
./scripts/init-admin.sh                 # nacos / nacos
./scripts/init-bench-user.sh            # benchuser / benchpass / BENCH_ROLE
./scripts/benchmark.sh naming
```

Benchmark as a **non-admin** user when measuring authorization cost: the
default `nacos` account is a global admin, so `validate_authority` short-circuits
and never loads role and permission bindings.

```bash
env USERNAME=benchuser PASSWORD=benchpass ./scripts/benchmark.sh naming
```

`env` is required: `USERNAME` is a special variable in zsh, so a plain
`USERNAME=benchuser ./scripts/benchmark.sh` leaves the variable unset in the
script and every request goes out unauthenticated.

### A/B comparisons: use `bench_ab.py`, not two manual runs

A single `benchmark.sh` run is **not** reproducible enough to judge changes of
10-20%. Measured on a 16-core macOS box with the load generator running on the
same machine, the spread between repeated runs of one identical configuration
reached **43%** on Instance Register and **49%** on Service List, and the
numbers drifted downwards over a session (RocksDB compaction, background load).
Two manual runs cannot separate that from a real effect.

`bench_ab.py` controls both dominant error sources:

1. **CPU oversubscription** — the server is started with `-d server` (no console
   server) and `--workers` (default 8), so main + console do not consume 32
   threads on 16 cores, and `wrk` gets cores of its own.
2. **Monotonic drift** — variants are measured in alternating order each round
   (AB, then BA, then AB...), which cancels a linear drift out of the medians.
3. **Store growth — the biggest one.** A 30s `naming` run writes several hundred
   thousand registrations, so RocksDB keeps growing and compacting and
   throughput falls steadily across a session (measured: 13905 -> 7884 RPS for
   an unchanged build). Pass `--fresh-store` to give every run its own empty
   embedded store under `--store-root`; it implies `--init`, because a new store
   has no users. This is what makes runs comparable; AB/BA alone cannot cancel
   it. (It is passed as a `--batata.persistence.embedded.data_dir=...` CLI
   override: the `BATATA_*` env mapping rewrites every underscore to a dot, so
   `data_dir` would arrive as `data.dir` and be silently ignored.)

Each variant is measured once per round after a discarded warmup run, and the
per-case **median** is reported with the observed min/max. A difference is only
called `RESOLVED` when it exceeds the spread of both variants — otherwise it is
reported as `within noise` and must not be quoted as a gain.

```bash
./scripts/bench_ab.py --case naming --rounds 3 --fresh-store \
    --variant off=BATATA_CORE_AUTH_CACHING_ENABLED=false \
    --variant on=
```

Useful flags: `--rounds` (default 3, minimum 2), `--warmup-rounds` (leading rounds
that run but are discarded), `--duration` (30s), `--warmup` (10s, discarded),
`--workers` (8), `--threads` / `--connections` for wrk, `--settle` (idle seconds
between variants), `--fresh-store` / `--store-root` (isolated empty store per
run), `--init` (run the init scripts per variant), `--out` (CSV path). The port
must be free before starting.

Every node in every mode is started with `--batata.server.http.access_log.enabled=false`
(`COMMON_ARGS`): the access log is on by default and costs milliseconds per
request, which is larger than the effects this driver resolves.

Before measuring, the driver logs in and registers one instance and requires the
body to say `code: 0`. Batata answers most failures with **HTTP 200 and an error
code inside the body**, so a variant that rejects everything looks *faster*
(sub-millisecond) rather than broken, and `wrk` reports no non-2xx. A variant
that never passes the probe is skipped instead of producing a fake win.

Concurrency changes the answer: at `-c20` the RBAC cache measured +37% to +54%
and resolved cleanly, while at `-c100` (server saturated) the same comparison was
dominated by queueing and spread up to 90%. Measure below saturation when the
question is "is this path cheaper", and at saturation only when the question is
"what is the ceiling".

Interpreting the output:

```
Instance Register
  off        median      9952   min      9701   max     10240   spread      539 ( 5.4%)
  on         median      9988   min      9810   max     10112   spread      302 ( 3.0%)
  -> on vs off: +36 RPS (+0.4%), noise floor 539 => within noise
```

If every case reports `within noise`, the honest conclusion is that the change
has no *measurable* throughput effect at this load — switch to a CPU profile
(`scripts/profile.sh`) or a lower-concurrency latency test instead of quoting
the median delta.

### Cluster mode

`--mode cluster` starts a real raft cluster instead of a single node. Node 1
(`--base-port`, default 8848) is the one being measured; the peers listen on
`base-port + 10*i` and only absorb raft + distro traffic, so they run with
`--peer-workers` (default 2) HTTP workers to leave cores for `wrk`.

```bash
./scripts/bench_ab.py --mode cluster --nodes 3 --case naming --rounds 3 \
    --fresh-store \
    --variant off=BATATA_CORE_AUTH_CACHING_ENABLED=false \
    --variant on=
```

Notes:

- `--fresh-store` creates one store directory per node (`<store>/node1..nodeN`).
- The driver waits for every node's health probe and then `--cluster-settle`
  seconds (default 10) so raft elects a leader before the first request;
  writing before election fails and would be measured as a fast error.
- The gRPC (+1000) and raft (-1000) sibling ports of every node must be free;
  the driver refuses to start otherwise.
- Cluster throughput is lower than standalone for the same load: every write
  goes through raft and every registration is fanned out by distro. Compare
  variants inside one mode, never across modes.

### Comparing the RBAC cache on and off

```bash
BATATA_CORE_AUTH_CACHING_ENABLED=false ./target/release/batata-server -d merged -m standalone
# equivalent: --batata.core.auth.caching.enabled=false
# or: batata.core.auth.caching.enabled: false in conf/application.yml
```

This is the Batata counterpart of Nacos' `nacos.core.auth.caching.enabled`.
When false every auth lookup reads straight from the storage layer, so the
cache contribution can be measured on a single binary. The same key also
gates the token cache; `batata.core.auth.cache.*` sizes the role and
permission caches.

Turn the cache on and off with the configuration key above and compare with
`bench_ab.py` (see the A/B section):

```bash
./scripts/bench_ab.py --case naming --rounds 3 \
    --variant off=BATATA_CORE_AUTH_CACHING_ENABLED=false \
    --variant on=
```

Do not compare a `sample`/`perf` run against a plain run: a concurrent profiler
distorts throughput, so CPU profiles and throughput runs must be separate.
