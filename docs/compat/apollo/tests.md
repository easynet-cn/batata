# Apollo Test Case Inventory

Upstream baseline: **Apollo (2025)** (local `~/work/github/easynet-cn/apollo`).

> Purpose: map upstream Apollo test cases to batata F-IDs for compatibility tracking.
> Test types: `HTTP_API` (Spring Boot integration test with real HTTP calls), `SDK_CLIENT` (Java SDK client compatibility test), `INTERNAL` (Mockito unit test, not ported).
>
> Scope:
> - **configservice**: 4 integration test files under `.../configservice/integration/` — 56 @Test methods
> - **adminservice**: controller + filter integration tests under `.../adminservice/controller/` and `.../adminservice/filter/` — 51 HTTP + 27 INTERNAL methods
> - **portal OpenAPI v1**: MockMvc-based controller tests under `.../openapi/v1/controller/` — ~180 methods across 18 files
> - **portal legacy WebAPI**: controller unit tests under `.../portal/controller/` — INTERNAL only (deprecated)
> - **SDK_CLIENT**: `ApolloOpenApiJavaClientCompatibilityTest` — 5 @Test methods
>
> Pure Mockito unit tests (`@RunWith(MockitoJUnitRunner.class)` with `@InjectMocks`) are listed as INTERNAL — they test Java service internals, not the HTTP contract, and are not portable to batata's Rust implementation.

Status: `✅ pass` | `⏳ pending` | `❌ fail` | `⏭️ skip` (feature ⚪/⛔)

---

## 1. configservice — Config fetch (`ConfigControllerIntegrationTest`)

> Source: `apollo-configservice/src/test/java/.../configservice/integration/ConfigControllerIntegrationTest.java`
> Endpoint: `GET /configs/{appId}/{clusterName}/{namespace}`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-CFGSVC-001-01 | F-APO-CFGSVC-001 | testQueryConfigWithDefaultClusterAndDefaultNamespaceOK | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-02 | F-APO-CFGSVC-001 | testQueryConfigWithDefaultClusterAndDefaultNamespaceAndIncorrectCase | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-03 | F-APO-CFGSVC-001 | testQueryGrayConfigWithDefaultClusterAndDefaultNamespaceOK | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-04 | F-APO-CFGSVC-001 | testQueryGrayConfigWithDefaultClusterAndDefaultNamespaceAndIncorrectCase | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-05 | F-APO-CFGSVC-001 | testQueryConfigFileWithDefaultClusterAndDefaultNamespaceOK | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-06 | F-APO-CFGSVC-001 | testQueryConfigWithNamespaceOK | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-07 | F-APO-CFGSVC-001 | testQueryConfigFileWithNamespaceOK | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-08 | F-APO-CFGSVC-001 | testQueryConfigError | HTTP_API | ⏳ | 404 for non-existent namespace |
| T-APO-CFGSVC-001-09 | F-APO-CFGSVC-001 | testQueryConfigNotModified | HTTP_API | ⏳ | 304 with matching releaseKey |
| T-APO-CFGSVC-001-10 | F-APO-CFGSVC-001 | testQueryPublicGrayConfigWithNoOverride | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-11 | F-APO-CFGSVC-001 | testQueryPublicConfigWithDataCenterFoundAndNoOverride | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-12 | F-APO-CFGSVC-001 | testQueryPublicConfigWithDataCenterFoundAndOverride | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-13 | F-APO-CFGSVC-001 | testQueryPublicConfigWithIncorrectCaseAndDataCenterFoundAndOverride | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-14 | F-APO-CFGSVC-001 | testQueryPublicConfigWithDataCenterNotFoundAndNoOverride | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-15 | F-APO-CFGSVC-001 | testQueryPublicConfigWithDataCenterNotFoundAndOverride | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-16 | F-APO-CFGSVC-001 | testQueryPublicGrayConfigWithOverride | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-17 | F-APO-CFGSVC-001 | testQueryPublicGrayConfigWithIncorrectCaseAndOverride | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-18 | F-APO-CFGSVC-001 | testQueryPrivateConfigFileWithPublicNamespaceExists | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-19 | F-APO-CFGSVC-001 | testQueryConfigForNoAppIdPlaceHolderWithPrivateNamespace | HTTP_API | ⏳ | |
| T-APO-CFGSVC-001-20 | F-APO-CFGSVC-001 | testQueryPublicConfigForNoAppIdPlaceHolder | HTTP_API | ⏳ | |

## 2. configservice — Config file fetch (`ConfigFileControllerIntegrationTest`)

> Source: `apollo-configservice/src/test/java/.../configservice/integration/ConfigFileControllerIntegrationTest.java`
> Endpoints: `GET /configfiles/...`, `GET /configfiles/json/...`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-CFGSVC-002-01 | F-APO-CFGSVC-002 | testQueryConfigAsProperties | HTTP_API | ⏳ | |
| T-APO-CFGSVC-002-02 | F-APO-CFGSVC-002 | testQueryConfigAsPropertiesWithGrayRelease | HTTP_API | ⏳ | |
| T-APO-CFGSVC-002-03 | F-APO-CFGSVC-002 | testQueryPublicConfigAsProperties | HTTP_API | ⏳ | |
| T-APO-CFGSVC-003-01 | F-APO-CFGSVC-003 | testQueryConfigAsJson | HTTP_API | ⏳ | |
| T-APO-CFGSVC-003-02 | F-APO-CFGSVC-003 | testQueryConfigAsJsonWithIncorrectCase | HTTP_API | ⏳ | |
| T-APO-CFGSVC-003-03 | F-APO-CFGSVC-003 | testQueryPublicConfigAsJson | HTTP_API | ⏳ | |
| T-APO-CFGSVC-003-04 | F-APO-CFGSVC-003 | testQueryPublicConfigAsJsonWithIncorrectCase | HTTP_API | ⏳ | |
| T-APO-CFGSVC-003-05 | F-APO-CFGSVC-003 | testQueryPublicConfigAsJsonWithGrayRelease | HTTP_API | ⏳ | |
| T-APO-CFGSVC-003-06 | F-APO-CFGSVC-003 | testQueryPublicConfigAsJsonWithGrayReleaseAndIncorrectCase | HTTP_API | ⏳ | |
| T-APO-CFGSVC-002-04 | F-APO-CFGSVC-002 | testConfigChanged | HTTP_API | ⏳ | config change after new release |

## 3. configservice — Long polling v1 (`NotificationControllerIntegrationTest`)

> Source: `apollo-configservice/src/test/java/.../configservice/integration/NotificationControllerIntegrationTest.java`
> Endpoint: `GET /notifications` (v1, deprecated)

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-LONGPOL-002-01 | F-APO-LONGPOL-002 | testPollNotificationWithDefaultNamespace | HTTP_API | ⏳ | |
| T-APO-LONGPOL-002-02 | F-APO-LONGPOL-002 | testPollNotificationWithDefaultNamespaceAsFile | HTTP_API | ⏳ | |
| T-APO-LONGPOL-002-03 | F-APO-LONGPOL-002 | testPollNotificationWithPrivateNamespaceAsFile | HTTP_API | ⏳ | |
| T-APO-LONGPOL-002-04 | F-APO-LONGPOL-002 | testPollNotificationWithDefaultNamespaceWithNotificationIdNull | HTTP_API | ⏳ | |
| T-APO-LONGPOL-002-05 | F-APO-LONGPOL-002 | testPollNotificationWithDefaultNamespaceWithNotificationIdOutDated | HTTP_API | ⏳ | |
| T-APO-LONGPOL-002-06 | F-APO-LONGPOL-002 | testPollNotificationWthPublicNamespaceAndNoDataCenter | HTTP_API | ⏳ | |
| T-APO-LONGPOL-002-07 | F-APO-LONGPOL-002 | testPollNotificationWthPublicNamespaceAndDataCenter | HTTP_API | ⏳ | |
| T-APO-LONGPOL-002-08 | F-APO-LONGPOL-002 | testPollNotificationWthPublicNamespaceAsFile | HTTP_API | ⏳ | |
| T-APO-LONGPOL-002-09 | F-APO-LONGPOL-002 | testPollNotificationWithPublicNamespaceWithNotificationIdOutDated | HTTP_API | ⏳ | |

## 4. configservice — Long polling v2 (`NotificationControllerV2IntegrationTest`)

> Source: `apollo-configservice/src/test/java/.../configservice/integration/NotificationControllerV2IntegrationTest.java`
> Endpoint: `GET /notifications/v2`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-LONGPOL-001-01 | F-APO-LONGPOL-001 | testPollNotificationWithDefaultNamespace | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-02 | F-APO-LONGPOL-001 | testPollNotificationWithDefaultNamespaceAsFile | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-03 | F-APO-LONGPOL-001 | testPollNotificationWithMultipleNamespaces | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-04 | F-APO-LONGPOL-001 | testPollNotificationWithMultipleNamespacesAndIncorrectCase | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-05 | F-APO-LONGPOL-001 | testPollNotificationWithPrivateNamespaceAsFile | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-06 | F-APO-LONGPOL-001 | testPollNotificationWithDefaultNamespaceWithNotificationIdOutDated | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-07 | F-APO-LONGPOL-001 | testPollNotificationWthPublicNamespaceAndNoDataCenter | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-08 | F-APO-LONGPOL-001 | testPollNotificationWthPublicNamespaceAndDataCenter | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-09 | F-APO-LONGPOL-001 | testPollNotificationWthMultipleNamespacesAndMultipleNamespacesChanged | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-10 | F-APO-LONGPOL-001 | testPollNotificationWthPublicNamespaceAsFile | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-11 | F-APO-LONGPOL-001 | testPollNotificationWithPublicNamespaceWithNotificationIdOutDated | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-12 | F-APO-LONGPOL-001 | testPollNotificationWithMultiplePublicNamespaceWithIncorrectCaseWithNotificationIdOutDated | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-13 | F-APO-LONGPOL-001 | testPollNotificationWithMultiplePublicNamespaceWithIncorrectCase2WithNotificationIdOutDated | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-14 | F-APO-LONGPOL-001 | testPollNotificationWithMultiplePublicNamespaceWithIncorrectCase3WithNotificationIdOutDated | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-15 | F-APO-LONGPOL-001 | testPollNotificationWithMultiplePublicNamespaceWithIncorrectCase4WithNotificationIdOutDated | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-16 | F-APO-LONGPOL-001 | testPollNotificationWithMultipleNamespacesAndNotificationIdsOutDated | HTTP_API | ⏳ | |
| T-APO-LONGPOL-001-17 | F-APO-LONGPOL-001 | testPollNotificationWithMultipleNamespacesAndNotificationIdsOutDatedAndIncorrectCase | HTTP_API | ⏳ | |

## 5. configservice — AccessKey auth filter (`ClientAuthenticationFilterTest`)

> Source: `apollo-configservice/src/test/java/.../configservice/filter/ClientAuthenticationFilterTest.java`
> Feature: F-APO-CFGSVC-007 (AccessKey HMAC-SHA1 signature)

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-CFGSVC-007-01 | F-APO-CFGSVC-007 | ClientAuthenticationFilterTest (all methods) | INTERNAL | ⏭️ | Feature ⚪ — AccessKey not implemented |

## 6. adminservice — Apps (`AppControllerTest`)

> Source: `apollo-adminservice/src/test/java/.../adminservice/controller/AppControllerTest.java`
> Type: Integration (extends AbstractControllerTest)

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-ADM-006-01 | F-APO-ADM-006 | testCheckIfAppIdUnique | HTTP_API | ⏳ | |
| T-APO-ADM-001-01 | F-APO-ADM-001 | testCreate | HTTP_API | ⏳ | |
| T-APO-ADM-001-02 | F-APO-ADM-001 | testCreateTwice | HTTP_API | ⏳ | duplicate → 400 |
| T-APO-ADM-003-01 | F-APO-ADM-003 | testFind | HTTP_API | ⏳ | |
| T-APO-ADM-003-02 | F-APO-ADM-003 | testFindNotExist | HTTP_API | ⏳ | |
| T-APO-ADM-005-01 | F-APO-ADM-005 | testDelete | HTTP_API | ⏳ | |
| T-APO-ADM-001-03 | F-APO-ADM-001 | shouldFailedWhenAppIdIsInvalid | HTTP_API | ⏳ | |

## 7. adminservice — App exception (`ControllerExceptionTest`, `ControllerIntegrationExceptionTest`)

> Source: `apollo-adminservice/src/test/java/.../adminservice/controller/ControllerExceptionTest.java`
> Source: `apollo-adminservice/src/test/java/.../adminservice/controller/ControllerIntegrationExceptionTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-ADM-003-03 | F-APO-ADM-003 | testFindNotExists | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-ADM-005-02 | F-APO-ADM-005 | testDeleteNotExists | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-ADM-002-01 | F-APO-ADM-002 | testFindEmpty | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-ADM-002-02 | F-APO-ADM-002 | testFindByName | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-ADM-001-04 | F-APO-ADM-001 | createFailed | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-ADM-001-05 | F-APO-ADM-001 | createAutoProvisionAccessKey | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-ADM-001-06 | F-APO-ADM-001 | createAutoProvisionAccessKeyFailedShouldNotAffectAppCreation | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-ADM-001-07 | F-APO-ADM-001 | testCreateFailed | HTTP_API | ⏳ | integration: adminService throws |
| T-APO-ADM-001-08 | F-APO-ADM-001 | testCreateWithAccessKeyAutoProvisionFailedAppStillCreated | HTTP_API | ⏳ | integration: accessKey fails, app OK |

## 8. adminservice — Clusters (`ClusterControllerTest`)

> Source: `apollo-adminservice/src/test/java/.../adminservice/controller/ClusterControllerTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-ADM-010-01 | F-APO-ADM-010 | testDeleteDefaultFail | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-ADM-010-02 | F-APO-ADM-010 | testDeleteSuccess | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-ADM-007-01 | F-APO-ADM-007 | shouldFailWhenRequestBodyInvalid | HTTP_API | ⏳ | |

## 9. adminservice — AppNamespace (`AppNamespaceControllerTest`)

> Source: `apollo-adminservice/src/test/java/.../adminservice/controller/AppNamespaceControllerTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-ADMSVC-007-01 | F-APO-ADMSVC-007 | testCreate | HTTP_API | ⏳ | |

## 10. adminservice — Namespace (`NamespaceControllerTest`)

> Source: `apollo-adminservice/src/test/java/.../adminservice/controller/NamespaceControllerTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-ADM-012-01 | F-APO-ADM-012 | create | HTTP_API | ⏳ | invalid name → 400 |

## 11. adminservice — Items (`ItemControllerTest`)

> Source: `apollo-adminservice/src/test/java/.../adminservice/controller/ItemControllerTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-ITEM-001-01 | F-APO-ITEM-001 | testCreate | HTTP_API | ⏳ | |
| T-APO-ITEM-003-01 | F-APO-ITEM-003 | testUpdate | HTTP_API | ⏳ | |
| T-APO-ITEM-004-01 | F-APO-ITEM-004 | testDelete | HTTP_API | ⏳ | |
| T-APO-ITEM-011-01 | F-APO-ITEM-011 | testSearch | HTTP_API | ⏳ | GET /items-search/key-and-value |

## 12. adminservice — ItemSet (`ItemSetControllerTest`)

> Source: `apollo-adminservice/src/test/java/.../adminservice/controller/ItemSetControllerTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-ITEM-012-01 | F-APO-ITEM-012 | testItemSetCreated | HTTP_API | ⏳ | |
| T-APO-ITEM-012-02 | F-APO-ITEM-012 | testItemSetCreatedWithInvalidNamespaceId | HTTP_API | ⏳ | 400 on namespace mismatch |
| T-APO-ITEM-012-03 | F-APO-ITEM-012 | testItemSetUpdated | HTTP_API | ⏳ | |
| T-APO-ITEM-012-04 | F-APO-ITEM-012 | testItemSetUpdatedWithInvalidNamespaceId | HTTP_API | ⏳ | |
| T-APO-ITEM-012-05 | F-APO-ITEM-012 | testItemSetDeleted | HTTP_API | ⏳ | |
| T-APO-ITEM-012-06 | F-APO-ITEM-012 | testItemSetDeletedWithInvalidNamespaceId | HTTP_API | ⏳ | |

## 13. adminservice — Release (`ReleaseControllerTest`)

> Source: `apollo-adminservice/src/test/java/.../adminservice/controller/ReleaseControllerTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-RELEASE-001-01 | F-APO-RELEASE-001 | testReleaseBuild | HTTP_API | ⏳ | |
| T-APO-RELEASE-001-02 | F-APO-RELEASE-001 | testMessageSendAfterBuildRelease | INTERNAL | ⏭️ | Mockito unit test |

## 14. adminservice — Server config (`ServerConfigControllerTest`)

> Source: `apollo-adminservice/src/test/java/.../adminservice/controller/ServerConfigControllerTest.java`
> Note: upstream path is `/server/config/find-all-config`; batata uses `/serverconfigs`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-ADMSVC-011-01 | F-APO-ADMSVC-011 | findAllServerConfig | HTTP_API | ⏳ | path: /server/config/find-all-config |
| T-APO-ADMSVC-012-01 | F-APO-ADMSVC-012 | createOrUpdatePortalDBConfig | HTTP_API | ⏳ | |
| T-APO-ADMSVC-012-02 | F-APO-ADMSVC-012 | createConfigShouldUseKeyAndClusterAsIdentity | HTTP_API | ⏳ | |
| T-APO-ADMSVC-012-03 | F-APO-ADMSVC-012 | updateConfigShouldOnlyAffectTargetClusterWhenSameKeyExists | HTTP_API | ⏳ | |
| T-APO-ADMSVC-012-04 | F-APO-ADMSVC-012 | deleteConfig | HTTP_API | ⏳ | |
| T-APO-ADMSVC-012-05 | F-APO-ADMSVC-012 | deleteConfigShouldOnlyDeleteTargetClusterWhenSameKeyExists | HTTP_API | ⏳ | |

## 15. adminservice — Instance config (`InstanceConfigControllerTest`)

> Source: `apollo-adminservice/src/test/java/.../adminservice/controller/InstanceConfigControllerTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-ADMSVC-001-01 | F-APO-ADMSVC-001 | getByRelease | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-ADMSVC-001-02 | F-APO-ADMSVC-001 | testGetByReleaseWhenReleaseIsNotFound | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-ADMSVC-004-01 | F-APO-ADMSVC-004 | testGetByReleasesNotIn | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-ADMSVC-002-01 | F-APO-ADMSVC-002 | testGetInstancesByNamespace | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-ADMSVC-002-02 | F-APO-ADMSVC-002 | testGetInstancesByNamespaceAndInstanceAppId | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-ADMSVC-003-01 | F-APO-ADMSVC-003 | testGetInstancesCountByNamespace | INTERNAL | ⏭️ | Mockito unit test |

## 16. adminservice — Namespace lock (`NamespaceLockTest`, `NamespaceUnlockAspectTest`)

> Source: `apollo-adminservice/src/test/java/.../adminservice/aop/NamespaceLockTest.java`
> Source: `apollo-adminservice/src/test/java/.../adminservice/aop/NamespaceUnlockAspectTest.java`
> Feature: F-APO-RELEASE-010 (namespace lock)

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-RELEASE-010-01 | F-APO-RELEASE-010 | acquireLockWithNotLockedAndSwitchON | INTERNAL | ⏭️ | AOP aspect test |
| T-APO-RELEASE-010-02 | F-APO-RELEASE-010 | acquireLockWithNotLockedAndSwitchOFF | INTERNAL | ⏭️ | AOP aspect test |
| T-APO-RELEASE-010-03 | F-APO-RELEASE-010 | acquireLockWithAlreadyLockedByOtherGuy | INTERNAL | ⏭️ | AOP aspect test |
| T-APO-RELEASE-010-04 | F-APO-RELEASE-010 | acquireLockWithAlreadyLockedBySelf | INTERNAL | ⏭️ | AOP aspect test |
| T-APO-RELEASE-010-05 | F-APO-RELEASE-010 | acquireLockWithNamespaceIdSwitchOn | INTERNAL | ⏭️ | AOP aspect test |
| T-APO-RELEASE-010-06 | F-APO-RELEASE-010 | testDuplicateLock | INTERNAL | ⏭️ | AOP aspect test |
| T-APO-RELEASE-010-07 | F-APO-RELEASE-010 | testNamespaceHasNoNormalItemsAndRelease | INTERNAL | ⏭️ | AOP aspect test |
| T-APO-RELEASE-010-08 | F-APO-RELEASE-010 | testNamespaceAddItem | INTERNAL | ⏭️ | AOP aspect test |
| T-APO-RELEASE-010-09 | F-APO-RELEASE-010 | testNamespaceModifyItem | INTERNAL | ⏭️ | AOP aspect test |
| T-APO-RELEASE-010-10 | F-APO-RELEASE-010 | testNamespaceDeleteItem | INTERNAL | ⏭️ | AOP aspect test |
| T-APO-RELEASE-010-11 | F-APO-RELEASE-010 | testChildNamespaceModified | INTERNAL | ⏭️ | AOP aspect test |
| T-APO-RELEASE-010-12 | F-APO-RELEASE-010 | testChildNamespaceNotModified | INTERNAL | ⏭️ | AOP aspect test |
| T-APO-RELEASE-010-13 | F-APO-RELEASE-010 | testParentNamespaceNotReleased | INTERNAL | ⏭️ | AOP aspect test |

## 17. adminservice — Auth filter (`AdminServiceAuthenticationFilterTest`, `AdminServiceAuthenticationIntegrationTest`)

> Source: `apollo-adminservice/src/test/java/.../adminservice/filter/AdminServiceAuthenticationFilterTest.java`
> Source: `apollo-adminservice/src/test/java/.../adminservice/filter/AdminServiceAuthenticationIntegrationTest.java`
> Feature: F-APO-ADMSVC-022 (admin token auth)

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-ADMSVC-022-01 | F-APO-ADMSVC-022 | testWithAccessControlDisabled | INTERNAL | ⏭️ | Feature ⚪ — admin auth not implemented |
| T-APO-ADMSVC-022-02 | F-APO-ADMSVC-022 | testWithAccessControlEnabledWithTokenSpecifiedWithValidTokenPassed | INTERNAL | ⏭️ | Feature ⚪ |
| T-APO-ADMSVC-022-03 | F-APO-ADMSVC-022 | testWithAccessControlEnabledWithTokenSpecifiedWithInvalidTokenPassed | INTERNAL | ⏭️ | Feature ⚪ |
| T-APO-ADMSVC-022-04 | F-APO-ADMSVC-022 | testWithAccessControlEnabledWithTokenSpecifiedWithNoTokenPassed | INTERNAL | ⏭️ | Feature ⚪ |
| T-APO-ADMSVC-022-05 | F-APO-ADMSVC-022 | testWithAccessControlEnabledWithMultipleTokenSpecifiedWithValidTokenPassed | INTERNAL | ⏭️ | Feature ⚪ |
| T-APO-ADMSVC-022-06 | F-APO-ADMSVC-022 | testWithAccessControlEnabledWithNoTokenSpecifiedWithTokenPassed | INTERNAL | ⏭️ | Feature ⚪ |
| T-APO-ADMSVC-022-07 | F-APO-ADMSVC-022 | testWithAccessControlEnabledWithNoTokenSpecifiedWithNoTokenPassed | INTERNAL | ⏭️ | Feature ⚪ |
| T-APO-ADMSVC-022-08 | F-APO-ADMSVC-022 | testWithConfigChanged | INTERNAL | ⏭️ | Feature ⚪ |
| T-APO-ADMSVC-022-09 | F-APO-ADMSVC-022 | testWithAccessControlDisabledExplicitly | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-ADMSVC-022-10 | F-APO-ADMSVC-022 | testWithAccessControlDisabledExplicitlyWithAccessToken | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-ADMSVC-022-11 | F-APO-ADMSVC-022 | testWithAccessControlEnabledWithValidAccessToken | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-ADMSVC-022-12 | F-APO-ADMSVC-022 | testWithAccessControlEnabledWithNoAccessToken | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-ADMSVC-022-13 | F-APO-ADMSVC-022 | testWithAccessControlEnabledWithInValidAccessToken | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-ADMSVC-022-14 | F-APO-ADMSVC-022 | testWithAccessControlEnabledWithNoTokenSpecified | HTTP_API | ⏭️ | Feature ⚪ |

## 18. portal OpenAPI — Apps (`AppControllerTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/AppControllerTest.java`
> Type: `@SpringBootTest` with MockMvc

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PORT-001-01 | F-APO-PORT-001 | testCreateAppForUserTokenUsesCurrentTokenUser | HTTP_API | ⏳ | |
| T-APO-PORT-003-01 | F-APO-PORT-003 | testFindAppsAuthorized | HTTP_API | ⏳ | |
| T-APO-PORT-002-01 | F-APO-PORT-002 | findAppsShouldRejectUserTokenWhenRequestedAppIsOutsideScope | HTTP_API | ⏳ | |
| T-APO-PORT-008-01 | F-APO-PORT-008 | testGetEnvClusters | HTTP_API | ⏳ | |
| T-APO-PORT-009-01 | F-APO-PORT-009 | testGetEnvClusterInfo | HTTP_API | ⏳ | |
| T-APO-PORT-002-02 | F-APO-PORT-002 | testFindAppsByIds | HTTP_API | ⏳ | |
| T-APO-PORT-002-03 | F-APO-PORT-002 | testFindAllApps | HTTP_API | ⏳ | |
| T-APO-PORT-002-04 | F-APO-PORT-002 | testFindAllAppsFiltersUserTokenScope | HTTP_API | ⏳ | |
| T-APO-PORT-005-01 | F-APO-PORT-005 | testGetApp | HTTP_API | ⏳ | |
| T-APO-PORT-005-02 | F-APO-PORT-005 | testGetAppNotFound | HTTP_API | ⏳ | 404 |
| T-APO-PORT-004-01 | F-APO-PORT-004 | testGetAppsBySelf | HTTP_API | ⏳ | |
| T-APO-PORT-004-02 | F-APO-PORT-004 | testGetAppsBySelfForPortalUser | HTTP_API | ⏳ | |
| T-APO-PORT-004-03 | F-APO-PORT-004 | testGetAppsBySelfForPortalUserWithoutLoginUser | HTTP_API | ⏳ | |
| T-APO-PORT-004-04 | F-APO-PORT-004 | testGetAppsBySelfForPortalUserWithoutRoles | HTTP_API | ⏳ | |
| T-APO-PORT-003-02 | F-APO-PORT-003 | testFindAppsAuthorizedForUserTokenUsesReadableApps | HTTP_API | ⏳ | |
| T-APO-PORT-004-05 | F-APO-PORT-004 | testGetAppsBySelfForUserTokenUsesReadableApps | HTTP_API | ⏳ | |
| T-APO-PORT-010-01 | F-APO-PORT-010 | testFindMissEnvs | HTTP_API | ⏳ | |
| T-APO-PORT-006-01 | F-APO-PORT-006 | testUpdateApp | HTTP_API | ⏳ | |
| T-APO-PORT-006-02 | F-APO-PORT-006 | testUpdateAppWithMismatchedAppId | HTTP_API | ⏳ | |
| T-APO-PORT-007-01 | F-APO-PORT-007 | testDeleteApp | HTTP_API | ⏳ | |

## 19. portal OpenAPI — Apps param bind (`AppControllerParamBindLowLevelTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/AppControllerParamBindLowLevelTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PORT-001-02 | F-APO-PORT-001 | createAppShouldBindFromPayload | HTTP_API | ⏳ | |
| T-APO-PORT-011-01 | F-APO-PORT-011 | createAppInEnvShouldAcceptOptionalEnvAndOperator | HTTP_API | ⏳ | |
| T-APO-PORT-004-06 | F-APO-PORT-004 | getAppsBySelfForConsumerShouldUseConsumerAppIds | HTTP_API | ⏳ | |
| T-APO-PORT-004-07 | F-APO-PORT-004 | getAppsBySelfForConsumerShouldRejectWhenConsumerHasNoAppIds | HTTP_API | ⏳ | |
| T-APO-PORT-004-08 | F-APO-PORT-004 | getAppsBySelfForPortalUserShouldUseUserInfoHolder | HTTP_API | ⏳ | |
| T-APO-PORT-004-09 | F-APO-PORT-004 | getAppsBySelfForPortalUserShouldReturnEmptyWhenNoUser | HTTP_API | ⏳ | |
| T-APO-PORT-004-10 | F-APO-PORT-004 | getAppsBySelfForUserTokenShouldUseReadableAppIds | HTTP_API | ⏳ | |
| T-APO-PORT-004-11 | F-APO-PORT-004 | getAppsBySelfForUserTokenShouldReturnEmptyWhenNoReadableApps | HTTP_API | ⏳ | |
| T-APO-PORT-004-12 | F-APO-PORT-004 | getAppsBySelfForAnonymousShouldReturnEmpty | HTTP_API | ⏳ | |
| T-APO-PORT-001-03 | F-APO-PORT-001 | createAppShouldRejectConsumerWithoutCreatePermission | HTTP_API | ⏳ | |
| T-APO-PORT-001-04 | F-APO-PORT-001 | createAppShouldAllowConsumerWithCreatePermission | HTTP_API | ⏳ | |
| T-APO-PORT-006-03 | F-APO-PORT-006 | updateAppShouldBindFromPayloadAndPath | HTTP_API | ⏳ | |
| T-APO-PORT-006-04 | F-APO-PORT-006 | updateAppShouldRejectMismatchedAppId | HTTP_API | ⏳ | |
| T-APO-PORT-007-02 | F-APO-PORT-007 | deleteAppShouldRejectConsumerWithoutAppAdmin | HTTP_API | ⏳ | |
| T-APO-PORT-007-03 | F-APO-PORT-007 | deleteAppShouldAllowConsumerWithAppAdmin | HTTP_API | ⏳ | |
| T-APO-PORT-007-04 | F-APO-PORT-007 | deleteAppShouldRejectPortalUserWithoutSuperAdmin | HTTP_API | ⏳ | |
| T-APO-PORT-007-05 | F-APO-PORT-007 | deleteAppShouldAllowPortalUserWithSuperAdmin | HTTP_API | ⏳ | |
| T-APO-PORT-007-06 | F-APO-PORT-007 | deleteAppShouldAllowUserTokenWithAppManageRole | HTTP_API | ⏳ | |

## 20. portal OpenAPI — Apps integration (`AppControllerIntegrationTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/AppControllerIntegrationTest.java`
> Type: DB integration with `@Sql`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PORT-003-03 | F-APO-PORT-003 | testFindAppsAuthorized | HTTP_API | ⏳ | DB-backed |

## 21. portal OpenAPI — Clusters (`ClusterControllerTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/ClusterControllerTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PORT-015-01 | F-APO-PORT-015 | testGetCluster | HTTP_API | ⏳ | |
| T-APO-PORT-014-01 | F-APO-PORT-014 | testCreateCluster | HTTP_API | ⏳ | |
| T-APO-PORT-014-02 | F-APO-PORT-014 | testCreateClusterWithAppIdMismatch | HTTP_API | ⏳ | |
| T-APO-PORT-016-01 | F-APO-PORT-016 | testDeleteCluster | HTTP_API | ⏳ | |

## 22. portal OpenAPI — Clusters param bind (`ClusterControllerParamBindLowLevelTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/ClusterControllerParamBindLowLevelTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PORT-015-02 | F-APO-PORT-015 | getCluster_shouldBind_path | HTTP_API | ⏳ | |
| T-APO-PORT-014-03 | F-APO-PORT-014 | createCluster_shouldBind_path_and_body | HTTP_API | ⏳ | |
| T-APO-PORT-014-04 | F-APO-PORT-014 | createCluster_shouldRejectUserTokenWithoutClusterScope | HTTP_API | ⏳ | |
| T-APO-PORT-016-02 | F-APO-PORT-016 | deleteCluster_shouldBind_path_and_query | HTTP_API | ⏳ | |
| T-APO-PORT-016-03 | F-APO-PORT-016 | deleteCluster_shouldAllowConsumerAppAdminWithoutSuperAdmin | HTTP_API | ⏳ | |
| T-APO-PORT-016-04 | F-APO-PORT-016 | deleteCluster_shouldRejectConsumerWithoutAppAdminOrSuperAdmin | HTTP_API | ⏳ | |
| T-APO-PORT-016-05 | F-APO-PORT-016 | deleteCluster_shouldAllowPortalSuperAdmin | HTTP_API | ⏳ | |
| T-APO-PORT-016-06 | F-APO-PORT-016 | deleteCluster_shouldRejectPortalUserWithoutSuperAdmin | HTTP_API | ⏳ | |

## 23. portal OpenAPI — Envs (`EnvControllerTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/EnvControllerTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PORT-013-01 | F-APO-PORT-013 | testGetEnvs | HTTP_API | ⏳ | |
| T-APO-PORT-013-02 | F-APO-PORT-013 | getEnvsShouldRejectUserTokenWithoutMetadataScope | HTTP_API | ⏳ | |

## 24. portal OpenAPI — Organizations (`OrganizationControllerTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/OrganizationControllerTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PMISC-010-01 | F-APO-PMISC-010 | testGetOrganizations | HTTP_API | ⏳ | |
| T-APO-PMISC-010-02 | F-APO-PMISC-010 | getOrganizationsShouldRejectUserTokenWithoutMetadataScope | HTTP_API | ⏳ | |

## 25. portal OpenAPI — Namespace (`NamespaceControllerTest`, `NamespaceControllerWithAuthorizationTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/NamespaceControllerTest.java`
> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/NamespaceControllerWithAuthorizationTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PORT-022-01 | F-APO-PORT-022 | shouldFailWhenAppNamespaceNameIsInvalid | HTTP_API | ⏳ | |
| T-APO-PORT-022-02 | F-APO-PORT-022 | testCreateAppNamespace | HTTP_API | ⏳ | @Ignore in upstream |
| T-APO-PORT-022-03 | F-APO-PORT-022 | testCreateAppNamespaceUnauthorized | HTTP_API | ⏳ | |
| T-APO-PORT-022-04 | F-APO-PORT-022 | testCreateAppNamespaceInvalidNamespaceName | HTTP_API | ⏳ | |
| T-APO-PORT-022-05 | F-APO-PORT-022 | testCreateAppNamespaceWithoutAuthority | HTTP_API | ⏳ | |

## 26. portal OpenAPI — Namespace param bind (`NamespaceControllerParamBindLowLevelTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/NamespaceControllerParamBindLowLevelTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PORT-022-06 | F-APO-PORT-022 | createAppNamespaceShouldBindFromPayload | HTTP_API | ⏳ | |
| T-APO-PORT-022-07 | F-APO-PORT-022 | createAppNamespaceShouldRejectUserTokenWithoutAppManageRole | HTTP_API | ⏳ | |
| T-APO-PORT-022-08 | F-APO-PORT-022 | createAppNamespaceShouldAllowAuthorizedConsumerToken | HTTP_API | ⏳ | |
| T-APO-PORT-022-09 | F-APO-PORT-022 | createAppNamespaceShouldRejectUnauthorizedConsumerToken | HTTP_API | ⏳ | |
| T-APO-PORT-021-01 | F-APO-PORT-021 | createNamespacesShouldBindFromPayload | HTTP_API | ⏳ | |
| T-APO-PORT-021-02 | F-APO-PORT-021 | createNamespacesShouldRejectUserTokenWithoutNamespaceScope | HTTP_API | ⏳ | |
| T-APO-PORT-021-03 | F-APO-PORT-021 | createNamespacesShouldAllowAuthorizedConsumerToken | HTTP_API | ⏳ | |
| T-APO-PORT-021-04 | F-APO-PORT-021 | createNamespacesShouldRejectUnauthorizedConsumerToken | HTTP_API | ⏳ | |
| T-APO-PORT-019-01 | F-APO-PORT-019 | deleteNamespaceShouldUseCurrentPortalUser | HTTP_API | ⏳ | |
| T-APO-PORT-019-02 | F-APO-PORT-019 | deleteNamespaceShouldRejectConsumerWithoutOperator | HTTP_API | ⏳ | |
| T-APO-PORT-018-01 | F-APO-PORT-018 | findNamespaceShouldRejectUserTokenWithoutConfigRead | HTTP_API | ⏳ | |
| T-APO-PORT-017-01 | F-APO-PORT-017 | findNamespacesShouldRejectUserTokenWithoutConfigRead | HTTP_API | ⏳ | |
| T-APO-PORT-020-01 | F-APO-PORT-020 | namespaceLockShouldRejectUserTokenWithoutAssignRole | HTTP_API | ⏳ | |
| T-APO-PORT-020-02 | F-APO-PORT-020 | namespaceLockShouldAllowAuthorizedConsumerToken | HTTP_API | ⏳ | |
| T-APO-PORT-020-03 | F-APO-PORT-020 | namespaceLockShouldRejectUnauthorizedConsumerToken | HTTP_API | ⏳ | |
| T-APO-PORT-026-01 | F-APO-PORT-026 | missingNamespacesShouldRejectUserTokenWithoutMetadataScope | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PORT-026-02 | F-APO-PORT-026 | missingNamespacesShouldAllowAuthorizedConsumerToken | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PORT-026-03 | F-APO-PORT-026 | missingNamespacesShouldRejectUnauthorizedConsumerToken | HTTP_API | ⏭️ | Feature ⚪ |

## 27. portal OpenAPI — Items param bind (`ItemControllerParamBindLowLevelTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/ItemControllerParamBindLowLevelTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PITEM-002-01 | F-APO-PITEM-002 | createItemShouldBindFromPathAndBody | HTTP_API | ⏳ | |
| T-APO-PITEM-002-02 | F-APO-PITEM-002 | createItemShouldRejectUserTokenWithoutModifyPermission | HTTP_API | ⏳ | |
| T-APO-PITEM-002-03 | F-APO-PITEM-002 | createItemShouldAllowAuthorizedConsumerToken | HTTP_API | ⏳ | |
| T-APO-PITEM-005-01 | F-APO-PITEM-005 | updateItemShouldBindFromPathAndBody | HTTP_API | ⏳ | |
| T-APO-PITEM-005-02 | F-APO-PITEM-005 | updateItemShouldRejectUserTokenWithoutModifyPermission | HTTP_API | ⏳ | |
| T-APO-PITEM-006-01 | F-APO-PITEM-006 | deleteItemShouldUseCurrentPortalUser | HTTP_API | ⏳ | |
| T-APO-PITEM-006-02 | F-APO-PITEM-006 | deleteItemShouldRejectConsumerWithoutOperator | HTTP_API | ⏳ | |
| T-APO-PITEM-001-01 | F-APO-PITEM-001 | findItemsShouldRejectUserTokenWithoutConfigRead | HTTP_API | ⏳ | |
| T-APO-PITEM-004-01 | F-APO-PITEM-004 | getItemShouldRejectUserTokenWithoutConfigRead | HTTP_API | ⏳ | |
| T-APO-PITEM-008-01 | F-APO-PITEM-008 | findBranchItemsShouldRejectUserTokenWithoutConfigRead | HTTP_API | ⏳ | |
| T-APO-PITEM-007-01 | F-APO-PITEM-007 | getItemByEncodedKeyShouldRejectUserTokenWithoutConfigRead | HTTP_API | ⏳ | |
| T-APO-PITEM-002-04 | F-APO-PITEM-002 | createItemShouldRejectAnonymousWriteRequest | HTTP_API | ⏳ | |
| T-APO-PITEM-005-03 | F-APO-PITEM-005 | updateItemShouldRejectAnonymousWriteRequest | HTTP_API | ⏳ | |
| T-APO-PITEM-006-03 | F-APO-PITEM-006 | deleteItemShouldRejectAnonymousWriteRequest | HTTP_API | ⏳ | |

## 28. portal OpenAPI — Releases & branches (`ReleaseBranchInstanceControllerTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/ReleaseBranchInstanceControllerTest.java`
> Type: `@ExtendWith(MockitoExtension.class)` — tests controller delegation logic

### 28a. Release tests

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PREL-001-01 | F-APO-PREL-001 | createReleaseShouldUseCurrentPortalUserAndIgnorePayloadReleasedBy | HTTP_API | ⏳ | |
| T-APO-PREL-001-02 | F-APO-PREL-001 | createReleaseShouldUseCurrentUserTokenUserAndIgnorePayloadReleasedBy | HTTP_API | ⏳ | |
| T-APO-PREL-001-03 | F-APO-PREL-001 | createReleaseShouldRejectUserTokenEmergencyPublishWhenEnvDisallowsIt | HTTP_API | ⏳ | |
| T-APO-PREL-001-04 | F-APO-PREL-001 | createReleaseShouldKeepConsumerPayloadReleasedByForLegacyClients | HTTP_API | ⏳ | |
| T-APO-PREL-001-05 | F-APO-PREL-001 | createReleaseShouldRejectConsumerWithoutPayloadOrQueryOperator | HTTP_API | ⏳ | |
| T-APO-PREL-005-01 | F-APO-PREL-005 | rollbackShouldUseCurrentPortalUserAndToReleaseId | HTTP_API | ⏳ | |
| T-APO-PREL-005-02 | F-APO-PREL-005 | rollbackShouldRequireConsumerOperator | HTTP_API | ⏳ | |
| T-APO-PREL-005-03 | F-APO-PREL-005 | rollbackShouldRejectPermissionDeniedRelease | HTTP_API | ⏳ | |
| T-APO-PREL-001-06 | F-APO-PREL-001 | createReleaseShouldRejectAnonymousWriteRequest | HTTP_API | ⏳ | |
| T-APO-PREL-007-01 | F-APO-PREL-007 | createGrayReleaseShouldUseBranchClusterAndCurrentPortalUser | HTTP_API | ⏳ | |
| T-APO-PREL-007-02 | F-APO-PREL-007 | createGrayReleaseShouldRejectUserTokenEmergencyPublishWhenEnvDisallowsIt | HTTP_API | ⏳ | |
| T-APO-PREL-008-01 | F-APO-PREL-008 | createGrayDelReleaseShouldRejectUserTokenEmergencyPublishWhenEnvDisallowsIt | HTTP_API | ⏳ | |
| T-APO-PREL-006-01 | F-APO-PREL-006 | compareReleaseShouldReturnGeneratedDiffShape | HTTP_API | ⏳ | |
| T-APO-PREL-006-02 | F-APO-PREL-006 | compareReleaseShouldAllowZeroReleaseIdSentinel | HTTP_API | ⏳ | |
| T-APO-PREL-006-03 | F-APO-PREL-006 | compareReleaseShouldRejectHiddenPortalRelease | HTTP_API | ⏳ | |
| T-APO-PREL-006-04 | F-APO-PREL-006 | compareReleaseShouldRejectHiddenUserTokenRelease | HTTP_API | ⏳ | |
| T-APO-PREL-006-05 | F-APO-PREL-006 | compareReleaseShouldRejectConsumerWithoutReleasePermission | HTTP_API | ⏳ | |
| T-APO-PREL-004-01 | F-APO-PREL-004 | getReleaseByIdShouldAcceptUnsignedDatabaseReleaseIdRange | HTTP_API | ⏳ | |
| T-APO-PREL-003-01 | F-APO-PREL-003 | findActiveReleasesShouldRejectConsumerWithoutReleasePermission | HTTP_API | ⏳ | |
| T-APO-PREL-003-02 | F-APO-PREL-003 | findActiveReleasesShouldDefaultMissingPageAndSize | HTTP_API | ⏳ | |
| T-APO-PREL-002-01 | F-APO-PREL-002 | loadLatestActiveReleaseShouldReturnEmptyBodyWhenNoActiveReleaseExists | HTTP_API | ⏳ | |
| T-APO-PREL-002-02 | F-APO-PREL-002 | loadLatestActiveReleaseShouldRejectUserTokenWithoutConfigRead | HTTP_API | ⏳ | |
| T-APO-PREL-003-03 | F-APO-PREL-003 | findActiveReleasesShouldRejectUserTokenWithoutConfigRead | HTTP_API | ⏳ | |
| T-APO-PREL-002-03 | F-APO-PREL-002 | loadLatestActiveReleaseShouldRejectConsumerWithoutReleasePermission | HTTP_API | ⏳ | |

### 28b. Branch tests

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PREL-010-01 | F-APO-PREL-010 | createBranchShouldUseCurrentPortalUser | HTTP_API | ⏳ | |
| T-APO-PREL-010-02 | F-APO-PREL-010 | createBranchShouldAcceptLowercaseEnv | HTTP_API | ⏳ | |
| T-APO-PREL-010-03 | F-APO-PREL-010 | createBranchShouldUseCurrentUserTokenUser | HTTP_API | ⏳ | |
| T-APO-PREL-009-01 | F-APO-PREL-009 | mergeBranchShouldUseCurrentPortalUserAndDeleteFlag | HTTP_API | ⏳ | |
| T-APO-PREL-009-02 | F-APO-PREL-009 | mergeBranchShouldDefaultDeleteBranchToTrue | HTTP_API | ⏳ | |
| T-APO-PREL-009-03 | F-APO-PREL-009 | mergeBranchShouldRejectUserTokenEmergencyPublishWhenEnvDisallowsIt | HTTP_API | ⏳ | |
| T-APO-PREL-012-01 | F-APO-PREL-012 | updateBranchRulesShouldUseCurrentPortalUserAndPathFields | HTTP_API | ⏳ | |
| T-APO-PREL-012-02 | F-APO-PREL-012 | canUpdateBranchRulesShouldAllowPortalUserWithOperatePermission | HTTP_API | ⏳ | |
| T-APO-PREL-010-04 | F-APO-PREL-010 | canCreateBranchShouldRequireUserTokenModifyPermission | HTTP_API | ⏳ | |
| T-APO-PREL-009-04 | F-APO-PREL-009 | canMergeBranchShouldRequireUserTokenReleasePermission | HTTP_API | ⏳ | |
| T-APO-PREL-012-03 | F-APO-PREL-012 | canUpdateBranchRulesShouldRequireUserTokenModifyPermission | HTTP_API | ⏳ | |
| T-APO-PREL-010-05 | F-APO-PREL-010 | findBranchShouldRejectUserTokenWithoutConfigRead | HTTP_API | ⏳ | |
| T-APO-PREL-012-04 | F-APO-PREL-012 | getBranchGrayRulesShouldRejectUserTokenWithoutConfigRead | HTTP_API | ⏳ | |

### 28c. Instance tests

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PMISC-003-01 | F-APO-PMISC-003 | getByNamespaceShouldReturnGeneratedInstancePage | HTTP_API | ⏳ | |
| T-APO-PMISC-003-02 | F-APO-PMISC-003 | getByNamespaceShouldDefaultMissingPageAndSize | HTTP_API | ⏳ | |
| T-APO-PMISC-003-03 | F-APO-PMISC-003 | getByNamespaceShouldRejectConsumerWithoutNamespacePermission | HTTP_API | ⏳ | |
| T-APO-PMISC-003-04 | F-APO-PMISC-003 | getByNamespaceShouldRejectUserTokenWithoutConfigRead | HTTP_API | ⏳ | |
| T-APO-PMISC-003-05 | F-APO-PMISC-003 | getByNamespaceShouldReturnEmptyPageWhenPortalUserShouldHideConfig | HTTP_API | ⏳ | |
| T-APO-PMISC-002-01 | F-APO-PMISC-002 | getByReleaseShouldDefaultMissingPageAndSize | HTTP_API | ⏳ | |
| T-APO-PMISC-004-01 | F-APO-PMISC-004 | getByReleasesNotInShouldReturnGeneratedInstances | HTTP_API | ⏳ | |
| T-APO-PMISC-002-02 | F-APO-PMISC-002 | getByReleaseShouldRejectConsumerWithoutReleasePermission | HTTP_API | ⏳ | |
| T-APO-PMISC-002-03 | F-APO-PMISC-002 | getByReleaseShouldRejectUserTokenWithoutConfigRead | HTTP_API | ⏳ | |
| T-APO-PMISC-004-02 | F-APO-PMISC-004 | getByReleasesNotInShouldRejectConsumerWithoutNamespacePermission | HTTP_API | ⏳ | |
| T-APO-PMISC-004-03 | F-APO-PMISC-004 | getByReleasesNotInShouldRejectUserTokenWithoutConfigRead | HTTP_API | ⏳ | |
| T-APO-PMISC-004-04 | F-APO-PMISC-004 | getByReleasesNotInShouldReturnEmptyListWhenPortalUserShouldHideConfig | HTTP_API | ⏳ | |
| T-APO-PMISC-004-05 | F-APO-PMISC-004 | getByReleasesNotInShouldRejectInvalidReleaseIds | HTTP_API | ⏳ | |
| T-APO-PMISC-001-01 | F-APO-PMISC-001 | getInstanceCountByNamespaceShouldRejectConsumerWithoutNamespacePermission | HTTP_API | ⏳ | |
| T-APO-PMISC-001-02 | F-APO-PMISC-001 | getInstanceCountByNamespaceShouldRejectUserTokenWithoutConfigRead | HTTP_API | ⏳ | |
| T-APO-PMISC-001-03 | F-APO-PMISC-001 | getInstanceCountByNamespaceShouldReturnZeroWhenPortalUserShouldHideConfig | HTTP_API | ⏳ | |

## 29. portal OpenAPI — Access keys (`AccessKeyControllerParamBindLowLevelTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/AccessKeyControllerParamBindLowLevelTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PMISC-006-01 | F-APO-PMISC-006 | enableAccessKeyShouldUseCurrentPortalUserAndDefaultMode | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-006-02 | F-APO-PMISC-006 | disableAccessKeyShouldRejectBlankConsumerOperator | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-005-01 | F-APO-PMISC-005 | createAccessKeyShouldUseConsumerOperatorQuery | HTTP_API | ⏳ | |
| T-APO-PMISC-005-02 | F-APO-PMISC-005 | findAccessKeysShouldRejectUserTokenWithoutEnvScope | HTTP_API | ⏳ | |

## 30. portal OpenAPI — Permissions (`PermissionControllerTest`, `PermissionControllerParamBindLowLevelTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/PermissionControllerTest.java`
> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/PermissionControllerParamBindLowLevelTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PMISC-009-01 | F-APO-PMISC-009 | getAppRolesShouldRejectUserTokenWithoutAppManageRole | HTTP_API | ⏳ | |
| T-APO-PMISC-009-02 | F-APO-PMISC-009 | getAppRolesShouldAllowUserTokenWithAppManageRole | HTTP_API | ⏳ | |
| T-APO-PMISC-008-01 | F-APO-PMISC-008 | hasAppPermissionShouldRejectUserTokenWithoutAppManageRole | HTTP_API | ⏳ | |
| T-APO-PMISC-008-02 | F-APO-PMISC-008 | getNamespaceEnvRoleUsersShouldRejectUserTokenWithoutNamespaceScope | HTTP_API | ⏳ | |
| T-APO-PMISC-009-03 | F-APO-PMISC-009 | assignClusterNamespaceRoleShouldRejectUserTokenWithoutClusterScope | HTTP_API | ⏳ | |
| T-APO-PMISC-007-01 | F-APO-PMISC-007 | hasRootPermissionShouldRejectUserTokenWithoutSystemAdmin | HTTP_API | ⏳ | |
| T-APO-PMISC-007-02 | F-APO-PMISC-007 | hasRootPermissionShouldAllowUserTokenWithSystemAdmin | HTTP_API | ⏳ | |
| T-APO-PMISC-007-03 | F-APO-PMISC-007 | hasCreateApplicationPermissionShouldRejectUserTokenWithoutSystemAdmin | HTTP_API | ⏳ | |
| T-APO-PMISC-009-04 | F-APO-PMISC-009 | initAppPermissionShouldAllowUserTokenWithAppManageRole | HTTP_API | ⏳ | |
| T-APO-PMISC-007-04 | F-APO-PMISC-007 | isManageAppMasterPermissionEnabledShouldRejectUserTokenWithoutSystemAdmin | HTTP_API | ⏳ | |
| T-APO-PMISC-007-05 | F-APO-PMISC-007 | isManageAppMasterPermissionEnabledShouldAllowUserTokenWithSystemAdmin | HTTP_API | ⏳ | |
| T-APO-PMISC-008-03 | F-APO-PMISC-008 | hasAppPermissionShouldUseExplicitUserIdQuery | HTTP_API | ⏳ | |
| T-APO-PMISC-009-05 | F-APO-PMISC-009 | assignNamespaceRoleShouldBindTargetUserAndResolvedOperator | HTTP_API | ⏳ | |
| T-APO-PMISC-009-06 | F-APO-PMISC-009 | initAppPermissionShouldUseCurrentPortalUser | HTTP_API | ⏳ | |
| T-APO-PMISC-009-07 | F-APO-PMISC-009 | initAppPermissionShouldAllowAuthorizedConsumerToken | HTTP_API | ⏳ | |
| T-APO-PMISC-009-08 | F-APO-PMISC-009 | initAppPermissionShouldRejectUnauthorizedConsumerToken | HTTP_API | ⏳ | |
| T-APO-PMISC-009-09 | F-APO-PMISC-009 | initClusterNamespacePermissionShouldUseCurrentPortalUser | HTTP_API | ⏳ | |
| T-APO-PMISC-009-10 | F-APO-PMISC-009 | initClusterNamespacePermissionShouldAllowAuthorizedConsumerToken | HTTP_API | ⏳ | |
| T-APO-PMISC-009-11 | F-APO-PMISC-009 | initClusterNamespacePermissionShouldRejectUnauthorizedConsumerToken | HTTP_API | ⏳ | |

## 31. portal OpenAPI — Users (`UserControllerTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/UserControllerTest.java`

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PMISC-011-01 | F-APO-PMISC-011 | getCurrentUserShouldReturnPortalSessionUser | HTTP_API | ⏳ | |
| T-APO-PMISC-012-01 | F-APO-PMISC-012 | searchUsersShouldDelegateToUserService | HTTP_API | ⏳ | |
| T-APO-PMISC-012-02 | F-APO-PMISC-012 | createOrUpdateUserShouldCreateViaSpringSecurityService | HTTP_API | ⏳ | |
| T-APO-PMISC-012-03 | F-APO-PMISC-012 | createOrUpdateUserShouldRejectUnauthorizedUserUpdate | HTTP_API | ⏳ | |
| T-APO-PMISC-012-04 | F-APO-PMISC-012 | searchUsersShouldRejectConsumerTokenWithoutManageUsersPermission | HTTP_API | ⏳ | |
| T-APO-PMISC-012-05 | F-APO-PMISC-012 | searchUsersShouldAllowConsumerTokenWithManageUsersPermission | HTTP_API | ⏳ | |
| T-APO-PMISC-012-06 | F-APO-PMISC-012 | getUserByUserIdShouldAllowConsumerTokenWithManageUsersPermission | HTTP_API | ⏳ | |
| T-APO-PMISC-012-07 | F-APO-PMISC-012 | getUserByUserIdShouldRejectUnknownUser | HTTP_API | ⏳ | |
| T-APO-PMISC-012-08 | F-APO-PMISC-012 | createOrUpdateUserShouldAllowConsumerTokenWithManageUsersPermission | HTTP_API | ⏳ | |
| T-APO-PMISC-012-09 | F-APO-PMISC-012 | createOrUpdateUserShouldRejectConsumerTokenWithoutManageUsersPermission | HTTP_API | ⏳ | |
| T-APO-PMISC-012-10 | F-APO-PMISC-012 | changeUserEnabledShouldAllowConsumerTokenWithManageUsersPermission | HTTP_API | ⏳ | |

## 32. portal OpenAPI — Management (`PortalManagementControllerTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/PortalManagementControllerTest.java`
> Covers: commits, release histories, portal/config DB config, item search, consumers, config import/export, namespace items export, page settings

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PREL-014-01 | F-APO-PREL-014 | findReleaseHistoriesShouldDefaultPaginationWhenOmitted | HTTP_API | ⏳ | |
| T-APO-PMISC-017-01 | F-APO-PMISC-017 | createOrUpdatePortalDBConfigShouldRejectBlankKey | HTTP_API | ⏳ | |
| T-APO-PMISC-017-02 | F-APO-PMISC-017 | createOrUpdateConfigDBConfigShouldRejectBlankValue | HTTP_API | ⏳ | |
| T-APO-PMISC-020-01 | F-APO-PMISC-020 | searchItemInfoByKeyOrValueShouldRejectNullCriteria | HTTP_API | ⏭️ | Feature ⚪ (at OpenAPI path) |
| T-APO-PMISC-020-02 | F-APO-PMISC-020 | searchItemInfoByKeyOrValueShouldAllowOneNullCriterion | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-014-01 | F-APO-PMISC-014 | createConsumerShouldAssignManageUsersRole | HTTP_API | ⏳ | |
| T-APO-PMISC-014-02 | F-APO-PMISC-014 | getConsumerTokenByAppIdShouldReturnConsumerToken | HTTP_API | ⏳ | |
| T-APO-PMISC-014-03 | F-APO-PMISC-014 | getConsumerListShouldReturnConsumerInfos | HTTP_API | ⏳ | |
| T-APO-PMISC-015-01 | F-APO-PMISC-015 | exportAllConfigsShouldReturnFileBackedResource | HTTP_API | ⏭️ | Feature ⚪ (at OpenAPI path) |
| T-APO-PMISC-015-02 | F-APO-PMISC-015 | exportAppConfigShouldReturnFileBackedResource | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-015-03 | F-APO-PMISC-015 | importAllConfigsShouldStreamMultipartInput | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-015-04 | F-APO-PMISC-015 | importAllConfigsShouldDefaultMissingConflictActionToIgnore | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-015-05 | F-APO-PMISC-015 | importAppConfigShouldStreamMultipartInput | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-015-06 | F-APO-PMISC-015 | importAppConfigShouldDefaultMissingConflictActionToIgnore | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PREL-013-01 | F-APO-PREL-013 | findCommitsShouldHideConfigWithoutNamespaceReadPermission | HTTP_API | ⏳ | |
| T-APO-PREL-013-02 | F-APO-PREL-013 | findCommitsShouldRejectInvalidEnv | HTTP_API | ⏳ | |
| T-APO-PREL-013-03 | F-APO-PREL-013 | findCommitsShouldAcceptLowercaseEnv | HTTP_API | ⏳ | |
| T-APO-PREL-013-04 | F-APO-PREL-013 | findCommitsShouldDefaultPaginationWhenOmitted | HTTP_API | ⏳ | |
| T-APO-PREL-013-05 | F-APO-PREL-013 | findCommitsByKeyShouldDefaultPaginationWhenOmitted | HTTP_API | ⏳ | |

## 33. portal OpenAPI — User tokens (`PortalUserTokenManagementControllerTest`, `UserTokenOpenApiControllerTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/PortalUserTokenManagementControllerTest.java`
> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/UserTokenOpenApiControllerTest.java`
> Feature: F-APO-PMISC-013 (user-tokens)

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PMISC-013-01 | F-APO-PMISC-013 | listUserTokensDelegatesToCurrentUser | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-013-02 | F-APO-PMISC-013 | createUserTokenDelegatesToCurrentUser | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-013-03 | F-APO-PMISC-013 | adminListDelegatesToAdminService | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-013-04 | F-APO-PMISC-013 | adminRevokeDelegatesWithCurrentOperator | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-013-05 | F-APO-PMISC-013 | adminDeleteDelegatesWithCurrentOperator | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-013-06 | F-APO-PMISC-013 | adminListRejectsNonPortalUserSession | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-013-07 | F-APO-PMISC-013 | adminListRejectsMissingPortalUserContext | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-013-08 | F-APO-PMISC-013 | adminEndpointsRequireSuperAdminPreAuthorize | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-013-09 | F-APO-PMISC-013 | currentShouldReturnUserTokenIdentityAndExplicitScope | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-013-10 | F-APO-PMISC-013 | currentShouldMakeUnboundedScopeExplicit | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-013-11 | F-APO-PMISC-013 | currentShouldSeparateReleaseReadAndPublishActions | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-013-12 | F-APO-PMISC-013 | currentShouldExposeGrantedOperationsForAlternativePermissions | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-013-13 | F-APO-PMISC-013 | currentShouldHideMetadataActionsForUserManageOnlyScope | HTTP_API | ⏭️ | Feature ⚪ |
| T-APO-PMISC-013-14 | F-APO-PMISC-013 | currentShouldRejectNonUserTokenIdentity | HTTP_API | ⏭️ | Feature ⚪ |

## 34. portal OpenAPI — Auth filter (`ConsumerAuthenticationFilterTest`, `PortalOpenApiAuthenticationScenariosTest`)

> Source: `apollo-portal/src/test/java/.../openapi/filter/ConsumerAuthenticationFilterTest.java`
> Source: `apollo-portal/src/test/java/.../portal/filter/PortalOpenApiAuthenticationScenariosTest.java`
> Feature: F-APO-PMISC-021 (portal token/session auth)

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PMISC-021-01 | F-APO-PMISC-021 | testAuthSuccessfully | INTERNAL | ⏭️ | Feature ⚪ — auth not enforced |
| T-APO-PMISC-021-02 | F-APO-PMISC-021 | testAuthFailed | INTERNAL | ⏭️ | Feature ⚪ |
| T-APO-PMISC-021-03 | F-APO-PMISC-021 | testRateLimitSuccessfully | INTERNAL | ⏭️ | Feature ⚪ |
| T-APO-PMISC-021-04 | F-APO-PMISC-021 | testRateLimitPartFailure | INTERNAL | ⏭️ | Feature ⚪ |
| T-APO-PMISC-021-05 | F-APO-PMISC-021 | PortalOpenApiAuthenticationScenariosTest (all scenarios) | HTTP_API | ⏭️ | Feature ⚪ — auth not enforced |

## 35. portal OpenAPI — Annotation parity (`OpenApiControllerAnnotationParityTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/OpenApiControllerAnnotationParityTest.java`
> Tests: @PreAuthorize / @Audit annotation parity between OpenAPI and legacy controllers

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-PORT-001-05 | F-APO-PORT-001 | appAuditAnnotationsShouldMatchLegacyController | INTERNAL | ⏭️ | Java annotation parity |
| T-APO-PORT-001-06 | F-APO-PORT-001 | appPermissionAnnotationsShouldMatchLegacyController | INTERNAL | ⏭️ | Java annotation parity |
| T-APO-PORT-014-05 | F-APO-PORT-014 | clusterAuditAnnotationsShouldMatchLegacyController | INTERNAL | ⏭️ | Java annotation parity |
| T-APO-PORT-014-06 | F-APO-PORT-014 | clusterPermissionAnnotationsShouldMatchLegacyController | INTERNAL | ⏭️ | Java annotation parity |
| T-APO-PREL-010-06 | F-APO-PREL-010 | namespaceBranchAuditAnnotationsShouldMatchLegacyController | INTERNAL | ⏭️ | Java annotation parity |
| T-APO-PMISC-008-04 | F-APO-PMISC-008 | permissionAnnotationsShouldMatchLegacyController | INTERNAL | ⏭️ | Java annotation parity |
| T-APO-PMISC-009-12 | F-APO-PMISC-009 | permissionAuditAnnotationsShouldCoverRoleAndInitializationMutations | INTERNAL | ⏭️ | Java annotation parity |
| T-APO-PMISC-009-13 | F-APO-PMISC-009 | openApiPreAuthorizeExpressionsShouldNotEmbedAuthTypeBranching | INTERNAL | ⏭️ | Java annotation parity |

## 36. portal — Legacy WebAPI deprecation (`LegacyWebApiControllerDeprecationTest`)

> Source: `apollo-portal/src/test/java/.../portal/controller/LegacyWebApiControllerDeprecationTest.java`
> Feature: F-APO-LEGACY-* (deprecated, low priority)

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-LEGACY-001-01 | F-APO-LEGACY-001 | LegacyWebApiControllerDeprecationTest (deprecation tests) | INTERNAL | ⏭️ | Feature ⚪ — legacy deprecated |

## 37. portal — Controller unit tests (INTERNAL, not ported)

> Various portal controller tests under `.../portal/controller/` that use Mockito mocks.
> These test Java service delegation, not HTTP contracts, and are not portable to batata.

| T-ID | Feature | Upstream file | Type | Status | Skip reason |
|------|---------|---------------|------|--------|-------------|
| T-APO-PITEM-011-01 | F-APO-PITEM-011 | ItemControllerTest (yamlSyntaxCheck*) | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-PITEM-003-01 | F-APO-PITEM-003 | ItemControllerAuthIntegrationTest | INTERNAL | ⏭️ | Permission integration test |
| T-APO-PREL-013-06 | F-APO-PREL-013 | CommitControllerTest | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-PMISC-015-07 | F-APO-PMISC-015 | ConfigsExportControllerTest | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-PMISC-015-08 | F-APO-PMISC-015 | ConfigsImportControllerTest | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-PMISC-014-04 | F-APO-PMISC-014 | ConsumerControllerTest | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-PMISC-020-03 | F-APO-PMISC-020 | GlobalSearchControllerTest | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-PMISC-020-04 | F-APO-PMISC-020 | SearchControllerTest | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-PMISC-016-01 | F-APO-PMISC-016 | SystemInfoControllerTest | INTERNAL | ⏭️ | Feature ⚪ |
| T-APO-PMISC-018-01 | F-APO-PMISC-018 | ReleaseHistoryControllerTest | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-PMISC-017-03 | F-APO-PMISC-017 | ServerConfigControllerTest | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-PMISC-012-11 | F-APO-PMISC-012 | UserInfoControllerTest | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-PMISC-019-01 | F-APO-PMISC-019 | FavoriteServiceTest | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-PMISC-021-06 | F-APO-PMISC-021 | SignInControllerTest | INTERNAL | ⏭️ | Mockito unit test |
| T-APO-PMISC-021-07 | F-APO-PMISC-021 | SsoHeartbeatControllerTest | INTERNAL | ⏭️ | Mockito unit test |

## 38. SDK client — Java compatibility (`ApolloOpenApiJavaClientCompatibilityTest`)

> Source: `apollo-portal/src/test/java/.../openapi/v1/controller/ApolloOpenApiJavaClientCompatibilityTest.java`
> Type: `@SpringBootTest` — tests Java SDK client API compatibility against the OpenAPI v1 endpoints

| T-ID | Feature | Upstream method | Type | Status | Skip reason |
|------|---------|-----------------|------|--------|-------------|
| T-APO-SDK-001-01 | F-APO-PORT-001, F-APO-PMISC-010 | legacyAppAndOrganizationMethodsShouldRemainCompatible | SDK_CLIENT | ⏳ | app + organization APIs |
| T-APO-SDK-001-02 | F-APO-PORT-014, F-APO-PORT-017 | legacyClusterAndNamespaceMethodsShouldRemainCompatible | SDK_CLIENT | ⏳ | cluster + namespace APIs |
| T-APO-SDK-001-03 | F-APO-PITEM-002, F-APO-PITEM-005, F-APO-PITEM-006 | legacyItemMethodsShouldPreserveOperatorsEncodedKeysAndPaging | SDK_CLIENT | ⏳ | item APIs |
| T-APO-SDK-001-04 | F-APO-PORT-001, F-APO-PORT-005 | openApiBaseDtoResponsesShouldIncludeAuditDisplayNames | SDK_CLIENT | ⏳ | DTO audit fields |
| T-APO-SDK-001-05 | F-APO-PREL-001, F-APO-PREL-005, F-APO-PMISC-002 | legacyReleaseAndInstanceMethodsShouldRemainCompatible | SDK_CLIENT | ⏳ | release + instance APIs |

## 39. portal — OpenAPI service tests (INTERNAL, not ported)

> Various service-layer tests under `.../openapi/server/service/` that use Mockito mocks.
> These test Java service delegation logic, not HTTP contracts.

| T-ID | Feature | Upstream file | Type | Status | Skip reason |
|------|---------|---------------|------|--------|-------------|
| T-APO-PMISC-005-03 | F-APO-PMISC-005 | ServerAccessKeyOpenApiServiceTest | INTERNAL | ⏭️ | Service-layer test |
| T-APO-PORT-005-03 | F-APO-PORT-005 | ServerAppOpenApiServiceTest | INTERNAL | ⏭️ | Service-layer test |
| T-APO-PITEM-001-02 | F-APO-PITEM-001 | ServerItemOpenApiServiceTest | INTERNAL | ⏭️ | Service-layer test |
| T-APO-PORT-022-10 | F-APO-PORT-022 | ServerNamespaceManagementOpenApiServiceTest | INTERNAL | ⏭️ | Service-layer test |
| T-APO-PMISC-009-14 | F-APO-PMISC-009 | ServerPermissionOpenApiServiceTest | INTERNAL | ⏭️ | Service-layer test |

## 40. portal — OpenAPI auth/util tests (INTERNAL, not ported)

> Auth and utility tests under `.../openapi/auth/`, `.../openapi/util/`, `.../openapi/service/`.

| T-ID | Feature | Upstream file | Type | Status | Skip reason |
|------|---------|---------------|------|--------|-------------|
| T-APO-PMISC-021-08 | F-APO-PMISC-021 | ConsumerPermissionValidatorTest | INTERNAL | ⏭️ | Auth logic test |
| T-APO-PMISC-014-05 | F-APO-PMISC-014 | ConsumerServiceTest | INTERNAL | ⏭️ | Service-layer test |
| T-APO-PMISC-014-06 | F-APO-PMISC-014 | ConsumerServiceIntegrationTest | INTERNAL | ⏭️ | DB integration test |
| T-APO-PMISC-009-15 | F-APO-PMISC-009 | ConsumerRolePermissionServiceTest | INTERNAL | ⏭️ | DB integration test |
| T-APO-PMISC-021-09 | F-APO-PMISC-021 | ConsumerAuthUtilTest | INTERNAL | ⏭️ | Utility test |
| T-APO-PMISC-021-10 | F-APO-PMISC-021 | ConsumerAuditUtilTest | INTERNAL | ⏭️ | Utility test |
| T-APO-PORT-009-02 | F-APO-PORT-009 | OpenApiModelConvertersTest | INTERNAL | ⏭️ | DTO converter test |
| T-APO-PMISC-021-11 | F-APO-PMISC-021 | OpenApiOperatorResolverTest | INTERNAL | ⏭️ | Operator resolution test |

---

# Summary

| Section | Description | HTTP_API | SDK_CLIENT | INTERNAL | Total |
|---------|-------------|----------|------------|----------|-------|
| 1 | configservice — Config fetch | 20 | 0 | 0 | 20 |
| 2 | configservice — Config file fetch | 10 | 0 | 0 | 10 |
| 3 | configservice — Long polling v1 | 9 | 0 | 0 | 9 |
| 4 | configservice — Long polling v2 | 17 | 0 | 0 | 17 |
| 5 | configservice — AccessKey filter | 0 | 0 | 1 | 1 |
| 6 | adminservice — Apps | 7 | 0 | 0 | 7 |
| 7 | adminservice — App exception | 2 | 0 | 7 | 9 |
| 8 | adminservice — Clusters | 1 | 0 | 2 | 3 |
| 9 | adminservice — AppNamespace | 1 | 0 | 0 | 1 |
| 10 | adminservice — Namespace | 1 | 0 | 0 | 1 |
| 11 | adminservice — Items | 4 | 0 | 0 | 4 |
| 12 | adminservice — ItemSet | 6 | 0 | 0 | 6 |
| 13 | adminservice — Release | 1 | 0 | 1 | 2 |
| 14 | adminservice — Server config | 6 | 0 | 0 | 6 |
| 15 | adminservice — Instance config | 0 | 0 | 6 | 6 |
| 16 | adminservice — Namespace lock | 0 | 0 | 13 | 13 |
| 17 | adminservice — Auth filter | 6 | 0 | 8 | 14 |
| 18 | portal — Apps | 20 | 0 | 0 | 20 |
| 19 | portal — Apps param bind | 18 | 0 | 0 | 18 |
| 20 | portal — Apps integration | 1 | 0 | 0 | 1 |
| 21 | portal — Clusters | 4 | 0 | 0 | 4 |
| 22 | portal — Clusters param bind | 8 | 0 | 0 | 8 |
| 23 | portal — Envs | 2 | 0 | 0 | 2 |
| 24 | portal — Organizations | 2 | 0 | 0 | 2 |
| 25 | portal — Namespace | 5 | 0 | 0 | 5 |
| 26 | portal — Namespace param bind | 15 | 0 | 0 | 15 |
| 27 | portal — Items param bind | 14 | 0 | 0 | 14 |
| 28a | portal — Releases | 25 | 0 | 0 | 25 |
| 28b | portal — Branches | 13 | 0 | 0 | 13 |
| 28c | portal — Instances | 16 | 0 | 0 | 16 |
| 29 | portal — Access keys | 2 | 0 | 0 | 2 |
| 30 | portal — Permissions | 19 | 0 | 0 | 19 |
| 31 | portal — Users | 11 | 0 | 0 | 11 |
| 32 | portal — Management | 19 | 0 | 0 | 19 |
| 33 | portal — User tokens | 14 | 0 | 0 | 14 |
| 34 | portal — Auth filter | 0 | 0 | 5 | 5 |
| 35 | portal — Annotation parity | 0 | 0 | 8 | 8 |
| 36 | portal — Legacy deprecation | 0 | 0 | 1 | 1 |
| 37 | portal — Controller unit tests | 0 | 0 | 15 | 15 |
| 38 | SDK client — Java compatibility | 0 | 5 | 0 | 5 |
| 39 | portal — Service tests | 0 | 0 | 5 | 5 |
| 40 | portal — Auth/util tests | 0 | 0 | 8 | 8 |
| **Total** | | **297** | **5** | **80** | **382** |

> **Porting priorities:**
> - **High**: Sections 1–4 (configservice, 56 tests) — core client contract, must pass for SDK compatibility
> - **High**: Section 38 (SDK client, 5 tests) — Java SDK compatibility validation
> - **Medium**: Sections 6–14, 18–32 (adminservice + portal OpenAPI, ~242 tests) — management API contract
> - **Low**: Sections 5, 15–17, 33–40 (80 INTERNAL tests) — Java-specific logic, not portable to Rust
