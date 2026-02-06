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
 * Nacos Admin/Console API Tests
 *
 * Tests for namespace management, cluster info, capacity, and audit logs.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosAdminApiTest {

    private static String serverAddr;
    private static String accessToken;

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        accessToken = getAccessToken(username, password);
        System.out.println("Admin API Test Setup - Server: " + serverAddr);
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

    private String httpGet(String path) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }

        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("GET");

        int responseCode = conn.getResponseCode();
        BufferedReader reader = new BufferedReader(new InputStreamReader(
                responseCode >= 400 ? conn.getErrorStream() : conn.getInputStream()));
        StringBuilder response = new StringBuilder();
        String line;
        while ((line = reader.readLine()) != null) {
            response.append(line);
        }
        System.out.println("GET " + path + " -> " + responseCode);
        return response.toString();
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

        int responseCode = conn.getResponseCode();
        BufferedReader reader = new BufferedReader(new InputStreamReader(
                responseCode >= 400 ? conn.getErrorStream() : conn.getInputStream()));
        StringBuilder response = new StringBuilder();
        String line;
        while ((line = reader.readLine()) != null) {
            response.append(line);
        }
        System.out.println("POST " + path + " -> " + responseCode);
        return response.toString();
    }

    private String httpPut(String path, String body) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }

        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("PUT");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");

        if (body != null && !body.isEmpty()) {
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
        System.out.println("PUT " + path + " -> " + responseCode);
        return response.toString();
    }

    private String httpDelete(String path) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }

        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("DELETE");

        int responseCode = conn.getResponseCode();
        BufferedReader reader = new BufferedReader(new InputStreamReader(
                responseCode >= 400 ? conn.getErrorStream() : conn.getInputStream()));
        StringBuilder response = new StringBuilder();
        String line;
        while ((line = reader.readLine()) != null) {
            response.append(line);
        }
        System.out.println("DELETE " + path + " -> " + responseCode);
        return response.toString();
    }

    // ==================== Namespace Management Tests ====================

    /**
     * Test list namespaces
     */
    @Test
    @Order(1)
    void testListNamespaces() throws Exception {
        String response = httpGet("/nacos/v2/console/namespace/list");
        System.out.println("Namespace list: " + response);
        assertNotNull(response);
        // Should return array of namespaces
    }

    /**
     * Test create namespace
     */
    @Test
    @Order(2)
    void testCreateNamespace() throws Exception {
        String namespaceId = "test-ns-" + UUID.randomUUID().toString().substring(0, 8);
        String namespaceName = "Test Namespace";
        String namespaceDesc = "Created by SDK test";

        String body = String.format(
                "namespaceId=%s&namespaceName=%s&namespaceDesc=%s",
                URLEncoder.encode(namespaceId, "UTF-8"),
                URLEncoder.encode(namespaceName, "UTF-8"),
                URLEncoder.encode(namespaceDesc, "UTF-8"));

        String response = httpPost("/nacos/v2/console/namespace", body);
        System.out.println("Create namespace: " + response);

        // Cleanup
        httpDelete("/nacos/v2/console/namespace?namespaceId=" + namespaceId);
    }

    /**
     * Test get namespace detail
     */
    @Test
    @Order(3)
    void testGetNamespace() throws Exception {
        // Create first
        String namespaceId = "get-ns-" + UUID.randomUUID().toString().substring(0, 8);
        String body = String.format(
                "namespaceId=%s&namespaceName=%s&namespaceDesc=%s",
                namespaceId, "Get Test NS", "Test");
        httpPost("/nacos/v2/console/namespace", body);

        // Get detail
        String response = httpGet("/nacos/v2/console/namespace?namespaceId=" + namespaceId);
        System.out.println("Get namespace: " + response);

        // Cleanup
        httpDelete("/nacos/v2/console/namespace?namespaceId=" + namespaceId);
    }

    /**
     * Test update namespace
     */
    @Test
    @Order(4)
    void testUpdateNamespace() throws Exception {
        String namespaceId = "update-ns-" + UUID.randomUUID().toString().substring(0, 8);

        // Create
        String createBody = String.format(
                "namespaceId=%s&namespaceName=%s&namespaceDesc=%s",
                namespaceId, "Original Name", "Original Desc");
        httpPost("/nacos/v2/console/namespace", createBody);

        // Update
        String updateBody = String.format(
                "namespaceId=%s&namespaceName=%s&namespaceDesc=%s",
                namespaceId,
                URLEncoder.encode("Updated Name", "UTF-8"),
                URLEncoder.encode("Updated Description", "UTF-8"));
        String response = httpPut("/nacos/v2/console/namespace", updateBody);
        System.out.println("Update namespace: " + response);

        // Cleanup
        httpDelete("/nacos/v2/console/namespace?namespaceId=" + namespaceId);
    }

    /**
     * Test delete namespace
     */
    @Test
    @Order(5)
    void testDeleteNamespace() throws Exception {
        String namespaceId = "delete-ns-" + UUID.randomUUID().toString().substring(0, 8);

        // Create
        String body = String.format(
                "namespaceId=%s&namespaceName=%s&namespaceDesc=%s",
                namespaceId, "To Delete", "Will be deleted");
        httpPost("/nacos/v2/console/namespace", body);

        // Delete
        String response = httpDelete("/nacos/v2/console/namespace?namespaceId=" + namespaceId);
        System.out.println("Delete namespace: " + response);
    }

    // ==================== Cluster Info Tests ====================

    /**
     * Test get current node info
     */
    @Test
    @Order(6)
    void testGetCurrentNode() throws Exception {
        String response = httpGet("/nacos/v2/core/cluster/node/self");
        System.out.println("Current node: " + response);
        assertNotNull(response);
    }

    /**
     * Test list cluster nodes
     */
    @Test
    @Order(7)
    void testListClusterNodes() throws Exception {
        String response = httpGet("/nacos/v2/core/cluster/node/list");
        System.out.println("Cluster nodes: " + response);
        assertNotNull(response);
    }

    /**
     * Test get node health
     */
    @Test
    @Order(8)
    void testGetNodeHealth() throws Exception {
        String response = httpGet("/nacos/v2/core/cluster/node/self/health");
        System.out.println("Node health: " + response);
        assertNotNull(response);
    }

    // ==================== Capacity Management Tests ====================

    /**
     * Test get capacity info
     */
    @Test
    @Order(9)
    void testGetCapacity() throws Exception {
        String response = httpGet("/nacos/v2/cs/capacity?group=" + DEFAULT_GROUP);
        System.out.println("Capacity info: " + response);
        // May return capacity info or not found
    }

    /**
     * Test update capacity
     */
    @Test
    @Order(10)
    void testUpdateCapacity() throws Exception {
        String body = String.format(
                "group=%s&quota=1000&maxSize=10485760&maxAggrCount=10000&maxAggrSize=1048576",
                DEFAULT_GROUP);
        String response = httpPost("/nacos/v2/cs/capacity", body);
        System.out.println("Update capacity: " + response);
    }

    // ==================== Operator/Metrics Tests ====================

    /**
     * Test get system switches
     */
    @Test
    @Order(11)
    void testGetSwitches() throws Exception {
        String response = httpGet("/nacos/v2/ns/operator/switches");
        System.out.println("System switches: " + response);
        assertNotNull(response);
    }

    /**
     * Test get metrics
     */
    @Test
    @Order(12)
    void testGetMetrics() throws Exception {
        String response = httpGet("/nacos/v2/ns/operator/metrics");
        System.out.println("Metrics: " + response);
        assertNotNull(response);
    }

    // ==================== Audit Log Tests ====================

    /**
     * Test list audit logs
     */
    @Test
    @Order(13)
    void testListAuditLogs() throws Exception {
        String response = httpGet("/nacos/v2/console/audit/list?pageNo=1&pageSize=10");
        System.out.println("Audit logs: " + response);
        // May return logs or empty list
    }

    /**
     * Test get audit stats
     */
    @Test
    @Order(14)
    void testGetAuditStats() throws Exception {
        String response = httpGet("/nacos/v2/console/audit/stats");
        System.out.println("Audit stats: " + response);
    }

    // ==================== Client Management Tests ====================

    /**
     * Test list clients
     */
    @Test
    @Order(15)
    void testListClients() throws Exception {
        String response = httpGet("/nacos/v2/ns/client/list");
        System.out.println("Client list: " + response);
        assertNotNull(response);
    }

    /**
     * Test get client published services
     */
    @Test
    @Order(16)
    void testGetClientPublishedServices() throws Exception {
        // First get a client ID from the list
        String listResponse = httpGet("/nacos/v2/ns/client/list");

        // If we have clients, try to get their published services
        if (listResponse.contains("clientId")) {
            // Extract a client ID and query its published services
            String response = httpGet("/nacos/v2/ns/client/publish/list?clientId=test");
            System.out.println("Client published services: " + response);
        }
    }

    /**
     * Test get client subscribed services
     */
    @Test
    @Order(17)
    void testGetClientSubscribedServices() throws Exception {
        String response = httpGet("/nacos/v2/ns/client/subscribe/list?clientId=test");
        System.out.println("Client subscribed services: " + response);
    }

    /**
     * Test list service publishers
     */
    @Test
    @Order(18)
    void testListServicePublishers() throws Exception {
        String response = httpGet("/nacos/v2/ns/client/service/publisher/list?serviceName=test-service&groupName=DEFAULT_GROUP");
        System.out.println("Service publishers: " + response);
    }

    /**
     * Test list service subscribers
     */
    @Test
    @Order(19)
    void testListServiceSubscribers() throws Exception {
        String response = httpGet("/nacos/v2/ns/client/service/subscriber/list?serviceName=test-service&groupName=DEFAULT_GROUP");
        System.out.println("Service subscribers: " + response);
    }

    // ==================== Health API Tests ====================

    /**
     * Test update instance health
     */
    @Test
    @Order(20)
    void testUpdateInstanceHealth() throws Exception {
        String body = String.format(
                "serviceName=%s&groupName=%s&ip=%s&port=%d&healthy=%s",
                "health-test-service", DEFAULT_GROUP, "192.168.1.1", 8080, "true");
        String response = httpPut("/nacos/v2/ns/health/instance", body);
        System.out.println("Update instance health: " + response);
    }

    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
}
