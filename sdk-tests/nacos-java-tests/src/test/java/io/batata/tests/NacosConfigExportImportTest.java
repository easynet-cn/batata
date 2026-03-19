package io.batata.tests;

import com.alibaba.nacos.api.config.model.ConfigDetailInfo;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.maintainer.client.NacosMaintainerFactory;
import com.alibaba.nacos.maintainer.client.config.ConfigMaintainerService;
import org.junit.jupiter.api.*;

import java.io.*;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.Properties;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Config Export/Import Tests
 *
 * Tests for config export/import functionality via V3 console API.
 * Uses ConfigMaintainerService for publish/get/delete operations,
 * and HTTP calls for export/import (not available in maintainer client).
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigExportImportTest {

    private static String serverAddr;
    private static String accessToken;
    private static ConfigMaintainerService maintainerService;
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

        maintainerService = NacosMaintainerFactory.createConfigMaintainerService(properties);
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
        boolean published = maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, content);
        assertTrue(published, "Publish config should succeed");
        Thread.sleep(1000);

        // Export by group (HTTP - not available in maintainer client)
        HttpURLConnection conn = createGetConnection(
                "/nacos/v3/console/cs/config/export2?group=" + DEFAULT_GROUP
                        + "&namespaceId=public");
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
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
    }

    /**
     * EI-002: Test export all configs in namespace
     */
    @Test
    @Order(2)
    void testExportAll() throws Exception {
        String dataId = "export-all-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "export.all.test=uniqueMarker123";

        maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, content);
        Thread.sleep(1000);

        HttpURLConnection conn = createGetConnection(
                "/nacos/v3/console/cs/config/export2?namespaceId=public");
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

        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
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
        maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, originalContent);
        Thread.sleep(1000);

        HttpURLConnection exportConn = createGetConnection(
                "/nacos/v3/console/cs/config/export2?namespaceId=public");
        assertEquals(200, exportConn.getResponseCode(), "Export should succeed");
        byte[] exportedData = exportConn.getInputStream().readAllBytes();
        assertNotNull(exportedData, "Exported data should not be null");
        assertTrue(exportedData.length > 0, "Exported data should not be empty");

        // Delete the config
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        Thread.sleep(500);

        // Verify config is gone
        try {
            ConfigDetailInfo afterDelete = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
            assertTrue(afterDelete == null || afterDelete.getContent() == null,
                    "Config should be deleted before import test");
        } catch (NacosException e) {
            // Expected - config is deleted
        }

        // Import the exported data (HTTP - not available in maintainer client)
        String response = uploadMultipart(
                "/nacos/v3/console/cs/config/import?namespaceId=public&policy=OVERWRITE",
                "file", "config-export.zip", exportedData);
        assertNotNull(response, "Import response should not be null");

        // Verify the imported config can be retrieved with correct content
        Thread.sleep(1000);
        ConfigDetailInfo afterImport = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(afterImport, "After import, config should be retrievable");
        assertEquals(originalContent, afterImport.getContent(),
                "After import, config should have original content: " + originalContent);

        // Cleanup
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
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
        maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, originalContent);
        Thread.sleep(1000);

        // Export (captures original)
        HttpURLConnection exportConn = createGetConnection(
                "/nacos/v3/console/cs/config/export2?namespaceId=public");
        assertEquals(200, exportConn.getResponseCode(), "Export should succeed");
        byte[] exportedData = exportConn.getInputStream().readAllBytes();
        assertTrue(exportedData.length > 0, "Exported data should not be empty");

        // Modify the config
        maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, modifiedContent);
        Thread.sleep(500);

        // Verify config is modified
        ConfigDetailInfo afterModify = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertEquals(modifiedContent, afterModify.getContent(),
                "Config should contain modified content before import");

        // Import with OVERWRITE should restore original
        String response = uploadMultipart(
                "/nacos/v3/console/cs/config/import?namespaceId=public&policy=OVERWRITE",
                "file", "config-export.zip", exportedData);
        assertNotNull(response, "Import response should not be null");

        Thread.sleep(1000);
        ConfigDetailInfo afterImport = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertEquals(originalContent, afterImport.getContent(),
                "After OVERWRITE import, config should be restored to original content");
        assertNotEquals(modifiedContent, afterImport.getContent(),
                "After OVERWRITE import, modified content should be gone");

        // Cleanup
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
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

        maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, originalContent);
        Thread.sleep(1000);

        HttpURLConnection exportConn = createGetConnection(
                "/nacos/v3/console/cs/config/export2?namespaceId=public");
        assertEquals(200, exportConn.getResponseCode(), "Export should succeed");
        byte[] exportedData = exportConn.getInputStream().readAllBytes();
        assertTrue(exportedData.length > 0, "Exported data should not be empty");

        // Modify
        maintainerService.publishConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, modifiedContent);
        Thread.sleep(500);

        // Import with SKIP should keep modified version (skip existing)
        String response = uploadMultipart(
                "/nacos/v3/console/cs/config/import?namespaceId=public&policy=SKIP",
                "file", "config-export.zip", exportedData);
        assertNotNull(response, "Import response should not be null");

        Thread.sleep(1000);
        ConfigDetailInfo afterImport = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertEquals(modifiedContent, afterImport.getContent(),
                "After SKIP import, config should still contain the modified content");
        assertNotEquals(originalContent, afterImport.getContent(),
                "After SKIP import, original content should NOT have overwritten the modified content");

        // Cleanup
        maintainerService.deleteConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
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
        maintainerService.publishConfig(dataId1, DEFAULT_GROUP, DEFAULT_NAMESPACE, content1);
        maintainerService.publishConfig(dataId2, DEFAULT_GROUP, DEFAULT_NAMESPACE, content2);
        Thread.sleep(1000);

        // Export
        HttpURLConnection exportConn = createGetConnection(
                "/nacos/v3/console/cs/config/export2?namespaceId=public");
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
        maintainerService.deleteConfig(dataId1, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        maintainerService.deleteConfig(dataId2, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        Thread.sleep(500);

        // Verify both are gone
        assertConfigDeleted(dataId1, content1, "Config 1 should be deleted");
        assertConfigDeleted(dataId2, content2, "Config 2 should be deleted");

        // Import should restore both
        String response = uploadMultipart(
                "/nacos/v3/console/cs/config/import?namespaceId=public&policy=OVERWRITE",
                "file", "config-export.zip", exportedData);
        assertNotNull(response, "Import response should not be null");

        Thread.sleep(1000);

        // Verify both configs are restored
        ConfigDetailInfo restored1 = maintainerService.getConfig(dataId1, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        ConfigDetailInfo restored2 = maintainerService.getConfig(dataId2, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        assertNotNull(restored1, "Config 1 should be restored after import");
        assertNotNull(restored2, "Config 2 should be restored after import");
        assertEquals(content1, restored1.getContent(),
                "After round-trip import, config 1 should be restored with content: " + content1);
        assertEquals(content2, restored2.getContent(),
                "After round-trip import, config 2 should be restored with content: " + content2);

        // Cleanup
        maintainerService.deleteConfig(dataId1, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        maintainerService.deleteConfig(dataId2, DEFAULT_GROUP, DEFAULT_NAMESPACE);
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
                        + "&namespaceId=public");
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
                "/nacos/v3/console/cs/config/import?namespaceId=public&policy=OVERWRITE",
                "file", "invalid.zip", invalidData);

        assertNotNull(response, "Import of invalid data should return a response");
        // The response should indicate an error (not silently succeed)
        // Check that the response does not indicate success, or contains error info
        assertFalse(response.contains("\"code\":0") && response.contains("\"data\":null"),
                "Import of invalid ZIP data should not silently succeed");
    }

    // ==================== Helper Methods ====================

    private void assertConfigDeleted(String dataId, String content, String message) {
        try {
            ConfigDetailInfo config = maintainerService.getConfig(dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE);
            assertTrue(config == null || config.getContent() == null || !config.getContent().contains(content), message);
        } catch (NacosException e) {
            // Expected - config is deleted
        }
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
