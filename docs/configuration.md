# Configuration

Main configuration file: `conf/application.yml` — this file is fully commented and is the
single source of truth for all configuration keys.

All configuration keys use the flat `batata.*` prefix. Keys can be overridden via:

- **Environment variables**: `BATATA_*` (e.g., `BATATA_SERVER_MAIN_PORT=9090`)
- **CLI property overrides**: `--batata.server.main.port=9090`

```yaml
# Server
batata.server.main.port: 8848
batata.server.context_path: /nacos
batata.standalone: true
#batata.deployment.type: merged

# Console
batata.console.port: 8081
batata.console.context_path:

# Database
batata.sql.init.platform: mysql
batata.db.url: "mysql://user:password@localhost:3306/batata"
batata.db.pool.max_connections: 100
batata.db.pool.min_connections: 10
batata.db.migration.enabled: false

# Authentication
batata.core.auth.enabled: true
batata.core.auth.system.type: nacos
batata.core.auth.plugin.nacos.token.secret.key: "your-base64-encoded-secret-key"
batata.core.auth.plugin.nacos.token.expire.seconds: 18000
```

## Environment Variables

```bash
export BATATA_DB_URL="mysql://user:pass@localhost:3306/batata"
export BATATA_SERVER_MAIN_PORT=8848
export BATATA_CORE_AUTH_ENABLED=true
export RUST_LOG=info
```

## Configuration Reference

### Server

| Key | Default | Description |
|-----|---------|-------------|
| `batata.server.main.port` | `8848` | Main HTTP server port |
| `batata.server.context_path` | `/nacos` | Server web context path |
| `batata.standalone` | `true` | Standalone mode (true=single, false=cluster) |
| `batata.deployment.type` | `merged` | Deployment type: merged/server/console/serverWithMcp |

### Console

| Key | Default | Description |
|-----|---------|-------------|
| `batata.console.port` | `8081` | Console HTTP server port |
| `batata.console.context_path` | _(empty)_ | Console web context path |
| `batata.console.remote.server_addr` | _(empty)_ | Remote server address (console-only mode) |
| `batata.console.remote.username` | _(empty)_ | Remote server username |
| `batata.console.remote.password` | _(empty)_ | Remote server password |

### Database

| Key | Default | Description |
|-----|---------|-------------|
| `batata.sql.init.platform` | _(empty)_ | Storage platform: `mysql`, `postgresql`, or empty (=embedded) |
| `batata.db.url` | _(empty)_ | Database connection URL |
| `batata.db.pool.max_connections` | `100` | Max pool connections |
| `batata.db.pool.min_connections` | `10` | Min pool connections |
| `batata.db.pool.connect_timeout` | `30` | Connection timeout (seconds) |
| `batata.db.pool.acquire_timeout` | `30` | Acquire timeout (seconds) |
| `batata.db.pool.idle_timeout` | `600` | Idle timeout (seconds) |
| `batata.db.pool.max_lifetime` | `1800` | Max connection lifetime (seconds) |
| `batata.db.migration.enabled` | `false` | Auto-run database migrations on startup |

### Authentication

| Key | Default | Description |
|-----|---------|-------------|
| `batata.core.auth.enabled` | `true` | Enable authentication |
| `batata.core.auth.admin.enabled` | `true` | Enable admin API auth |
| `batata.core.auth.console.enabled` | `true` | Enable console API auth |
| `batata.core.auth.system.type` | `nacos` | Auth type: `nacos`, `ldap`, or `oauth` |
| `batata.core.auth.plugin.nacos.token.secret.key` | _(required)_ | JWT secret (Base64-encoded) |
| `batata.core.auth.plugin.nacos.token.expire.seconds` | `18000` | Token expiry (seconds) |
| `batata.core.auth.server.identity.key` | _(empty)_ | Server identity key for inter-node trust |
| `batata.core.auth.server.identity.value` | _(empty)_ | Server identity value |

### Rate Limiting

| Key | Default | Description |
|-----|---------|-------------|
| `batata.ratelimit.enabled` | `false` | Enable API rate limiting |
| `batata.ratelimit.max_requests` | `10000` | Max requests per window |
| `batata.ratelimit.window_seconds` | `60` | Rate limit window (seconds) |
| `batata.ratelimit.auth.enabled` | `false` | Enable auth rate limiting |
| `batata.ratelimit.auth.max_attempts` | `5` | Max auth attempts per window |
| `batata.ratelimit.auth.lockout_seconds` | `300` | Auth lockout duration (seconds) |

### Observability

| Key | Default | Description |
|-----|---------|-------------|
| `batata.otel.enabled` | `false` | Enable OpenTelemetry |
| `batata.otel.endpoint` | `http://localhost:4317` | OTLP gRPC endpoint |
| `batata.otel.service_name` | `batata` | Service name for traces |
| `batata.otel.sampling_ratio` | `0.1` | Sampling ratio (0.0 to 1.0) |
| `batata.otel.export_timeout_secs` | `10` | Export timeout (seconds) |
| `batata.logs.path` | `logs` | Log directory |
| `batata.logs.level` | `info` | Log level: trace/debug/info/warn/error |
| `batata.logs.console.enabled` | `true` | Enable console (stdout) logging |
| `batata.logs.file.enabled` | `true` | Enable file logging |

### Cluster

| Key | Default | Description |
|-----|---------|-------------|
| `batata.member.list` | _(empty)_ | Comma-separated member addresses (ip:port) |
| `batata.core.member.lookup.type` | `file` | Member lookup type: `file` or `address-server` |

### Config Module

| Key | Default | Description |
|-----|---------|-------------|
| `batata.config.retention.days` | `30` | Config history retention days |
| `batata.config.encryption.enabled` | `false` | Enable config encryption |
| `batata.config.encryption.plugin.type` | `aes-gcm` | Encryption algorithm |
| `batata.config.encryption.key` | _(empty)_ | Encryption key (32-byte Base64-encoded) |

### Service Mesh (xDS)

| Key | Default | Description |
|-----|---------|-------------|
| `batata.mesh.xds.enabled` | `false` | Enable xDS protocol |
| `batata.mesh.xds.port` | `15010` | xDS gRPC server port |
| `batata.mesh.xds.server.id` | `batata-xds-server` | xDS server identifier |
| `batata.mesh.xds.sync.interval.ms` | `5000` | Service sync interval (ms) |
| `batata.mesh.xds.generate.listeners` | `true` | Generate LDS resources |
| `batata.mesh.xds.generate.routes` | `true` | Generate RDS resources |
| `batata.mesh.xds.default.listener.port` | `15001` | Default listener port |

### Consul Compatibility

| Key | Default | Description |
|-----|---------|-------------|
| `batata.plugin.consul.enabled` | `false` | Enable Consul API compatibility |
| `batata.plugin.consul.port` | `8500` | Consul API port |
| `batata.plugin.consul.datacenter` | `dc1` | Consul datacenter name |

### Apollo Compatibility Plugin

| Key | Default | Description |
|-----|---------|-------------|
| `batata.plugin.apollo.enabled` | `true` | Enable Apollo Config Service / Admin Service / OpenAPI compatibility |
| `batata.plugin.apollo.port` | `8080` | Apollo HTTP API port (use independent ports per node in cluster mode, e.g. 18080/18081/18082) |
| `batata.plugin.apollo.openapi.token` | `admin` | Default token for Apollo OpenAPI access-key auth |

### AI / MCP Registry

| Key | Default | Description |
|-----|---------|-------------|
| `batata.ai.mcp.registry.enabled` | `false` | Enable MCP Registry |
| `batata.ai.mcp.registry.port` | `9080` | MCP Registry port |

## OpenTelemetry Configuration

Enable distributed tracing with OpenTelemetry OTLP export:

**application.yml:**

```yaml
batata.otel.enabled: true
batata.otel.endpoint: "http://localhost:4317"
batata.otel.service_name: "batata"
batata.otel.sampling_ratio: 1.0
batata.otel.export_timeout_secs: 10
```

**Environment Variables (override config file):**

```bash
export BATATA_OTEL_ENABLED=true
export BATATA_OTEL_ENDPOINT="http://localhost:4317"
export BATATA_OTEL_SERVICE_NAME="batata"
export BATATA_OTEL_SAMPLING_RATIO=1.0
export BATATA_OTEL_EXPORT_TIMEOUT_SECS=10
```

**Compatible backends:** Jaeger, Zipkin, Tempo, Datadog, Honeycomb, and any OTLP-compatible collector.

## Multi-datacenter Configuration

Enable locality-aware clustering with datacenter, region, and zone settings:

```bash
export BATATA_DATACENTER="dc1"
export BATATA_REGION="us-east"
export BATATA_ZONE="zone-a"
export BATATA_LOCALITY_WEIGHT=1.0
export BATATA_CROSS_DC_REPLICATION=true
export BATATA_CROSS_DC_SYNC_DELAY_SECS=1
export BATATA_REPLICATION_FACTOR=1
```

**Features:**

- Locality-aware member selection (local-first sync)
- Cross-datacenter replication with configurable delay
- Automatic datacenter topology discovery
- Region/zone hierarchy support

## xDS / Service Mesh Configuration

```yaml
batata.mesh.xds.enabled: true
batata.mesh.xds.port: 15010
batata.mesh.xds.server.id: "batata-xds-server"
batata.mesh.xds.sync.interval.ms: 5000
batata.mesh.xds.generate.listeners: true
batata.mesh.xds.generate.routes: true
batata.mesh.xds.default.listener.port: 15001
```

**Supported xDS Resources:**

- **EDS** (Endpoint Discovery Service) - Service endpoints
- **CDS** (Cluster Discovery Service) - Upstream clusters
- **LDS** (Listener Discovery Service) - Listeners configuration
- **RDS** (Route Discovery Service) - Route configuration
- **ADS** (Aggregated Discovery Service) - All resources via single stream

**Compatible with:** Envoy Proxy, Istio, and any xDS-compatible service mesh.

## Metrics

Prometheus metrics are available at `/nacos/actuator/prometheus`:

```
# HELP batata_http_requests_total Total HTTP requests
# TYPE batata_http_requests_total counter
batata_http_requests_total{method="GET",path="/nacos/v2/cs/config"} 1234

# HELP batata_config_count Current configuration count
# TYPE batata_config_count gauge
batata_config_count{namespace="public"} 100

# HELP batata_service_count Current service count
# TYPE batata_service_count gauge
batata_service_count{namespace="public"} 50

# HELP batata_cluster_member_count Cluster member count
# TYPE batata_cluster_member_count gauge
batata_cluster_member_count 3
```
