# API Routes Reference

This document provides the complete HTTP API routing tables for the two original projects that Batata provides compatibility with: **Nacos** and **Consul**.

## Changelog

| Date | Description |
|------|-------------|
| 2026-03-01 | Re-verified all V3 Admin routes against Nacos source (`~/work/github/easynet-cn/nacos`). Major corrections: Config Controller (removed separate PUT, fixed `/search`→`/list`, added `/metadata`, `/batch`, `/beta`, `/config/listener`, Config Ops section); removed Config Gray Rules section (Nacos uses `/beta` not `/gray/{dataId}`); Listener Controller corrected to single base endpoint (IP query); History Controller added `/previous` and `/configs`, fixed `/detail/{nid}`→base GET with query param; Capacity added POST; Metrics fixed to `/cluster` and `/ip` sub-paths. Naming Admin: Health fixed (removed base GET, added `/checkers`); Client expanded to 7 endpoints; Operator fixed to sub-paths only (`/switches`, `/metrics`, `/log`); Instance added `DELETE /metadata/batch` and `PUT /partial`, fixed POST→PUT for metadata; Cluster removed non-existent GET; Service added `PUT /service/cluster`. Comparison section: ~20 items previously marked EXTRA corrected to OK (they exist in Nacos). |
| 2026-02-28 | Added V3 Admin Listener `/ip` endpoint. Added route completeness integration tests. Fixed comparison section for items that don't exist in Nacos (marked N/A). |
| 2026-02-28 | Initial document created with full route tables for Nacos V2/V3 and Consul APIs. Comparison section added. |

---

## Table of Contents

- [1. Nacos API Routes](#1-nacos-api-routes)
  - [1.1 V2 Open API (Main Server - Port 8848)](#11-v2-open-api-main-server---port-8848)
  - [1.2 V3 Admin API (Main Server - Port 8848)](#12-v3-admin-api-main-server---port-8848)
  - [1.3 V3 Client API (Main Server - Port 8848)](#13-v3-client-api-main-server---port-8848)
  - [1.4 V3 Console API (Console Server)](#14-v3-console-api-console-server)
  - [1.5 V3 Auth API (Console Server)](#15-v3-auth-api-console-server)
- [2. Consul API Routes](#2-consul-api-routes)
  - [2.1 ACL Endpoints](#21-acl-endpoints)
  - [2.2 Agent Endpoints](#22-agent-endpoints)
  - [2.3 Catalog Endpoints](#23-catalog-endpoints)
  - [2.4 Health Endpoints](#24-health-endpoints)
  - [2.5 KV Store Endpoints](#25-kv-store-endpoints)
  - [2.6 Session Endpoints](#26-session-endpoints)
  - [2.7 Event Endpoints](#27-event-endpoints)
  - [2.8 Status Endpoints](#28-status-endpoints)
  - [2.9 Operator Endpoints](#29-operator-endpoints)
  - [2.10 Connect Endpoints](#210-connect-endpoints)
  - [2.11 Config Entry Endpoints](#211-config-entry-endpoints)
  - [2.12 Coordinate Endpoints](#212-coordinate-endpoints)
  - [2.13 Discovery Chain Endpoints](#213-discovery-chain-endpoints)
  - [2.14 Peering Endpoints](#214-peering-endpoints)
  - [2.15 Prepared Query Endpoints](#215-prepared-query-endpoints)
  - [2.16 Snapshot & Transaction Endpoints](#216-snapshot--transaction-endpoints)
  - [2.17 Federation & Internal Endpoints](#217-federation--internal-endpoints)
  - [2.18 UI Internal Endpoints](#218-ui-internal-endpoints)

---

## 1. Nacos API Routes (Reference from Original Project)

> **Source**: All routes verified against `~/work/github/easynet-cn/nacos` source code.

Nacos 3.x uses a split deployment model:
- **Main Server** (default port 8848, contextPath `/nacos`): Serves V2 Open API, V3 Admin API, V3 Client API, and V3 Auth API.
- **Console Server** (default port 8081, no contextPath): Serves V3 Console API, V2 Console API, and V3 Auth API.

> **Note**: Nacos 3.x focuses on V2 and V3 APIs. V1 API (except auth login) is deprecated and NOT supported. Auth routes are registered on **both** servers to support SDK authentication flow (HTTP login on main server before gRPC).

### 1.1 V2 Open API (Main Server - Port 8848)

All routes are prefixed with `/nacos`.

#### Config Module (`/nacos/v2/cs`)

**Config Controller** (`/nacos/v2/cs/config`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v2/cs/config` | Get configuration |
| POST | `/nacos/v2/cs/config` | Publish configuration (add or update) |
| DELETE | `/nacos/v2/cs/config` | Delete configuration |
| GET | `/nacos/v2/cs/config/searchDetail` | Search config by detail/content |

**History Controller** (`/nacos/v2/cs/history`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v2/cs/history` | Query specific history entry |
| GET | `/nacos/v2/cs/history/list` | Query list of config history |
| GET | `/nacos/v2/cs/history/detail` | Query detailed history (original and updated versions) |
| GET | `/nacos/v2/cs/history/previous` | Query previous config history |
| GET | `/nacos/v2/cs/history/configs` | Query configs list by namespace |

#### Naming Module (`/nacos/v2/ns`)

**Instance Controller** (`/nacos/v2/ns/instance`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v2/ns/instance` | Register new instance |
| DELETE | `/nacos/v2/ns/instance` | Deregister instance |
| PUT | `/nacos/v2/ns/instance` | Update instance |
| PATCH | `/nacos/v2/ns/instance` | Patch instance (partial update) |
| GET | `/nacos/v2/ns/instance` | Get instance detail |
| GET | `/nacos/v2/ns/instance/list` | Get all instances of a service |
| PUT | `/nacos/v2/ns/instance/metadata/batch` | Batch update instance metadata |
| DELETE | `/nacos/v2/ns/instance/metadata/batch` | Batch delete instance metadata |
| PUT | `/nacos/v2/ns/instance/beat` | Create heartbeat for instance |
| GET | `/nacos/v2/ns/instance/statuses/{key}` | List all instances with health status |

**Service Controller** (`/nacos/v2/ns/service`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v2/ns/service` | Create a new service |
| DELETE | `/nacos/v2/ns/service` | Remove service |
| PUT | `/nacos/v2/ns/service` | Update service |
| GET | `/nacos/v2/ns/service` | Get service detail |
| GET | `/nacos/v2/ns/service/list` | List all service names |

**Health Controller** (`/nacos/v2/ns/health`)

| Method | Path | Description |
|--------|------|-------------|
| PUT | `/nacos/v2/ns/health` | Update instance health check |
| PUT | `/nacos/v2/ns/health/instance` | Update instance health check (alternative path) |

**Operator Controller** (`/nacos/v2/ns/operator` and `/nacos/v2/ns/ops`)

> **Note**: Nacos registers these endpoints under BOTH `/operator` and `/ops` dual paths.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v2/ns/operator/switches` | Get system switch information |
| PUT | `/nacos/v2/ns/operator/switches` | Update system switch information |
| GET | `/nacos/v2/ns/operator/metrics` | Get naming service metrics |
| GET | `/nacos/v2/ns/ops/switches` | (alias) Get system switch information |
| PUT | `/nacos/v2/ns/ops/switches` | (alias) Update system switch information |
| GET | `/nacos/v2/ns/ops/metrics` | (alias) Get naming service metrics |

**Client Info Controller** (`/nacos/v2/ns/client`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v2/ns/client/list` | Query all clients |
| GET | `/nacos/v2/ns/client` | Query client by clientId |
| GET | `/nacos/v2/ns/client/publish/list` | Query services registered by a client |
| GET | `/nacos/v2/ns/client/subscribe/list` | Query services subscribed by a client |
| GET | `/nacos/v2/ns/client/service/publisher/list` | Query clients that published a service |
| GET | `/nacos/v2/ns/client/service/subscriber/list` | Query clients subscribed to a service |

**Catalog Controller** (`/nacos/v2/ns/catalog`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v2/ns/catalog/instances` | List instances of a specific service |

#### Core Module (`/nacos/v2/core`)

**Cluster Controller** (`/nacos/v2/core/cluster`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v2/core/cluster/node/self` | Get current node information |
| GET | `/nacos/v2/core/cluster/node/list` | List cluster member nodes |
| GET | `/nacos/v2/core/cluster/node/self/health` | Get current node health status |
| PUT | `/nacos/v2/core/cluster/node/list` | Update cluster nodes |
| PUT | `/nacos/v2/core/cluster/lookup` | Update addressing/lookup mode |
| DELETE | `/nacos/v2/core/cluster/nodes` | Remove cluster nodes |

**Ops Controller** (`/nacos/v2/core/ops`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v2/core/ops/raft` | Raft operations (transferLeader, doSnapshot, etc.) |
| GET | `/nacos/v2/core/ops/ids` | Get ID generator health |
| PUT | `/nacos/v2/core/ops/log` | Update log level |

### 1.2 V3 Admin API (Main Server - Port 8848)

All routes are prefixed with `/nacos`. These are administrative APIs running on the main server.

#### Naming Admin (`/nacos/v3/admin/ns`)

**Service Controller** (`/nacos/v3/admin/ns/service`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v3/admin/ns/service` | Create service |
| GET | `/nacos/v3/admin/ns/service` | Get service detail |
| PUT | `/nacos/v3/admin/ns/service` | Update service |
| DELETE | `/nacos/v3/admin/ns/service` | Delete service |
| GET | `/nacos/v3/admin/ns/service/list` | List services |
| GET | `/nacos/v3/admin/ns/service/subscribers` | Get service subscribers |
| GET | `/nacos/v3/admin/ns/service/selector/types` | Get selector types |
| PUT | `/nacos/v3/admin/ns/service/cluster` | Update cluster (via service controller) |

**Instance Controller** (`/nacos/v3/admin/ns/instance`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v3/admin/ns/instance` | Register instance |
| DELETE | `/nacos/v3/admin/ns/instance` | Deregister instance |
| PUT | `/nacos/v3/admin/ns/instance` | Update instance |
| GET | `/nacos/v3/admin/ns/instance` | Get instance detail |
| GET | `/nacos/v3/admin/ns/instance/list` | List instances |
| PUT | `/nacos/v3/admin/ns/instance/metadata/batch` | Batch update instance metadata |
| DELETE | `/nacos/v3/admin/ns/instance/metadata/batch` | Batch delete instance metadata |
| PUT | `/nacos/v3/admin/ns/instance/partial` | Partial update instance (patch) |

**Cluster Controller** (`/nacos/v3/admin/ns/cluster`)

| Method | Path | Description |
|--------|------|-------------|
| PUT | `/nacos/v3/admin/ns/cluster` | Update cluster |

**Health Controller** (`/nacos/v3/admin/ns/health`)

| Method | Path | Description |
|--------|------|-------------|
| PUT | `/nacos/v3/admin/ns/health/instance` | Update instance health status |
| GET | `/nacos/v3/admin/ns/health/checkers` | Get health checkers |

**Client Controller** (`/nacos/v3/admin/ns/client`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/ns/client` | Get client detail |
| GET | `/nacos/v3/admin/ns/client/list` | List all clients |
| GET | `/nacos/v3/admin/ns/client/publish/list` | Get services published by a client |
| GET | `/nacos/v3/admin/ns/client/subscribe/list` | Get services subscribed by a client |
| GET | `/nacos/v3/admin/ns/client/service/publisher/list` | Get clients that publish a service |
| GET | `/nacos/v3/admin/ns/client/service/subscriber/list` | Get clients subscribed to a service |
| GET | `/nacos/v3/admin/ns/client/distro` | Get responsible server for a client |

**Operator Controller** (`/nacos/v3/admin/ns/ops`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/ns/ops/switches` | Get system switches |
| PUT | `/nacos/v3/admin/ns/ops/switches` | Update system switches |
| GET | `/nacos/v3/admin/ns/ops/metrics` | Get naming metrics |
| PUT | `/nacos/v3/admin/ns/ops/log` | Set log level |

#### Config Admin (`/nacos/v3/admin/cs`)

**Config Controller** (`/nacos/v3/admin/cs/config`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/cs/config` | Get configuration detail |
| POST | `/nacos/v3/admin/cs/config` | Publish configuration (create or update) |
| DELETE | `/nacos/v3/admin/cs/config` | Delete configuration |
| PUT | `/nacos/v3/admin/cs/config/metadata` | Update config metadata only |
| DELETE | `/nacos/v3/admin/cs/config/batch` | Batch delete configurations by IDs |
| GET | `/nacos/v3/admin/cs/config/list` | List/search configurations (paginated) |
| GET | `/nacos/v3/admin/cs/config/listener` | Get config listeners (by dataId/group) |
| GET | `/nacos/v3/admin/cs/config/beta` | Query beta configuration |
| DELETE | `/nacos/v3/admin/cs/config/beta` | Remove beta configuration |
| POST | `/nacos/v3/admin/cs/config/import` | Import configurations from ZIP |
| GET | `/nacos/v3/admin/cs/config/export` | Export configurations as ZIP |
| POST | `/nacos/v3/admin/cs/config/clone` | Clone configuration to another namespace |

**Config Listener** (`/nacos/v3/admin/cs/listener`)

> **Note**: This is a separate `ListenerControllerV3` (not the `/config/listener` in ConfigController above).
> The ConfigController's `/config/listener` queries listeners for a specific config (by dataId/group).
> This controller queries all subscribed configs for a specific client IP.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/cs/listener` | Get all subscribed configs by client IP |

**Config History** (`/nacos/v3/admin/cs/history`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/cs/history` | Get history detail (by `nid` query param) |
| GET | `/nacos/v3/admin/cs/history/list` | List config history |
| GET | `/nacos/v3/admin/cs/history/previous` | Get previous config history (by `id` query param) |
| GET | `/nacos/v3/admin/cs/history/configs` | List configs by namespace |

**Config Capacity** (`/nacos/v3/admin/cs/capacity`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/cs/capacity` | Get capacity info |
| POST | `/nacos/v3/admin/cs/capacity` | Create/update capacity settings |

**Config Metrics** (`/nacos/v3/admin/cs/metrics`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/cs/metrics/cluster` | Get cluster-wide config metrics |
| GET | `/nacos/v3/admin/cs/metrics/ip` | Get client config metrics by IP |

**Config Ops** (`/nacos/v3/admin/cs/ops`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v3/admin/cs/ops/localCache` | Dump local cache from store |
| PUT | `/nacos/v3/admin/cs/ops/log` | Set log level |
| GET | `/nacos/v3/admin/cs/ops/derby` | Derby operations |
| POST | `/nacos/v3/admin/cs/ops/derby/import` | Import derby data |

#### Core Admin (`/nacos/v3/admin/core`)

**Cluster Controller** (`/nacos/v3/admin/core/cluster`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/core/cluster/node/self` | Get current node info |
| GET | `/nacos/v3/admin/core/cluster/node/list` | List cluster nodes |
| PUT | `/nacos/v3/admin/core/cluster/node/list` | Update cluster node metadata |
| PUT | `/nacos/v3/admin/core/cluster/lookup` | Update addressing/lookup mode |

**Namespace Controller** (`/nacos/v3/admin/core/namespace`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/core/namespace/list` | List namespaces |
| GET | `/nacos/v3/admin/core/namespace` | Get namespace detail |
| POST | `/nacos/v3/admin/core/namespace` | Create namespace |
| PUT | `/nacos/v3/admin/core/namespace` | Update namespace |
| DELETE | `/nacos/v3/admin/core/namespace` | Delete namespace |
| GET | `/nacos/v3/admin/core/namespace/check` | Check if namespace ID exists |

**State Controller** (`/nacos/v3/admin/core/state`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/core/state` | Get server state |
| GET | `/nacos/v3/admin/core/state/liveness` | Server liveness check |
| GET | `/nacos/v3/admin/core/state/readiness` | Server readiness check |

**Ops Controller** (`/nacos/v3/admin/core/ops`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v3/admin/core/ops/raft` | Raft operations |
| GET | `/nacos/v3/admin/core/ops/ids` | Get ID generator info |
| PUT | `/nacos/v3/admin/core/ops/log` | Set log level |

**Server Loader Controller** (`/nacos/v3/admin/core/loader`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/core/loader/current` | Get current SDK clients |
| POST | `/nacos/v3/admin/core/loader/reloadCurrent` | Rebalance SDK connections on current server |
| POST | `/nacos/v3/admin/core/loader/smartReloadCluster` | Smart reload cluster connections |
| POST | `/nacos/v3/admin/core/loader/reloadClient` | Reload specific SDK client |
| GET | `/nacos/v3/admin/core/loader/cluster` | Get server loader metrics |

#### AI Admin (`/nacos/v3/admin/ai`)

**MCP Controller** (`/nacos/v3/admin/ai/mcp`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/ai/mcp/list` | List MCP servers |
| GET | `/nacos/v3/admin/ai/mcp` | Get MCP server detail |
| POST | `/nacos/v3/admin/ai/mcp` | Create MCP server |
| PUT | `/nacos/v3/admin/ai/mcp` | Update MCP server |
| DELETE | `/nacos/v3/admin/ai/mcp` | Delete MCP server |
| POST | `/nacos/v3/admin/ai/mcp/import/validate` | Validate MCP import |
| POST | `/nacos/v3/admin/ai/mcp/import/execute` | Execute MCP import |
| GET | `/nacos/v3/admin/ai/mcp/tools` | Get MCP tools |

**A2A Controller** (`/nacos/v3/admin/ai/a2a`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v3/admin/ai/a2a` | Register agent |
| GET | `/nacos/v3/admin/ai/a2a` | Get agent card |
| PUT | `/nacos/v3/admin/ai/a2a` | Update agent |
| DELETE | `/nacos/v3/admin/ai/a2a` | Delete agent |
| GET | `/nacos/v3/admin/ai/a2a/list` | List agents |
| GET | `/nacos/v3/admin/ai/a2a/version/list` | List agent versions |

### 1.3 V3 Client API (Main Server - Port 8848)

All routes are prefixed with `/nacos`. These are SDK client-facing HTTP APIs for non-gRPC clients.

#### Config Client (`/nacos/v3/client/cs`)

**Config Open API Controller** (`/nacos/v3/client/cs/config`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/client/cs/config` | Get configuration |

#### Naming Client (`/nacos/v3/client/ns`)

**Instance Open API Controller** (`/nacos/v3/client/ns/instance`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v3/client/ns/instance` | Register instance or heartbeat |
| DELETE | `/nacos/v3/client/ns/instance` | Deregister instance |
| GET | `/nacos/v3/client/ns/instance/list` | Get instance list |

### 1.4 V3 Console API (Console Server)

All routes are under the console server (default port 8081, no `/nacos` prefix).

#### Health Controller (`/v3/console/health`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v3/console/health/liveness` | Check if Nacos is in broken state |
| GET | `/v3/console/health/readiness` | Check if Nacos is ready to receive requests |

#### Server State Controller (`/v3/console/server`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v3/console/server/state` | Get server state |
| GET | `/v3/console/server/announcement` | Get announcement content by language |
| GET | `/v3/console/server/guide` | Get console UI guide information |

#### Config Controller (`/v3/console/cs/config`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v3/console/cs/config` | Get specific configuration |
| POST | `/v3/console/cs/config` | Add or update configuration |
| DELETE | `/v3/console/cs/config` | Delete configuration |
| DELETE | `/v3/console/cs/config/batchDelete` | Batch delete configurations |
| GET | `/v3/console/cs/config/list` | Get configuration list |
| GET | `/v3/console/cs/config/searchDetail` | Search config by detail/content |
| GET | `/v3/console/cs/config/listener` | Get subscribed client information |
| GET | `/v3/console/cs/config/listener/ip` | Get subscribe info from client by IP |
| GET | `/v3/console/cs/config/export2` | Export configuration with metadata |
| POST | `/v3/console/cs/config/import` | Import and publish configuration |
| POST | `/v3/console/cs/config/clone` | Clone configuration |
| DELETE | `/v3/console/cs/config/beta` | Remove beta configuration |
| GET | `/v3/console/cs/config/beta` | Query beta configuration |

#### History Controller (`/v3/console/cs/history`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v3/console/cs/history` | Query detailed config history |
| GET | `/v3/console/cs/history/list` | Query list of config history |
| GET | `/v3/console/cs/history/previous` | Query previous config history |
| GET | `/v3/console/cs/history/configs` | Query configs list by namespace |

#### Service Controller (`/v3/console/ns/service`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v3/console/ns/service` | Create a new service |
| DELETE | `/v3/console/ns/service` | Remove service |
| PUT | `/v3/console/ns/service` | Update service |
| GET | `/v3/console/ns/service` | Get service detail |
| GET | `/v3/console/ns/service/list` | List service detail information |
| GET | `/v3/console/ns/service/selector/types` | Get all selector types |
| GET | `/v3/console/ns/service/subscribers` | Get subscriber list |
| PUT | `/v3/console/ns/service/cluster` | Update cluster |

#### Instance Controller (`/v3/console/ns/instance`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v3/console/ns/instance/list` | List instances of a service |
| PUT | `/v3/console/ns/instance` | Update instance |

#### Cluster Controller (`/v3/console/core/cluster`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v3/console/core/cluster/nodes` | List cluster member nodes |

#### Namespace Controller (`/v3/console/core/namespace`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v3/console/core/namespace/list` | Get namespace list |
| GET | `/v3/console/core/namespace` | Get namespace detail by ID |
| POST | `/v3/console/core/namespace` | Create namespace |
| PUT | `/v3/console/core/namespace` | Edit namespace |
| DELETE | `/v3/console/core/namespace` | Delete namespace by ID |
| GET | `/v3/console/core/namespace/exist` | Check if namespace ID exists |

#### MCP Controller (`/v3/console/ai/mcp`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v3/console/ai/mcp/list` | List MCP servers |
| GET | `/v3/console/ai/mcp` | Get MCP server detail |
| POST | `/v3/console/ai/mcp` | Create new MCP server |
| PUT | `/v3/console/ai/mcp` | Update existing MCP server |
| DELETE | `/v3/console/ai/mcp` | Delete MCP server |
| GET | `/v3/console/ai/mcp/importToolsFromMcp` | Import tools from MCP result |
| POST | `/v3/console/ai/mcp/import/validate` | Validate MCP server import request |
| POST | `/v3/console/ai/mcp/import/execute` | Execute MCP server import operation |

#### A2A Controller (`/v3/console/ai/a2a`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v3/console/ai/a2a` | Register agent |
| GET | `/v3/console/ai/a2a` | Get agent card |
| PUT | `/v3/console/ai/a2a` | Update agent |
| DELETE | `/v3/console/ai/a2a` | Delete agent |
| GET | `/v3/console/ai/a2a/list` | List agents |
| GET | `/v3/console/ai/a2a/version/list` | List all versions for target agent |

#### V2 Console Routes (Console Server)

**Namespace Controller** (`/v2/console/namespace`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v2/console/namespace/list` | Get namespace list |
| GET | `/v2/console/namespace` | Get namespace by ID |
| POST | `/v2/console/namespace` | Create namespace |
| PUT | `/v2/console/namespace` | Edit namespace |
| DELETE | `/v2/console/namespace` | Delete namespace by ID |

**Health Controller** (`/v2/console/health`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v2/console/health/liveness` | Check if Nacos is in broken state |
| GET | `/v2/console/health/readiness` | Check if Nacos is ready to receive requests |

### 1.5 V3 Auth API (Both Servers)

Auth routes are registered on **both** Main Server (under `/nacos/v3/auth`) and Console Server (under `/v3/auth`).

#### User Controller (`/v3/auth/user`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v3/auth/user` | Create a new user |
| POST | `/v3/auth/user/admin` | Create admin user (only when no admin exists) |
| DELETE | `/v3/auth/user` | Delete an existing user |
| PUT | `/v3/auth/user` | Update user password |
| GET | `/v3/auth/user/list` | Get paged users (accurate or fuzzy search) |
| GET | `/v3/auth/user/search` | Fuzzy matching username |
| POST | `/v3/auth/user/login` | Login to Nacos |

#### Role Controller (`/v3/auth/role`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v3/auth/role` | Create role or add role to user |
| DELETE | `/v3/auth/role` | Delete a role |
| GET | `/v3/auth/role/list` | Get roles list (accurate or fuzzy search) |
| GET | `/v3/auth/role/search` | Fuzzy matching role name |

#### Permission Controller (`/v3/auth/permission`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v3/auth/permission` | Add permission to a role |
| DELETE | `/v3/auth/permission` | Delete permission from a role |
| GET | `/v3/auth/permission/list` | Query permissions of a role |
| GET | `/v3/auth/permission` | Check if a permission is duplicate |

### 1.6 Batata vs Nacos Route Comparison

This section documents all differences between the original Nacos routes and Batata's implementation.

#### Legend

- **OK** = Fully implemented with matching path
- **PATH** = Route exists but path differs from Nacos
- **MISSING** = Not implemented in Batata
- **EXTRA** = Batata-only extension (not in original Nacos)

#### V2 Open API Differences

| Nacos Route | Batata Status | Notes |
|-------------|---------------|-------|
| `GET /v2/cs/history/detail` | **OK** | Implemented |
| `PUT /v2/ns/health` | **OK** | Both base path and `/instance` map to the same handler |
| `GET /v2/ns/ops/switches` | **OK** | Dual-path alias for `/operator/switches` |
| `PUT /v2/ns/ops/switches` | **OK** | Dual-path alias for `/operator/switches` |
| `GET /v2/ns/ops/metrics` | **OK** | Dual-path alias for `/operator/metrics` |
| `POST /v2/core/ops/raft` | **OK** | V2 core ops implemented |
| `GET /v2/core/ops/ids` | **OK** | V2 core ops implemented |
| `PUT /v2/core/ops/log` | **OK** | V2 core ops implemented |
| `GET /v2/console/health/liveness` | **OK** | V2 console health implemented |
| `GET /v2/console/health/readiness` | **OK** | V2 console health implemented |
| `POST /v2/cs/config/listener` | **EXTRA** | Batata has this; not in Nacos V2 config controller |
| `GET /v2/cs/capacity` | **EXTRA** | Batata has capacity management in V2; Nacos only in V3 admin |
| `POST /v2/cs/capacity` | **EXTRA** | Same |
| `DELETE /v2/cs/capacity` | **EXTRA** | Same |

#### V3 Admin API Differences

**Config Admin:**

| Nacos Route | Batata Status | Notes |
|-------------|---------------|-------|
| `GET /v3/admin/cs/config/list` | **OK** | List/search configs (paginated) |
| `POST /v3/admin/cs/config/clone` | **OK** | Clone config (both Nacos and Batata use POST) |
| `PUT /v3/admin/cs/config/metadata` | **OK** | Update config metadata |
| `DELETE /v3/admin/cs/config/batch` | **PATH** | Nacos uses `/batch`; Batata may use `/batchDelete` |
| `GET /v3/admin/cs/config/beta` | **OK** | Query beta config |
| `DELETE /v3/admin/cs/config/beta` | **OK** | Remove beta config |
| `GET /v3/admin/cs/config/listener` | **OK** | Get config listeners by dataId/group |
| `GET /v3/admin/cs/listener` | **PATH** | Nacos ListenerControllerV3 queries by IP at base path; Batata uses `/listener/ip` sub-path |
| `GET /v3/admin/cs/history` | **OK** | Get history detail by `nid` query param |
| `GET /v3/admin/cs/history/previous` | **OK** | Get previous config history |
| `GET /v3/admin/cs/history/configs` | **OK** | List configs by namespace |
| `GET /v3/admin/cs/capacity` | **OK** | Get capacity info |
| `POST /v3/admin/cs/capacity` | **OK** | Update capacity settings |
| `GET /v3/admin/cs/metrics/cluster` | **OK** | Cluster-wide config metrics |
| `GET /v3/admin/cs/metrics/ip` | **OK** | Client config metrics by IP |
| `POST /v3/admin/cs/ops/localCache` | **OK** | Dump local cache from store |
| `PUT /v3/admin/cs/ops/log` | **OK** | Set log level |
| `GET /v3/admin/cs/ops/derby` | **OK** | Derby operations |
| `POST /v3/admin/cs/ops/derby/import` | **OK** | Import derby data |

**Naming Admin:**

| Nacos Route | Batata Status | Notes |
|-------------|---------------|-------|
| `PUT /v3/admin/ns/instance/metadata/batch` | **PATH** | Batata uses `PUT /v3/admin/ns/instance/metadata` (without `/batch` suffix) |
| `DELETE /v3/admin/ns/instance/metadata/batch` | **OK** | Batch delete metadata |
| `PUT /v3/admin/ns/instance/partial` | **OK** | Partial instance update (patch) |
| `PUT /v3/admin/ns/health/instance` | **OK** | Update instance health status |
| `GET /v3/admin/ns/health/checkers` | **OK** | Get health checkers list |
| `GET /v3/admin/ns/service/subscribers` | **OK** | Implemented |
| `GET /v3/admin/ns/service/selector/types` | **OK** | Implemented |
| `PUT /v3/admin/ns/service/cluster` | **OK** | Update cluster via service controller |
| `GET /v3/admin/ns/client/list` | **OK** | List all clients |
| `GET /v3/admin/ns/client/publish/list` | **OK** | Client published services |
| `GET /v3/admin/ns/client/subscribe/list` | **OK** | Client subscribed services |
| `GET /v3/admin/ns/client/service/publisher/list` | **OK** | Service publishers |
| `GET /v3/admin/ns/client/service/subscriber/list` | **OK** | Service subscribers |
| `GET /v3/admin/ns/client/distro` | **OK** | Get responsible server for client |
| `GET /v3/admin/ns/ops/switches` | **OK** | Get system switches |
| `PUT /v3/admin/ns/ops/switches` | **OK** | Update system switches |
| `GET /v3/admin/ns/ops/metrics` | **OK** | Get naming metrics |
| `PUT /v3/admin/ns/ops/log` | **OK** | Set log level |
| `PUT /v3/admin/ns/service/cluster` | **OK** | Update cluster via service controller |
| `GET /v3/admin/ns/cluster` | **EXTRA** | Batata has GET cluster detail; Nacos only has PUT |
| `POST /v3/admin/ns/cluster` | **EXTRA** | Create cluster |
| `GET /v3/admin/ns/cluster/statistics` | **EXTRA** | Cluster statistics |

**Core Admin:**

| Nacos Route | Batata Status | Notes |
|-------------|---------------|-------|
| `PUT /v3/admin/core/cluster/node/list` | **OK** | Implemented |
| `GET /v3/admin/core/namespace/check` | **OK** | Implemented |
| `GET /v3/admin/core/state` | **OK** | Implemented |
| `GET /v3/admin/core/state/liveness` | **OK** | Implemented |
| `GET /v3/admin/core/state/readiness` | **OK** | Implemented |
| `GET /v3/admin/core/cluster/node/self/health` | **EXTRA** | Not in Nacos V3 admin (only in V2 core) |
| `GET /v3/admin/core/loader/current` | **OK** | Implemented |
| `POST /v3/admin/core/loader/reloadCurrent` | **OK** | Implemented |
| `POST /v3/admin/core/loader/smartReloadCluster` | **OK** | Implemented |
| `POST /v3/admin/core/loader/reloadClient` | **OK** | Implemented |
| `GET /v3/admin/core/loader/cluster` | **OK** | Implemented |

**AI Admin:**

| Nacos Route | Batata Status | Notes |
|-------------|---------------|-------|
| `POST /v3/admin/ai/mcp/import/validate` | **OK** | Implemented |
| `POST /v3/admin/ai/mcp/import/execute` | **OK** | Implemented |
| `GET /v3/admin/ai/mcp/tools` | N/A | Does not exist in Nacos; tools fetched via `/v3/console/ai/mcp/importToolsFromMcp` |
| `GET /v3/admin/ai/a2a/version/list` | **OK** | Implemented |

#### V3 Console API Differences

| Nacos Route | Batata Status | Notes |
|-------------|---------------|-------|
| `GET /v3/console/cs/config/export2` | **PATH** | Batata uses `/export` instead of `/export2` |
| `POST /v3/console/cs/config/beta` | **EXTRA** | Batata has publish beta; Nacos console only has GET and DELETE for beta |
| `GET /v3/console/cs/history/diff` | **EXTRA** | History diff |
| `POST /v3/console/cs/history/rollback` | **EXTRA** | History rollback |
| `GET /v3/console/cs/history/search` | **EXTRA** | Advanced history search |
| `GET /v3/console/health` | **EXTRA** | Base health endpoint (Nacos only has liveness/readiness) |
| `GET /v3/console/metrics` | **EXTRA** | Prometheus metrics |
| Various cluster endpoints | **EXTRA** | Batata has 9 extra cluster endpoints (healthy nodes, self, count, leader, etc.) |

#### V3 Auth API Differences

| Nacos Route | Batata Status | Notes |
|-------------|---------------|-------|
| All V3 auth routes | **OK** | Fully implemented |
| `GET /v3/auth/oauth/providers` | **EXTRA** | OAuth2/OIDC support (Batata extension) |
| `GET /v3/auth/oauth/login/{provider}` | **EXTRA** | OAuth2 login initiation |
| `GET /v3/auth/oauth/callback/{provider}` | **EXTRA** | OAuth2 callback |

#### Batata-Only Extensions (Not in Nacos)

These are additional features provided by Batata:

| Category | Routes | Description |
|----------|--------|-------------|
| AI Registry API | `/v1/ai/mcp/*` | MCP server registry (no `/nacos` prefix) |
| AI Registry API | `/v1/ai/a2a/*` | A2A agent registry (no `/nacos` prefix) |
| Cloud Integration | `/v1/cloud/prometheus/*` | Prometheus service discovery |
| Cloud Integration | `/v1/cloud/k8s/*` | Kubernetes integration |
| MCP Registry | `/v0/servers` (port 9080) | MCP Registry OpenAPI spec |
| Consul Compat | `/v1/*` (port 8500) | Full Consul API compatibility |
| Console Plugin | `/v3/console/core/plugin/*` | Plugin management |
| OAuth2 Auth | `/v3/auth/oauth/*` | OAuth2/OIDC authentication |

#### Summary of Differences

The following are **intentional design differences** between Nacos and Batata (not missing features):

1. **`GET /v3/console/cs/config/export2`**: Nacos uses `export2` but Batata uses `export`.
2. **Admin Listener path**: Nacos has a separate `ListenerControllerV3` at `/v3/admin/cs/listener` (base) for IP-based query; Batata puts this under `/v3/admin/cs/listener/ip`.
3. **Admin Config batch delete**: Nacos uses `DELETE /v3/admin/cs/config/batch`; Batata may use a slightly different path.
4. **No separate PUT for config**: Nacos uses `POST` for both create and update (publish); there is no separate `PUT /v3/admin/cs/config`.
5. **No gray/{dataId} endpoints**: Nacos V3 admin uses `GET /beta` and `DELETE /beta` (not `gray/{dataId}` path params).

---

## 2. Consul API Routes

All Consul API routes are prefixed with `/v1`. Routes are registered in `agent/http_register.go`.

### 2.1 ACL Endpoints

| Method | Path | Description |
|--------|------|-------------|
| PUT | `/v1/acl/bootstrap` | Bootstrap ACL system |
| POST | `/v1/acl/login` | Login with auth method |
| POST | `/v1/acl/logout` | Logout (destroy token from auth method) |
| GET | `/v1/acl/replication` | Get ACL replication status |
| POST | `/v1/acl/authorize` | Authorize ACL request (internal) |

**Policies**

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/acl/policies` | List all ACL policies |
| PUT | `/v1/acl/policy` | Create ACL policy |
| GET | `/v1/acl/policy/:id` | Read ACL policy by ID |
| PUT | `/v1/acl/policy/:id` | Update ACL policy |
| DELETE | `/v1/acl/policy/:id` | Delete ACL policy |
| GET | `/v1/acl/policy/name/:name` | Read ACL policy by name |

**Roles**

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/acl/roles` | List all ACL roles |
| PUT | `/v1/acl/role` | Create ACL role |
| GET | `/v1/acl/role/:id` | Read ACL role by ID |
| PUT | `/v1/acl/role/:id` | Update ACL role |
| DELETE | `/v1/acl/role/:id` | Delete ACL role |
| GET | `/v1/acl/role/name/:name` | Read ACL role by name |

**Tokens**

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/acl/tokens` | List all ACL tokens |
| PUT | `/v1/acl/token` | Create ACL token |
| GET | `/v1/acl/token/self` | Read own ACL token |
| GET | `/v1/acl/token/:accessor_id` | Read ACL token |
| PUT | `/v1/acl/token/:accessor_id` | Update ACL token |
| DELETE | `/v1/acl/token/:accessor_id` | Delete ACL token |

**Binding Rules**

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/acl/binding-rules` | List all binding rules |
| PUT | `/v1/acl/binding-rule` | Create binding rule |
| GET | `/v1/acl/binding-rule/:id` | Read binding rule |
| PUT | `/v1/acl/binding-rule/:id` | Update binding rule |
| DELETE | `/v1/acl/binding-rule/:id` | Delete binding rule |

**Auth Methods**

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/acl/auth-methods` | List all auth methods |
| PUT | `/v1/acl/auth-method` | Create auth method |
| GET | `/v1/acl/auth-method/:name` | Read auth method |
| PUT | `/v1/acl/auth-method/:name` | Update auth method |
| DELETE | `/v1/acl/auth-method/:name` | Delete auth method |

**Templated Policies**

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/acl/templated-policies` | List all templated policies |
| GET | `/v1/acl/templated-policy/name/:name` | Read templated policy by name |
| POST | `/v1/acl/templated-policy/preview/:name` | Preview templated policy |

### 2.2 Agent Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/agent/self` | Get local agent configuration and member info |
| GET | `/v1/agent/host` | Get host info |
| GET | `/v1/agent/version` | Get agent version |
| GET | `/v1/agent/members` | List cluster members known to agent |
| GET | `/v1/agent/metrics` | Get agent metrics |
| GET | `/v1/agent/metrics/stream` | Stream agent metrics |
| GET | `/v1/agent/monitor` | Stream agent logs |
| PUT | `/v1/agent/reload` | Reload agent configuration |
| PUT | `/v1/agent/maintenance` | Toggle node maintenance mode |
| PUT | `/v1/agent/join/:address` | Join a cluster |
| PUT | `/v1/agent/leave` | Gracefully leave cluster |
| PUT | `/v1/agent/force-leave/:node` | Force remove a node |
| PUT | `/v1/agent/token/:type` | Update agent ACL token |

**Services**

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/agent/services` | List all registered services |
| GET | `/v1/agent/service/:service_id` | Get service configuration |
| PUT | `/v1/agent/service/register` | Register a new service |
| PUT | `/v1/agent/service/deregister/:service_id` | Deregister a service |
| PUT | `/v1/agent/service/maintenance/:service_id` | Toggle service maintenance mode |
| GET | `/v1/agent/health/service/id/:service_id` | Get service health by ID |
| GET | `/v1/agent/health/service/name/:service_name` | Get service health by name |

**Checks**

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/agent/checks` | List all registered checks |
| PUT | `/v1/agent/check/register` | Register a new check |
| PUT | `/v1/agent/check/deregister/:check_id` | Deregister a check |
| PUT | `/v1/agent/check/pass/:check_id` | Mark check as passing |
| PUT | `/v1/agent/check/warn/:check_id` | Mark check as warning |
| PUT | `/v1/agent/check/fail/:check_id` | Mark check as critical |
| PUT | `/v1/agent/check/update/:check_id` | Update check status with output |

**Connect**

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/agent/connect/authorize` | Authorize a Connect connection |
| GET | `/v1/agent/connect/ca/roots` | Get Connect CA roots |
| GET | `/v1/agent/connect/ca/leaf/:service` | Get Connect leaf certificate |

### 2.3 Catalog Endpoints

| Method | Path | Description |
|--------|------|-------------|
| PUT | `/v1/catalog/register` | Register node, service, or check |
| PUT | `/v1/catalog/deregister` | Deregister node, service, or check |
| GET | `/v1/catalog/datacenters` | List known datacenters |
| GET | `/v1/catalog/nodes` | List nodes in a datacenter |
| GET | `/v1/catalog/services` | List all services in a datacenter |
| GET | `/v1/catalog/service/:service` | List nodes providing a service |
| GET | `/v1/catalog/connect/:service` | List Connect-capable service instances |
| GET | `/v1/catalog/node/:node` | List services registered on a node |
| GET | `/v1/catalog/node-services/:node` | List all services for a node (detailed) |
| GET | `/v1/catalog/gateway-services/:gateway` | List services associated with a gateway |

### 2.4 Health Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/health/node/:node` | Get health checks for a node |
| GET | `/v1/health/checks/:service` | Get health checks for a service |
| GET | `/v1/health/state/:state` | List checks in a given state |
| GET | `/v1/health/service/:service` | Get healthy service instances |
| GET | `/v1/health/connect/:service` | Get healthy Connect proxy instances |
| GET | `/v1/health/ingress/:service` | Get healthy ingress gateway instances |

### 2.5 KV Store Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/kv/:key` | Read a key (supports `?recurse`, `?keys`, `?raw`) |
| PUT | `/v1/kv/:key` | Create/update a key (supports CAS via `?cas`) |
| DELETE | `/v1/kv/:key` | Delete a key (supports `?recurse`, `?cas`) |

### 2.6 Session Endpoints

| Method | Path | Description |
|--------|------|-------------|
| PUT | `/v1/session/create` | Create a new session |
| PUT | `/v1/session/destroy/:session` | Destroy a session |
| PUT | `/v1/session/renew/:session` | Renew a session TTL |
| GET | `/v1/session/info/:session` | Read a session |
| GET | `/v1/session/node/:node` | List sessions for a node |
| GET | `/v1/session/list` | List all active sessions |

### 2.7 Event Endpoints

| Method | Path | Description |
|--------|------|-------------|
| PUT | `/v1/event/fire/:name` | Fire a custom user event |
| GET | `/v1/event/list` | List recent events |

### 2.8 Status Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/status/leader` | Get Raft leader address |
| GET | `/v1/status/peers` | Get Raft peer addresses |

### 2.9 Operator Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/operator/raft/configuration` | Get Raft configuration |
| DELETE | `/v1/operator/raft/peer` | Remove Raft peer |
| POST | `/v1/operator/raft/transfer-leader` | Transfer Raft leadership |
| GET | `/v1/operator/autopilot/configuration` | Get Autopilot configuration |
| PUT | `/v1/operator/autopilot/configuration` | Update Autopilot configuration |
| GET | `/v1/operator/autopilot/health` | Get Autopilot server health |
| GET | `/v1/operator/autopilot/state` | Get Autopilot state |
| GET | `/v1/operator/keyring` | List gossip encryption keys |
| POST | `/v1/operator/keyring` | Install gossip encryption key |
| PUT | `/v1/operator/keyring` | Change primary gossip encryption key |
| DELETE | `/v1/operator/keyring` | Remove gossip encryption key |
| GET | `/v1/operator/usage` | Get operator usage stats |
| GET | `/v1/operator/utilization` | Get operator utilization stats |

### 2.10 Connect Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/connect/ca/roots` | Get Connect CA root certificates |
| GET | `/v1/connect/ca/configuration` | Get Connect CA configuration |
| PUT | `/v1/connect/ca/configuration` | Update Connect CA configuration |
| GET | `/v1/connect/intentions` | List all intentions |
| POST | `/v1/connect/intentions` | Create intention |
| GET | `/v1/connect/intentions/match` | Match intentions by source/destination |
| GET | `/v1/connect/intentions/check` | Check if connection is authorized |
| GET | `/v1/connect/intentions/exact` | Get exact intention by source/destination |
| PUT | `/v1/connect/intentions/exact` | Update exact intention |
| DELETE | `/v1/connect/intentions/exact` | Delete exact intention |
| GET | `/v1/connect/intentions/:id` | Read intention by ID |
| PUT | `/v1/connect/intentions/:id` | Update intention by ID |
| DELETE | `/v1/connect/intentions/:id` | Delete intention by ID |

### 2.11 Config Entry Endpoints

| Method | Path | Description |
|--------|------|-------------|
| PUT | `/v1/config` | Create or update a config entry |
| GET | `/v1/config/:kind` | List config entries by kind |
| GET | `/v1/config/:kind/:name` | Get specific config entry |
| DELETE | `/v1/config/:kind/:name` | Delete a config entry |

### 2.12 Coordinate Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/coordinate/datacenters` | List WAN coordinates of all datacenters |
| GET | `/v1/coordinate/nodes` | List LAN coordinates of all nodes |
| GET | `/v1/coordinate/node/:node` | Get coordinate of a specific node |
| PUT | `/v1/coordinate/update` | Update node coordinate |

### 2.13 Discovery Chain Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/discovery-chain/:service` | Get compiled discovery chain for a service |
| POST | `/v1/discovery-chain/:service` | Get compiled discovery chain with overrides |

### 2.14 Peering Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/peering/token` | Generate peering token |
| POST | `/v1/peering/establish` | Establish peering connection |
| GET | `/v1/peering/:name` | Read peering by name |
| DELETE | `/v1/peering/:name` | Delete peering by name |
| GET | `/v1/peerings` | List all peerings |

### 2.15 Prepared Query Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/query` | List all prepared queries |
| POST | `/v1/query` | Create a prepared query |
| GET | `/v1/query/:id` | Read a prepared query |
| PUT | `/v1/query/:id` | Update a prepared query |
| DELETE | `/v1/query/:id` | Delete a prepared query |
| GET | `/v1/query/:id/execute` | Execute a prepared query |
| GET | `/v1/query/:id/explain` | Explain a prepared query |

### 2.16 Snapshot & Transaction Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/snapshot` | Generate and download a snapshot |
| PUT | `/v1/snapshot` | Restore from a snapshot |
| PUT | `/v1/txn` | Execute a transaction |

### 2.17 Federation & Internal Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/internal/federation-states` | List federation states |
| GET | `/v1/internal/federation-states/mesh-gateways` | List mesh gateways by federation state |
| GET | `/v1/internal/federation-state/:dc` | Get federation state for a datacenter |
| PUT | `/v1/internal/service-virtual-ip` | Assign manual service VIPs |
| GET | `/v1/exported-services` | List exported services |
| GET | `/v1/imported-services` | List imported services |

### 2.18 UI Internal Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/internal/ui/nodes` | List nodes for UI |
| GET | `/v1/internal/ui/node/:node` | Get node info for UI |
| GET | `/v1/internal/ui/services` | List services for UI |
| GET | `/v1/internal/ui/exported-services` | List exported services for UI |
| GET | `/v1/internal/ui/catalog-overview` | Get catalog overview for UI |
| GET | `/v1/internal/ui/gateway-services-nodes/:gateway` | List gateway service nodes for UI |
| GET | `/v1/internal/ui/gateway-intentions/:gateway` | List gateway intentions for UI |
| GET | `/v1/internal/ui/service-topology/:service` | Get service topology for UI |
| GET | `/v1/internal/ui/metrics-proxy/*` | Proxy metrics requests for UI |

---

## Summary

| Project | Module | Server | Endpoint Count |
|---------|--------|--------|---------------|
| **Nacos** | V2 Config | Main (8848) | 9 |
| **Nacos** | V2 Naming | Main (8848) | 24 |
| **Nacos** | V2 Core | Main (8848) | 9 |
| **Nacos** | V3 Admin Naming | Main (8848) | 22 |
| **Nacos** | V3 Admin Config | Main (8848) | 15 |
| **Nacos** | V3 Admin Core | Main (8848) | 17 |
| **Nacos** | V3 Admin AI | Main (8848) | 10 |
| **Nacos** | V3 Client | Main (8848) | 4 |
| **Nacos** | V2 Console | Console (8081) | 7 |
| **Nacos** | V3 Console | Console (8081) | 49 |
| **Nacos** | V3 Auth | Console (8081) | 16 |
| **Consul** | All API Routes | - | 138 |
