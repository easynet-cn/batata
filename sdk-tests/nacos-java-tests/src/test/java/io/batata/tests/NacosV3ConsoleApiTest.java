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
 * Nacos V3 Console API Tests
 *
 * Tests for Nacos 3.x specific Console APIs at /v3/console/*
 * These APIs are different from the V2 SDK APIs and are primarily used by the Nacos Console UI.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosV3ConsoleApiTest {

    private static String serverAddr;
    private static String accessToken;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final String DEFAULT_NAMESPACE = "public";

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        // Login via V3 Auth API
        accessToken = loginV3(username, password);
        System.out.println("Nacos V3 Console API Test Setup - Server: " + serverAddr);
        System.out.println("Access Token: " + (accessToken.isEmpty() ? "NONE" : "OK"));
    }

    private static String loginV3(String username, String password) throws Exception {
        String loginUrl = String.format("http://%s/nacos/v3/auth/user/login", serverAddr);
        URL url = new URL(loginUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("POST");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");

        String body = "username=" + URLEncoder.encode(username, "UTF-8") +
                "&password=" + URLEncoder.encode(password, "UTF-8");
        conn.getOutputStream().write(body.getBytes(StandardCharsets.UTF_8));

        int responseCode = conn.getResponseCode();
        System.out.println("V3 Login response code: " + responseCode);

        if (responseCode == 200) {
            BufferedReader reader = new BufferedReader(new InputStreamReader(conn.getInputStream()));
            StringBuilder response = new StringBuilder();
            String line;
            while ((line = reader.readLine()) != null) {
                response.append(line);
            }
            String resp = response.toString();
            System.out.println("V3 Login response: " + resp);

            // Parse token from response
            if (resp.contains("accessToken")) {
                int start = resp.indexOf("accessToken") + 14;
                int end = resp.indexOf("\"", start);
                if (end > start) {
                    return resp.substring(start, end);
                }
            }
            // Try alternate format
            if (resp.contains("token")) {
                int start = resp.indexOf("token") + 8;
                int end = resp.indexOf("\"", start);
                if (end > start) {
                    return resp.substring(start, end);
                }
            }
        }

        // Fallback to V1 login
        return loginV1(username, password);
    }

    private static String loginV1(String username, String password) throws Exception {
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
                if (end > start) {
                    return resp.substring(start, end);
                }
            }
        }
        return "";
    }

    private HttpURLConnection createConnection(String method, String path) throws Exception {
        // Ensure path has /nacos prefix
        String normalizedPath = path.startsWith("/nacos") ? path : "/nacos" + path;
        String fullUrl = String.format("http://%s%s", serverAddr, normalizedPath);
        if (!accessToken.isEmpty()) {
            fullUrl += (normalizedPath.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }

        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod(method);
        return conn;
    }

    private String httpGet(String path) throws Exception {
        HttpURLConnection conn = createConnection("GET", path);
        return readResponse(conn);
    }

    private String httpPost(String path, String body, String contentType) throws Exception {
        HttpURLConnection conn = createConnection("POST", path);
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", contentType);

        if (body != null && !body.isEmpty()) {
            try (OutputStream os = conn.getOutputStream()) {
                os.write(body.getBytes(StandardCharsets.UTF_8));
            }
        }
        return readResponse(conn);
    }

    private String httpPut(String path, String body, String contentType) throws Exception {
        HttpURLConnection conn = createConnection("PUT", path);
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", contentType);

        if (body != null && !body.isEmpty()) {
            try (OutputStream os = conn.getOutputStream()) {
                os.write(body.getBytes(StandardCharsets.UTF_8));
            }
        }
        return readResponse(conn);
    }

    private String httpDelete(String path) throws Exception {
        HttpURLConnection conn = createConnection("DELETE", path);
        return readResponse(conn);
    }

    private String readResponse(HttpURLConnection conn) throws Exception {
        int responseCode = conn.getResponseCode();
        InputStream is = responseCode >= 400 ? conn.getErrorStream() : conn.getInputStream();
        if (is == null) {
            return "Status: " + responseCode;
        }

        BufferedReader reader = new BufferedReader(new InputStreamReader(is));
        StringBuilder response = new StringBuilder();
        String line;
        while ((line = reader.readLine()) != null) {
            response.append(line);
        }
        return response.toString();
    }

    // ==================== V3 Config Console API Tests ====================

    /**
     * V3C-001: Test V3 config list (search)
     */
    @Test
    @Order(1)
    void testV3ConfigList() throws Exception {
        String response = httpGet("/v3/console/cs/config/list?pageNo=1&pageSize=10&namespaceId=" + DEFAULT_NAMESPACE);
        System.out.println("V3 Config List: " + response);
        assertNotNull(response);
    }

    /**
     * V3C-002: Test V3 config get
     */
    @Test
    @Order(2)
    void testV3ConfigGet() throws Exception {
        String dataId = "v3-config-" + UUID.randomUUID().toString().substring(0, 8);

        // Create via V3
        String createBody = String.format(
                "dataId=%s&groupName=%s&namespaceId=%s&content=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8"),
                URLEncoder.encode("v3.test=value", "UTF-8"));
        httpPost("/v3/console/cs/config", createBody, "application/x-www-form-urlencoded");

        // Get via V3
        String response = httpGet(String.format(
                "/v3/console/cs/config?dataId=%s&groupName=%s&namespaceId=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8")));
        System.out.println("V3 Config Get: " + response);

        // Cleanup
        httpDelete(String.format(
                "/v3/console/cs/config?dataId=%s&groupName=%s&tenant=%s",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8")));
    }

    /**
     * V3C-003: Test V3 config create/update
     */
    @Test
    @Order(3)
    void testV3ConfigCreateUpdate() throws Exception {
        String dataId = "v3-create-" + UUID.randomUUID().toString().substring(0, 8);

        String body = String.format(
                "dataId=%s&groupName=%s&namespaceId=%s&content=%s&type=properties",
                URLEncoder.encode(dataId, "UTF-8"),
                URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                URLEncoder.encode(DEFAULT_NAMESPACE, "UTF-8"),
                URLEncoder.encode("created.by=v3.console.api\nversion=1", "UTF-8"));

        String response = httpPost("/v3/console/cs/config", body, "application/x-www-form-urlencoded");
        System.out.println("V3 Config Create: " + response);
        assertTrue(response.contains("true") || response.contains("success"), "Create should succeed");

        // Cleanup
        httpDelete(String.format(
                "/v3/console/cs/config?dataId=%s&groupName=%s&tenant=%s",
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE));
    }

    /**
     * V3C-004: Test V3 config delete
     */
    @Test
    @Order(4)
    void testV3ConfigDelete() throws Exception {
        String dataId = "v3-delete-" + UUID.randomUUID().toString().substring(0, 8);

        // Create first
        String createBody = String.format(
                "dataId=%s&groupName=%s&namespaceId=%s&content=%s",
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, "to.delete=true");
        httpPost("/v3/console/cs/config", createBody, "application/x-www-form-urlencoded");

        // Delete
        String response = httpDelete(String.format(
                "/v3/console/cs/config?dataId=%s&groupName=%s&tenant=%s",
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE));
        System.out.println("V3 Config Delete: " + response);
    }

    /**
     * V3C-005: Test V3 config beta (gray release)
     */
    @Test
    @Order(5)
    void testV3ConfigBeta() throws Exception {
        String dataId = "v3-beta-" + UUID.randomUUID().toString().substring(0, 8);

        // Create a config first
        String createBody = String.format(
                "dataId=%s&groupName=%s&namespaceId=%s&content=%s",
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE, "beta.test=value");
        httpPost("/v3/console/cs/config", createBody, "application/x-www-form-urlencoded");

        // Get beta config
        String response = httpGet(String.format(
                "/v3/console/cs/config/beta?dataId=%s&groupName=%s&namespaceId=%s",
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE));
        System.out.println("V3 Config Beta: " + response);

        // Cleanup
        httpDelete(String.format(
                "/v3/console/cs/config?dataId=%s&groupName=%s&tenant=%s",
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE));
    }

    /**
     * V3C-006: Test V3 config listeners
     */
    @Test
    @Order(6)
    void testV3ConfigListener() throws Exception {
        String dataId = "v3-listener-" + UUID.randomUUID().toString().substring(0, 8);

        String response = httpGet(String.format(
                "/v3/console/cs/config/listener?dataId=%s&groupName=%s&namespaceId=%s",
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE));
        System.out.println("V3 Config Listener: " + response);
    }

    /**
     * V3C-007: Test V3 config export
     */
    @Test
    @Order(7)
    void testV3ConfigExport() throws Exception {
        String response = httpGet(String.format(
                "/v3/console/cs/config/export?namespaceId=%s",
                DEFAULT_NAMESPACE));
        System.out.println("V3 Config Export response length: " + response.length());
        // Export returns ZIP file or error
    }

    // ==================== V3 Service Console API Tests ====================

    /**
     * V3S-001: Test V3 service list
     */
    @Test
    @Order(10)
    void testV3ServiceList() throws Exception {
        String response = httpGet(String.format(
                "/v3/console/ns/service/list?pageNo=1&pageSize=10&namespaceId=%s&groupName=%s",
                DEFAULT_NAMESPACE, DEFAULT_GROUP));
        System.out.println("V3 Service List: " + response);
        assertNotNull(response);
    }

    /**
     * V3S-002: Test V3 service create
     */
    @Test
    @Order(11)
    void testV3ServiceCreate() throws Exception {
        String serviceName = "v3-service-" + UUID.randomUUID().toString().substring(0, 8);

        String body = String.format(
                "{\"serviceName\":\"%s\",\"groupName\":\"%s\",\"namespaceId\":\"%s\",\"protectThreshold\":0.5}",
                serviceName, DEFAULT_GROUP, DEFAULT_NAMESPACE);

        String response = httpPost("/v3/console/ns/service", body, "application/json");
        System.out.println("V3 Service Create: " + response);

        // Cleanup
        httpDelete(String.format(
                "/v3/console/ns/service?serviceName=%s&groupName=%s&namespaceId=%s",
                serviceName, DEFAULT_GROUP, DEFAULT_NAMESPACE));
    }

    /**
     * V3S-003: Test V3 service get
     */
    @Test
    @Order(12)
    void testV3ServiceGet() throws Exception {
        String serviceName = "v3-get-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Create first
        String body = String.format(
                "{\"serviceName\":\"%s\",\"groupName\":\"%s\",\"namespaceId\":\"%s\"}",
                serviceName, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        httpPost("/v3/console/ns/service", body, "application/json");

        // Get
        String response = httpGet(String.format(
                "/v3/console/ns/service?serviceName=%s&groupName=%s&namespaceId=%s",
                serviceName, DEFAULT_GROUP, DEFAULT_NAMESPACE));
        System.out.println("V3 Service Get: " + response);

        // Cleanup
        httpDelete(String.format(
                "/v3/console/ns/service?serviceName=%s&groupName=%s&namespaceId=%s",
                serviceName, DEFAULT_GROUP, DEFAULT_NAMESPACE));
    }

    /**
     * V3S-004: Test V3 service update
     */
    @Test
    @Order(13)
    void testV3ServiceUpdate() throws Exception {
        String serviceName = "v3-update-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Create first
        String createBody = String.format(
                "{\"serviceName\":\"%s\",\"groupName\":\"%s\",\"namespaceId\":\"%s\",\"protectThreshold\":0.5}",
                serviceName, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        httpPost("/v3/console/ns/service", createBody, "application/json");

        // Update
        String updateBody = String.format(
                "{\"serviceName\":\"%s\",\"groupName\":\"%s\",\"namespaceId\":\"%s\",\"protectThreshold\":0.8}",
                serviceName, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        String response = httpPut("/v3/console/ns/service", updateBody, "application/json");
        System.out.println("V3 Service Update: " + response);

        // Cleanup
        httpDelete(String.format(
                "/v3/console/ns/service?serviceName=%s&groupName=%s&namespaceId=%s",
                serviceName, DEFAULT_GROUP, DEFAULT_NAMESPACE));
    }

    /**
     * V3S-005: Test V3 service delete
     */
    @Test
    @Order(14)
    void testV3ServiceDelete() throws Exception {
        String serviceName = "v3-delete-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Create first
        String body = String.format(
                "{\"serviceName\":\"%s\",\"groupName\":\"%s\",\"namespaceId\":\"%s\"}",
                serviceName, DEFAULT_GROUP, DEFAULT_NAMESPACE);
        httpPost("/v3/console/ns/service", body, "application/json");

        // Delete
        String response = httpDelete(String.format(
                "/v3/console/ns/service?serviceName=%s&groupName=%s&namespaceId=%s",
                serviceName, DEFAULT_GROUP, DEFAULT_NAMESPACE));
        System.out.println("V3 Service Delete: " + response);
    }

    /**
     * V3S-006: Test V3 subscriber list
     */
    @Test
    @Order(15)
    void testV3SubscriberList() throws Exception {
        String serviceName = "test-service";
        String response = httpGet(String.format(
                "/v3/console/ns/subscriber/list?serviceName=%s&groupName=%s&namespaceId=%s&pageNo=1&pageSize=10",
                serviceName, DEFAULT_GROUP, DEFAULT_NAMESPACE));
        System.out.println("V3 Subscriber List: " + response);
    }

    /**
     * V3S-007: Test V3 selector types
     */
    @Test
    @Order(16)
    void testV3SelectorTypes() throws Exception {
        String response = httpGet("/v3/console/ns/service/selector/types");
        System.out.println("V3 Selector Types: " + response);
        assertTrue(response.contains("none") || response.contains("label"), "Should return selector types");
    }

    // ==================== V3 Auth API Tests ====================

    /**
     * V3A-001: Test V3 user search
     */
    @Test
    @Order(20)
    void testV3UserSearch() throws Exception {
        String response = httpGet("/v3/auth/user/search?pageNo=1&pageSize=10");
        System.out.println("V3 User Search: " + response);
    }

    /**
     * V3A-002: Test V3 role search
     */
    @Test
    @Order(21)
    void testV3RoleSearch() throws Exception {
        String response = httpGet("/v3/auth/role/search?pageNo=1&pageSize=10");
        System.out.println("V3 Role Search: " + response);
    }

    /**
     * V3A-003: Test V3 permission search
     */
    @Test
    @Order(22)
    void testV3PermissionSearch() throws Exception {
        String response = httpGet("/v3/auth/permission/searchPage?pageNo=1&pageSize=10");
        System.out.println("V3 Permission Search: " + response);
    }

    /**
     * V3A-004: Test V3 user CRUD
     */
    @Test
    @Order(23)
    void testV3UserCrud() throws Exception {
        String username = "v3user" + UUID.randomUUID().toString().substring(0, 4);

        // Create user
        String createBody = String.format(
                "username=%s&password=test123456",
                username);
        String createResponse = httpPost("/v3/auth/user", createBody, "application/x-www-form-urlencoded");
        System.out.println("V3 User Create: " + createResponse);

        // Delete user
        String deleteResponse = httpDelete("/v3/auth/user?username=" + username);
        System.out.println("V3 User Delete: " + deleteResponse);
    }

    // ==================== V3 Cluster/Health API Tests ====================

    /**
     * V3H-001: Test V3 health liveness
     */
    @Test
    @Order(30)
    void testV3HealthLiveness() throws Exception {
        String response = httpGet("/v3/console/health/liveness");
        System.out.println("V3 Health Liveness: " + response);
    }

    /**
     * V3H-002: Test V3 health readiness
     */
    @Test
    @Order(31)
    void testV3HealthReadiness() throws Exception {
        String response = httpGet("/v3/console/health/readiness");
        System.out.println("V3 Health Readiness: " + response);
    }

    /**
     * V3H-003: Test V3 cluster nodes
     */
    @Test
    @Order(32)
    void testV3ClusterNodes() throws Exception {
        String response = httpGet("/v3/console/cluster/nodes");
        System.out.println("V3 Cluster Nodes: " + response);
    }

    /**
     * V3H-004: Test V3 server state
     */
    @Test
    @Order(33)
    void testV3ServerState() throws Exception {
        String response = httpGet("/v3/console/server/state");
        System.out.println("V3 Server State: " + response);
    }

    /**
     * V3H-005: Test V3 metrics
     */
    @Test
    @Order(34)
    void testV3Metrics() throws Exception {
        String response = httpGet("/v3/console/metrics");
        System.out.println("V3 Metrics: " + response);
    }

    // ==================== V3 Namespace API Tests ====================

    /**
     * V3N-001: Test V3 namespace list
     */
    @Test
    @Order(40)
    void testV3NamespaceList() throws Exception {
        String response = httpGet("/v3/console/namespace/list");
        System.out.println("V3 Namespace List: " + response);
        assertNotNull(response);
    }

    /**
     * V3N-002: Test V3 namespace CRUD
     */
    @Test
    @Order(41)
    void testV3NamespaceCrud() throws Exception {
        String namespaceId = "v3ns" + UUID.randomUUID().toString().substring(0, 4);

        // Create
        String createBody = String.format(
                "namespaceId=%s&namespaceName=V3TestNamespace&namespaceDesc=Created+by+V3+test",
                namespaceId);
        String createResponse = httpPost("/v3/console/namespace", createBody, "application/x-www-form-urlencoded");
        System.out.println("V3 Namespace Create: " + createResponse);

        // Get
        String getResponse = httpGet("/v3/console/namespace?namespaceId=" + namespaceId);
        System.out.println("V3 Namespace Get: " + getResponse);

        // Delete
        String deleteResponse = httpDelete("/v3/console/namespace?namespaceId=" + namespaceId);
        System.out.println("V3 Namespace Delete: " + deleteResponse);
    }

    // ==================== V3 History API Tests ====================

    /**
     * V3HI-001: Test V3 config history list
     */
    @Test
    @Order(50)
    void testV3HistoryList() throws Exception {
        String dataId = "test-config";
        String response = httpGet(String.format(
                "/v3/console/history/list?dataId=%s&groupName=%s&namespaceId=%s&pageNo=1&pageSize=10",
                dataId, DEFAULT_GROUP, DEFAULT_NAMESPACE));
        System.out.println("V3 History List: " + response);
    }

    // ==================== V3 Instance API Tests ====================

    /**
     * V3I-001: Test V3 instance list
     */
    @Test
    @Order(60)
    void testV3InstanceList() throws Exception {
        String serviceName = "test-service";
        String response = httpGet(String.format(
                "/v3/console/ns/instance/list?serviceName=%s&groupName=%s&namespaceId=%s",
                serviceName, DEFAULT_GROUP, DEFAULT_NAMESPACE));
        System.out.println("V3 Instance List: " + response);
    }
}
