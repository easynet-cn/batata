# Batata SDK Compatibility Tests

This directory contains integration tests using official client SDKs from Nacos and Consul projects to validate Batata's API compatibility.

## Test Structure

```
sdk-tests/
├── nacos-java-tests/          # Nacos Java SDK tests (328 tests)
│   ├── pom.xml
│   └── src/test/java/io/batata/tests/
│       ├── NacosConfigServiceTest.java      # Config CRUD, listeners
│       ├── NacosNamingServiceTest.java      # Service discovery
│       ├── NacosGrpcTest.java               # gRPC handlers (16 tests)
│       ├── NacosAdminApiTest.java           # Namespace, cluster, capacity (20 tests)
│       ├── NacosConfigHistoryTest.java      # History tracking
│       ├── NacosAggregateConfigTest.java    # Aggregate configs
│       ├── NacosV3ConsoleApiTest.java       # Nacos 3.x V3 Console APIs (25 tests)
│       ├── NacosInstanceSelectionTest.java  # Instance selection tests (13 tests)
│       ├── NacosAdvancedNamingTest.java     # Batch ops, metadata (16 tests)
│       ├── NacosFailoverTest.java           # Cache & failover (13 tests)
│       ├── NacosClusterManagementTest.java  # Cluster management (15 tests)
│       ├── NacosConfigListenerTest.java     # Config listeners (13 tests)
│       ├── NacosServiceSubscribeTest.java   # Service subscriptions (12 tests)
│       ├── NacosHealthCheckTest.java        # Health check functionality (12 tests)
│       ├── NacosMetadataTest.java           # Instance/service metadata (15 tests)
│       ├── NacosBatchConfigTest.java        # Batch config operations (15 tests)
│       ├── NacosServiceSelectorTest.java    # Service selector strategies (15 tests)
│       ├── NacosConfigTypeTest.java         # Config formats YAML/JSON/XML (18 tests)
│       ├── NacosConnectionTest.java         # Connection management (15 tests)
│       ├── NacosPersistenceTest.java        # Ephemeral/persistent instances (16 tests)
│       ├── NacosWeightTest.java             # Weight & load balancing (15 tests)
│       ├── NacosServerStatusTest.java       # Server status & cluster info (15 tests) NEW
│       └── NacosNamespaceTest.java          # Namespace management (15 tests) NEW
├── consul-go-tests/           # Consul Go SDK tests (355 tests)
│   ├── go.mod
│   ├── agent_test.go                        # Basic agent API
│   ├── agent_advanced_test.go               # Advanced agent features
│   ├── agent_connect_test.go                # Connect & proxy (19 tests)
│   ├── kv_test.go                           # KV store operations
│   ├── health_test.go                       # Health API
│   ├── health_advanced_test.go              # Health advanced (17 tests)
│   ├── catalog_test.go                      # Catalog API
│   ├── catalog_advanced_test.go             # Catalog advanced (18 tests)
│   ├── catalog_filter_test.go               # Catalog filtering/pagination (18 tests)
│   ├── session_test.go                      # Session API (8 tests)
│   ├── acl_test.go                          # ACL tokens, policies, roles (15 tests)
│   ├── event_test.go                        # Event API
│   ├── query_test.go                        # Prepared queries
│   ├── query_advanced_test.go               # Advanced prepared queries (18 tests)
│   ├── status_test.go                       # Cluster status
│   ├── lock_test.go                         # Distributed locking (10 tests)
│   ├── semaphore_test.go                    # Resource semaphores (11 tests)
│   ├── connect_test.go                      # Connect CA/Intentions (9 tests)
│   ├── operator_test.go                     # Operator Raft/Autopilot (19 tests)
│   ├── config_entry_test.go                 # Config entries (17 tests)
│   ├── snapshot_test.go                     # Snapshot backup/restore (9 tests)
│   ├── txn_test.go                          # KV transactions (17 tests)
│   ├── namespace_test.go                    # Namespace/Partition/Peering (16 tests)
│   ├── coordinate_test.go                   # Coordinate/Debug/Raw API (16 tests)
│   ├── watch_test.go                        # Watch key/service/health (16 tests)
│   ├── metrics_test.go                      # Agent metrics/self/members (22 tests)
│   ├── service_mesh_test.go                 # Service mesh features (18 tests)
│   ├── discovery_test.go                    # Service discovery patterns (15 tests)
│   └── maintenance_test.go                  # Maintenance mode & lifecycle (14 tests) NEW
├── docker-compose.yml         # Test environment
└── README.md                  # This file

Total: 683 SDK compatibility tests
```

## Prerequisites

- Java 21+ (for Nacos tests)
- Go 1.21+ (for Consul tests)
- Docker and Docker Compose
- Maven 3.8+

## Running Tests

### Option 1: Local Development

1. Start Batata server:
```bash
# From project root
cargo run -p batata-server
```

2. Run Nacos Java tests:
```bash
cd sdk-tests/nacos-java-tests
mvn test
```

3. Run Consul Go tests:
```bash
cd sdk-tests/consul-go-tests
go test -v ./...
```

### Option 2: Docker Compose

```bash
# Start all services
cd sdk-tests
docker-compose up -d

# Run all tests
docker-compose --profile tests up

# Run specific test suite
docker-compose --profile tests up nacos-tests
docker-compose --profile tests up consul-tests

# Cleanup
docker-compose down -v
```

## Environment Variables

### Nacos Tests
| Variable | Default | Description |
|----------|---------|-------------|
| `NACOS_SERVER` | `127.0.0.1:8848` | Batata server address |
| `NACOS_USERNAME` | `nacos` | Auth username |
| `NACOS_PASSWORD` | `nacos` | Auth password |

### Consul Tests
| Variable | Default | Description |
|----------|---------|-------------|
| `CONSUL_HTTP_ADDR` | `127.0.0.1:8848` | Batata server address |

## Test Categories

### P0 - Critical (Must Pass)
Core functionality that must work for basic SDK compatibility.

### P1 - Important (Should Pass)
Important features that most applications use.

### P2 - Nice to Have
Advanced features that are less commonly used.

## Test Coverage

### Nacos SDK (328 tests)
- **Config Service**: Publish, Get, Delete, Listen, Namespace, Group, History
- **Config Listeners**: Add/remove, Multiple listeners, Custom executors, Rapid changes
- **Batch Config**: Batch publish, Batch get, Batch delete, Large batch operations
- **Config Types**: Properties, YAML, JSON, XML, Text formats, Unicode support
- **Naming Service**: Register, Deregister, Discover, Subscribe, Weight, Metadata
- **Service Subscriptions**: Subscribe/Unsubscribe, Multiple subscribers, Cluster filter
- **Instance Selection**: Health filter, Cluster filter, Weighted selection
- **Service Selector**: Random, Round robin, Weighted, Cluster-based, Metadata filter
- **Health Check**: Health status, Healthy instance filter, TCP/HTTP checks, Beat mechanism
- **Metadata**: Instance metadata, Service metadata, Metadata filtering, Preservation
- **Batch Operations**: Batch register/deregister, Pagination
- **Cluster Management**: Multi-cluster instances, Cluster filtering, Concurrent ops
- **gRPC Handlers**: Config query/publish/remove, Naming handlers, Streaming
- **gRPC Error Handling**: Timeout, Permission, Connection recovery, Concurrent errors
- **Failover**: Local cache, Snapshot, Failover switch
- **V3 Console API**: Config, Service, Auth, Health, Namespace management
- **Connection Management**: Setup, Timeout, Health check, Retry, Pool, Keep-alive
- **Persistence**: Ephemeral vs persistent instances, Heartbeat, Auto-deregister
- **Weight & Load Balancing**: Weight distribution, Dynamic adjustment, Traffic routing
- **Server Status**: Health check, Cluster nodes, Metrics, Capacity, Switches
- **Namespace**: Isolation, CRUD operations, Migration, Cross-namespace access

### Consul SDK (355 tests)
- **Agent API**: Service Register/Deregister, Health Checks, Maintenance
- **Agent Connect**: Sidecar proxies, Multi-port, Gateways
- **Agent Metrics**: Gauges, Counters, Samples, Self info, Members, Reload
- **KV API**: Put, Get, Delete, List, CAS, Watch
- **KV Transactions**: Set, Get, Delete, CAS, Atomicity, Multi-op
- **Health API**: Service Health, Node Health, State queries, Filters
- **Catalog API**: Services, Nodes, Datacenters, Connect services
- **Catalog Filtering**: Tag filter, Expression filter, Pagination, Consistency modes
- **Session API**: Create, Destroy, Renew, List
- **ACL API**: Tokens, Policies, Roles CRUD
- **Lock API**: Distributed locking, Contention, One-shot
- **Semaphore API**: Resource limiting, Multi-holder
- **Connect API**: CA Roots, Intentions CRUD, Authorization
- **Operator API**: Raft configuration, Autopilot, Keyring, Leader transfer
- **Config Entries**: Service defaults, Routers, Splitters, Resolvers, Gateways
- **Snapshot API**: Backup, Restore, Stale reads
- **Prepared Queries**: Create, Update, Delete, Execute, Templates, Failover, DNS
- **Namespace/Partition**: Namespace CRUD, Partition CRUD, Cross-DC, Peering
- **Coordinate/Debug**: Datacenter coordinates, Node coordinates, Debug profiles, Raw API
- **Watch**: Key watch, Service watch, Health watch, Event watch, Connect watch
- **Service Mesh**: Proxies, Upstreams, Gateways, Routers, Splitters, Resolvers, mTLS
- **Discovery**: Lookup, Tags, Health filter, Datacenter, Blocking, Failover
- **Maintenance**: Node/service maintenance, Health impact, Discovery impact

## Compatibility Matrix

| Protocol | API | Status | Tests |
|----------|-----|--------|-------|
| Nacos V2 HTTP | Full | :white_check_mark: | 115+ |
| Nacos V3 Console | Full | :white_check_mark: | 25 |
| Nacos gRPC | Full | :white_check_mark: | 31 |
| Nacos Cluster | Full | :white_check_mark: | 15 |
| Nacos Naming | Full | :white_check_mark: | 48 |
| Nacos Connection | Full | :white_check_mark: | 15 |
| Consul Agent | Full | :white_check_mark: | 80 |
| Consul KV | Full | :white_check_mark: | 46 |
| Consul Health/Catalog | Full | :white_check_mark: | 71 |
| Consul Lock/Semaphore | Full | :white_check_mark: | 21 |
| Consul Connect | Full | :white_check_mark: | 28 |
| Consul Operator | Full | :white_check_mark: | 19 |
| Consul Config Entries | Full | :white_check_mark: | 17 |
| Consul Snapshot | Partial | :warning: | 9 |
| Consul Watch | Full | :white_check_mark: | 16 |
| Consul Service Mesh | Full | :white_check_mark: | 18 |

## Troubleshooting

### Connection Refused
Ensure Batata server is running and accessible at the configured address.

### Authentication Failed
Check that the Batata server has authentication enabled and credentials match.

### Timeout Errors
- Increase test timeouts for slow environments
- Check network connectivity between test container and Batata

### Missing Dependencies
```bash
# Nacos tests
cd nacos-java-tests && mvn dependency:resolve

# Consul tests
cd consul-go-tests && go mod download
```

## Contributing

1. Add new test cases following the existing pattern
2. Use appropriate priority (P0/P1/P2) based on feature importance
3. Include cleanup code to avoid test pollution
4. Document any Batata-specific behavior differences
