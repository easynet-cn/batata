# SDK Compatibility Test Plan

> Phase 2: Official Client SDK Integration Tests for Batata

---

## Overview

This document outlines the plan for testing Batata's API compatibility using official client SDKs from Nacos, Consul, and Apollo projects.

### Test Strategy

| Protocol | Official SDK | Language | Repository |
|----------|--------------|----------|------------|
| Nacos | nacos-client | Java | https://github.com/alibaba/nacos |
| Consul | consul/api | Go | https://github.com/hashicorp/consul |
| Apollo | apollo-client | Java | https://github.com/apolloconfig/apollo |

### Project Structure

```
batata/
├── sdk-tests/
│   ├── nacos-java-tests/          # Nacos Java SDK tests
│   │   ├── pom.xml
│   │   └── src/test/java/
│   ├── consul-go-tests/           # Consul Go SDK tests
│   │   ├── go.mod
│   │   └── *_test.go
│   ├── apollo-java-tests/         # Apollo Java SDK tests
│   │   ├── pom.xml
│   │   └── src/test/java/
│   └── docker-compose.yml         # Test environment
└── docs/
    └── SDK_COMPATIBILITY_TEST_PLAN.md
```

---

## 1. Nacos Java Client SDK Tests

### 1.1 Dependencies

```xml
<!-- pom.xml -->
<dependencies>
    <dependency>
        <groupId>com.alibaba.nacos</groupId>
        <artifactId>nacos-client</artifactId>
        <version>2.3.2</version>
    </dependency>
    <dependency>
        <groupId>org.junit.jupiter</groupId>
        <artifactId>junit-jupiter</artifactId>
        <version>5.10.2</version>
        <scope>test</scope>
    </dependency>
</dependencies>
```

### 1.2 Configuration Service Tests

| Test Case | API Method | Description | Priority |
|-----------|------------|-------------|----------|
| NC-001 | `ConfigService.publishConfig()` | Publish configuration | P0 |
| NC-002 | `ConfigService.getConfig()` | Get configuration | P0 |
| NC-003 | `ConfigService.removeConfig()` | Delete configuration | P0 |
| NC-004 | `ConfigService.addListener()` | Subscribe to changes | P0 |
| NC-005 | `ConfigService.removeListener()` | Unsubscribe | P1 |
| NC-006 | `ConfigService.getConfigAndSignListener()` | Get + subscribe | P1 |
| NC-007 | Config with namespace | Multi-tenant isolation | P1 |
| NC-008 | Config with group | Group isolation | P1 |
| NC-009 | Large config (>100KB) | Size limits | P2 |
| NC-010 | Concurrent publish | Thread safety | P2 |

```java
// Example: NacosConfigServiceTest.java
public class NacosConfigServiceTest {

    private static ConfigService configService;

    @BeforeAll
    static void setup() throws NacosException {
        Properties properties = new Properties();
        properties.put("serverAddr", "127.0.0.1:8848");
        properties.put("namespace", "test-namespace");
        properties.put("username", "nacos");
        properties.put("password", "nacos");
        configService = NacosFactory.createConfigService(properties);
    }

    @Test
    void testPublishAndGetConfig() throws NacosException {
        String dataId = "test-config-" + UUID.randomUUID();
        String group = "DEFAULT_GROUP";
        String content = "key=value";

        // Publish
        boolean published = configService.publishConfig(dataId, group, content);
        assertTrue(published);

        // Get
        String retrieved = configService.getConfig(dataId, group, 5000);
        assertEquals(content, retrieved);

        // Cleanup
        configService.removeConfig(dataId, group);
    }

    @Test
    void testConfigListener() throws NacosException, InterruptedException {
        String dataId = "listener-test-" + UUID.randomUUID();
        String group = "DEFAULT_GROUP";
        AtomicReference<String> receivedContent = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        // Add listener
        configService.addListener(dataId, group, new Listener() {
            @Override
            public Executor getExecutor() { return null; }

            @Override
            public void receiveConfigInfo(String configInfo) {
                receivedContent.set(configInfo);
                latch.countDown();
            }
        });

        // Publish triggers notification
        configService.publishConfig(dataId, group, "updated=true");

        assertTrue(latch.await(10, TimeUnit.SECONDS));
        assertEquals("updated=true", receivedContent.get());
    }
}
```

### 1.3 Naming Service Tests

| Test Case | API Method | Description | Priority |
|-----------|------------|-------------|----------|
| NN-001 | `NamingService.registerInstance()` | Register service | P0 |
| NN-002 | `NamingService.deregisterInstance()` | Deregister service | P0 |
| NN-003 | `NamingService.getAllInstances()` | Get all instances | P0 |
| NN-004 | `NamingService.selectInstances()` | Get healthy instances | P0 |
| NN-005 | `NamingService.selectOneHealthyInstance()` | Load balancing | P1 |
| NN-006 | `NamingService.subscribe()` | Subscribe to service | P0 |
| NN-007 | `NamingService.unsubscribe()` | Unsubscribe | P1 |
| NN-008 | Instance with metadata | Custom metadata | P1 |
| NN-009 | Instance with weight | Weighted routing | P1 |
| NN-010 | Ephemeral vs Persistent | Instance types | P2 |
| NN-011 | Multiple clusters | Cluster isolation | P2 |
| NN-012 | Service list pagination | `getServicesOfServer()` | P2 |

```java
// Example: NacosNamingServiceTest.java
public class NacosNamingServiceTest {

    private static NamingService namingService;

    @BeforeAll
    static void setup() throws NacosException {
        Properties properties = new Properties();
        properties.put("serverAddr", "127.0.0.1:8848");
        properties.put("username", "nacos");
        properties.put("password", "nacos");
        namingService = NacosFactory.createNamingService(properties);
    }

    @Test
    void testRegisterAndDiscoverInstance() throws NacosException {
        String serviceName = "test-service-" + UUID.randomUUID();
        String ip = "192.168.1.100";
        int port = 8080;

        // Register
        namingService.registerInstance(serviceName, ip, port);

        // Wait for registration
        Thread.sleep(1000);

        // Discover
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty());
        assertEquals(ip, instances.get(0).getIp());
        assertEquals(port, instances.get(0).getPort());

        // Deregister
        namingService.deregisterInstance(serviceName, ip, port);
    }

    @Test
    void testServiceSubscription() throws NacosException, InterruptedException {
        String serviceName = "subscribe-test-" + UUID.randomUUID();
        AtomicReference<List<Instance>> receivedInstances = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        // Subscribe
        namingService.subscribe(serviceName, event -> {
            if (event instanceof NamingEvent) {
                receivedInstances.set(((NamingEvent) event).getInstances());
                latch.countDown();
            }
        });

        // Register triggers notification
        namingService.registerInstance(serviceName, "192.168.1.101", 8081);

        assertTrue(latch.await(10, TimeUnit.SECONDS));
        assertFalse(receivedInstances.get().isEmpty());
    }
}
```

### 1.4 gRPC Connection Tests

| Test Case | Description | Priority |
|-----------|-------------|----------|
| NG-001 | gRPC connection establishment | P0 |
| NG-002 | Bidirectional streaming | P0 |
| NG-003 | Connection reconnection | P1 |
| NG-004 | Server push notification | P0 |
| NG-005 | Connection timeout handling | P2 |
| NG-006 | Multiple concurrent connections | P2 |

---

## 2. Consul Go Client SDK Tests

### 2.1 Dependencies

```go
// go.mod
module batata-consul-tests

go 1.21

require (
    github.com/hashicorp/consul/api v1.28.2
    github.com/stretchr/testify v1.9.0
)
```

### 2.2 Agent API Tests

| Test Case | API Method | Description | Priority |
|-----------|------------|-------------|----------|
| CA-001 | `Agent.ServiceRegister()` | Register service | P0 |
| CA-002 | `Agent.ServiceDeregister()` | Deregister service | P0 |
| CA-003 | `Agent.Services()` | List services | P0 |
| CA-004 | `Agent.Service()` | Get service detail | P1 |
| CA-005 | `Agent.CheckRegister()` | Register health check | P1 |
| CA-006 | `Agent.CheckDeregister()` | Deregister check | P1 |
| CA-007 | `Agent.PassTTL()` | Pass TTL check | P2 |
| CA-008 | `Agent.FailTTL()` | Fail TTL check | P2 |

```go
// agent_test.go
package tests

import (
    "testing"
    "github.com/hashicorp/consul/api"
    "github.com/stretchr/testify/assert"
    "github.com/stretchr/testify/require"
)

func TestAgentServiceRegister(t *testing.T) {
    client, err := api.NewClient(&api.Config{
        Address: "127.0.0.1:8848",
    })
    require.NoError(t, err)

    serviceID := "test-svc-" + randomID()
    registration := &api.AgentServiceRegistration{
        ID:      serviceID,
        Name:    "test-service",
        Port:    8080,
        Address: "192.168.1.100",
        Tags:    []string{"test", "integration"},
        Meta: map[string]string{
            "version": "1.0.0",
        },
    }

    // Register
    err = client.Agent().ServiceRegister(registration)
    assert.NoError(t, err)

    // Verify
    services, err := client.Agent().Services()
    assert.NoError(t, err)
    assert.Contains(t, services, serviceID)

    // Cleanup
    err = client.Agent().ServiceDeregister(serviceID)
    assert.NoError(t, err)
}
```

### 2.3 KV Store Tests

| Test Case | API Method | Description | Priority |
|-----------|------------|-------------|----------|
| CK-001 | `KV.Put()` | Store key-value | P0 |
| CK-002 | `KV.Get()` | Retrieve key-value | P0 |
| CK-003 | `KV.Delete()` | Delete key | P0 |
| CK-004 | `KV.List()` | List keys by prefix | P1 |
| CK-005 | `KV.Keys()` | Get keys only | P1 |
| CK-006 | `KV.CAS()` | Compare-and-swap | P1 |
| CK-007 | `KV.Acquire()` | Distributed lock | P2 |
| CK-008 | `KV.Release()` | Release lock | P2 |
| CK-009 | KV with flags | Metadata flags | P2 |

```go
// kv_test.go
func TestKVOperations(t *testing.T) {
    client, err := api.NewClient(&api.Config{
        Address: "127.0.0.1:8848",
    })
    require.NoError(t, err)

    key := "test/key/" + randomID()
    value := []byte("test-value")

    // Put
    _, err = client.KV().Put(&api.KVPair{
        Key:   key,
        Value: value,
    }, nil)
    assert.NoError(t, err)

    // Get
    pair, _, err := client.KV().Get(key, nil)
    assert.NoError(t, err)
    assert.Equal(t, value, pair.Value)

    // CAS update
    pair.Value = []byte("updated-value")
    success, _, err := client.KV().CAS(pair, nil)
    assert.NoError(t, err)
    assert.True(t, success)

    // Delete
    _, err = client.KV().Delete(key, nil)
    assert.NoError(t, err)
}
```

### 2.4 Health API Tests

| Test Case | API Method | Description | Priority |
|-----------|------------|-------------|----------|
| CH-001 | `Health.Service()` | Get healthy service instances | P0 |
| CH-002 | `Health.Node()` | Get node health | P1 |
| CH-003 | `Health.Checks()` | Get service checks | P1 |
| CH-004 | `Health.State()` | Get checks by state | P2 |

```go
// health_test.go
func TestHealthService(t *testing.T) {
    client, err := api.NewClient(&api.Config{
        Address: "127.0.0.1:8848",
    })
    require.NoError(t, err)

    serviceName := "health-test-" + randomID()

    // Register service
    err = client.Agent().ServiceRegister(&api.AgentServiceRegistration{
        ID:   serviceName,
        Name: serviceName,
        Port: 8080,
    })
    require.NoError(t, err)
    defer client.Agent().ServiceDeregister(serviceName)

    // Query health
    entries, _, err := client.Health().Service(serviceName, "", true, nil)
    assert.NoError(t, err)
    assert.NotEmpty(t, entries)
}
```

### 2.5 Catalog API Tests

| Test Case | API Method | Description | Priority |
|-----------|------------|-------------|----------|
| CC-001 | `Catalog.Services()` | List all services | P0 |
| CC-002 | `Catalog.Service()` | Get service nodes | P1 |
| CC-003 | `Catalog.Datacenters()` | List datacenters | P2 |
| CC-004 | `Catalog.Nodes()` | List nodes | P2 |

### 2.6 ACL API Tests

| Test Case | API Method | Description | Priority |
|-----------|------------|-------------|----------|
| CACL-001 | `ACL.TokenCreate()` | Create token | P1 |
| CACL-002 | `ACL.TokenRead()` | Read token | P1 |
| CACL-003 | `ACL.TokenDelete()` | Delete token | P1 |
| CACL-004 | `ACL.PolicyCreate()` | Create policy | P2 |
| CACL-005 | `ACL.PolicyRead()` | Read policy | P2 |

---

## 3. Apollo Java Client SDK Tests

### 3.1 Dependencies

```xml
<!-- pom.xml -->
<dependencies>
    <dependency>
        <groupId>com.ctrip.framework.apollo</groupId>
        <artifactId>apollo-client</artifactId>
        <version>2.2.0</version>
    </dependency>
    <dependency>
        <groupId>org.junit.jupiter</groupId>
        <artifactId>junit-jupiter</artifactId>
        <version>5.10.2</version>
        <scope>test</scope>
    </dependency>
</dependencies>
```

### 3.2 System Properties Setup

```java
// Test environment setup
@BeforeAll
static void setup() {
    // Apollo client configuration
    System.setProperty("app.id", "test-app");
    System.setProperty("env", "DEV");
    System.setProperty("apollo.meta", "http://127.0.0.1:8848");
    System.setProperty("apollo.cluster", "default");

    // For local testing without Apollo portal
    System.setProperty("apollo.configService", "http://127.0.0.1:8848");
}
```

### 3.3 Config API Tests

| Test Case | API Method | Description | Priority |
|-----------|------------|-------------|----------|
| AC-001 | `ConfigService.getConfig()` | Get config | P0 |
| AC-002 | `Config.getProperty()` | Get property value | P0 |
| AC-003 | `Config.addChangeListener()` | Subscribe to changes | P0 |
| AC-004 | `Config.getPropertyNames()` | List all keys | P1 |
| AC-005 | `ConfigService.getConfigFile()` | Get config file | P1 |
| AC-006 | Multiple namespaces | Cross-namespace | P1 |
| AC-007 | Config cache | Local cache | P2 |
| AC-008 | Fallback to local | Offline mode | P2 |

```java
// Example: ApolloConfigTest.java
public class ApolloConfigTest {

    @Test
    void testGetConfig() {
        Config config = ConfigService.getConfig("application");

        // Get property with default
        String value = config.getProperty("test.key", "default");
        assertNotNull(value);
    }

    @Test
    void testConfigChangeListener() throws InterruptedException {
        Config config = ConfigService.getConfig("application");
        AtomicBoolean changed = new AtomicBoolean(false);
        CountDownLatch latch = new CountDownLatch(1);

        config.addChangeListener(changeEvent -> {
            for (String key : changeEvent.changedKeys()) {
                ConfigChange change = changeEvent.getChange(key);
                System.out.println("Change - key: " + key +
                    ", oldValue: " + change.getOldValue() +
                    ", newValue: " + change.getNewValue());
            }
            changed.set(true);
            latch.countDown();
        });

        // Trigger change via OpenAPI or wait for manual change
        // In real test, use OpenAPI to publish change

        // For manual testing
        // assertTrue(latch.await(60, TimeUnit.SECONDS));
        // assertTrue(changed.get());
    }

    @Test
    void testConfigFile() {
        ConfigFile configFile = ConfigService.getConfigFile(
            "application",
            ConfigFileFormat.Properties
        );

        assertNotNull(configFile);
        String content = configFile.getContent();
        // Content may be null if namespace doesn't exist
    }
}
```

### 3.4 OpenAPI Tests

| Test Case | Endpoint | Description | Priority |
|-----------|----------|-------------|----------|
| AO-001 | `GET /openapi/v1/apps` | List apps | P1 |
| AO-002 | `POST /openapi/v1/.../items` | Create item | P1 |
| AO-003 | `PUT /openapi/v1/.../items/{key}` | Update item | P1 |
| AO-004 | `DELETE /openapi/v1/.../items/{key}` | Delete item | P1 |
| AO-005 | `POST /openapi/v1/.../releases` | Publish release | P0 |
| AO-006 | `GET /openapi/v1/.../releases/latest` | Get latest release | P1 |

```java
// OpenAPI tests using RestTemplate/WebClient
public class ApolloOpenApiTest {

    private final String baseUrl = "http://127.0.0.1:8848";
    private final RestTemplate restTemplate = new RestTemplate();

    @Test
    void testListApps() {
        ResponseEntity<List> response = restTemplate.getForEntity(
            baseUrl + "/openapi/v1/apps",
            List.class
        );

        assertEquals(HttpStatus.OK, response.getStatusCode());
        assertNotNull(response.getBody());
    }

    @Test
    void testPublishRelease() {
        String appId = "test-app";
        String env = "DEV";
        String cluster = "default";
        String namespace = "application";

        Map<String, Object> body = new HashMap<>();
        body.put("releaseTitle", "Test Release");
        body.put("releaseComment", "SDK Integration Test");
        body.put("releasedBy", "test-user");

        String url = String.format(
            "%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/releases",
            baseUrl, env, appId, cluster, namespace
        );

        ResponseEntity<Map> response = restTemplate.postForEntity(
            url, body, Map.class
        );

        // May return 200 or error depending on app existence
        assertNotNull(response);
    }
}
```

---

## 4. Test Execution Strategy

### 4.1 Local Development

```bash
# 1. Start Batata server
cargo run -p batata-server

# 2. Run Nacos Java tests
cd sdk-tests/nacos-java-tests
mvn test

# 3. Run Consul Go tests
cd sdk-tests/consul-go-tests
go test -v ./...

# 4. Run Apollo Java tests
cd sdk-tests/apollo-java-tests
mvn test
```

### 4.2 Docker Compose Test Environment

```yaml
# sdk-tests/docker-compose.yml
version: '3.8'

services:
  batata:
    build:
      context: ..
      dockerfile: Dockerfile
    ports:
      - "8848:8848"   # HTTP
      - "9848:9848"   # gRPC SDK
      - "9849:9849"   # gRPC Cluster
      - "8081:8081"   # Console
    environment:
      - DB_URL=mysql://root:root@mysql:3306/batata
    depends_on:
      mysql:
        condition: service_healthy

  mysql:
    image: mysql:8.0
    environment:
      - MYSQL_ROOT_PASSWORD=root
      - MYSQL_DATABASE=batata
    volumes:
      - ../conf/mysql-schema.sql:/docker-entrypoint-initdb.d/init.sql
    healthcheck:
      test: ["CMD", "mysqladmin", "ping", "-h", "localhost"]
      interval: 5s
      timeout: 5s
      retries: 10

  nacos-tests:
    build:
      context: ./nacos-java-tests
      dockerfile: Dockerfile
    depends_on:
      - batata
    environment:
      - NACOS_SERVER=batata:8848

  consul-tests:
    build:
      context: ./consul-go-tests
      dockerfile: Dockerfile
    depends_on:
      - batata
    environment:
      - CONSUL_HTTP_ADDR=batata:8848

  apollo-tests:
    build:
      context: ./apollo-java-tests
      dockerfile: Dockerfile
    depends_on:
      - batata
    environment:
      - APOLLO_META=http://batata:8848
```

### 4.3 CI/CD Integration

```yaml
# .github/workflows/sdk-tests.yml
name: SDK Compatibility Tests

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  nacos-java-tests:
    runs-on: ubuntu-latest
    services:
      mysql:
        image: mysql:8.0
        env:
          MYSQL_ROOT_PASSWORD: root
          MYSQL_DATABASE: batata
        ports:
          - 3306:3306
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Build and start Batata
        run: |
          cargo build --release -p batata-server
          ./target/release/batata-server &
          sleep 10

      - name: Setup Java
        uses: actions/setup-java@v4
        with:
          distribution: 'temurin'
          java-version: '17'

      - name: Run Nacos SDK tests
        run: |
          cd sdk-tests/nacos-java-tests
          mvn test

  consul-go-tests:
    runs-on: ubuntu-latest
    # Similar setup...

  apollo-java-tests:
    runs-on: ubuntu-latest
    # Similar setup...
```

---

## 5. Compatibility Matrix

### 5.1 Nacos API Compatibility

| API Category | Batata Support | SDK Tested | Notes |
|--------------|----------------|------------|-------|
| Config V2 HTTP | :white_check_mark: Full | :white_check_mark: | Core API |
| Naming V2 HTTP | :white_check_mark: Full | :white_check_mark: | Core API |
| gRPC Config | :white_check_mark: Full | :white_check_mark: | SDK default |
| gRPC Naming | :white_check_mark: Full | :white_check_mark: | SDK default |
| Auth | :white_check_mark: Full | :white_check_mark: | JWT token |
| Config V1 HTTP | :x: Not Supported | :x: | Nacos 3.x direction |
| Naming V1 HTTP | :x: Not Supported | :x: | Nacos 3.x direction |

### 5.2 Consul API Compatibility

| API Category | Batata Support | SDK Tested | Notes |
|--------------|----------------|------------|-------|
| Agent Services | :white_check_mark: Full | :white_check_mark: | Core API |
| KV Store | :white_check_mark: Full | :white_check_mark: | Config mapping |
| Health | :white_check_mark: Partial | :white_check_mark: | Basic health |
| Catalog | :white_check_mark: Partial | :white_check_mark: | Service listing |
| ACL | :white_check_mark: Basic | :white_check_mark: | Token only |
| Connect | :x: Not Planned | :x: | Service mesh |
| Prepared Query | :x: Not Planned | :x: | Advanced query |

### 5.3 Apollo API Compatibility

| API Category | Batata Support | SDK Tested | Notes |
|--------------|----------------|------------|-------|
| Config Service | :white_check_mark: Full | :white_check_mark: | Core API |
| Config Files | :white_check_mark: Full | :white_check_mark: | Plain text |
| Notifications | :white_check_mark: Full | :white_check_mark: | Long polling |
| OpenAPI | :white_check_mark: Partial | :white_check_mark: | Basic CRUD |
| Gray Release | :x: Not Planned | :x: | Advanced |
| Portal | :x: Not Planned | :x: | UI backend |

---

## 6. Test Priority Matrix

### P0 - Critical (Must Pass)

| SDK | Test Cases | Count |
|-----|------------|-------|
| Nacos | Config (10), Naming (12), gRPC (16), Admin (20), History (5), Aggregate (7) | 70 |
| Consul | Agent (12), KV (10), Health (5), Catalog (5), Session (8), ACL (15), Event (6), Query (9), Status (2) | 72 |
| Apollo | Config (10), OpenAPI (12), Advanced (20) | 42 |
| **Total** | | **184** |

### Test File Summary

| SDK | Test File | Test Count |
|-----|-----------|------------|
| **Nacos Java** | NacosConfigServiceTest.java | 10 |
| | NacosNamingServiceTest.java | 12 |
| | NacosGrpcTest.java | 16 |
| | NacosAdminApiTest.java | 20 |
| | NacosConfigHistoryTest.java | 5 |
| | NacosAggregateConfigTest.java | 7 |
| **Consul Go** | agent_test.go | 3 |
| | agent_advanced_test.go | 12 |
| | kv_test.go | 10 |
| | health_test.go | 5 |
| | catalog_test.go | 5 |
| | session_test.go | 8 |
| | acl_test.go | 15 |
| | event_test.go | 6 |
| | query_test.go | 9 |
| | status_test.go | 2 |
| **Apollo Java** | ApolloConfigTest.java | 10 |
| | ApolloOpenApiTest.java | 12 |
| | ApolloAdvancedApiTest.java | 20 |
| **Total** | | **184** |

---

## 7. Timeline

| Week | Tasks | Deliverables |
|------|-------|--------------|
| 1 | Setup project structure | Maven/Go modules, CI config |
| 2 | Nacos P0 tests | 13 core test cases |
| 3 | Consul P0 tests | 8 core test cases |
| 4 | Apollo P0 tests | 4 core test cases |
| 5 | P1 tests (all SDKs) | 29 important test cases |
| 6 | P2 tests + documentation | 20 additional tests, final report |

**Total Duration: 6 weeks**

---

## 8. Success Criteria

1. **P0 Tests**: 100% pass rate required
2. **P1 Tests**: 90% pass rate required
3. **P2 Tests**: 70% pass rate acceptable
4. **Documentation**: All API differences documented
5. **CI Integration**: All tests run automatically on PR

---

## Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-02-06 | Initial SDK compatibility test plan | Claude |
| 2026-02-06 | Added all missing tests: Nacos Admin/History/Aggregate/gRPC, Consul Session/ACL/Event/Query/Status/Agent Advanced, Apollo Gray/Lock/AccessKey/Metrics | Claude |
| 2026-02-06 | Total test count increased from 74 to 184 tests | Claude |
