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
        System.out.println("Config Export/Import Test Setup - Server: " + serverAddr);
    }

    // ==================== Export Tests ====================

    /**
     * EI-001: Test export configs by group
     */
    @Test
    @Order(1)
    void testExportByGroup() throws Exception {
        String dataId = "export-group-" + UUID.randomUUID().toString().substring(0, 8);

        // Create a config in the group
        publishConfig(dataId, DEFAULT_GROUP, "export.group.test=value");
        Thread.sleep(1000);

        // Export by group
        HttpURLConnection conn = createGetConnection(
                "/nacos/v3/console/cs/config/export?group=" + DEFAULT_GROUP
                        + "&namespaceId=" + DEFAULT_NAMESPACE);
        int responseCode = conn.getResponseCode();
        System.out.println("Export by group response code: " + responseCode);

        if (responseCode == 200) {
            String contentType = conn.getContentType();
            int contentLength = conn.getContentLength();
            System.out.println("Export content type: " + contentType + ", length: " + contentLength);
            // Should return ZIP or octet-stream
            byte[] data = conn.getInputStream().readAllBytes();
            assertTrue(data.length > 0, "Export should return non-empty data");
        }

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

        publishConfig(dataId, DEFAULT_GROUP, "export.all.test=value");
        Thread.sleep(1000);

        HttpURLConnection conn = createGetConnection(
                "/nacos/v3/console/cs/config/export?namespaceId=" + DEFAULT_NAMESPACE);
        int responseCode = conn.getResponseCode();
        System.out.println("Export all response code: " + responseCode);

        if (responseCode == 200) {
            byte[] data = conn.getInputStream().readAllBytes();
            System.out.println("Export all data size: " + data.length + " bytes");
            assertTrue(data.length > 0, "Export should return non-empty data");
        }

        deleteConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * EI-003: Test import configs
     */
    @Test
    @Order(3)
    void testImportConfigs() throws Exception {
        String dataId = "import-test-" + UUID.randomUUID().toString().substring(0, 8);

        // First publish and export
        publishConfig(dataId, DEFAULT_GROUP, "import.test=original");
        Thread.sleep(1000);

        HttpURLConnection exportConn = createGetConnection(
                "/nacos/v3/console/cs/config/export?namespaceId=" + DEFAULT_NAMESPACE);
        byte[] exportedData = null;
        if (exportConn.getResponseCode() == 200) {
            exportedData = exportConn.getInputStream().readAllBytes();
        }

        if (exportedData != null && exportedData.length > 0) {
            // Import the exported data
            String response = uploadMultipart(
                    "/nacos/v3/console/cs/config/import?namespaceId=" + DEFAULT_NAMESPACE + "&policy=OVERWRITE",
                    "file", "config-export.zip", exportedData);
            System.out.println("Import response: " + response);
        } else {
            System.out.println("Skipping import test - export returned no data");
        }

        deleteConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * EI-004: Test import with overwrite policy
     */
    @Test
    @Order(4)
    void testImportWithOverwritePolicy() throws Exception {
        String dataId = "import-overwrite-" + UUID.randomUUID().toString().substring(0, 8);

        // Publish original
        publishConfig(dataId, DEFAULT_GROUP, "import.overwrite=original");
        Thread.sleep(1000);

        // Export
        HttpURLConnection exportConn = createGetConnection(
                "/nacos/v3/console/cs/config/export?namespaceId=" + DEFAULT_NAMESPACE);
        byte[] exportedData = null;
        if (exportConn.getResponseCode() == 200) {
            exportedData = exportConn.getInputStream().readAllBytes();
        }

        // Modify the config
        publishConfig(dataId, DEFAULT_GROUP, "import.overwrite=modified");
        Thread.sleep(500);

        if (exportedData != null && exportedData.length > 0) {
            // Import with OVERWRITE should restore original
            String response = uploadMultipart(
                    "/nacos/v3/console/cs/config/import?namespaceId=" + DEFAULT_NAMESPACE + "&policy=OVERWRITE",
                    "file", "config-export.zip", exportedData);
            System.out.println("Import with overwrite: " + response);
        }

        deleteConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * EI-005: Test import with skip policy
     */
    @Test
    @Order(5)
    void testImportWithSkipPolicy() throws Exception {
        String dataId = "import-skip-" + UUID.randomUUID().toString().substring(0, 8);

        publishConfig(dataId, DEFAULT_GROUP, "import.skip=original");
        Thread.sleep(1000);

        HttpURLConnection exportConn = createGetConnection(
                "/nacos/v3/console/cs/config/export?namespaceId=" + DEFAULT_NAMESPACE);
        byte[] exportedData = null;
        if (exportConn.getResponseCode() == 200) {
            exportedData = exportConn.getInputStream().readAllBytes();
        }

        // Modify
        publishConfig(dataId, DEFAULT_GROUP, "import.skip=modified");
        Thread.sleep(500);

        if (exportedData != null && exportedData.length > 0) {
            // Import with SKIP should keep modified version
            String response = uploadMultipart(
                    "/nacos/v3/console/cs/config/import?namespaceId=" + DEFAULT_NAMESPACE + "&policy=SKIP",
                    "file", "config-export.zip", exportedData);
            System.out.println("Import with skip: " + response);
        }

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
                "/nacos/v3/console/cs/config/export?namespaceId=" + DEFAULT_NAMESPACE);
        byte[] exportedData = null;
        if (exportConn.getResponseCode() == 200) {
            exportedData = exportConn.getInputStream().readAllBytes();
        }

        // Delete configs
        deleteConfig(dataId1, DEFAULT_GROUP);
        deleteConfig(dataId2, DEFAULT_GROUP);
        Thread.sleep(500);

        if (exportedData != null && exportedData.length > 0) {
            // Import should restore
            String response = uploadMultipart(
                    "/nacos/v3/console/cs/config/import?namespaceId=" + DEFAULT_NAMESPACE + "&policy=OVERWRITE",
                    "file", "config-export.zip", exportedData);
            System.out.println("Round trip import: " + response);
        }

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
                "/nacos/v3/console/cs/config/export?group=" + nonExistentGroup
                        + "&namespaceId=" + DEFAULT_NAMESPACE);
        int responseCode = conn.getResponseCode();
        System.out.println("Export empty group response code: " + responseCode);

        if (responseCode == 200) {
            byte[] data = conn.getInputStream().readAllBytes();
            System.out.println("Export empty group data size: " + data.length + " bytes");
        } else {
            InputStream errorStream = conn.getErrorStream();
            if (errorStream != null) {
                String error = new String(errorStream.readAllBytes(), StandardCharsets.UTF_8);
                System.out.println("Export empty group error: " + error);
            }
        }
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
        System.out.println("Upload " + path + " -> " + responseCode);
        return response.toString();
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
