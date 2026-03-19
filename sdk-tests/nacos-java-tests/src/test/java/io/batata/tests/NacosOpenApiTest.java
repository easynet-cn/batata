package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.config.model.ConfigBasicInfo;
import com.alibaba.nacos.api.config.model.ConfigDetailInfo;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.model.Page;
import com.alibaba.nacos.maintainer.client.NacosMaintainerFactory;
import com.alibaba.nacos.maintainer.client.config.ConfigMaintainerService;
import org.junit.jupiter.api.*;

import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Open API Tests
 *
 * Tests for config management using ConfigMaintainerService and SDK.
 * Aligned with Nacos AbstractConfigAPIConfigITCase Open API tests.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosOpenApiTest {

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
        if (configService != null) configService.shutDown();
    }

    // ==================== P1: Open API Config Detail ====================

    /**
     * OA-001: Test get config detail via maintainer client
     *
     * Aligned with Nacos AbstractConfigAPIConfigITCase.testOpenApiDetailConfig()
     */
    @Test
    @Order(1)
    void testOpenApiDetailConfig() throws Exception {
        String dataId = "oa-detail-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "oa.detail.key=value1";

        // Publish via SDK
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, content);
        assertTrue(published, "Config publish via SDK should succeed");
        Thread.sleep(1000);

        // Get detail via maintainer client
        ConfigDetailInfo configDetail = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(configDetail, "Config detail should not be null");
        assertEquals(content, configDetail.getContent(),
                "Maintainer client should return the published config content");
        assertEquals(dataId, configDetail.getDataId(),
                "Config detail should have correct dataId");

        // Verify via SDK get to confirm consistency
        String sdkContent = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertEquals(content, sdkContent,
                "SDK get should return the same content as published");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * OA-002: Test fuzzy search config via maintainer client
     *
     * Aligned with Nacos AbstractConfigAPIConfigITCase.testOpenApiFuzzySearchConfig()
     */
    @Test
    @Order(2)
    void testOpenApiFuzzySearchConfig() throws Exception {
        String prefix = "oa-fuzzy-" + UUID.randomUUID().toString().substring(0, 6);
        String dataId1 = prefix + "-config-a";
        String dataId2 = prefix + "-config-b";

        boolean pub1 = configService.publishConfig(dataId1, DEFAULT_GROUP, "fuzzy.a=true");
        boolean pub2 = configService.publishConfig(dataId2, DEFAULT_GROUP, "fuzzy.b=true");
        assertTrue(pub1, "Publishing first fuzzy config should succeed");
        assertTrue(pub2, "Publishing second fuzzy config should succeed");
        Thread.sleep(1000);

        // Fuzzy search using searchConfigs (blur mode)
        Page<ConfigBasicInfo> searchResult = maintainerService.searchConfigs(
                prefix, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(searchResult, "Fuzzy search result should not be null");

        // Both configs should appear in the search results
        List<ConfigBasicInfo> items = searchResult.getPageItems();
        assertNotNull(items, "Search result items should not be null");
        boolean foundFirst = items.stream().anyMatch(c -> dataId1.equals(c.getDataId()));
        boolean foundSecond = items.stream().anyMatch(c -> dataId2.equals(c.getDataId()));
        assertTrue(foundFirst, "Fuzzy search should find first config '" + dataId1 + "'");
        assertTrue(foundSecond, "Fuzzy search should find second config '" + dataId2 + "'");

        // Cleanup
        configService.removeConfig(dataId1, DEFAULT_GROUP);
        configService.removeConfig(dataId2, DEFAULT_GROUP);
    }

    /**
     * OA-003: Test accurate search config via maintainer client
     *
     * Aligned with Nacos AbstractConfigAPIConfigITCase.testOpenApiSearchConfig()
     */
    @Test
    @Order(3)
    void testOpenApiAccurateSearchConfig() throws Exception {
        String dataId = "oa-search-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "search.key=found";

        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, content);
        assertTrue(published, "Config publish should succeed");
        Thread.sleep(1000);

        // Accurate search using listConfigs
        Page<ConfigBasicInfo> searchResult = maintainerService.listConfigs(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(searchResult, "Accurate search result should not be null");
        boolean found = searchResult.getPageItems().stream()
                .anyMatch(c -> dataId.equals(c.getDataId()));
        assertTrue(found, "Accurate search should find config by exact dataId '" + dataId + "'");

        // Verify negative case: searching for a non-existent dataId should not find this config
        String nonExistent = "nonexistent-" + UUID.randomUUID().toString().substring(0, 8);
        Page<ConfigBasicInfo> negResult = maintainerService.listConfigs(
                nonExistent, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        boolean notFound = negResult.getPageItems().stream()
                .anyMatch(c -> dataId.equals(c.getDataId()));
        assertFalse(notFound, "Search for non-existent dataId should not return our config");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * OA-004: Test config with Chinese characters
     *
     * Aligned with Nacos AbstractConfigAPIConfigITCase.testOpenApiSearchConfigChinese()
     */
    @Test
    @Order(4)
    void testOpenApiSearchWithChinese() throws Exception {
        String dataId = "oa-chinese-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "\u914d\u7f6e\u5185\u5bb9=\u4e2d\u6587\u6d4b\u8bd5";

        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, content);
        assertTrue(published, "Publishing config with Chinese content should succeed");
        Thread.sleep(1000);

        // Get config via maintainer client and verify Chinese content is preserved
        ConfigDetailInfo configDetail = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(configDetail, "Chinese config detail should not be null");
        assertEquals(content, configDetail.getContent(),
                "Maintainer client should return the exact Chinese content");

        // Verify via SDK to confirm round-trip integrity
        String sdkContent = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertEquals(content, sdkContent,
                "SDK get should return the exact Chinese content that was published");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * OA-005: Test publish, get, and delete via maintainer client
     *
     * Aligned with Nacos ConfigAPIV2ConfigITCase.test()
     */
    @Test
    @Order(5)
    void testV2PublishAndGet() throws Exception {
        String dataId = "oa-v2-crud-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "v2.api.key=v2-value";

        // Publish via maintainer client
        boolean published = maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, content);
        assertTrue(published, "Maintainer client publish should succeed");

        Thread.sleep(500);

        // Get via maintainer client
        ConfigDetailInfo configDetail = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(configDetail, "Get config detail should not be null");
        assertEquals(content, configDetail.getContent(),
                "Get should return published content");

        // Verify via SDK to ensure consistency between maintainer client and gRPC
        String sdkContent = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertEquals(content, sdkContent,
                "SDK get should return the same content published via maintainer client");

        // Delete via maintainer client
        boolean deleted = maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertTrue(deleted, "Maintainer client delete should succeed");

        Thread.sleep(500);

        // Verify deleted via SDK
        String afterDelete = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertNull(afterDelete, "Config should be null after deletion via maintainer client");
    }

    /**
     * OA-006: Test get config with null group (uses DEFAULT_GROUP)
     *
     * Aligned with Nacos AbstractConfigAPIConfigITCase.testGetConfigWithNullGroup()
     */
    @Test
    @Order(6)
    void testGetConfigWithNullGroup() throws NacosException, InterruptedException {
        String dataId = "oa-null-group-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "null.group=value";

        // Publish with null group (SDK defaults to DEFAULT_GROUP)
        boolean published = configService.publishConfig(dataId, null, content);
        assertTrue(published, "Publish with null group should succeed");

        Thread.sleep(500);

        // Get with null group
        String retrieved = configService.getConfig(dataId, null, 5000);
        assertEquals(content, retrieved, "Should retrieve config published with null group");

        // Also verify it is accessible with explicit DEFAULT_GROUP
        String withGroup = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(content, withGroup,
                "Config published with null group should be accessible via DEFAULT_GROUP");

        // Cleanup
        configService.removeConfig(dataId, null);
    }

    /**
     * OA-007: Test remove listener for non-existent dataId
     *
     * Aligned with Nacos AbstractConfigAPIConfigITCase.testRemoveListenerForNonexistentDataId()
     */
    @Test
    @Order(7)
    void testRemoveListenerForNonExistentDataId() {
        String dataId = "oa-nonexist-listener-" + UUID.randomUUID().toString().substring(0, 8);

        // Should not throw exception
        assertDoesNotThrow(() -> {
            configService.removeListener(dataId, DEFAULT_GROUP,
                    new com.alibaba.nacos.api.config.listener.Listener() {
                        @Override
                        public java.util.concurrent.Executor getExecutor() {
                            return null;
                        }

                        @Override
                        public void receiveConfigInfo(String configInfo) {
                        }
                    });
        }, "Removing listener for non-existent dataId should not throw");
    }

    /**
     * OA-008: Test remove last listener preserves remaining
     *
     * Aligned with Nacos AbstractConfigAPIConfigITCase.testRemoveLastListener()
     */
    @Test
    @Order(8)
    void testRemoveOneListenerPreservesOther() throws NacosException, InterruptedException {
        String dataId = "oa-keep-listener-" + UUID.randomUUID().toString().substring(0, 8);
        java.util.concurrent.CountDownLatch latch = new java.util.concurrent.CountDownLatch(1);
        java.util.concurrent.atomic.AtomicReference<String> receivedContent = new java.util.concurrent.atomic.AtomicReference<>();

        com.alibaba.nacos.api.config.listener.Listener listener1 = new com.alibaba.nacos.api.config.listener.Listener() {
            @Override
            public java.util.concurrent.Executor getExecutor() { return null; }

            @Override
            public void receiveConfigInfo(String configInfo) {}
        };

        com.alibaba.nacos.api.config.listener.Listener listener2 = new com.alibaba.nacos.api.config.listener.Listener() {
            @Override
            public java.util.concurrent.Executor getExecutor() { return null; }

            @Override
            public void receiveConfigInfo(String configInfo) {
                receivedContent.set(configInfo);
                latch.countDown();
            }
        };

        // Add two listeners
        configService.addListener(dataId, DEFAULT_GROUP, listener1);
        configService.addListener(dataId, DEFAULT_GROUP, listener2);
        Thread.sleep(500);

        // Remove first listener
        configService.removeListener(dataId, DEFAULT_GROUP, listener1);
        Thread.sleep(500);

        // Publish - remaining listener should still fire
        String content = "remaining.listener=fires";
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, content);
        assertTrue(published, "Config publish should succeed");

        boolean received = latch.await(10, java.util.concurrent.TimeUnit.SECONDS);
        assertTrue(received, "Remaining listener should still receive notifications after first is removed");
        assertEquals(content, receivedContent.get(),
                "Remaining listener should receive the exact published content");

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener2);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }
}
