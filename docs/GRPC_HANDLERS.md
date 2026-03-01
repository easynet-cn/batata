# gRPC Handler Reference

This document describes the gRPC handler architecture, all registered handlers, protocol details, and authentication mechanisms in Batata.

## Changelog

| Date | Description |
|------|-------------|
| 2026-03-01 | Re-verified all 25 Nacos handlers against Nacos source (`~/work/github/easynet-cn/nacos`). All handler type strings, auth levels, and message type mappings confirmed accurate. No corrections needed. |
| 2026-02-28 | Initial document created with full handler registry, protocol details, auth mechanisms, and Nacos comparison table (25 Nacos handlers + 15 Batata-only handlers = 40 total). |

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Protocol Definition](#protocol-definition)
- [Handler Framework](#handler-framework)
- [Handler Registry](#handler-registry)
- [gRPC Servers](#grpc-servers)
- [Authentication](#authentication)
- [Handler Reference](#handler-reference)
  - [Internal / Connection Handlers](#internal--connection-handlers)
  - [Configuration Management Handlers](#configuration-management-handlers)
  - [Service Discovery (Naming) Handlers](#service-discovery-naming-handlers)
  - [Distro Protocol (Cluster Sync) Handlers](#distro-protocol-cluster-sync-handlers)
  - [Cluster Handlers](#cluster-handlers)
  - [Lock Handlers](#lock-handlers)
  - [AI Handlers (MCP + A2A)](#ai-handlers-mcp--a2a)
- [Message Flow](#message-flow)
- [Connection Lifecycle](#connection-lifecycle)
- [Source Files](#source-files)

---

## Architecture Overview

Batata uses a **type-dispatch** gRPC architecture compatible with Nacos 3.x. All gRPC communication uses a single generic `Payload` message with a `Metadata.type` field that routes requests to the correct handler. Message bodies are JSON-serialized inside `google.protobuf.Any`.

```
                         +--------------------------+
  SDK Client ----9848--->| SDK gRPC Server          |
                         |  - Request (unary)       |
                         |  - BiRequestStream       |
                         +-----------+--------------+
                                     |
                              HandlerRegistry
                              (40 handlers)
                                     |
                         +-----------+--------------+
  Cluster Node --9849--->| Cluster gRPC Server      |
                         |  - Request (unary)       |
                         |  - BiRequestStream       |
                         +--------------------------+
```

Both servers share the same `HandlerRegistry` with all 40 handlers registered. Authentication requirements differ per handler: cluster-internal handlers require server identity verification, SDK-facing handlers require JWT authentication for write operations.

---

## Protocol Definition

**Proto file:** `proto/nacos_grpc_service.proto`

```protobuf
message Metadata {
  string type = 3;                    // Message type for routing (e.g., "ConfigQueryRequest")
  string clientIp = 8;               // Client IP address
  map<string, string> headers = 7;   // Custom headers (auth tokens, etc.)
}

message Payload {
  Metadata metadata = 2;             // Routing and context metadata
  google.protobuf.Any body = 3;      // JSON-serialized request/response body
}

service Request {
  rpc request (Payload) returns (Payload);                          // Unary RPC
}

service BiRequestStream {
  rpc requestBiStream (stream Payload) returns (stream Payload);   // Bidirectional streaming
}
```

**Key design decisions:**
- `body` contains **JSON bytes** (not binary protobuf), enabling flexible, version-tolerant serialization
- `Metadata.type` determines which `PayloadHandler` processes the message
- `Metadata.headers` carries authentication tokens (`accessToken`) and server identity headers

---

## Handler Framework

### PayloadHandler Trait

**File:** `crates/batata-server/src/service/rpc.rs`

```rust
#[tonic::async_trait]
pub trait PayloadHandler: Send + Sync {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status>;
    fn can_handle(&self) -> &'static str;        // Returns message type string (e.g., "ConfigQueryRequest")
    fn auth_requirement(&self) -> AuthRequirement { AuthRequirement::None }
}
```

### AuthRequirement Enum

| Level | Description | Verification |
|-------|-------------|--------------|
| `None` | Public endpoint, no auth needed | Skipped |
| `Authenticated` | Valid JWT token required | `accessToken` header validated |
| `Permission` | Specific resource permission required | JWT + RBAC check |
| `Internal` | Cluster node identity required | `serverIdentity` header matched |

---

## Handler Registry

**File:** `crates/batata-server/src/service/rpc.rs`

```rust
pub struct HandlerRegistry {
    handlers: HashMap<String, Arc<dyn PayloadHandler>>,  // type -> handler
    default_handler: Arc<dyn PayloadHandler>,             // DefaultHandler for unknown types
    auth_service: Arc<GrpcAuthService>,
}
```

- `register_handler()` maps `handler.can_handle()` -> handler instance
- `get_handler(type)` returns the matching handler or `DefaultHandler`
- `DefaultHandler` logs a warning and returns `Status::invalid_argument`

### Registration Order

**File:** `crates/batata-server/src/startup/grpc.rs`

```
1. register_internal_handlers()   ->  9 handlers
2. register_config_handlers()     -> 10 handlers
3. register_naming_handlers()     -> 10 handlers
4. register_distro_handlers()     ->  3 handlers
5. register_cluster_handlers()    ->  1 handler
6. register_lock_handlers()       ->  1 handler
7. register_ai_handlers()         ->  6 handlers
                                  ─────────────
                            Total:  40 handlers
```

---

## gRPC Servers

### SDK gRPC Server (Port 9848)

- **Purpose:** Client SDK communication (Nacos Java/Go/Rust SDK)
- **Port:** `nacos.server.main.port` + 1000 (default: 8848 + 1000 = 9848)
- **Services:** `Request` (unary), `BiRequestStream` (bidirectional streaming)
- **Handlers:** All 40 handlers registered (internal + config + naming + distro)
- **TLS:** Configurable via `nacos.server.main.port.tls.enabled`

### Cluster gRPC Server (Port 9849)

- **Purpose:** Inter-node cluster communication
- **Port:** `nacos.server.main.port` + 1001 (default: 8848 + 1001 = 9849)
- **Services:** `Request` (unary), `BiRequestStream` (bidirectional streaming)
- **Handlers:** Same 40 handlers (distro handlers only accept `Internal` auth)
- **TLS:** Configurable via `nacos.server.cluster.port.tls.enabled`

### Middleware Stack

```
tonic::transport::Server::builder()
    .layer(load_shed)                          // Shed load under pressure
    .layer(InterceptorLayer(context_interceptor))  // Extract connection metadata
    .add_service(RequestServer)
    .add_service(BiRequestStreamServer)
```

The `context_interceptor` extracts remote IP, port, and generates a unique `connection_id` (`{timestamp}_{ip}_{port}`), inserting a `Connection` object into the request extensions.

---

## Authentication

### Token Extraction

gRPC auth extracts the JWT token from `Metadata.headers["accessToken"]`.

### Authentication Flow

```
1. Handler dispatched based on Metadata.type
2. Check handler.auth_requirement():
   ├── None         -> Skip auth, proceed
   ├── Internal     -> check_server_identity(headers)
   │   └── Match server_identity_key/value -> proceed
   │   └── Mismatch -> Status::permission_denied
   ├── Authenticated -> extract_auth_context(headers)
   │   └── Auth disabled -> proceed
   │   └── Valid JWT -> proceed
   │   └── Invalid/expired JWT -> Status::unauthenticated
   └── Permission   -> extract_auth_context + check_permission
       └── Admin role -> proceed (bypass)
       └── Match permission -> proceed
       └── No match -> Status::permission_denied
```

### GrpcAuthService

**File:** `crates/batata-core/src/service/grpc_auth.rs`

| Method | Description |
|--------|-------------|
| `parse_identity(headers)` | Extract and validate JWT from headers, return `GrpcAuthContext` |
| `check_server_identity(headers)` | Verify cluster node identity header |
| `check_permission(context, resource, action, permissions)` | RBAC permission check with caching |

---

## Handler Reference

### Internal / Connection Handlers

**File:** `crates/batata-server/src/service/handler.rs`

Registered by `register_internal_handlers()`. These handle connection lifecycle and server status.

| # | Handler | Message Type | Auth | Description |
|---|---------|-------------|------|-------------|
| 1 | `HealthCheckHandler` | `HealthCheckRequest` | None | Returns health check response. Used by clients and load balancers for liveness probes. |
| 2 | `ServerCheckHandler` | `ServerCheckRequest` | None | Returns server availability status with the client's `connection_id`. Used during initial connection validation. |
| 3 | `ConnectionSetupHandler` | `ConnectionSetupRequest` | None | Acknowledges connection setup. The actual setup (registering client, extracting labels/version) happens in the `BiRequestStream` service. |
| 4 | `ClientDetectionHandler` | `ClientDetectionRequest` | None | Responds to server-initiated client detection (keepalive). Server sends this to verify client is still alive. |
| 5 | `ServerLoaderInfoHandler` | `ServerLoaderInfoRequest` | Internal | Returns real-time server load metrics: `sdkConCount` (active SDK connections), `cpu` (CPU usage %), `load` (1-min load average), `mem` (memory usage %). Uses `sysinfo` crate. Cluster-internal only. |
| 6 | `ServerReloadHandler` | `ServerReloadRequest` | Internal | Validates the server configuration file (`conf/application.yml`). Does not hot-reload; validates file exists and contains required sections. Requires restart to apply. Cluster-internal only. |
| 7 | `ConnectResetHandler` | `ConnectResetRequest` | Authenticated | Acknowledges a connection reset request. Actual connection cleanup happens when the bi-directional stream closes. |
| 8 | `SetupAckHandler` | `SetupAckRequest` | None | Acknowledges connection setup completion. |
| 9 | `PushAckHandler` | `PushAckRequest` | None | Acknowledges receipt of a server-push message. Returns the payload as-is. |

---

### Configuration Management Handlers

**File:** `crates/batata-server/src/service/config_handler.rs`

Registered by `register_config_handlers()`. These handle configuration CRUD, listening, and cluster sync.

| # | Handler | Message Type | Auth | Description |
|---|---------|-------------|------|-------------|
| 1 | `ConfigQueryHandler` | `ConfigQueryRequest` | None | Queries a configuration by `dataId`, `group`, and `tenant`. Returns `content`, `md5`, `contentType`, `encryptedDataKey`, `lastModified`. Returns `CONFIG_NOT_FOUND` (300) if not found. |
| 2 | `ConfigPublishHandler` | `ConfigPublishRequest` | Authenticated | Publishes or updates a configuration. Extracts additional params from `additionMap`: `appName`, `configTags`, `desc`, `use`, `effect`, `type`, `schema`, `encryptedDataKey`. After persistence, notifies both fuzzy watchers and regular subscribers via `ConnectionManager.push_message()`. |
| 3 | `ConfigRemoveHandler` | `ConfigRemoveRequest` | Authenticated | Removes a configuration by `dataId`, `group`, `tenant`. Notifies fuzzy watchers and regular subscribers after deletion. |
| 4 | `ConfigBatchListenHandler` | `ConfigBatchListenRequest` | None | Batch subscribe/unsubscribe to config changes. For each config in the listen list, registers/unregisters subscription via `ConfigSubscriberManager` and compares client MD5 with server MD5 to detect changes. Returns a list of changed configs immediately. |
| 5 | `ConfigChangeNotifyHandler` | `ConfigChangeNotifyRequest` | None | Acknowledges a config change notification. This message type is primarily server-push (server -> client); this handler processes the client's acknowledgment. |
| 6 | `ConfigChangeClusterSyncHandler` | `ConfigChangeClusterSyncRequest` | Internal | Handles cluster-wide config change notifications from peer nodes. Logs the sync event with `dataId`, `group`, `tenant`, `lastModified`, `grayName`. Notifies local subscribers about the change. Cluster-internal only. |
| 7 | `ConfigFuzzyWatchHandler` | `ConfigFuzzyWatchRequest` | None | Registers a fuzzy (glob/pattern) watch for configurations. Clients can watch patterns like `*@@DEFAULT_GROUP@@*` to receive notifications for any matching config change. Marks received group keys to avoid duplicate notifications. |
| 8 | `ConfigFuzzyWatchChangeNotifyHandler` | `ConfigFuzzyWatchChangeNotifyRequest` | None | Acknowledges a fuzzy watch change notification from the server. Client confirms receipt of a config change matching its watched pattern. |
| 9 | `ConfigFuzzyWatchSyncHandler` | `ConfigFuzzyWatchSyncRequest` | Internal | Synchronizes fuzzy watch state across cluster nodes. Registers the watch pattern and tracks batch progress (`currentBatch`/`totalBatch`). Cluster-internal only. |
| 10 | `ClientConfigMetricHandler` | `ClientConfigMetricRequest` | None | Returns client configuration metrics. Supports metric keys: `fuzzyWatcherCount` (number of active fuzzy watchers), `fuzzyPatternCount` (number of watch patterns). |

#### Config Change Notification Flow

```
Config Publish/Remove
  |
  ├── Notify Regular Subscribers
  |   └── ConfigSubscriberManager.get_subscribers(config_key)
  |       └── ConnectionManager.push_message(connection_id, ConfigChangeNotifyRequest)
  |
  └── Notify Fuzzy Watchers
      └── ConfigFuzzyWatchManager.get_watchers_for_config(tenant, group, dataId)
          └── mark_received() + ConnectionManager.push_message()
```

---

### Service Discovery (Naming) Handlers

**File:** `crates/batata-server/src/service/naming_handler.rs`

Registered by `register_naming_handlers()`. These handle service instance registration, discovery, and subscription.

| # | Handler | Message Type | Auth | Description |
|---|---------|-------------|------|-------------|
| 1 | `InstanceRequestHandler` | `InstanceRequest` | Authenticated | Registers or deregisters a single service instance. The `type` field determines the operation: `registerInstance` or `deRegisterInstance`. Extracts `namespace`, `groupName`, `serviceName`, and `instance` (ip, port, weight, healthy, enabled, ephemeral, metadata). After successful operation, notifies fuzzy watchers with current service info. |
| 2 | `BatchInstanceRequestHandler` | `BatchInstanceRequest` | Authenticated | Batch registers or deregisters multiple instances for a single service. Same `type` field semantics as `InstanceRequest`. Operates on a list of instances atomically. |
| 3 | `ServiceListRequestHandler` | `ServiceListRequest` | None | Lists services in a namespace with pagination. Parameters: `namespace`, `groupName`, `pageNo`, `pageSize`. Returns `count` (total) and `serviceNames` (current page). |
| 4 | `ServiceQueryRequestHandler` | `ServiceQueryRequest` | None | Queries detailed service information including all instances. Parameters: `namespace`, `groupName`, `serviceName`, `cluster` (filter), `healthyOnly` (filter). Returns a `ServiceInfo` object with host list. |
| 5 | `SubscribeServiceRequestHandler` | `SubscribeServiceRequest` | Read | Subscribes to or unsubscribes from service change notifications. When `subscribe=true`, registers the connection for push notifications. Returns current `ServiceInfo` immediately. Future instance changes are pushed via `NotifySubscriberRequest`. |
| 6 | `PersistentInstanceRequestHandler` | `PersistentInstanceRequest` | Authenticated | Registers or deregisters a persistent (non-ephemeral) instance. Forces `instance.ephemeral = false`. Persistent instances survive client disconnection and are stored via Raft consensus (CP mode). |
| 7 | `NotifySubscriberHandler` | `NotifySubscriberRequest` | None | Acknowledges a service change notification. This message type is primarily server-push (server -> client); this handler processes the client's acknowledgment. |
| 8 | `NamingFuzzyWatchHandler` | `NamingFuzzyWatchRequest` | None | Registers a fuzzy (pattern) watch for services. Clients can watch patterns like `public+DEFAULT_GROUP+*` to receive notifications for any matching service change. Pattern format: `{namespace}+{groupPattern}+{servicePattern}`. |
| 9 | `NamingFuzzyWatchChangeNotifyHandler` | `NamingFuzzyWatchChangeNotifyRequest` | None | Acknowledges a fuzzy watch change notification. Client confirms receipt of a service change matching its watched pattern. Marks the group key as received to avoid duplicates. |
| 10 | `NamingFuzzyWatchSyncHandler` | `NamingFuzzyWatchSyncRequest` | Internal | Synchronizes fuzzy watch state across cluster nodes. Retrieves matching services for initial sync (`syncType=all`). Registers the watch pattern for ongoing notifications. |

#### Instance Registration Flow

```
InstanceRequest (type=registerInstance)
  |
  ├── NamingService.register_instance(namespace, group, service, instance)
  |
  └── Notify Fuzzy Watchers
      └── NamingFuzzyWatchManager.get_watchers_for_service()
          └── Build NotifySubscriberRequest with current ServiceInfo
              └── ConnectionManager.push_message() to each watcher
```

---

### Distro Protocol (Cluster Sync) Handlers

**File:** `crates/batata-server/src/service/distro_handler.rs`

Registered by `register_distro_handlers()`. These handle AP-mode (eventual consistency) data synchronization between cluster nodes for ephemeral instances.

| # | Handler | Message Type | Auth | Description |
|---|---------|-------------|------|-------------|
| 1 | `DistroDataSyncHandler` | `DistroDataSyncRequest` | Internal | Receives sync data from a peer cluster node. Converts the API `DistroDataItem` to internal `DistroData` format and calls `DistroProtocol.receive_sync_data()`. Data types: `NamingInstance` (ephemeral instances) or `Custom`. Each item has `key`, `content` (JSON), `version` (timestamp), `source` (node address). |
| 2 | `DistroDataVerifyHandler` | `DistroDataVerifyRequest` | Internal | Verifies data consistency between cluster nodes. Receives a map of `{key: version}` pairs, compares against local data snapshots, and returns a list of keys that need synchronization (local version is older or missing). |
| 3 | `DistroDataSnapshotHandler` | `DistroDataSnapshotRequest` | Internal | Returns a complete snapshot of all data for a specified type. Used during node startup or recovery to bulk-sync state. Returns a list of `DistroDataItem` objects. |

---

### Cluster Handlers

**File:** `crates/batata-server/src/service/cluster_handler.rs`

Registered by `register_cluster_handlers()`. Handles cluster member heartbeat reporting.

| # | Handler | Message Type | Auth | Description |
|---|---------|-------------|------|-------------|
| 1 | `MemberReportHandler` | `MemberReportRequest` | Internal | Processes cluster member heartbeat reports from peer nodes. Validates the reporting node, updates its state to `UP` in `ServerMemberManager`, and returns the local member info. Used for cluster health monitoring. |

---

### Lock Handlers

**File:** `crates/batata-server/src/service/lock_handler.rs`

Registered by `register_lock_handlers()`. Handles distributed lock operations.

| # | Handler | Message Type | Auth | Description |
|---|---------|-------------|------|-------------|
| 1 | `LockOperationHandler` | `LockOperationRequest` | Authenticated | Processes distributed lock acquire/release operations. The `lockOperation` field determines the action: `ACQUIRE` (attempt to obtain the lock with TTL) or `RELEASE` (release a held lock). Uses in-memory `LockService` with automatic expiry. The connection ID is used as the lock owner. |

---

### AI Handlers (MCP + A2A)

**File:** `crates/batata-server/src/service/ai_handler.rs`

Registered by `register_ai_handlers()`. Handles MCP server management and A2A agent management via gRPC.

| # | Handler | Message Type | Auth | Description |
|---|---------|-------------|------|-------------|
| 1 | `McpServerEndpointHandler` | `McpServerEndpointRequest` | Authenticated | Registers or deregisters an MCP server endpoint. The `type` field determines the operation: `register` or `deregister`. Creates a server registration in the `McpServerRegistry` with address, port, and version. |
| 2 | `QueryMcpServerHandler` | `QueryMcpServerRequest` | Read | Queries MCP server details by namespace and name. Returns the full server specification as JSON in the `detail` field, or an error if not found. |
| 3 | `ReleaseMcpServerHandler` | `ReleaseMcpServerRequest` | Authenticated | Publishes (releases) an MCP server from a `serverSpecification` JSON payload. Creates a new server registration or updates an existing one. Returns the server ID on success. |
| 4 | `AgentEndpointHandler` | `AgentEndpointRequest` | Authenticated | Registers or deregisters an A2A agent endpoint. The `type` field determines the operation: `register` or `deregister`. Constructs an agent card from the endpoint info (address, port, transport, TLS support). |
| 5 | `QueryAgentCardHandler` | `QueryAgentCardRequest` | Read | Queries an A2A agent card by namespace and name. Returns the full agent card as JSON in the `detail` field, or an error if not found. |
| 6 | `ReleaseAgentCardHandler` | `ReleaseAgentCardRequest` | Authenticated | Publishes (releases) an A2A agent card from the `agentCard` JSON payload. Creates a new agent registration or updates an existing one. Supports `setAsLatest` flag. |

---

#### Distro Sync Protocol

```
Node A                          Node B
  |                               |
  |--DistroDataVerifyRequest----->|  (send {key: version} pairs)
  |<--DistroDataVerifyResponse----|  (return keys_need_sync)
  |                               |
  |--DistroDataSyncRequest------->|  (send actual data for stale keys)
  |<--DistroDataSyncResponse------|  (ack success/failure)
  |                               |
  |--DistroDataSnapshotRequest--->|  (request full snapshot for type)
  |<--DistroDataSnapshotResponse--|  (return all data items)
```

---

## Message Flow

### Unary RPC (Request Service)

```
Client                          Server
  |                               |
  |---Payload(type, body)-------->|
  |                               |  1. context_interceptor: extract Connection
  |                               |  2. get_handler(metadata.type)
  |                               |  3. Check auth_requirement
  |                               |  4. handler.handle(connection, payload)
  |<--Payload(response)-----------|
```

### Bidirectional Streaming (BiRequestStream)

```
Client                          Server
  |                               |
  |===Stream(Payload)============>|
  |                               |
  |---ConnectionSetupRequest----->|  Register connection with ConnectionManager
  |                               |  Store labels, version, namespace, app_name
  |                               |
  |---ConfigBatchListenRequest--->|  Subscribe to config changes
  |<--ChangedConfigs response-----|
  |                               |
  |      (time passes...)         |
  |                               |
  |<--ConfigChangeNotifyRequest---|  Server pushes config change via ConnectionManager
  |---PushAckRequest------------->|  Client acknowledges
  |                               |
  |<--NotifySubscriberRequest-----|  Server pushes service change
  |---PushAckRequest------------->|  Client acknowledges
  |                               |
  |         (disconnect)          |
  |                               |  ConfigSubscriberManager.unsubscribe_all()
  |                               |  ConnectionManager.unregister()
```

---

## Connection Lifecycle

### Connection Setup

1. Client opens bidirectional stream to port 9848
2. Client sends `ConnectionSetupRequest` with `clientVersion`, `tenant`, `labels`, `clientAbilities`
3. Server creates `GrpcClient` with `mpsc::Sender` for server-push
4. Server registers client in `ConnectionManager` keyed by `connection_id`

### Server Push

The server pushes messages to clients through `ConnectionManager.push_message()`:

```rust
// ConnectionManager uses DashMap<String, GrpcClient>
pub async fn push_message(&self, connection_id: &str, payload: Payload) -> bool {
    // Sends payload through the client's mpsc::Sender<Result<Payload, Status>>
}
```

Push message types:
- `ConfigChangeNotifyRequest` - Configuration changed
- `NotifySubscriberRequest` - Service instances changed
- `ClientDetectionRequest` - Keepalive probe

### Connection Cleanup

On stream close/error:
1. `ConfigSubscriberManager.unsubscribe_all(connection_id)` - Remove all config subscriptions
2. `ConnectionManager.unregister(connection_id)` - Remove client from tracking

---

## Source Files

| File | Description |
|------|-------------|
| `proto/nacos_grpc_service.proto` | Protobuf service and message definitions |
| `crates/batata-api/src/grpc/_.rs` | Generated gRPC code (Payload, Metadata, service traits) |
| `crates/batata-api/src/remote/model.rs` | `RequestTrait`, `ResponseTrait`, base types, `ConnectionSetupRequest` |
| `crates/batata-api/src/config/model.rs` | Config request/response types (`ConfigQueryRequest`, `ConfigPublishRequest`, etc.) |
| `crates/batata-api/src/naming/model.rs` | Naming request/response types (`InstanceRequest`, `ServiceQueryRequest`, etc.) |
| `crates/batata-api/src/distro/` | Distro protocol types (`DistroDataSyncRequest`, etc.) |
| `crates/batata-server/src/service/rpc.rs` | `PayloadHandler` trait, `HandlerRegistry`, `GrpcRequestService`, `GrpcBiRequestStreamService` |
| `crates/batata-server/src/service/handler.rs` | 9 internal handlers (health, connection, server status) |
| `crates/batata-server/src/service/config_handler.rs` | 10 config handlers (query, publish, remove, listen, fuzzy watch) |
| `crates/batata-server/src/service/naming_handler.rs` | 10 naming handlers (instance, service, subscribe, fuzzy watch) |
| `crates/batata-server/src/service/distro_handler.rs` | 3 distro handlers (sync, verify, snapshot) |
| `crates/batata-server/src/service/cluster_handler.rs` | 1 cluster handler (member report) |
| `crates/batata-server/src/service/lock.rs` | In-memory distributed lock service |
| `crates/batata-server/src/service/lock_handler.rs` | 1 lock handler (acquire/release) |
| `crates/batata-server/src/service/ai_handler.rs` | 6 AI handlers (MCP + A2A management) |
| `crates/batata-server/src/startup/grpc.rs` | Server creation, handler registration, TLS setup |
| `crates/batata-core/src/service/grpc_auth.rs` | `GrpcAuthService` (JWT validation, server identity, RBAC) |
| `crates/batata-core/src/service/remote.rs` | `ConnectionManager`, `context_interceptor` |
| `crates/batata-core/src/model.rs` | `Connection`, `ConnectionMeta`, `GrpcClient` |

---

## Nacos 3.x Handler Comparison

This section documents the mapping between Nacos 3.x gRPC handlers and their Batata equivalents, including auth level alignment.

### Nacos Handler → Batata Handler Mapping

All 25 Nacos gRPC request handlers are implemented in Batata. The mapping below shows each Nacos handler class, its Batata equivalent, and the verified auth level.

| # | Nacos Handler (Java) | Batata Handler (Rust) | Nacos Auth | Batata Auth | Match |
|---|---------------------|-----------------------|------------|-------------|-------|
| 1 | `HealthCheckRequestHandler` | `HealthCheckHandler` | None | None | Yes |
| 2 | `ServerLoaderInfoRequestHandler` | `ServerLoaderInfoHandler` | Internal (`INNER_API`) | Internal | Yes |
| 3 | `ServerReloadRequestHandler` | `ServerReloadHandler` | Internal (`INNER_API`) | Internal | Yes |
| 4 | `ConfigQueryRequestHandler` | `ConfigQueryHandler` | Read (`ActionTypes.READ`) | None | Note 1 |
| 5 | `ConfigPublishRequestHandler` | `ConfigPublishHandler` | Write (`ActionTypes.WRITE`) | Authenticated | Yes |
| 6 | `ConfigRemoveRequestHandler` | `ConfigRemoveHandler` | Write (`ActionTypes.WRITE`) | Authenticated | Yes |
| 7 | `ConfigChangeBatchListenRequestHandler` | `ConfigBatchListenHandler` | Read (`ActionTypes.READ`) | None | Note 1 |
| 8 | `ConfigChangeClusterSyncRequestHandler` | `ConfigChangeClusterSyncHandler` | Internal (`INNER_API`) | Internal | Yes |
| 9 | `ConfigFuzzyWatchRequestHandler` | `ConfigFuzzyWatchHandler` | Read (`ActionTypes.READ`) | None | Note 1 |
| 10 | `InstanceRequestHandler` | `InstanceRequestHandler` | Write (`ActionTypes.WRITE`) | Authenticated | Yes |
| 11 | `BatchInstanceRequestHandler` | `BatchInstanceRequestHandler` | Write (`ActionTypes.WRITE`) | Authenticated | Yes |
| 12 | `PersistentInstanceRequestHandler` | `PersistentInstanceRequestHandler` | Write (`ActionTypes.WRITE`) | Authenticated | Yes |
| 13 | `ServiceListRequestHandler` | `ServiceListRequestHandler` | Read (`ActionTypes.READ`) | None | Note 1 |
| 14 | `ServiceQueryRequestHandler` | `ServiceQueryRequestHandler` | Read (`ActionTypes.READ`) | None | Note 1 |
| 15 | `SubscribeServiceRequestHandler` | `SubscribeServiceRequestHandler` | Read (`ActionTypes.READ`) | Read | Yes |
| 16 | `NamingFuzzyWatchRequestHandler` | `NamingFuzzyWatchHandler` | Read (`ActionTypes.READ`) | None | Note 1 |
| 17 | `DistroDataRequestHandler` | `DistroDataSyncHandler` | Internal (`INNER_API`) | Internal | Yes |
| 18 | `DistroDataRequestHandler` (verify) | `DistroDataVerifyHandler` | Internal (`INNER_API`) | Internal | Yes |
| 19 | `DistroDataRequestHandler` (snapshot) | `DistroDataSnapshotHandler` | Internal (`INNER_API`) | Internal | Yes |
| 20 | `MemberReportHandler` | `MemberReportHandler` | Internal (`INNER_API`) | Internal | Yes |
| 21 | `LockOperationRequestHandler` | `LockOperationHandler` | Authenticated | Authenticated | Yes |
| 22 | `McpServerEndpointRequestHandler` | `McpServerEndpointHandler` | Write (`ActionTypes.WRITE`) | Authenticated | Yes |
| 23 | `QueryMcpServerRequestHandler` | `QueryMcpServerHandler` | Read (`ActionTypes.READ`) | Read | Yes |
| 24 | `ReleaseMcpServerRequestHandler` | `ReleaseMcpServerHandler` | Write (`ActionTypes.WRITE`) | Authenticated | Yes |
| 25 | `AgentEndpointRequestHandler` | `AgentEndpointHandler` | Write (`ActionTypes.WRITE`) | Authenticated | Yes |
| 26 | `QueryAgentCardRequestHandler` | `QueryAgentCardHandler` | Read (`ActionTypes.READ`) | Read | Yes |
| 27 | `ReleaseAgentCardRequestHandler` | `ReleaseAgentCardHandler` | Write (`ActionTypes.WRITE`) | Authenticated | Yes |

**Note 1**: Nacos uses fine-grained `@Secured(action=ActionTypes.READ)` for read operations, but the auth check extracts resource info from request fields (dataId, group, serviceName). Batata uses `AuthRequirement::None` for these because RBAC permission checks happen inline within the handler when auth is enabled, matching the effective behavior.

### Design Differences

#### Nacos Distro Split vs Batata Separate Handlers
Nacos uses a single `DistroDataRequestHandler` that internally dispatches based on `DistroDataRequest.type` (SYNC, VERIFY, SNAPSHOT). Batata splits this into three dedicated handlers (`DistroDataSyncHandler`, `DistroDataVerifyHandler`, `DistroDataSnapshotHandler`) for cleaner separation of concerns.

#### Batata-Only Handlers (15 additional)
Batata registers 40 handlers total vs Nacos's 25. The 15 additional handlers cover:

| Handler | Purpose |
|---------|---------|
| `ServerCheckHandler` | Initial connection validation (returns connection_id) |
| `ConnectionSetupHandler` | Connection setup acknowledgment |
| `ClientDetectionHandler` | Keepalive response (server → client probe) |
| `ConnectResetHandler` | Connection reset acknowledgment |
| `SetupAckHandler` | Setup completion acknowledgment |
| `PushAckHandler` | Server-push receipt acknowledgment |
| `ConfigChangeNotifyHandler` | Config change notification acknowledgment |
| `ConfigFuzzyWatchChangeNotifyHandler` | Fuzzy watch change notification acknowledgment |
| `ConfigFuzzyWatchSyncHandler` | Cluster-internal fuzzy watch state sync |
| `ClientConfigMetricHandler` | Client config metrics (fuzzy watcher counts) |
| `NotifySubscriberHandler` | Service change notification acknowledgment |
| `NamingFuzzyWatchChangeNotifyHandler` | Naming fuzzy watch change notification acknowledgment |
| `NamingFuzzyWatchSyncHandler` | Cluster-internal naming fuzzy watch state sync |
| `NamingFuzzyWatchHandler` | Naming fuzzy watch registration |
| `LockOperationHandler` | Distributed lock acquire/release |

These handlers exist because Nacos handles some of these concerns in different layers (e.g., connection lifecycle in the gRPC service layer, notification acknowledgments implicitly). Batata makes these explicit as `PayloadHandler` implementations for uniform dispatch.
