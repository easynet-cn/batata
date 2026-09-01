# Apollo Feature Inventory

Upstream baseline: **Apollo (2025)** (local `~/work/github/easynet-cn/apollo`).

> Purpose & scope: track batata's Apollo **compatibility** implementation status.
> Apollo splits into three services: **configservice** (client fetch + long polling), **adminservice** (backend write ops, Java SDK/portal backend), **portal** (aggregation + **OpenAPI v1**, used by the portal UI itself in this version). The legacy portal WebAPI is `@Deprecated` and kept only for compatibility.
>
> Granularity: each row is one external HTTP contract. Paths verified against the modules' controllers and batata source code.

Status: `🟢 full` | `🟡 partial` | `⚡ in-progress` | `⚪ planned` | `⛔ missing`

---

## 1. configservice — client fetch (`F-APO-CFGSVC-`)

> Core client contract. Auth: optional **AccessKey HMAC-SHA1 signature** (`Authorization` + `Timestamp`) when the app has an AccessKey; otherwise open.

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-CFGSVC-001 | GET `/configs/{appId}/{clusterName}/{namespace}` | | 🟢 | | **core**: `?releaseKey=&ip=&label=&dataCenter=&messages=`; 404 no release, 304 same releaseKey |
| F-APO-CFGSVC-002 | GET `/configfiles/{appId}/{clusterName}/{namespace}` | | 🟢 | | text/plain properties; 404; also renders YAML/XML by ns suffix |
| F-APO-CFGSVC-003 | GET `/configfiles/json/{appId}/{clusterName}/{namespace}` | | 🟢 | | json kv map |
| F-APO-CFGSVC-004 | GET `/configfiles/yaml/{appId}/{clusterName}/{namespace}` | | 🟢 | | YAML format (rendered via generic /configfiles handler) |
| F-APO-CFGSVC-005 | GET `/configfiles/xml/{appId}/{clusterName}/{namespace}` | | 🟢 | | XML format (rendered via generic /configfiles handler) |
| F-APO-CFGSVC-006 | GET `/configfiles/raw/{appId}/{clusterName}/{namespace}` | | 🟢 | | raw content; content-type by ns suffix (route registered config.rs) |
| F-APO-CFGSVC-007 | AccessKey signature auth (HMAC-SHA1) | | 🟢 | unit vector + path tests | port of ClientAuthenticationFilter: appId from path/query (blank→400 InvalidAppId); enabled key ⇒ signature REQUIRED else 401; ALL enabled secrets tried; 60s default skew (`APOLLO_ACCESS_KEY_AUTH_TIME_DIFF_TOLERANCE_SECS`); CSPRNG secrets |

## 2. configservice — long polling (`F-APO-LONGPOL-`)

> Apollo's core change-notification mechanism. No `waitTime` param — server holds `longPollingTimeoutInMilli` (default 90s); timeout → **304**.

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-LONGPOL-001 | GET `/notifications/v2` | | 🟢 | unit: notification_test | **core**: persisted ReleaseMessage rows are the id truth-source (row pk, restart-safe); watch-key expansion (cluster/dataCenter/default + public-ns owner keys); hub = wake accelerator; 60s hold → 200 `[{namespaceName,notificationId,messages.details}]` / 304 |
| F-APO-LONGPOL-002 | GET `/notifications` (v1, deprecated) | | 🟢 | | 30s suspend + `!=` compare vs persisted latest id, single-object response (upstream deprecated controller semantics) |

## 3. configservice — metaservice (`F-APO-META-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-META-001 | GET `/services/config?appId=&ip=` | | 🟢 | route_test | database-discovery mode (upstream DatabaseDiscoveryClientImpl): live `apollo_service_registry` rows within 61s window; empty registry falls back to this node; heartbeat self-registration every 10s in plugin init |
| F-APO-META-002 | GET `/services/admin` | | 🟢 | route_test | same discovery, adminservice role registered by the same process |
| F-APO-META-003 | GET `/` | | 🟢 | route_test | both service lists via discovery |

## 4. adminservice — apps/clusters/namespaces (`F-APO-ADM-`)

> Auth: `Authorization` = admin token whitelist when `apollo.admin-service.access.enabled=true`.

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-ADM-001 | POST `/apps` | | 🟢 | | create app (auto-issuable AccessKey) |
| F-APO-ADM-002 | GET `/apps` | | 🟢 | | list `?name=` |
| F-APO-ADM-003 | GET `/apps/{appId}` | | 🟢 | | read |
| F-APO-ADM-004 | PUT `/apps/{appId}` | | 🟢 | | update |
| F-APO-ADM-005 | DELETE `/apps/{appId}?operator=` | | 🟢 | | delete |
| F-APO-ADM-006 | GET `/apps/{appId}/unique` | | 🟢 | | appId uniqueness |
| F-APO-ADM-007 | POST `/apps/{appId}/clusters?autoCreatePrivateNamespace=` | | 🟢 | | create cluster |
| F-APO-ADM-008 | GET `/apps/{appId}/clusters` | | 🟢 | | list |
| F-APO-ADM-009 | GET `/apps/{appId}/clusters/{clusterName}` | | 🟢 | | read |
| F-APO-ADM-010 | DELETE `/apps/{appId}/clusters/{clusterName}?operator=` | | 🟢 | | delete |
| F-APO-ADM-011 | GET `/apps/{appId}/cluster/{clusterName}/unique` | | 🟢 | live smoke | returns `true` when the name is free (upstream isClusterNameUnique) |
| F-APO-ADM-012 | POST `/apps/{appId}/clusters/{clusterName}/namespaces` | | 🟢 | | create namespace |
| F-APO-ADM-013 | GET `/apps/{appId}/clusters/{clusterName}/namespaces` | | 🟢 | | list |
| F-APO-ADM-014 | GET `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}` | | 🟢 | | read |
| F-APO-ADM-015 | DELETE `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}?operator=` | | 🟢 | | delete |
| F-APO-ADM-016 | GET `/namespaces/{namespaceId}` | | 🟢 | | by id |
| F-APO-ADM-017 | GET `/namespaces/find-by-item?itemKey=` | | 🟢 | live smoke | items-by-key → namespaces paged (`find_namespace_ids_by_item_key`) |
| F-APO-ADM-018 | GET `.../associated-public-namespace` | | 🟢 | live smoke | default→owner ns; custom w/o release → owner default ns; 404 when public ns absent |
| F-APO-ADM-019 | GET `/apps/{appId}/namespaces/publish_info` | | 🟢 | live smoke | clusterName→bool (any namespace with unpublished item changes) |

## 5. adminservice — items (`F-APO-ITEM-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-ITEM-001 | POST `.../namespaces/{namespaceName}/items` | | 🟢 | | add (takes ns lock) |
| F-APO-ITEM-002 | POST `.../items/{namespaceName}/comment_items` | | 🟢 | | comment-only item |
| F-APO-ITEM-003 | PUT `.../items/{itemId}` | | 🟢 | | update (value/type/comment) |
| F-APO-ITEM-004 | DELETE `/items/{itemId}?operator=` | | 🟢 | | delete by id |
| F-APO-ITEM-005 | GET `.../namespaces/{namespaceName}/items` | | 🟢 | | list |
| F-APO-ITEM-006 | GET `.../items/deleted` | | ⚪ | | deleted since last publish |
| F-APO-ITEM-007 | GET `.../items/{key}` | | 🟢 | | read by key |
| F-APO-ITEM-008 | GET `.../encodedItems/{key}` | | ⚪ | | base64-key read |
| F-APO-ITEM-009 | GET `.../items-with-page` | | ⚪ | | paged |
| F-APO-ITEM-010 | GET `/items/{itemId}` | | 🟢 | | by id |
| F-APO-ITEM-011 | GET `/items-search/key-and-value?key=&value=` | | 🟡 | | global search — exists at `/search` (non-standard path) |
| F-APO-ITEM-012 | POST `.../namespaces/{namespaceName}/itemset` | | 🟢 | | batch change (takes lock) |

## 6. adminservice — release / branch / history (`F-APO-RELEASE-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-RELEASE-001 | POST `.../namespaces/{namespaceName}/releases` | | 🟢 | | **core**: publish `?name=&operator=` |
| F-APO-RELEASE-002 | GET `/releases/{releaseId}` | | 🟢 | | read |
| F-APO-RELEASE-003 | GET `/releases?releaseIds=` | | 🟢 | | batch |
| F-APO-RELEASE-004 | GET `.../releases/all` | | 🟢 | | history |
| F-APO-RELEASE-005 | GET `.../releases/active` | | 🟢 | | active list |
| F-APO-RELEASE-006 | GET `.../releases/latest` | | 🟢 | | latest |
| F-APO-RELEASE-007 | PUT `/releases/{releaseId}/rollback?toReleaseId=&operator=` | | 🟢 | | **rollback** |
| F-APO-RELEASE-008 | POST `.../namespaces/{namespaceName}/updateAndPublish` | | 🟢 | | merge+publish |
| F-APO-RELEASE-009 | POST `.../namespaces/{namespaceName}/gray-del-releases` | | 🟢 | | gray delete keys |
| F-APO-RELEASE-010 | GET `.../namespaces/{namespaceName}/lock` | | 🟢 | | ns lock |
| F-APO-RELEASE-011 | GET `.../namespaces/{namespaceName}/commit?key=` | | 🟢 | | commit history |
| F-APO-RELEASE-012 | GET `.../releases/histories` | | 🟢 | | publish history |
| F-APO-RELEASE-013 | GET `/releases/histories/by_release_id_and_operation` | | 🟢 | | |
| F-APO-RELEASE-014 | GET `/releases/histories/by_previous_release_id_and_operation` | | 🟢 | | |

## 7. adminservice — branch (gray) (`F-APO-BRANCH-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-BRANCH-001 | POST `.../namespaces/{namespaceName}/branches?operator=` | | 🟢 | branch_test | real branch model: child Cluster (ParentClusterId>0, timestamp-hex name, ids now assigned) + child Namespace; idempotent (upstream NamespaceBranchService.createBranch) |
| F-APO-BRANCH-002 | GET `.../namespaces/{namespaceName}/branches` | | 🟢 | | list |
| F-APO-BRANCH-003 | DELETE `.../namespaces/{namespaceName}/branches/{branchName}?operator=` | | 🟢 | | delete |
| F-APO-BRANCH-004 | GET `.../branches/{branchName}/rules` | | 🟢 | | gray rules |
| F-APO-BRANCH-005 | PUT `.../branches/{branchName}/rules` | | 🟢 | | **update gray rules** (clientAppId/ip/label) |

## 8. adminservice — instances / accesskey / appnamespace / server-config (`F-APO-ADMSVC-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-ADMSVC-001 | GET `/instances/by-release?releaseId=` | | 🟢 | route_test (audit loop) | release → releaseKey → matching instance configs joined w/ Instance rows |
| F-APO-ADMSVC-002 | GET `/instances/by-namespace?appId=&clusterName=&namespaceName=` | | 🟢 | route_test | audited on every /configs fetch (InstanceConfigAuditUtil port, 2 caches) |
| F-APO-ADMSVC-003 | GET `/instances/by-namespace/count` | | 🟢 | route_test | distinct instance count |
| F-APO-ADMSVC-004 | GET `/instances/by-namespace-and-releases-not-in` | | 🟢 | route_test | configs whose releaseKey not in given ids |
| F-APO-ADMSVC-005 | POST/GET/DELETE `/apps/{appId}/accesskeys` | `create/list/delete_access_key_admin` | 🟢 | | admin paths registered (`admin.rs` `/apps/{app_id}/accesskeys` POST+GET, `/{id}` DELETE) — earlier "NOT registered" note was stale |
| F-APO-ADMSVC-006 | PUT `/apps/{appId}/accesskeys/{id}/enable` / `.../disable` | `enable/disable_access_key` | 🟢 | | registered (`admin.rs` `/apps/{app_id}/accesskeys/{id}/enable|disable` PUT) — earlier "NOT implemented" note was stale |
| F-APO-ADMSVC-007 | POST/GET/DELETE `/apps/{appId}/appnamespaces` | | 🟢 | | appnamespace CRUD |
| F-APO-ADMSVC-008 | GET `/appnamespaces` | | 🟢 | | list public appnamespaces |
| F-APO-ADMSVC-009 | GET `/appnamespaces/{publicNamespaceName}/namespaces` | | 🟢 | | associated namespaces |
| F-APO-ADMSVC-010 | GET `/appnamespaces/{publicNamespaceName}/associated-namespaces/count` | | 🟢 | | |
| F-APO-ADMSVC-011 | GET `/serverconfigs` | | 🟢 | | list server config (path: `/serverconfigs`, not `/server/config/find-all-config`) |
| F-APO-ADMSVC-012 | POST/PUT/DELETE `/serverconfigs/{key}` | | 🟢 | | set/update/delete server config |
| F-APO-ADMSVC-013 | GET `/instance-configs?instanceId=` | | 🟢 | | instance config query (batata-specific) |
| F-APO-ADMSVC-014 | POST `/instances` | | 🟢 | | client instance registration |
| F-APO-ADMSVC-015 | PUT `/instances` | | 🟢 | | client instance heartbeat |
| F-APO-ADMSVC-016 | GET/POST `/configs/{appId}/{cluster}/{ns}/export` / `/configs/import` | | 🟢 | | config import/export (admin path) |
| F-APO-ADMSVC-017 | POST `/configs/sync` / `/configs/sync/app` | | 🟢 | | config sync (batata-specific) |
| F-APO-ADMSVC-018 | GET `/audit` / `/audit/by-entity` | | 🟢 | | audit log query (admin path) |
| F-APO-ADMSVC-019 | GET/POST/DELETE `/favorites` | | 🟢 | | favorites (admin path) |
| F-APO-ADMSVC-020 | GET `/search?key=&value=&appId=&clusterName=` | | 🟢 | | item search (non-standard path) |
| F-APO-ADMSVC-021 | GET `/permissions` | | 🟢 | | list permissions (admin path) |
| F-APO-ADMSVC-022 | Admin token auth | `AdminAuthMiddleware` | 🟢 | | `Authorization: <token>` whitelist vs `APOLLO_ADMIN_SERVICE_ACCESS_TOKENS`; 401 on invalid/missing. Wraps the whole admin scope (`src/route/admin.rs`). |

## 9. portal — OpenAPI v1 apps/envs/clusters/namespaces (`F-APO-PORT-`)

> Auth: ConsumerToken (`Authorization`) OR portal session cookie OR user token; writes additionally need `@PreAuthorize` permission. **The portal UI itself uses these endpoints** (legacy WebAPI deprecated).
>
> **Auth status (2026-09-01):** enforced by `OpenApiAuthMiddleware` (see PMISC-021 🟢) — consumer token or user token required; the session-cookie branch is absent (no portal session store in batata). The `@PreAuthorize` permission layer is degraded to single-tenant (`hasRootPermission`/`hasPermission` always true, see PMISC-007/008).

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-PORT-001 | POST `/openapi/v1/apps` | | 🟢 | | create app (auth: see PMISC-021) |
| F-APO-PORT-002 | GET `/openapi/v1/apps?appIds=` | | 🟢 | | list |
| F-APO-PORT-003 | GET `/openapi/v1/apps/authorized` | | 🟢 | p5_batch2_test | batata is single-tenant: returns all apps (auth model excluded this round) |
| F-APO-PORT-004 | GET `/openapi/v1/apps/by-self?page=&size=` | | 🟢 | p5_batch2_test | same as authorized under single-tenant model |
| F-APO-PORT-005 | GET `/openapi/v1/apps/{appId}` | | 🟢 | | |
| F-APO-PORT-006 | PUT `/openapi/v1/apps/{appId}?operator=` | | 🟢 | | |
| F-APO-PORT-007 | DELETE `/openapi/v1/apps/{appId}?operator=` | | 🟢 | | super admin |
| F-APO-PORT-008 | GET `/openapi/v1/apps/{appId}/envclusters` | | 🟢 | | env→clusters |
| F-APO-PORT-009 | GET `/openapi/v1/apps/{appId}/env-cluster-info` | `openapi_env_cluster_info` | 🟢 | p6_portal_misc_test | returns `EnvClusterInfoDTO` (one entry per canonical env DEV/FAT/UAT/PRO, each with the app's root clusters). Single-tenant: same cluster list under every env the app has namespaces in. |
| F-APO-PORT-010 | GET `/openapi/v1/apps/{appId}/miss-envs` | `openapi_miss_envs` | 🟢 | | returns envs the app has NOT been created in (complement of envs with ≥1 namespace). batata keeps namespaces across all envs uniformly, so an app with any namespace reports no missing envs. |
| F-APO-PORT-011 | POST `/openapi/v1/apps/envs/{env}` | `openapi_create_app_env` | 🟢 | | creates the requested app namespaces under the default cluster. Body is `List<AppNamespaceDTO>`; single-tenant degradation (no per-env cluster separation). |
| F-APO-PORT-012 | GET `/apps/search/by-appid-or-name` | | 🟢 | live smoke | registered at the UPSTREAM portal-root path; id/name contains, paged |
| F-APO-PORT-013 | GET `/openapi/v1/envs` | | 🟢 | | env list |
| F-APO-PORT-014 | POST `/openapi/v1/envs/{env}/apps/{appId}/clusters` | | 🟢 | | create cluster |
| F-APO-PORT-015 | GET `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}` | | 🟢 | | |
| F-APO-PORT-016 | DELETE `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}?operator=` | | 🟢 | | |
| F-APO-PORT-017 | GET `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces` | | 🟢 | | list |
| F-APO-PORT-018 | GET `.../namespaces/{namespaceName}` | | 🟢 | | with items |
| F-APO-PORT-019 | DELETE `/openapi/v1/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}?operator=` | | 🟢 | | unlink |
| F-APO-PORT-020 | GET `.../namespaces/{namespaceName}/lock` | | 🟢 | | returns lock info or null (NamespaceLockPersistence) |
| F-APO-PORT-021 | POST `/openapi/v1/namespaces` | | 🟢 | | create namespace via OpenAPI |
| F-APO-PORT-022 | GET/POST/DELETE `/openapi/v1/appnamespaces` | | 🟢 | | public catalog page + create + delete |
| F-APO-PORT-023 | GET `/openapi/v1/apps/{appId}/appnamespaces` | | 🟢 | | |
| F-APO-PORT-024 | GET `/openapi/v1/apps/{appId}/appnamespaces/{namespaceName}/usage` | `openapi_appnamespace_usage` | 🟢 | | returns `AppNamespaceUsageDTO` (envs/clusters/usedBy derived from the namespaces table). Single-tenant: every usage maps to all 4 canonical envs. |
| F-APO-PORT-025 | GET `/openapi/v1/apps/{appId}/namespaces/releases/status` | | 🟢 | p5_batch2_test | frontend getNamespacePublishInfo — same payload as ADM-019 publish_info |
| F-APO-PORT-026 | GET `.../missing-namespaces` | | 🟡 | p5_batch2_test | public namespaces not yet linked to cluster (GET done; POST semantics pending) |

## 10. portal — items (`F-APO-PITEM-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-PITEM-001 | GET `.../namespaces/{namespaceName}/items?page=&size=` | | 🟢 | | paged (auth: see PMISC-021) |
| F-APO-PITEM-002 | POST `.../namespaces/{namespaceName}/items` | | 🟢 | | create |
| F-APO-PITEM-003 | PUT `.../namespaces/{namespaceName}/items` | | 🟢 | p5_batch2_test | full properties text → diff apply create/update/delete in one save (portal editor flow) |
| F-APO-PITEM-004 | GET `.../items/{key}` | | 🟢 | | read |
| F-APO-PITEM-005 | PUT `.../items/{key}?createIfNotExists=&operator=` | | 🟢 | | update |
| F-APO-PITEM-006 | DELETE `.../items/{key}?operator=` | | 🟢 | | delete |
| F-APO-PITEM-007 | GET/PUT/DELETE `.../encodedItems/{key}` | | 🟢 | p5_batch2_test | url-safe base64 key decode + item ops |
| F-APO-PITEM-008 | GET `.../branches/{branchName}/items` | | 🟢 | p5_batch2_test | branch namespace items via real branch model |
| F-APO-PITEM-009 | POST `.../items/diff` | `openapi_items_diff` → `ConfigSyncService::compare` | 🟢 | p6_portal_misc_test | POST `/openapi/v1/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}/items/diff`. Returns one `ItemDiffs` (create/update/delete) per target namespace. Faithful to `portal/ItemController.java:180-203`. |
| F-APO-PITEM-010 | POST `.../items/synchronize` | `openapi_items_synchronize` → `ConfigSyncService::synchronize` | 🟢 | p6_portal_misc_test | Upstream is **PUT** `/openapi/v1/apps/{appId}/namespaces/{namespaceName}/items` (no env/cluster in path). Applies create/update/delete change sets into each target namespace; returns per-namespace item lists. |
| F-APO-PITEM-011 | POST `.../items/validation` | | 🟢 | p5_batch2_test | properties parse: comments/kv/dup-key/bad-line → 400 with line info |
| F-APO-PITEM-012 | POST `.../items/revocation?operator=` | | 🟢 | p5_batch2_test | restore items to latest release state (delete extras / revert values / recreate removed) |

## 11. portal — releases / branch / history (`F-APO-PREL-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-PREL-001 | POST `.../namespaces/{namespaceName}/releases` | | 🟢 | | **core**: publish |
| F-APO-PREL-002 | GET `.../namespaces/{namespaceName}/releases/latest` | | 🟢 | | |
| F-APO-PREL-003 | GET `.../namespaces/{namespaceName}/releases/active` | | 🟢 | | |
| F-APO-PREL-004 | GET `/openapi/v1/envs/{env}/releases/{releaseId}` | | 🟢 | | |
| F-APO-PREL-005 | PUT `/openapi/v1/envs/{env}/releases/{releaseId}/rollback?toReleaseId=&operator=` | | 🟢 | | rollback |
| F-APO-PREL-006 | GET `/openapi/v1/envs/{env}/releases/comparison` | | 🟢 | | compare |
| F-APO-PREL-007 | POST `.../namespaces/{namespaceName}/branches/{branchName}/releases` | | 🟢 | | **gray publish** |
| F-APO-PREL-008 | POST `.../branches/{branchName}/gray-del-releases` | | 🟢 | | gray delete keys |
| F-APO-PREL-009 | POST `.../branches/{branchName}/merge?deleteBranch=` | | 🟢 | | merge |
| F-APO-PREL-010 | GET/POST `.../namespaces/{namespaceName}/branches` | | 🟢 | | list/create |
| F-APO-PREL-011 | DELETE `.../branches/{branchName}?operator=` | | 🟢 | | |
| F-APO-PREL-012 | GET/PUT `.../branches/{branchName}/rules` | | 🟢 | | gray rules |
| F-APO-PREL-013 | GET `.../namespaces/{namespaceName}/commits` | | 🟢 | | commit history |
| F-APO-PREL-014 | GET `.../releases/histories` | | 🟢 | | publish history |

## 12. portal — instances / accesskey / perms / users / misc (`F-APO-PMISC-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-PMISC-001 | GET `.../namespaces/{namespaceName}/instances` | `openapi_list_instances` → `InstanceService::list_open_instances` | 🟢 | p6_portal_misc_test | paged `OpenInstancePageDTO` (default size 20). Path includes `appId` for batata's storage key (`configAppId`+`cluster`+`namespace`); `env` accepted for upstream parity, ignored. |
| F-APO-PMISC-002 | GET `/openapi/v1/envs/{env}/instances/by-release` | `openapi_list_instances_by_release` → `InstanceService::list_open_instances_by_release` | 🟢 | | comma-separated `releaseIds`; 404 (`findReleaseOrThrow`) if any requested release id is missing. |
| F-APO-PMISC-003 | GET `/openapi/v1/envs/{env}/instances/by-namespace` | `openapi_count_instances` → `InstanceService::count_open_instances` | 🟢 | | returns the distinct instance config count (JSON number). |
| F-APO-PMISC-004 | GET `/openapi/v1/envs/{env}/instances/by-namespace-and-releases-not-in` | `openapi_list_instances_by_release_not_in` → `InstanceService::list_open_instances_by_release_not_in` | 🟢 | | instance configs whose release is NOT in `releaseIds`; 404 on a missing release id. |
| F-APO-PMISC-005 | POST/GET/DELETE `/openapi/v1/apps/{appId}/envs/{env}/accesskeys` | | 🟡 | | create/list/delete (no enable/disable) |
| F-APO-PMISC-006 | PUT `.../accesskeys/{accessKeyId}/activation` / `.../deactivation` | | 🟢 | | enable/disable implemented (openapi.rs activation/deactivation routes registered) |
| F-APO-PMISC-007 | GET `/openapi/v1/permissions/root` | `openapi_permissions_root` | 🟢 | p6_portal_misc_test | single-tenant degradation: always `{hasRootPermission: true}`. No auth system in batata; see `src/auth/permission.rs`. |
| F-APO-PMISC-008 | GET `/openapi/v1/apps/{appId}/permissions/{permissionType}` | `openapi_permissions_app` | 🟢 | p6_portal_misc_test | single-tenant degradation: always `{hasPermission: true}`. |
| F-APO-PMISC-009 | GET/POST/DELETE `/openapi/v1/apps/{appId}/roles/{roleType}` | `openapi_roles` | 🟢 | p6_portal_misc_test | GET returns `RoleDTO` (roleName = `{ROLETYPE}-{appId}`, empty members). Single-tenant degradation: no role store populated. |
| F-APO-PMISC-010 | GET `/openapi/v1/organizations` | `openapi_organizations` | 🟢 | p6_portal_misc_test | returns a fixed default organization `[{orgId:"1", orgName:"default"}]`. Single-tenant degradation. |
| F-APO-PMISC-011 | GET `/openapi/v1/user` | `openapi_current_user` | 🟢 | p6_portal_misc_test | fixed admin user `{username:"apollo", roles:["ROLE_ADMIN"], ...}`. Single-tenant degradation. |
| F-APO-PMISC-012 | GET/POST/PUT/DELETE `/openapi/v1/users` | `openapi_list/create/update/delete_user` | 🟢 | p6_portal_misc_test | user CRUD over `UserPersistence`; always includes the fixed admin entry on list. |
| F-APO-PMISC-013 | GET/POST/DELETE `/openapi/v1/user-tokens` | | ⚪ | | DB schema only, no API endpoints |
| F-APO-PMISC-014 | GET `/openapi/v1/consumers` / consumer-tokens | | 🟡 | | open platform |
| F-APO-PMISC-015 | GET/POST `/openapi/v1/configs/import|export` | | ⚪ | | NOT at OpenAPI path (exists at admin path) |
| F-APO-PMISC-016 | GET `/openapi/v1/system-info` | `openapi_system_info` | 🟢 | p6_portal_misc_test | returns `{apolloVersion, gitCommitId}` from cargo env. Single-tenant degradation (no upstream version). |
| F-APO-PMISC-017 | GET/POST `/openapi/v1/server/portal-db/config*` | | 🟡 | | server config |
| F-APO-PMISC-018 | GET `/openapi/v1/apollo/audit/*` | | ⚪ | | NOT at OpenAPI path (exists at admin path) |
| F-APO-PMISC-019 | GET `/openapi/v1/favorites` | | ⚪ | | NOT at OpenAPI path (exists at admin path) |
| F-APO-PMISC-020 | GET `/openapi/v1/global-search/item-info/by-key-or-value` | | ⚪ | | NOT at OpenAPI path (exists at `/search`) |
| F-APO-PMISC-021 | Portal token / session auth | `OpenApiAuthMiddleware` | 🟢 | | Three mechanisms, all enforced: ① ConsumerToken — raw `Authorization` header looked up in `apollo_consumer_token` + appId match; ② user token (portal-management paths) — `UserTokenService::validate` (sha256 + prefix); ③ session cookie — **not** implemented (degraded: no portal session store in batata), so portal UI traffic must use a consumer or user token. `/openapi/v1/user/login` is public. See `src/middleware/auth.rs`. |

## 13. portal — legacy WebAPI (`F-APO-LEGACY-`) (deprecated, low priority)

> `@Deprecated` compatibility surface, NOT the UI path. Skip unless a legacy client needs it.

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-LEGACY-001 | GET `/apps`, `/apps/{appId}`, `/apps/{appId}/navtree`, `/apps/{appId}/miss_envs` | | ⚪ | | |
| F-APO-LEGACY-002 | POST/PUT/DELETE `/apps`, `/apps/{appId}`, `/apps/envs/{env}` | | ⚪ | | |
| F-APO-LEGACY-003 | GET `/envs` | | ⚪ | | |
| F-APO-LEGACY-004 | POST/DELETE/GET `apps/{appId}/envs/{env}/clusters(/...)` | | ⚪ | | |
| F-APO-LEGACY-005 | GET `/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces(/...)` | | ⚪ | | |
| F-APO-LEGACY-006 | POST `/apps/{appId}/namespaces` (batch) | | ⚪ | | |
| F-APO-LEGACY-007 | GET/POST/DELETE `/apps/{appId}/appnamespaces(/...)`, `/appnamespaces/public` | | ⚪ | | |
| F-APO-LEGACY-008 | GET/POST/PUT/DELETE `.../items`, `/items/{itemId}` | | ⚪ | | |
| F-APO-LEGACY-009 | PUT `.../items` (text bulk) | | ⚪ | | |
| F-APO-LEGACY-010 | POST `.../syntax-check`, PUT `.../revoke-items` | | ⚪ | | |
| F-APO-LEGACY-011 | POST `.../releases`, `.../branches/{branchName}/releases` | | ⚪ | | publish/gray |
| F-APO-LEGACY-012 | PUT `/envs/{env}/releases/{releaseId}/rollback` | | ⚪ | | |
| F-APO-LEGACY-013 | GET `.../releases/all|active`, `/envs/{env}/releases/{releaseId}` | | ⚪ | | |
| F-APO-LEGACY-014 | GET/POST/DELETE/PUT `.../branches(/...)`, `/merge`, `/rules` | | ⚪ | | |
| F-APO-LEGACY-015 | GET `.../commits`, `.../releases/histories` | | ⚪ | | |
| F-APO-LEGACY-016 | GET `.../lock` | | ⚪ | | ns lock |
| F-APO-LEGACY-017 | GET `.../instances/by-*` | | ⚪ | | |
| F-APO-LEGACY-018 | POST/GET/DELETE `/consumers(/...)` | | ⚪ | | open platform |
| F-APO-LEGACY-019 | POST/GET/PUT `/users`, `/user` | | ⚪ | | |
| F-APO-LEGACY-020 | GET/POST/DELETE `/server/*` config | | ⚪ | | |
| F-APO-LEGACY-021 | GET/POST `/configs/import|export` | | ⚪ | | |
| F-APO-LEGACY-022 | GET `/signin`, `/sso_heartbeat` | | ⚪ | | |

---

# Summary (auto-updated)

| Section | 🟢 | 🟡 | ⚡ | ⚪ | ⛔ | Total | Impl rate |
|---------|----|----|----|----|----|-------|-----------|
| 1 configservice fetch | 7 | 0 | 0 | 0 | 0 | 7 | 100% |
| 2 long polling | 2 | 0 | 0 | 0 | 0 | 2 | 100% |
| 3 metaservice | 3 | 0 | 0 | 0 | 0 | 3 | 100% |
| 4 adminservice apps/ns | 19 | 0 | 0 | 0 | 0 | 19 | 100% |
| 5 adminservice items | 8 | 1 | 0 | 3 | 0 | 12 | 71% |
| 6 adminservice release | 14 | 0 | 0 | 0 | 0 | 14 | 100% |
| 7 adminservice branch | 5 | 0 | 0 | 0 | 0 | 5 | 100% |
| 8 adminservice misc | 22 | 0 | 0 | 0 | 0 | 22 | 100% |
| 9 portal apps/ns | 25 | 1 | 0 | 0 | 0 | 26 | 98% |
| 10 portal items | 12 | 0 | 0 | 0 | 0 | 12 | 100% |
| 11 portal releases | 14 | 0 | 0 | 0 | 0 | 14 | 100% |
| 12 portal misc | 13 | 3 | 0 | 5 | 0 | 21 | 69% |
| 13 legacy webapi | 0 | 0 | 0 | 22 | 0 | 22 | 0% |
| **Total** | 144 | 5 | 0 | 30 | 0 | 179 | 82% |

> Impl rate = (🟢 + 0.5×🟡) / Total. 2026-08-25 (P0–P5): LONGPOL-001/002, CFGSVC-007, META-001~003, ADMSVC-001..004, PORT-020/021/022 verified 🟢 after faithful-porting work (persisted notifications, enforced AccessKey, database-discovery metaservice + instance audit, real branch-entity model, OpenAPI namespace/appnamespace/lock endpoints).
>
> 2026-08-26 live verification (native server on :8080): curl suite 9/9 incl. real-restart notification catch-up; Java SDK 33/33 (A1 long-poll, A2 public-ns, A3 signed fetch, A4 branch+gray-by-label). Fixes surfaced by live runs: app bootstrap now creates default Cluster + `application` Namespace; AppNamespace creation instantiates Namespace across root clusters; gray-rule ReleaseId resolves by Release **row pk** (upstream semantics) with legacy-column fallback; `config_app_id` folded into the original instance_config migration (no new version — project unreleased).
>
> 2026-08-26 SQL-backend verification (podman mysql:8.0 :3307 / postgres:16 :5433): persistence suite green on BOTH backends after two real PG-only bugs fixed — ① `Stored*→ActiveModel` wrote explicit `id: Set(0)` so PostgreSQL honored it literally (MySQL auto-increment silently replaced 0, masking the bug); now `NotSet` when 0 so DB assigns ids. ② sea-orm maps Rust `i8` to PG `"char"`, clashing with SMALLINT columns (`item.type`, `access_key.mode`, `release_history.operation`) — all widened to `i16`. Also: `PUT items/{x}` now falls back to by-id (upstream itemId semantics), lock response carries `lockedBy`; config_service_test live trio green. Legacy WebAPI (Section 13, deprecated upstream) intentionally not implemented.
2026-08-26 batch2: PITEM-003/007/008/011/012 + PORT-003/004/025/026 verified via in-process suite (`p5_batch2_test`, 27 Rust tests green); PORT-010 miss-envs & PORT-024 usage deferred — upstream response DTOs live in the generated openapi-java artifact, not extractable locally. Deprecated policy: v1 `/notifications` removed.
>
> 2026-08-31 batch3 (single-tenant degradation round): implemented PMISC-001~004 (openapi instances: paged envelope + by-release with 404-on-missing-release + by-namespace count + by-release-not-in), PITEM-009 (`items/diff` → `ConfigSyncService::compare`, one `ItemDiffs` per target namespace) and PITEM-010 (`items/synchronize` — upstream is **PUT** `/apps/{appId}/namespaces/{namespaceName}/items`), PORT-009/010/011/024 (env-cluster-info / miss-envs / create-app-env / appnamespace-usage), and PMISC-007~012/016 (permission/role/organization/user/system-info degraded to faithful single-tenant payloads; `auth/` module wired). New DTOs (`OpenInstanceDTO/PageDTO`, `ItemDiffs`, `NamespaceSyncModel`, `EnvClusterInfoDTO`, `AppNamespaceUsageDTO`, `OrganizationDTO`, `SystemInfoDTO`) + `InstanceConfigPersistence` paging/filter/by-release methods + `InstancePersistence::get_instance_by_id` added across both backends. `UserPersistence`/`ConsumerRolePersistence` re-exported from `traits/mod.rs` so the wired `auth` module resolves. Verified in-process via `p6_portal_misc_test`. Note: PMISC-013/015/018/019/020/021 (user-tokens, import/export, audit, favorites, global-search, portal-token/session auth) remain out of scope this round — see PMISC-021 for the auth blocker.
Orphan-migration disposition: `apollo_service_registry` now wired (metaservice); `apollo_users/user_token/user_token_audit/authorities/audit_log/audit_log_data_influence/consumer_audit/consumer_role` remain schema-only — tied to portal auth (excluded this round) and audit UI, revisit with PMISC auth work.
>
> 2026-09-01 auth round (blocking items closed): **ADMSVC-022 ⚪→🟢** and **PMISC-021 ⚪→🟢**, implemented faithfully (no degradation of the enforcement path). ① `AdminAuthMiddleware` wraps the whole admin scope and enforces the `Authorization: <token>` whitelist from `APOLLO_ADMIN_SERVICE_ACCESS_TOKENS` (401 on missing/invalid) — port of upstream `AdminServiceAuthenticationFilter`. ② `OpenApiAuthMiddleware` wraps the whole openapi scope with three branches: `/openapi/v1/user/login` is public; portal-management paths require a user token verified by `UserTokenService::validate` (sha256 hash + 32-char prefix lookup, new `apollo_user_token` table + `UserTokenPersistence` on both backends); all other paths require a ConsumerToken — the **raw** `Authorization` header is looked up in `apollo_consumer_token` and cross-checked against the consumer's appId, matching upstream `ConsumerAuthenticationFilter` (an earlier assumption that the token was HMAC-derived was corrected against upstream source). Consumer tokens are minted by `generate_consumer_token` as `"{consumerId}-{uuid}"`, identical to upstream `ConsumerTokenService.createToken`. New files: `src/middleware/auth.rs`, `src/service/user_token_service.rs`, `src/entity/apollo_user_token.rs`; `AuthConfig` added to `src/model/config.rs` and injected via `web::Data` in `src/plugin.rs`. **Degradation (documented, not silent):** batata has no portal session store, so the upstream session-cookie branch is absent — portal UI traffic must present a consumer or user token. PMISC-013 (`/user-tokens` CRUD) stays ⚪: the entity/service/persistence exist but no route is registered yet.
>
> 2026-09-01 type-round: **all Apollo primary keys and integer foreign keys widened `i32 → i64`** across the entity layer (29 entities), `Stored*` structs, `IdGenerator` (`AtomicI32 → AtomicI64`), DTOs, persistence traits (incl. `Arc<T>` forwarding impls), and route signatures (`web::Path<i32>` → `i64`, `as i32` casts removed, `HashMap<i32,…>`/`Vec<i32>` id collections retyped). This also closed two pre-existing `i32`/`i64` mismatches (`apollo_gray_release_rule.release_id`, `apollo_release.release_id` self-reference). Critically, the migration helper `unsigned_int` / `unsigned_int_null` now emit **`BIGINT`** instead of 4-byte `INTEGER` on both MySQL and PostgreSQL — without this, i64 ids would overflow once past `i32::MAX`. (Note: PostgreSQL has no `UNSIGNED` integer types at all; despite the MySQL-inherited names, these helpers never emitted `UNSIGNED` on PG.) Existing databases must be rebuilt for the widened columns to take effect. One latent bug surfaced by the widening and fixed: `src/auth/permission.rs` was passing a `permission_id` into `list_permission_by_type(permission_type)` — type-coincidental under `i32` and now corrected via a new `PermissionPersistence::get_permission_by_id`. Full crate compiles with 0 errors.
>
> 2026-09-01 Summary recount (important): the Summary table had **drifted badly from the per-item rows** and its Totals were not even the sum of their own columns (it claimed 123 🟢 / 36 ⚪ while the section rows added to 115 / 44). Rebuilt the whole table by counting the status column of all 179 feature rows: **144 🟢 / 5 🟡 / 30 ⚪ → impl rate 82%**, not the previously advertised 74%. The largest corrections are Section 8 (adminservice misc, was 16🟢/1🟡/5⚪ → actually 22🟢, 100%) and Section 12 (portal misc, was 1🟢/13🟡/7⚪ → actually 13🟢/3🟡/5⚪, 69%); Section 4 is 100%, Section 9 is 98%. The long-polling ⛔ entry was a stale marker — LONGPOL-001/002 are both 🟢. **Read the per-item tables as the source of truth**; the Summary is derived from them and must be recounted whenever markers change.
>
> 2026-09-01 Java-SDK round (`sdk-tests/apollo-java-tests`, Apollo OpenAPI client 2.5.0): full suite **33/33 green** against the live server (current binary, MySQL backend). Two real bugs it surfaced and fixed: ① `AccessKeyService::create` left `data_change_last_modified_by` NULL, rejected by the NOT NULL column — now seeded with the operator, matching upstream (portal OpenAPI `createAccessKey` sets both creator and modifier). ② gray/merge publish generated `release_key` strings exceeding the 64-char column (app+cluster+ns+timestamp+suffix) → MySQL error 1406; the `apollo_release.release_key` column was widened to 256 (folded into the existing migration; unpublished project). Rust suites re-verified after the fixes: embedded 34/0, MySQL 34/0.
>
> 2026-09-01 semantic-column round: all `i16` semantic columns (item.type, access_key.mode, release_history.operation, gray_release_rule.branch_status + the ReleaseOperation/BRANCH_STATUS consts) widened to **`i32`**, mirroring the `i64` id round — the integer type system is now exactly two tiers (`i64` ids, `i32` semantics) and the `tiny_int`/`unsigned_tiny_int` helpers are retired in favor of `signed_int`/`signed_int_null` (4-byte `INTEGER` on both backends). This removes the last MySQL/PG column-type fork and the i8→"char" class of pitfalls. The persistence_test suite was made rerun-safe against persistent SQL databases (consumer/permission/namespace/release keys timestamped per run — their UNIQUE constraints and no-cleanup pattern collided on second run). The live-server suite (`config_service_test`) now builds its reqwest client with `no_proxy()`: reqwest 0.12+ picks up the macOS system proxy by default and ignores its `localhost` bypass entry, so a system proxy turns every request into a 502. Full regression: embedded 34/0, MySQL (fresh schema) 34/0, PostgreSQL (fresh schema) 34/0, live-server suite 3/3. Pre-existing SQL databases must be rebuilt for the widened columns.
>
> 2026-09-01 doc-audit correction (markers vs. code): **ADMSVC-005 🟡→🟢** and **ADMSVC-006 ⚪→🟢** — both access-key route groups are in fact registered on the admin path (`admin.rs`: `/apps/{app_id}/accesskeys` POST+GET, `/{id}` DELETE, `/{id}/enable` and `/{id}/disable` PUT); the "NOT registered / NOT implemented" notes were stale. Section 4 impl rate 84% → 95%. This is the same class of drift the audit phase flagged (markers based on assumed rather than verified state), so remaining ⚪/🟡 cells should still be re-checked against the route tables before being treated as gaps.

> Impl rate = (🟢 + 0.5×🟡) / Total. 2026-08-25 calibration: CFGSVC-006/PMISC-006 verified implemented ⚪→🟢; CFGSVC-007 AccessKey exists but opt-in-only ⚪→🟡; META-001~003 placeholder host-echo ⚪→🟡; LONGPOL notes corrected to reflect actual hold-behavior.

> 2026-08-10 correction: Section 9 — 14 portal apps/ns endpoints upgraded 🟡→🟢 (routes exist, handlers work; auth tracked separately as PMISC-021); 11 endpoints downgraded 🟡→⚪ (routes not registered). Section 10 — 5 portal item endpoints upgraded 🟡→🟢 (routes exist); 7 remain 🟡 (routes not registered, noted individually). Legacy WebAPI (Section 13) confirmed @Deprecated upstream with no SDK client dependency — skip all 22.
