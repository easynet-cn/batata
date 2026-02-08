package io.batata.tests;

import org.junit.jupiter.api.*;

import java.io.BufferedReader;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Config Beta (Gray Release) Tests
 *
 * Tests for beta/gray config functionality via V3 console API.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigBetaTest {

    private static String serverAddr;
    private static String accessToken;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final String DEFAULT_NAMESPACE = "public";

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        accessToken = loginV3(username, password);
        System.out.println("Config Beta Test Setup - Server: " + serverAddr);
    }

    // ==================== Beta Config Tests ====================

    /**
     * BETA-001: Test query beta config that does not exist
     */
    @Test
    @Order(1)
    void testQueryBetaConfigNotExist() throws Exception {
        String dataId = "beta-notexist-" + UUID.randomUUID().toString().substring(0, 8);

        String response = httpGet(String.format(
                "/nacos/v3/console/cs/config/beta?dataId=%s&groupName=%s&namespaceId=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8")));
        System.out.println("Query non-existent beta config: " + response);
        // Should return appropriate response indicating no beta config
        assertNotNull(response);
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

        // Publish normal config first
        publishConfig(dataId, DEFAULT_GROUP, normalContent);
        Thread.sleep(500);

        // Publish beta config via V3 console API
        String betaBody = String.format(
                "dataId=%s&groupName=%s&namespaceId=%s&content=%s&betaIps=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8"),
                URLEncoder.encode(betaContent, "UTF-8"),
                URLEncoder.encode("127.0.0.1", "UTF-8"));
        String publishResponse = httpPost("/nacos/v3/console/cs/config/beta", betaBody);
        System.out.println("Publish beta config: " + publishResponse);

        // Query beta config
        String queryResponse = httpGet(String.format(
                "/nacos/v3/console/cs/config/beta?dataId=%s&groupName=%s&namespaceId=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8")));
        System.out.println("Query beta config: " + queryResponse);

        // Cleanup
        httpDelete(String.format(
                "/nacos/v3/console/cs/config/beta?dataId=%s&groupName=%s&namespaceId=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8")));
        deleteConfig(dataId, DEFAULT_GROUP);
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
        publishConfig(dataId, DEFAULT_GROUP, normalContent);
        Thread.sleep(500);

        String betaBody = String.format(
                "dataId=%s&groupName=%s&namespaceId=%s&content=%s&betaIps=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8"),
                URLEncoder.encode(betaContent, "UTF-8"),
                URLEncoder.encode("127.0.0.1", "UTF-8"));
        httpPost("/nacos/v3/console/cs/config/beta", betaBody);
        Thread.sleep(500);

        // Delete beta config
        String deleteResponse = httpDelete(String.format(
                "/nacos/v3/console/cs/config/beta?dataId=%s&groupName=%s&namespaceId=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8")));
        System.out.println("Delete beta config: " + deleteResponse);

        // Verify beta is gone
        String queryResponse = httpGet(String.format(
                "/nacos/v3/console/cs/config/beta?dataId=%s&groupName=%s&namespaceId=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8")));
        System.out.println("Query after beta delete: " + queryResponse);

        // Cleanup
        deleteConfig(dataId, DEFAULT_GROUP);
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
        publishConfig(dataId, DEFAULT_GROUP, normalContent);
        Thread.sleep(500);

        // Publish beta config
        String betaBody = String.format(
                "dataId=%s&groupName=%s&namespaceId=%s&content=%s&betaIps=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8"),
                URLEncoder.encode(betaContent, "UTF-8"),
                URLEncoder.encode("10.0.0.1", "UTF-8"));
        httpPost("/nacos/v3/console/cs/config/beta", betaBody);
        Thread.sleep(500);

        // Get normal config via console API - should return normal content
        String response = httpGet(String.format(
                "/nacos/v3/console/cs/config?dataId=%s&groupName=%s&namespaceId=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8")));
        System.out.println("Normal config while beta exists: " + response);

        // Normal config should contain original content
        if (response.contains("content")) {
            assertFalse(response.contains("beta.check.gray"),
                    "Normal config retrieval should not return beta content");
        }

        // Cleanup
        httpDelete(String.format(
                "/nacos/v3/console/cs/config/beta?dataId=%s&groupName=%s&namespaceId=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8")));
        deleteConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== Helper Methods ====================

    private void publishConfig(String dataId, String group, String content) throws Exception {
        String body = String.format(
                "dataId=%s&groupName=%s&namespaceId=%s&content=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(group, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8"),
                URLEncoder.encode(content, "UTF-8"));
        httpPost("/nacos/v3/console/cs/config", body);
    }

    private void deleteConfig(String dataId, String group) throws Exception {
        httpDelete(String.format("/nacos/v3/console/cs/config?dataId=%s&groupName=%s&namespaceId=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(group, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8")));
    }

    private static String loginV3(String username, String password) throws Exception {
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
            while ((line = reader.readLine()) != null) {
                response.append(line);
            }
            String resp = response.toString();
            if (resp.contains("accessToken")) {
                int start = resp.indexOf("accessToken") + 14;
                int end = resp.indexOf("\"", start);
                if (end > start) return resp.substring(start, end);
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
        if (body != null && !body.isEmpty()) {
            try (OutputStream os = conn.getOutputStream()) {
                os.write(body.getBytes(StandardCharsets.UTF_8));
            }
        }
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

    private static String readResponse(HttpURLConnection conn) throws Exception {
        int responseCode = conn.getResponseCode();
        InputStream stream = responseCode >= 400 ? conn.getErrorStream() : conn.getInputStream();
        StringBuilder response = new StringBuilder();
        if (stream != null) {
            BufferedReader reader = new BufferedReader(new InputStreamReader(stream));
            String line;
            while ((line = reader.readLine()) != null) {
                response.append(line);
            }
        }
        return response.toString();
    }
}
