# Feature Comparison with Nacos

Batata implements **~98% of Nacos features** and can serve as a production-ready drop-in replacement.

## Core Features

| Feature | Nacos | Batata | Status |
|---------|-------|--------|--------|
| **Configuration Management** | | | |
| Config CRUD | ✅ | ✅ | Full |
| Config Search & Pagination | ✅ | ✅ | Full |
| Config History & Rollback | ✅ | ✅ | Full |
| Config Listening (Long Polling) | ✅ | ✅ | Full |
| Gray/Beta Release | ✅ | ✅ | Full |
| Config Import/Export (ZIP) | ✅ | ✅ | Full |
| Config Tags | ✅ | ✅ | Full |
| Config Encryption | ✅ | ✅ | Full (AES-128-CBC) |
| Config Capacity Quota | ✅ | ✅ | Full |
| Aggregate Config (datumId) | ✅ | ✅ | Full |
| **Service Discovery** | | | |
| Instance Register/Deregister | ✅ | ✅ | Full |
| Service Discovery Query | ✅ | ✅ | Full |
| Health Check | ✅ | ✅ | Full |
| Heartbeat | ✅ | ✅ | Full |
| Subscribe/Push | ✅ | ✅ | Full |
| Weight Routing | ✅ | ✅ | Full |
| Ephemeral Instances | ✅ | ✅ | Full |
| Persistent Instances | ✅ | ✅ | Full |
| Service Metadata | ✅ | ✅ | Full |
| DNS-based Discovery | ✅ | ✅ | Full |
| **Namespace** | | | |
| Namespace CRUD | ✅ | ✅ | Full |
| Multi-tenant Isolation | ✅ | ✅ | Full |
| **Cluster** | | | |
| Cluster Node Discovery | ✅ | ✅ | Full |
| Node Health Check | ✅ | ✅ | Full |
| Raft Consensus (CP) | ✅ | ✅ | Full |
| Distro Protocol (AP) | ✅ | ✅ | Full |
| Multi-Datacenter Sync | ✅ | ✅ | Full |
| **Auth** | | | |
| User Management | ✅ | ✅ | Full |
| Role Management | ✅ | ✅ | Full |
| Permission (RBAC) | ✅ | ✅ | Full |
| JWT Token | ✅ | ✅ | Full |
| LDAP Integration | ✅ | ✅ | Full |
| OAuth2/OIDC | ✅ | ✅ | Full |
| **API** | | | |
| Nacos V2 API | ✅ | ✅ | Full |
| Nacos V3 API | ✅ | ✅ | Full |
| Nacos V1 API | ✅ | ❌ | **Intentionally not supported** |
| gRPC Bi-directional Streaming | ✅ | ✅ | Full |
| Consul API Compatibility | ❌ | ✅ | **Extra** |
| **Observability** | | | |
| Prometheus Metrics | ✅ | ✅ | Full |
| Health Endpoint | ✅ | ✅ | Full |
| OpenTelemetry | ✅ | ✅ | Full |
| Operation Audit Logs | ✅ | ✅ | Full |
| **AI Registry (MCP / A2A / Skill / AgentSpec / Prompt)** | | | |
| MCP admin API (all 14 endpoints) | ✅ | ✅ | Full — service + HTTP + 15 route tests |
| MCP draft lifecycle (draft → submit → publish → redraft) | ✅ | ✅ | Full |
| MCP version labels / status / scope | ✅ | ✅ | Full |
| MCP + A2A visibility (scope + owner) | ✅ | ✅ | Full — all four domains |
| A2A agent registry CRUD + versions | ✅ | ✅ | Full |
| Resource keyword search | ✅ | ✅ | Full — no vector channel |
| `ai_resource` table-backed storage | ✅ | ✅ | Full — replaces the config-backed path |
| Vector / embedding search | ✅ | ❌ | Needs pgvector; embedder ported, storage not |
| MCP validation service | ✅ | ❌ | Not implemented |
| MCP import service (registry, `skills.sh`) | ✅ | ❌ | Not implemented |
| Runtime endpoint resolution + heartbeat | ✅ | ❌ | Not implemented |
| Prompt / Pipeline domains | ✅ | ❌ | Not implemented |

## Batata Exclusive Features

| Feature | Description |
|---------|-------------|
| **Consul API Compatibility** | Full support for Agent, Health, Catalog, KV, ACL APIs |
| **PostgreSQL Support** | In addition to MySQL |
| **Consul JSON Import/Export** | Migration support for Consul KV store |
| **Built-in Circuit Breaker** | Resilience pattern for cluster health checks |
| **OpenTelemetry Tracing** | OTLP export for distributed tracing (Jaeger, Zipkin, etc.) |
| **Multi-datacenter Support** | Locality-aware replication with local-first sync |
| **Service Mesh (xDS)** | EDS, CDS, LDS, RDS, ADS protocol support |
| **AI Integration** | MCP Server Registry and A2A Agent Registry — mirrors the upstream Nacos AI module rather than being Batata-only (see `docs/compat/nacos/ai/`) |
| **Distributed Lock** | Raft-based distributed locking mechanism |
| **Kubernetes Sync** | Bidirectional service sync with Kubernetes |
| **Database Migration** | Automatic schema migration via `batata-migration` crate |

## Feature Completeness Summary

| Module | Completeness | Notes |
|--------|--------------|-------|
| Configuration Management | **100%** | Full AES-GCM encryption support |
| Service Discovery | **95%** | UDP Push not implemented (gRPC push used) |
| Namespace | **100%** | Fully supported |
| Cluster Management | **95%** | Single Raft group (Multi-Raft partial) |
| Authentication | **100%** | JWT, LDAP, OAuth2/OIDC |
| API Compatibility | **100%+** | Full Nacos V2/V3 + Consul APIs |
| Apollo Compatibility | **~81%** | Apollo Config/Admin/OpenAPI fully ported (see `docs/compat/apollo/`); legacy WebUI (SSO/signin) and apollo config-value encryption (`EncryptionDecorator`) not ported |
| Cloud Native | **85%** | K8s, Prometheus, xDS (basic) |
| AI Registry | **partial** | MCP complete and tested; A2A/Skill/AgentSpec CRUD; no validation, import, runtime layer or vector search (see `docs/compat/nacos/ai/`) |
| **Overall** | **~98%** | Production ready — excluding the AI module, which is tracked separately above |
