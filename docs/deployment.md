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
