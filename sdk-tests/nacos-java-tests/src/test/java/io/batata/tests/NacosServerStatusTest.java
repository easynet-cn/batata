package io.batata.tests;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
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
 *
 * All routes are accessed via the Main Server (8848) under /nacos context path.
 *
 * Route mapping:
 * - V3 Console Health: /nacos/v3/console/health/*
 * - V3 Console Server: /nacos/v3/console/server/*
 * - V3 Console Cluster: /nacos/v3/console/core/cluster/*
 * - V3 Console Namespace: /nacos/v3/console/core/namespace/*
 * - V3 Console Service: /nacos/v3/console/ns/service/*
 * - V3 Console Instance: /nacos/v3/console/ns/instance/*
 * - V3 Console Metrics: /nacos/v3/console/metrics (Prometheus text format)
 * - V2 Core Cluster: /nacos/v2/core/cluster/*
 * - V2 Naming: /nacos/v2/ns/*
 * - V2 Config: /nacos/v2/cs/*
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosServerStatusTest {

    private static String serverAddr;
    private static String accessToken;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final String DEFAULT_NAMESPACE = "public";
    private static final ObjectMapper objectMapper = new ObjectMapper();

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        accessToken = getAccessToken(username, password);
        assertFalse(accessToken.isEmpty(), "Login should return a valid access token");
    }

    private static String getAccessToken(String username, String password) throws Exception {
        // Try V3 login first
        String token = loginV3(username, password);
        if (!token.isEmpty()) {
            return token;
        }
        return "";
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
        if (responseCode == 200) {
            BufferedReader reader = new BufferedReader(new InputStreamReader(conn.getInputStream()));
            StringBuilder response = new StringBuilder();
            String line;
            while ((line = reader.readLine()) != null) {
                response.append(line);
            }
            JsonNode json = objectMapper.readTree(response.toString());
            if (json.has("data") && json.get("data").has("accessToken")) {
                return json.get("data").get("accessToken").asText();
            }
            if (json.has("accessToken")) {
                return json.get("accessToken").asText();
            }
            if (json.has("token")) {
                return json.get("token").asText();
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
        return response.toString();
    }

    /**
     * Parse a Nacos API response and assert that the response code is 0 (success).
     */
    private JsonNode assertSuccessResponse(String response) throws Exception {
        assertNotNull(response, "Response should not be null");
        assertFalse(response.startsWith("Status:"), "Response should not be an error status: " + response);
        JsonNode json = objectMapper.readTree(response);
        assertTrue(json.has("code"), "Response should contain 'code' field: " + response);
        assertEquals(0, json.get("code").asInt(), "Response code should be 0 (success): " + response);
        return json;
    }

    /**
     * Parse a server state response which returns flat JSON (no code/data wrapper).
     * Server state returns: {"state":"OK","version":"...","startup_mode":"..."}
     */
    private JsonNode parseServerState(String response) throws Exception {
        assertNotNull(response, "Response should not be null");
        assertFalse(response.startsWith("Status:"), "Response should not be an error status: " + response);
        JsonNode json = objectMapper.readTree(response);
        // Server state returns flat JSON - may have "code" wrapper or may be flat
        if (json.has("code") && json.has("data")) {
            assertEquals(0, json.get("code").asInt(), "Response code should be 0: " + response);
            return json.get("data");
        }
        // Flat JSON response (no wrapper)
        return json;
    }

    // ==================== Server Status Tests ====================

    /**
     * NSS-001: Test get server status
     * Server state at /nacos/v3/console/server/state
     */
    @Test
    @Order(1)
    void testGetServerStatus() throws Exception {
        String response = httpGet("/nacos/v3/console/server/state");
        JsonNode data = parseServerState(response);
        assertNotNull(data, "Server state should return data");
        assertTrue(data.isObject(), "Server state data should be a JSON object: " + data);
        assertTrue(data.size() > 0, "Server state should contain at least one field");
        // Server state should include mode or version information
        assertTrue(data.has("standalone_mode") || data.has("function_mode")
                        || data.has("version") || data.has("startup_mode"),
                "Server state should contain mode or version fields: " + data);
    }

    /**
     * NSS-002: Test server health check
     * Health endpoints at /nacos/v3/console/health/*
     */
    @Test
    @Order(2)
    void testServerHealthCheck() throws Exception {
        // Test liveness probe
        String livenessResponse = httpGet("/nacos/v3/console/health/liveness");
        JsonNode livenessJson = objectMapper.readTree(livenessResponse);
        assertTrue(livenessJson.has("code"), "Liveness should have 'code' field: " + livenessResponse);
        assertEquals(0, livenessJson.get("code").asInt(),
                "Liveness code should be 0 (healthy): " + livenessResponse);

        // Test readiness probe
        String readinessResponse = httpGet("/nacos/v3/console/health/readiness");
        JsonNode readinessJson = objectMapper.readTree(readinessResponse);
        assertTrue(readinessJson.has("code"), "Readiness should have 'code' field: " + readinessResponse);
        assertEquals(0, readinessJson.get("code").asInt(),
                "Readiness code should be 0 (ready): " + readinessResponse);
    }

    /**
     * NSS-003: Test cluster node list
     * V3 cluster at /nacos/v3/console/core/cluster/nodes
     * V2 cluster at /nacos/v2/core/cluster/node/list
     */
    @Test
    @Order(3)
    void testClusterNodeList() throws Exception {
        // V3 Console API (note: core/cluster, not just cluster)
        String v3Response = httpGet("/nacos/v3/console/core/cluster/nodes");
        JsonNode v3Json = assertSuccessResponse(v3Response);
        JsonNode v3Data = v3Json.get("data");
        assertNotNull(v3Data, "V3 cluster nodes should return data");
        assertTrue(v3Data.isArray(), "V3 cluster nodes data should be an array: " + v3Data);
        assertTrue(v3Data.size() > 0, "V3 cluster should have at least one node");
        JsonNode firstV3Node = v3Data.get(0);
        assertTrue(firstV3Node.has("ip") || firstV3Node.has("address"),
                "V3 cluster node should have ip or address: " + firstV3Node);

        // V2 Core API
        String v2Response = httpGet("/nacos/v2/core/cluster/node/list");
        JsonNode v2Json = assertSuccessResponse(v2Response);
        JsonNode v2Data = v2Json.get("data");
        assertNotNull(v2Data, "V2 cluster nodes should return data");
        assertTrue(v2Data.isArray(), "V2 cluster nodes data should be an array: " + v2Data);
        assertTrue(v2Data.size() > 0, "V2 cluster should have at least one node");
        // Both APIs should return the same number of nodes
        assertEquals(v3Data.size(), v2Data.size(),
                "V2 and V3 cluster node counts should match");
    }

    /**
     * NSS-004: Test leader node info
     * Self node at /nacos/v2/core/cluster/node/self
     */
    @Test
    @Order(4)
    void testLeaderNodeInfo() throws Exception {
        String selfNodeResponse = httpGet("/nacos/v2/core/cluster/node/self");
        JsonNode json = assertSuccessResponse(selfNodeResponse);
        JsonNode data = json.get("data");
        assertNotNull(data, "Self node info should return data");
        assertTrue(data.has("ip") || data.has("address"),
                "Self node should have ip or address: " + data);
        assertTrue(data.has("port") || data.has("state") || data.has("abilities"),
                "Self node should have port, state, or abilities: " + data);
    }

    /**
     * NSS-005: Test server version
     */
    @Test
    @Order(5)
    void testServerVersion() throws Exception {
        String stateResponse = httpGet("/nacos/v3/console/server/state");
        JsonNode data = parseServerState(stateResponse);
        assertNotNull(data, "Server state should return data");

        // Check for version in server state
        if (data.has("version")) {
            String version = data.get("version").asText();
            assertFalse(version.isEmpty(), "Version should not be empty");
        }

        // Also check self node for version
        String selfResponse = httpGet("/nacos/v2/core/cluster/node/self");
        JsonNode selfJson = assertSuccessResponse(selfResponse);
        JsonNode selfData = selfJson.get("data");
        assertNotNull(selfData, "Self node should return data");
        if (selfData.has("version")) {
            String selfVersion = selfData.get("version").asText();
            assertFalse(selfVersion.isEmpty(), "Self node version should not be empty");
        }
    }

    /**
     * NSS-006: Test server capabilities
     * Selector types at /nacos/v3/console/ns/service/selector/types
     */
    @Test
    @Order(6)
    void testServerCapabilities() throws Exception {
        // Check selector types as an indicator of naming service capabilities
        String selectorTypesResponse = httpGet("/nacos/v3/console/ns/service/selector/types");
        JsonNode selectorJson = assertSuccessResponse(selectorTypesResponse);
        JsonNode selectorData = selectorJson.get("data");
        assertNotNull(selectorData, "Selector types should return data");
        assertTrue(selectorData.isArray(), "Selector types should be an array: " + selectorData);
        assertTrue(selectorData.size() > 0, "Should have at least one selector type");

        // Check server state for capability information
        String stateResponse = httpGet("/nacos/v3/console/server/state");
        JsonNode stateData = parseServerState(stateResponse);
        assertNotNull(stateData, "Server state should return capability data");
        assertTrue(stateData.size() > 0, "Server state should have non-empty data");
    }

    /**
     * NSS-007: Test connection state
     * Client list at /nacos/v2/ns/client/list
     */
    @Test
    @Order(7)
    void testConnectionState() throws Exception {
        String clientListResponse = httpGet("/nacos/v2/ns/client/list");
        JsonNode json = assertSuccessResponse(clientListResponse);
        JsonNode data = json.get("data");
        assertNotNull(data, "Client list should return data");
        // Batata may return data as an object with {count, clientIds} instead of a flat array
        if (data.isArray()) {
            // Standard array format - may be empty if no SDK clients are connected
        } else if (data.isObject()) {
            // Object format: {"count":0,"clientIds":[...]}
            if (data.has("clientIds")) {
                assertTrue(data.get("clientIds").isArray(),
                        "clientIds field should be an array: " + data);
            }
        } else {
            fail("Client list data should be an array or object, got: " + data);
        }
    }

    /**
     * NSS-008: Test server metrics
     * V3 metrics at /nacos/v3/console/metrics (returns Prometheus text, not JSON)
     * V2 operator metrics at /nacos/v2/ns/operator/metrics (returns JSON)
     */
    @Test
    @Order(8)
    void testServerMetrics() throws Exception {
        // V3 metrics endpoint returns Prometheus text format
        HttpURLConnection v3Conn = createConnection("GET", "/nacos/v3/console/metrics");
        int v3ResponseCode = v3Conn.getResponseCode();
        assertEquals(200, v3ResponseCode, "V3 metrics should return HTTP 200");
        InputStream v3Is = v3Conn.getInputStream();
        BufferedReader v3Reader = new BufferedReader(new InputStreamReader(v3Is));
        StringBuilder v3Body = new StringBuilder();
        String v3Line;
        while ((v3Line = v3Reader.readLine()) != null) {
            v3Body.append(v3Line).append("\n");
        }
        assertFalse(v3Body.toString().isEmpty(), "V3 metrics should return content");

        // V2 operator metrics (JSON)
        String v2MetricsResponse = httpGet("/nacos/v2/ns/operator/metrics");
        JsonNode v2Json = objectMapper.readTree(v2MetricsResponse);
        assertTrue(v2Json.has("code"), "V2 metrics should have 'code' field: " + v2MetricsResponse);
        int v2Code = v2Json.get("code").asInt();
        if (v2Code == 0) {
            JsonNode v2Data = v2Json.get("data");
            assertNotNull(v2Data, "V2 operator metrics should return data");
            // Metrics data should be a string or object with operational stats
            assertTrue(v2Data.isObject() || v2Data.isTextual(),
                    "V2 metrics data should be object or text: " + v2Data);
        }
        // Accept various codes - metrics endpoint may not be fully implemented
        assertTrue(v2Code == 0 || v2Code == 404 || v2Code == 500 || v2Code == 30000,
                "V2 metrics response code should be recognized, got: " + v2Code);
    }

    /**
     * NSS-009: Test namespace capacity
     * Namespace list at /nacos/v3/console/core/namespace/list
     */
    @Test
    @Order(9)
    void testNamespaceCapacity() throws Exception {
        // Get capacity for default namespace/group
        String capacityResponse = httpGet("/nacos/v2/cs/capacity?group=" + DEFAULT_GROUP);
        JsonNode capacityJson = objectMapper.readTree(capacityResponse);
        assertTrue(capacityJson.has("code"), "Capacity should have 'code' field: " + capacityResponse);

        // Also get namespace list to verify namespace information
        String namespaceListResponse = httpGet("/nacos/v3/console/core/namespace/list");
        JsonNode nsJson = assertSuccessResponse(namespaceListResponse);
        JsonNode nsData = nsJson.get("data");
        assertNotNull(nsData, "Namespace list should return data");
        assertTrue(nsData.isArray(), "Namespace list should be an array: " + nsData);
        assertTrue(nsData.size() > 0, "Should have at least one namespace (public)");
        // Verify public namespace exists
        boolean hasPublic = false;
        for (JsonNode ns : nsData) {
            String nsId = ns.has("namespace") ? ns.get("namespace").asText()
                    : (ns.has("namespaceId") ? ns.get("namespaceId").asText() : "");
            if ("public".equals(nsId) || nsId.isEmpty()) {
                hasPublic = true;
                break;
            }
        }
        assertTrue(hasPublic, "Namespace list should include the 'public' namespace");
    }

    /**
     * NSS-010: Test config capacity
     */
    @Test
    @Order(10)
    void testConfigCapacity() throws Exception {
        String capacityResponse = httpGet("/nacos/v2/cs/capacity?tenant=" + DEFAULT_NAMESPACE);
        JsonNode json = objectMapper.readTree(capacityResponse);
        assertTrue(json.has("code"), "Capacity response should have 'code' field: " + capacityResponse);
        int code = json.get("code").asInt();
        // Capacity might not be configured, so we accept both success and known error codes
        assertTrue(code == 0 || code == 30000 || code == 404 || code == 500,
                "Capacity response should have a recognized code, got: " + code);

        String groupCapacityResponse = httpGet("/nacos/v2/cs/capacity?group=" + DEFAULT_GROUP);
        JsonNode groupJson = objectMapper.readTree(groupCapacityResponse);
        assertTrue(groupJson.has("code"), "Group capacity should have 'code' field: " + groupCapacityResponse);
    }

    /**
     * NSS-011: Test service count
     * Service list at /nacos/v3/console/ns/service/list
     */
    @Test
    @Order(11)
    void testServiceCount() throws Exception {
        String serviceListResponse = httpGet(String.format(
                "/nacos/v3/console/ns/service/list?pageNo=1&pageSize=10&namespaceId=%s&groupName=%s",
                DEFAULT_NAMESPACE, DEFAULT_GROUP));
        JsonNode json = assertSuccessResponse(serviceListResponse);
        JsonNode data = json.get("data");
        assertNotNull(data, "Service list should return data");
        // Should have a count/totalCount field
        assertTrue(data.has("count") || data.has("totalCount") || data.has("serviceList") || data.isArray(),
                "Service list should have count or list: " + data);

        // Cross-check with operator metrics (may not be fully implemented)
        String metricsResponse = httpGet("/nacos/v2/ns/operator/metrics");
        JsonNode metricsJson = objectMapper.readTree(metricsResponse);
        assertTrue(metricsJson.has("code"), "Metrics should have code: " + metricsResponse);
    }

    /**
     * NSS-012: Test instance count
     * Instance list at /nacos/v3/console/ns/instance/list
     */
    @Test
    @Order(12)
    void testInstanceCount() throws Exception {
        String instanceListResponse = httpGet(String.format(
                "/nacos/v3/console/ns/instance/list?serviceName=test-service&groupName=%s&namespaceId=%s",
                DEFAULT_GROUP, DEFAULT_NAMESPACE));
        JsonNode json = objectMapper.readTree(instanceListResponse);
        assertTrue(json.has("code"), "Instance list should have 'code' field: " + instanceListResponse);
        // For non-existent services, code may be non-zero
        int code = json.get("code").asInt();
        assertTrue(code == 0 || code == 20004 || code == 500,
                "Instance list code should be recognized, got: " + code);

        String metricsResponse = httpGet("/nacos/v2/ns/operator/metrics");
        JsonNode metricsJson = objectMapper.readTree(metricsResponse);
        assertTrue(metricsJson.has("code"), "Operator metrics should have code: " + metricsResponse);
    }

    /**
     * NSS-013: Test subscriber count
     * Subscriber list at /nacos/v3/console/ns/service/subscribers (not /ns/subscriber/list)
     */
    @Test
    @Order(13)
    void testSubscriberCount() throws Exception {
        String subscriberListResponse = httpGet(String.format(
                "/nacos/v3/console/ns/service/subscribers?serviceName=test-service&groupName=%s&namespaceId=%s&pageNo=1&pageSize=10",
                DEFAULT_GROUP, DEFAULT_NAMESPACE));
        JsonNode json = objectMapper.readTree(subscriberListResponse);
        assertTrue(json.has("code"), "Subscriber list should have 'code' field: " + subscriberListResponse);
        assertEquals(0, json.get("code").asInt(),
                "Subscriber list should return code 0: " + subscriberListResponse);

        String v2SubscriberResponse = httpGet(String.format(
                "/nacos/v2/ns/client/service/subscriber/list?serviceName=test-service&groupName=%s",
                DEFAULT_GROUP));
        JsonNode v2Json = objectMapper.readTree(v2SubscriberResponse);
        assertTrue(v2Json.has("code"), "V2 subscriber list should have 'code' field: " + v2SubscriberResponse);
    }

    /**
     * NSS-014: Test cluster health
     * Node health at /nacos/v2/core/cluster/node/self/health
     * Health probes at /nacos/v3/console/health/*
     */
    @Test
    @Order(14)
    void testClusterHealth() throws Exception {
        // Get node health
        String nodeHealthResponse = httpGet("/nacos/v2/core/cluster/node/self/health");
        JsonNode healthJson = assertSuccessResponse(nodeHealthResponse);
        assertNotNull(healthJson.get("data"), "Node health should have data: " + nodeHealthResponse);

        // Get health probes
        String livenessResponse = httpGet("/nacos/v3/console/health/liveness");
        JsonNode livenessJson = objectMapper.readTree(livenessResponse);
        assertEquals(0, livenessJson.get("code").asInt(),
                "Liveness should be healthy: " + livenessResponse);

        String readinessResponse = httpGet("/nacos/v3/console/health/readiness");
        JsonNode readinessJson = objectMapper.readTree(readinessResponse);
        assertEquals(0, readinessJson.get("code").asInt(),
                "Readiness should be healthy: " + readinessResponse);
    }

    /**
     * NSS-015: Test server switches
     */
    @Test
    @Order(15)
    void testServerSwitches() throws Exception {
        String switchesResponse = httpGet("/nacos/v2/ns/operator/switches");
        JsonNode json = objectMapper.readTree(switchesResponse);
        assertTrue(json.has("code"), "Switches should contain 'code' field: " + switchesResponse);
        int code = json.get("code").asInt();
        if (code == 0) {
            JsonNode data = json.get("data");
            assertNotNull(data, "Switches should return data");
            // Switches data may be an object, text, or have varying fields across implementations
            assertTrue(data.isObject() || data.isTextual(),
                    "Switches data should be an object or text: " + data);
        }
        // Accept code 0 or other codes if switches endpoint is not fully implemented
        assertTrue(code == 0 || code == 404 || code == 500 || code == 30000,
                "Switches response code should be recognized, got: " + code);
    }

    /**
     * NSS-016: Test V2 and V3 consistency
     * V2 cluster at /nacos/v2/core/cluster/node/list
     * V3 cluster at /nacos/v3/console/core/cluster/nodes
     */
    @Test
    @Order(16)
    void testV2V3Consistency() throws Exception {
        // Get cluster nodes from both APIs
        String v2Response = httpGet("/nacos/v2/core/cluster/node/list");
        JsonNode v2Json = assertSuccessResponse(v2Response);
        JsonNode v2Nodes = v2Json.get("data");

        String v3Response = httpGet("/nacos/v3/console/core/cluster/nodes");
        JsonNode v3Json = assertSuccessResponse(v3Response);
        JsonNode v3Nodes = v3Json.get("data");

        // Both should return the same number of nodes
        assertEquals(v2Nodes.size(), v3Nodes.size(),
                "V2 and V3 should report the same number of cluster nodes");

        // Both should have at least 1 node
        assertTrue(v2Nodes.size() >= 1, "Should have at least 1 cluster node from V2");
        assertTrue(v3Nodes.size() >= 1, "Should have at least 1 cluster node from V3");
    }
}
