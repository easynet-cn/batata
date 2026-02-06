# Batata Integration Test Plan

> Comprehensive testing plan for Batata project - Phase 1: Internal Tests

---

## Status Legend

| Status | Icon | Description |
|--------|------|-------------|
| Pending | :white_large_square: | Task not yet started |
| In Progress | :arrows_counterclockwise: | Task is being worked on |
| Complete | :white_check_mark: | Task is complete |
| Blocked | :no_entry: | Task is blocked |

---

## Overview

### Current Test Status

| Category | Endpoints/Handlers | Current Coverage | Target Coverage |
|----------|-------------------|------------------|-----------------|
| Nacos HTTP API (V2/V3) | 110 | 0% | 80% |
| gRPC Handlers | 31 | 0% | 70% |
| Consul API | 78+ | ~5% (KV only) | 60% |
| Apollo API | 30+ | ~10% (mapping only) | 60% |
| Database Entities | 17 tables | 0% | 70% |
| Unit Tests | - | :white_check_mark: 661+ | - |

### Implementation Phases

```
P0 (Infrastructure) --> P1 (Core API) --> P2 (gRPC) --> P3 (Database) --> P4 (Compatibility)
     2-3 days             3-4 days        3-4 days      3-4 days           2-3 days
```

---

## Phase 0: Test Infrastructure (P0)

> **Priority**: CRITICAL - All other tests depend on this

### P0.1 TestServer Helper

| Task ID | Description | Status | Start Date | End Date | Notes |
|---------|-------------|--------|------------|----------|-------|
| P0-001 | Create tests/common/mod.rs module structure | :white_check_mark: | 2026-02-06 | 2026-02-06 | Export all common utilities |
| P0-002 | Implement TestServer struct | :white_check_mark: | 2026-02-06 | 2026-02-06 | Start/stop server, health check |
| P0-003 | Implement TestServer::start() with random port | :white_check_mark: | 2026-02-06 | 2026-02-06 | Avoid port conflicts |
| P0-004 | Implement TestServer::start_with_db() | :white_check_mark: | 2026-02-06 | 2026-02-06 | Database integration |
| P0-005 | Implement graceful shutdown in Drop | :white_check_mark: | 2026-02-06 | 2026-02-06 | Clean resource release |

### P0.2 HTTP Test Client

| Task ID | Description | Status | Start Date | End Date | Notes |
|---------|-------------|--------|------------|----------|-------|
| P0-101 | Implement TestClient struct | :white_check_mark: | 2026-02-06 | 2026-02-06 | HTTP client wrapper |
| P0-102 | Implement login and token management | :white_check_mark: | 2026-02-06 | 2026-02-06 | JWT token handling |
| P0-103 | Implement GET/POST/PUT/DELETE helpers | :white_check_mark: | 2026-02-06 | 2026-02-06 | Generic request methods |
| P0-104 | Implement response parsing utilities | :white_check_mark: | 2026-02-06 | 2026-02-06 | JSON deserialization |

### P0.3 Database Test Infrastructure

| Task ID | Description | Status | Start Date | End Date | Notes |
|---------|-------------|--------|------------|----------|-------|
| P0-201 | Implement TestDatabase struct | :white_check_mark: | 2026-02-06 | 2026-02-06 | Database connection wrapper |
| P0-202 | Implement MySQL test database setup | :white_check_mark: | 2026-02-06 | 2026-02-06 | Schema migration |
| P0-203 | Implement PostgreSQL test database setup | :white_check_mark: | 2026-02-06 | 2026-02-06 | Schema migration |
| P0-204 | Implement database cleanup utilities | :white_check_mark: | 2026-02-06 | 2026-02-06 | Transaction rollback or truncate |

### P0.4 Docker Compose Test Environment

| Task ID | Description | Status | Start Date | End Date | Notes |
|---------|-------------|--------|------------|----------|-------|
| P0-301 | Create docker-compose.test.yml | :white_check_mark: | 2026-02-06 | 2026-02-06 | MySQL + PostgreSQL containers |
| P0-302 | Add health checks for containers | :white_check_mark: | 2026-02-06 | 2026-02-06 | Wait for ready state |
| P0-303 | Document test environment setup | :white_check_mark: | 2026-02-06 | 2026-02-06 | README instructions

---

## Phase 1: Core HTTP API Tests (P1)

> **Priority**: HIGH - Core functionality validation

### P1.1 Configuration API Tests

| Task ID | Description | Endpoint | Status | Start Date | End Date | Notes |
|---------|-------------|----------|--------|------------|----------|-------|
| P1-001 | Test publish config | `POST /nacos/v2/cs/config` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_publish_and_get_config |
| P1-002 | Test get config | `GET /nacos/v2/cs/config` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_publish_and_get_config |
| P1-003 | Test delete config | `DELETE /nacos/v2/cs/config` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_delete_config |
| P1-004 | Test config not found | `GET /nacos/v2/cs/config` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_get_config_not_found |
| P1-005 | Test config with namespace | All config endpoints | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_config_with_namespace |
| P1-006 | Test config parameter validation | All config endpoints | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_config_parameter_validation |
| P1-007 | Test config update (overwrite) | `POST /nacos/v2/cs/config` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_config_update |
| P1-008 | Test config MD5 verification | `GET /nacos/v2/cs/config` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_config_md5 |

### P1.2 Config History API Tests

| Task ID | Description | Endpoint | Status | Start Date | End Date | Notes |
|---------|-------------|----------|--------|------------|----------|-------|
| P1-101 | Test history list | `GET /nacos/v2/cs/history/list` | :white_check_mark: | 2026-02-06 | 2026-02-06 | Pagination |
| P1-102 | Test get history version | `GET /nacos/v2/cs/history` | :white_check_mark: | 2026-02-06 | 2026-02-06 | By nid |
| P1-103 | Test previous history | `GET /nacos/v2/cs/history/previous` | :white_check_mark: | 2026-02-06 | 2026-02-06 | Previous version |
| P1-104 | Test namespace configs | `GET /nacos/v2/cs/history/configs` | :white_check_mark: | 2026-02-06 | 2026-02-06 | All configs in ns |

### P1.3 Service Discovery API Tests

| Task ID | Description | Endpoint | Status | Start Date | End Date | Notes |
|---------|-------------|----------|--------|------------|----------|-------|
| P1-201 | Test register instance | `POST /nacos/v2/ns/instance` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_register_instance |
| P1-202 | Test deregister instance | `DELETE /nacos/v2/ns/instance` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_deregister_instance |
| P1-203 | Test update instance | `PUT /nacos/v2/ns/instance` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_update_instance |
| P1-204 | Test get instance | `GET /nacos/v2/ns/instance` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_get_instance |
| P1-205 | Test instance list | `GET /nacos/v2/ns/instance/list` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_get_instance_list |
| P1-206 | Test healthy only filter | `GET /nacos/v2/ns/instance/list` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_instance_list_healthy_only |
| P1-207 | Test batch update metadata | `PUT /nacos/v2/ns/instance/metadata/batch` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_batch_update_metadata |
| P1-208 | Test instance with weight | `POST /nacos/v2/ns/instance` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_instance_with_weight |

### P1.4 Service API Tests

| Task ID | Description | Endpoint | Status | Start Date | End Date | Notes |
|---------|-------------|----------|--------|------------|----------|-------|
| P1-301 | Test create service | `POST /nacos/v2/ns/service` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_create_service |
| P1-302 | Test delete service | `DELETE /nacos/v2/ns/service` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_delete_service |
| P1-303 | Test update service | `PUT /nacos/v2/ns/service` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_update_service |
| P1-304 | Test get service | `GET /nacos/v2/ns/service` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_get_service |
| P1-305 | Test service list | `GET /nacos/v2/ns/service/list` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_service_list |
| P1-306 | Test service not found | `GET /nacos/v2/ns/service` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_service_not_found |

### P1.5 Authentication API Tests

| Task ID | Description | Endpoint | Status | Start Date | End Date | Notes |
|---------|-------------|----------|--------|------------|----------|-------|
| P1-401 | Test login success | `POST /v3/auth/user/login` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_login_success |
| P1-402 | Test login failure | `POST /v3/auth/user/login` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_login_failure_wrong_password |
| P1-403 | Test token validation | Protected endpoints | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_token_validation |
| P1-404 | Test token expiration | Protected endpoints | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_access_without_token |
| P1-405 | Test create user | `POST /v3/auth/user` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_create_user |
| P1-406 | Test user list | `GET /v3/auth/users` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_user_list |
| P1-407 | Test create role | `POST /v3/auth/role` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_create_role |
| P1-408 | Test assign permission | `POST /v3/auth/permission` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_assign_permission |

### P1.6 Namespace API Tests

| Task ID | Description | Endpoint | Status | Start Date | End Date | Notes |
|---------|-------------|----------|--------|------------|----------|-------|
| P1-501 | Test create namespace | `POST /nacos/v2/console/namespace` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_create_namespace |
| P1-502 | Test namespace list | `GET /nacos/v2/console/namespace/list` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_namespace_list |
| P1-503 | Test get namespace | `GET /nacos/v2/console/namespace` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_get_namespace |
| P1-504 | Test update namespace | `PUT /nacos/v2/console/namespace` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_update_namespace |
| P1-505 | Test delete namespace | `DELETE /nacos/v2/console/namespace` | :white_check_mark: | 2026-02-06 | 2026-02-06 | test_delete_namespace |

---

## Phase 2: gRPC API Tests (P2)

> **Priority**: HIGH - SDK communication validation

### P2.1 Connection Handlers

| Task ID | Description | Handler | Status | Start Date | End Date | Notes |
|---------|-------------|---------|--------|------------|----------|-------|
| P2-001 | Test health check | HealthCheckHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | Basic health |
| P2-002 | Test server check | ServerCheckHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | Server status |
| P2-003 | Test connection setup | ConnectionSetupHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | Init connection |
| P2-004 | Test client detection | ClientDetectionHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | Client status |
| P2-005 | Test setup ack | SetupAckHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | Acknowledgment |

### P2.2 Config Handlers

| Task ID | Description | Handler | Status | Start Date | End Date | Notes |
|---------|-------------|---------|--------|------------|----------|-------|
| P2-101 | Test config query | ConfigQueryHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | Query config |
| P2-102 | Test config publish | ConfigPublishHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | Publish config |
| P2-103 | Test config remove | ConfigRemoveHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | Remove config |
| P2-104 | Test batch listen | ConfigBatchListenHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | Multiple configs |
| P2-105 | Test change notify | ConfigChangeNotifyHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | Push notification |

### P2.3 Naming Handlers

| Task ID | Description | Handler | Status | Start Date | End Date | Notes |
|---------|-------------|---------|--------|------------|----------|-------|
| P2-201 | Test instance request | InstanceRequestHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | Register/deregister |
| P2-202 | Test batch instance | BatchInstanceRequestHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | Batch operations |
| P2-203 | Test service list | ServiceListRequestHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | List services |
| P2-204 | Test service query | ServiceQueryRequestHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | Query detail |
| P2-205 | Test subscribe | SubscribeServiceRequestHandler | :white_check_mark: | 2026-02-06 | 2026-02-06 | Subscribe/unsubscribe |

### P2.4 Bidirectional Streaming

| Task ID | Description | Status | Start Date | End Date | Notes |
|---------|-------------|--------|------------|----------|-------|
| P2-301 | Test stream lifecycle | :white_check_mark: | 2026-02-06 | 2026-02-06 | Connect -> messages -> close |
| P2-302 | Test multiple messages | :white_check_mark: | 2026-02-06 | 2026-02-06 | Send/receive sequence |
| P2-303 | Test stream error handling | :white_check_mark: | 2026-02-06 | 2026-02-06 | Error recovery |
| P2-304 | Test concurrent streams | :white_check_mark: | 2026-02-06 | 2026-02-06 | Multiple clients |

---

## Phase 3: Database Integration Tests (P3)

> **Priority**: MEDIUM - Persistence validation

### P3.1 Configuration Entities

| Task ID | Description | Entity | Status | Start Date | End Date | Notes |
|---------|-------------|--------|--------|------------|----------|-------|
| P3-001 | Test config_info CRUD | config_info | :white_check_mark: | 2026-02-06 | 2026-02-06 | MySQL |
| P3-002 | Test config_info CRUD | config_info | :white_check_mark: | 2026-02-06 | 2026-02-06 | PostgreSQL |
| P3-003 | Test unique constraint | config_info | :white_check_mark: | 2026-02-06 | 2026-02-06 | (dataId, group, tenant) |
| P3-004 | Test his_config_info insert | his_config_info | :white_check_mark: | 2026-02-06 | 2026-02-06 | History tracking |
| P3-005 | Test config_tags_relation | config_tags_relation | :white_check_mark: | 2026-02-06 | 2026-02-06 | Tag associations |
| P3-006 | Test config_info_aggr | config_info_aggr | :white_check_mark: | 2026-02-06 | 2026-02-06 | Aggregate configs |

### P3.2 Service Discovery Entities

| Task ID | Description | Entity | Status | Start Date | End Date | Notes |
|---------|-------------|--------|--------|------------|----------|-------|
| P3-101 | Test service_info CRUD | service_info | :white_check_mark: | 2026-02-06 | 2026-02-06 | Basic ops |
| P3-102 | Test cluster_info with FK | cluster_info | :white_check_mark: | 2026-02-06 | 2026-02-06 | Foreign key |
| P3-103 | Test instance_info with FK | instance_info | :white_check_mark: | 2026-02-06 | 2026-02-06 | Foreign key |
| P3-104 | Test cascade delete | service_info | :white_check_mark: | 2026-02-06 | 2026-02-06 | Delete service -> clusters -> instances |
| P3-105 | Test relationship loading | All | :white_check_mark: | 2026-02-06 | 2026-02-06 | Eager/lazy loading |

### P3.3 Authentication Entities

| Task ID | Description | Entity | Status | Start Date | End Date | Notes |
|---------|-------------|--------|--------|------------|----------|-------|
| P3-201 | Test users CRUD | users | :white_check_mark: | 2026-02-06 | 2026-02-06 | Username as PK |
| P3-202 | Test roles assignment | roles | :white_check_mark: | 2026-02-06 | 2026-02-06 | User-role mapping |
| P3-203 | Test permissions CRUD | permissions | :white_check_mark: | 2026-02-06 | 2026-02-06 | Role permissions |
| P3-204 | Test multi-role user | roles | :white_check_mark: | 2026-02-06 | 2026-02-06 | Multiple roles |

### P3.4 Consul ACL Entities

| Task ID | Description | Entity | Status | Start Date | End Date | Notes |
|---------|-------------|--------|--------|------------|----------|-------|
| P3-301 | Test ACL token CRUD | consul_acl_tokens | :white_check_mark: | 2026-02-06 | 2026-02-06 | Token management |
| P3-302 | Test ACL policy CRUD | consul_acl_policies | :white_check_mark: | 2026-02-06 | 2026-02-06 | Policy management |
| P3-303 | Test unique constraints | Both | :white_check_mark: | 2026-02-06 | 2026-02-06 | accessor_id, policy_id |

---

## Phase 4: Compatibility Layer Tests (P4)

> **Priority**: MEDIUM - External SDK compatibility

### P4.1 Consul API Tests

| Task ID | Description | Endpoint | Status | Start Date | End Date | Notes |
|---------|-------------|----------|--------|------------|----------|-------|
| P4-001 | Test service register | `PUT /v1/agent/service/register` | :white_check_mark: | 2026-02-06 | 2026-02-06 | Agent API |
| P4-002 | Test service deregister | `PUT /v1/agent/service/deregister/{id}` | :white_check_mark: | 2026-02-06 | 2026-02-06 | Agent API |
| P4-003 | Test list services | `GET /v1/agent/services` | :white_check_mark: | 2026-02-06 | 2026-02-06 | Agent API |
| P4-004 | Test health check | `GET /v1/health/service/{service}` | :white_check_mark: | 2026-02-06 | 2026-02-06 | Health API |
| P4-005 | Test KV put | `PUT /v1/kv/{key}` | :white_check_mark: | 2026-02-06 | 2026-02-06 | KV API |
| P4-006 | Test KV get | `GET /v1/kv/{key}` | :white_check_mark: | 2026-02-06 | 2026-02-06 | KV API |
| P4-007 | Test KV delete | `DELETE /v1/kv/{key}` | :white_check_mark: | 2026-02-06 | 2026-02-06 | KV API |
| P4-008 | Test KV CAS | `PUT /v1/kv/{key}?cas=` | :white_check_mark: | 2026-02-06 | 2026-02-06 | Compare-and-swap |
| P4-009 | Test catalog services | `GET /v1/catalog/services` | :white_check_mark: | 2026-02-06 | 2026-02-06 | Catalog API |
| P4-010 | Test ACL token | `PUT /v1/acl/token` | :white_check_mark: | 2026-02-06 | 2026-02-06 | ACL API |

### P4.2 Apollo API Tests

| Task ID | Description | Endpoint | Status | Start Date | End Date | Notes |
|---------|-------------|----------|--------|------------|----------|-------|
| P4-101 | Test get config | `GET /configs/{appId}/{cluster}/{ns}` | :white_check_mark: | 2026-02-06 | 2026-02-06 | Config API |
| P4-102 | Test config files | `GET /configfiles/{appId}/{cluster}/{ns}` | :white_check_mark: | 2026-02-06 | 2026-02-06 | Plain text |
| P4-103 | Test config JSON | `GET /configfiles/json/{appId}/{cluster}/{ns}` | :white_check_mark: | 2026-02-06 | 2026-02-06 | JSON format |
| P4-104 | Test notifications | `GET /notifications/v2` | :white_check_mark: | 2026-02-06 | 2026-02-06 | Long polling |
| P4-105 | Test release key | `GET /configs/...` | :white_check_mark: | 2026-02-06 | 2026-02-06 | 304 Not Modified |
| P4-106 | Test OpenAPI apps | `GET /openapi/v1/apps` | :white_check_mark: | 2026-02-06 | 2026-02-06 | OpenAPI |
| P4-107 | Test OpenAPI items | `GET /openapi/v1/.../items` | :white_check_mark: | 2026-02-06 | 2026-02-06 | Config items |
| P4-108 | Test OpenAPI publish | `POST /openapi/v1/.../releases` | :white_check_mark: | 2026-02-06 | 2026-02-06 | Publish release |

---

## Test File Structure

```
crates/batata-server/tests/
|-- common/
|   |-- mod.rs              # P0: Public exports
|   |-- server.rs           # P0: TestServer
|   |-- client.rs           # P0: TestClient
|   +-- db.rs               # P0: TestDatabase
|-- http_api/
|   |-- mod.rs
|   |-- config_api_test.rs      # P1.1, P1.2
|   |-- naming_api_test.rs      # P1.3, P1.4
|   |-- auth_api_test.rs        # P1.5
|   +-- namespace_api_test.rs   # P1.6
|-- grpc_api/
|   |-- mod.rs
|   |-- connection_test.rs      # P2.1
|   |-- config_handler_test.rs  # P2.2
|   |-- naming_handler_test.rs  # P2.3
|   +-- stream_test.rs          # P2.4
|-- persistence/
|   |-- mod.rs
|   |-- config_test.rs          # P3.1
|   |-- naming_test.rs          # P3.2
|   |-- auth_test.rs            # P3.3
|   +-- consul_acl_test.rs      # P3.4
+-- compatibility/
    |-- consul_api_test.rs      # P4.1
    +-- apollo_api_test.rs      # P4.2

docker-compose.test.yml         # P0.4: Test containers
```

---

## Statistics Overview

| Phase | Total Tasks | Complete | In Progress | Pending | Completion Rate |
|-------|-------------|----------|-------------|---------|-----------------|
| P0 (Infrastructure) | 17 | 17 | 0 | 0 | 100% |
| P1 (Core HTTP API) | 37 | 37 | 0 | 0 | 100% |
| P2 (gRPC API) | 19 | 19 | 0 | 0 | 100% |
| P3 (Database) | 17 | 17 | 0 | 0 | 100% |
| P4 (Compatibility) | 18 | 18 | 0 | 0 | 100% |
| **Total** | **108** | **108** | **0** | **0** | **100%** |

---

## Estimated Timeline

| Phase | Tasks | Estimated Duration | Dependencies |
|-------|-------|-------------------|--------------|
| P0 | 17 | 2-3 days | None |
| P1 | 37 | 3-4 days | P0 |
| P2 | 19 | 3-4 days | P0 |
| P3 | 17 | 3-4 days | P0 |
| P4 | 18 | 2-3 days | P0, P1 |
| **Total** | **108** | **13-18 days** | |

---

## Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-02-06 | Initial test plan document created | Claude |
| 2026-02-06 | **P0 Complete**: Test infrastructure (TestServer, TestClient, TestDatabase, docker-compose.test.yml) | Claude |
| 2026-02-06 | **P1 Complete**: All Core HTTP API tests (Config, History, Naming, Service, Auth, Namespace - 37 test cases) | Claude |
| 2026-02-06 | **P2 Complete**: gRPC API tests (Connection, Config, Naming handlers, Streaming - 19 test cases) | Claude |
| 2026-02-06 | **P3 Complete**: Database persistence tests (Config, Service, Auth, Consul ACL entities - 17 test cases) | Claude |
| 2026-02-06 | **P4 Complete**: Compatibility layer tests (Consul API, Apollo API - 18 test cases) | Claude |
| 2026-02-06 | **Phase 1 COMPLETE**: All 108 internal test cases created and ready for execution | Claude |

---

## Next Steps

1. ~~**Start P0**: Build test infrastructure~~ :white_check_mark: COMPLETE
2. ~~**P1**: Core HTTP API tests~~ :white_check_mark: COMPLETE
3. ~~**P2**: gRPC API tests~~ :white_check_mark: COMPLETE
4. ~~**P3**: Database persistence tests~~ :white_check_mark: COMPLETE
5. ~~**P4**: Compatibility layer tests~~ :white_check_mark: COMPLETE
6. **Run Tests**: Execute tests with running Batata server
7. **Phase 2**: Official SDK compatibility tests (Nacos Java, Consul Go, Apollo Java)

---

## Phase 2: Official SDK Compatibility Tests

> After Phase 1 is complete, use official SDKs for compatibility validation

| SDK | Language | Version | Test Cases |
|-----|----------|---------|------------|
| nacos-client | Java | 2.3.2 | 70 tests |
| consul/api | Go | 1.28.2 | 75 tests |
| apollo-client | Java | 2.2.0 | 42 tests |

**Total: 187 SDK compatibility test cases** :white_check_mark:

See [SDK_COMPATIBILITY_TEST_PLAN.md](./SDK_COMPATIBILITY_TEST_PLAN.md) for detailed test plan.

### Test Project Location

```
sdk-tests/
├── nacos-java-tests/          # Nacos Java SDK (Maven)
├── consul-go-tests/           # Consul Go SDK (Go modules)
├── apollo-java-tests/         # Apollo Java SDK (Maven)
├── docker-compose.yml         # Test environment
└── README.md                  # Setup instructions
```

### Quick Start

```bash
# Start Batata server
cargo run -p batata-server

# Run Nacos tests
cd sdk-tests/nacos-java-tests && mvn test

# Run Consul tests
cd sdk-tests/consul-go-tests && go test -v ./...

# Run Apollo tests
cd sdk-tests/apollo-java-tests && mvn test
```
