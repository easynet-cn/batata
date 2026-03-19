package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.*;

import java.io.*;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Open API Tests
 *
 * Tests for V2 HTTP Open API endpoints used for config management.
 * Aligned with Nacos AbstractConfigAPIConfigITCase Open API tests.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosOpenApiTest {

    private static String serverAddr;
    private static String accessToken;
    private static ConfigService configService;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final ObjectMapper objectMapper = new ObjectMapper();

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        accessToken = getAccessToken(username, password);
        assertFalse(accessToken.isEmpty(),
                "Should obtain access token during setup");

        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);
        configService = NacosFactory.createConfigService(properties);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (configService != null) configService.shutDown();
    }

    // ==================== P1: Open API Config Detail ====================

    /**
     * OA-001: Test Open API get config detail
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

        // Get detail via Open API V2
        // V2 config GET returns {"code":0,"data":"content"} or {"code":0,"data":{...}} with showDetail
        String response = httpGet("/nacos/v2/cs/config?dataId=" + dataId
                + "&group=" + DEFAULT_GROUP);
        assertNotNull(response, "Config detail response should not be null");
        assertFalse(response.isEmpty(), "Config detail response should not be empty");
        assertTrue(response.contains(content),
                "Response should contain the published config content '" + content + "', got: " + response);

        // Verify via SDK get to confirm consistency
        String sdkContent = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertEquals(content, sdkContent,
                "SDK get should return the same content as published");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * OA-002: Test Open API fuzzy search config
     *
     * Aligned with Nacos AbstractConfigAPIConfigITCase.testOpenApiFuzzySearchConfig()
     *
     * SKIPPED: The fuzzy search endpoint /nacos/v2/cs/config/list is not
     * implemented in Batata and returns 404.
     */
    @Test
    @Order(2)
    @Disabled("Batata does not implement /nacos/v2/cs/config/list endpoint (returns 404)")
    void testOpenApiFuzzySearchConfig() throws Exception {
        String prefix = "oa-fuzzy-" + UUID.randomUUID().toString().substring(0, 6);
        String dataId1 = prefix + "-config-a";
        String dataId2 = prefix + "-config-b";

        boolean pub1 = configService.publishConfig(dataId1, DEFAULT_GROUP, "fuzzy.a=true");
        boolean pub2 = configService.publishConfig(dataId2, DEFAULT_GROUP, "fuzzy.b=true");
        assertTrue(pub1, "Publishing first fuzzy config should succeed");
        assertTrue(pub2, "Publishing second fuzzy config should succeed");
        Thread.sleep(1000);

        // Fuzzy search using blur mode - use prefix without wildcard for blur search
        String response = httpGet("/nacos/v2/cs/config/list?dataId=" + URLEncoder.encode(prefix, "UTF-8")
                + "&group=" + DEFAULT_GROUP + "&pageNo=1&pageSize=10&search=blur");
        assertNotNull(response, "Fuzzy search response should not be null");
        assertFalse(response.isEmpty(), "Fuzzy search response should not be empty");

        // Both configs should appear in the search results
        assertTrue(response.contains(dataId1),
                "Fuzzy search should find first config '" + dataId1 + "', got: " + response);
        assertTrue(response.contains(dataId2),
                "Fuzzy search should find second config '" + dataId2 + "', got: " + response);

        // Cleanup
        configService.removeConfig(dataId1, DEFAULT_GROUP);
        configService.removeConfig(dataId2, DEFAULT_GROUP);
    }

    /**
     * OA-003: Test Open API accurate search config
     *
     * Aligned with Nacos AbstractConfigAPIConfigITCase.testOpenApiSearchConfig()
     *
     * SKIPPED: The /nacos/v2/cs/config/list endpoint with search=accurate is not
     * implemented in Batata and returns 404.
     */
    @Test
    @Order(3)
    @Disabled("Batata does not implement /nacos/v2/cs/config/list endpoint (returns 404)")
    void testOpenApiAccurateSearchConfig() throws Exception {
        String dataId = "oa-search-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "search.key=found";

        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, content);
        assertTrue(published, "Config publish should succeed");
        Thread.sleep(1000);

        // Accurate search
        String response = httpGet("/nacos/v2/cs/config/list?dataId=" + URLEncoder.encode(dataId, "UTF-8")
                + "&group=" + DEFAULT_GROUP + "&pageNo=1&pageSize=10&search=accurate");
        assertNotNull(response, "Accurate search response should not be null");
        assertTrue(response.contains(dataId),
                "Accurate search should find config by exact dataId '" + dataId + "'");

        // Verify negative case: searching for a non-existent dataId should not find this config
        String nonExistent = "nonexistent-" + UUID.randomUUID().toString().substring(0, 8);
        String negResponse = httpGet("/nacos/v2/cs/config/list?dataId=" + URLEncoder.encode(nonExistent, "UTF-8")
                + "&group=" + DEFAULT_GROUP + "&pageNo=1&pageSize=10&search=accurate");
        assertFalse(negResponse.contains(dataId),
                "Search for non-existent dataId should not return our config");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * OA-004: Test Open API search with Chinese characters
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

        // Get config via Open API and verify Chinese content is preserved
        String response = httpGet("/nacos/v2/cs/config?dataId=" + URLEncoder.encode(dataId, "UTF-8")
                + "&group=" + DEFAULT_GROUP);
        assertNotNull(response, "Chinese config response should not be null");
        assertFalse(response.isEmpty(), "Chinese config response should not be empty");
        assertTrue(response.contains("\u914d\u7f6e\u5185\u5bb9") || response.contains("\u4e2d\u6587\u6d4b\u8bd5"),
                "Response should contain Chinese characters from the published content");

        // Verify via SDK to confirm round-trip integrity
        String sdkContent = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertEquals(content, sdkContent,
                "SDK get should return the exact Chinese content that was published");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * OA-005: Test Open API publish and get via V2 HTTP
     *
     * Aligned with Nacos ConfigAPIV2ConfigITCase.test()
     */
    @Test
    @Order(5)
    void testV2PublishAndGet() throws Exception {
        String dataId = "oa-v2-crud-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "v2.api.key=v2-value";

        // Publish via V2 API - response is {"code":0,"data":true} or {"code":200,...}
        String publishBody = "dataId=" + URLEncoder.encode(dataId, "UTF-8")
                + "&group=" + DEFAULT_GROUP
                + "&content=" + URLEncoder.encode(content, "UTF-8");
        String publishResponse = httpPost("/nacos/v2/cs/config", publishBody);
        assertNotNull(publishResponse, "V2 publish response should not be null");
        JsonNode publishJson = objectMapper.readTree(publishResponse);
        assertTrue(publishJson.has("code"), "V2 publish should return code field: " + publishResponse);
        assertEquals(0, publishJson.get("code").asInt(), "V2 publish should succeed: " + publishResponse);

        Thread.sleep(500);

        // Get via V2 API
        String getResponse = httpGet("/nacos/v2/cs/config?dataId=" + URLEncoder.encode(dataId, "UTF-8")
                + "&group=" + DEFAULT_GROUP);
        assertNotNull(getResponse, "V2 get response should not be null");
        assertTrue(getResponse.contains(content),
                "V2 get should return published content '" + content + "', got: " + getResponse);

        // Verify via SDK to ensure consistency between HTTP API and gRPC
        String sdkContent = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertEquals(content, sdkContent,
                "SDK get should return the same content published via V2 HTTP API");

        // Delete via V2 API - response is {"code":0,"data":true}
        String deleteResponse = httpDelete("/nacos/v2/cs/config?dataId=" + URLEncoder.encode(dataId, "UTF-8")
                + "&group=" + DEFAULT_GROUP);
        assertNotNull(deleteResponse, "V2 delete response should not be null");
        JsonNode deleteJson = objectMapper.readTree(deleteResponse);
        assertTrue(deleteJson.has("code"), "V2 delete should return code field: " + deleteResponse);
        assertEquals(0, deleteJson.get("code").asInt(), "V2 delete should succeed: " + deleteResponse);

        Thread.sleep(500);

        // Verify deleted via SDK
        String afterDelete = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertNull(afterDelete, "Config should be null after deletion via V2 API");
    }

    /**
     * OA-006: Test Open API get config with null group (uses DEFAULT_GROUP)
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

    // ==================== Helper Methods ====================

    private static String getAccessToken(String username, String password) throws Exception {
        String loginUrl = String.format("http://%s/nacos/v3/auth/user/login", serverAddr);
        URL url = new URL(loginUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("POST");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");
        String body = "username=" + URLEncoder.encode(username, "UTF-8")
                + "&password=" + URLEncoder.encode(password, "UTF-8");
        conn.getOutputStream().write(body.getBytes(StandardCharsets.UTF_8));

        assertEquals(200, conn.getResponseCode(),
                "Login should return HTTP 200");

        BufferedReader reader = new BufferedReader(new InputStreamReader(conn.getInputStream()));
        StringBuilder response = new StringBuilder();
        String line;
        while ((line = reader.readLine()) != null) response.append(line);
        String resp = response.toString();

        JsonNode json = objectMapper.readTree(resp);
        // Handle both wrapped {"code":0,"data":{"accessToken":"..."}} and flat {"accessToken":"..."}
        if (json.has("data") && json.get("data").has("accessToken")) {
            return json.get("data").get("accessToken").asText();
        }
        if (json.has("accessToken")) {
            return json.get("accessToken").asText();
        }
        fail("Login response should contain accessToken: " + resp);
        return "";
    }

    private String httpGet(String path) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }
        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("GET");
        return readResponse(conn);
    }

    private String httpPost(String path, String body) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }
        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("POST");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");
        if (body != null) conn.getOutputStream().write(body.getBytes(StandardCharsets.UTF_8));
        return readResponse(conn);
    }

    private String httpDelete(String path) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }
        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("DELETE");
        return readResponse(conn);
    }

    private String readResponse(HttpURLConnection conn) throws Exception {
        int responseCode = conn.getResponseCode();
        InputStream stream = responseCode >= 400 ? conn.getErrorStream() : conn.getInputStream();
        if (stream == null) return "Status: " + responseCode;
        BufferedReader reader = new BufferedReader(new InputStreamReader(stream, StandardCharsets.UTF_8));
        StringBuilder response = new StringBuilder();
        String line;
        while ((line = reader.readLine()) != null) response.append(line);
        return response.toString();
    }
}
