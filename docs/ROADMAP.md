# Batata Roadmap

> Batata project roadmap - Implementation plan to achieve Nacos 3.x feature parity

## Version Planning

### v1.0.0 - Core Functionality (Current)

**Goal**: Complete implementation of Nacos 2.x core features

| Module | Status | Completion |
|--------|--------|------------|
| Configuration Management | ✅ Complete | 100% |
| Service Discovery | ✅ Complete | 100% |
| Namespace | ✅ Complete | 100% |
| Authentication (JWT/RBAC) | ✅ Complete | 100% |
| gRPC Communication | ✅ Complete | 100% |
| Raft Protocol | ✅ Complete | 100% |
| Distro Protocol | ✅ Complete | 100% |
| Consul Compatibility | ✅ Complete | 100% |
| V3 Console API | ✅ Complete | 100% |
| Fuzzy Watch | ✅ Complete | 100% |

---

### v1.1.0 - API Enhancement

**Goal**: Implement Nacos V2 Open API and Admin API

| Feature | Priority | Status | Owner | ETA |
|---------|----------|--------|-------|-----|
| V2 Config API (`/nacos/v2/cs/*`) | P0 | 🔲 Pending | - | - |
| V2 Naming API (`/nacos/v2/ns/*`) | P0 | 🔲 Pending | - | - |
| V2 Client API (`/nacos/v2/ns/client/*`) | P1 | 🔲 Pending | - | - |
| V2 Operator API (`/nacos/v2/ns/operator/*`) | P1 | 🔲 Pending | - | - |
| V2 Cluster API (`/nacos/v2/core/cluster/*`) | P1 | 🔲 Pending | - | - |
| Admin API (`/nacos/v2/admin/*`) | P1 | 🔲 Pending | - | - |

---

### v1.2.0 - Security Enhancement

**Goal**: Enterprise-grade security features

| Feature | Priority | Status | Owner | ETA |
|---------|----------|--------|-------|-----|
| LDAP Authentication | P0 | 🔲 Pending | - | - |
| gRPC SSL/TLS | P0 | 🔲 Pending | - | - |
| Encryption Plugin System | P1 | 🔲 Pending | - | - |
| Audit Log Enhancement | P2 | 🔲 Pending | - | - |

---

### v2.0.0 - Service Mesh Support

**Goal**: Implement Nacos 3.0 core new features

| Feature | Priority | Status | Owner | ETA |
|---------|----------|--------|-------|-----|
| xDS Protocol - EDS | P0 | 🔲 Pending | - | - |
| xDS Protocol - LDS | P0 | 🔲 Pending | - | - |
| xDS Protocol - RDS | P1 | 🔲 Pending | - | - |
| xDS Protocol - CDS | P1 | 🔲 Pending | - | - |
| Istio MCP Server | P2 | 🔲 Pending | - | - |

---

### v2.1.0 - AI Capabilities

**Goal**: Implement Nacos 3.0/3.1 AI-related features

| Feature | Priority | Status | Owner | ETA |
|---------|----------|--------|-------|-----|
| MCP (Model Content Protocol) Management | P0 | 🔲 Pending | - | - |
| MCP Multi-Namespace Management | P1 | 🔲 Pending | - | - |
| MCP Multi-Version Management | P1 | 🔲 Pending | - | - |
| MCP Server Import | P1 | 🔲 Pending | - | - |
| A2A (Agent-to-Agent) Registry | P0 | 🔲 Pending | - | - |
| Agent Endpoint Batch Registration | P1 | 🔲 Pending | - | - |

---

### v2.2.0 - Cloud Native Integration

**Goal**: Kubernetes and cloud-native ecosystem integration

| Feature | Priority | Status | Owner | ETA |
|---------|----------|--------|-------|-----|
| Kubernetes Service Sync | P0 | 🔲 Pending | - | - |
| Prometheus Service Discovery | P1 | 🔲 Pending | - | - |
| Helm Chart | P1 | 🔲 Pending | - | - |
| Kubernetes Operator | P2 | 🔲 Pending | - | - |

---

### v2.3.0 - Plugin Ecosystem

**Goal**: Complete plugin framework and built-in plugins

| Feature | Priority | Status | Owner | ETA |
|---------|----------|--------|-------|-----|
| Control Plugin (Rate Limiting/Circuit Breaker) | P0 | 🔲 Pending | - | - |
| Webhook Plugin | P1 | 🔲 Pending | - | - |
| Config Change Plugin | P1 | 🔲 Pending | - | - |
| CMDB Plugin | P2 | 🔲 Pending | - | - |
| Custom Data Source Plugin | P2 | 🔲 Pending | - | - |

---

### v2.4.0 - Advanced Features

**Goal**: Experimental and advanced features

| Feature | Priority | Status | Owner | ETA |
|---------|----------|--------|-------|-----|
| Distributed Lock | P1 | 🔲 Pending | - | - |
| Multi-Datacenter Enhancement | P1 | 🔲 Pending | - | - |
| Config Gray Release Enhancement | P2 | 🔲 Pending | - | - |

---

## Milestones

```
2024 Q1   2024 Q2   2024 Q3   2024 Q4   2025 Q1   2025 Q2
   |---------|---------|---------|---------|---------|
   v1.0      v1.1      v1.2      v2.0      v2.1      v2.2+
   Core      API       Security  Mesh      AI        Cloud
```

---

## Contribution Guide

### How to Claim a Task

1. Check available tasks in [TASK_TRACKER.md](./TASK_TRACKER.md)
2. Create an Issue for the task
3. Update task status to "In Progress" and fill in the owner
4. Submit a PR when complete and update task status

### Priority Definitions

- **P0**: Core functionality, must implement
- **P1**: Important functionality, should implement
- **P2**: Optional functionality, implement if time permits

### Status Definitions

- 🔲 Pending
- 🔄 In Progress
- ✅ Complete
- ⏸️ Paused
- ❌ Cancelled

---

## References

- [Nacos 3.0.0 Release Notes](https://github.com/alibaba/nacos/releases/tag/3.0.0)
- [Nacos 3.1.1 Release Notes](https://github.com/alibaba/nacos/releases/tag/3.1.1)
- [Nacos Open API Documentation](https://nacos.io/docs/latest/guide/user/open-api/)
- [Nacos xDS Support](https://nacos.io/docs/latest/ecology/use-nacos-with-istio/)
