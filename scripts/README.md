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
| **Standalone + Embedded** | RocksDB | 1 | `spring.sql.init.platform` = empty |
| **Standalone + ExternalDb** | MySQL/PostgreSQL | 1 | `spring.sql.init.platform` = mysql/postgresql |
| **Cluster + ExternalDb** | MySQL/PostgreSQL (shared) | 3 | `spring.sql.init.platform` = mysql/postgresql |
| **Cluster + Embedded** | Raft + RocksDB | 3 | `spring.sql.init.platform` = empty |

The storage mode is determined by `spring.sql.init.platform` in `conf/application.yml`, **independent** of standalone/cluster mode. See [Storage Mode Decision Logic](#storage-mode-decision-logic) for details.

## Quick Start

### 1. Standalone + Embedded (RocksDB)

No external database required. Simplest mode to test.

```bash
# Ensure spring.sql.init.platform is empty in conf/application.yml
# Start server
cargo run -p batata-server -- -m standalone &

# Wait for server to be ready, then run tests
./scripts/run_console_tests.sh standalone-embedded
```

### 2. Standalone + ExternalDb (MySQL/PostgreSQL)

```bash
# Initialize database
mysql -u user -p batata < conf/mysql-schema.sql

# Ensure spring.sql.init.platform: mysql in conf/application.yml
# Start server
cargo run -p batata-server -- -m standalone --db-url "mysql://user:pass@localhost:3306/batata" &

# Run tests
./scripts/run_console_tests.sh standalone-externaldb
```

### 3. Cluster + ExternalDb (3-node)

All nodes share the same external database for config storage. Naming service uses distro protocol for ephemeral instance sync.

```bash
# Initialize database
mysql -u user -p batata < conf/mysql-schema.sql

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
./scripts/run_console_tests.sh cluster-externaldb
```

### 4. Cluster + Embedded (Raft + RocksDB, 3-node)

No external database. Config storage uses Raft consensus + RocksDB.

```bash
# Create conf/cluster.conf
echo "127.0.0.1:8848
127.0.0.1:8858
127.0.0.1:8868" > conf/cluster.conf

# Ensure spring.sql.init.platform is empty in conf/application.yml
# Start 3 nodes (each needs separate data dir)
cargo run -p batata-server -- -m cluster \
  --main-port 8848 --console-port 8081 &

cargo run -p batata-server -- -m cluster \
  --main-port 8858 --console-port 8082 &

cargo run -p batata-server -- -m cluster \
  --main-port 8868 --console-port 8083 &

# Run tests
./scripts/run_console_tests.sh cluster-embedded
```

## File Structure

```
scripts/
├── README.md                       # This file
├── run_console_tests.sh            # Main entry point / test runner
├── test_utils.sh                   # Shared utilities (login, HTTP, assertions)
├── test_standalone_embedded.sh     # Standalone + RocksDB tests
├── test_standalone_externaldb.sh   # Standalone + MySQL/PostgreSQL tests
├── test_cluster_externaldb.sh      # 3-node Cluster + ExternalDb tests
└── test_cluster_embedded.sh        # 3-node Cluster + Raft+RocksDB tests
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
MAIN_PORT=9848 CONSOLE_PORT=9081 ./scripts/run_console_tests.sh standalone-embedded
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

The storage mode is determined by `spring.sql.init.platform` configuration, with priority **higher** than the standalone/cluster flag:

```
spring.sql.init.platform = "mysql" or "postgresql"
    → ExternalDb (regardless of standalone/cluster)

spring.sql.init.platform = empty + standalone mode
    → StandaloneEmbedded (single-node RocksDB)

spring.sql.init.platform = empty + cluster mode
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

Ensure `conf/cluster.conf` contains all node addresses, or set `nacos.member.list` in `conf/application.yml`:

```yaml
nacos.member.list: "127.0.0.1:8848,127.0.0.1:8858,127.0.0.1:8868"
```

### Embedded mode but using external DB

If `spring.sql.init.platform` is set to `mysql` or `postgresql` in `conf/application.yml`, the server will use ExternalDb mode even if started with `-m standalone`. Clear the platform value for embedded mode:

```yaml
spring.sql.init.platform:   # empty = embedded mode
```
