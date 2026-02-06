# Batata SDK Compatibility Tests

This directory contains integration tests using official client SDKs from Nacos, Consul, and Apollo projects to validate Batata's API compatibility.

## Test Structure

```
sdk-tests/
├── nacos-java-tests/          # Nacos Java SDK tests (70 tests)
│   ├── pom.xml
│   └── src/test/java/io/batata/tests/
│       ├── NacosConfigServiceTest.java      # Config CRUD, listeners
│       ├── NacosNamingServiceTest.java      # Service discovery
│       ├── NacosGrpcTest.java               # gRPC handlers
│       ├── NacosAdminApiTest.java           # Namespace, cluster, capacity
│       ├── NacosConfigHistoryTest.java      # History tracking
│       └── NacosAggregateConfigTest.java    # Aggregate configs
├── consul-go-tests/           # Consul Go SDK tests (75 tests)
│   ├── go.mod
│   ├── agent_test.go                        # Basic agent API
│   ├── agent_advanced_test.go               # Advanced agent features
│   ├── kv_test.go                           # KV store operations
│   ├── health_test.go                       # Health API
│   ├── catalog_test.go                      # Catalog API
│   ├── session_test.go                      # Session/lock API
│   ├── acl_test.go                          # ACL tokens, policies, roles
│   ├── event_test.go                        # Event API
│   ├── query_test.go                        # Prepared queries
│   └── status_test.go                       # Cluster status
├── apollo-java-tests/         # Apollo Java SDK tests (42 tests)
│   ├── pom.xml
│   └── src/test/java/io/batata/tests/
│       ├── ApolloConfigTest.java            # Config SDK
│       ├── ApolloOpenApiTest.java           # OpenAPI CRUD
│       └── ApolloAdvancedApiTest.java       # Gray release, locks, keys
├── docker-compose.yml         # Test environment
└── README.md                  # This file

Total: 187 SDK compatibility tests
```

## Prerequisites

- Java 17+ (for Nacos and Apollo tests)
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

4. Run Apollo Java tests:
```bash
cd sdk-tests/apollo-java-tests
mvn test
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
docker-compose --profile tests up apollo-tests

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

### Apollo Tests
| Variable | Default | Description |
|----------|---------|-------------|
| `APOLLO_META` | `http://127.0.0.1:8848` | Apollo meta service URL |
| `APOLLO_CONFIG_SERVICE` | `http://127.0.0.1:8848` | Config service URL |
| `APOLLO_APP_ID` | `test-app` | Application ID |
| `APOLLO_ENV` | `DEV` | Environment |
| `APOLLO_CLUSTER` | `default` | Cluster name |

## Test Categories

### P0 - Critical (Must Pass)
Core functionality that must work for basic SDK compatibility.

### P1 - Important (Should Pass)
Important features that most applications use.

### P2 - Nice to Have
Advanced features that are less commonly used.

## Test Coverage

### Nacos SDK
- **Config Service**: Publish, Get, Delete, Listen, Namespace, Group
- **Naming Service**: Register, Deregister, Discover, Subscribe, Weight, Metadata

### Consul SDK
- **Agent API**: Service Register/Deregister, Health Checks
- **KV API**: Put, Get, Delete, List, CAS, Locks
- **Health API**: Service Health, Node Health
- **Catalog API**: Services, Datacenters

### Apollo SDK
- **Config API**: Get Config, Properties, Listeners
- **OpenAPI**: Apps, Items, Releases, Namespaces

## Compatibility Matrix

| Protocol | API | Status |
|----------|-----|--------|
| Nacos V2 HTTP | Full | :white_check_mark: |
| Nacos gRPC | Full | :white_check_mark: |
| Consul Agent | Full | :white_check_mark: |
| Consul KV | Full | :white_check_mark: |
| Consul Health | Partial | :warning: |
| Apollo Config | Full | :white_check_mark: |
| Apollo OpenAPI | Partial | :warning: |

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

# Apollo tests
cd apollo-java-tests && mvn dependency:resolve
```

## Contributing

1. Add new test cases following the existing pattern
2. Use appropriate priority (P0/P1/P2) based on feature importance
3. Include cleanup code to avoid test pollution
4. Document any Batata-specific behavior differences
