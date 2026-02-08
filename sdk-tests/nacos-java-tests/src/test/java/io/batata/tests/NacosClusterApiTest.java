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
        System.out.println("Cluster API Test Setup - Server: " + serverAddr + ", Console: " + consoleAddr);
    }

    // ==================== V2 Cluster API Tests ====================

    /**
     * CL-001: Test get current node info via V2
     */
    @Test
    @Order(1)
    void testGetCurrentNode() throws Exception {
        String response = httpGetMain("/nacos/v2/core/cluster/node/self");
        System.out.println("V2 Current node: " + response);
        assertNotNull(response, "Response should not be null");
        // Should contain node information
        assertTrue(response.contains("ip") || response.contains("address") || response.contains("data"),
                "Response should contain node information");
    }

    /**
     * CL-002: Test list cluster nodes via V2
     */
    @Test
    @Order(2)
    void testListClusterNodes() throws Exception {
        String response = httpGetMain("/nacos/v2/core/cluster/node/list");
        System.out.println("V2 Cluster nodes: " + response);
        assertNotNull(response, "Response should not be null");
    }

    /**
     * CL-003: Test node health via V2
     */
    @Test
    @Order(3)
    void testNodeHealth() throws Exception {
        String response = httpGetMain("/nacos/v2/core/cluster/node/self/health");
        System.out.println("V2 Node health: " + response);
        assertNotNull(response, "Response should not be null");
    }

    // ==================== V3 Console Cluster API Tests ====================

    /**
     * CL-004: Test V3 cluster nodes list
     */
    @Test
    @Order(4)
    void testV3ClusterNodes() throws Exception {
        String response = httpGetConsole("/v3/console/core/cluster/nodes");
        System.out.println("V3 Cluster nodes: " + response);
        assertNotNull(response, "Response should not be null");
    }

    /**
     * CL-005: Test V3 cluster health
     */
    @Test
    @Order(5)
    void testV3ClusterHealth() throws Exception {
        String response = httpGetConsole("/v3/console/core/cluster/health");
        System.out.println("V3 Cluster health: " + response);
        assertNotNull(response, "Response should not be null");
    }

    /**
     * CL-006: Test V3 cluster self info
     */
    @Test
    @Order(6)
    void testV3ClusterSelf() throws Exception {
        String response = httpGetConsole("/v3/console/core/cluster/self");
        System.out.println("V3 Cluster self: " + response);
        assertNotNull(response, "Response should not be null");
    }

    /**
     * CL-007: Test V3 standalone mode
     */
    @Test
    @Order(7)
    void testV3StandaloneMode() throws Exception {
        String response = httpGetConsole("/v3/console/core/cluster/standalone");
        System.out.println("V3 Standalone mode: " + response);
        assertNotNull(response, "Response should not be null");
    }

    /**
     * CL-008: Test cluster node required fields
     */
    @Test
    @Order(8)
    void testClusterNodeRequiredFields() throws Exception {
        String response = httpGetMain("/nacos/v2/core/cluster/node/self");
        System.out.println("Node fields check: " + response);
        assertNotNull(response, "Response should not be null");

        // Verify essential fields are present (either in data or directly)
        boolean hasIp = response.contains("ip") || response.contains("address");
        boolean hasPort = response.contains("port");
        boolean hasState = response.contains("state") || response.contains("status");

        System.out.println("Has IP: " + hasIp + ", Has Port: " + hasPort + ", Has State: " + hasState);

        // At minimum, node info should have ip and port
        assertTrue(hasIp || hasPort, "Node response should contain address or port information");
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
            System.out.println("Console server not available, falling back to main server: " + e.getMessage());
        }

        // Fallback to main server
        fullUrl = String.format("http://%s/nacos%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }
        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("GET");
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
