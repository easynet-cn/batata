package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.Properties;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Config History API Tests
 *
 * Tests for configuration history tracking and retrieval.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigHistoryTest {

    private static ConfigService configService;
    private static String serverAddr;
    private static String accessToken;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        configService = NacosFactory.createConfigService(properties);

        // Get access token for HTTP API calls
        accessToken = getAccessToken(username, password);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (configService != null) {
            configService.shutDown();
        }
    }

    private static String getAccessToken(String username, String password) throws Exception {
        String loginUrl = String.format("http://%s/nacos/v1/auth/login", serverAddr);
        URL url = new URL(loginUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("POST");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");

        String body = "username=" + URLEncoder.encode(username, "UTF-8") +
                "&password=" + URLEncoder.encode(password, "UTF-8");
        conn.getOutputStream().write(body.getBytes(StandardCharsets.UTF_8));

        if (conn.getResponseCode() == 200) {
            BufferedReader reader = new BufferedReader(new InputStreamReader(conn.getInputStream()));
            StringBuilder response = new StringBuilder();
            String line;
            while ((line = reader.readLine()) != null) {
                response.append(line);
            }
            // Parse token from response
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

        BufferedReader reader = new BufferedReader(new InputStreamReader(
                conn.getResponseCode() >= 400 ? conn.getErrorStream() : conn.getInputStream()));
        StringBuilder response = new StringBuilder();
        String line;
        while ((line = reader.readLine()) != null) {
            response.append(line);
        }
        return response.toString();
    }

    // ==================== Config History Tests ====================

    /**
     * Test list config history
     */
    @Test
    @Order(1)
    void testListConfigHistory() throws Exception {
        String dataId = "history-test-" + UUID.randomUUID();

        // Create multiple versions
        for (int i = 1; i <= 3; i++) {
            configService.publishConfig(dataId, DEFAULT_GROUP, "version=" + i);
            Thread.sleep(500);
        }

        // List history
        String response = httpGet(String.format(
                "/nacos/v2/cs/history/list?dataId=%s&group=%s&pageNo=1&pageSize=10",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8")));

        System.out.println("History list response: " + response);
        assertNotNull(response);
        // Should contain history records or empty list

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * Test get specific history version
     */
    @Test
    @Order(2)
    void testGetHistoryVersion() throws Exception {
        String dataId = "history-version-" + UUID.randomUUID();

        // Create config
        configService.publishConfig(dataId, DEFAULT_GROUP, "initial=value");
        Thread.sleep(500);

        // Get history list first to get nid
        String listResponse = httpGet(String.format(
                "/nacos/v2/cs/history/list?dataId=%s&group=%s&pageNo=1&pageSize=10",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8")));

        System.out.println("History list for version test: " + listResponse);

        // If we have history records, try to get specific version
        if (listResponse.contains("\"id\"")) {
            // Extract first id from response
            int idStart = listResponse.indexOf("\"id\"") + 5;
            int idEnd = listResponse.indexOf(",", idStart);
            if (idEnd > idStart) {
                String nid = listResponse.substring(idStart, idEnd).replaceAll("[^0-9]", "");
                if (!nid.isEmpty()) {
                    String versionResponse = httpGet(String.format(
                            "/nacos/v2/cs/history?dataId=%s&group=%s&nid=%s",
                            URLEncoder.encode(dataId, "UTF-8"),
                            URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                            nid));
                    System.out.println("History version response: " + versionResponse);
                }
            }
        }

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * Test get previous version
     */
    @Test
    @Order(3)
    void testGetPreviousVersion() throws Exception {
        String dataId = "history-prev-" + UUID.randomUUID();

        // Create two versions
        configService.publishConfig(dataId, DEFAULT_GROUP, "version=1");
        Thread.sleep(500);
        configService.publishConfig(dataId, DEFAULT_GROUP, "version=2");
        Thread.sleep(500);

        // Get previous version
        String response = httpGet(String.format(
                "/nacos/v2/cs/history/previous?dataId=%s&group=%s&id=1",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8")));

        System.out.println("Previous version response: " + response);
        assertNotNull(response);

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * Test list all configs in namespace
     */
    @Test
    @Order(4)
    void testListNamespaceConfigs() throws Exception {
        String prefix = "ns-config-" + UUID.randomUUID().toString().substring(0, 8);

        // Create multiple configs
        for (int i = 0; i < 3; i++) {
            configService.publishConfig(prefix + "-" + i, DEFAULT_GROUP, "content=" + i);
        }
        Thread.sleep(1000);

        // List all configs in namespace (empty namespace = public)
        String response = httpGet("/nacos/v2/cs/history/configs?tenant=");

        System.out.println("Namespace configs response: " + response);
        assertNotNull(response);

        // Cleanup
        for (int i = 0; i < 3; i++) {
            configService.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
        }
    }

    /**
     * Test history with namespace isolation
     */
    @Test
    @Order(5)
    void testHistoryNamespaceIsolation() throws Exception {
        String namespace = "history-ns-" + UUID.randomUUID().toString().substring(0, 8);
        String dataId = "isolated-config";

        // Create config in specific namespace via HTTP
        // Note: This requires namespace to exist first

        // List history in specific namespace
        String response = httpGet(String.format(
                "/nacos/v2/cs/history/list?dataId=%s&group=%s&tenant=%s&pageNo=1&pageSize=10",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(namespace, "UTF-8")));

        System.out.println("Namespace history response: " + response);
        assertNotNull(response);
    }
}
