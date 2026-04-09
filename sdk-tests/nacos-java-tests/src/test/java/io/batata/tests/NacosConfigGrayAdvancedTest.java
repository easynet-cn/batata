package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.maintainer.client.NacosMaintainerFactory;
import com.alibaba.nacos.maintainer.client.config.ConfigMaintainerService;
import org.junit.jupiter.api.*;

import java.io.IOException;
import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.util.Properties;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Config Gray Release Advanced Tests
 *
 * Tests for Batata-extended gray release features:
 * - Percentage-based gray release
 * - IP-range (CIDR) gray release
 *
 * These are Batata extensions beyond standard Nacos (which only supports beta/tag).
 * Uses HTTP API directly since the Nacos Java SDK doesn't have methods for these.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigGrayAdvancedTest {

    private static ConfigService configService;
    private static ConfigMaintainerService maintainerService;
    private static HttpClient httpClient;
    private static String serverAddr;
    private static String accessToken;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final String DEFAULT_NAMESPACE = "";

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);

        configService = NacosFactory.createConfigService(properties);
        maintainerService = NacosMaintainerFactory.createConfigMaintainerService(properties);
        httpClient = HttpClient.newHttpClient();

        // Get access token for admin API calls
        accessToken = login(serverAddr, username, password);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (configService != null) configService.shutDown();
    }

    /**
     * Login to get access token
     */
    private static String login(String addr, String username, String password) throws IOException, InterruptedException {
        String body = "username=" + URLEncoder.encode(username, StandardCharsets.UTF_8)
                + "&password=" + URLEncoder.encode(password, StandardCharsets.UTF_8);

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://" + addr + "/nacos/v3/auth/user/login"))
                .header("Content-Type", "application/x-www-form-urlencoded")
                .POST(HttpRequest.BodyPublishers.ofString(body))
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() == 200) {
            // Extract accessToken from JSON response
            String responseBody = response.body();
            int idx = responseBody.indexOf("\"accessToken\"");
            if (idx >= 0) {
                int start = responseBody.indexOf("\"", idx + 14) + 1;
                int end = responseBody.indexOf("\"", start);
                return responseBody.substring(start, end);
            }
        }
        return "";
    }

    /**
     * Publish gray config via admin HTTP API
     */
    private HttpResponse<String> publishGrayConfig(String dataId, String content, String extraParams) throws IOException, InterruptedException {
        String body = "dataId=" + URLEncoder.encode(dataId, StandardCharsets.UTF_8)
                + "&groupName=" + URLEncoder.encode(DEFAULT_GROUP, StandardCharsets.UTF_8)
                + "&content=" + URLEncoder.encode(content, StandardCharsets.UTF_8)
                + extraParams;

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create("http://" + serverAddr + "/nacos/v3/admin/cs/config/gray?accessToken=" + accessToken))
                .header("Content-Type", "application/x-www-form-urlencoded")
                .POST(HttpRequest.BodyPublishers.ofString(body))
                .build();

        return httpClient.send(request, HttpResponse.BodyHandlers.ofString());
    }

    // ==================== Percentage Gray Release Tests ====================

    /**
     * GRAY-ADV-001: Test percentage-based gray config publish
     */
    @Test
    @Order(1)
    void testPercentageGrayPublish() throws Exception {
        String dataId = "gray-pct-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "gray.pct.normal=value";
        String grayContent = "gray.pct.gray=value";

        // Publish normal config first
        boolean published = maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        assertTrue(published, "Normal config publish should succeed");
        Thread.sleep(500);

        // Publish percentage gray config (50%)
        HttpResponse<String> response = publishGrayConfig(dataId, grayContent, "&percentage=50");
        assertEquals(200, response.statusCode(),
                "Percentage gray publish should return 200. Body: " + response.body());
        assertTrue(response.body().contains("true") || response.body().contains("200"),
                "Response should indicate success. Body: " + response.body());

        // Cleanup
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
    }

    /**
     * GRAY-ADV-002: Test percentage gray with boundary values (0 and 100)
     */
    @Test
    @Order(2)
    void testPercentageGrayBoundaryValues() throws Exception {
        String dataId0 = "gray-pct0-" + UUID.randomUUID().toString().substring(0, 8);
        String dataId100 = "gray-pct100-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "normal=value";
        String grayContent = "gray=value";

        // Setup normal configs
        maintainerService.publishConfig(dataId0, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        maintainerService.publishConfig(dataId100, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        Thread.sleep(500);

        // 0% — no clients should get gray config
        HttpResponse<String> resp0 = publishGrayConfig(dataId0, grayContent, "&percentage=0");
        assertEquals(200, resp0.statusCode(),
                "0% gray publish should succeed. Body: " + resp0.body());

        // 100% — all clients should get gray config
        HttpResponse<String> resp100 = publishGrayConfig(dataId100, grayContent, "&percentage=100");
        assertEquals(200, resp100.statusCode(),
                "100% gray publish should succeed. Body: " + resp100.body());

        // Cleanup
        maintainerService.deleteConfig(dataId0, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        maintainerService.deleteConfig(dataId100, DEFAULT_GROUP, DEFAULT_NAMESPACE);
    }

    /**
     * GRAY-ADV-003: Test percentage gray with invalid value (>100)
     */
    @Test
    @Order(3)
    void testPercentageGrayInvalidValue() throws Exception {
        String dataId = "gray-pct-invalid-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "normal=value";
        String grayContent = "gray=value";

        maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        Thread.sleep(500);

        // 150% — should be rejected
        HttpResponse<String> response = publishGrayConfig(dataId, grayContent, "&percentage=150");
        assertTrue(response.statusCode() == 400 || response.body().contains("error")
                        || response.body().contains("must be between"),
                "Percentage > 100 should be rejected. Status: " + response.statusCode()
                        + " Body: " + response.body());

        // Cleanup
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
    }

    // ==================== IP Range Gray Release Tests ====================

    /**
     * GRAY-ADV-004: Test IP-range (CIDR) gray config publish
     */
    @Test
    @Order(4)
    void testIpRangeGrayPublish() throws Exception {
        String dataId = "gray-cidr-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "gray.cidr.normal=value";
        String grayContent = "gray.cidr.gray=value";

        // Publish normal config first
        boolean published = maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        assertTrue(published, "Normal config publish should succeed");
        Thread.sleep(500);

        // Publish IP range gray config (local network CIDR)
        String cidrRange = URLEncoder.encode("10.0.0.0/8,172.16.0.0/12,192.168.0.0/16", StandardCharsets.UTF_8);
        HttpResponse<String> response = publishGrayConfig(dataId, grayContent, "&ipRange=" + cidrRange);
        assertEquals(200, response.statusCode(),
                "IP-range gray publish should return 200. Body: " + response.body());
        assertTrue(response.body().contains("true") || response.body().contains("200"),
                "Response should indicate success. Body: " + response.body());

        // Cleanup
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
    }

    /**
     * GRAY-ADV-005: Test IP-range gray with single host CIDR
     */
    @Test
    @Order(5)
    void testIpRangeGraySingleHost() throws Exception {
        String dataId = "gray-cidr-single-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "normal=value";
        String grayContent = "gray=value";

        maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        Thread.sleep(500);

        // Single host CIDR: 127.0.0.1/32
        String cidr = URLEncoder.encode("127.0.0.1/32", StandardCharsets.UTF_8);
        HttpResponse<String> response = publishGrayConfig(dataId, grayContent, "&ipRange=" + cidr);
        assertEquals(200, response.statusCode(),
                "Single-host CIDR gray publish should succeed. Body: " + response.body());

        // Cleanup
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
    }

    /**
     * GRAY-ADV-006: Test that neither parameter provided returns error
     */
    @Test
    @Order(6)
    void testGrayPublishNoTypeParameter() throws Exception {
        String dataId = "gray-notype-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "normal=value";
        String grayContent = "gray=value";

        maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, normalContent);
        Thread.sleep(500);

        // No betaIps, tag, percentage, or ipRange — should be rejected
        HttpResponse<String> response = publishGrayConfig(dataId, grayContent, "");
        assertTrue(response.statusCode() == 400 || response.body().contains("must be provided"),
                "Gray publish without type should be rejected. Status: " + response.statusCode()
                        + " Body: " + response.body());

        // Cleanup
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
    }

    /**
     * GRAY-ADV-007: Test gray config retrieved by SDK matches for matching client IP
     *
     * When client IP matches the gray rule, SDK should get the gray content.
     * Since the test client connects from 127.0.0.1, a beta rule with 127.0.0.1 should match.
     */
    @Test
    @Order(7)
    void testGrayConfigRetrievedByMatchingClient() throws Exception {
        String dataId = "gray-match-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "gray.match.normal=true";
        String grayContent = "gray.match.gray=true";

        // Publish normal config
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, normalContent);
        assertTrue(published, "Normal config publish should succeed");
        Thread.sleep(1000);

        // Publish beta config targeting 127.0.0.1 (test client's IP)
        boolean betaPublished = maintainerService.publishBetaConfig(
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, grayContent,
                null, null, null, null, null, "127.0.0.1");
        assertTrue(betaPublished, "Beta config targeting localhost should succeed");
        Thread.sleep(1000);

        // SDK get should return gray content (since client connects from 127.0.0.1)
        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertNotNull(retrieved, "Config should not be null");
        assertEquals(grayContent, retrieved,
                "SDK should receive gray content when client IP matches beta rule");

        // Cleanup
        maintainerService.stopBeta(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        Thread.sleep(500);
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
    }
}
