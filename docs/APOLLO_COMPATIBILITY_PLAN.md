# Apollo Config Compatibility Implementation Plan

This document outlines the plan to implement Apollo Config client compatibility in Batata.

**Created**: 2026-02-04
**Status**: In Progress
**Target Version**: v2.6.0

## Overview

Apollo Config is a distributed configuration management system developed by Ctrip. By implementing Apollo client API compatibility, Batata can serve as a drop-in replacement for Apollo Config Server, allowing existing Apollo clients to connect without modification.

## Goals

1. Support Apollo Java/Go/.NET client SDK connections
2. Implement Config Service API for configuration retrieval
3. Implement notification long-polling for real-time updates
4. Provide concept mapping between Apollo and Nacos models

## Non-Goals (Phase 1)

1. Apollo Portal UI compatibility
2. Release approval workflow
3. Apollo Admin Service API (full implementation)

## Architecture

### Concept Mapping

| Apollo Concept | Nacos Concept | Mapping Strategy |
|---------------|---------------|------------------|
| `env` | `namespace` | Direct mapping |
| `appId` | Prefix in `dataId` | `{appId}+{namespace}` → `dataId` |
| `cluster` | `group` | Direct mapping |
| `namespace` | Part of `dataId` | `{appId}+{namespace}` → `dataId` |
| `releaseKey` | `md5` | Generate Apollo-style releaseKey |

### API Endpoints

#### Config Service API (Client SDK)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/configs/{appId}/{clusterName}/{namespace}` | GET | Get configuration |
| `/configfiles/{appId}/{clusterName}/{namespace}` | GET | Get config as plain text |
| `/configfiles/json/{appId}/{clusterName}/{namespace}` | GET | Get config as JSON |
| `/notifications/v2` | GET | Long polling for updates |

#### Open API (Management - Phase 2)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/openapi/v1/apps` | GET/POST | App management |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces` | GET | Namespace list |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items` | GET/POST/PUT/DELETE | Config items |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/releases` | POST | Publish config |

### Crate Structure

```
crates/batata-plugin-apollo/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # Crate entry point
│   ├── api/
│   │   ├── mod.rs
│   │   ├── config.rs          # /configs endpoint
│   │   ├── configfiles.rs     # /configfiles endpoint
│   │   ├── notification.rs    # /notifications/v2 endpoint
│   │   └── openapi.rs         # Open API endpoints (Phase 2)
│   ├── model/
│   │   ├── mod.rs
│   │   ├── apollo_config.rs   # ApolloConfig response
│   │   ├── notification.rs    # ApolloConfigNotification
│   │   └── request.rs         # Request parameters
│   ├── service/
│   │   ├── mod.rs
│   │   ├── config_service.rs  # Config retrieval logic
│   │   └── notification_service.rs # Long polling logic
│   └── mapping.rs             # Apollo <-> Nacos mapping
```

## Implementation Phases

### Phase 1: Core Client API (Priority: High)

**Goal**: Enable Apollo clients to fetch configurations and receive updates.

1. Create `batata-plugin-apollo` crate
2. Implement `/configs/{appId}/{cluster}/{namespace}` endpoint
3. Implement `/configfiles/*` endpoints
4. Implement `/notifications/v2` long polling
5. Implement concept mapping layer
6. Wire into HTTP server

### Phase 2: Open API (Priority: Medium)

**Goal**: Enable configuration management via Apollo Open API.

1. Implement app management endpoints
2. Implement namespace management endpoints
3. Implement configuration CRUD endpoints
4. Implement release/publish endpoints

### Phase 3: Advanced Features (Priority: Low)

**Goal**: Full Apollo feature compatibility.

1. Namespace lock mechanism
2. Gray release support
3. Access key authentication
4. Metrics and monitoring

## Testing Strategy

1. **Unit Tests**: Test mapping functions and response formatting
2. **Integration Tests**: Test with official Apollo Java client
3. **Compatibility Tests**: Verify against Apollo server responses

## References

- [Apollo Config GitHub](https://github.com/apolloconfig/apollo)
- [Apollo Open API Documentation](https://www.apolloconfig.com/#/en/portal/apollo-open-api-platform)
- [Apollo Client SDK](https://github.com/apolloconfig/apollo-java)
