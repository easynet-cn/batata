package io.batata.tests;

import com.alibaba.nacos.api.config.model.ConfigDetailInfo;
import com.alibaba.nacos.api.config.model.ConfigGrayInfo;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.maintainer.client.NacosMaintainerFactory;
import com.alibaba.nacos.maintainer.client.config.ConfigMaintainerService;
import org.junit.jupiter.api.*;

import java.util.Properties;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Config Beta (Gray Release) Tests
 *
 * Tests for beta/gray config functionality via ConfigMaintainerService.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigBetaTest {

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

        maintainerService = NacosMaintainerFactory.createConfigMaintainerService(properties);
    }

    // ==================== Beta Config Tests ====================

    /**
     * BETA-001: Test query beta config that does not exist
     */
    @Test
    @Order(1)
    void testQueryBetaConfigNotExist() throws Exception {
        String dataId = "beta-notexist-" + UUID.randomUUID().toString().substring(0, 8);

        // Query beta for a non-existent config - should throw NacosException or return null
        try {
            ConfigGrayInfo grayInfo = maintainerService.queryBeta(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
            // If no exception, the result should indicate no beta config exists
            // grayInfo may be null or have no content
            assertTrue(grayInfo == null || grayInfo.getContent() == null,
                    "Query for non-existent beta config should not return beta config data");
        } catch (NacosException e) {
            // Expected - beta config does not exist
            assertTrue(true, "NacosException expected for non-existent beta config");
        }
    }

    /**
     * BETA-002: Test publish and query beta config
     */
    @Test
    @Order(2)
    void testPublishAndQueryBetaConfig() throws Exception {
        String dataId = "beta-publish-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "beta.normal=value";
        String betaContent = "beta.gray=value";
        String betaIps = "127.0.0.1";

        // Publish normal config first
        boolean published = maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        assertTrue(published, "Normal config publish should succeed");
        Thread.sleep(500);

        // Publish beta config
        boolean betaPublished = maintainerService.publishBetaConfig(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, betaContent,
                null, null, null, null, null, betaIps);
        assertTrue(betaPublished, "Beta config publish should succeed");

        // Query beta config
        ConfigGrayInfo grayInfo = maintainerService.queryBeta(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(grayInfo, "Beta query response should not be null");
        assertEquals(betaContent, grayInfo.getContent(),
                "Beta config query should return the published beta content");
        assertNotNull(grayInfo.getGrayRule(),
                "Beta config query should return gray rule containing beta IPs");
        assertTrue(grayInfo.getGrayRule().contains(betaIps),
                "Beta config gray rule should contain the beta IPs: " + betaIps);

        // Cleanup
        maintainerService.stopBeta(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
    }

    /**
     * BETA-003: Test delete beta config
     */
    @Test
    @Order(3)
    void testDeleteBetaConfig() throws Exception {
        String dataId = "beta-delete-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "beta.delete.normal=value";
        String betaContent = "beta.delete.gray=value";

        // Publish normal and beta config
        maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        Thread.sleep(500);

        maintainerService.publishBetaConfig(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, betaContent,
                null, null, null, null, null, "127.0.0.1");
        Thread.sleep(500);

        // Verify beta exists before deletion
        ConfigGrayInfo beforeDelete = maintainerService.queryBeta(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(beforeDelete, "Beta config should exist before deletion");
        assertEquals(betaContent, beforeDelete.getContent(),
                "Beta config should have the published content before deletion");

        // Delete beta config (stop beta)
        boolean stopped = maintainerService.stopBeta(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertTrue(stopped, "Stop beta should succeed");

        // Verify beta is gone
        try {
            ConfigGrayInfo afterDelete = maintainerService.queryBeta(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
            assertTrue(afterDelete == null || afterDelete.getContent() == null,
                    "After deletion, beta config content should no longer be returned");
        } catch (NacosException e) {
            // Expected - beta config no longer exists
            assertTrue(true, "NacosException expected after beta deletion");
        }

        // Verify normal config still exists after beta deletion
        ConfigDetailInfo normalConfig = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(normalConfig, "Normal config should still exist after beta config deletion");
        assertEquals(normalContent, normalConfig.getContent(),
                "Normal config should still have its original content");

        // Cleanup
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
    }

    /**
     * BETA-004: Test beta config does not affect normal config retrieval
     */
    @Test
    @Order(4)
    void testBetaConfigDoesNotAffectNormal() throws Exception {
        String dataId = "beta-normal-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "beta.check.normal=original";
        String betaContent = "beta.check.gray=different";

        // Publish normal config
        maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        Thread.sleep(500);

        // Publish beta config with a different IP (not the test client)
        boolean betaPublished = maintainerService.publishBetaConfig(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, betaContent,
                null, null, null, null, null, "10.0.0.1");
        assertTrue(betaPublished, "Beta publish should succeed");
        Thread.sleep(500);

        // Get normal config - should return normal content, not beta
        ConfigDetailInfo normalConfig = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(normalConfig, "Normal config response should not be null");
        assertEquals(normalContent, normalConfig.getContent(),
                "Normal config retrieval should return the normal content");
        assertNotEquals(betaContent, normalConfig.getContent(),
                "Normal config retrieval should NOT return beta content");

        // Verify beta config is independently queryable
        ConfigGrayInfo betaQuery = maintainerService.queryBeta(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(betaQuery, "Beta query should return data");
        assertEquals(betaContent, betaQuery.getContent(),
                "Beta endpoint should return the beta content");
        assertTrue(betaQuery.getGrayRule().contains("10.0.0.1"),
                "Beta endpoint should return the configured beta IPs");

        // Cleanup
        maintainerService.stopBeta(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
    }

    /**
     * BETA-005: Test update beta config content and IPs
     */
    @Test
    @Order(5)
    void testUpdateBetaConfig() throws Exception {
        String dataId = "beta-update-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "beta.update.normal=value";
        String betaContent1 = "beta.update.gray=v1";
        String betaContent2 = "beta.update.gray=v2";
        String betaIps1 = "127.0.0.1";
        String betaIps2 = "127.0.0.1,192.168.1.1";

        // Publish normal config
        maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        Thread.sleep(500);

        // Publish initial beta config
        maintainerService.publishBetaConfig(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, betaContent1,
                null, null, null, null, null, betaIps1);
        Thread.sleep(500);

        // Verify initial beta
        ConfigGrayInfo query1 = maintainerService.queryBeta(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(query1, "Initial beta query should not be null");
        assertEquals(betaContent1, query1.getContent(),
                "Initial beta config should contain first content");
        assertTrue(query1.getGrayRule().contains(betaIps1),
                "Initial beta config should contain first IPs");

        // Update beta config with new content and IPs
        maintainerService.publishBetaConfig(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, betaContent2,
                null, null, null, null, null, betaIps2);
        Thread.sleep(500);

        // Verify updated beta
        ConfigGrayInfo query2 = maintainerService.queryBeta(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(query2, "Updated beta query should not be null");
        assertEquals(betaContent2, query2.getContent(),
                "Updated beta config should contain new content");
        assertNotEquals(betaContent1, query2.getContent(),
                "Updated beta config should NOT contain old content");

        // Cleanup
        maintainerService.stopBeta(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
    }
}
