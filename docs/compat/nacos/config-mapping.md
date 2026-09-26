# Nacos Configuration Mapping

This document maps Nacos configuration keys to their Batata equivalents. It exists
so that upstream Nacos configuration changes can be reviewed **without** copying
Nacos files into `conf/`.

- Upstream Nacos source: `distribution/conf/` (see the local checkout, e.g. `~/work/github/easynet-cn/nacos`)
- Upstream Nacos binary distribution: `~/work/nacos/conf/`
- Nacos reference files kept for comparison: [`conf/`](conf/) in this directory
- To diff against a local Nacos distribution: `scripts/diff-nacos-conf.sh`

## Naming convention

| | Nacos | Batata |
|---|---|---|
| Prefix | `nacos.` (plus Spring keys, e.g. `server.*`, `spring.*`, `db.*`) | flat `batata.*` |
| File | `application.properties` | `conf/application.yml` |
| Override | JVM `-D` / env | `BATATA_*` env or `--batata.<key>=<value>` CLI |

## Key mapping

### Server

| Nacos | Batata | Notes |
|---|---|---|
| `nacos.server.main.port` | `batata.server.main.port` | both default `8848` |
| `nacos.server.contextPath` | `batata.server.context_path` | both default `/nacos` |
| `nacos.console.port` | `batata.console.port` | ⚠️ Nacos `8080`, Batata **`8081`** (Batata reserves 8080 for the Apollo plugin) |
| `nacos.console.contextPath` | `batata.console.context_path` | both empty by default |
| `nacos.console.ui.enabled` | `batata.console.ui.enabled` | Batata serves [`batata-ui`](https://github.com/easynet-cn/batata-ui) from `console-ui/` |
| _(none)_ | `batata.deployment.type` | Batata-only: `merged` / `server` / `console` / `serverWithMcp` |

### Datasource

| Nacos | Batata | Notes |
|---|---|---|
| `spring.sql.init.platform` | `batata.sql.init.platform` | `mysql` / `postgresql` / empty (= embedded RocksDB) |
| `db.url.0` (JDBC URL) | `batata.db.url` | Batata uses a plain URL, not JDBC |
| `db.user` / `db.password` | _(encoded in `batata.db.url`)_ | |
| `db.num` | _(N/A)_ | Batata uses a single datasource |
| _(none)_ | `batata.db.migration.enabled` | Batata-only: SeaORM auto-migration (recommended) |

### Cluster

| Nacos | Batata | Notes |
|---|---|---|
| `nacos.member.list` | `batata.member.list` | comma-separated `ip:port`; Batata also falls back to `conf/cluster.conf` |
| `nacos.core.member.lookup.type` | `batata.core.member.lookup.type` | `file` / `address-server` |
| `nacos.core.protocol.raft.data.*` | `batata.raft.*` | Batata uses OpenRaft, see `conf/application.yml` |
| `nacos.core.protocol.distro.data.*` | _(internal)_ | Distro (AP) timings are not exposed as config keys |

### Auth

| Nacos | Batata | Notes |
|---|---|---|
| `nacos.core.auth.enabled` | `batata.core.auth.enabled` | ⚠️ Nacos default `false`, Batata default **`true`** |
| `nacos.core.auth.admin.enabled` | `batata.core.auth.admin.enabled` | |
| `nacos.core.auth.console.enabled` | `batata.core.auth.console.enabled` | |
| `nacos.core.auth.system.type` | `batata.core.auth.system.type` | `nacos` / `ldap` / `oauth` (Nacos calls OIDC `oidc`) |
| `nacos.core.auth.server.identity.key` | `batata.core.auth.server.identity.key` | |
| `nacos.core.auth.server.identity.value` | `batata.core.auth.server.identity.value` | |
| `nacos.core.auth.plugin.nacos.token.secret.key` | `batata.core.auth.plugin.nacos.token.secret.key` | |
| `nacos.core.auth.plugin.nacos.token.expire.seconds` | `batata.core.auth.plugin.nacos.token.expire.seconds` | both default `18000` |
| `nacos.core.auth.caching.enabled` | `batata.core.auth.cache.*` | Batata splits cache TTL/capacity per area |

### gRPC

| Nacos | Batata | Notes |
|---|---|---|
| `nacos.remote.server.grpc.sdk.max-inbound-message-size` | `batata.remote.server.grpc.sdk.max_inbound_message_size` | Nacos uses `-`, Batata uses `_` |
| `nacos.remote.server.grpc.sdk.keep-alive-time` | `batata.remote.server.grpc.sdk.keep_alive_time` | |
| `nacos.remote.server.grpc.sdk.keep-alive-timeout` | `batata.remote.server.grpc.sdk.keep_alive_timeout` | |
| `nacos.remote.server.grpc.sdk.permit-keep-alive-time` | `batata.remote.server.grpc.sdk.permit_keep_alive_time` | |
| `nacos.remote.server.grpc.cluster.*` | `batata.remote.server.grpc.cluster.*` | |

### AI

| Nacos | Batata | Notes |
|---|---|---|
| `nacos.ai.mcp.registry.enabled` | `batata.ai.mcp.registry.enabled` | |
| `nacos.ai.registry.port` (Nacos 3.x) / `nacos.ai.mcp.registry.port` | `batata.ai.mcp.registry.port` | both default `9080` |

### Not ported

| Nacos | Status |
|---|---|
| `nacos.security.ignore.urls` | Batata derives public paths in code |
| `nacos.naming.empty-service.*` | Batata handles empty-service cleanup internally |
| `nacos.istio.mcp.server.enabled` | Batata covers mesh via `batata.mesh.xds.*` |
| `nacos.k8s.sync.*` | Batata K8s sync is part of `batata-plugin-cloud` |
| Oracle / Derby datasources | **Not supported** — Batata supports only SeaORM-backed databases (MySQL, PostgreSQL) plus embedded RocksDB |

## Reference files in `conf/`

Files under [`conf/`](conf/) are upstream Nacos files kept only for comparison.
They are **not** read by Batata at runtime.

| File | Nacos origin |
|---|---|
| `application.properties` | `distribution/conf/application.properties` |
| `cluster.conf.example` | `distribution/conf/cluster.conf.example` |
| `mysql-schema.sql` | Nacos binary `conf/mysql-schema.sql` |
| `pg-schema.sql` | Nacos binary `conf/pg-schema.sql` |
| `pg-grant-nacos-readwrite.sql` | Nacos binary `conf/pg-grant-nacos-readwrite.sql` |
| `pg-upgrade-null-tenant-id.sql` | Nacos binary `conf/pg-upgrade-null-tenant-id.sql` |
| `1.4.0-ipv6_support-update.sql` | `distribution/conf/1.4.0-ipv6_support-update.sql` |
| `mysql-upgrade-visibility-permission-resource.sql` | Nacos (visibility/permission upgrade script) |
| `pg-upgrade-visibility-permission-resource.sql` | Nacos (visibility/permission upgrade script) |

Batata creates and migrates its own schema via SeaORM (`batata.db.migration.enabled`),
so these DDL files are reference material only.

### Files shared with Nacos that Batata *does* load

These live in the runtime `conf/` directory and are read by Batata
(`crates/batata-console/src/v3/server_state.rs`), but their upstream origin is Nacos
`distribution/conf/`:

- `announcement_en-US.conf`
- `announcement_zh-CN.conf`
- `console-guide.conf`
