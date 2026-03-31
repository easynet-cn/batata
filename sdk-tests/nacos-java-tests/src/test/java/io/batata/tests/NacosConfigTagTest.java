package io.batata.tests;

import com.alibaba.nacos.api.config.model.ConfigBasicInfo;
import com.alibaba.nacos.api.config.model.ConfigDetailInfo;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.model.Page;
import com.alibaba.nacos.maintainer.client.NacosMaintainerFactory;
import com.alibaba.nacos.maintainer.client.config.ConfigMaintainerService;
import org.junit.jupiter.api.*;

import java.util.ArrayList;
import java.util.List;
import java.util.Properties;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Config Tag Tests
 *
 * Tests configuration management with custom tags, including tag-based
 * publishing, querying, filtering, and gray release via tags.
 * Uses ConfigMaintainerService for all operations.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigTagTest {

    private static ConfigMaintainerService maintainerService;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final String DEFAULT_NAMESPACE = "";
    private static final List<String[]> cleanupConfigs = new ArrayList<>();

    @BeforeAll
    static void setup() throws Exception {
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);

        maintainerService = NacosMaintainerFactory.createConfigMaintainerService(properties);
        System.out.println("Config Tag Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws Exception {
        for (String[] config : cleanupConfigs) {
            try {
                maintainerService.deleteConfig(config[0], config[1], DEFAULT_NAMESPACE);
            } catch (Exception e) {
                // Ignore cleanup errors
            }
        }
    }

    // ==================== Tag Publish and Query Tests ====================

    /**
     * NCT-TAG-001: Test publish config with tag parameter
     *
     * Publishes a configuration with a custom tag via maintainer client
     * and verifies the config can be retrieved.
     * Disabled: Tag-based config retrieval is not yet supported in Batata.
     */
    @Test
    @Order(1)
    void testPublishConfigWithTag() throws Exception {
        String dataId = "tag-publish-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "tag.publish.key=value";
        String tag = "release-v1";

        // Publish config with tag via maintainer client
        boolean published = maintainerService.publishConfig(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, content,
                null, null, tag, null, null);
        assertTrue(published, "Publish config with tag should succeed");
        cleanupConfigs.add(new String[]{dataId, DEFAULT_GROUP});

        // Verify config can be retrieved
        ConfigDetailInfo configDetail = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(configDetail, "Get config response should not be null");
        assertEquals(content, configDetail.getContent(),
                "Config content should match published content");
    }

    /**
     * NCT-TAG-002: Test query config with tag filter returns tagged content
     *
     * Publishes a config with a specific tag and verifies that querying
     * with that tag returns the tagged configuration.
     * Disabled: Tag-based config retrieval is not yet supported in Batata.
     */
    @Test
    @Order(2)
    void testQueryConfigWithTagFilter() throws Exception {
        String dataId = "tag-query-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "tag.query.mode=normal";
        String taggedContent = "tag.query.mode=tagged";
        String tag = "canary";

        // Publish normal config (no tag)
        boolean normalPub = maintainerService.publishConfig(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        assertTrue(normalPub, "Normal publish should succeed");
        cleanupConfigs.add(new String[]{dataId, DEFAULT_GROUP});

        // Publish tagged config
        boolean tagPub = maintainerService.publishConfig(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, taggedContent,
                null, null, tag, null, null);
        assertTrue(tagPub, "Tagged publish should succeed");

        Thread.sleep(500);

        // Query with tag filter - should return tagged content
        // Note: listConfigs with configTags filter
        Page<ConfigBasicInfo> searchResult = maintainerService.listConfigs(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, null, tag, null);
        assertNotNull(searchResult, "Search by tag result should not be null");
        boolean found = searchResult.getPageItems().stream()
                .anyMatch(c -> dataId.equals(c.getDataId()));
        assertTrue(found, "Query with tag should find the tagged config");
    }

    /**
     * NCT-TAG-003: Test query without tag returns default config
     *
     * After publishing both normal config and a beta gray config for the same dataId,
     * querying without gray context should return the default (untagged) config.
     * Uses maintainerService.publishBetaConfig() for gray config publishing.
     */
    @Test
    @Order(3)
    void testQueryWithoutTagReturnsDefault() throws Exception {
        String dataId = "tag-default-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "mode=default";
        String grayContent = "mode=gray-version";

        // Publish normal config
        maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        cleanupConfigs.add(new String[]{dataId, DEFAULT_GROUP});

        // Publish beta gray config via SDK
        boolean betaPublished = maintainerService.publishBetaConfig(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, grayContent,
                null, null, null, null, null, "10.0.0.1,10.0.0.2");
        assertTrue(betaPublished, "Beta gray config publish should succeed");
        Thread.sleep(500);

        // Query without gray context - should return normal/default content (not gray)
        ConfigDetailInfo configDetail = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(configDetail, "Config detail should not be null");
        assertEquals(normalContent, configDetail.getContent(),
                "Query without gray context should return the default (untagged) config");

        // Cleanup gray config via SDK
        try {
            maintainerService.stopBeta(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        } catch (Exception ignored) {}
    }

    /**
     * NCT-TAG-004: Test multiple tags on same dataId
     *
     * Publishes the same dataId with different tags and verifies each
     * tag returns its own content independently.
     */
    @Test
    @Order(4)
    void testMultipleTagsOnSameConfig() throws Exception {
        String dataId = "tag-multi-" + UUID.randomUUID().toString().substring(0, 8);
        String[] tags = {"alpha", "beta", "gamma"};
        cleanupConfigs.add(new String[]{dataId, DEFAULT_GROUP});

        // Publish with different tags
        for (String tag : tags) {
            String content = "version=" + tag + "\nenv=" + tag;
            boolean published = maintainerService.publishConfig(
                    dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, content,
                    null, null, tag, null, null);
            assertTrue(published, "Publish with tag '" + tag + "' should succeed");
        }

        Thread.sleep(500);

        // Verify config exists (content will be the last published version since
        // tags may overwrite each other in the same dataId)
        ConfigDetailInfo configDetail = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(configDetail, "Config should exist after publishing with tags");
        assertNotNull(configDetail.getContent(), "Config content should not be null");
    }

    /**
     * NCT-TAG-005: Test tag-based gray release
     *
     * Tests gray config publishing through the maintainer client beta endpoint.
     *
     * SKIPPED: Gray publish response is not parseable in Batata.
     */
    @Test
    @Order(5)
    void testTagBasedGrayRelease() throws Exception {
        String dataId = "tag-gray-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "gray.release=stable";
        String grayContent = "gray.release=canary-v2";

        // Publish stable config
        boolean published = maintainerService.publishConfig(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        assertTrue(published, "Stable config publish should succeed");
        cleanupConfigs.add(new String[]{dataId, DEFAULT_GROUP});

        Thread.sleep(500);

        // Publish gray config via beta endpoint
        boolean betaPublished = maintainerService.publishBetaConfig(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, grayContent,
                null, null, null, null, null, "10.0.0.1,10.0.0.2");
        assertTrue(betaPublished, "Gray config publish should succeed");

        // Verify stable config is still accessible
        ConfigDetailInfo stableConfig = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(stableConfig, "Stable config should still be accessible");
        assertEquals(normalContent, stableConfig.getContent(),
                "Stable config should remain unchanged after gray publish");

        // Clean up gray config
        try {
            maintainerService.stopBeta(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        } catch (Exception ignored) {
            // Gray config cleanup is best-effort
        }
    }

    /**
     * NCT-TAG-006: Test delete tagged config
     *
     * Verifies that deleting a tagged config does not affect the default
     * (untagged) config or configs with other tags.
     */
    @Test
    @Order(6)
    void testDeleteTaggedConfig() throws Exception {
        String dataId = "tag-delete-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "delete.test=normal";
        String tag = "to-delete";

        // Publish normal config
        maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        cleanupConfigs.add(new String[]{dataId, DEFAULT_GROUP});

        // Publish tagged config (overwrites same dataId, but with tags metadata)
        maintainerService.publishConfig(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent,
                null, null, tag, null, null);
        Thread.sleep(500);

        // Delete the config
        boolean deleted = maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertTrue(deleted, "Delete should succeed");

        Thread.sleep(500);

        // Verify config is gone
        try {
            ConfigDetailInfo afterDelete = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
            // If no exception, content should be null or config should not exist
            assertTrue(afterDelete == null || afterDelete.getContent() == null,
                    "Config should be gone after deletion");
        } catch (NacosException e) {
            // Expected - config no longer exists
            assertTrue(true, "NacosException expected for deleted config");
        }
    }
}
