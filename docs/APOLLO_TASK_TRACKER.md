# Apollo Compatibility Task Tracker

> Task tracking for Apollo Config client compatibility implementation

**Last Updated**: 2026-02-04

---

## Status Legend

| Status | Icon | Description |
|--------|------|-------------|
| Pending | :white_large_square: | Task not yet started |
| In Progress | :arrows_counterclockwise: | Task is being worked on |
| Complete | :white_check_mark: | Task is complete |
| Blocked | :no_entry: | Task is blocked |

---

## Phase 1: Core Client API (v2.6.0)

### 1.1 Crate Setup

| Task ID | Description | Status | Start Date | End Date | Notes |
|---------|-------------|--------|------------|----------|-------|
| APO-001 | Create batata-plugin-apollo crate | :white_check_mark: | 2026-02-04 | 2026-02-04 | Cargo.toml, lib.rs |
| APO-002 | Define Apollo model types | :white_check_mark: | 2026-02-04 | 2026-02-04 | ApolloConfig, ApolloConfigNotification |
| APO-003 | Implement concept mapping module | :white_check_mark: | 2026-02-04 | 2026-02-04 | Apollo <-> Nacos mapping |

### 1.2 Config Service API

| Task ID | Description | Endpoint | Status | Start Date | End Date | Notes |
|---------|-------------|----------|--------|------------|----------|-------|
| APO-101 | Get configuration | `GET /configs/{appId}/{cluster}/{namespace}` | :white_check_mark: | 2026-02-04 | 2026-02-04 | Core config retrieval |
| APO-102 | Get config as properties | `GET /configfiles/{appId}/{cluster}/{namespace}` | :white_check_mark: | 2026-02-04 | 2026-02-04 | Plain text format |
| APO-103 | Get config as JSON | `GET /configfiles/json/{appId}/{cluster}/{namespace}` | :white_check_mark: | 2026-02-04 | 2026-02-04 | JSON format |
| APO-104 | Config response with releaseKey | - | :white_check_mark: | 2026-02-04 | 2026-02-04 | 304 Not Modified support |

### 1.3 Notification API

| Task ID | Description | Endpoint | Status | Start Date | End Date | Notes |
|---------|-------------|----------|--------|------------|----------|-------|
| APO-201 | Long polling notification | `GET /notifications/v2` | :white_check_mark: | 2026-02-04 | 2026-02-04 | HTTP long polling |
| APO-202 | Notification service | - | :white_check_mark: | 2026-02-04 | 2026-02-04 | Broadcast channel |
| APO-203 | Notification ID tracking | - | :white_check_mark: | 2026-02-04 | 2026-02-04 | Track notificationId per namespace |

### 1.4 Integration

| Task ID | Description | Status | Start Date | End Date | Notes |
|---------|-------------|--------|------------|----------|-------|
| APO-301 | Wire Apollo routes to HTTP server | :white_check_mark: | 2026-02-04 | 2026-02-04 | startup/http.rs |
| APO-302 | Add to workspace Cargo.toml | :white_check_mark: | 2026-02-04 | 2026-02-04 | Workspace member |
| APO-303 | Update ARCHITECTURE.md | :white_check_mark: | 2026-02-04 | 2026-02-04 | Document new crate |

---

## Phase 2: Open API (v2.7.0)

### 2.1 App Management

| Task ID | Description | Endpoint | Status | Start Date | End Date | Notes |
|---------|-------------|----------|--------|------------|----------|-------|
| APO-401 | Get apps | `GET /openapi/v1/apps` | :white_check_mark: | 2026-02-04 | 2026-02-04 | Extract from dataId |
| APO-402 | Create app | `POST /openapi/v1/apps` | :white_large_square: | - | - | Not needed (auto-created) |
| APO-403 | Get env clusters | `GET /openapi/v1/apps/{appId}/envclusters` | :white_check_mark: | 2026-02-04 | 2026-02-04 | List namespaces+groups |

### 2.2 Namespace Management

| Task ID | Description | Endpoint | Status | Start Date | End Date | Notes |
|---------|-------------|----------|--------|------------|----------|-------|
| APO-501 | List namespaces | `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces` | :white_check_mark: | 2026-02-04 | 2026-02-04 | - |
| APO-502 | Get namespace | `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}` | :white_check_mark: | 2026-02-04 | 2026-02-04 | - |
| APO-503 | Create namespace | `POST /openapi/v1/apps/{appId}/appnamespaces` | :white_large_square: | - | - | Not needed (auto-created) |

### 2.3 Configuration Items

| Task ID | Description | Endpoint | Status | Start Date | End Date | Notes |
|---------|-------------|----------|--------|------------|----------|-------|
| APO-601 | Get item | `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items/{key}` | :white_check_mark: | 2026-02-04 | 2026-02-04 | - |
| APO-602 | Create item | `POST /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items` | :white_check_mark: | 2026-02-04 | 2026-02-04 | - |
| APO-603 | Update item | `PUT /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items/{key}` | :white_check_mark: | 2026-02-04 | 2026-02-04 | - |
| APO-604 | Delete item | `DELETE /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items/{key}` | :white_check_mark: | 2026-02-04 | 2026-02-04 | - |
| APO-605 | List items | `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items` | :white_check_mark: | 2026-02-04 | 2026-02-04 | - |

### 2.4 Release Management

| Task ID | Description | Endpoint | Status | Start Date | End Date | Notes |
|---------|-------------|----------|--------|------------|----------|-------|
| APO-701 | Publish config | `POST /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/releases` | :white_check_mark: | 2026-02-04 | 2026-02-04 | - |
| APO-702 | Get latest release | `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/releases/latest` | :white_check_mark: | 2026-02-04 | 2026-02-04 | - |
| APO-703 | Rollback release | `PUT /openapi/v1/envs/{env}/releases/{releaseId}/rollback` | :white_large_square: | - | - | Future enhancement |

---

## Phase 3: Advanced Features (v2.8.0)

| Task ID | Description | Status | Start Date | End Date | Notes |
|---------|-------------|--------|------------|----------|-------|
| APO-801 | Namespace lock | :white_large_square: | - | - | Prevent concurrent edits |
| APO-802 | Gray release | :white_large_square: | - | - | Gradual rollout |
| APO-803 | Access key auth | :white_large_square: | - | - | Signature verification |
| APO-804 | Client metrics | :white_large_square: | - | - | Connection tracking |

---

## Statistics

| Phase | Total Tasks | Complete | In Progress | Pending | Completion Rate |
|-------|-------------|----------|-------------|---------|-----------------|
| Phase 1 (Core) | 13 | 13 | 0 | 0 | 100% |
| Phase 2 (Open API) | 13 | 10 | 0 | 3 | 77% |
| Phase 3 (Advanced) | 4 | 0 | 0 | 4 | 0% |
| **Total** | **30** | **23** | **0** | **7** | **77%** |

---

## Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-02-04 | Created Apollo task tracker | Claude |
| 2026-02-04 | Completed Phase 1: Core Client API (13 tasks) | Claude |
| 2026-02-04 | Completed Phase 2: Open API (10 tasks) | Claude |

---

## How to Update This Document

1. **Start a task**: Change status to :arrows_counterclockwise:, fill in start date
2. **Complete a task**: Change status to :white_check_mark:, fill in end date
3. **Add notes**: Add important information in the notes column
4. **Update statistics**: Update the numbers in the statistics section
5. **Record changes**: Add a record in the change log
