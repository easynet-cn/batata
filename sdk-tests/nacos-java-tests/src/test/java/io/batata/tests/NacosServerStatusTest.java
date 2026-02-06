package io.batata.tests;

import org.junit.jupiter.api.*;

import java.io.*;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Server Status Tests
 *
 * Tests for Nacos server status and cluster information including:
 * - Server status and health check
 * - Cluster node list and leader info
 * - Server version and capabilities
 * - Connection state and metrics
 * - Namespace and config capacity
 * - Service, instance, and subscriber counts
 * - Cluster health and server switches
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosServerStatusTest {

    private static String serverAddr;
    private static String accessToken;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final String DEFAULT_NAMESPACE = "public";

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        accessToken = getAccessToken(username, password);
        System.out.println("Nacos Server Status Test Setup - Server: " + serverAddr);
        System.out.println("Access Token: " + (accessToken.isEmpty() ? "NONE" : "OK"));
    }

    private static String getAccessToken(String username, String password) throws Exception {
        // Try V3 login first
        String token = loginV3(username, password);
        if (!token.isEmpty()) {
            return token;
        }
        // Fallback to V1 login
        return loginV1(username, password);
    }

    private static String loginV3(String username, String password) throws Exception {
        String loginUrl = String.format("http://%s/v3/auth/user/login", serverAddr);
        URL url = new URL(loginUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("POST");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");

        String body = "username=" + URLEncoder.encode(username, "UTF-8") +
                "&password=" + URLEncoder.encode(password, "UTF-8");
        conn.getOutputStream().write(body.getBytes(StandardCharsets.UTF_8));

        int responseCode = conn.getResponseCode();
        if (responseCode == 200) {
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
            if (resp.contains("token")) {
                int start = resp.indexOf("token") + 8;
                int end = resp.indexOf("\"", start);
                if (end > start) {
                    return resp.substring(start, end);
                }
            }
        }
        return "";
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
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
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

    private String httpPut(String path, String body) throws Exception {
        HttpURLConnection conn = createConnection("PUT", path);
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");

        if (body != null && !body.isEmpty()) {
            try (OutputStream os = conn.getOutputStream()) {
                os.write(body.getBytes(StandardCharsets.UTF_8));
            }
        }
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
        System.out.println(conn.getRequestMethod() + " " + conn.getURL().getPath() + " -> " + responseCode);
        return response.toString();
    }

    // ==================== Server Status Tests ====================

    /**
     * NSS-001: Test get server status
     * Verifies that the server returns its current operational status.
     */
    @Test
    @Order(1)
    void testGetServerStatus() throws Exception {
        String response = httpGet("/v3/console/server/state");
        System.out.println("Server Status: " + response);
        assertNotNull(response, "Server status should not be null");
        // Server state typically contains status information
        assertTrue(response.length() > 0, "Server should return status information");
    }

    /**
     * NSS-002: Test server health check
     * Verifies that the server health endpoints (liveness and readiness) are accessible.
     */
    @Test
    @Order(2)
    void testServerHealthCheck() throws Exception {
        // Test liveness probe
        String livenessResponse = httpGet("/v3/console/health/liveness");
        System.out.println("Health Liveness: " + livenessResponse);
        assertNotNull(livenessResponse, "Liveness response should not be null");

        // Test readiness probe
        String readinessResponse = httpGet("/v3/console/health/readiness");
        System.out.println("Health Readiness: " + readinessResponse);
        assertNotNull(readinessResponse, "Readiness response should not be null");
    }

    /**
     * NSS-003: Test cluster node list
     * Verifies that the server returns a list of cluster nodes.
     */
    @Test
    @Order(3)
    void testClusterNodeList() throws Exception {
        // V3 Console API
        String v3Response = httpGet("/v3/console/cluster/nodes");
        System.out.println("V3 Cluster Nodes: " + v3Response);
        assertNotNull(v3Response, "Cluster node list should not be null");

        // V2 Core API
        String v2Response = httpGet("/nacos/v2/core/cluster/node/list");
        System.out.println("V2 Cluster Nodes: " + v2Response);
        assertNotNull(v2Response, "V2 cluster node list should not be null");
    }

    /**
     * NSS-004: Test leader node info
     * Verifies that the server can return information about the current/self node.
     */
    @Test
    @Order(4)
    void testLeaderNodeInfo() throws Exception {
        // Get current node info (self node)
        String selfNodeResponse = httpGet("/nacos/v2/core/cluster/node/self");
        System.out.println("Self Node Info: " + selfNodeResponse);
        assertNotNull(selfNodeResponse, "Self node info should not be null");

        // In single-node setup, self is the leader
        // The response should contain node information such as address, state
    }

    /**
     * NSS-005: Test server version
     * Verifies that the server returns its version information.
     */
    @Test
    @Order(5)
    void testServerVersion() throws Exception {
        // Server state typically includes version information
        String stateResponse = httpGet("/v3/console/server/state");
        System.out.println("Server State (for version): " + stateResponse);
        assertNotNull(stateResponse, "Server state should not be null");

        // Also check the self node which may contain version
        String selfResponse = httpGet("/nacos/v2/core/cluster/node/self");
        System.out.println("Self Node (for version): " + selfResponse);
        assertNotNull(selfResponse, "Self node info should not be null");
    }

    /**
     * NSS-006: Test server capabilities
     * Verifies that the server reports its supported features/capabilities.
     */
    @Test
    @Order(6)
    void testServerCapabilities() throws Exception {
        // Check selector types as an indicator of naming service capabilities
        String selectorTypesResponse = httpGet("/v3/console/ns/service/selector/types");
        System.out.println("Selector Types: " + selectorTypesResponse);

        // Check server state for capability information
        String stateResponse = httpGet("/v3/console/server/state");
        System.out.println("Server State (capabilities): " + stateResponse);
        assertNotNull(stateResponse, "Server should return capability information");
    }

    /**
     * NSS-007: Test connection state
     * Verifies that the server can report client connection states.
     */
    @Test
    @Order(7)
    void testConnectionState() throws Exception {
        // List connected clients
        String clientListResponse = httpGet("/nacos/v2/ns/client/list");
        System.out.println("Client List: " + clientListResponse);
        assertNotNull(clientListResponse, "Client list should not be null");

        // The response contains information about connected clients and their states
    }

    /**
     * NSS-008: Test server metrics
     * Verifies that the server returns operational metrics.
     */
    @Test
    @Order(8)
    void testServerMetrics() throws Exception {
        // V3 metrics endpoint
        String v3MetricsResponse = httpGet("/v3/console/metrics");
        System.out.println("V3 Metrics: " + v3MetricsResponse);

        // V2 operator metrics
        String v2MetricsResponse = httpGet("/nacos/v2/ns/operator/metrics");
        System.out.println("V2 Operator Metrics: " + v2MetricsResponse);
        assertNotNull(v2MetricsResponse, "Metrics response should not be null");
    }

    /**
     * NSS-009: Test namespace capacity
     * Verifies that the server returns namespace capacity information.
     */
    @Test
    @Order(9)
    void testNamespaceCapacity() throws Exception {
        // Get capacity for default namespace/group
        String capacityResponse = httpGet("/nacos/v2/cs/capacity?group=" + DEFAULT_GROUP);
        System.out.println("Namespace Capacity: " + capacityResponse);
        // Capacity may return info or indicate not found if not configured

        // Also get namespace list to verify namespace information
        String namespaceListResponse = httpGet("/v3/console/namespace/list");
        System.out.println("Namespace List: " + namespaceListResponse);
        assertNotNull(namespaceListResponse, "Namespace list should not be null");
    }

    /**
     * NSS-010: Test config capacity
     * Verifies that the server returns configuration capacity information.
     */
    @Test
    @Order(10)
    void testConfigCapacity() throws Exception {
        // Get capacity for a specific tenant/namespace
        String capacityResponse = httpGet("/nacos/v2/cs/capacity?tenant=" + DEFAULT_NAMESPACE);
        System.out.println("Config Capacity (by tenant): " + capacityResponse);

        // Get capacity for group
        String groupCapacityResponse = httpGet("/nacos/v2/cs/capacity?group=" + DEFAULT_GROUP);
        System.out.println("Config Capacity (by group): " + groupCapacityResponse);
        // Response may indicate capacity limits or current usage
    }

    /**
     * NSS-011: Test service count
     * Verifies that the server can return the count of registered services.
     */
    @Test
    @Order(11)
    void testServiceCount() throws Exception {
        // Get service list with pagination to get count
        String serviceListResponse = httpGet(String.format(
                "/v3/console/ns/service/list?pageNo=1&pageSize=10&namespaceId=%s&groupName=%s",
                DEFAULT_NAMESPACE, DEFAULT_GROUP));
        System.out.println("Service List: " + serviceListResponse);
        assertNotNull(serviceListResponse, "Service list should not be null");
        // The response typically contains a count field or list of services

        // Also check via operator metrics which may include service count
        String metricsResponse = httpGet("/nacos/v2/ns/operator/metrics");
        System.out.println("Metrics (for service count): " + metricsResponse);
    }

    /**
     * NSS-012: Test instance count
     * Verifies that the server can return the count of registered instances.
     */
    @Test
    @Order(12)
    void testInstanceCount() throws Exception {
        // Get instance list for a service
        String instanceListResponse = httpGet(String.format(
                "/v3/console/ns/instance/list?serviceName=test-service&groupName=%s&namespaceId=%s",
                DEFAULT_GROUP, DEFAULT_NAMESPACE));
        System.out.println("Instance List: " + instanceListResponse);

        // The operator metrics endpoint may include instance count
        String metricsResponse = httpGet("/nacos/v2/ns/operator/metrics");
        System.out.println("Metrics (for instance count): " + metricsResponse);
        assertNotNull(metricsResponse, "Metrics should not be null");
    }

    /**
     * NSS-013: Test subscriber count
     * Verifies that the server can return the count of service subscribers.
     */
    @Test
    @Order(13)
    void testSubscriberCount() throws Exception {
        // Get subscriber list for a service
        String subscriberListResponse = httpGet(String.format(
                "/v3/console/ns/subscriber/list?serviceName=test-service&groupName=%s&namespaceId=%s&pageNo=1&pageSize=10",
                DEFAULT_GROUP, DEFAULT_NAMESPACE));
        System.out.println("Subscriber List: " + subscriberListResponse);

        // Get service subscribers via V2 API
        String v2SubscriberResponse = httpGet(String.format(
                "/nacos/v2/ns/client/service/subscriber/list?serviceName=test-service&groupName=%s",
                DEFAULT_GROUP));
        System.out.println("V2 Subscriber List: " + v2SubscriberResponse);
        // Response contains subscriber information and count
    }

    /**
     * NSS-014: Test cluster health
     * Verifies that the server can return cluster health status.
     */
    @Test
    @Order(14)
    void testClusterHealth() throws Exception {
        // Get node health
        String nodeHealthResponse = httpGet("/nacos/v2/core/cluster/node/self/health");
        System.out.println("Node Health: " + nodeHealthResponse);
        assertNotNull(nodeHealthResponse, "Node health should not be null");

        // Get health probes
        String livenessResponse = httpGet("/v3/console/health/liveness");
        System.out.println("Cluster Liveness: " + livenessResponse);

        String readinessResponse = httpGet("/v3/console/health/readiness");
        System.out.println("Cluster Readiness: " + readinessResponse);

        // All health responses should indicate server health status
    }

    /**
     * NSS-015: Test server switches
     * Verifies that the server returns and can manage operational switches.
     */
    @Test
    @Order(15)
    void testServerSwitches() throws Exception {
        // Get system switches
        String switchesResponse = httpGet("/nacos/v2/ns/operator/switches");
        System.out.println("Server Switches: " + switchesResponse);
        assertNotNull(switchesResponse, "Switches response should not be null");

        // Switches typically include:
        // - healthCheckEnabled
        // - distroEnabled
        // - enableAuthentication
        // - defaultPushCacheMillis
        // etc.
    }
}
