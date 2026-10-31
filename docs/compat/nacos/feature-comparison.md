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
| MCP admin API (all 19 endpoints) | ✅ | ✅ | Full — service + HTTP + 7 route tests |
| MCP console API (all 22 endpoints) | ✅ | ✅ | Full, plus `/stats` and `/search` |
| MCP draft lifecycle (draft → submit → publish → redraft) | ✅ | ✅ | Full |
| MCP version labels / status / scope | ✅ | ✅ | Full |
| MCP + A2A visibility (scope + owner) | ✅ | ✅ | Full — all four domains |
| A2A agent registry CRUD + versions | ✅ | ✅ | Full |
| Resource keyword search | ✅ | ✅ | Full — client API, projection, task queue and consumer |
| `ai_resource` table-backed storage | ✅ | ✅ | Full — replaces the config-backed path |
| MCP validation service | ✅ | ✅ | `batata_common::model::ai::mcp_validation` |
| MCP import service | ✅ | ✅ | `/import/validate` + `/import/execute`; unified importer fetches the MCP Registry for real. `skills.sh` / well-known fetchers ❌ |
| Runtime endpoint resolution + heartbeat | ✅ | ⚠️ | RAD client endpoints + heartbeat + version ranges; `McpClientController`'s 6 endpoints ❌ |
| Prompt / Pipeline domains | ✅ | ⚠️ | Prompt 23/24 admin + 18/18 console; Pipeline 2/4 on both |
| Vector / embedding search | ✅ | ❌ | Needs pgvector; embedder ported, storage not |

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
| AI Registry | **~91%** | 226 of 249 upstream AI endpoints matched (23 missing, 10 Batata-only extras). MCP admin/console, Skill, AgentSpec, A2A and resource import are at parity; open: `McpClientController` (6), Agent console lifecycle (8), Agent admin `/version` + `/runtime-endpoints`, Prompt admin `/version`, Pipeline `/{pipelineId}` + bare `GET`, vector channel (see `docs/compat/nacos/ai/`) |
| **Overall** | **~98%** | Production ready — excluding the AI module, which is tracked separately above |
