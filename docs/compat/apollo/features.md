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
| F-APO-CFGSVC-006 | GET `/configfiles/raw/{appId}/{clusterName}/{namespace}` | | ⚪ | | raw content; content-type by ns suffix |
| F-APO-CFGSVC-007 | AccessKey signature auth (HMAC-SHA1) | | ⚪ | | intercepts `/configs` `/configfiles` `/notifications` |

## 2. configservice — long polling (`F-APO-LONGPOL-`)

> Apollo's core change-notification mechanism. No `waitTime` param — server holds `longPollingTimeoutInMilli` (default 90s); timeout → **304**.

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-LONGPOL-001 | GET `/notifications/v2` | | 🟡 | | **core**: returns correct 200/304 but does NOT hold request (short-poll, not long-poll) |
| F-APO-LONGPOL-002 | GET `/notifications` (v1, deprecated) | | 🟡 | | delegates to same handler as v2; not distinct v1 behavior |

## 3. configservice — metaservice (`F-APO-META-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-META-001 | GET `/services/config?appId=&ip=` | | ⚪ | | configservice instances |
| F-APO-META-002 | GET `/services/admin` | | ⚪ | | adminservice instances |
| F-APO-META-003 | GET `/` | | ⚪ | | both (non-Eureka discovery only) |

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
| F-APO-ADM-011 | GET `/apps/{appId}/cluster/{clusterName}/unique` | | ⚪ | | cluster name uniqueness — NOT implemented |
| F-APO-ADM-012 | POST `/apps/{appId}/clusters/{clusterName}/namespaces` | | 🟢 | | create namespace |
| F-APO-ADM-013 | GET `/apps/{appId}/clusters/{clusterName}/namespaces` | | 🟢 | | list |
| F-APO-ADM-014 | GET `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}` | | 🟢 | | read |
| F-APO-ADM-015 | DELETE `/apps/{appId}/clusters/{clusterName}/namespaces/{namespaceName}?operator=` | | 🟢 | | delete |
| F-APO-ADM-016 | GET `/namespaces/{namespaceId}` | | 🟢 | | by id |
| F-APO-ADM-017 | GET `/namespaces/find-by-item?itemKey=` | | ⚪ | | reverse lookup — NOT implemented |
| F-APO-ADM-018 | GET `.../associated-public-namespace` | | ⚪ | | NOT implemented |
| F-APO-ADM-019 | GET `/apps/{appId}/namespaces/publish_info` | | ⚪ | | per-cluster publish state — NOT implemented |

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
| F-APO-BRANCH-001 | POST `.../namespaces/{namespaceName}/branches?operator=` | | 🟢 | | create gray branch |
| F-APO-BRANCH-002 | GET `.../namespaces/{namespaceName}/branches` | | 🟢 | | list |
| F-APO-BRANCH-003 | DELETE `.../namespaces/{namespaceName}/branches/{branchName}?operator=` | | 🟢 | | delete |
| F-APO-BRANCH-004 | GET `.../branches/{branchName}/rules` | | 🟢 | | gray rules |
| F-APO-BRANCH-005 | PUT `.../branches/{branchName}/rules` | | 🟢 | | **update gray rules** (clientAppId/ip/label) |

## 8. adminservice — instances / accesskey / appnamespace / server-config (`F-APO-ADMSVC-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-ADMSVC-001 | GET `/instances/by-release?releaseId=` | | ⚪ | | NOT implemented |
| F-APO-ADMSVC-002 | GET `/instances/by-namespace?appId=&clusterName=&namespaceName=` | | ⚪ | | NOT implemented |
| F-APO-ADMSVC-003 | GET `/instances/by-namespace/count` | | ⚪ | | NOT implemented |
| F-APO-ADMSVC-004 | GET `/instances/by-namespace-and-releases-not-in` | | ⚪ | | NOT implemented |
| F-APO-ADMSVC-005 | POST/GET/DELETE `/apps/{appId}/accesskeys` | | 🟡 | | adminservice paths NOT registered; only OpenAPI v1 has create/list/delete |
| F-APO-ADMSVC-006 | PUT `/apps/{appId}/accesskeys/{id}/enable` / `.../disable` | | ⚪ | | enable/disable NOT implemented |
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
| F-APO-ADMSVC-022 | Admin token auth | | ⚪ | | `Authorization` whitelist — NOT implemented |

## 9. portal — OpenAPI v1 apps/envs/clusters/namespaces (`F-APO-PORT-`)

> Auth: ConsumerToken (`Authorization`) OR portal session cookie OR user token; writes additionally need `@PreAuthorize` permission. **The portal UI itself uses these endpoints** (legacy WebAPI deprecated).

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-PORT-001 | POST `/openapi/v1/apps` | | 🟡 | | create app (no auth enforcement) |
| F-APO-PORT-002 | GET `/openapi/v1/apps?appIds=` | | 🟡 | | list |
| F-APO-PORT-003 | GET `/openapi/v1/apps/authorized` | | 🟡 | | |
| F-APO-PORT-004 | GET `/openapi/v1/apps/by-self?page=&size=` | | 🟡 | | |
| F-APO-PORT-005 | GET `/openapi/v1/apps/{appId}` | | 🟡 | | |
| F-APO-PORT-006 | PUT `/openapi/v1/apps/{appId}?operator=` | | 🟡 | | |
| F-APO-PORT-007 | DELETE `/openapi/v1/apps/{appId}?operator=` | | 🟡 | | super admin |
| F-APO-PORT-008 | GET `/openapi/v1/apps/{appId}/envclusters` | | 🟡 | | env→clusters |
| F-APO-PORT-009 | GET `/openapi/v1/apps/{appId}/env-cluster-info` | | 🟡 | | |
| F-APO-PORT-010 | GET `/openapi/v1/apps/{appId}/miss-envs` | | 🟡 | | |
| F-APO-PORT-011 | POST `/openapi/v1/apps/envs/{env}` | | 🟡 | | |
| F-APO-PORT-012 | GET `/openapi/v1/apps/search/by-appid-or-name` | | 🟡 | | |
| F-APO-PORT-013 | GET `/openapi/v1/envs` | | 🟡 | | env list |
| F-APO-PORT-014 | POST `/openapi/v1/envs/{env}/apps/{appId}/clusters` | | 🟡 | | create cluster |
| F-APO-PORT-015 | GET `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}` | | 🟡 | | |
| F-APO-PORT-016 | DELETE `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}?operator=` | | 🟡 | | |
| F-APO-PORT-017 | GET `/openapi/v1/envs/{env}/apps/{appId}/clusters/{clusterName}/namespaces` | | 🟡 | | list |
| F-APO-PORT-018 | GET `.../namespaces/{namespaceName}` | | 🟡 | | with items |
| F-APO-PORT-019 | DELETE `/openapi/v1/apps/{appId}/envs/{env}/clusters/{clusterName}/namespaces/{namespaceName}?operator=` | | 🟡 | | unlink |
| F-APO-PORT-020 | GET `.../namespaces/{namespaceName}/lock` | | 🟡 | | ns lock |
| F-APO-PORT-021 | POST `/openapi/v1/namespaces` | | 🟡 | | batch create |
| F-APO-PORT-022 | GET/POST/DELETE `/openapi/v1/appnamespaces` | | 🟡 | | public appnamespace |
| F-APO-PORT-023 | GET `/openapi/v1/apps/{appId}/appnamespaces` | | 🟡 | | |
| F-APO-PORT-024 | GET `/openapi/v1/apps/{appId}/appnamespaces/{namespaceName}/usage` | | 🟡 | | |
| F-APO-PORT-025 | GET `/openapi/v1/apps/{appId}/namespaces/releases/status` | | 🟡 | | per-env publish status |
| F-APO-PORT-026 | GET/POST `.../missing-namespaces` | | ⚪ | | NOT implemented |

## 10. portal — items (`F-APO-PITEM-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-APO-PITEM-001 | GET `.../namespaces/{namespaceName}/items?page=&size=` | | 🟡 | | paged |
| F-APO-PITEM-002 | POST `.../namespaces/{namespaceName}/items` | | 🟡 | | create |
| F-APO-PITEM-003 | PUT `.../namespaces/{namespaceName}/items` | | 🟡 | | bulk text update |
| F-APO-PITEM-004 | GET `.../items/{key}` | | 🟡 | | read |
| F-APO-PITEM-005 | PUT `.../items/{key}?createIfNotExists=&operator=` | | 🟡 | | update |
| F-APO-PITEM-006 | DELETE `.../items/{key}?operator=` | | 🟡 | | delete |
| F-APO-PITEM-007 | GET/PUT/DELETE `.../encodedItems/{key}` | | 🟡 | | base64 key |
| F-APO-PITEM-008 | GET `.../branches/{branchName}/items` | | 🟡 | | branch items |
| F-APO-PITEM-009 | POST `.../items/diff` | | 🟡 | | multi-ns diff |
| F-APO-PITEM-010 | POST `.../items/synchronize` | | 🟡 | | sync |
| F-APO-PITEM-011 | POST `.../items/validation` | | 🟡 | | syntax check |
| F-APO-PITEM-012 | POST `.../items/revocation?operator=` | | 🟡 | | revoke unpublished |

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
| F-APO-PMISC-001 | GET `.../namespaces/{namespaceName}/instances` | | 🟡 | | ns instance count |
| F-APO-PMISC-002 | GET `/openapi/v1/envs/{env}/instances/by-release` | | 🟡 | | |
| F-APO-PMISC-003 | GET `/openapi/v1/envs/{env}/instances/by-namespace` | | 🟡 | | |
| F-APO-PMISC-004 | GET `/openapi/v1/envs/{env}/instances/by-namespace-and-releases-not-in` | | 🟡 | | |
| F-APO-PMISC-005 | POST/GET/DELETE `/openapi/v1/apps/{appId}/envs/{env}/accesskeys` | | 🟡 | | create/list/delete (no enable/disable) |
| F-APO-PMISC-006 | PUT `.../accesskeys/{accessKeyId}/activation` / `.../deactivation` | | ⚪ | | NOT implemented |
| F-APO-PMISC-007 | GET `/openapi/v1/permissions/root` | | 🟡 | | is super admin |
| F-APO-PMISC-008 | GET `/openapi/v1/apps/{appId}/permissions/{permissionType}` | | 🟡 | | has permission |
| F-APO-PMISC-009 | GET/POST/DELETE `/openapi/v1/apps/{appId}/roles/{roleType}` | | 🟡 | | roles/members |
| F-APO-PMISC-010 | GET `/openapi/v1/organizations` | | 🟡 | | |
| F-APO-PMISC-011 | GET `/openapi/v1/user` | | 🟡 | | current user |
| F-APO-PMISC-012 | GET/POST/PUT/DELETE `/openapi/v1/users` | | 🟡 | | user mgmt |
| F-APO-PMISC-013 | GET/POST/DELETE `/openapi/v1/user-tokens` | | ⚪ | | DB schema only, no API endpoints |
| F-APO-PMISC-014 | GET `/openapi/v1/consumers` / consumer-tokens | | 🟡 | | open platform |
| F-APO-PMISC-015 | GET/POST `/openapi/v1/configs/import|export` | | ⚪ | | NOT at OpenAPI path (exists at admin path) |
| F-APO-PMISC-016 | GET `/openapi/v1/system-info` | | ⚪ | | NOT implemented |
| F-APO-PMISC-017 | GET/POST `/openapi/v1/server/portal-db/config*` | | 🟡 | | server config |
| F-APO-PMISC-018 | GET `/openapi/v1/apollo/audit/*` | | ⚪ | | NOT at OpenAPI path (exists at admin path) |
| F-APO-PMISC-019 | GET `/openapi/v1/favorites` | | ⚪ | | NOT at OpenAPI path (exists at admin path) |
| F-APO-PMISC-020 | GET `/openapi/v1/global-search/item-info/by-key-or-value` | | ⚪ | | NOT at OpenAPI path (exists at `/search`) |
| F-APO-PMISC-021 | Portal token / session auth | | ⚪ | | consumer/session/user-token — NOT enforced |

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
| 1 configservice fetch | 5 | 0 | 0 | 2 | 0 | 7 | 71% |
| 2 long polling | 0 | 2 | 0 | 0 | 0 | 2 | 50% |
| 3 metaservice | 0 | 0 | 0 | 3 | 0 | 3 | 0% |
| 4 adminservice apps/ns | 15 | 0 | 0 | 4 | 0 | 19 | 79% |
| 5 adminservice items | 8 | 1 | 0 | 3 | 0 | 12 | 75% |
| 6 adminservice release | 14 | 0 | 0 | 0 | 0 | 14 | 100% |
| 7 adminservice branch | 5 | 0 | 0 | 0 | 0 | 5 | 100% |
| 8 adminservice misc | 15 | 1 | 0 | 6 | 0 | 22 | 73% |
| 9 portal apps/ns | 0 | 25 | 0 | 1 | 0 | 26 | 48% |
| 10 portal items | 0 | 12 | 0 | 0 | 0 | 12 | 50% |
| 11 portal releases | 14 | 0 | 0 | 0 | 0 | 14 | 100% |
| 12 portal misc | 0 | 13 | 0 | 8 | 0 | 21 | 31% |
| 13 legacy webapi | 0 | 0 | 0 | 22 | 0 | 22 | 0% |
| **Total** | 76 | 54 | 0 | 49 | 0 | 179 | 73% |

> Paths verified against module controllers AND batata source code. Key corrections from previous version: 6 adminservice features downgraded from 🟢 to ⚪ (cluster unique, find-by-item, associated-public-namespace, publish_info, instances by-*, accesskey enable/disable); 3 adminservice features added (instance-configs, config sync, audit/favorites/search at admin path); long polling downgraded from ⚡ to 🟡 (route exists but short-poll not long-poll); 4 portal misc features downgraded from 🟡 to ⚪ (user-tokens, system-info, audit/favorites/global-search at OpenAPI path). Summary totals: 76 🟢, 54 🟡, 49 ⚪, 179 total, 73% impl rate.
