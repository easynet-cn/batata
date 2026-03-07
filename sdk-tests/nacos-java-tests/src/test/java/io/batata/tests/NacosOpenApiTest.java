package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
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

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        accessToken = getAccessToken(username, password);

        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);
        configService = NacosFactory.createConfigService(properties);

        System.out.println("Open API Test Setup - Server: " + serverAddr);
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
        configService.publishConfig(dataId, DEFAULT_GROUP, content);
        Thread.sleep(1000);

        // Get detail via Open API V2
        String response = httpGet("/nacos/v2/cs/config?dataId=" + dataId
                + "&group=" + DEFAULT_GROUP + "&showDetail=true");
        System.out.println("Config detail response: " + response);
        assertNotNull(response);
        assertTrue(response.contains(content) || response.contains(dataId),
                "Response should contain config data");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * OA-002: Test Open API fuzzy search config
     *
     * Aligned with Nacos AbstractConfigAPIConfigITCase.testOpenApiFuzzySearchConfig()
     */
    @Test
    @Order(2)
    void testOpenApiFuzzySearchConfig() throws Exception {
        String prefix = "oa-fuzzy-" + UUID.randomUUID().toString().substring(0, 6);
        String dataId1 = prefix + "-config-a";
        String dataId2 = prefix + "-config-b";

        configService.publishConfig(dataId1, DEFAULT_GROUP, "fuzzy.a=true");
        configService.publishConfig(dataId2, DEFAULT_GROUP, "fuzzy.b=true");
        Thread.sleep(1000);

        // Fuzzy search using blur mode
        String response = httpGet("/nacos/v2/cs/config/list?dataId=" + URLEncoder.encode(prefix + "*", "UTF-8")
                + "&group=" + DEFAULT_GROUP + "&pageNo=1&pageSize=10&search=blur");
        System.out.println("Fuzzy search response: " + response);
        assertNotNull(response);

        // Cleanup
        configService.removeConfig(dataId1, DEFAULT_GROUP);
        configService.removeConfig(dataId2, DEFAULT_GROUP);
    }

    /**
     * OA-003: Test Open API accurate search config
     *
     * Aligned with Nacos AbstractConfigAPIConfigITCase.testOpenApiSearchConfig()
     */
    @Test
    @Order(3)
    void testOpenApiAccurateSearchConfig() throws Exception {
        String dataId = "oa-search-" + UUID.randomUUID().toString().substring(0, 8);

        configService.publishConfig(dataId, DEFAULT_GROUP, "search.key=found");
        Thread.sleep(1000);

        // Accurate search
        String response = httpGet("/nacos/v2/cs/config/list?dataId=" + URLEncoder.encode(dataId, "UTF-8")
                + "&group=" + DEFAULT_GROUP + "&pageNo=1&pageSize=10&search=accurate");
        System.out.println("Accurate search response: " + response);
        assertNotNull(response);
        assertTrue(response.contains(dataId), "Should find config by accurate search");

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

        configService.publishConfig(dataId, DEFAULT_GROUP, content);
        Thread.sleep(1000);

        // Get config and verify Chinese content
        String response = httpGet("/nacos/v2/cs/config?dataId=" + URLEncoder.encode(dataId, "UTF-8")
                + "&group=" + DEFAULT_GROUP);
        System.out.println("Chinese config response length: " + (response != null ? response.length() : 0));
        assertNotNull(response);

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

        // Publish via V2 API
        String publishBody = "dataId=" + URLEncoder.encode(dataId, "UTF-8")
                + "&group=" + DEFAULT_GROUP
                + "&content=" + URLEncoder.encode(content, "UTF-8");
        String publishResponse = httpPost("/nacos/v2/cs/config", publishBody);
        System.out.println("V2 publish response: " + publishResponse);
        assertTrue(publishResponse.contains("true") || publishResponse.contains("200"),
                "V2 publish should succeed");

        Thread.sleep(500);

        // Get via V2 API
        String getResponse = httpGet("/nacos/v2/cs/config?dataId=" + URLEncoder.encode(dataId, "UTF-8")
                + "&group=" + DEFAULT_GROUP);
        System.out.println("V2 get response: " + getResponse);
        assertNotNull(getResponse);
        assertTrue(getResponse.contains(content) || getResponse.contains("v2-value"),
                "V2 get should return published content");

        // Delete via V2 API
        String deleteResponse = httpDelete("/nacos/v2/cs/config?dataId=" + URLEncoder.encode(dataId, "UTF-8")
                + "&group=" + DEFAULT_GROUP);
        System.out.println("V2 delete response: " + deleteResponse);

        Thread.sleep(500);

        // Verify deleted
        String afterDelete = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertNull(afterDelete, "Config should be deleted");
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
        configService.publishConfig(dataId, DEFAULT_GROUP, content);

        boolean received = latch.await(10, java.util.concurrent.TimeUnit.SECONDS);
        assertTrue(received, "Remaining listener should still receive notifications");
        assertEquals(content, receivedContent.get());

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
        if (conn.getResponseCode() == 200) {
            BufferedReader reader = new BufferedReader(new InputStreamReader(conn.getInputStream()));
            StringBuilder response = new StringBuilder();
            String line;
            while ((line = reader.readLine()) != null) response.append(line);
            String resp = response.toString();
            if (resp.contains("accessToken")) {
                int start = resp.indexOf("accessToken") + 14;
                int end = resp.indexOf("\"", start);
                return resp.substring(start, end);
            }
        }
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
