package io.batata.tests;

import org.junit.jupiter.api.*;

import java.io.*;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Config Export/Import Tests
 *
 * Tests for config export/import functionality via V3 console API.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigExportImportTest {

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
    }

    // ==================== Export Tests ====================

    /**
     * EI-001: Test export configs by group
     */
    @Test
    @Order(1)
    void testExportByGroup() throws Exception {
        String dataId = "export-group-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "export.group.test=value";

        // Create a config in the group
        publishConfig(dataId, DEFAULT_GROUP, content);
        Thread.sleep(1000);

        // Export by group
        HttpURLConnection conn = createGetConnection(
                "/nacos/v3/console/cs/config/export2?group=" + DEFAULT_GROUP
                        + "&namespaceId=" + DEFAULT_NAMESPACE);
        int responseCode = conn.getResponseCode();
        assertEquals(200, responseCode, "Export by group should return HTTP 200");

        byte[] data = conn.getInputStream().readAllBytes();
        assertTrue(data.length > 0, "Export should return non-empty data");

        // ZIP files start with PK magic bytes (0x50, 0x4B)
        assertTrue(data.length >= 2 && data[0] == 0x50 && data[1] == 0x4B,
                "Export data should be a valid ZIP file (PK header)");

        // Verify the exported ZIP contains our config by checking the raw bytes for the dataId
        String dataAsString = new String(data, StandardCharsets.ISO_8859_1);
        assertTrue(dataAsString.contains(dataId),
                "Exported ZIP should contain the published config dataId: " + dataId);

        // Cleanup
        deleteConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * EI-002: Test export all configs in namespace
     */
    @Test
    @Order(2)
    void testExportAll() throws Exception {
        String dataId = "export-all-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "export.all.test=uniqueMarker123";

        publishConfig(dataId, DEFAULT_GROUP, content);
        Thread.sleep(1000);

        HttpURLConnection conn = createGetConnection(
                "/nacos/v3/console/cs/config/export2?namespaceId=" + DEFAULT_NAMESPACE);
        int responseCode = conn.getResponseCode();
        assertEquals(200, responseCode, "Export all should return HTTP 200");

        byte[] data = conn.getInputStream().readAllBytes();
        assertTrue(data.length > 0, "Export all should return non-empty data");

        // Verify ZIP format
        assertTrue(data.length >= 2 && data[0] == 0x50 && data[1] == 0x4B,
                "Export data should be a valid ZIP file");

        // Verify the exported data contains our specific config
        String dataAsString = new String(data, StandardCharsets.ISO_8859_1);
        assertTrue(dataAsString.contains(dataId),
                "Exported ZIP should contain the config dataId: " + dataId);

        deleteConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * EI-003: Test import configs
     */
    @Test
    @Order(3)
    void testImportConfigs() throws Exception {
        String dataId = "import-test-" + UUID.randomUUID().toString().substring(0, 8);
        String originalContent = "import.test=original";

        // Publish and export
        publishConfig(dataId, DEFAULT_GROUP, originalContent);
        Thread.sleep(1000);

        HttpURLConnection exportConn = createGetConnection(
                "/nacos/v3/console/cs/config/export2?namespaceId=" + DEFAULT_NAMESPACE);
        assertEquals(200, exportConn.getResponseCode(), "Export should succeed");
        byte[] exportedData = exportConn.getInputStream().readAllBytes();
        assertNotNull(exportedData, "Exported data should not be null");
        assertTrue(exportedData.length > 0, "Exported data should not be empty");

        // Delete the config
        deleteConfig(dataId, DEFAULT_GROUP);
        Thread.sleep(500);

        // Verify config is gone
        String afterDelete = getConfig(dataId, DEFAULT_GROUP);
        assertFalse(afterDelete.contains(originalContent),
                "Config should be deleted before import test");

        // Import the exported data
        String response = uploadMultipart(
                "/nacos/v3/console/cs/config/import?namespaceId=" + DEFAULT_NAMESPACE + "&policy=OVERWRITE",
                "file", "config-export.zip", exportedData);
        assertNotNull(response, "Import response should not be null");

        // Verify the imported config can be retrieved with correct content
        Thread.sleep(1000);
        String afterImport = getConfig(dataId, DEFAULT_GROUP);
        assertTrue(afterImport.contains(originalContent),
                "After import, config should be retrievable with original content: " + originalContent);

        // Cleanup
        deleteConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * EI-004: Test import with overwrite policy
     */
    @Test
    @Order(4)
    void testImportWithOverwritePolicy() throws Exception {
        String dataId = "import-overwrite-" + UUID.randomUUID().toString().substring(0, 8);
        String originalContent = "import.overwrite=original";
        String modifiedContent = "import.overwrite=modified";

        // Publish original
        publishConfig(dataId, DEFAULT_GROUP, originalContent);
        Thread.sleep(1000);

        // Export (captures original)
        HttpURLConnection exportConn = createGetConnection(
                "/nacos/v3/console/cs/config/export2?namespaceId=" + DEFAULT_NAMESPACE);
        assertEquals(200, exportConn.getResponseCode(), "Export should succeed");
        byte[] exportedData = exportConn.getInputStream().readAllBytes();
        assertTrue(exportedData.length > 0, "Exported data should not be empty");

        // Modify the config
        publishConfig(dataId, DEFAULT_GROUP, modifiedContent);
        Thread.sleep(500);

        // Verify config is modified
        String afterModify = getConfig(dataId, DEFAULT_GROUP);
        assertTrue(afterModify.contains(modifiedContent),
                "Config should contain modified content before import");

        // Import with OVERWRITE should restore original
        String response = uploadMultipart(
                "/nacos/v3/console/cs/config/import?namespaceId=" + DEFAULT_NAMESPACE + "&policy=OVERWRITE",
                "file", "config-export.zip", exportedData);
        assertNotNull(response, "Import response should not be null");

        Thread.sleep(1000);
        String afterImport = getConfig(dataId, DEFAULT_GROUP);
        assertTrue(afterImport.contains(originalContent),
                "After OVERWRITE import, config should be restored to original content");
        assertFalse(afterImport.contains(modifiedContent),
                "After OVERWRITE import, modified content should be gone");

        // Cleanup
        deleteConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * EI-005: Test import with skip policy
     */
    @Test
    @Order(5)
    void testImportWithSkipPolicy() throws Exception {
        String dataId = "import-skip-" + UUID.randomUUID().toString().substring(0, 8);
        String originalContent = "import.skip=original";
        String modifiedContent = "import.skip=modified";

        publishConfig(dataId, DEFAULT_GROUP, originalContent);
        Thread.sleep(1000);

        HttpURLConnection exportConn = createGetConnection(
                "/nacos/v3/console/cs/config/export2?namespaceId=" + DEFAULT_NAMESPACE);
        assertEquals(200, exportConn.getResponseCode(), "Export should succeed");
        byte[] exportedData = exportConn.getInputStream().readAllBytes();
        assertTrue(exportedData.length > 0, "Exported data should not be empty");

        // Modify
        publishConfig(dataId, DEFAULT_GROUP, modifiedContent);
        Thread.sleep(500);

        // Import with SKIP should keep modified version (skip existing)
        String response = uploadMultipart(
                "/nacos/v3/console/cs/config/import?namespaceId=" + DEFAULT_NAMESPACE + "&policy=SKIP",
                "file", "config-export.zip", exportedData);
        assertNotNull(response, "Import response should not be null");

        Thread.sleep(1000);
        String afterImport = getConfig(dataId, DEFAULT_GROUP);
        assertTrue(afterImport.contains(modifiedContent),
                "After SKIP import, config should still contain the modified content");
        assertFalse(afterImport.contains(originalContent),
                "After SKIP import, original content should NOT have overwritten the modified content");

        // Cleanup
        deleteConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * EI-006: Test export-import round trip
     */
    @Test
    @Order(6)
    void testExportImportRoundTrip() throws Exception {
        String dataId1 = "roundtrip-1-" + UUID.randomUUID().toString().substring(0, 8);
        String dataId2 = "roundtrip-2-" + UUID.randomUUID().toString().substring(0, 8);
        String content1 = "roundtrip.key1=value1";
        String content2 = "roundtrip.key2=value2";

        // Publish two configs
        publishConfig(dataId1, DEFAULT_GROUP, content1);
        publishConfig(dataId2, DEFAULT_GROUP, content2);
        Thread.sleep(1000);

        // Export
        HttpURLConnection exportConn = createGetConnection(
                "/nacos/v3/console/cs/config/export2?namespaceId=" + DEFAULT_NAMESPACE);
        assertEquals(200, exportConn.getResponseCode(), "Export should succeed");
        byte[] exportedData = exportConn.getInputStream().readAllBytes();
        assertTrue(exportedData.length > 0, "Exported data should not be empty");

        // Verify exported data contains both configs
        String dataAsString = new String(exportedData, StandardCharsets.ISO_8859_1);
        assertTrue(dataAsString.contains(dataId1),
                "Exported ZIP should contain first config dataId: " + dataId1);
        assertTrue(dataAsString.contains(dataId2),
                "Exported ZIP should contain second config dataId: " + dataId2);

        // Delete both configs
        deleteConfig(dataId1, DEFAULT_GROUP);
        deleteConfig(dataId2, DEFAULT_GROUP);
        Thread.sleep(500);

        // Verify both are gone
        String check1 = getConfig(dataId1, DEFAULT_GROUP);
        String check2 = getConfig(dataId2, DEFAULT_GROUP);
        assertFalse(check1.contains(content1), "Config 1 should be deleted");
        assertFalse(check2.contains(content2), "Config 2 should be deleted");

        // Import should restore both
        String response = uploadMultipart(
                "/nacos/v3/console/cs/config/import?namespaceId=" + DEFAULT_NAMESPACE + "&policy=OVERWRITE",
                "file", "config-export.zip", exportedData);
        assertNotNull(response, "Import response should not be null");

        Thread.sleep(1000);

        // Verify both configs are restored
        String restored1 = getConfig(dataId1, DEFAULT_GROUP);
        String restored2 = getConfig(dataId2, DEFAULT_GROUP);
        assertTrue(restored1.contains(content1),
                "After round-trip import, config 1 should be restored with content: " + content1);
        assertTrue(restored2.contains(content2),
                "After round-trip import, config 2 should be restored with content: " + content2);

        // Cleanup
        deleteConfig(dataId1, DEFAULT_GROUP);
        deleteConfig(dataId2, DEFAULT_GROUP);
    }

    /**
     * EI-007: Test export empty/non-existent group
     */
    @Test
    @Order(7)
    void testExportEmptyGroup() throws Exception {
        String nonExistentGroup = "NON_EXISTENT_GROUP_" + UUID.randomUUID().toString().substring(0, 8);

        HttpURLConnection conn = createGetConnection(
                "/nacos/v3/console/cs/config/export2?group=" + nonExistentGroup
                        + "&namespaceId=" + DEFAULT_NAMESPACE);
        int responseCode = conn.getResponseCode();

        // Either returns 200 with empty/minimal ZIP or returns an error code
        if (responseCode == 200) {
            byte[] data = conn.getInputStream().readAllBytes();
            // If 200, the data should be a valid ZIP but may be empty or very small
            assertNotNull(data, "Response data should not be null even for empty group");
        } else {
            // Non-200 is acceptable for empty group export (e.g. 400 or 404)
            assertTrue(responseCode == 400 || responseCode == 404 || responseCode == 500,
                    "Export of non-existent group should return 200 (empty), 400, 404, or 500, got: " + responseCode);
        }
    }

    /**
     * EI-008: Test import with invalid/corrupted ZIP data
     */
    @Test
    @Order(8)
    void testImportInvalidData() throws Exception {
        byte[] invalidData = "this is not a zip file".getBytes(StandardCharsets.UTF_8);

        String response = uploadMultipart(
                "/nacos/v3/console/cs/config/import?namespaceId=" + DEFAULT_NAMESPACE + "&policy=OVERWRITE",
                "file", "invalid.zip", invalidData);

        assertNotNull(response, "Import of invalid data should return a response");
        // The response should indicate an error (not silently succeed)
        // Check that the response does not indicate success, or contains error info
        assertFalse(response.contains("\"code\":0") && response.contains("\"data\":null"),
                "Import of invalid ZIP data should not silently succeed");
    }

    // ==================== Helper Methods ====================

    private void publishConfig(String dataId, String group, String content) throws Exception {
        String body = String.format(
                "dataId=%s&groupName=%s&namespaceId=%s&content=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(group, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8"),
                URLEncoder.encode(content, "UTF-8"));
        String response = httpPost("/nacos/v3/console/cs/config", body);
        assertNotNull(response, "Publish config response should not be null for dataId: " + dataId);
    }

    private String getConfig(String dataId, String group) throws Exception {
        return httpGet(String.format("/nacos/v3/console/cs/config?dataId=%s&groupName=%s&namespaceId=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(group, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8")));
    }

    private void deleteConfig(String dataId, String group) throws Exception {
        httpDelete(String.format("/nacos/v3/console/cs/config?dataId=%s&groupName=%s&namespaceId=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(group, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8")));
    }

    private HttpURLConnection createGetConnection(String path) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }
        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("GET");
        return conn;
    }

    private String uploadMultipart(String path, String fieldName, String fileName, byte[] data) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }

        String boundary = "----Boundary" + System.currentTimeMillis();
        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("POST");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "multipart/form-data; boundary=" + boundary);

        try (OutputStream os = conn.getOutputStream()) {
            // Write file part
            os.write(("--" + boundary + "\r\n").getBytes(StandardCharsets.UTF_8));
            os.write(("Content-Disposition: form-data; name=\"" + fieldName + "\"; filename=\"" + fileName + "\"\r\n").getBytes(StandardCharsets.UTF_8));
            os.write("Content-Type: application/zip\r\n\r\n".getBytes(StandardCharsets.UTF_8));
            os.write(data);
            os.write(("\r\n--" + boundary + "--\r\n").getBytes(StandardCharsets.UTF_8));
        }

        return readResponse(conn);
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

    private static String httpPost(String path, String body) throws Exception {
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

    private static String httpDelete(String path) throws Exception {
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
