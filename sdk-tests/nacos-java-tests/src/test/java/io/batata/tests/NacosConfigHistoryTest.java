package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.config.model.ConfigBasicInfo;
import com.alibaba.nacos.api.config.model.ConfigHistoryBasicInfo;
import com.alibaba.nacos.api.config.model.ConfigHistoryDetailInfo;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.model.Page;
import com.alibaba.nacos.maintainer.client.NacosMaintainerFactory;
import com.alibaba.nacos.maintainer.client.config.ConfigMaintainerService;
import org.junit.jupiter.api.*;

import java.util.List;
import java.util.Properties;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Config History API Tests
 *
 * Tests for configuration history tracking and retrieval.
 * Uses ConfigMaintainerService for history operations.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigHistoryTest {

    private static ConfigService configService;
    private static ConfigMaintainerService maintainerService;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final String DEFAULT_NAMESPACE = "";

    @BeforeAll
    static void setup() throws Exception {
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);

        configService = NacosFactory.createConfigService(properties);
        maintainerService = NacosMaintainerFactory.createConfigMaintainerService(properties);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (configService != null) {
            configService.shutDown();
        }
    }

    // ==================== Config History Tests ====================

    /**
     * Test list config history
     */
    @Test
    @Order(1)
    void testListConfigHistory() throws Exception {
        String dataId = "history-test-" + UUID.randomUUID().toString().substring(0, 8);
        int versionCount = 3;

        // Create multiple versions
        for (int i = 1; i <= versionCount; i++) {
            boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "version=" + i);
            assertTrue(published, "Config version " + i + " should be published successfully");
            Thread.sleep(500);
        }

        // List history via maintainer client
        Page<ConfigHistoryBasicInfo> historyPage = maintainerService.listConfigHistory(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, 1, 10);

        assertNotNull(historyPage, "History list page should not be null");
        assertTrue(historyPage.getTotalCount() >= versionCount,
                "History should contain at least " + versionCount + " entries, got totalCount: " + historyPage.getTotalCount());

        // Verify history records have IDs
        assertNotNull(historyPage.getPageItems(), "History page items should not be null");
        assertFalse(historyPage.getPageItems().isEmpty(), "History page items should not be empty");
        assertNotNull(historyPage.getPageItems().get(0).getId(),
                "History records should have IDs");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * Test get specific history version
     */
    @Test
    @Order(2)
    void testGetHistoryVersion() throws Exception {
        String dataId = "history-version-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "initial=value";

        // Create config
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, content);
        assertTrue(published, "Config should be published successfully");
        Thread.sleep(500);

        // Get history list first to get nid
        Page<ConfigHistoryBasicInfo> historyPage = maintainerService.listConfigHistory(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, 1, 10);

        assertNotNull(historyPage, "History list page should not be null");
        assertFalse(historyPage.getPageItems().isEmpty(), "History should have at least one entry");

        Long nid = historyPage.getPageItems().get(0).getId();
        assertNotNull(nid, "Should be able to extract a history record ID");

        // Get specific history version
        ConfigHistoryDetailInfo historyDetail = maintainerService.getConfigHistoryInfo(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, nid);

        assertNotNull(historyDetail, "History version detail should not be null");
        assertEquals(content, historyDetail.getContent(),
                "History version should contain the original content");
        assertEquals(dataId, historyDetail.getDataId(),
                "History version should reference the correct dataId");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * Test history count increases after updates
     */
    @Test
    @Order(3)
    void testHistoryCountIncreasesAfterUpdates() throws Exception {
        String dataId = "history-count-" + UUID.randomUUID().toString().substring(0, 8);

        // Publish initial version
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "version=1");
        assertTrue(published, "Initial config should be published successfully");
        Thread.sleep(500);

        // Get initial history count
        Page<ConfigHistoryBasicInfo> page1 = maintainerService.listConfigHistory(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, 1, 100);
        int initialCount = page1.getTotalCount();
        assertTrue(initialCount >= 1,
                "After first publish, history should have at least 1 entry, got: " + initialCount);

        // Publish more versions
        for (int i = 2; i <= 4; i++) {
            configService.publishConfig(dataId, DEFAULT_GROUP, "version=" + i);
            Thread.sleep(500);
        }

        // Get updated history count
        Page<ConfigHistoryBasicInfo> page2 = maintainerService.listConfigHistory(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, 1, 100);
        int updatedCount = page2.getTotalCount();

        assertTrue(updatedCount > initialCount,
                "History count should increase after updates; initial=" + initialCount + ", updated=" + updatedCount);
        assertTrue(updatedCount >= 4,
                "After 4 publishes, history should have at least 4 entries, got: " + updatedCount);

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * Test get previous version
     */
    @Test
    @Order(4)
    void testGetPreviousVersion() throws Exception {
        String dataId = "history-prev-" + UUID.randomUUID().toString().substring(0, 8);
        String content1 = "version=1";
        String content2 = "version=2";

        // Create two versions
        boolean pub1 = configService.publishConfig(dataId, DEFAULT_GROUP, content1);
        assertTrue(pub1, "First version should be published successfully");
        Thread.sleep(500);
        boolean pub2 = configService.publishConfig(dataId, DEFAULT_GROUP, content2);
        assertTrue(pub2, "Second version should be published successfully");
        Thread.sleep(500);

        // Get history list to find the record ID
        Page<ConfigHistoryBasicInfo> historyPage = maintainerService.listConfigHistory(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, 1, 10);
        assertNotNull(historyPage, "History list should not be null");
        assertFalse(historyPage.getPageItems().isEmpty(), "Should find at least one history record");

        Long recordId = historyPage.getPageItems().get(0).getId();
        assertNotNull(recordId, "Should find at least one history record ID");

        // Get previous version using the record ID
        ConfigHistoryDetailInfo previousVersion = maintainerService.getPreviousConfigHistoryInfo(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, recordId);

        assertNotNull(previousVersion, "Previous version response should not be null");
        // The previous version should contain meaningful config data
        assertTrue(previousVersion.getDataId() != null || previousVersion.getContent() != null,
                "Previous version response should contain meaningful config data");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * Test list all configs in namespace
     */
    @Test
    @Order(5)
    void testListNamespaceConfigs() throws Exception {
        String prefix = "ns-config-" + UUID.randomUUID().toString().substring(0, 8);
        int configCount = 3;

        // Create multiple configs
        for (int i = 0; i < configCount; i++) {
            boolean published = configService.publishConfig(prefix + "-" + i, DEFAULT_GROUP, "content=" + i);
            assertTrue(published, "Config " + prefix + "-" + i + " should be published successfully");
        }
        Thread.sleep(1000);

        // List all configs in namespace (empty namespace = public) via maintainer client
        List<ConfigBasicInfo> configList = maintainerService.getConfigListByNamespace(DEFAULT_NAMESPACE);

        assertNotNull(configList, "Namespace configs list should not be null");
        // Verify our configs appear in the response
        for (int i = 0; i < configCount; i++) {
            String expectedDataId = prefix + "-" + i;
            boolean found = configList.stream().anyMatch(c -> expectedDataId.equals(c.getDataId()));
            assertTrue(found, "Namespace config list should contain config: " + expectedDataId);
        }

        // Cleanup
        for (int i = 0; i < configCount; i++) {
            configService.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
        }
    }

    /**
     * Test history with namespace isolation
     *
     * SKIPPED: Embedded RocksDB prefix search may match partial keys across namespaces,
     * causing isolated namespace queries to return unexpected results.
     */
    @Test
    @Order(6)
    void testHistoryNamespaceIsolation() throws Exception {
        String dataId = "isolated-config-" + UUID.randomUUID().toString().substring(0, 8);
        String namespace = "history-isolation-unique-ns-" + UUID.randomUUID().toString();

        // Publish config in default namespace
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "default-ns-content");
        assertTrue(published, "Config in default namespace should be published");
        Thread.sleep(500);

        // Query history in a different (non-existent) namespace - should have no records for this dataId
        Page<ConfigHistoryBasicInfo> isolatedPage = maintainerService.listConfigHistory(
                dataId, DEFAULT_GROUP, namespace, 1, 10);

        assertNotNull(isolatedPage, "Namespace history page should not be null");
        int isolatedCount = isolatedPage.getTotalCount();
        // In a namespace where we never published, there should be 0 records
        assertTrue(isolatedCount == 0,
                "History in isolated namespace should have 0 entries, got: " + isolatedCount);

        // Verify default namespace has history
        Page<ConfigHistoryBasicInfo> defaultPage = maintainerService.listConfigHistory(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, 1, 10);
        int defaultCount = defaultPage.getTotalCount();
        assertTrue(defaultCount >= 1,
                "History in default namespace should have at least 1 entry, got: " + defaultCount);

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }
}
