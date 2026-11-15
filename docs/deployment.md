# Deployment

## Deployment Modes

Batata supports four deployment modes, controlled by `batata.deployment.type` or the `-d` CLI flag:

| Mode | Flag | Description |
|------|------|-------------|
| `merged` (default) | `-d merged` | Console (8081) + Main Server (8848) in same process |
| `server` | `-d server` | Main Server only (8848) + gRPC (9848/9849), no console |
| `console` | `-d console` | Console only (8081), connects to remote server |
| `serverWithMcp` | `-d serverWithMcp` | Main Server + MCP Registry (9080) |

Only `merged` and `console` serve the web console UI (port `8081`).

## Startup Scripts

Convenience scripts are provided in the `scripts/` directory:

```bash
# Start in embedded mode (no database required)
./scripts/startup.sh -m standalone -p embedded

# Start server-only (main server on 8848, no console)
./scripts/startup.sh -d server -m standalone -p embedded

# Start console-only (connects to a remote server)
./scripts/startup.sh -d console -R http://127.0.0.1:8848

# Start with MySQL database
./scripts/startup.sh -m standalone -U "mysql://user:password@localhost:3306/batata"

# Initialize admin user (required on first startup)
./scripts/init-admin.sh [username] [password] [server_url]

# Test console/server route separation
./scripts/test.sh separation
```

## Separated Console Deployment

`-d console` runs the console as its own process. It serves the web UI plus the
`/v3/console/**` and `/v3/auth/**` surfaces, and holds **no** database, Raft log
or RocksDB store — every byte it renders comes from a core node over HTTP.

Addressing: the console resolves the core in this order, so the first match
wins —

1. `batata.member.list`
2. `conf/cluster.conf`
3. `batata.console.remote.server_addr`

Because a checkout usually ships a `conf/cluster.conf`, pin the target
explicitly when the core is not on the address listed there:

```bash
./target/release/batata-server -d console \
  --batata.member.list=127.0.0.1:8848 \
  --batata.console.port=8081
```

Authentication: the console runs the auth plugin **locally**, so token
validation and the RBAC decision use the end user's identity; only the
user/role/permission *data* is fetched from the core. Console-to-core calls
authenticate with the server identity headers when they are configured
(`batata.core.auth.server.identity.key` / `.value`), and fall back to
`batata.console.remote.username` / `.password` otherwise. When auth is enabled,
configure the identity pair so the console does not have to log in as a user.

Known gaps in remote mode (they return an error instead of proxying):

- cluster member state updates (`ConsoleDataSource::cluster_update_member_state`)
- service cluster updates (`ConsoleDataSource::service_update_cluster`)

Everything else on `/v3/console/**` — configuration CRUD, history, namespaces,
services — is proxied and covered by
`crates/batata-console/tests/console_remote_deployment.rs`.

## Console Same-Origin Gateway

The packaged `batata-ui` is a single SPA served by the console port (8081). Its
Consul and Apollo panes call root-relative prefixes `/consul-api` and
`/apollo-api`; the console forwards them to the matching protocol-adapter HTTP
servers. **No API listening port changes** — the browser only needs 8081. Port
8848 stays a pure SDK/Admin surface and serves no UI.

| Key | Default | Purpose |
|-----|---------|---------|
| `batata.console.gateway.enabled` | `true` | Master switch for both routes |
| `batata.console.gateway.consul.target` | empty | Ordered, comma-separated upstream URLs |
| `batata.console.gateway.apollo.target` | empty | Same for Apollo |
| `batata.console.gateway.connect.timeout.secs` | `5` | Connect timeout only; no total request timeout |

Route registration is decided once at startup:

- **No explicit target**: the route is registered only when
  `batata.plugin.<consul|apollo>.enabled` is true in this process, and forwards
  to `127.0.0.1:<batata.plugin.*.port>`. Each cluster node therefore forwards
  to its own plugin server (8500/8510/8520, 18080/18090/18100).
- **Explicit target**: the route is registered even when the local plugin is
  disabled. This is required for `-d console`, where the console process never
  starts plugin servers. Point it at a server node's plugin port.
- **Disabled or unused route**: the scope is not registered and the prefix
  returns 404 instead of the SPA shell (HTML 200).

Multiple targets are **sticky**: every request uses the current node, and only
a transport error switches to the next one (matching `batata-client`'s
connection failover). Request-level load balancing is deliberately avoided —
Consul blocking queries and `/v1/health/stream` SSE hold per-process
subscriptions. Responses are streamed without a total timeout, so `?wait=5m`
long polls are not cut short; hop-by-hop headers are dropped and upstream
`Content-Encoding` is preserved.

Example for a separated console against a multi-node cluster:

```bash
./target/release/batata-server -d console \
  --batata.console.port=8081 \
  --batata.console.gateway.consul.target=http://10.0.0.1:8500,http://10.0.0.2:8510 \
  --batata.console.gateway.apollo.target=http://10.0.0.1:18080
```

Security: upstream targets come from configuration only, never from request
headers or query parameters. Enabling the gateway exposes the plugin APIs on
8081, so keep the console's rate limiter enabled and restrict that port.

### Default-off plugins and the UI contract

Both protocol adapters are **disabled by default** in `conf/application.yml`
and in the typed configuration (`PluginConsulConfig` / `PluginApolloConfig`).
With no opt-in, the gateway registers no scope, `/consul-api/**` and
`/apollo-api/**` fail closed with 404, and the console UI shows no Consul or
Apollo provider entry.

The console advertises the effective state through
`GET /v3/console/server/state`, which always returns string flags
`consul_enabled` and `apollo_enabled` (`"true"` / `"false"`), even when a
disabled plugin never registers a state provider. The frontend uses these two
keys — not a build-time flag — to decide whether the provider switcher is
rendered. Do not add a UI entry that bypasses this check.

Enable the plugins only for the run that needs them, **without editing the
config file**:

```bash
# CLI (highest priority)
./target/debug/batata-server -d merged -m standalone \
  --batata.plugin.consul.enabled=true \
  --batata.plugin.apollo.enabled=true

# Or environment variables
BATATA_PLUGIN_CONSUL_ENABLED=true BATATA_PLUGIN_APOLLO_ENABLED=true \
  ./target/debug/batata-server -d merged -m standalone
```

`--dotted.key=value` is the supported CLI form (`--key value` is not parsed).
Both forms override the YAML file and leave its default-off values untouched.

### Running the UI integration tests

`gateway.spec.ts` is production-shaped: it runs against the console port
(8081), not the Vite dev server, and derives its expectations from
`/v3/console/server/state`, so the same spec validates both the enabled shape
(upstream JSON, provider entries visible) and the default-off shape (404,
no entries).

```bash
# 1. Start batata with the plugins enabled (see above).
# 2. Build and sync the packaged UI into batata/console-ui.
cd ../batata-ui && pnpm build:ui
# 3. Run the gateway spec against 8081.
pnpm exec playwright test --config e2e/playwright.config.ts gateway.spec.ts
```

If Playwright's bundled Chromium is not installed, set `E2E_CHROME_PATH` to a
local Chrome executable (for example
`/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`); the setting
is optional and does not change the default behaviour. To install the bundled
browser instead, run `pnpm exec playwright install chromium` yourself.

Implementation note: the console context path is empty by default, so its
`web::scope("")` is a catch-all and actix does not fall back to sibling
services. The two gateway scopes are therefore registered **before** the
console scope; registering them afterwards makes every gateway request a 404
even when the plugins are enabled.

## Standalone Mode

```bash
# Run directly
cargo run --release -p batata-server

# Or with explicit standalone flag
./target/release/batata-server --batata.standalone=true
```

## Cluster Mode

1. Configure `conf/cluster.conf` with member addresses:

   ```
   192.168.1.1:8848
   192.168.1.2:8848
   192.168.1.3:8848
   ```

2. Update `conf/application.yml`:

   ```yaml
   batata.standalone: false
   #batata.member.list: "192.168.1.10:8848,192.168.1.11:8848,192.168.1.12:8848"
   ```

3. Start all nodes

**Upgrading the consensus layer.** The OpenRaft 0.9 to 0.10 upgrade is
lockstep: stop all nodes at once, wipe their RocksDB data directories, then
start the fresh cluster and re-initialize membership. Rolling or mixed-version
restarts are not supported. See [Database](database.md#consensus-data-on-upgrade-openraft-010)
for the full procedure, and export config data before the wipe.

## Podman

The image is built from the [`Containerfile`](../../Containerfile) at the
repository root:

```bash
# Build image
podman build -f Containerfile -t batata:latest .

# Run container
podman run -d \
  -p 8848:8848 \
  -p 8081:8081 \
  -p 9848:9848 \
  -v $(pwd)/conf:/app/conf \
  batata:latest
```

## Podman Compose

Compose files live in `deploy/compose/`. Run them from the repository root:

```bash
# Embedded mode (no DB)
podman-compose -f deploy/compose/podman-compose.yml up batata

# With MySQL
podman-compose -f deploy/compose/podman-compose.yml --profile mysql up

# With PostgreSQL
podman-compose -f deploy/compose/podman-compose.yml --profile postgres up

# 3-node cluster
podman-compose -f deploy/compose/podman-compose.yml --profile cluster up

# Server + Console separated
podman-compose -f deploy/compose/podman-compose.yml --profile split up
```

`scripts/podman.sh up` wraps these invocations.

## Docker Compose

`deploy/compose/docker-compose.yml` uses a pre-built image instead of building
from source — this is the file shipped in the release tarball:

```bash
docker compose -f deploy/compose/docker-compose.yml --profile mysql up
```

## Kubernetes

Manifests for a single-replica Deployment are provided under `deploy/k8s/`:

- `configmap.yaml` — runtime configuration
- `deployment.yaml` — batata container (mounts the ConfigMap at `/app/conf`)
- `service.yaml` — ClusterIP Service exposing the console/main/gRPC ports

By default the ConfigMap ships **only two files** that batata needs at runtime:

- `application.yml` — main server configuration
- `cluster.conf` — cluster peer addresses (one `host:port` per line)

These are mounted at `/app/conf`, overriding the baked-in `conf/` directory, so
the pod's `/app/conf` contains exactly those two files. The `*.sql` files under
`conf/` are reference DDL and are **not** used at runtime (batata creates its
schema automatically via SeaORM migrations).

The committed `configmap.yaml` is generated from the real files. Regenerate it
after editing `conf/application.yml` or `conf/cluster.conf`:

```bash
kubectl create configmap batata-config \
  --from-file=application.yml=conf/application.yml \
  --from-file=cluster.conf=conf/cluster.conf \
  --dry-run=client -o yaml > deploy/k8s/configmap.yaml
```

Apply to the cluster:

```bash
kubectl apply -f deploy/k8s/configmap.yaml \
              -f deploy/k8s/deployment.yaml \
              -f deploy/k8s/service.yaml
```

Override the container image in `deployment.yaml` (default `easynet/batata:latest`)
with the image built from your release binary.
