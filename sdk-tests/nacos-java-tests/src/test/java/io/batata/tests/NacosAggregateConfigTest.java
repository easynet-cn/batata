package io.batata.tests;

import org.junit.jupiter.api.*;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Aggregate Config API Tests
 *
 * Tests for aggregate configuration management.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosAggregateConfigTest {

    private static String serverAddr;
    private static String accessToken;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        accessToken = getAccessToken(username, password);
        System.out.println("Aggregate Config Test Setup - Server: " + serverAddr);
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
            String resp = response.toString();
            if (resp.contains("accessToken")) {
                int start = resp.indexOf("accessToken") + 14;
                int end = resp.indexOf("\"", start);
                return resp.substring(start, end);
            }
        }
        return "";
    }

    private String httpRequest(String method, String path, String body) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }

        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod(method);

        if (body != null && !body.isEmpty()) {
            conn.setDoOutput(true);
            conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");
            try (OutputStream os = conn.getOutputStream()) {
                os.write(body.getBytes(StandardCharsets.UTF_8));
            }
        }

        int responseCode = conn.getResponseCode();
        BufferedReader reader = new BufferedReader(new InputStreamReader(
                responseCode >= 400 ? conn.getErrorStream() : conn.getInputStream()));
        StringBuilder response = new StringBuilder();
        String line;
        while ((line = reader.readLine()) != null) {
            response.append(line);
        }
        System.out.println(method + " " + path + " -> " + responseCode);
        return response.toString();
    }

    // ==================== Aggregate Config Tests ====================

    /**
     * Test publish aggregate config
     */
    @Test
    @Order(1)
    void testPublishAggregateConfig() throws Exception {
        String dataId = "aggr-test-" + UUID.randomUUID().toString().substring(0, 8);
        String datumId = "datum-1";
        String content = "aggregate.key=aggregate.value";

        String body = String.format(
                "dataId=%s&group=%s&datumId=%s&content=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(datumId, "UTF-8"),
                URLEncoder.encode(content, "UTF-8"));

        String response = httpRequest("POST", "/nacos/v2/cs/config/aggr", body);
        System.out.println("Publish aggregate config: " + response);

        // Cleanup
        httpRequest("DELETE", String.format(
                "/nacos/v2/cs/config/aggr?dataId=%s&group=%s&datumId=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(datumId, "UTF-8")), null);
    }

    /**
     * Test get aggregate config
     */
    @Test
    @Order(2)
    void testGetAggregateConfig() throws Exception {
        String dataId = "aggr-get-" + UUID.randomUUID().toString().substring(0, 8);
        String datumId = "datum-get";

        // Create first
        String body = String.format(
                "dataId=%s&group=%s&datumId=%s&content=%s",
                dataId, DEFAULT_GROUP, datumId, "get.test=value");
        httpRequest("POST", "/nacos/v2/cs/config/aggr", body);

        // Get
        String response = httpRequest("GET", String.format(
                "/nacos/v2/cs/config/aggr?dataId=%s&group=%s&datumId=%s",
                dataId, DEFAULT_GROUP, datumId), null);
        System.out.println("Get aggregate config: " + response);

        // Cleanup
        httpRequest("DELETE", String.format(
                "/nacos/v2/cs/config/aggr?dataId=%s&group=%s&datumId=%s",
                dataId, DEFAULT_GROUP, datumId), null);
    }

    /**
     * Test delete aggregate config
     */
    @Test
    @Order(3)
    void testDeleteAggregateConfig() throws Exception {
        String dataId = "aggr-delete-" + UUID.randomUUID().toString().substring(0, 8);
        String datumId = "datum-delete";

        // Create first
        String body = String.format(
                "dataId=%s&group=%s&datumId=%s&content=%s",
                dataId, DEFAULT_GROUP, datumId, "delete.test=value");
        httpRequest("POST", "/nacos/v2/cs/config/aggr", body);

        // Delete
        String response = httpRequest("DELETE", String.format(
                "/nacos/v2/cs/config/aggr?dataId=%s&group=%s&datumId=%s",
                dataId, DEFAULT_GROUP, datumId), null);
        System.out.println("Delete aggregate config: " + response);
    }

    /**
     * Test list aggregate configs
     */
    @Test
    @Order(4)
    void testListAggregateConfigs() throws Exception {
        String dataId = "aggr-list-" + UUID.randomUUID().toString().substring(0, 8);

        // Create multiple datums
        for (int i = 0; i < 3; i++) {
            String body = String.format(
                    "dataId=%s&group=%s&datumId=%s&content=%s",
                    dataId, DEFAULT_GROUP, "datum-" + i, "list.test=" + i);
            httpRequest("POST", "/nacos/v2/cs/config/aggr", body);
        }

        // List
        String response = httpRequest("GET", String.format(
                "/nacos/v2/cs/config/aggr/list?dataId=%s&group=%s&pageNo=1&pageSize=10",
                dataId, DEFAULT_GROUP), null);
        System.out.println("List aggregate configs: " + response);

        // Cleanup
        for (int i = 0; i < 3; i++) {
            httpRequest("DELETE", String.format(
                    "/nacos/v2/cs/config/aggr?dataId=%s&group=%s&datumId=%s",
                    dataId, DEFAULT_GROUP, "datum-" + i), null);
        }
    }

    /**
     * Test list datum IDs
     */
    @Test
    @Order(5)
    void testListDatumIds() throws Exception {
        String dataId = "aggr-datumids-" + UUID.randomUUID().toString().substring(0, 8);

        // Create multiple datums
        for (int i = 0; i < 3; i++) {
            String body = String.format(
                    "dataId=%s&group=%s&datumId=%s&content=%s",
                    dataId, DEFAULT_GROUP, "id-datum-" + i, "datumid.test=" + i);
            httpRequest("POST", "/nacos/v2/cs/config/aggr", body);
        }

        // List datum IDs
        String response = httpRequest("GET", String.format(
                "/nacos/v2/cs/config/aggr/datumIds?dataId=%s&group=%s",
                dataId, DEFAULT_GROUP), null);
        System.out.println("List datum IDs: " + response);

        // Cleanup
        for (int i = 0; i < 3; i++) {
            httpRequest("DELETE", String.format(
                    "/nacos/v2/cs/config/aggr?dataId=%s&group=%s&datumId=%s",
                    dataId, DEFAULT_GROUP, "id-datum-" + i), null);
        }
    }

    /**
     * Test count aggregate configs
     */
    @Test
    @Order(6)
    void testCountAggregateConfigs() throws Exception {
        String dataId = "aggr-count-" + UUID.randomUUID().toString().substring(0, 8);

        // Create multiple datums
        for (int i = 0; i < 5; i++) {
            String body = String.format(
                    "dataId=%s&group=%s&datumId=%s&content=%s",
                    dataId, DEFAULT_GROUP, "count-datum-" + i, "count.test=" + i);
            httpRequest("POST", "/nacos/v2/cs/config/aggr", body);
        }

        // Count
        String response = httpRequest("GET", String.format(
                "/nacos/v2/cs/config/aggr/count?dataId=%s&group=%s",
                dataId, DEFAULT_GROUP), null);
        System.out.println("Count aggregate configs: " + response);

        // Cleanup
        for (int i = 0; i < 5; i++) {
            httpRequest("DELETE", String.format(
                    "/nacos/v2/cs/config/aggr?dataId=%s&group=%s&datumId=%s",
                    dataId, DEFAULT_GROUP, "count-datum-" + i), null);
        }
    }

    /**
     * Test aggregate config with namespace
     */
    @Test
    @Order(7)
    void testAggregateConfigWithNamespace() throws Exception {
        String namespace = "aggr-ns-test";
        String dataId = "aggr-ns-" + UUID.randomUUID().toString().substring(0, 8);
        String datumId = "ns-datum";

        String body = String.format(
                "dataId=%s&group=%s&datumId=%s&content=%s&tenant=%s",
                dataId, DEFAULT_GROUP, datumId, "namespace.test=value", namespace);
        String response = httpRequest("POST", "/nacos/v2/cs/config/aggr", body);
        System.out.println("Aggregate config with namespace: " + response);

        // Cleanup
        httpRequest("DELETE", String.format(
                "/nacos/v2/cs/config/aggr?dataId=%s&group=%s&datumId=%s&tenant=%s",
                dataId, DEFAULT_GROUP, datumId, namespace), null);
    }
}
