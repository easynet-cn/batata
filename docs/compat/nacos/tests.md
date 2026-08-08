# Nacos Test Inventory

Upstream baseline: **Nacos 3.3.0-SNAPSHOT** (`~/work/github/easynet-cn/nacos`).

> Maps upstream Nacos API contract tests to batata F-IDs.
> Tests are the **source of truth** for compatibility — if upstream tests pass against batata, batata is compatible.
>
> Port status: `⬜ not ported` | `🔄 in progress` | `✅ ported` | `⏭️ skip (feature ⚪/⛔)`
>
> Test type: `HTTP_API` (HTTP integration test) | `GRPC_API` (gRPC integration test) | `SDK_CLIENT` (SDK client API test)

---

# Module 1: Admin API Tests (HTTP)

> Source: `test/openapi-test/src/test/java/com/alibaba/nacos/test/adminapi/`
> Server: `127.0.0.1:8848`, path prefix `/nacos/v3/admin/`

## 1.1 Config Admin API

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-ADM-CS-001-01 | F-NAC-ADM-CS-001 | ConfigAdminApiOpenApiITCase | testPublishQueryUpdateMetadataAndDeleteConfig | HTTP_API | ⬜ |
| T-NAC-ADM-CS-001-02 | F-NAC-ADM-CS-001 | ConfigAdminApiOpenApiITCase | testPublishConfigRequiredParametersReturnBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-CS-001-03 | F-NAC-ADM-CS-001 | ConfigAdminApiOpenApiITCase | testQueryConfigNotFoundAndInvalidNamespaceReturnControlledErrors | HTTP_API | ⬜ |
| T-NAC-ADM-CS-001-04 | F-NAC-ADM-CS-001 | ConfigAdminApiOpenApiITCase | testDeleteConfigRequiredParametersReturnBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-CS-001-05 | F-NAC-ADM-CS-001 | ConfigAdminApiOpenApiITCase | testMetadataUpdateRequiresExistingConfigIdentityFields | HTTP_API | ⬜ |
| T-NAC-ADM-CS-003-01 | F-NAC-ADM-CS-003 | ConfigBetaAdminApiOpenApiITCase | testQueryAndStopBetaConfig | HTTP_API | ⬜ |
| T-NAC-ADM-CS-003-02 | F-NAC-ADM-CS-003 | ConfigBetaAdminApiOpenApiITCase | testBetaRequiredParametersAndAbsentConfigReturnControlledErrors | HTTP_API | ⬜ |
| T-NAC-ADM-CS-002-01 | F-NAC-ADM-CS-002 | ConfigBatchDeleteAdminApiOpenApiITCase | testBatchDeleteExistingConfigsAndIgnoreMissingId | HTTP_API | ⬜ |
| T-NAC-ADM-CS-002-02 | F-NAC-ADM-CS-002 | ConfigBatchDeleteAdminApiOpenApiITCase | testBatchDeleteSkipsIdsOutsideNamespace | HTTP_API | ⬜ |
| T-NAC-ADM-CS-002-03 | F-NAC-ADM-CS-002 | ConfigBatchDeleteAdminApiOpenApiITCase | testBatchDeleteIdsValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-CS-013-01 | F-NAC-ADM-CS-013 | ConfigCapacityAdminApiOpenApiITCase | testUpdateAndQueryGroupCapacity | HTTP_API | ⬜ |
| T-NAC-ADM-CS-013-02 | F-NAC-ADM-CS-013 | ConfigCapacityAdminApiOpenApiITCase | testCapacityValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-CS-006-01 | F-NAC-ADM-CS-006 | ConfigCloneAdminApiOpenApiITCase | testCloneConfigToTargetIdentity | HTTP_API | ⬜ |
| T-NAC-ADM-CS-006-02 | F-NAC-ADM-CS-006 | ConfigCloneAdminApiOpenApiITCase | testCloneConfigWithSourceNamespace | HTTP_API | ⬜ |
| T-NAC-ADM-CS-006-03 | F-NAC-ADM-CS-006 | ConfigCloneAdminApiOpenApiITCase | testCloneConfigSkipsIdsOutsideSourceNamespace | HTTP_API | ⬜ |
| T-NAC-ADM-CS-006-04 | F-NAC-ADM-CS-006 | ConfigCloneAdminApiOpenApiITCase | testCloneValidationAndBusinessFailuresReturnResultEnvelope | HTTP_API | ⬜ |
| T-NAC-ADM-CS-007-01 | F-NAC-ADM-CS-007 | ConfigExportAdminApiOpenApiITCase | testExportConfigByIdReturnsZipWithContentAndMetadata | HTTP_API | ⬜ |
| T-NAC-ADM-CS-007-02 | F-NAC-ADM-CS-007 | ConfigExportAdminApiOpenApiITCase | testExportInvalidNamespaceReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-CS-007-03 | F-NAC-ADM-CS-007 | ConfigExportAdminApiOpenApiITCase | testExportByIdSkipsConfigsOutsideNamespace | HTTP_API | ⬜ |
| T-NAC-ADM-CS-004-01 | F-NAC-ADM-CS-004 | ConfigGrayAdminApiOpenApiITCase | testPublishAndQueryGrayConfig | HTTP_API | ⬜ |
| T-NAC-ADM-CS-004-02 | F-NAC-ADM-CS-004 | ConfigGrayAdminApiOpenApiITCase | testGrayValidationAndAbsentConfigReturnControlledErrors | HTTP_API | ⬜ |
| T-NAC-ADM-CS-009-01 | F-NAC-ADM-CS-009 | ConfigHistoryAdminApiOpenApiITCase | testHistoryListDetailPreviousAndNamespaceConfigs | HTTP_API | ⬜ |
| T-NAC-ADM-CS-009-02 | F-NAC-ADM-CS-009 | ConfigHistoryAdminApiOpenApiITCase | testHistoryValidationAndAbsentHistoryReturnControlledErrors | HTTP_API | ⬜ |
| T-NAC-ADM-CS-008-01 | F-NAC-ADM-CS-008 | ConfigImportAdminApiOpenApiITCase | testImportConfigZipPublishesConfig | HTTP_API | ⬜ |
| T-NAC-ADM-CS-008-02 | F-NAC-ADM-CS-008 | ConfigImportAdminApiOpenApiITCase | testImportMissingFileAndBadMetadataReturnFailureResult | HTTP_API | ⬜ |
| T-NAC-ADM-CS-001-06 | F-NAC-ADM-CS-001 | ConfigListAdminApiOpenApiITCase | testListConfigsByFuzzyDataIdAdvancedFiltersAndPagination | HTTP_API | ⬜ |
| T-NAC-ADM-CS-001-07 | F-NAC-ADM-CS-001 | ConfigListAdminApiOpenApiITCase | testListConfigAllowsBlankDataIdAndNoMatchReturnsEmptyPage | HTTP_API | ⬜ |
| T-NAC-ADM-CS-001-08 | F-NAC-ADM-CS-001 | ConfigListAdminApiOpenApiITCase | testListConfigInvalidPaginationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-CS-012-01 | F-NAC-ADM-CS-012 | ConfigListenerAdminApiOpenApiITCase | testConfigListenerQueryReturnsConfigQueryType | HTTP_API | ⬜ |
| T-NAC-ADM-CS-012-02 | F-NAC-ADM-CS-012 | ConfigListenerAdminApiOpenApiITCase | testIpListenerQueryAcceptsAllAndNamespaceFilter | HTTP_API | ⬜ |
| T-NAC-ADM-CS-012-03 | F-NAC-ADM-CS-012 | ConfigListenerAdminApiOpenApiITCase | testListenerRequiredParametersReturnBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-CS-014-01 | F-NAC-ADM-CS-014 | ConfigMetricsAdminApiOpenApiITCase | testLocalAndClusterMetricsReturnResultMaps | HTTP_API | ⬜ |
| T-NAC-ADM-CS-014-02 | F-NAC-ADM-CS-014 | ConfigMetricsAdminApiOpenApiITCase | testMetricsValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-CS-015-01 | F-NAC-ADM-CS-015 | ConfigOpsAdminApiOpenApiITCase | testLocalCacheDumpReturnsSuccessEnvelope | HTTP_API | ⬜ |
| T-NAC-ADM-CS-015-02 | F-NAC-ADM-CS-015 | ConfigOpsAdminApiOpenApiITCase | testOpsRequiredParametersReturnBadRequest | HTTP_API | ⬜ |

## 1.2 Naming Admin API

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-ADM-NS-001-01 | F-NAC-ADM-NS-001 | ServiceAdminApiOpenApiITCase | testCreateDetailUpdateListAndDeleteService | HTTP_API | ⬜ |
| T-NAC-ADM-NS-001-02 | F-NAC-ADM-NS-001 | ServiceAdminApiOpenApiITCase | testServiceValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-NS-002-01 | F-NAC-ADM-NS-002 | InstanceAdminApiOpenApiITCase | testRegisterWithDefaultsDetailUpdatePartialUpdateAndDelete | HTTP_API | ⬜ |
| T-NAC-ADM-NS-002-02 | F-NAC-ADM-NS-002 | InstanceAdminApiOpenApiITCase | testListFiltersByClusterAndHealthyOnly | HTTP_API | ⬜ |
| T-NAC-ADM-NS-002-03 | F-NAC-ADM-NS-002 | InstanceAdminApiOpenApiITCase | testInstanceValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-NS-002-04 | F-NAC-ADM-NS-002 | InstanceAdminApiOpenApiITCase | testMissingAndMismatchedInstanceReturnControlledErrors | HTTP_API | ⬜ |
| T-NAC-ADM-NS-003-01 | F-NAC-ADM-NS-003 | InstanceMetadataAdminApiOpenApiITCase | testBatchUpdateAndDeleteInstanceMetadata | HTTP_API | ⬜ |
| T-NAC-ADM-NS-003-02 | F-NAC-ADM-NS-003 | InstanceMetadataAdminApiOpenApiITCase | testBatchMetadataValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-NS-004-01 | F-NAC-ADM-NS-004 | ClusterAdminApiOpenApiITCase | testUpdateClusterMetadataAndDefaultGroup | HTTP_API | ⬜ |
| T-NAC-ADM-NS-004-02 | F-NAC-ADM-NS-004 | ClusterAdminApiOpenApiITCase | testClusterValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-NS-004-03 | F-NAC-ADM-NS-004 | ClusterAdminApiOpenApiITCase | testUpdateClusterForMissingServiceReturnsControlledError | HTTP_API | ⬜ |
| T-NAC-ADM-NS-005-01 | F-NAC-ADM-NS-005 | HealthAdminApiOpenApiITCase | testCheckersReturnBuiltInHealthCheckerTypes | HTTP_API | ⬜ |
| T-NAC-ADM-NS-006-01 | F-NAC-ADM-NS-006 | HealthAdminApiOpenApiITCase | testUpdatePersistentInstanceHealthWhenClusterUsesNoneChecker | HTTP_API | ⬜ |
| T-NAC-ADM-NS-006-02 | F-NAC-ADM-NS-006 | HealthAdminApiOpenApiITCase | testUpdateHealthValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-NS-006-03 | F-NAC-ADM-NS-006 | HealthAdminApiOpenApiITCase | testUpdateHealthWithoutManualCheckerReturnsControlledError | HTTP_API | ⬜ |
| T-NAC-ADM-NS-007-01 | F-NAC-ADM-NS-008 | OperatorAdminApiOpenApiITCase | testSwitchesMetricsAndLogLevelOperations | HTTP_API | ⬜ |
| T-NAC-ADM-NS-007-02 | F-NAC-ADM-NS-008 | OperatorAdminApiOpenApiITCase | testOperatorValidationReturnsBadRequestAndControlledServerError | HTTP_API | ⬜ |
| T-NAC-ADM-NS-007-03 | F-NAC-ADM-NS-007 | ClientAdminApiOpenApiITCase | testClientDiagnosticsForRegisteredHttpInstance | HTTP_API | ⬜ |
| T-NAC-ADM-NS-007-04 | F-NAC-ADM-NS-007 | ClientAdminApiOpenApiITCase | testClientValidationAndNotFoundReturnControlledErrors | HTTP_API | ⬜ |

## 1.3 Core Admin API

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-ADM-CORE-001-01 | F-NAC-ADM-CORE-001 | NamespaceAdminApiOpenApiITCase | testCreateDetailUpdateListCheckAndDeleteNamespace | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-001-02 | F-NAC-ADM-CORE-001 | NamespaceAdminApiOpenApiITCase | testNamespaceValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-002-01 | F-NAC-ADM-CORE-002 | CoreClusterAdminApiOpenApiITCase | testSelfAndNodeListExposeCurrentMemberState | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-002-02 | F-NAC-ADM-CORE-002 | CoreClusterAdminApiOpenApiITCase | testClusterValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-003-01 | F-NAC-ADM-CORE-003 | CoreOpsAdminApiOpenApiITCase | testIdsAndLogUpdate | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-003-02 | F-NAC-ADM-CORE-003 | CoreOpsAdminApiOpenApiITCase | testCoreOpsValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-004-01 | F-NAC-ADM-CORE-004 | CoreStateAdminApiOpenApiITCase | testServerStateLivenessAndReadiness | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-005-01 | F-NAC-ADM-CORE-005 | ServerLoaderAdminApiOpenApiITCase | testCurrentClientsAndClusterMetrics | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-005-02 | F-NAC-ADM-CORE-005 | ServerLoaderAdminApiOpenApiITCase | testLoaderValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-006-01 | F-NAC-ADM-CORE-006 | PluginAdminApiOpenApiITCase | testListFilterAndDetailPluginInventory | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-006-02 | F-NAC-ADM-CORE-006 | PluginAdminApiOpenApiITCase | testNacosAuthPluginConfigMetadata | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-006-03 | F-NAC-ADM-CORE-006 | PluginAdminApiOpenApiITCase | testLdapAuthPluginConfigMetadata | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-006-04 | F-NAC-ADM-CORE-006 | PluginAdminApiOpenApiITCase | testOidcAuthPluginConfigMetadata | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-006-05 | F-NAC-ADM-CORE-006 | PluginAdminApiOpenApiITCase | testPluginDetailNotFoundReturnsControlledError | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-006-06 | F-NAC-ADM-CORE-006 | PluginAdminApiOpenApiITCase | testPluginMutationValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-006-07 | F-NAC-ADM-CORE-006 | PluginAdminApiOpenApiITCase | testCriticalAndExclusiveStateChangesAreRejected | HTTP_API | ⬜ |
| T-NAC-ADM-CORE-006-08 | F-NAC-ADM-CORE-006 | PluginAdminApiOpenApiITCase | testNonConfigurablePluginRejectsConfigUpdate | HTTP_API | ⬜ |

## 1.4 Auth Admin API

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-ADM-AUTH-001-01 | F-NAC-ADM-AUTH-001 | VisibilityGrantAuthApiITCase | testGrantAndRevokeVisibilityGrant | HTTP_API | ⏭️ |
| T-NAC-ADM-AUTH-001-02 | F-NAC-ADM-AUTH-001 | VisibilityGrantAuthApiITCase | testGrantValidationAndNotFoundErrors | HTTP_API | ⏭️ |

## 1.5 AI Admin API

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-ADM-AI-001-01 | F-NAC-ADM-AI-001 | McpAdminApiOpenApiITCase | testCreateUpdateListGetAndDeleteMcpServer | HTTP_API | ⬜ |
| T-NAC-ADM-AI-001-02 | F-NAC-ADM-AI-001 | McpAdminApiOpenApiITCase | testCreateMcpServerValidationAndConflictErrors | HTTP_API | ⬜ |
| T-NAC-ADM-AI-001-03 | F-NAC-ADM-AI-001 | McpAdminApiOpenApiITCase | testGetListAndDeleteMcpServerValidationErrors | HTTP_API | ⬜ |
| T-NAC-ADM-AI-002-01 | F-NAC-ADM-AI-002 | A2aAdminApiOpenApiITCase | testRegisterLegacyAgentCardAndGetAgentCardSuccess | HTTP_API | ⬜ |
| T-NAC-ADM-AI-002-02 | F-NAC-ADM-AI-002 | A2aAdminApiOpenApiITCase | testRegisterV1AgentCardAndGetAgentCardSuccess | HTTP_API | ⬜ |
| T-NAC-ADM-AI-002-03 | F-NAC-ADM-AI-002 | A2aAdminApiOpenApiITCase | testUpdateAgentCardAndListApisSuccess | HTTP_API | ⬜ |
| T-NAC-ADM-AI-002-04 | F-NAC-ADM-AI-002 | A2aAdminApiOpenApiITCase | testGetListAndVersionListValidationAndNotFoundErrors | HTTP_API | ⬜ |
| T-NAC-ADM-AI-002-05 | F-NAC-ADM-AI-002 | A2aAdminApiOpenApiITCase | testRegisterInvalidAgentCardReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-ADM-AI-003-01 | F-NAC-ADM-AI-003 | AgentAdminApiOpenApiITCase | testDefaultNamespaceCrudOverviewListAndVersionReads | HTTP_API | ⬜ |
| T-NAC-ADM-AI-003-02 | F-NAC-ADM-AI-003 | AgentAdminApiOpenApiITCase | testListAppliesEveryFilterBeforePaging | HTTP_API | ⬜ |
| T-NAC-ADM-AI-003-03 | F-NAC-ADM-AI-003 | AgentAdminApiOpenApiITCase | testExplicitNamespaceIsolationAndValidationErrors | HTTP_API | ⬜ |
| T-NAC-ADM-AI-003-04 | F-NAC-ADM-AI-003 | AgentVersionAdminApiOpenApiITCase | testDraftLifecycleLabelsAndNoPipelineSubmit | HTTP_API | ⬜ |
| T-NAC-ADM-AI-003-05 | F-NAC-ADM-AI-003 | AgentVersionAdminApiOpenApiITCase | testDraftAndLifecycleValidationErrors | HTTP_API | ⬜ |
| T-NAC-ADM-AI-003-06 | F-NAC-ADM-AI-003 | AgentRuntimeEndpointAdminApiOpenApiITCase | testMissingDefinitionReturnsDefaultNamespaceEmptySnapshot | HTTP_API | ⬜ |
| T-NAC-ADM-AI-004-01 | F-NAC-ADM-AI-004 | AgentSpecAdminApiOpenApiITCase | testAgentSpecLifecycleGovernanceListAndDelete | HTTP_API | ⬜ |
| T-NAC-ADM-AI-004-02 | F-NAC-ADM-AI-004 | AgentSpecAdminApiOpenApiITCase | testAgentSpecForkSubmitDeleteDraftAndUpdateAutoCreate | HTTP_API | ⬜ |
| T-NAC-ADM-AI-004-03 | F-NAC-ADM-AI-004 | AgentSpecAdminApiOpenApiITCase | testAgentSpecValidationAndNotFoundErrors | HTTP_API | ⬜ |
| T-NAC-ADM-AI-004-04 | F-NAC-ADM-AI-004 | AgentSpecUploadAdminApiOpenApiITCase | testSingleAgentSpecUploadOverwriteAndVersionBump | HTTP_API | ⬜ |
| T-NAC-ADM-AI-004-05 | F-NAC-ADM-AI-004 | AgentSpecUploadAdminApiOpenApiITCase | testSeedArchiveUploadImportsMultipleAgentSpecs | HTTP_API | ⬜ |
| T-NAC-ADM-AI-004-06 | F-NAC-ADM-AI-004 | AgentSpecUploadAdminApiOpenApiITCase | testAgentSpecUploadValidationErrors | HTTP_API | ⬜ |
| T-NAC-ADM-AI-005-01 | F-NAC-ADM-AI-005 | SkillAdminApiOpenApiITCase | testSkillLifecycleGovernanceListDownloadAndDelete | HTTP_API | ⬜ |
| T-NAC-ADM-AI-005-02 | F-NAC-ADM-AI-005 | SkillAdminApiOpenApiITCase | testSkillForkSubmitAndDeleteDraft | HTTP_API | ⬜ |
| T-NAC-ADM-AI-005-03 | F-NAC-ADM-AI-005 | SkillAdminApiOpenApiITCase | testSkillValidationAndNotFoundErrors | HTTP_API | ⬜ |
| T-NAC-ADM-AI-005-04 | F-NAC-ADM-AI-005 | SkillUploadAdminApiOpenApiITCase | testSingleSkillUploadOverwriteAndVersionBump | HTTP_API | ⬜ |
| T-NAC-ADM-AI-005-05 | F-NAC-ADM-AI-005 | SkillUploadAdminApiOpenApiITCase | testUploadUsesFirstAvailableVersionSource | HTTP_API | ⬜ |
| T-NAC-ADM-AI-005-06 | F-NAC-ADM-AI-005 | SkillUploadAdminApiOpenApiITCase | testBatchSkillUploadSuccessAndPartialFailure | HTTP_API | ⬜ |
| T-NAC-ADM-AI-005-07 | F-NAC-ADM-AI-005 | SkillUploadAdminApiOpenApiITCase | testSkillUploadValidationErrors | HTTP_API | ⬜ |
| T-NAC-ADM-AI-006-01 | F-NAC-ADM-AI-006 | PromptAdminApiOpenApiITCase | testPromptLifecycleGovernanceListDownloadAndDelete | HTTP_API | ⬜ |
| T-NAC-ADM-AI-006-02 | F-NAC-ADM-AI-006 | PromptAdminApiOpenApiITCase | testPromptDeleteDraftRemovesEditingVersionOnly | HTTP_API | ⬜ |
| T-NAC-ADM-AI-006-03 | F-NAC-ADM-AI-006 | PromptAdminApiOpenApiITCase | testPromptSubmitAndLegacyCompatibilityEndpoints | HTTP_API | ⬜ |
| T-NAC-ADM-AI-006-04 | F-NAC-ADM-AI-006 | PromptAdminApiOpenApiITCase | testPromptValidationAndNotFoundErrors | HTTP_API | ⬜ |
| T-NAC-ADM-AI-007-01 | F-NAC-ADM-AI-007 | PipelineAdminApiOpenApiITCase | testListPipelinesCurrentAndLegacyReturnPageContract | HTTP_API | ⬜ |
| T-NAC-ADM-AI-007-02 | F-NAC-ADM-AI-007 | PipelineAdminApiOpenApiITCase | testListPipelinesValidationErrors | HTTP_API | ⬜ |
| T-NAC-ADM-AI-007-03 | F-NAC-ADM-AI-007 | PipelineAdminApiOpenApiITCase | testPipelineDetailValidationAndNotFoundErrors | HTTP_API | ⬜ |
| T-NAC-ADM-AI-008-01 | F-NAC-ADM-AI-008 | AiResourceImportAdminApiOpenApiITCase | testListSourcesFiltersAndSanitizesSourceInfo | HTTP_API | ⏭️ |
| T-NAC-ADM-AI-008-02 | F-NAC-ADM-AI-008 | AiResourceImportAdminApiOpenApiITCase | testSearchValidationAndSourceErrors | HTTP_API | ⏭️ |
| T-NAC-ADM-AI-008-03 | F-NAC-ADM-AI-008 | AiResourceImportAdminApiOpenApiITCase | testValidateSelectedItemsBoundariesAndSourceErrors | HTTP_API | ⏭️ |
| T-NAC-ADM-AI-008-04 | F-NAC-ADM-AI-008 | AiResourceImportAdminApiOpenApiITCase | testExecuteSelectedItemsBoundariesAndSourceErrors | HTTP_API | ⏭️ |

---

# Module 2: Console API Tests (HTTP)

> Source: `test/openapi-test/src/test/java/com/alibaba/nacos/test/consoleapi/`
> Server: `127.0.0.1:8080`, path prefix `/v3/console/`

## 2.1 Config Console API

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-CFG-014-01 | F-NAC-CFG-014 | ConfigConsoleApiOpenApiITCase | testPublishQueryRepublishAndDeleteConfig | HTTP_API | ⬜ |
| T-NAC-CFG-014-02 | F-NAC-CFG-014 | ConfigConsoleApiOpenApiITCase | testPublishConfigRequiredParametersReturnBadRequest | HTTP_API | ⬜ |
| T-NAC-CFG-014-03 | F-NAC-CFG-014 | ConfigConsoleApiOpenApiITCase | testQueryDeleteNotFoundAndInvalidNamespaceReturnControlledErrors | HTTP_API | ⬜ |
| T-NAC-CFG-016-01 | F-NAC-CFG-016 | ConfigBatchDeleteConsoleApiOpenApiITCase | testBatchDeleteExistingConfigsAndIgnoreMissingId | HTTP_API | ⬜ |
| T-NAC-CFG-016-02 | F-NAC-CFG-016 | ConfigBatchDeleteConsoleApiOpenApiITCase | testBatchDeleteSkipsIdsOutsideNamespace | HTTP_API | ⬜ |
| T-NAC-CFG-016-03 | F-NAC-CFG-016 | ConfigBatchDeleteConsoleApiOpenApiITCase | testBatchDeleteIdsValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-CFG-020-01 | F-NAC-CFG-020 | ConfigBetaConsoleApiOpenApiITCase | testQueryAndStopBetaConfig | HTTP_API | ⬜ |
| T-NAC-CFG-020-02 | F-NAC-CFG-020 | ConfigBetaConsoleApiOpenApiITCase | testBetaRequiredParametersAndAbsentConfigReturnControlledErrors | HTTP_API | ⬜ |
| T-NAC-CFG-027-01 | F-NAC-CFG-027 | ConfigCloneConsoleApiOpenApiITCase | testCloneConfigToTargetIdentity | HTTP_API | ⬜ |
| T-NAC-CFG-027-02 | F-NAC-CFG-027 | ConfigCloneConsoleApiOpenApiITCase | testCloneConfigWithSourceNamespace | HTTP_API | ⬜ |
| T-NAC-CFG-027-03 | F-NAC-CFG-027 | ConfigCloneConsoleApiOpenApiITCase | testCloneConfigSkipsIdsOutsideSourceNamespace | HTTP_API | ⬜ |
| T-NAC-CFG-027-04 | F-NAC-CFG-027 | ConfigCloneConsoleApiOpenApiITCase | testCloneValidationAndBusinessFailuresReturnResultEnvelope | HTTP_API | ⬜ |
| T-NAC-CFG-026-01 | F-NAC-CFG-026 | ConfigExportConsoleApiOpenApiITCase | testExportConfigByIdReturnsZipWithContentAndMetadata | HTTP_API | ⬜ |
| T-NAC-CFG-026-02 | F-NAC-CFG-026 | ConfigExportConsoleApiOpenApiITCase | testExportInvalidNamespaceReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-CFG-026-03 | F-NAC-CFG-026 | ConfigExportConsoleApiOpenApiITCase | testExportByIdSkipsConfigsOutsideNamespace | HTTP_API | ⬜ |
| T-NAC-CFG-017-01 | F-NAC-CFG-017 | ConfigHistoryConsoleApiOpenApiITCase | testHistoryListDetailPreviousAndNamespaceConfigs | HTTP_API | ⬜ |
| T-NAC-CFG-017-02 | F-NAC-CFG-017 | ConfigHistoryConsoleApiOpenApiITCase | testHistoryValidationAndAbsentHistoryReturnControlledErrors | HTTP_API | ⬜ |
| T-NAC-CFG-025-01 | F-NAC-CFG-025 | ConfigImportConsoleApiOpenApiITCase | testImportConfigZipPublishesConfig | HTTP_API | ⬜ |
| T-NAC-CFG-025-02 | F-NAC-CFG-025 | ConfigImportConsoleApiOpenApiITCase | testImportMissingFileAndBadMetadataReturnFailureResult | HTTP_API | ⬜ |
| T-NAC-CFG-011-01 | F-NAC-CFG-011 | ConfigListConsoleApiOpenApiITCase | testListConfigsByFuzzyDataIdAdvancedFiltersAndContentSearch | HTTP_API | ⬜ |
| T-NAC-CFG-011-02 | F-NAC-CFG-011 | ConfigListConsoleApiOpenApiITCase | testListAllowsBlankDataIdAndNoMatchReturnsEmptyPage | HTTP_API | ⬜ |
| T-NAC-CFG-011-03 | F-NAC-CFG-011 | ConfigListConsoleApiOpenApiITCase | testListAndSearchInvalidPaginationReturnBadRequest | HTTP_API | ⬜ |
| T-NAC-CFG-023-01 | F-NAC-CFG-023 | ConfigListenerConsoleApiOpenApiITCase | testConfigListenerQueryReturnsConfigQueryType | HTTP_API | ⬜ |
| T-NAC-CFG-023-02 | F-NAC-CFG-023 | ConfigListenerConsoleApiOpenApiITCase | testIpListenerQueryAcceptsAllAndNamespaceFilter | HTTP_API | ⬜ |
| T-NAC-CFG-023-03 | F-NAC-CFG-023 | ConfigListenerConsoleApiOpenApiITCase | testListenerRequiredParametersReturnBadRequest | HTTP_API | ⬜ |

## 2.2 Core Console API

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-CORE-001-01 | F-NAC-CORE-001,003 | NamespaceConsoleApiOpenApiITCase | testCreateDetailUpdateListExistAndDeleteNamespace | HTTP_API | ⬜ |
| T-NAC-CORE-001-02 | F-NAC-CORE-001,003 | NamespaceConsoleApiOpenApiITCase | testNamespaceValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-CORE-004-01 | F-NAC-CORE-004 | ClusterConsoleApiOpenApiITCase | testNodeListAndKeywordFiltering | HTTP_API | ⬜ |
| T-NAC-CORE-014-01 | F-NAC-CORE-014,015 | HealthConsoleApiOpenApiITCase | testLivenessAndReadinessReturnHealthyResult | HTTP_API | ⬜ |
| T-NAC-CORE-006-01 | F-NAC-CORE-006,007,008 | ServerStateConsoleApiOpenApiITCase | testServerStateGuideAndAnnouncement | HTTP_API | ⬜ |
| T-NAC-CORE-006-02 | F-NAC-CORE-007 | ServerStateConsoleApiOpenApiITCase | testUnsupportedAnnouncementLanguageReturnsFailureResult | HTTP_API | ⬜ |
| T-NAC-CORE-009-01 | F-NAC-CORE-009,010,012,013 | PluginConsoleApiOpenApiITCase | testListFilterDetailAndAvailability | HTTP_API | ⬜ |
| T-NAC-CORE-009-02 | F-NAC-CORE-010 | PluginConsoleApiOpenApiITCase | testNacosAuthPluginConfigMetadata | HTTP_API | ⬜ |
| T-NAC-CORE-009-03 | F-NAC-CORE-010 | PluginConsoleApiOpenApiITCase | testLdapAuthPluginConfigMetadata | HTTP_API | ⬜ |
| T-NAC-CORE-009-04 | F-NAC-CORE-010 | PluginConsoleApiOpenApiITCase | testOidcAuthPluginConfigMetadata | HTTP_API | ⬜ |
| T-NAC-CORE-009-05 | F-NAC-CORE-013 | PluginConsoleApiOpenApiITCase | testLocalOnlyConfigCanBeClearedWithEmptyMap | HTTP_API | ⬜ |
| T-NAC-CORE-009-06 | F-NAC-CORE-009,010 | PluginConsoleApiOpenApiITCase | testPluginValidationAndNotFoundReturnControlledErrors | HTTP_API | ⬜ |
| T-NAC-CORE-009-07 | F-NAC-CORE-013 | PluginConsoleApiOpenApiITCase | testNonConfigurablePluginRejectsConfigUpdate | HTTP_API | ⬜ |
| T-NAC-CORE-009-08 | F-NAC-CORE-011 | PluginConsoleApiOpenApiITCase | testCriticalAndExclusiveStateChangesAreRejected | HTTP_API | ⬜ |

## 2.3 Naming Console API

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-NS-012-01 | F-NAC-NS-012,013,014 | ServiceConsoleApiOpenApiITCase | testCreateDetailUpdateListSubscribersSelectorAndDeleteService | HTTP_API | ⬜ |
| T-NAC-NS-012-02 | F-NAC-NS-012 | ServiceConsoleApiOpenApiITCase | testServiceValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-NS-017-01 | F-NAC-NS-017,018,019 | InstanceConsoleApiOpenApiITCase | testListUpdateAndDeletePersistentInstance | HTTP_API | ⬜ |
| T-NAC-NS-017-02 | F-NAC-NS-017 | InstanceConsoleApiOpenApiITCase | testInstanceValidationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-NS-016-01 | F-NAC-NS-016 | ServiceClusterConsoleApiOpenApiITCase | testUpdateClusterMetadataAndDefaultGroup | HTTP_API | ⬜ |
| T-NAC-NS-016-02 | F-NAC-NS-016 | ServiceClusterConsoleApiOpenApiITCase | testClusterValidationAndMissingServiceReturnControlledErrors | HTTP_API | ⬜ |

## 2.4 AI Console API

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-AI-005-01 | F-NAC-AI-005 | McpConsoleApiOpenApiITCase | testCreateUpdateListGetAndDeleteMcpServer | HTTP_API | ⬜ |
| T-NAC-AI-005-02 | F-NAC-AI-005 | McpConsoleApiOpenApiITCase | testCreateMcpServerValidationAndConflictErrors | HTTP_API | ⬜ |
| T-NAC-AI-005-03 | F-NAC-AI-005 | McpConsoleApiOpenApiITCase | testGetListAndDeleteMcpServerValidationErrors | HTTP_API | ⬜ |
| T-NAC-AI-005-04 | F-NAC-AI-005,006 | McpConsoleApiOpenApiITCase | testImportToolsAndImportRequestValidationErrors | HTTP_API | ⬜ |
| T-NAC-AI-007-01 | F-NAC-AI-007 | A2aConsoleApiOpenApiITCase | testRegisterLegacyAgentCardAndGetAgentCardSuccess | HTTP_API | ⬜ |
| T-NAC-AI-007-02 | F-NAC-AI-007 | A2aConsoleApiOpenApiITCase | testRegisterV1AgentCardAndGetAgentCardSuccess | HTTP_API | ⬜ |
| T-NAC-AI-007-03 | F-NAC-AI-007 | A2aConsoleApiOpenApiITCase | testUpdateAgentCardAndListApisSuccess | HTTP_API | ⬜ |
| T-NAC-AI-007-04 | F-NAC-AI-007 | A2aConsoleApiOpenApiITCase | testGetListAndVersionListValidationAndNotFoundErrors | HTTP_API | ⬜ |
| T-NAC-AI-007-05 | F-NAC-AI-007 | A2aConsoleApiOpenApiITCase | testRegisterInvalidAgentCardReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-AI-001-01 | F-NAC-AI-001 | AgentConsoleApiOpenApiITCase | testAllConsoleManagementPathsAndRuntimeNamingReference | HTTP_API | ⬜ |
| T-NAC-AI-001-02 | F-NAC-AI-001 | AgentConsoleApiOpenApiITCase | testConsoleBindingValidationAndExplicitRuntimeNamespace | HTTP_API | ⬜ |
| T-NAC-AI-004-01 | F-NAC-AI-004 | PromptConsoleApiOpenApiITCase | testPromptLifecycleGovernanceListDownloadAndDelete | HTTP_API | ⬜ |
| T-NAC-AI-004-02 | F-NAC-AI-004 | PromptConsoleApiOpenApiITCase | testPromptDeleteDraftRemovesEditingVersionOnly | HTTP_API | ⬜ |
| T-NAC-AI-004-03 | F-NAC-AI-004 | PromptConsoleApiOpenApiITCase | testPromptSubmitDraftSuccess | HTTP_API | ⬜ |
| T-NAC-AI-004-04 | F-NAC-AI-004 | PromptConsoleApiOpenApiITCase | testPromptValidationAndNotFoundErrors | HTTP_API | ⬜ |
| T-NAC-AI-003-01 | F-NAC-AI-003 | SkillConsoleApiOpenApiITCase | testSkillLifecycleGovernanceListDownloadAndDelete | HTTP_API | ⬜ |
| T-NAC-AI-003-02 | F-NAC-AI-003 | SkillConsoleApiOpenApiITCase | testSkillForkSubmitAndDeleteDraft | HTTP_API | ⬜ |
| T-NAC-AI-003-03 | F-NAC-AI-003 | SkillConsoleApiOpenApiITCase | testSkillValidationAndNotFoundErrors | HTTP_API | ⬜ |
| T-NAC-AI-003-04 | F-NAC-AI-003,011 | SkillUploadConsoleApiOpenApiITCase | testSingleSkillUploadOverwriteAndVersionBump | HTTP_API | ⬜ |
| T-NAC-AI-003-05 | F-NAC-AI-003,011 | SkillUploadConsoleApiOpenApiITCase | testUploadUsesFirstAvailableVersionSource | HTTP_API | ⬜ |
| T-NAC-AI-003-06 | F-NAC-AI-003,011 | SkillUploadConsoleApiOpenApiITCase | testBatchSkillUploadSuccessAndPartialFailure | HTTP_API | ⬜ |
| T-NAC-AI-003-07 | F-NAC-AI-003,011 | SkillUploadConsoleApiOpenApiITCase | testSkillUploadValidationErrors | HTTP_API | ⬜ |
| T-NAC-AI-002-01 | F-NAC-AI-002,010 | AgentSpecConsoleApiOpenApiITCase | testAgentSpecLifecycleGovernanceListAndDelete | HTTP_API | ⬜ |
| T-NAC-AI-002-02 | F-NAC-AI-002,010 | AgentSpecConsoleApiOpenApiITCase | testAgentSpecForkSubmitDeleteDraftAndUpdateAutoCreate | HTTP_API | ⬜ |
| T-NAC-AI-002-03 | F-NAC-AI-002,010 | AgentSpecConsoleApiOpenApiITCase | testAgentSpecValidationAndNotFoundErrors | HTTP_API | ⬜ |
| T-NAC-AI-002-04 | F-NAC-AI-002,010 | AgentSpecUploadConsoleApiOpenApiITCase | testSingleAgentSpecUploadOverwriteAndVersionBump | HTTP_API | ⬜ |
| T-NAC-AI-002-05 | F-NAC-AI-002,010 | AgentSpecUploadConsoleApiOpenApiITCase | testSeedArchiveUploadImportsMultipleAgentSpecs | HTTP_API | ⬜ |
| T-NAC-AI-002-06 | F-NAC-AI-002,010 | AgentSpecUploadConsoleApiOpenApiITCase | testAgentSpecUploadValidationErrors | HTTP_API | ⬜ |
| T-NAC-AI-008-01 | F-NAC-AI-008 | PipelineConsoleApiOpenApiITCase | testListPipelinesCurrentAndLegacyReturnPageContract | HTTP_API | ⬜ |
| T-NAC-AI-008-02 | F-NAC-AI-008 | PipelineConsoleApiOpenApiITCase | testListPipelinesValidationErrors | HTTP_API | ⬜ |
| T-NAC-AI-008-03 | F-NAC-AI-008 | PipelineConsoleApiOpenApiITCase | testPipelineDetailValidationAndNotFoundErrors | HTTP_API | ⬜ |
| T-NAC-AI-006-01 | F-NAC-AI-006 | AiResourceImportConsoleApiOpenApiITCase | testListSourcesFiltersAndSanitizesSourceInfo | HTTP_API | ⬜ |
| T-NAC-AI-006-02 | F-NAC-AI-006 | AiResourceImportConsoleApiOpenApiITCase | testSearchValidationAndSourceErrors | HTTP_API | ⬜ |
| T-NAC-AI-006-03 | F-NAC-AI-006 | AiResourceImportConsoleApiOpenApiITCase | testValidateSelectedItemsBoundariesAndSourceErrors | HTTP_API | ⬜ |
| T-NAC-AI-006-04 | F-NAC-AI-006 | AiResourceImportConsoleApiOpenApiITCase | testExecuteSelectedItemsBoundariesAndSourceErrors | HTTP_API | ⬜ |
| T-NAC-AI-009-01 | F-NAC-AI-009 | CopilotConsoleApiOpenApiITCase | testCopilotConfigSaveAndGetSuccess | HTTP_API | ⬜ |
| T-NAC-AI-009-02 | F-NAC-AI-009 | CopilotConsoleApiOpenApiITCase | testCopilotConfigMalformedJsonReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-AI-009-03 | F-NAC-AI-009 | CopilotConsoleApiOpenApiITCase | testCopilotSseValidationErrorEvents | HTTP_API | ⬜ |

---

# Module 3: Client API Tests (HTTP)

> Source: `test/openapi-test/src/test/java/com/alibaba/nacos/test/openapi/client/`
> Server: `127.0.0.1:8848`, path prefix `/nacos/v3/client/`

## 3.1 Config Client API

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-CLIENT-001-01 | F-NAC-CLIENT-001 | ConfigOpenApiITCase | testGetConfigWhenNotExists | HTTP_API | ⬜ |
| T-NAC-CLIENT-001-02 | F-NAC-CLIENT-001 | ConfigOpenApiITCase | testGetConfigSuccessAfterPublish | HTTP_API | ⬜ |
| T-NAC-CLIENT-001-03 | F-NAC-CLIENT-001 | ConfigOpenApiITCase | testGetConfigOmitNamespaceUsesPublic | HTTP_API | ⬜ |
| T-NAC-CLIENT-001-04 | F-NAC-CLIENT-001 | ConfigOpenApiITCase | testGetConfigWrongNamespaceNotFound | HTTP_API | ⬜ |
| T-NAC-CLIENT-001-05 | F-NAC-CLIENT-001 | ConfigOpenApiITCase | testGetConfigMissingDataIdReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-CLIENT-001-06 | F-NAC-CLIENT-001 | ConfigOpenApiITCase | testGetConfigMissingGroupNameReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-CLIENT-001-07 | F-NAC-CLIENT-001 | ConfigOpenApiITCase | testGetConfigLegacyGroupParameterDoesNotReplaceGroupName | HTTP_API | ⬜ |
| T-NAC-CLIENT-001-08 | F-NAC-CLIENT-001 | ConfigOpenApiITCase | testGetConfigInvalidNamespaceReturnsBadRequest | HTTP_API | ⬜ |

## 3.2 Naming Client API

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-CLIENT-002-01 | F-NAC-CLIENT-002 | InstanceRegisterOpenApiITCase | testRegisterWithDefaultValuesMakesInstanceDiscoverable | HTTP_API | ⬜ |
| T-NAC-CLIENT-002-02 | F-NAC-CLIENT-002 | InstanceRegisterOpenApiITCase | testRegisterWithExplicitFieldsPersistsClientVisibleState | HTTP_API | ⬜ |
| T-NAC-CLIENT-002-03 | F-NAC-CLIENT-002 | InstanceRegisterOpenApiITCase | testRegisterPersistentInstanceWhenEphemeralFalse | HTTP_API | ⬜ |
| T-NAC-CLIENT-005-01 | F-NAC-CLIENT-005 | InstanceRegisterOpenApiITCase | testHeartbeatExistingInstanceReturnsSuccess | HTTP_API | ⬜ |
| T-NAC-CLIENT-005-02 | F-NAC-CLIENT-005 | InstanceRegisterOpenApiITCase | testHeartbeatAbsentInstanceReturnsInstanceNotFoundResult | HTTP_API | ⬜ |
| T-NAC-CLIENT-002-04 | F-NAC-CLIENT-002 | InstanceRegisterOpenApiITCase | testRegisterMissingServiceNameReturnsBadRequestResultBody | HTTP_API | ⬜ |
| T-NAC-CLIENT-002-05 | F-NAC-CLIENT-002 | InstanceRegisterOpenApiITCase | testRegisterMissingIpReturnsBadRequestResultBody | HTTP_API | ⬜ |
| T-NAC-CLIENT-002-06 | F-NAC-CLIENT-002 | InstanceRegisterOpenApiITCase | testRegisterMissingPortReturnsBadRequestResultBody | HTTP_API | ⬜ |
| T-NAC-CLIENT-002-07 | F-NAC-CLIENT-002 | InstanceRegisterOpenApiITCase | testRegisterInvalidWeightReturnsBadRequestResultBody | HTTP_API | ⬜ |
| T-NAC-CLIENT-004-01 | F-NAC-CLIENT-004 | InstanceListOpenApiITCase | testListDefaultNamespaceAndGroupFiltersDisabledInstances | HTTP_API | ⬜ |
| T-NAC-CLIENT-004-02 | F-NAC-CLIENT-004 | InstanceListOpenApiITCase | testListWithoutClusterNameReturnsAllEnabledInstances | HTTP_API | ⬜ |
| T-NAC-CLIENT-004-03 | F-NAC-CLIENT-004 | InstanceListOpenApiITCase | testListWithExplicitGroupNameIsolatesDefaultGroup | HTTP_API | ⬜ |
| T-NAC-CLIENT-004-04 | F-NAC-CLIENT-004 | InstanceListOpenApiITCase | testListHealthyOnlyParameterDoesNotFilterOpenApiResult | HTTP_API | ⬜ |
| T-NAC-CLIENT-004-05 | F-NAC-CLIENT-004 | InstanceListOpenApiITCase | testListUnknownServiceReturnsEmptySuccessResult | HTTP_API | ⬜ |
| T-NAC-CLIENT-004-06 | F-NAC-CLIENT-004 | InstanceListOpenApiITCase | testListMissingServiceNameReturnsBadRequestResultBody | HTTP_API | ⬜ |
| T-NAC-CLIENT-003-01 | F-NAC-CLIENT-003 | InstanceDeregisterOpenApiITCase | testDeregisterDefaultIdentityRemovesInstance | HTTP_API | ⬜ |
| T-NAC-CLIENT-003-02 | F-NAC-CLIENT-003 | InstanceDeregisterOpenApiITCase | testDeregisterExplicitGroupAndClusterDoesNotRemoveOtherIdentity | HTTP_API | ⬜ |
| T-NAC-CLIENT-003-03 | F-NAC-CLIENT-003 | InstanceDeregisterOpenApiITCase | testDeregisterAbsentInstanceReturnsSuccess | HTTP_API | ⬜ |
| T-NAC-CLIENT-003-04 | F-NAC-CLIENT-003 | InstanceDeregisterOpenApiITCase | testDeregisterMissingServiceNameReturnsBadRequestResultBody | HTTP_API | ⬜ |
| T-NAC-CLIENT-003-05 | F-NAC-CLIENT-003 | InstanceDeregisterOpenApiITCase | testDeregisterMissingIpReturnsBadRequestResultBody | HTTP_API | ⬜ |
| T-NAC-CLIENT-003-06 | F-NAC-CLIENT-003 | InstanceDeregisterOpenApiITCase | testDeregisterMissingPortReturnsBadRequestResultBody | HTTP_API | ⬜ |

## 3.3 AI Client API

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-CLIENT-006-01 | F-NAC-CLIENT-006 | PromptClientOpenApiITCase | testQueryPromptByLatestVersionLabelAndMd5 | HTTP_API | ⬜ |
| T-NAC-CLIENT-006-02 | F-NAC-CLIENT-006 | PromptClientOpenApiITCase | testQueryPromptMissingPromptKeyReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-CLIENT-006-03 | F-NAC-CLIENT-006 | PromptClientOpenApiITCase | testQueryPromptUnknownNamespaceReturnsNotFoundResultBody | HTTP_API | ⬜ |
| T-NAC-CLIENT-006-04 | F-NAC-CLIENT-006 | PromptClientOpenApiITCase | testQueryPromptUnknownResourceReturnsNotFoundResultBody | HTTP_API | ⬜ |
| T-NAC-CLIENT-006-05 | F-NAC-CLIENT-006 | PromptClientOpenApiITCase | testQueryPromptUnknownVersionReturnsNotFoundAndUnknownLabelFallsBackLatest | HTTP_API | ⬜ |
| T-NAC-CLIENT-007-01 | F-NAC-CLIENT-007 | SkillClientOpenApiITCase | testDownloadSkillByLatestVersionAndLabel | HTTP_API | ⬜ |
| T-NAC-CLIENT-007-02 | F-NAC-CLIENT-007 | SkillClientOpenApiITCase | testDownloadSkillMissingNameReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-CLIENT-007-03 | F-NAC-CLIENT-007 | SkillClientOpenApiITCase | testDownloadSkillInvalidNameReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-CLIENT-007-04 | F-NAC-CLIENT-007 | SkillClientOpenApiITCase | testDownloadSkillUnknownResourceReturnsNotFoundResultBody | HTTP_API | ⬜ |
| T-NAC-CLIENT-007-05 | F-NAC-CLIENT-007 | SkillClientOpenApiITCase | testDownloadSkillUnknownVersionAndLabelReturnNotFoundResultBody | HTTP_API | ⬜ |
| T-NAC-CLIENT-008-01 | F-NAC-CLIENT-008 | AgentSpecClientOpenApiITCase | testGetAgentSpecByLatestVersionAndLabel | HTTP_API | ⬜ |
| T-NAC-CLIENT-008-02 | F-NAC-CLIENT-008 | AgentSpecClientOpenApiITCase | testGetAgentSpecMissingNameReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-CLIENT-008-03 | F-NAC-CLIENT-008 | AgentSpecClientOpenApiITCase | testGetAgentSpecUnknownResourceReturnsNotFoundResultBody | HTTP_API | ⬜ |
| T-NAC-CLIENT-008-04 | F-NAC-CLIENT-008 | AgentSpecClientOpenApiITCase | testGetAgentSpecUnknownVersionAndLabelFallback | HTTP_API | ⬜ |
| T-NAC-CLIENT-009-01 | F-NAC-CLIENT-009 | AgentSpecSearchClientOpenApiITCase | testSearchAgentSpecsByKeywordAndPagination | HTTP_API | ⬜ |
| T-NAC-CLIENT-009-02 | F-NAC-CLIENT-009 | AgentSpecSearchClientOpenApiITCase | testSearchAgentSpecsEmptyKeywordUsesPublicNamespace | HTTP_API | ⬜ |
| T-NAC-CLIENT-009-03 | F-NAC-CLIENT-009 | AgentSpecSearchClientOpenApiITCase | testSearchAgentSpecsNoMatchReturnsEmptyPage | HTTP_API | ⬜ |
| T-NAC-CLIENT-009-04 | F-NAC-CLIENT-009 | AgentSpecSearchClientOpenApiITCase | testSearchAgentSpecsInvalidPaginationReturnsBadRequest | HTTP_API | ⬜ |
| T-NAC-CLIENT-010-01 | F-NAC-CLIENT-010 | AgentDiscoveryClientOpenApiITCase | testSearchAndDiscoverOnlineAgent | HTTP_API | ⏭️ |
| T-NAC-CLIENT-010-02 | F-NAC-CLIENT-010 | AgentDiscoveryClientOpenApiITCase | testSearchAndDiscoverValidationAndNotFound | HTTP_API | ⏭️ |
| T-NAC-CLIENT-011-01 | F-NAC-CLIENT-011 | AgentEndpointClientOpenApiITCase | testCompletePublisherLifecycleAndQueryIsolation | HTTP_API | ⏭️ |
| T-NAC-CLIENT-011-02 | F-NAC-CLIENT-011 | AgentEndpointClientOpenApiITCase | testEndpointHeadersBodyAndClientIdValidation | HTTP_API | ⏭️ |

---

# Module 4: Java SDK Tests (gRPC)

> Source: `test/java-sdk-test/src/test/java/com/alibaba/nacos/test/sdk/`
> Server: `127.0.0.1:8848` via gRPC

## 4.1 Config SDK

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-CFG-002-01 | F-NAC-CFG-001,002,003 | ConfigServiceJavaSdkITCase | testPublishQueryCasAndRemoveConfig | GRPC_API | ⬜ |
| T-NAC-CFG-002-02 | F-NAC-CFG-001 | ConfigServiceJavaSdkITCase | testMissingConfigResultAndRemoveAreEmptyAndIdempotent | GRPC_API | ⬜ |
| T-NAC-CFG-002-03 | F-NAC-CFG-002 | ConfigServiceJavaSdkITCase | testCasBoundaryForMissingAndEmptyMd5 | GRPC_API | ⬜ |
| T-NAC-CFG-004-01 | F-NAC-CFG-004,006 | ConfigServiceJavaSdkITCase | testGetConfigAndSignListenerReceivesUpdates | GRPC_API | ⬜ |
| T-NAC-CFG-004-02 | F-NAC-CFG-004,006 | ConfigServiceJavaSdkITCase | testAddListenerReceivesPublishedUpdate | GRPC_API | ⬜ |
| T-NAC-CFG-004-03 | F-NAC-CFG-004 | ConfigServiceJavaSdkITCase | testRemoveListenerStopsLaterCallbacks | GRPC_API | ⬜ |
| T-NAC-CFG-004-04 | F-NAC-CFG-004 | ConfigServiceJavaSdkITCase | testNullConfigListenerIsRejected | GRPC_API | ⬜ |
| T-NAC-CFG-002-04 | F-NAC-CFG-002 | ConfigServiceJavaSdkITCase | testConfigValidationAndDefaultGroupBoundary | GRPC_API | ⬜ |
| T-NAC-CFG-002-05 | F-NAC-CFG-001 | ConfigServiceJavaSdkITCase | testValidJsonTypeIsPreservedInQueryResult | GRPC_API | ⬜ |
| T-NAC-CFG-002-06 | F-NAC-CFG-002 | ConfigServiceJavaSdkITCase | testConfigFilterTransformsPublishAndQueryContent | GRPC_API | ⬜ |
| T-NAC-CFG-008-01 | F-NAC-CFG-008,009,010 | ConfigServiceJavaSdkITCase | testFuzzyWatchReturnsKeysAndStopsAfterCancel | GRPC_API | ⬜ |

## 4.2 Naming SDK

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-NS-001-01 | F-NAC-NS-001,002,005 | NamingServiceJavaSdkITCase | testRegisterQuerySelectListAndDeregisterInstance | GRPC_API | ⬜ |
| T-NAC-NS-001-02 | F-NAC-NS-001,002 | NamingServiceJavaSdkITCase | testDefaultGroupStringOverloadsRegisterAndDeregisterInstance | GRPC_API | ⬜ |
| T-NAC-NS-001-03 | F-NAC-NS-001,002 | NamingServiceJavaSdkITCase | testClusterStringOverloadsRegisterAndDeregisterInstance | GRPC_API | ⬜ |
| T-NAC-NS-004-01 | F-NAC-NS-004 | NamingServiceJavaSdkITCase | testSinglePersistentInstanceRegisterAndDeregister | GRPC_API | ⬜ |
| T-NAC-NS-001-04 | F-NAC-NS-001,002 | NamingServiceJavaSdkITCase | testDuplicateRegisterAndMissingDeregisterAreIdempotent | GRPC_API | ⬜ |
| T-NAC-NS-003-01 | F-NAC-NS-003 | NamingServiceJavaSdkITCase | testBatchRegisterAndPartialBatchDeregisterInstance | GRPC_API | ⬜ |
| T-NAC-NS-003-02 | F-NAC-NS-003 | NamingServiceJavaSdkITCase | testEmptyBatchRegisterLeavesNoInstances | GRPC_API | ⬜ |
| T-NAC-NS-005-01 | F-NAC-NS-005 | NamingServiceJavaSdkITCase | testHealthySelectionFiltersDisabledAndZeroWeightInstances | GRPC_API | ⬜ |
| T-NAC-NS-005-02 | F-NAC-NS-005 | NamingServiceJavaSdkITCase | testUnhealthySelectionReturnsExplicitUnhealthyInstances | GRPC_API | ⬜ |
| T-NAC-NS-007-01 | F-NAC-NS-007 | NamingServiceJavaSdkITCase | testServiceListPaginationBoundaries | GRPC_API | ⬜ |
| T-NAC-NS-007-02 | F-NAC-NS-007 | NamingServiceJavaSdkITCase | testServiceListSelectorOverloadsReturnStableShape | GRPC_API | ⬜ |
| T-NAC-NS-006-01 | F-NAC-NS-006,008 | NamingServiceJavaSdkITCase | testSubscribeReceivesInstanceChangeEvent | GRPC_API | ⬜ |
| T-NAC-NS-006-02 | F-NAC-NS-005,006 | NamingServiceJavaSdkITCase | testGetAllInstancesSubscribeTrueUsesPushedCache | GRPC_API | ⬜ |
| T-NAC-NS-006-03 | F-NAC-NS-006,008 | NamingServiceJavaSdkITCase | testClusterSubscribeFiltersInstanceChangeEvents | GRPC_API | ⬜ |
| T-NAC-NS-006-04 | F-NAC-NS-006,008 | NamingServiceJavaSdkITCase | testSelectorSubscribeFiltersInstanceChangeEvents | GRPC_API | ⬜ |
| T-NAC-NS-009-01 | F-NAC-NS-009,010,011 | NamingServiceJavaSdkITCase | testFuzzyWatchReturnsServiceKeysAndStopsAfterCancel | GRPC_API | ⬜ |
| T-NAC-NS-006-05 | F-NAC-NS-006 | NamingServiceJavaSdkITCase | testUnsubscribeStopsLaterInstanceChangeEvents | GRPC_API | ⬜ |
| T-NAC-NS-001-05 | F-NAC-NS-001 | NamingServiceJavaSdkITCase | testInvalidNamingParametersThrowNacosException | GRPC_API | ⬜ |

## 4.3 Lock SDK

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-LOCK-001-01 | F-NAC-LOCK-001,002 | LockServiceJavaSdkITCase | testAcquireCompeteReleaseAndReacquireLock | GRPC_API | ⬜ |
| T-NAC-LOCK-001-02 | F-NAC-LOCK-001 | LockServiceJavaSdkITCase | testDirectRemoteTryLockAndReleaseLock | GRPC_API | ⬜ |
| T-NAC-LOCK-003-01 | F-NAC-LOCK-003 | LockServiceJavaSdkITCase | testExpiredLockCanBeAcquiredByAnotherClient | GRPC_API | ⬜ |
| T-NAC-LOCK-001-03 | F-NAC-LOCK-001 | LockServiceJavaSdkITCase | testInvalidLockInputThrowsControlledException | GRPC_API | ⬜ |

## 4.4 AI SDK

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-AISDK-001-01 | F-NAC-AISDK-001,002,003 | AiServiceJavaSdkITCase | testReleaseQueryAndSubscribeMcpServer | GRPC_API | ⬜ |
| T-NAC-AISDK-001-02 | F-NAC-AISDK-001,002,003 | AiServiceJavaSdkITCase | testMcpServerLatestDuplicateAndEndpointScenarios | GRPC_API | ⬜ |
| T-NAC-AISDK-001-03 | F-NAC-AISDK-001,003 | AiServiceJavaSdkITCase | testReleaseMcpServerWithDirectEndpointSpecification | GRPC_API | ⬜ |
| T-NAC-AISDK-001-04 | F-NAC-AISDK-003 | AiServiceJavaSdkITCase | testRegisterMcpServerEndpointForRemoteRefServer | GRPC_API | ⬜ |
| T-NAC-AISDK-001-05 | F-NAC-AISDK-002 | AiServiceJavaSdkITCase | testMissingMcpSubscriptionReturnsNullableShape | GRPC_API | ⬜ |
| T-NAC-AISDK-004-01 | F-NAC-AISDK-004,005,006 | AiServiceJavaSdkITCase | testReleaseQueryAndSubscribeAgentCard | GRPC_API | ⬜ |
| T-NAC-AISDK-004-02 | F-NAC-AISDK-004 | AiServiceJavaSdkITCase | testAgentCardDuplicateReleaseKeepsExistingVersion | GRPC_API | ⬜ |
| T-NAC-AISDK-004-03 | F-NAC-AISDK-004,005,006 | AiServiceJavaSdkITCase | testAgentCardLatestVersionAndEndpointScenarios | GRPC_API | ⬜ |
| T-NAC-AISDK-004-04 | F-NAC-AISDK-006 | AiServiceJavaSdkITCase | testAgentCardBatchEndpointOverwritesSingleEndpoint | GRPC_API | ⬜ |
| T-NAC-AISDK-004-05 | F-NAC-AISDK-006,005 | AiServiceJavaSdkITCase | testAgentEndpointTlsQueryAndMissingCardBoundaries | GRPC_API | ⬜ |
| T-NAC-AISDK-004-06 | F-NAC-AISDK-005 | AiServiceJavaSdkITCase | testAgentCardUnsubscribeStopsLaterCallbacks | GRPC_API | ⬜ |
| T-NAC-AISDK-007-01 | F-NAC-AISDK-007 | AiServiceJavaSdkITCase | testMissingAiSubscriptionResourcesReturnNullableShapes | GRPC_API | ⬜ |
| T-NAC-AISDK-007-02 | F-NAC-AISDK-007 | AiServiceJavaSdkITCase | testInvalidAiParametersThrowNacosException | GRPC_API | ⬜ |
| T-NAC-AISDK-008-01 | F-NAC-AISDK-008 | AgentDiscoveryServiceJavaSdkITCase | shouldSearchDiscoverAndIsolateNamespaces | GRPC_API | ⏭️ |
| T-NAC-AISDK-008-02 | F-NAC-AISDK-008 | AgentDiscoveryServiceJavaSdkITCase | shouldReplaceAndPartiallyDeregisterCompletePublications | GRPC_API | ⏭️ |
| T-NAC-AISDK-008-03 | F-NAC-AISDK-008 | AgentDiscoveryServiceJavaSdkITCase | shouldAggregateIndependentSdkPublishers | GRPC_API | ⏭️ |
| T-NAC-AISDK-008-04 | F-NAC-AISDK-008 | AgentDiscoveryServiceJavaSdkITCase | shouldDiscoverPreRegistrationAndPollUntilAgentAppears | GRPC_API | ⏭️ |
| T-NAC-AISDK-008-05 | F-NAC-AISDK-008 | AgentDiscoveryServiceJavaSdkITCase | shouldPollExistingAgentOnlyWhenCompleteFingerprintChanges | GRPC_API | ⏭️ |
| T-NAC-AISDK-008-06 | F-NAC-AISDK-008 | AgentDiscoveryServiceJavaSdkITCase | shouldTrackVersionEvolutionAcrossRegistrationOrders | GRPC_API | ⏭️ |
| T-NAC-AISDK-008-07 | F-NAC-AISDK-008 | AgentDiscoveryServiceJavaSdkITCase | shouldApplyPublicationRangeAcrossOnlineVersions | GRPC_API | ⏭️ |
| T-NAC-AISDK-008-08 | F-NAC-AISDK-008 | AgentDiscoveryServiceJavaSdkITCase | shouldRestoreGrpcAndHttpPublicationsAndPollingAfterRealServerRestart | GRPC_API | ⏭️ |
| T-NAC-AISDK-008-09 | F-NAC-AISDK-008 | AgentDiscoveryServiceJavaSdkITCase | shouldDeregisterActiveHttpPublicationDuringIdempotentShutdown | GRPC_API | ⏭️ |
| T-NAC-AISDK-008-10 | F-NAC-AISDK-008 | AgentDiscoveryServiceJavaSdkITCase | shouldKeepHttpAndGrpcDiscoverySemanticsEquivalent | GRPC_API | ⏭️ |
| T-NAC-AISDK-008-11 | F-NAC-AISDK-008 | AgentDiscoveryServiceJavaSdkITCase | shouldRejectInvalidBoundariesBeforeRemoteMutation | GRPC_API | ⏭️ |

---

# Module 5: Lock Service Tests (gRPC)

> Source: `test/lock-test/src/test/java/com/alibaba/nacos/test/lock/`
> Server: `127.0.0.1:8848` via gRPC

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-LOCK-001-04 | F-NAC-LOCK-001,002 | BasicLockITCase | testBasicLockUnlockFlow | GRPC_API | ⬜ |
| T-NAC-LOCK-002-01 | F-NAC-LOCK-002 | BasicLockITCase | testDuplicateUnlockShouldFail | GRPC_API | ⬜ |
| T-NAC-LOCK-002-02 | F-NAC-LOCK-002 | BasicLockITCase | testCannotUnlockOthersLock | GRPC_API | ⬜ |
| T-NAC-LOCK-003-02 | F-NAC-LOCK-003 | BasicLockITCase | testLockAutoExpiration | GRPC_API | ⬜ |
| T-NAC-LOCK-004-01 | F-NAC-LOCK-004 | BasicLockITCase | testLockRenew | GRPC_API | ⬜ |
| T-NAC-LOCK-004-02 | F-NAC-LOCK-004 | BasicLockITCase | testCannotRenewOthersLock | GRPC_API | ⬜ |
| T-NAC-LOCK-001-05 | F-NAC-LOCK-001 | BasicLockITCase | testLockMutualExclusion | GRPC_API | ⬜ |
| T-NAC-LOCK-002-03 | F-NAC-LOCK-002 | BasicLockITCase | testExpiredLockUnlockRejected | GRPC_API | ⬜ |
| T-NAC-LOCK-005-01 | F-NAC-LOCK-005 | ReentrantLockITCase | testReentrantLockMultipleAcquire | GRPC_API | ⬜ |
| T-NAC-LOCK-006-01 | F-NAC-LOCK-006 | ReentrantLockITCase | testNonReentrantLockRejectsReentry | GRPC_API | ⬜ |
| T-NAC-LOCK-007-01 | F-NAC-LOCK-007 | LockFifoITCase | testMultipleWaitersFifoOnRelease | GRPC_API | ⬜ |
| T-NAC-LOCK-007-02 | F-NAC-LOCK-007 | LockFifoITCase | testFifoWithMixedWaitMethods | GRPC_API | ⬜ |
| T-NAC-LOCK-007-03 | F-NAC-LOCK-007 | LockFifoITCase | testFifoPreservedWhenMiddleWaiterCancels | GRPC_API | ⬜ |
| T-NAC-LOCK-007-04 | F-NAC-LOCK-007 | LockFifoITCase | testFifoConcurrentLockNoPreHolder | GRPC_API | ⬜ |
| T-NAC-LOCK-007-05 | F-NAC-LOCK-007 | LockFifoITCase | testFifoAfterLockExpiry | GRPC_API | ⬜ |
| T-NAC-LOCK-009-01 | F-NAC-LOCK-009 | JucLockITCase | testReentrantTryLock | GRPC_API | ⬜ |
| T-NAC-LOCK-009-02 | F-NAC-LOCK-009 | JucLockITCase | testReentrantTryLockWithTimeout | GRPC_API | ⬜ |
| T-NAC-LOCK-009-03 | F-NAC-LOCK-009 | JucLockITCase | testReentrantLockReentry | GRPC_API | ⬜ |
| T-NAC-LOCK-009-04 | F-NAC-LOCK-009 | JucLockITCase | testReentrantTryLockReentry | GRPC_API | ⬜ |
| T-NAC-LOCK-009-05 | F-NAC-LOCK-009 | JucLockITCase | testReentrantLockMutualExclusion | GRPC_API | ⬜ |
| T-NAC-LOCK-009-06 | F-NAC-LOCK-009 | JucLockITCase | testReentrantTryLockFails | GRPC_API | ⬜ |
| T-NAC-LOCK-009-07 | F-NAC-LOCK-009 | JucLockITCase | testReentrantTryLockTimeoutThenSuccess | GRPC_API | ⬜ |
| T-NAC-LOCK-009-08 | F-NAC-LOCK-009 | JucLockITCase | testReentrantTryLockTimeoutFails | GRPC_API | ⬜ |
| T-NAC-LOCK-009-09 | F-NAC-LOCK-009 | JucLockITCase | testReentrantUnlockWithoutLock | GRAC_API | ⬜ |
| T-NAC-LOCK-009-10 | F-NAC-LOCK-009 | JucLockITCase | testReentrantNewConditionUnsupported | GRPC_API | ⬜ |
| T-NAC-LOCK-009-11 | F-NAC-LOCK-009 | JucLockITCase | testReentrantLockInterruptibly | GRPC_API | ⬜ |
| T-NAC-LOCK-009-12 | F-NAC-LOCK-009 | JucLockITCase | testLockInterruptiblyInterruptShouldCancelServerWaiter | GRPC_API | ⬜ |
| T-NAC-LOCK-009-13 | F-NAC-LOCK-009 | JucLockITCase | testNonReentrantLockTryLockRejectsReentry | GRPC_API | ⬜ |
| T-NAC-LOCK-009-14 | F-NAC-LOCK-009 | JucLockITCase | testNonReentrantLockLockRejectsReentry | GRPC_API | ⬜ |
| T-NAC-LOCK-009-15 | F-NAC-LOCK-009 | JucLockITCase | testNonReentrantTryLockRejectsReentry | GRPC_API | ⬜ |
| T-NAC-LOCK-009-16 | F-NAC-LOCK-009 | JucLockITCase | testNonReentrantTryLockWithTimeoutRejectsReentry | GRPC_API | ⬜ |
| T-NAC-LOCK-009-17 | F-NAC-LOCK-009 | JucLockITCase | testNonReentrantLockInterruptiblyRejectsReentry | GRPC_API | ⬜ |
| T-NAC-LOCK-009-18 | F-NAC-LOCK-009 | JucLockITCase | testNonReentrantLockMutualExclusion | GRPC_API | ⬜ |
| T-NAC-LOCK-009-19 | F-NAC-LOCK-009 | JucLockITCase | testWatchdogAutoRenew | GRPC_API | ⬜ |
| T-NAC-LOCK-009-20 | F-NAC-LOCK-009 | JucLockITCase | testReentrantLockConcurrency | GRPC_API | ⬜ |
| T-NAC-LOCK-009-21 | F-NAC-LOCK-009 | JucLockITCase | testJucHighConcurrencyStability | GRPC_API | ⬜ |
| T-NAC-LOCK-009-22 | F-NAC-LOCK-009 | JucLockITCase | testReentrantLockHandoff | GRPC_API | ⬜ |
| T-NAC-LOCK-008-01 | F-NAC-LOCK-008 | JucLockITCase | testConnectionCleanupFullyReleasesReentrantLock | GRPC_API | ⬜ |
| T-NAC-LOCK-009-23 | F-NAC-LOCK-009 | JucLockITCase | testFifoMultipleWaitersOrdered | GRPC_API | ⬜ |
| T-NAC-LOCK-009-24 | F-NAC-LOCK-009 | JucLockITCase | testNewClientCannotStealLockWithWaiters | GRPC_API | ⬜ |
| T-NAC-LOCK-009-25 | F-NAC-LOCK-009 | JucLockRegressionITCase | testTryLockTimeoutInterruptCleansServerQueue | GRPC_API | ⬜ |
| T-NAC-LOCK-009-26 | F-NAC-LOCK-009 | JucLockRegressionITCase | testUnlockExceptionAllowsReacquire | GRPC_API | ⬜ |
| T-NAC-LOCK-009-27 | F-NAC-LOCK-009 | JucLockRegressionITCase | testInterruptHeadWaiterPromotesNext | GRPC_API | ⬜ |
| T-NAC-LOCK-009-28 | F-NAC-LOCK-009 | JucLockRegressionITCase | testTryLockTimeoutCleansServerQueue | GRPC_API | ⬜ |
| T-NAC-LOCK-009-29 | F-NAC-LOCK-009 | JucLockRegressionITCase | testWatchdogRenewKeepsLockAlive | GRPC_API | ⬜ |
| T-NAC-LOCK-009-30 | F-NAC-LOCK-009 | JucLockRegressionITCase | testThreadPoolNoThreadLocalLeak | GRPC_API | ⬜ |
| T-NAC-LOCK-009-31 | F-NAC-LOCK-009 | JucLockRegressionITCase | testNonReentrantLocalGuardPreventsSelfDeadlock | GRPC_API | ⬜ |
| T-NAC-LOCK-009-32 | F-NAC-LOCK-009 | JucLockRegressionITCase | testCancelWaitIdempotent | GRPC_API | ⬜ |
| T-NAC-LOCK-009-33 | F-NAC-LOCK-009 | JucLockRegressionITCase | testLockInterruptThrowsAndRestoresFlag | GRPC_API | ⬜ |
| T-NAC-LOCK-007-06 | F-NAC-LOCK-007 | WaitQueueLockITCase | testWaitQueueEnqueue | GRPC_API | ⬜ |
| T-NAC-LOCK-007-07 | F-NAC-LOCK-007 | WaitQueueLockITCase | testWaitQueueTimeout | GRPC_API | ⬜ |
| T-NAC-LOCK-007-08 | F-NAC-LOCK-007 | WaitQueueLockITCase | testWaitQueueSequentialAcquisition | GRPC_API | ⬜ |
| T-NAC-LOCK-007-09 | F-NAC-LOCK-007 | WaitQueueLockITCase | testWaitQueueMultipleWaiters | GRPC_API | ⬜ |
| T-NAC-LOCK-007-10 | F-NAC-LOCK-007 | WaitQueueLockITCase | testWaitQueueNoWait | GRPC_API | ⬜ |
| T-NAC-LOCK-007-11 | F-NAC-LOCK-007 | WaitQueueLockITCase | testNonHeadWaiterRetryMustNotAcquireFreeLock | GRPC_API | ⬜ |
| T-NAC-LOCK-007-12 | F-NAC-LOCK-007 | WaitQueueLockITCase | testOutOfOrderRetryMustNotLeaveStaleWaiter | GRPC_API | ⬜ |
| T-NAC-LOCK-007-13 | F-NAC-LOCK-007 | WaitQueueLockITCase | testCancelWaitShouldRemoveServerWaiter | GRPC_API | ⬜ |
| T-NAC-LOCK-001-06 | F-NAC-LOCK-001 | ConcurrentLockITCase | testConcurrentLockCompetition | GRPC_API | ⬜ |
| T-NAC-LOCK-001-07 | F-NAC-LOCK-001,002 | ConcurrentLockITCase | testLockReleaseAndAcquire | GRPC_API | ⬜ |
| T-NAC-LOCK-001-08 | F-NAC-LOCK-001 | ConcurrentLockITCase | testHighConcurrencyStability | GRPC_API | ⬜ |
| T-NAC-LOCK-010-01 | F-NAC-LOCK-010 | BackwardCompatibilityITCase | testBackwardCompatibility_NoOwnerShouldUseConnectionId | GRPC_API | ⬜ |

---

# Module 6: Maintainer SDK Tests (HTTP)

> Source: `test/maintainer-sdk-test/src/test/java/com/alibaba/nacos/test/maintainer/`
> Server: `127.0.0.1:8848` via HTTP, path prefix `/nacos/v3/admin/`

| T-ID | F-ID | Upstream file | Method | Type | Status |
|------|------|---------------|--------|------|--------|
| T-NAC-MAINT-001-01 | F-NAC-MAINT-001 | ConfigMaintainerServiceMaintainerSdkITCase | shouldManageConfigLifecycle | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-001-02 | F-NAC-MAINT-001 | ConfigMaintainerServiceMaintainerSdkITCase | shouldDeleteConfigByStorageId | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-001-03 | F-NAC-MAINT-001 | ConfigMaintainerServiceMaintainerSdkITCase | shouldCloneConfigWithinNamespace | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-001-04 | F-NAC-MAINT-001 | ConfigMaintainerServiceMaintainerSdkITCase | shouldCloneConfigAcrossNamespaces | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-001-05 | F-NAC-MAINT-001 | ConfigMaintainerServiceMaintainerSdkITCase | shouldSkipCloneIdsOutsideSourceNamespace | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-001-06 | F-NAC-MAINT-001 | ConfigMaintainerServiceMaintainerSdkITCase | shouldApplyCloneConflictPolicies | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-001-07 | F-NAC-MAINT-001 | ConfigMaintainerServiceMaintainerSdkITCase | shouldReturnCloneFailureDataForEmptySelection | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-001-08 | F-NAC-MAINT-001 | ConfigMaintainerServiceMaintainerSdkITCase | shouldQueryConfigHistory | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-001-09 | F-NAC-MAINT-001 | ConfigMaintainerServiceMaintainerSdkITCase | shouldManageBetaConfig | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-001-10 | F-NAC-MAINT-001 | ConfigMaintainerServiceMaintainerSdkITCase | shouldQueryConfigListenerDiagnostics | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-001-11 | F-NAC-MAINT-001 | ConfigMaintainerServiceMaintainerSdkITCase | shouldRunConfigOpsMaintenanceCommands | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-001-12 | F-NAC-MAINT-001 | ConfigMaintainerServiceMaintainerSdkITCase | shouldRejectInvalidConfigParameters | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-003-01 | F-NAC-MAINT-003 | CoreMaintainerServiceMaintainerSdkITCase | shouldQueryStandaloneServerState | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-003-02 | F-NAC-MAINT-003 | CoreMaintainerServiceMaintainerSdkITCase | shouldQueryReadOnlyCoreOperationalViews | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-003-03 | F-NAC-MAINT-003 | CoreMaintainerServiceMaintainerSdkITCase | shouldQueryPluginDetailAndTypeFilter | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-003-04 | F-NAC-MAINT-003 | CoreMaintainerServiceMaintainerSdkITCase | shouldManageNamespaceLifecycle | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-003-05 | F-NAC-MAINT-003 | CoreMaintainerServiceMaintainerSdkITCase | shouldRejectInvalidNamespaceParameters | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-003-06 | F-NAC-MAINT-003 | CoreMaintainerServiceMaintainerSdkITCase | shouldMapUnavailableServerToNacosException | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-002-01 | F-NAC-MAINT-002 | NamingMaintainerServiceMaintainerSdkITCase | shouldManageServiceLifecycle | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-002-02 | F-NAC-MAINT-002 | NamingMaintainerServiceMaintainerSdkITCase | shouldManagePersistentInstanceLifecycle | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-002-03 | F-NAC-MAINT-002 | NamingMaintainerServiceMaintainerSdkITCase | shouldManageClusterAndInstanceHealth | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-002-04 | F-NAC-MAINT-002 | NamingMaintainerServiceMaintainerSdkITCase | shouldRejectInvalidNamingParameters | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-002-05 | F-NAC-MAINT-002 | NamingMaintainerServiceMaintainerSdkITCase | shouldQueryNamingStaticDiagnostics | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-002-06 | F-NAC-MAINT-002 | NamingMaintainerServiceMaintainerSdkITCase | shouldQueryNamingClientDiagnostics | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-002-07 | F-NAC-MAINT-002 | NamingMaintainerServiceMaintainerSdkITCase | shouldQueryNamingDiagnostics | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-004-01 | F-NAC-MAINT-004 | AiMaintainerServiceMaintainerSdkITCase | shouldCreateAiMaintainerServiceAndQueryDelegates | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-004-02 | F-NAC-MAINT-004 | AiMaintainerServiceMaintainerSdkITCase | shouldManageMcpServerLifecycle | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-004-03 | F-NAC-MAINT-004 | AiMaintainerServiceMaintainerSdkITCase | shouldManageA2aAgentLifecycle | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-004-04 | F-NAC-MAINT-004 | AiMaintainerServiceMaintainerSdkITCase | shouldManagePromptLifecycle | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-004-05 | F-NAC-MAINT-004,005 | AiMaintainerServiceMaintainerSdkITCase | shouldManageSkillAndAgentSpecLifecycle | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-004-06 | F-NAC-MAINT-004 | AiMaintainerServiceMaintainerSdkITCase | shouldUploadSkillFromZipWithTargetVersion | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-004-07 | F-NAC-MAINT-004 | AiMaintainerServiceMaintainerSdkITCase | shouldBatchUploadSkillsFromZip | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-004-08 | F-NAC-MAINT-004 | AiMaintainerServiceMaintainerSdkITCase | shouldUploadAgentSpecFromZip | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-004-09 | F-NAC-MAINT-004 | AiMaintainerServiceMaintainerSdkITCase | shouldRejectInvalidAiMaintainerParameters | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-005-01 | F-NAC-MAINT-005 | AgentMaintainerServiceMaintainerSdkITCase | shouldManageAgentInDefaultNamespace | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-005-02 | F-NAC-MAINT-005 | AgentMaintainerServiceMaintainerSdkITCase | shouldApplyEveryListFilterBeforePaging | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-005-03 | F-NAC-MAINT-005 | AgentMaintainerServiceMaintainerSdkITCase | shouldManageDraftLifecycleInExplicitNamespace | SDK_CLIENT | ⬜ |
| T-NAC-MAINT-005-04 | F-NAC-MAINT-005 | AgentMaintainerServiceMaintainerSdkITCase | shouldMapValidationErrorsWithoutBreakingLegacyDelegate | SDK_CLIENT | ⬜ |

---

# Summary

| Module | Test files | Test methods | Mapped T-IDs | ⏭️ Skip | ⬜ To port |
|--------|-----------|-------------|-------------|---------|-----------|
| 1. Admin API (HTTP) | 39 | ~115 | 80 | 6 | 74 |
| 2. Console API (HTTP) | 28 | ~83 | 65 | 0 | 65 |
| 3. Client API (HTTP) | 10 | ~51 | 29 | 4 | 25 |
| 4. Java SDK (gRPC) | 5 | ~54 | 35 | 11 | 24 |
| 5. Lock Service (gRPC) | 9 | ~62 | 57 | 0 | 57 |
| 6. Maintainer SDK (HTTP) | 5 | ~38 | 38 | 0 | 38 |
| **Total** | **91** | **~420** | **304** | **21** | **283** |

> 21 tests marked ⏭️ skip because the corresponding F-ID is ⚪ (not implemented).
> 283 tests are candidates for porting — all connect to a real server and verify API contracts.
> Porting strategy: adapt each upstream test to point at batata server, skip tests for ⚪ features, fix batata bugs when tests fail.
