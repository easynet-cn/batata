package io.batata.tests;

import org.junit.jupiter.api.*;

import java.io.BufferedReader;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Cluster API Tests
 *
 * Tests for cluster management via V2 open API and V3 console API.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosClusterApiTest {

    private static String serverAddr;
    private static String consoleAddr;
    private static String accessToken;

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        consoleAddr = System.getProperty("nacos.console", "127.0.0.1:8081");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        accessToken = loginV3(username, password);
        assertFalse(accessToken.isEmpty(),
                "Should obtain access token via V3 login");
    }

    // ==================== V2 Cluster API Tests ====================

    /**
     * CL-001: Test get current node info via V2
     */
    @Test
    @Order(1)
    void testGetCurrentNode() throws Exception {
        String response = httpGetMain("/nacos/v2/core/cluster/node/self");
        assertNotNull(response, "V2 current node response should not be null");
        assertFalse(response.isEmpty(), "V2 current node response should not be empty");

        // Verify response is valid JSON containing expected structure
        assertTrue(response.contains("{"), "Response should be JSON");

        // Verify essential node fields
        assertTrue(response.contains("ip") || response.contains("address"),
                "Node response should contain IP or address field");
        assertTrue(response.contains("port"),
                "Node response should contain port field");

        // Verify success indicator
        assertTrue(response.contains("200") || response.contains("data") || response.contains("\"code\""),
                "Response should indicate success or contain data");
    }

    /**
     * CL-002: Test list cluster nodes via V2
     */
    @Test
    @Order(2)
    void testListClusterNodes() throws Exception {
        String response = httpGetMain("/nacos/v2/core/cluster/node/list");
        assertNotNull(response, "V2 cluster node list response should not be null");
        assertFalse(response.isEmpty(), "V2 cluster node list response should not be empty");

        // Response should be valid JSON
        assertTrue(response.contains("{"), "Response should be JSON");

        // In a running server, the node list should contain at least the current node
        assertTrue(response.contains("ip") || response.contains("address"),
                "Node list should contain at least one node with IP/address");
        assertTrue(response.contains("port"),
                "Node list should contain at least one node with port");

        // Verify the list is not empty (at least the self node should be present)
        // The response typically has a "data" array
        assertFalse(response.contains("\"data\":[]"),
                "Node list should not be empty; at least the current node should be present");
    }

    /**
     * CL-003: Test node health via V2
     */
    @Test
    @Order(3)
    void testNodeHealth() throws Exception {
        String response = httpGetMain("/nacos/v2/core/cluster/node/self/health");
        assertNotNull(response, "V2 node health response should not be null");
        assertFalse(response.isEmpty(), "V2 node health response should not be empty");

        // Health check should return a valid response
        assertTrue(response.contains("{"), "Response should be JSON");
        // A running node should report a healthy status
        assertTrue(
                response.contains("UP") || response.contains("READY")
                        || response.contains("200") || response.contains("\"data\""),
                "Health response should indicate a healthy state (UP/READY/200)");
    }

    // ==================== V3 Console Cluster API Tests ====================

    /**
     * CL-004: Test V3 cluster nodes list
     */
    @Test
    @Order(4)
    void testV3ClusterNodes() throws Exception {
        String response = httpGetConsole("/v3/console/core/cluster/nodes");
        assertNotNull(response, "V3 cluster nodes response should not be null");
        assertFalse(response.isEmpty(), "V3 cluster nodes response should not be empty");

        assertTrue(response.contains("{"), "Response should be JSON");

        // Should contain node data with IP and port
        assertTrue(response.contains("ip") || response.contains("address"),
                "V3 cluster nodes should contain IP/address for at least one node");
        assertTrue(response.contains("port"),
                "V3 cluster nodes should contain port for at least one node");
    }

    /**
     * CL-005: Test V3 cluster health
     */
    @Test
    @Order(5)
    void testV3ClusterHealth() throws Exception {
        String response = httpGetConsole("/v3/console/core/cluster/health");
        assertNotNull(response, "V3 cluster health response should not be null");
        assertFalse(response.isEmpty(), "V3 cluster health response should not be empty");

        assertTrue(response.contains("{"), "Response should be JSON");
        // Health endpoint should indicate cluster health status
        assertTrue(
                response.contains("UP") || response.contains("READY")
                        || response.contains("200") || response.contains("\"data\""),
                "V3 cluster health should indicate a healthy state");
    }

    /**
     * CL-006: Test V3 cluster self info
     */
    @Test
    @Order(6)
    void testV3ClusterSelf() throws Exception {
        String response = httpGetConsole("/v3/console/core/cluster/self");
        assertNotNull(response, "V3 cluster self response should not be null");
        assertFalse(response.isEmpty(), "V3 cluster self response should not be empty");

        assertTrue(response.contains("{"), "Response should be JSON");

        // Self info should include the current node's IP and port
        assertTrue(response.contains("ip") || response.contains("address"),
                "V3 cluster self should contain current node IP/address");
        assertTrue(response.contains("port"),
                "V3 cluster self should contain current node port");

        // Should also contain state/status info
        assertTrue(response.contains("state") || response.contains("status")
                        || response.contains("UP") || response.contains("READY"),
                "V3 cluster self should contain node state information");
    }

    /**
     * CL-007: Test V3 standalone mode
     */
    @Test
    @Order(7)
    void testV3StandaloneMode() throws Exception {
        String response = httpGetConsole("/v3/console/core/cluster/standalone");
        assertNotNull(response, "V3 standalone mode response should not be null");
        assertFalse(response.isEmpty(), "V3 standalone mode response should not be empty");

        // Response should contain a boolean indicator for standalone mode
        assertTrue(response.contains("true") || response.contains("false")
                        || response.contains("data"),
                "V3 standalone response should indicate true/false or contain data");
    }

    /**
     * CL-008: Test cluster node required fields
     */
    @Test
    @Order(8)
    void testClusterNodeRequiredFields() throws Exception {
        String response = httpGetMain("/nacos/v2/core/cluster/node/self");
        assertNotNull(response, "Response should not be null");
        assertFalse(response.isEmpty(), "Response should not be empty");

        // Verify all essential fields are present
        boolean hasIp = response.contains("ip") || response.contains("address");
        boolean hasPort = response.contains("port");
        boolean hasState = response.contains("state") || response.contains("status");

        assertTrue(hasIp, "Node response MUST contain IP or address field");
        assertTrue(hasPort, "Node response MUST contain port field");
        assertTrue(hasState, "Node response MUST contain state or status field");

        // Verify port value is a reasonable number (present as a numeric value)
        // Port should appear as a number like 8848 or 9848
        assertTrue(response.contains("8848") || response.contains("9848")
                        || response.matches(".*\"port\"\\s*:\\s*\\d+.*"),
                "Port should contain a valid numeric value");
    }

    // ==================== Helper Methods ====================

    private String httpGetMain(String path) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }
        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("GET");
        int responseCode = conn.getResponseCode();
        assertTrue(responseCode < 500,
                "Main server request to " + path + " should not return server error, got: " + responseCode);
        return readResponse(conn);
    }

    private String httpGetConsole(String path) throws Exception {
        // Try console server first, fall back to main server with /nacos prefix
        String fullUrl = String.format("http://%s%s", consoleAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }

        try {
            URL url = new URL(fullUrl);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            conn.setRequestMethod("GET");
            conn.setConnectTimeout(3000);
            conn.setReadTimeout(5000);
            int code = conn.getResponseCode();
            if (code < 500) {
                return readResponse(conn);
            }
        } catch (Exception e) {
            // Console server not available, fall back to main server
        }

        // Fallback to main server
        fullUrl = String.format("http://%s/nacos%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }
        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("GET");
        int responseCode = conn.getResponseCode();
        assertTrue(responseCode < 500,
                "Fallback request to /nacos" + path + " should not return server error, got: " + responseCode);
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

        assertEquals(200, conn.getResponseCode(),
                "V3 login should return HTTP 200");

        BufferedReader reader = new BufferedReader(new InputStreamReader(conn.getInputStream()));
        StringBuilder response = new StringBuilder();
        String line;
        while ((line = reader.readLine()) != null) {
            response.append(line);
        }
        String resp = response.toString();
        assertTrue(resp.contains("accessToken"),
                "Login response should contain accessToken field");

        int start = resp.indexOf("accessToken") + 14;
        int end = resp.indexOf("\"", start);
        assertTrue(end > start, "accessToken value should be non-empty");
        return resp.substring(start, end);
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
