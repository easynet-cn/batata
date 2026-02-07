# API Routes Reference

This document provides the complete HTTP API routing tables for the three original projects that Batata provides compatibility with: **Nacos**, **Consul**, and **Apollo Config**.

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
- [3. Apollo Config API Routes](#3-apollo-config-api-routes)
  - [3.1 Config Service API](#31-config-service-api)
  - [3.2 Admin Service API](#32-admin-service-api)
  - [3.3 Portal Open API v1](#33-portal-open-api-v1)
  - [3.4 Portal Internal API](#34-portal-internal-api)

---

## 1. Nacos API Routes

Nacos 3.x uses a split deployment model:
- **Main Server** (default port 8848, contextPath `/nacos`): Serves V2 Open API, V3 Admin API, and V3 Client API.
- **Console Server** (default port 8081, no contextPath): Serves V3 Console API, V3/V1 Auth API for admin management.

The `NacosServerWebApplication` explicitly excludes `com.alibaba.nacos.console.*` and `com.alibaba.nacos.plugin.auth.impl.*` packages, so console and auth controllers do NOT run on the main server.

> **Note**: Nacos 3.x direction focuses on V2 and V3 APIs. V1 API (except auth) is deprecated and NOT supported.

### 1.1 V2 Open API (Main Server - Port 8848)

All routes are prefixed with `/nacos`.

#### Config Module (`/nacos/v2/cs`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v2/cs/config` | Get configuration |
| POST | `/nacos/v2/cs/config` | Publish configuration (add or update) |
| DELETE | `/nacos/v2/cs/config` | Delete configuration |
| GET | `/nacos/v2/cs/config/searchDetail` | Search config by detail/content |
| GET | `/nacos/v2/cs/history/list` | Query list of config history |
| GET | `/nacos/v2/cs/history` | Query specific history entry |
| GET | `/nacos/v2/cs/history/detail` | Query detailed history with original and updated versions |
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

**Operator Controller** (`/nacos/v2/ns/operator`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v2/ns/operator/switches` | Get system switch information |
| PUT | `/nacos/v2/ns/operator/switches` | Update system switch information |
| GET | `/nacos/v2/ns/operator/metrics` | Get naming service metrics |

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

**Instance Controller** (`/nacos/v3/admin/ns/instance`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v3/admin/ns/instance` | Register instance |
| DELETE | `/nacos/v3/admin/ns/instance` | Deregister instance |
| PUT | `/nacos/v3/admin/ns/instance` | Update instance |
| GET | `/nacos/v3/admin/ns/instance` | Get instance detail |
| GET | `/nacos/v3/admin/ns/instance/list` | List instances |
| PUT | `/nacos/v3/admin/ns/instance/metadata` | Update instance metadata |

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
| GET | `/nacos/v3/admin/ns/client/list` | Get client list |
| GET | `/nacos/v3/admin/ns/client` | Get client detail |
| GET | `/nacos/v3/admin/ns/client/publish/list` | Get published services by client |
| GET | `/nacos/v3/admin/ns/client/subscribe/list` | Get subscribed services by client |

**Operator Controller** (`/nacos/v3/admin/ns/ops`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/ns/ops/switches` | Get switch information |
| PUT | `/nacos/v3/admin/ns/ops/switches` | Update switches |
| GET | `/nacos/v3/admin/ns/ops/metrics` | Get naming metrics |
| PUT | `/nacos/v3/admin/ns/ops/log` | Set log level |

#### Config Admin (`/nacos/v3/admin/cs`)

**Config Controller** (`/nacos/v3/admin/cs/config`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v3/admin/cs/config` | Create configuration |
| GET | `/nacos/v3/admin/cs/config` | Get configuration |
| PUT | `/nacos/v3/admin/cs/config` | Update configuration |
| DELETE | `/nacos/v3/admin/cs/config` | Delete configuration |
| POST | `/nacos/v3/admin/cs/config/import` | Import configurations |
| GET | `/nacos/v3/admin/cs/config/export` | Export configurations |

**Config Ops Controller** (`/nacos/v3/admin/cs/ops`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v3/admin/cs/ops/localCache` | Dump local cache |
| PUT | `/nacos/v3/admin/cs/ops/log` | Set log level |
| GET | `/nacos/v3/admin/cs/ops/derby` | Derby query |
| POST | `/nacos/v3/admin/cs/ops/derby/import` | Import from external DB |

**Capacity Controller** (`/nacos/v3/admin/cs/capacity`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/cs/capacity` | Get capacity |
| POST | `/nacos/v3/admin/cs/capacity` | Set capacity |

**Listener Controller** (`/nacos/v3/admin/cs/listener`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/cs/listener` | Get listener state |

**History Controller** (`/nacos/v3/admin/cs/history`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/cs/history/list` | List config history |
| GET | `/nacos/v3/admin/cs/history/detail` | Get history detail |

**Metrics Controller** (`/nacos/v3/admin/cs/metrics`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/cs/metrics/cluster` | Get cluster metrics |
| GET | `/nacos/v3/admin/cs/metrics/ip` | Get IP metrics |

#### Core Admin (`/nacos/v3/admin/core`)

**Cluster Controller** (`/nacos/v3/admin/core/cluster`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/core/cluster/node/self` | Get current node info |
| GET | `/nacos/v3/admin/core/cluster/node/list` | List cluster nodes |
| GET | `/nacos/v3/admin/core/cluster/node/self/health` | Get node health |
| PUT | `/nacos/v3/admin/core/cluster/lookup` | Update node lookup mode |

**Namespace Controller** (`/nacos/v3/admin/core/namespace`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/core/namespace/list` | List namespaces |
| GET | `/nacos/v3/admin/core/namespace` | Get namespace detail |
| POST | `/nacos/v3/admin/core/namespace` | Create namespace |
| PUT | `/nacos/v3/admin/core/namespace` | Update namespace |
| DELETE | `/nacos/v3/admin/core/namespace` | Delete namespace |

**Ops Controller** (`/nacos/v3/admin/core/ops`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v3/admin/core/ops/raft` | Raft operations |
| GET | `/nacos/v3/admin/core/ops/ids` | Get ID generator info |
| PUT | `/nacos/v3/admin/core/ops/log` | Set log level |

**Server Loader Controller** (`/nacos/v3/admin/core/loader`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/core/loader/current` | Get current clients |
| POST | `/nacos/v3/admin/core/loader/reloadCurrent` | Reload current count |
| POST | `/nacos/v3/admin/core/loader/smartReloadCluster` | Smart reload cluster |
| GET | `/nacos/v3/admin/core/loader/reloadClient` | Reload client |
| GET | `/nacos/v3/admin/core/loader/cluster` | Get cluster info |

#### AI Admin (`/nacos/v3/admin/ai`)

**MCP Controller** (`/nacos/v3/admin/ai/mcp`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/admin/ai/mcp/list` | List MCP servers |
| GET | `/nacos/v3/admin/ai/mcp` | Get MCP server detail |
| POST | `/nacos/v3/admin/ai/mcp` | Create MCP server |
| PUT | `/nacos/v3/admin/ai/mcp` | Update MCP server |
| DELETE | `/nacos/v3/admin/ai/mcp` | Delete MCP server |

**A2A Controller** (`/nacos/v3/admin/ai/a2a`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v3/admin/ai/a2a` | Register agent |
| GET | `/nacos/v3/admin/ai/a2a` | Get agent card |
| PUT | `/nacos/v3/admin/ai/a2a` | Update agent |
| DELETE | `/nacos/v3/admin/ai/a2a` | Delete agent |
| GET | `/nacos/v3/admin/ai/a2a/list` | List agents |

### 1.3 V3 Client API (Main Server - Port 8848)

All routes are prefixed with `/nacos`. These are SDK client-facing APIs running on the main server.

#### Naming Client (`/nacos/v3/client/ns`)

**Instance Open API Controller** (`/nacos/v3/client/ns/instance`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/nacos/v3/client/ns/instance` | Register instance or heartbeat |
| DELETE | `/nacos/v3/client/ns/instance` | Deregister instance |
| GET | `/nacos/v3/client/ns/instance/list` | Get instance list |

#### Config Client (`/nacos/v3/client/cs`)

**Config Open API Controller** (`/nacos/v3/client/cs/config`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/nacos/v3/client/cs/config` | Get configuration |

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

### 1.5 V3 Auth API (Console Server)

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

## 3. Apollo Config API Routes

Apollo uses a microservice architecture with three main services:
- **Config Service** (default port 8080): Client-facing API for fetching configurations.
- **Admin Service** (default port 8090): Internal API for portal to manage configurations.
- **Portal** (default port 8070): Web UI backend with both Open API and internal endpoints.

### 3.1 Config Service API

#### Config Query (`/configs`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/configs/{appId}/{clusterName}/{namespace}` | Query configuration (supports incremental sync) |

#### Config File Query (`/configfiles`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/configfiles/{appId}/{clusterName}/{namespace}` | Query config as properties format |
| GET | `/configfiles/json/{appId}/{clusterName}/{namespace}` | Query config as JSON format |
| GET | `/configfiles/raw/{appId}/{clusterName}/{namespace}` | Query config in raw format (properties/YAML/JSON/XML) |

#### Notifications (`/notifications`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/notifications` | Long polling for config change notification (deprecated) |

#### Meta Service

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | List all service instances |
| GET | `/services/meta` | Get meta service info (deprecated) |
| GET | `/services/config` | Get Config Service instances |
| GET | `/services/admin` | Get Admin Service instances |

### 3.2 Admin Service API

#### App Management

| Method | Path | Description |
|--------|------|-------------|
| POST | `/apps` | Create application |
| DELETE | `/apps/{appId}` | Delete application |
| PUT | `/apps/{appId}` | Update application |
| GET | `/apps` | Find applications (with optional name filter) |
| GET | `/apps/{appId}` | Get application by ID |
| GET | `/apps/{appId}/unique` | Check if app ID is unique |

#### Access Key Management

| Method | Path | Description |
|--------|------|-------------|
| POST | `/apps/{appId}/accesskeys` | Create access key |
| GET | `/apps/{appId}/accesskeys` | Find access keys by app |
| DELETE | `/apps/{appId}/accesskeys/{id}` | Delete access key |
| PUT | `/apps/{appId}/accesskeys/{id}/enable` | Enable access key |
| PUT | `/apps/{appId}/accesskeys/{id}/disable` | Disable access key |

#### Cluster Management

| Method | Path | Description |
|--------|------|-------------|
| POST | `/apps/{appId}/clusters` | Create cluster |
| DELETE | `/apps/{appId}/clusters/{clusterName}` | Delete cluster |
| GET | `/apps/{appId}/clusters` | Find clusters by app |
| GET | `/apps/{appId}/clusters/{clusterName}` | Get cluster by name |
| GET | `/apps/{appId}/cluster/{clusterName}/unique` | Check cluster name uniqueness |

#### App Namespace Management

| Method | Path | Description |
|--------|------|-------------|
| POST | `/apps/{appId}/appnamespaces` | Create app namespace |
| DELETE | `/apps/{appId}/appnamespaces/{namespaceName}` | Delete app namespace |
| GET | `/apps/{appId}/appnamespaces` | Get app namespaces by app ID |
| GET | `/appnamespaces/{publicNamespaceName}/namespaces` | Find public namespace all namespaces |
| GET | `/appnamespaces/{publicNamespaceName}/associated-namespaces/count` | Count associated namespaces |

#### Namespace Management

| Method | Path | Description |
|--------|------|-------------|
| POST | `/apps/{appId}/clusters/{clusterName}/namespaces` | Create namespace |
| DELETE | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}` | Delete namespace |
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces` | Find namespaces |
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}` | Get namespace |
| GET | `/namespaces/{namespaceId}` | Get namespace by ID |
| GET | `/namespaces/find-by-item` | Find namespaces by item (paginated) |
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/associated-public-namespace` | Find associated public namespace |
| GET | `/apps/{appId}/namespaces/publish_info` | Get namespace publish info |
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/lock` | Get namespace lock owner |

#### Item Management

| Method | Path | Description |
|--------|------|-------------|
| POST | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/items` | Create item |
| POST | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/comment_items` | Create comment item |
| PUT | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/items/{itemId}` | Update item |
| DELETE | `/items/{itemId}` | Delete item |
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/items` | Find items |
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/items/deleted` | Find deleted items |
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/items/{key}` | Get item by key |
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/encodedItems/{key}` | Get item by base64-encoded key |
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/items-with-page` | Find items with pagination |
| GET | `/items/{itemId}` | Get item by ID |
| GET | `/items-search/key-and-value` | Search items by key and value |
| POST | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/itemset` | Batch update items |

#### Commit History

| Method | Path | Description |
|--------|------|-------------|
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/commit` | Find commits with optional key filter |

#### Release Management

| Method | Path | Description |
|--------|------|-------------|
| POST | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/releases` | Publish release |
| GET | `/releases/{releaseId}` | Get release by ID |
| GET | `/releases` | Find releases by release IDs |
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/releases/all` | Find all releases |
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/releases/active` | Find active releases |
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/releases/latest` | Get latest release |
| PUT | `/releases/{releaseId}/rollback` | Rollback release |
| POST | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/updateAndPublish` | Merge branch and publish |
| POST | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/gray-del-releases` | Gray deletion release |

#### Release History

| Method | Path | Description |
|--------|------|-------------|
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/releases/histories` | Find release histories by namespace |
| GET | `/releases/histories/by_release_id_and_operation` | Find history by release ID and operation |
| GET | `/releases/histories/by_previous_release_id_and_operation` | Find history by previous release ID and operation |

#### Namespace Branch (Gray Release)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/branches` | Create namespace branch |
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/branches` | Load namespace branch |
| DELETE | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/branches/{branchName}` | Delete branch |
| GET | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/branches/{branchName}/rules` | Get branch gray release rules |
| PUT | `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/branches/{branchName}/rules` | Update branch gray release rules |

#### Instance & Server Config

| Method | Path | Description |
|--------|------|-------------|
| GET | `/instances/by-release` | Get instances by release ID |
| GET | `/instances/by-namespace-and-releases-not-in` | Get instances by namespace with release filter |
| GET | `/instances/by-namespace` | Get instances by namespace |
| GET | `/instances/by-namespace/count` | Get instance count by namespace |
| GET | `/server/config/find-all-config` | Find all server configs |
| POST | `/server/config` | Create or update server config |

### 3.3 Portal Open API v1

All routes are prefixed with `/openapi/v1`.

#### App Management

| Method | Path | Description |
|--------|------|-------------|
| POST | `/openapi/v1/apps` | Create application |
| GET | `/openapi/v1/apps` | Find applications by IDs or list all |
| GET | `/openapi/v1/apps/authorized` | Find authorized applications |
| GET | `/openapi/v1/apps/{appId}` | Get single app info |
| PUT | `/openapi/v1/apps/{appId}` | Update application |
| DELETE | `/openapi/v1/apps/{appId}` | Delete application |
| GET | `/openapi/v1/apps/self` | Get current consumer's apps (paginated) |
| POST | `/openapi/v1/apps/envs/{env}` | Create app in specific environment |
| GET | `/openapi/v1/apps/{appId}/envs` | Find missing environments |
| GET | `/openapi/v1/apps/{appId}/navtree` | Get app navigation tree |

#### Environment & Organization

| Method | Path | Description |
|--------|------|-------------|
| GET | `/openapi/v1/envs` | Get list of environments |
| GET | `/openapi/v1/organizations` | Get organizations |

#### Cluster Management

| Method | Path | Description |
|--------|------|-------------|
| GET | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}` | Get cluster |
| POST | `/openapi/v1/envs/{env}/apps/{appId}/clusters` | Create cluster |
| DELETE | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}` | Delete cluster |

#### Namespace Management

| Method | Path | Description |
|--------|------|-------------|
| POST | `/openapi/v1/apps/{appId}/appnamespaces` | Create namespace |
| GET | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces` | Find namespaces |
| GET | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}` | Load namespace |
| GET | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/lock` | Get namespace lock |

#### Item Management

| Method | Path | Description |
|--------|------|-------------|
| GET | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/items/{key}` | Get item |
| GET | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/encodedItems/{key}` | Get item by base64-encoded key |
| POST | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/items` | Create item |
| PUT | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/items/{key}` | Update item |
| PUT | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/encodedItems/{key}` | Update item by base64-encoded key |
| DELETE | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/items/{key}` | Delete item |
| DELETE | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/encodedItems/{key}` | Delete item by base64-encoded key |
| GET | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/items` | Find items (paginated) |

#### Instance Management

| Method | Path | Description |
|--------|------|-------------|
| GET | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/instances` | Get instance count |

#### Release Management

| Method | Path | Description |
|--------|------|-------------|
| POST | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/releases` | Create release |
| GET | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/releases/latest` | Get latest active release |
| PUT | `/openapi/v1/envs/{env}/releases/{releaseId}/rollback` | Rollback release |

#### Branch & Gray Release

| Method | Path | Description |
|--------|------|-------------|
| GET | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/branches` | Find branch |
| POST | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/branches` | Create branch |
| DELETE | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/branches/{branchName}` | Delete branch |
| GET | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/branches/{branchName}/rules` | Get gray release rules |
| PUT | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/branches/{branchName}/rules` | Update gray release rules |
| POST | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/branches/{branchName}/merge` | Merge branch release |
| POST | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/branches/{branchName}/releases` | Create gray release |
| POST | `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}/branches/{branchName}/gray-del-releases` | Create gray deletion release |

### 3.4 Portal Internal API

These are internal portal endpoints used by the web UI.

#### App (`/apps`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/apps` | Find applications |
| GET | `/apps/by-self` | Find apps by current user (paginated) |
| POST | `/apps` | Create application |
| PUT | `/apps/{appId}` | Update application |
| DELETE | `/apps/{appId}` | Delete application |
| GET | `/apps/{appId}` | Load application |
| GET | `/apps/{appId}/navtree` | Get navigation tree |
| POST | `/apps/envs/{env}` | Create app in environment |
| GET | `/apps/{appId}/miss_envs` | Find missing environments |

#### Cluster

| Method | Path | Description |
|--------|------|-------------|
| POST | `/apps/{appId}/envs/{env}/clusters` | Create cluster |
| DELETE | `/apps/{appId}/envs/{env}/clusters/{clusterName}` | Delete cluster |
| GET | `/apps/{appId}/envs/{env}/clusters/{clusterName}` | Load cluster |

#### Namespace

| Method | Path | Description |
|--------|------|-------------|
| GET | `/appnamespaces/public` | Find public app namespaces |
| GET | `/appnamespaces/public/names` | Find public namespace names |
| GET | `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces` | Find namespaces |
| GET | `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}` | Find namespace |
| POST | `/apps/{appId}/namespaces` | Create namespace |
| POST | `/apps/{appId}/appnamespaces` | Create app namespace |
| DELETE | `/apps/{appId}/appnamespaces/{namespaceName}` | Delete app namespace |
| GET | `/apps/{appId}/appnamespaces/{namespaceName}` | Find app namespace |
| DELETE | `/apps/{appId}/envs/{env}/clusters/{clusterName}/linked-namespaces/{namespaceName}` | Delete linked namespace |
| GET | `/apps/{appId}/envs/{env}/clusters/{clusterName}/linked-namespaces/{namespaceName}/usage` | Find linked namespace usage |
| GET | `/apps/{appId}/namespaces/{namespaceName}/usage` | Find namespace usage |
| GET | `/apps/{appId}/namespaces/publish_info` | Get namespace publish info |
| GET | `/apps/{appId}/envs/{env}/clusters/{clusterName}/missing-namespaces` | Find missing namespaces |
| POST | `/apps/{appId}/envs/{env}/clusters/{clusterName}/missing-namespaces` | Create missing namespaces |

#### Item

| Method | Path | Description |
|--------|------|-------------|
| GET | `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}/items` | Find items |
| POST | `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}/item` | Create item |
| PUT | `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}/item` | Update item |
| DELETE | `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}/items/{itemId}` | Delete item |
| PUT | `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}/items` | Modify items by text |
| GET | `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}/branches/{branchName}/items` | Find branch items |
| POST | `/namespaces/{namespaceName}/diff` | Compare namespace items |
| PUT | `/apps/{appId}/namespaces/{namespaceName}/items` | Sync items to multiple namespaces |
| POST | `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}/syntax-check` | Syntax check YAML/JSON |
| PUT | `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}/revoke-items` | Revoke items |

#### Release

| Method | Path | Description |
|--------|------|-------------|
| POST | `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}/releases` | Create release |
| POST | `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}/branches/{branchName}/releases` | Create gray release |
| GET | `/envs/{env}/releases/{releaseId}` | Get release |
| GET | `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}/releases/all` | Find all releases |
| GET | `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}/releases/active` | Find active releases |
| GET | `/envs/{env}/releases/compare` | Compare releases |
| PUT | `/envs/{env}/releases/{releaseId}/rollback` | Rollback release |

#### Environment

| Method | Path | Description |
|--------|------|-------------|
| GET | `/envs` | Get environment list |

#### Audit (`/apollo/audit`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/apollo/audit/properties` | Get audit log properties |
| GET | `/apollo/audit/logs` | Find all audit logs (paginated) |
| GET | `/apollo/audit/trace` | Find audit log trace details |
| GET | `/apollo/audit/logs/opName` | Find audit logs by operation name and time range |
| GET | `/apollo/audit/logs/dataInfluences/field` | Find data influences by field |
| GET | `/apollo/audit/logs/by-name-or-type-or-operator` | Search audit logs |

#### Server Info (`/apollo`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/apollo/net` | Get network info |
| GET | `/apollo/server` | Get server info |
| GET | `/apollo/version` | Get Apollo server version |

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
| **Apollo** | Config Service | - | 8 |
| **Apollo** | Admin Service | - | 79 |
| **Apollo** | Portal Open API v1 | - | 40 |
| **Apollo** | Portal Internal | - | 50+ |
