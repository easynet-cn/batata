# Consul Test Case Inventory

Upstream baseline: **Hashicorp Consul 2.1.0-dev** (local `~/work/github/easynet-cn/consul`).

> Purpose: map upstream Consul test cases to batata F-IDs for compatibility tracking.
> Test types: `HTTP_API` (agent endpoint tests), `SDK_CLIENT` (Go SDK `api/` package tests), `CLI` (command tests).
> Internal RPC tests (`agent/consul/*_test.go`) are excluded — they test Consul's internal Go RPC layer, not the HTTP contract.
> gRPC tests (`agent/grpc-external/`) are excluded — gRPC/8502 is out of scope for batata's Consul compatibility.

Status: `✅ pass` | `⏳ pending` | `❌ fail` | `⏭️ skip` (feature ⚪/⛔)

---

## 1. Agent — SDK client tests (`api/agent_test.go`)

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-AGNT-001-01 | F-CON-AGNT-001 | api/agent_test.go | TestAPI_AgentSelf | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-001-02 | F-CON-AGNT-001 | api/agent_test.go | TestAPI_NewClient_TokenFileCLIFirstPriority | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-004-01 | F-CON-AGNT-004 | api/agent_test.go | TestAPI_AgentMetrics | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-002-01 | F-CON-AGNT-002 | api/agent_test.go | TestAPI_AgentHost | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-024-01 | F-CON-AGNT-024 | api/agent_test.go | TestAPI_AgentReload | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-018-01 | F-CON-AGNT-018 | api/agent_test.go | TestAPI_AgentMembers | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-018-02 | F-CON-AGNT-018 | api/agent_test.go | TestAPI_AgentMembersOpts | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-006-01 | F-CON-AGNT-006 | api/agent_test.go | TestAPI_AgentServices | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-006-02 | F-CON-AGNT-006 | api/agent_test.go | TestAPI_AgentServicesWithFilterOpts | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-007-01 | F-CON-AGNT-007 | api/agent_test.go | TestAPI_AgentService | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-007-02 | F-CON-AGNT-007 | api/agent_test.go | TestAPI_AgentServiceAddress | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-010-01 | F-CON-AGNT-010 | api/agent_test.go | TestAPI_AgentServiceAndReplaceChecks | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-010-02 | F-CON-AGNT-010 | api/agent_test.go | TestAgent_ServiceRegisterOpts_WithContextTimeout | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-010-03 | F-CON-AGNT-010 | api/agent_test.go | TestAPI_AgentEnableTagOverride | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-005-01 | F-CON-AGNT-005 | api/agent_test.go | TestAPI_AgentChecks | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-005-02 | F-CON-AGNT-005 | api/agent_test.go | TestAPI_AgentChecksWithFilterOpts | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-012-01 | F-CON-AGNT-012 | api/agent_test.go | TestAPI_AgentScriptCheck | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-012-02 | F-CON-AGNT-012 | api/agent_test.go | TestAPI_AgentCheckStartPassing | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-017-01 | F-CON-AGNT-017 | api/agent_test.go | TestAPI_AgentUpdateTTLOpts | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-017-02 | F-CON-AGNT-017 | api/agent_test.go | TestAPI_AgentSetTTLStatus | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-019-01 | F-CON-AGNT-019 | api/agent_test.go | TestAPI_AgentJoin | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-020-01 | F-CON-AGNT-020 | api/agent_test.go | TestAPI_AgentLeave | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-021-01 | F-CON-AGNT-021 | api/agent_test.go | TestAPI_AgentForceLeave | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-021-02 | F-CON-AGNT-021 | api/agent_test.go | TestAPI_AgentForceLeavePrune | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-026-01 | F-CON-AGNT-026 | api/agent_test.go | TestAPI_AgentMonitor | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-026-02 | F-CON-AGNT-026 | api/agent_test.go | TestAPI_AgentMonitorJSON | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-022-01 | F-CON-AGNT-022 | api/agent_test.go | TestAPI_ServiceMaintenanceOpts | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-023-01 | F-CON-AGNT-023 | api/agent_test.go | TestAPI_NodeMaintenance | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-025-01 | F-CON-AGNT-025 | api/agent_test.go | TestAPI_AgentUpdateToken | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-008-01 | F-CON-AGNT-008 | api/agent_test.go | TestAPI_AgentHealthServiceOpts | SDK_CLIENT | ⏳ | |
| T-CON-AGNT-009-01 | F-CON-AGNT-009 | api/agent_test.go | TestAPI_AgentHealthServiceByID | SDK_CLIENT | ⏳ | |
| T-CON-CONN-015-01 | F-CON-CONN-015 | api/agent_test.go | TestAPI_AgentConnectCARoots_empty | SDK_CLIENT | ⏳ | |
| T-CON-CONN-015-02 | F-CON-CONN-015 | api/agent_test.go | TestAPI_AgentConnectCARoots_list | SDK_CLIENT | ⏳ | |
| T-CON-CONN-016-01 | F-CON-CONN-016 | api/agent_test.go | TestAPI_AgentConnectCALeaf | SDK_CLIENT | ⏳ | |
| T-CON-CONN-014-01 | F-CON-CONN-014 | api/agent_test.go | TestAPI_AgentConnectAuthorize | SDK_CLIENT | ⏳ | |

## 2. Agent — HTTP endpoint tests (`agent/agent_endpoint_test.go`)

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-AGNT-006-03 | F-CON-AGNT-006 | agent/agent_endpoint_test.go | TestAgent_Services | HTTP_API | ⏳ | |
| T-CON-AGNT-006-04 | F-CON-AGNT-006 | agent/agent_endpoint_test.go | TestAgent_ServicesFiltered | HTTP_API | ⏳ | |
| T-CON-AGNT-007-03 | F-CON-AGNT-007 | agent/agent_endpoint_test.go | TestAgent_Service | HTTP_API | ⏳ | |
| T-CON-AGNT-005-03 | F-CON-AGNT-005 | agent/agent_endpoint_test.go | TestAgent_Checks | HTTP_API | ⏳ | |
| T-CON-AGNT-005-04 | F-CON-AGNT-005 | agent/agent_endpoint_test.go | TestAgent_ChecksWithFilter | HTTP_API | ⏳ | |
| T-CON-AGNT-009-02 | F-CON-AGNT-009 | agent/agent_endpoint_test.go | TestAgent_HealthServiceByID | HTTP_API | ⏳ | |
| T-CON-AGNT-008-02 | F-CON-AGNT-008 | agent/agent_endpoint_test.go | TestAgent_HealthServiceByName | HTTP_API | ⏳ | |
| T-CON-AGNT-001-03 | F-CON-AGNT-001 | agent/agent_endpoint_test.go | TestAgent_Self | HTTP_API | ⏳ | |
| T-CON-AGNT-004-02 | F-CON-AGNT-004 | agent/agent_endpoint_test.go | TestAgent_Metrics_ACLDeny | HTTP_API | ⏳ | |
| T-CON-AGNT-024-02 | F-CON-AGNT-024 | agent/agent_endpoint_test.go | TestAgent_Reload | HTTP_API | ⏳ | |
| T-CON-AGNT-018-03 | F-CON-AGNT-018 | agent/agent_endpoint_test.go | TestAgent_Members | HTTP_API | ⏳ | |
| T-CON-AGNT-018-04 | F-CON-AGNT-018 | agent/agent_endpoint_test.go | TestAgent_Members_WAN | HTTP_API | ⏳ | |
| T-CON-AGNT-019-02 | F-CON-AGNT-019 | agent/agent_endpoint_test.go | TestAgent_Join | HTTP_API | ⏳ | |
| T-CON-AGNT-020-02 | F-CON-AGNT-020 | agent/agent_endpoint_test.go | TestAgent_Leave | HTTP_API | ⏳ | |
| T-CON-AGNT-021-03 | F-CON-AGNT-021 | agent/agent_endpoint_test.go | TestAgent_ForceLeave | HTTP_API | ⏳ | |
| T-CON-AGNT-021-04 | F-CON-AGNT-021 | agent/agent_endpoint_test.go | TestAgent_ForceLeavePrune | HTTP_API | ⏳ | |
| T-CON-AGNT-012-03 | F-CON-AGNT-012 | agent/agent_endpoint_test.go | TestAgent_RegisterCheck | HTTP_API | ⏳ | |
| T-CON-AGNT-013-01 | F-CON-AGNT-013 | agent/agent_endpoint_test.go | TestAgent_DeregisterCheck | HTTP_API | ⏳ | |
| T-CON-AGNT-014-01 | F-CON-AGNT-014 | agent/agent_endpoint_test.go | TestAgent_PassCheck | HTTP_API | ⏳ | |
| T-CON-AGNT-015-01 | F-CON-AGNT-015 | agent/agent_endpoint_test.go | TestAgent_WarnCheck | HTTP_API | ⏳ | |
| T-CON-AGNT-016-01 | F-CON-AGNT-016 | agent/agent_endpoint_test.go | TestAgent_FailCheck | HTTP_API | ⏳ | |
| T-CON-AGNT-017-03 | F-CON-AGNT-017 | agent/agent_endpoint_test.go | TestAgent_UpdateCheck | HTTP_API | ⏳ | |
| T-CON-AGNT-010-04 | F-CON-AGNT-010 | agent/agent_endpoint_test.go | TestAgent_RegisterService | HTTP_API | ⏳ | |
| T-CON-AGNT-011-01 | F-CON-AGNT-011 | agent/agent_endpoint_test.go | TestAgent_DeregisterService | HTTP_API | ⏳ | |
| T-CON-AGNT-022-02 | F-CON-AGNT-022 | agent/agent_endpoint_test.go | TestAgent_ServiceMaintenance_Enable | HTTP_API | ⏳ | |
| T-CON-AGNT-023-02 | F-CON-AGNT-023 | agent/agent_endpoint_test.go | TestAgent_NodeMaintenance_Enable | HTTP_API | ⏳ | |
| T-CON-AGNT-025-02 | F-CON-AGNT-025 | agent/agent_endpoint_test.go | TestAgent_Token | HTTP_API | ⏳ | |
| T-CON-AGNT-002-02 | F-CON-AGNT-002 | agent/agent_endpoint_test.go | TestAgent_Host | HTTP_API | ⏳ | |

## 3. Catalog — SDK + HTTP tests

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-CAT-003-01 | F-CON-CAT-003 | api/catalog_test.go | TestAPI_CatalogDatacenters | SDK_CLIENT | ⏳ | |
| T-CON-CAT-004-01 | F-CON-CAT-004 | api/catalog_test.go | TestAPI_CatalogNodes | SDK_CLIENT | ⏳ | |
| T-CON-CAT-004-02 | F-CON-CAT-004 | api/catalog_test.go | TestAPI_CatalogNodes_MetaFilter | SDK_CLIENT | ⏳ | |
| T-CON-CAT-004-03 | F-CON-CAT-004 | api/catalog_test.go | TestAPI_CatalogNodes_Filter | SDK_CLIENT | ⏳ | |
| T-CON-CAT-005-01 | F-CON-CAT-005 | api/catalog_test.go | TestAPI_CatalogServices | SDK_CLIENT | ⏳ | |
| T-CON-CAT-005-02 | F-CON-CAT-005 | api/catalog_test.go | TestAPI_CatalogServices_NodeMetaFilter | SDK_CLIENT | ⏳ | |
| T-CON-CAT-006-01 | F-CON-CAT-006 | api/catalog_test.go | TestAPI_CatalogService | SDK_CLIENT | ⏳ | |
| T-CON-CAT-006-02 | F-CON-CAT-006 | api/catalog_test.go | TestAPI_CatalogService_SingleTag | SDK_CLIENT | ⏳ | |
| T-CON-CAT-006-03 | F-CON-CAT-006 | api/catalog_test.go | TestAPI_CatalogService_MultipleTags | SDK_CLIENT | ⏳ | |
| T-CON-CAT-006-04 | F-CON-CAT-006 | api/catalog_test.go | TestAPI_CatalogService_Filter | SDK_CLIENT | ⏳ | |
| T-CON-CAT-007-01 | F-CON-CAT-007 | api/catalog_test.go | TestAPI_CatalogConnect | SDK_CLIENT | ⏳ | |
| T-CON-CAT-007-02 | F-CON-CAT-007 | api/catalog_test.go | TestAPI_CatalogConnect_Filter | SDK_CLIENT | ⏳ | |
| T-CON-CAT-008-01 | F-CON-CAT-008 | api/catalog_test.go | TestAPI_CatalogNode | SDK_CLIENT | ⏳ | |
| T-CON-CAT-008-02 | F-CON-CAT-008 | api/catalog_test.go | TestAPI_CatalogNode_Filter | SDK_CLIENT | ⏳ | |
| T-CON-CAT-001-01 | F-CON-CAT-001 | api/catalog_test.go | TestAPI_CatalogRegistration | SDK_CLIENT | ⏳ | |
| T-CON-CAT-010-01 | F-CON-CAT-010 | api/catalog_test.go | TestAPI_CatalogGatewayServices_Terminating | SDK_CLIENT | ⏳ | |
| T-CON-CAT-010-02 | F-CON-CAT-010 | api/catalog_test.go | TestAPI_CatalogGatewayServices_Ingress | SDK_CLIENT | ⏳ | |
| T-CON-CAT-001-02 | F-CON-CAT-001 | agent/catalog_endpoint_test.go | TestCatalogRegister | HTTP_API | ⏳ | |
| T-CON-CAT-002-01 | F-CON-CAT-002 | agent/catalog_endpoint_test.go | TestCatalogDeregister | HTTP_API | ⏳ | |
| T-CON-CAT-003-02 | F-CON-CAT-003 | agent/catalog_endpoint_test.go | TestCatalogDatacenters | HTTP_API | ⏳ | |
| T-CON-CAT-004-04 | F-CON-CAT-004 | agent/catalog_endpoint_test.go | TestCatalogNodes | HTTP_API | ⏳ | |
| T-CON-CAT-004-05 | F-CON-CAT-004 | agent/catalog_endpoint_test.go | TestCatalogNodes_Blocking | HTTP_API | ⏳ | |
| T-CON-CAT-005-03 | F-CON-CAT-005 | agent/catalog_endpoint_test.go | TestCatalogServices | HTTP_API | ⏳ | |
| T-CON-CAT-006-05 | F-CON-CAT-006 | agent/catalog_endpoint_test.go | TestCatalogServiceNodes | HTTP_API | ⏳ | |
| T-CON-CAT-007-03 | F-CON-CAT-007 | agent/catalog_endpoint_test.go | TestCatalogConnectServiceNodes_good | HTTP_API | ⏳ | |

## 4. Health — SDK + HTTP tests

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-HEALTH-001-01 | F-CON-HEALTH-001 | api/health_test.go | TestAPI_HealthNode | SDK_CLIENT | ⏳ | |
| T-CON-HEALTH-001-02 | F-CON-HEALTH-001 | api/health_test.go | TestAPI_HealthNode_Filter | SDK_CLIENT | ⏳ | |
| T-CON-HEALTH-002-01 | F-CON-HEALTH-002 | api/health_test.go | TestAPI_HealthChecks | SDK_CLIENT | ⏳ | |
| T-CON-HEALTH-002-02 | F-CON-HEALTH-002 | api/health_test.go | TestAPI_HealthChecks_Filter | SDK_CLIENT | ⏳ | |
| T-CON-HEALTH-003-01 | F-CON-HEALTH-003 | api/health_test.go | TestAPI_HealthService | SDK_CLIENT | ⏳ | |
| T-CON-HEALTH-003-02 | F-CON-HEALTH-003 | api/health_test.go | TestAPI_HealthService_SingleTag | SDK_CLIENT | ⏳ | |
| T-CON-HEALTH-003-03 | F-CON-HEALTH-003 | api/health_test.go | TestAPI_HealthService_Filter | SDK_CLIENT | ⏳ | |
| T-CON-HEALTH-004-01 | F-CON-HEALTH-004 | api/health_test.go | TestAPI_HealthConnect | SDK_CLIENT | ⏳ | |
| T-CON-HEALTH-004-02 | F-CON-HEALTH-004 | api/health_test.go | TestAPI_HealthConnect_Filter | SDK_CLIENT | ⏳ | |
| T-CON-HEALTH-005-01 | F-CON-HEALTH-005 | api/health_test.go | TestAPI_HealthIngress | SDK_CLIENT | ⏳ | |
| T-CON-HEALTH-006-01 | F-CON-HEALTH-006 | api/health_test.go | TestAPI_HealthState | SDK_CLIENT | ⏳ | |
| T-CON-HEALTH-006-02 | F-CON-HEALTH-006 | api/health_test.go | TestAPI_HealthState_Filter | SDK_CLIENT | ⏳ | |
| T-CON-HEALTH-001-03 | F-CON-HEALTH-001 | agent/health_endpoint_test.go | TestHealthNodeChecks | HTTP_API | ⏳ | |
| T-CON-HEALTH-002-03 | F-CON-HEALTH-002 | agent/health_endpoint_test.go | TestHealthServiceChecks | HTTP_API | ⏳ | |
| T-CON-HEALTH-003-04 | F-CON-HEALTH-003 | agent/health_endpoint_test.go | TestHealthServiceNodes | HTTP_API | ⏳ | |
| T-CON-HEALTH-003-05 | F-CON-HEALTH-003 | agent/health_endpoint_test.go | TestHealthServiceNodes_Blocking | HTTP_API | ⏳ | |
| T-CON-HEALTH-006-03 | F-CON-HEALTH-006 | agent/health_endpoint_test.go | TestHealthChecksInState | HTTP_API | ⏳ | |
| T-CON-HEALTH-004-03 | F-CON-HEALTH-004 | agent/health_endpoint_test.go | TestHealthConnectServiceNodes | HTTP_API | ⏳ | |
| T-CON-HEALTH-005-02 | F-CON-HEALTH-005 | agent/health_endpoint_test.go | TestHealthIngressServiceNodes | HTTP_API | ⏳ | |

## 5. KV — SDK + HTTP tests

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-KV-001-01 | F-CON-KV-001 | api/kv_test.go | TestAPI_ClientPutGetDelete | SDK_CLIENT | ⏳ | |
| T-CON-KV-001-02 | F-CON-KV-001 | api/kv_test.go | TestAPI_ClientList_DeleteRecurse | SDK_CLIENT | ⏳ | |
| T-CON-KV-001-03 | F-CON-KV-001 | api/kv_test.go | TestAPI_ClientKeys_DeleteRecurse | SDK_CLIENT | ⏳ | |
| T-CON-KV-001-04 | F-CON-KV-001 | api/kv_test.go | TestAPI_ClientWatchGet | SDK_CLIENT | ⏳ | |
| T-CON-KV-001-05 | F-CON-KV-001 | api/kv_test.go | TestAPI_ClientWatchList | SDK_CLIENT | ⏳ | |
| T-CON-KV-002-01 | F-CON-KV-002 | api/kv_test.go | TestAPI_ClientCAS | SDK_CLIENT | ⏳ | |
| T-CON-KV-002-02 | F-CON-KV-002 | api/kv_test.go | TestAPI_ClientAcquireRelease | SDK_CLIENT | ⏳ | |
| T-CON-KV-003-01 | F-CON-KV-003 | api/kv_test.go | TestAPI_ClientDeleteCAS | SDK_CLIENT | ⏳ | |
| T-CON-KV-001-06 | F-CON-KV-001 | agent/kvs_endpoint_test.go | TestKVSEndpoint_PUT_GET_DELETE | HTTP_API | ⏳ | |
| T-CON-KV-001-07 | F-CON-KV-001 | agent/kvs_endpoint_test.go | TestKVSEndpoint_Recurse | HTTP_API | ⏳ | |
| T-CON-KV-001-08 | F-CON-KV-001 | agent/kvs_endpoint_test.go | TestKVSEndpoint_ListKeys | HTTP_API | ⏳ | |
| T-CON-KV-001-09 | F-CON-KV-001 | agent/kvs_endpoint_test.go | TestKVSEndpoint_GET_Raw | HTTP_API | ⏳ | |
| T-CON-KV-002-03 | F-CON-KV-002 | agent/kvs_endpoint_test.go | TestKVSEndpoint_CAS | HTTP_API | ⏳ | |
| T-CON-KV-002-04 | F-CON-KV-002 | agent/kvs_endpoint_test.go | TestKVSEndpoint_AcquireRelease | HTTP_API | ⏳ | |
| T-CON-KV-003-02 | F-CON-KV-003 | agent/kvs_endpoint_test.go | TestKVSEndpoint_DELETE_CAS | HTTP_API | ⏳ | |

## 6. Session — SDK + HTTP tests

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-SES-001-01 | F-CON-SES-001 | api/session_test.go | TestAPI_SessionCreateDestroy | SDK_CLIENT | ⏳ | |
| T-CON-SES-003-01 | F-CON-SES-003 | api/session_test.go | TestAPI_SessionCreateRenewDestroy | SDK_CLIENT | ⏳ | |
| T-CON-SES-004-01 | F-CON-SES-004 | api/session_test.go | TestAPI_SessionInfo | SDK_CLIENT | ⏳ | |
| T-CON-SES-005-01 | F-CON-SES-005 | api/session_test.go | TestAPI_SessionNode | SDK_CLIENT | ⏳ | |
| T-CON-SES-006-01 | F-CON-SES-006 | api/session_test.go | TestAPI_SessionList | SDK_CLIENT | ⏳ | |
| T-CON-SES-001-02 | F-CON-SES-001 | agent/session_endpoint_test.go | TestSessionCreate | HTTP_API | ⏳ | |
| T-CON-SES-002-01 | F-CON-SES-002 | agent/session_endpoint_test.go | TestSessionDestroy | HTTP_API | ⏳ | |
| T-CON-SES-003-02 | F-CON-SES-003 | agent/session_endpoint_test.go | TestSessionTTLRenew | HTTP_API | ⏳ | |
| T-CON-SES-004-02 | F-CON-SES-004 | agent/session_endpoint_test.go | TestSessionGet | HTTP_API | ⏳ | |
| T-CON-SES-006-02 | F-CON-SES-006 | agent/session_endpoint_test.go | TestSessionList | HTTP_API | ⏳ | |
| T-CON-SES-005-02 | F-CON-SES-005 | agent/session_endpoint_test.go | TestSessionsForNode | HTTP_API | ⏳ | |

## 7. ACL — SDK + HTTP tests

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-ACL-001-01 | F-CON-ACL-001 | api/acl_test.go | TestAPI_ACLBootstrap | SDK_CLIENT | ⏳ | |
| T-CON-ACL-002-01 | F-CON-ACL-002 | api/acl_test.go | TestAPI_ACLToken_CreateReadDelete | SDK_CLIENT | ⏳ | |
| T-CON-ACL-003-01 | F-CON-ACL-003 | api/acl_test.go | TestAPI_ACLToken_CreateUpdate | SDK_CLIENT | ⏳ | |
| T-CON-ACL-004-01 | F-CON-ACL-004 | api/acl_test.go | TestAPI_ACLToken_Clone | SDK_CLIENT | ⏳ | |
| T-CON-ACL-005-01 | F-CON-ACL-005 | api/acl_test.go | TestAPI_ACLToken_CreateReadDelete | SDK_CLIENT | ⏳ | |
| T-CON-ACL-008-01 | F-CON-ACL-008 | api/acl_test.go | TestAPI_ACLToken_List | SDK_CLIENT | ⏳ | |
| T-CON-ACL-008-02 | F-CON-ACL-008 | api/acl_test.go | TestAPI_ACLToken_ListFiltered | SDK_CLIENT | ⏳ | |
| T-CON-ACL-009-01 | F-CON-ACL-009 | api/acl_test.go | TestAPI_ACLPolicy_CreateReadDelete | SDK_CLIENT | ⏳ | |
| T-CON-ACL-011-01 | F-CON-ACL-011 | api/acl_test.go | TestAPI_ACLPolicy_CreateReadDelete | SDK_CLIENT | ⏳ | |
| T-CON-ACL-013-01 | F-CON-ACL-013 | api/acl_test.go | TestAPI_ACLPolicy_CreateReadByNameDelete | SDK_CLIENT | ⏳ | |
| T-CON-ACL-014-01 | F-CON-ACL-014 | api/acl_test.go | TestAPI_ACLPolicy_List | SDK_CLIENT | ⏳ | |
| T-CON-ACL-027-01 | F-CON-ACL-027 | api/acl_test.go | TestAPI_AuthMethod_List | SDK_CLIENT | ⏳ | |
| T-CON-ACL-039-01 | F-CON-ACL-039 | api/acl_test.go | TestAPI_ACLReplication | SDK_CLIENT | ⏳ | |
| T-CON-ACL-001-02 | F-CON-ACL-001 | agent/acl_endpoint_test.go | TestACL_Bootstrap | HTTP_API | ⏳ | |
| T-CON-ACL-021-01 | F-CON-ACL-021 | agent/acl_endpoint_test.go | TestACL_LoginProcedure_HTTP | HTTP_API | ⏳ | |
| T-CON-ACL-022-01 | F-CON-ACL-022 | agent/acl_endpoint_test.go | TestACLEndpoint_LoginLogout_jwt | HTTP_API | ⏳ | |
| T-CON-ACL-028-01 | F-CON-ACL-028 | agent/acl_endpoint_test.go | TestACL_Authorize | HTTP_API | ⏳ | |
| T-CON-ACL-039-02 | F-CON-ACL-039 | agent/acl_endpoint_test.go | TestHTTPHandlers_ACLReplicationStatus | HTTP_API | ⏳ | |

## 8. Config Entries — SDK + HTTP tests

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-CFGE-001-01 | F-CON-CFGE-001 | api/config_entry_test.go | TestAPI_ConfigEntries | SDK_CLIENT | ⏳ | |
| T-CON-CFGE-002-01 | F-CON-CFGE-002 | api/config_entry_test.go | TestAPI_ConfigEntries | SDK_CLIENT | ⏳ | |
| T-CON-CFGE-003-01 | F-CON-CFGE-003 | api/config_entry_test.go | TestAPI_ConfigEntries | SDK_CLIENT | ⏳ | |
| T-CON-CFGE-004-01 | F-CON-CFGE-004 | api/config_entry_test.go | TestAPI_ConfigEntries | SDK_CLIENT | ⏳ | |
| T-CON-CFGE-001-02 | F-CON-CFGE-001 | agent/config_endpoint_test.go | TestConfig_Get | HTTP_API | ⏳ | |
| T-CON-CFGE-002-02 | F-CON-CFGE-002 | agent/config_endpoint_test.go | TestConfig_Get | HTTP_API | ⏳ | |
| T-CON-CFGE-003-02 | F-CON-CFGE-003 | agent/config_endpoint_test.go | TestConfig_Apply | HTTP_API | ⏳ | |
| T-CON-CFGE-003-03 | F-CON-CFGE-003 | agent/config_endpoint_test.go | TestConfig_Apply_CAS | HTTP_API | ⏳ | |
| T-CON-CFGE-004-02 | F-CON-CFGE-004 | agent/config_endpoint_test.go | TestConfig_Delete | HTTP_API | ⏳ | |
| T-CON-CFGE-004-03 | F-CON-CFGE-004 | agent/config_endpoint_test.go | TestConfig_Delete_CAS | HTTP_API | ⏳ | |

## 9. Operator — SDK + HTTP tests

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-OP-001-01 | F-CON-OP-001 | api/operator_raft_test.go | TestAPI_OperatorRaftGetConfiguration | SDK_CLIENT | ⏳ | |
| T-CON-OP-003-01 | F-CON-OP-003 | api/operator_raft_test.go | TestAPI_OperatorRaftRemovePeerByAddress | SDK_CLIENT | ⏳ | |
| T-CON-OP-003-02 | F-CON-OP-003 | api/operator_raft_test.go | TestAPI_OperatorRaftRemovePeerByID | SDK_CLIENT | ⏳ | |
| T-CON-OP-002-01 | F-CON-OP-002 | api/operator_raft_test.go | TestAPI_OperatorRaftLeaderTransfer | SDK_CLIENT | ⏳ | |
| T-CON-OP-004-01 | F-CON-OP-004 | api/operator_keyring_test.go | TestAPI_OperatorKeyringInstallListPutRemove | SDK_CLIENT | ⏳ | |
| T-CON-OP-005-01 | F-CON-OP-005 | api/operator_autopilot_test.go | TestAPI_OperatorAutopilotGetSetConfiguration | SDK_CLIENT | ⏳ | |
| T-CON-OP-006-01 | F-CON-OP-006 | api/operator_autopilot_test.go | TestAPI_OperatorAutopilotCASConfiguration | SDK_CLIENT | ⏳ | |
| T-CON-OP-007-01 | F-CON-OP-007 | api/operator_autopilot_test.go | TestAPI_OperatorAutopilotServerHealth | SDK_CLIENT | ⏳ | |
| T-CON-OP-008-01 | F-CON-OP-008 | api/operator_autopilot_test.go | TestAPI_OperatorAutopilotState | SDK_CLIENT | ⏳ | |
| T-CON-OP-009-01 | F-CON-OP-009 | api/operator_usage_test.go | TestAPI_OperatorUsage | SDK_CLIENT | ⏳ | |
| T-CON-OP-001-02 | F-CON-OP-001 | agent/operator_endpoint_test.go | TestOperator_RaftConfiguration | HTTP_API | ⏳ | |
| T-CON-OP-005-02 | F-CON-OP-005 | agent/operator_endpoint_test.go | TestOperator_AutopilotGetConfiguration | HTTP_API | ⏳ | |
| T-CON-OP-006-02 | F-CON-OP-006 | agent/operator_endpoint_test.go | TestOperator_AutopilotSetConfiguration | HTTP_API | ⏳ | |
| T-CON-OP-006-03 | F-CON-OP-006 | agent/operator_endpoint_test.go | TestOperator_AutopilotCASConfiguration | HTTP_API | ⏳ | |
| T-CON-OP-007-02 | F-CON-OP-007 | agent/operator_endpoint_test.go | TestOperator_ServerHealth | HTTP_API | ⏳ | |
| T-CON-OP-008-02 | F-CON-OP-008 | agent/operator_endpoint_test.go | TestOperator_AutopilotState | HTTP_API | ⏳ | |

## 10. Connect mesh — SDK + HTTP tests

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-CONN-001-01 | F-CON-CONN-001 | api/connect_ca_test.go | TestAPI_ConnectCARoots_empty | SDK_CLIENT | ⏳ | |
| T-CON-CONN-001-02 | F-CON-CONN-001 | api/connect_ca_test.go | TestAPI_ConnectCARoots_list | SDK_CLIENT | ⏳ | |
| T-CON-CONN-002-01 | F-CON-CONN-002 | api/connect_ca_test.go | TestAPI_ConnectCAConfig_get_set | SDK_CLIENT | ⏳ | |
| T-CON-CONN-003-01 | F-CON-CONN-003 | api/connect_ca_test.go | TestAPI_ConnectCAConfig_get_set | SDK_CLIENT | ⏳ | |
| T-CON-CONN-004-01 | F-CON-CONN-004 | api/connect_intention_test.go | TestAPI_ConnectIntentionCreateListGetUpdateDelete | SDK_CLIENT | ⏳ | |
| T-CON-CONN-012-01 | F-CON-CONN-012 | api/connect_intention_test.go | TestAPI_ConnectIntentionMatch | SDK_CLIENT | ⏳ | |
| T-CON-CONN-013-01 | F-CON-CONN-013 | api/connect_intention_test.go | TestAPI_ConnectIntentionCheck | SDK_CLIENT | ⏳ | |
| T-CON-CONN-001-03 | F-CON-CONN-001 | agent/connect_ca_endpoint_test.go | TestConnectCARoots_empty | HTTP_API | ⏳ | |
| T-CON-CONN-001-04 | F-CON-CONN-001 | agent/connect_ca_endpoint_test.go | TestConnectCARoots_list | HTTP_API | ⏳ | |
| T-CON-CONN-002-02 | F-CON-CONN-002 | agent/connect_ca_endpoint_test.go | TestConnectCAConfig | HTTP_API | ⏳ | |
| T-CON-CONN-003-02 | F-CON-CONN-003 | agent/connect_ca_endpoint_test.go | TestConnectCAConfig | HTTP_API | ⏳ | |
| T-CON-CONN-004-02 | F-CON-CONN-004 | agent/intentions_endpoint_test.go | TestIntentionList | HTTP_API | ⏳ | |
| T-CON-CONN-006-01 | F-CON-CONN-006 | agent/intentions_endpoint_test.go | TestIntentionCreate | HTTP_API | ⏳ | |
| T-CON-CONN-012-02 | F-CON-CONN-012 | agent/intentions_endpoint_test.go | TestIntentionMatch | HTTP_API | ⏳ | |
| T-CON-CONN-013-02 | F-CON-CONN-013 | agent/intentions_endpoint_test.go | TestIntentionCheck | HTTP_API | ⏳ | |

## 11. Status / Coordinate / Event / Snapshot / Query / Txn — SDK + HTTP tests

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-OTH-001-01 | F-CON-OTH-001 | api/status_test.go | TestAPI_StatusLeader | SDK_CLIENT | ⏳ | |
| T-CON-OTH-002-01 | F-CON-OTH-002 | api/status_test.go | TestAPI_StatusPeers | SDK_CLIENT | ⏳ | |
| T-CON-OTH-003-01 | F-CON-OTH-003 | api/coordinate_test.go | TestAPI_CoordinateDatacenters | SDK_CLIENT | ⏳ | |
| T-CON-OTH-004-01 | F-CON-OTH-004 | api/coordinate_test.go | TestAPI_CoordinateNodes | SDK_CLIENT | ⏳ | |
| T-CON-OTH-005-01 | F-CON-OTH-005 | api/coordinate_test.go | TestAPI_CoordinateNode | SDK_CLIENT | ⏳ | |
| T-CON-OTH-006-01 | F-CON-OTH-006 | api/coordinate_test.go | TestAPI_CoordinateUpdate | SDK_CLIENT | ⏳ | |
| T-CON-OTH-007-01 | F-CON-OTH-007 | api/event_test.go | TestAPI_EventFireList | SDK_CLIENT | ⏳ | |
| T-CON-OTH-008-01 | F-CON-OTH-008 | api/event_test.go | TestAPI_EventFireList | SDK_CLIENT | ⏳ | |
| T-CON-OTH-009-01 | F-CON-OTH-009 | api/snapshot_test.go | TestAPI_Snapshot | SDK_CLIENT | ⏳ | |
| T-CON-OTH-010-01 | F-CON-OTH-010 | api/snapshot_test.go | TestAPI_Snapshot | SDK_CLIENT | ⏳ | |
| T-CON-OTH-018-01 | F-CON-OTH-018 | api/txn_test.go | TestAPI_ClientTxn | SDK_CLIENT | ⏳ | |
| T-CON-OTH-018-02 | F-CON-OTH-018 | api/txn_test.go | TestAPI_ClientTxnWrite | SDK_CLIENT | ⏳ | |
| T-CON-OTH-011-01 | F-CON-OTH-011 | api/prepared_query_test.go | TestAPI_PreparedQuery | SDK_CLIENT | ⏳ | |
| T-CON-OTH-016-01 | F-CON-OTH-016 | api/prepared_query_test.go | TestAPI_PreparedQuery | SDK_CLIENT | ⏳ | |
| T-CON-OTH-017-01 | F-CON-OTH-017 | api/prepared_query_test.go | TestAPI_PreparedQuery | SDK_CLIENT | ⏳ | |
| T-CON-OTH-001-02 | F-CON-OTH-001 | agent/status_endpoint_test.go | TestStatusLeader | HTTP_API | ⏳ | |
| T-CON-OTH-002-02 | F-CON-OTH-002 | agent/status_endpoint_test.go | TestStatusPeers | HTTP_API | ⏳ | |
| T-CON-OTH-003-02 | F-CON-OTH-003 | agent/coordinate_endpoint_test.go | TestCoordinate_Datacenters | HTTP_API | ⏳ | |
| T-CON-OTH-004-02 | F-CON-OTH-004 | agent/coordinate_endpoint_test.go | TestCoordinate_Nodes | HTTP_API | ⏳ | |
| T-CON-OTH-005-02 | F-CON-OTH-005 | agent/coordinate_endpoint_test.go | TestCoordinate_Node | HTTP_API | ⏳ | |
| T-CON-OTH-006-02 | F-CON-OTH-006 | agent/coordinate_endpoint_test.go | TestCoordinate_Update | HTTP_API | ⏳ | |
| T-CON-OTH-007-02 | F-CON-OTH-007 | agent/event_endpoint_test.go | TestEventFire | HTTP_API | ⏳ | |
| T-CON-OTH-008-02 | F-CON-OTH-008 | agent/event_endpoint_test.go | TestEventList | HTTP_API | ⏳ | |
| T-CON-OTH-009-02 | F-CON-OTH-009 | agent/snapshot_endpoint_test.go | TestSnapshot | HTTP_API | ⏳ | |
| T-CON-OTH-010-02 | F-CON-OTH-010 | agent/snapshot_endpoint_test.go | TestSnapshot | HTTP_API | ⏳ | |
| T-CON-OTH-018-03 | F-CON-OTH-018 | agent/txn_endpoint_test.go | TestTxnEndpoint_Bad_JSON | HTTP_API | ⏳ | |

## 12. Peering / Namespace — SDK tests

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-PEER-001-01 | F-CON-PEER-001 | api/peering_test.go | TestAPI_Peering_Read_ErrorHandling | SDK_CLIENT | ⏳ | |
| T-CON-PEER-002-01 | F-CON-PEER-002 | api/peering_test.go | TestAPI_Peering_GenerateToken_Read_Establish_Delete | SDK_CLIENT | ⏳ | |
| T-CON-PEER-003-01 | F-CON-PEER-003 | api/peering_test.go | TestAPI_Peering_GenerateToken_ExternalAddresses | SDK_CLIENT | ⏳ | |
| T-CON-PEER-004-01 | F-CON-PEER-004 | api/peering_test.go | TestAPI_Peering_GenerateToken_Read_Establish_Delete | SDK_CLIENT | ⏳ | |
| T-CON-PEER-005-01 | F-CON-PEER-005 | api/peering_test.go | TestAPI_Peering_List | SDK_CLIENT | ⏳ | |
| T-CON-PEER-008-01 | F-CON-PEER-008 | api/namespace_test.go | TestAPI_Namespaces | SDK_CLIENT | ⏳ | |
| T-CON-PEER-009-01 | F-CON-PEER-009 | api/namespace_test.go | TestAPI_Namespaces | SDK_CLIENT | ⏳ | |
| T-CON-PEER-010-01 | F-CON-PEER-010 | api/namespace_test.go | TestAPI_Namespaces | SDK_CLIENT | ⏳ | |
| T-CON-PEER-011-01 | F-CON-PEER-011 | api/namespace_test.go | TestAPI_Namespaces | SDK_CLIENT | ⏳ | |
| T-CON-PEER-012-01 | F-CON-PEER-012 | api/namespace_test.go | TestAPI_Namespaces | SDK_CLIENT | ⏳ | |

## 13. Discovery Chain — SDK tests

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-DC-001-01 | F-CON-DC-001 | api/discovery_chain_test.go | TestAPI_DiscoveryChain_Get | SDK_CLIENT | ⏳ | |

## 14. Blocking query contract — HTTP tests

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-CORE-001-01 | F-CON-CORE-001 | agent/http_test.go | TestParseWait | HTTP_API | ⏳ | |
| T-CON-CORE-001-02 | F-CON-CORE-001 | agent/http_test.go | TestParseWait_InvalidTime | HTTP_API | ⏳ | |
| T-CON-CORE-001-03 | F-CON-CORE-001 | agent/http_test.go | TestParseWait_InvalidIndex | HTTP_API | ⏳ | |
| T-CON-CORE-002-01 | F-CON-CORE-002 | agent/http_test.go | TestSetIndex | HTTP_API | ⏳ | |
| T-CON-CORE-004-01 | F-CON-CORE-004 | agent/http_test.go | TestSetLastContact | HTTP_API | ⏳ | |
| T-CON-CORE-004-02 | F-CON-CORE-004 | agent/http_test.go | TestSetKnownLeader | HTTP_API | ⏳ | |
| T-CON-CORE-005-01 | F-CON-CORE-005 | agent/http_test.go | TestParseConsistency | HTTP_API | ⏳ | |
| T-CON-CORE-005-02 | F-CON-CORE-005 | agent/http_test.go | TestParseConsistencyAndMaxStale | HTTP_API | ⏳ | |

## 15. Watch (SDK client) — long-poll subscription tests

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-CORE-001-04 | F-CON-CORE-001 | api/watch/funcs_test.go | TestKeyWatch | SDK_CLIENT | ⏳ | |
| T-CON-CORE-001-05 | F-CON-CORE-001 | api/watch/funcs_test.go | TestKeyPrefixWatch | SDK_CLIENT | ⏳ | |
| T-CON-CORE-001-06 | F-CON-CORE-001 | api/watch/funcs_test.go | TestServicesWatch | SDK_CLIENT | ⏳ | |
| T-CON-CORE-001-07 | F-CON-CORE-001 | api/watch/funcs_test.go | TestNodesWatch | SDK_CLIENT | ⏳ | |
| T-CON-CORE-001-08 | F-CON-CORE-001 | api/watch/funcs_test.go | TestServiceWatch | SDK_CLIENT | ⏳ | |
| T-CON-CORE-001-09 | F-CON-CORE-001 | api/watch/funcs_test.go | TestChecksWatch_State | SDK_CLIENT | ⏳ | |
| T-CON-CORE-001-10 | F-CON-CORE-001 | api/watch/funcs_test.go | TestEventWatch | SDK_CLIENT | ⏳ | |
| T-CON-CONN-001-05 | F-CON-CONN-001 | api/watch/funcs_test.go | TestConnectRootsWatch | SDK_CLIENT | ⏳ | |
| T-CON-CONN-016-02 | F-CON-CONN-016 | api/watch/funcs_test.go | TestConnectLeafWatch | SDK_CLIENT | ⏳ | |

## 16. Lock & Semaphore (SDK client) — KV-based coordination

| T-ID | Feature | Upstream file | Upstream method | Type | Status | Skip reason |
|------|---------|---------------|-----------------|------|--------|-------------|
| T-CON-KV-002-05 | F-CON-KV-002 | api/lock_test.go | TestAPI_LockLockUnlock | SDK_CLIENT | ⏳ | |
| T-CON-KV-002-06 | F-CON-KV-002 | api/lock_test.go | TestAPI_LockContend | SDK_CLIENT | ⏳ | |
| T-CON-KV-002-07 | F-CON-KV-002 | api/lock_test.go | TestAPI_LockOneShot | SDK_CLIENT | ⏳ | |
| T-CON-KV-002-08 | F-CON-KV-002 | api/semaphore_test.go | TestAPI_SemaphoreAcquireRelease | SDK_CLIENT | ⏳ | |
| T-CON-KV-002-09 | F-CON-KV-002 | api/semaphore_test.go | TestAPI_SemaphoreContend | SDK_CLIENT | ⏳ | |

---

# Summary

| Module | SDK tests | HTTP tests | Watch/Lock | Total | Pending | Skip |
|--------|-----------|------------|------------|-------|---------|------|
| Agent | 31 | 28 | 0 | 59 | 59 | 0 |
| Catalog | 17 | 9 | 0 | 26 | 26 | 0 |
| Health | 12 | 9 | 0 | 21 | 21 | 0 |
| KV | 7 | 8 | 5 | 20 | 20 | 0 |
| Session | 6 | 6 | 0 | 12 | 12 | 0 |
| ACL | 14 | 6 | 0 | 20 | 20 | 0 |
| Config Entries | 4 | 6 | 0 | 10 | 10 | 0 |
| Operator | 10 | 7 | 0 | 17 | 17 | 0 |
| Connect mesh | 7 | 8 | 2 | 17 | 17 | 0 |
| Status/Coord/Event/Snap/Query/Txn | 16 | 11 | 0 | 27 | 27 | 0 |
| Peering/Namespace | 10 | 0 | 0 | 10 | 10 | 0 |
| Discovery Chain | 1 | 0 | 0 | 1 | 1 | 0 |
| Blocking query | 0 | 7 | 8 | 15 | 15 | 0 |
| **Total** | **125** | **99** | **15** | **255** | **255** | **0** |

> All 255 test cases map to features marked 🟢 or 🟡 in features.md. Features marked ⚪ (OIDC, partition, filter on agent) have no upstream test mapping because they are not implemented. Internal RPC tests (agent/consul/*) and gRPC tests (agent/grpc-external/*) are excluded as they test Consul internals, not the HTTP contract batata reimplements.
