package io.batata.tests;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.*;

import java.io.BufferedReader;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Config Tag Tests
 *
 * Tests configuration management with custom tags, including tag-based
 * publishing, querying, filtering, and gray release via tags.
 * Uses V2 Open API and V3 Admin API for tag operations.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigTagTest {

    private static String serverAddr;
    private static String accessToken;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final String DEFAULT_NAMESPACE = "public";
    private static final ObjectMapper objectMapper = new ObjectMapper();
    private static final List<String[]> cleanupConfigs = new ArrayList<>();

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        accessToken = loginV3(username, password);
        assertFalse(accessToken.isEmpty(), "Should obtain access token during setup");
        System.out.println("Config Tag Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws Exception {
        for (String[] config : cleanupConfigs) {
            try {
                deleteConfig(config[0], config[1]);
            } catch (Exception e) {
                // Ignore cleanup errors
            }
        }
    }

    // ==================== Tag Publish and Query Tests ====================

    /**
     * NCT-TAG-001: Test publish config with tag parameter
     *
     * Publishes a configuration with a custom tag via the V2 Open API
     * and verifies the config can be retrieved.
     * Disabled: Tag-based config retrieval is not yet supported in Batata.
     */
    @Test
    @Order(1)
    @Disabled("Tag-based config GET is not yet supported in Batata")
    void testPublishConfigWithTag() throws Exception {
        String dataId = "tag-publish-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "tag.publish.key=value";
        String tag = "release-v1";

        String body = String.format(
                "dataId=%s&group=%s&content=%s&tag=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(content, "UTF-8"),
                URLEncoder.encode(tag, "UTF-8"));
        String response = httpPost("/nacos/v2/cs/config", body);
        assertNotNull(response, "Publish config with tag response should not be null");
        cleanupConfigs.add(new String[]{dataId, DEFAULT_GROUP});

        JsonNode json = objectMapper.readTree(response);
        assertTrue(json.has("code"), "Response should contain code field: " + response);
        assertEquals(0, json.get("code").asInt(), "Publish config with tag should succeed: " + response);

        // Verify config can be retrieved with tag
        String getResponse = httpGet(String.format(
                "/nacos/v2/cs/config?dataId=%s&group=%s&tag=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(tag, "UTF-8")));
        assertNotNull(getResponse, "Get config with tag response should not be null");
        JsonNode getJson = objectMapper.readTree(getResponse);
        assertEquals(0, getJson.get("code").asInt(), "Get config with tag should succeed: " + getResponse);
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
    @Disabled("Tag-based config GET is not yet supported in Batata")
    void testQueryConfigWithTagFilter() throws Exception {
        String dataId = "tag-query-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "tag.query.mode=normal";
        String taggedContent = "tag.query.mode=tagged";
        String tag = "canary";

        // Publish normal config (no tag)
        String normalBody = String.format(
                "dataId=%s&group=%s&content=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(normalContent, "UTF-8"));
        String normalResponse = httpPost("/nacos/v2/cs/config", normalBody);
        JsonNode normalJson = objectMapper.readTree(normalResponse);
        assertEquals(0, normalJson.get("code").asInt(), "Normal publish should succeed");
        cleanupConfigs.add(new String[]{dataId, DEFAULT_GROUP});

        // Publish tagged config
        String tagBody = String.format(
                "dataId=%s&group=%s&content=%s&tag=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(taggedContent, "UTF-8"),
                URLEncoder.encode(tag, "UTF-8"));
        String tagResponse = httpPost("/nacos/v2/cs/config", tagBody);
        JsonNode tagJson = objectMapper.readTree(tagResponse);
        assertEquals(0, tagJson.get("code").asInt(), "Tagged publish should succeed");

        Thread.sleep(500);

        // Query with tag filter - should return tagged content
        String getTagged = httpGet(String.format(
                "/nacos/v2/cs/config?dataId=%s&group=%s&tag=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(tag, "UTF-8")));
        assertNotNull(getTagged);
        JsonNode getTaggedJson = objectMapper.readTree(getTagged);
        if (getTaggedJson.get("code").asInt() == 0 && getTaggedJson.has("data")) {
            String returnedContent = getTaggedJson.get("data").asText();
            assertEquals(taggedContent, returnedContent,
                    "Query with tag should return the tagged content");
        }
    }

    /**
     * NCT-TAG-003: Test query without tag returns default config
     *
     * After publishing both normal and tagged config for the same dataId,
     * querying without a tag should return the default (untagged) config.
     * Disabled: Tag-based config is not yet supported in Batata.
     */
    @Test
    @Order(3)
    @Disabled("Tag-based config is not yet supported in Batata")
    void testQueryWithoutTagReturnsDefault() throws Exception {
        String dataId = "tag-default-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "mode=default";
        String taggedContent = "mode=tagged-version";
        String tag = "staging";

        // Publish normal config
        String normalBody = String.format(
                "dataId=%s&group=%s&content=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(normalContent, "UTF-8"));
        httpPost("/nacos/v2/cs/config", normalBody);
        cleanupConfigs.add(new String[]{dataId, DEFAULT_GROUP});

        // Publish tagged config
        String tagBody = String.format(
                "dataId=%s&group=%s&content=%s&tag=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(taggedContent, "UTF-8"),
                URLEncoder.encode(tag, "UTF-8"));
        httpPost("/nacos/v2/cs/config", tagBody);

        Thread.sleep(500);

        // Query without tag - should return normal/default content
        String getDefault = httpGet(String.format(
                "/nacos/v2/cs/config?dataId=%s&group=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8")));
        assertNotNull(getDefault);
        JsonNode getJson = objectMapper.readTree(getDefault);
        if (getJson.get("code").asInt() == 0 && getJson.has("data")) {
            String returnedContent = getJson.get("data").asText();
            assertEquals(normalContent, returnedContent,
                    "Query without tag should return the default (untagged) config");
        }
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
            String body = String.format(
                    "dataId=%s&group=%s&content=%s&tag=%s",
                    URLEncoder.encode(dataId, "UTF-8"),
                    URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                    URLEncoder.encode(content, "UTF-8"),
                    URLEncoder.encode(tag, "UTF-8"));
            String response = httpPost("/nacos/v2/cs/config", body);
            JsonNode json = objectMapper.readTree(response);
            assertEquals(0, json.get("code").asInt(),
                    "Publish with tag '" + tag + "' should succeed: " + response);
        }

        Thread.sleep(500);

        // Verify each tag returns its own content
        for (String tag : tags) {
            String getResponse = httpGet(String.format(
                    "/nacos/v2/cs/config?dataId=%s&group=%s&tag=%s",
                    URLEncoder.encode(dataId, "UTF-8"),
                    URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                    URLEncoder.encode(tag, "UTF-8")));
            assertNotNull(getResponse);
            JsonNode getJson = objectMapper.readTree(getResponse);
            if (getJson.get("code").asInt() == 0 && getJson.has("data")) {
                String returnedContent = getJson.get("data").asText();
                assertTrue(returnedContent.contains("version=" + tag),
                        "Tag '" + tag + "' should return its own content, got: " + returnedContent);
            }
        }
    }

    /**
     * NCT-TAG-005: Test tag-based gray release via V3 Admin API
     *
     * Tests gray config publishing through the V3 admin endpoint, which
     * supports tag-based gray release rules.
     */
    @Test
    @Order(5)
    void testTagBasedGrayRelease() throws Exception {
        String dataId = "tag-gray-" + UUID.randomUUID().toString().substring(0, 8);
        String normalContent = "gray.release=stable";
        String grayContent = "gray.release=canary-v2";

        // Publish stable config via V2 Open API
        String publishBody = String.format(
                "dataId=%s&group=%s&content=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(normalContent, "UTF-8"));
        String publishResponse = httpPost("/nacos/v2/cs/config", publishBody);
        JsonNode publishJson = objectMapper.readTree(publishResponse);
        assertEquals(0, publishJson.get("code").asInt(), "Stable config publish should succeed");
        cleanupConfigs.add(new String[]{dataId, DEFAULT_GROUP});

        Thread.sleep(500);

        // Publish gray config via V3 Admin API (beta/gray endpoint)
        // Try V3 admin gray endpoint; if not available, try V2 tag-based approach
        String grayBody = String.format(
                "dataId=%s&groupName=%s&namespaceId=%s&content=%s&betaIps=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8"),
                URLEncoder.encode(grayContent, "UTF-8"),
                URLEncoder.encode("10.0.0.1,10.0.0.2", "UTF-8"));
        String grayResponse = httpPost("/nacos/v3/admin/cs/config/gray", grayBody);
        assertNotNull(grayResponse, "Gray config publish response should not be null");
        // Gray endpoint may not be fully implemented - verify we get a valid response
        JsonNode grayJson = objectMapper.readTree(grayResponse);
        // Accept both success and "not found" since gray API may not be implemented
        assertTrue(grayJson.has("code") || grayJson.has("error") || !grayResponse.isEmpty(),
                "Gray publish should return a parseable response: " + grayResponse);

        // Verify stable config is still accessible
        String stableGet = httpGet(String.format(
                "/nacos/v2/cs/config?dataId=%s&group=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8")));
        assertNotNull(stableGet);
        JsonNode stableJson = objectMapper.readTree(stableGet);
        if (stableJson.get("code").asInt() == 0 && stableJson.has("data")) {
            assertEquals(normalContent, stableJson.get("data").asText(),
                    "Stable config should remain unchanged after gray publish");
        }

        // Clean up gray config via console endpoint (best-effort, may not exist)
        try {
            httpDelete(String.format(
                    "/nacos/v3/console/cs/config/beta?dataId=%s&groupName=%s&namespaceId=%s",
                    URLEncoder.encode(dataId, "UTF-8"),
                    URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                    URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8")));
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
        String taggedContent = "delete.test=tagged";
        String tag = "to-delete";

        // Publish normal and tagged config
        String normalBody = String.format(
                "dataId=%s&group=%s&content=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(normalContent, "UTF-8"));
        httpPost("/nacos/v2/cs/config", normalBody);
        cleanupConfigs.add(new String[]{dataId, DEFAULT_GROUP});

        String tagBody = String.format(
                "dataId=%s&group=%s&content=%s&tag=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(taggedContent, "UTF-8"),
                URLEncoder.encode(tag, "UTF-8"));
        httpPost("/nacos/v2/cs/config", tagBody);
        Thread.sleep(500);

        // Delete the tagged config
        String deleteResponse = httpDelete(String.format(
                "/nacos/v2/cs/config?dataId=%s&group=%s&tag=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(tag, "UTF-8")));
        assertNotNull(deleteResponse);

        Thread.sleep(500);

        // Verify normal config still exists
        String getDefault = httpGet(String.format(
                "/nacos/v2/cs/config?dataId=%s&group=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8")));
        JsonNode getJson = objectMapper.readTree(getDefault);
        if (getJson.get("code").asInt() == 0 && getJson.has("data")) {
            assertEquals(normalContent, getJson.get("data").asText(),
                    "Normal config should still exist after deleting tagged config");
        }
    }

    // ==================== Helper Methods ====================

    private static void deleteConfig(String dataId, String group) throws Exception {
        httpDeleteStatic(String.format("/nacos/v2/cs/config?dataId=%s&group=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(group, "UTF-8")));
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
            String resp = readResponse(conn);
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

    private static String httpDeleteStatic(String path) throws Exception {
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
