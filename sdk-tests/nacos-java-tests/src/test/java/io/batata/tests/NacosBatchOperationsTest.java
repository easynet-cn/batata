package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.*;

import java.io.BufferedReader;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Batch Operations Tests
 *
 * Tests for batch instance registration/deregistration via SDK and
 * batch config publish/delete via V2 Open API.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosBatchOperationsTest {

    private static NamingService namingService;
    private static ConfigService configService;
    private static String serverAddr;
    private static String accessToken;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final ObjectMapper objectMapper = new ObjectMapper();

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);

        namingService = NacosFactory.createNamingService(properties);
        configService = NacosFactory.createConfigService(properties);
        accessToken = loginV3(username, password);

        assertNotNull(namingService, "NamingService should be created successfully");
        assertNotNull(configService, "ConfigService should be created successfully");
        System.out.println("Batch Operations Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (namingService != null) {
            namingService.shutDown();
        }
        if (configService != null) {
            configService.shutDown();
        }
    }

    // ==================== Batch Instance Registration Tests ====================

    /**
     * NBO-001: Test batch register multiple instances via SDK
     *
     * Registers multiple instances for a single service and verifies
     * all instances are registered and visible.
     */
    @Test
    @Order(1)
    void testBatchRegisterInstances() throws NacosException, InterruptedException {
        String serviceName = "nbo001-batch-reg-" + UUID.randomUUID().toString().substring(0, 8);
        int instanceCount = 5;

        // Register multiple instances
        for (int i = 0; i < instanceCount; i++) {
            Instance instance = new Instance();
            instance.setIp("10.0.1." + (i + 1));
            instance.setPort(8080 + i);
            instance.setWeight(1.0);
            instance.setHealthy(true);

            Map<String, String> metadata = new HashMap<>();
            metadata.put("index", String.valueOf(i));
            metadata.put("batch", "true");
            instance.setMetadata(metadata);

            namingService.registerInstance(serviceName, instance);
        }

        Thread.sleep(2000);

        // Verify all instances are registered
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertEquals(instanceCount, instances.size(),
                "Should have " + instanceCount + " instances after batch register");

        // Verify each instance has correct metadata
        Set<String> registeredIps = new HashSet<>();
        for (Instance inst : instances) {
            registeredIps.add(inst.getIp());
            assertEquals("true", inst.getMetadata().get("batch"),
                    "Each instance should have batch=true metadata");
        }

        for (int i = 0; i < instanceCount; i++) {
            assertTrue(registeredIps.contains("10.0.1." + (i + 1)),
                    "IP 10.0.1." + (i + 1) + " should be registered");
        }

        // Cleanup
        for (int i = 0; i < instanceCount; i++) {
            namingService.deregisterInstance(serviceName, "10.0.1." + (i + 1), 8080 + i);
        }
    }

    /**
     * NBO-002: Test batch deregister multiple instances
     *
     * Registers multiple instances, then deregisters all of them,
     * and verifies all are removed.
     */
    @Test
    @Order(2)
    void testBatchDeregisterInstances() throws NacosException, InterruptedException {
        String serviceName = "nbo002-batch-dereg-" + UUID.randomUUID().toString().substring(0, 8);
        int instanceCount = 4;

        // Register instances
        for (int i = 0; i < instanceCount; i++) {
            namingService.registerInstance(serviceName, "10.0.2." + (i + 1), 9090 + i);
        }
        Thread.sleep(1500);

        // Verify all registered
        List<Instance> beforeDeregister = namingService.getAllInstances(serviceName);
        assertEquals(instanceCount, beforeDeregister.size(),
                "Should have " + instanceCount + " instances before deregister");

        // Batch deregister all instances
        for (int i = 0; i < instanceCount; i++) {
            namingService.deregisterInstance(serviceName, "10.0.2." + (i + 1), 9090 + i);
        }
        Thread.sleep(1500);

        // Verify all removed
        List<Instance> afterDeregister = namingService.getAllInstances(serviceName);
        assertTrue(afterDeregister.isEmpty(),
                "Should have no instances after batch deregister, but found: " + afterDeregister.size());
    }

    /**
     * NBO-003: Test batch register with batchRegisterInstance API
     *
     * Uses the SDK's batchRegisterInstance method to register multiple
     * instances in a single call.
     */
    @Test
    @Order(3)
    void testBatchRegisterInstanceApi() throws NacosException, InterruptedException {
        String serviceName = "nbo003-batch-api-" + UUID.randomUUID().toString().substring(0, 8);

        List<Instance> instancesToRegister = new ArrayList<>();
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("10.0.3." + (i + 1));
            instance.setPort(7070 + i);
            instance.setWeight(1.0);
            instance.setHealthy(true);
            instance.setEphemeral(true);
            instancesToRegister.add(instance);
        }

        // Use batchRegisterInstance for atomic batch registration
        namingService.batchRegisterInstance(serviceName, DEFAULT_GROUP, instancesToRegister);
        Thread.sleep(2000);

        // Verify all instances registered
        List<Instance> registered = namingService.getAllInstances(serviceName);
        assertEquals(3, registered.size(),
                "batchRegisterInstance should register all 3 instances");

        Set<String> ips = new HashSet<>();
        for (Instance inst : registered) {
            ips.add(inst.getIp());
        }
        assertTrue(ips.contains("10.0.3.1"), "Instance 10.0.3.1 should be registered");
        assertTrue(ips.contains("10.0.3.2"), "Instance 10.0.3.2 should be registered");
        assertTrue(ips.contains("10.0.3.3"), "Instance 10.0.3.3 should be registered");

        // Cleanup
        for (Instance inst : instancesToRegister) {
            namingService.deregisterInstance(serviceName, inst.getIp(), inst.getPort());
        }
    }

    /**
     * NBO-004: Test batch deregister via batchDeregisterInstance API
     *
     * Registers instances via batch API, then deregisters them via
     * batchDeregisterInstance, verifying all are removed.
     */
    @Test
    @Order(4)
    void testBatchDeregisterInstanceApi() throws NacosException, InterruptedException {
        String serviceName = "nbo004-batch-dereg-api-" + UUID.randomUUID().toString().substring(0, 8);

        List<Instance> instances = new ArrayList<>();
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("10.0.4." + (i + 1));
            instance.setPort(6060 + i);
            instance.setWeight(1.0);
            instance.setHealthy(true);
            instance.setEphemeral(true);
            instances.add(instance);
        }

        // Register via batch API
        namingService.batchRegisterInstance(serviceName, DEFAULT_GROUP, instances);
        Thread.sleep(2000);

        List<Instance> beforeDeregister = namingService.getAllInstances(serviceName);
        assertEquals(3, beforeDeregister.size(), "Should have 3 instances before batch deregister");

        // Batch deregister
        namingService.batchDeregisterInstance(serviceName, DEFAULT_GROUP, instances);
        Thread.sleep(3000);

        // Verify all removed (use subscribe=false for direct query)
        List<Instance> afterDeregister = namingService.getAllInstances(serviceName, DEFAULT_GROUP, new ArrayList<>(), false);
        assertTrue(afterDeregister.isEmpty(),
                "Should have no instances after batchDeregisterInstance");
    }

    // ==================== Batch Config Operations via Open API ====================

    /**
     * NBO-005: Test batch config publish via V2 Open API
     *
     * Publishes multiple configs via the V2 Open API and verifies all
     * exist by querying through the SDK.
     */
    @Test
    @Order(5)
    void testBatchConfigPublishViaOpenApi() throws Exception {
        String prefix = "nbo005-api-pub-" + UUID.randomUUID().toString().substring(0, 8);
        int configCount = 5;

        // Publish multiple configs via Open API
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String content = "batch.api.config=" + i + "\nindex=" + i;
            String body = String.format(
                    "dataId=%s&group=%s&content=%s",
                    URLEncoder.encode(dataId, "UTF-8"),
                    URLEncoder.encode(DEFAULT_GROUP, "UTF-8"),
                    URLEncoder.encode(content, "UTF-8"));
            String response = httpPost("/nacos/v2/cs/config", body);
            JsonNode json = objectMapper.readTree(response);
            assertEquals(0, json.get("code").asInt(),
                    "Config publish via API should succeed for " + dataId + ": " + response);
        }

        Thread.sleep(1000);

        // Verify all configs exist via SDK
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String content = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertNotNull(content, "Config " + dataId + " should exist after batch publish");
            assertTrue(content.contains("batch.api.config=" + i),
                    "Config content should match for " + dataId);
        }

        // Cleanup
        for (int i = 0; i < configCount; i++) {
            configService.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
        }
    }

    /**
     * NBO-006: Test batch config delete via V2 Open API
     *
     * Publishes multiple configs, then deletes all via the V2 Open API,
     * and verifies all are gone.
     */
    @Test
    @Order(6)
    void testBatchConfigDeleteViaOpenApi() throws Exception {
        String prefix = "nbo006-api-del-" + UUID.randomUUID().toString().substring(0, 8);
        int configCount = 5;

        // Setup: publish configs via SDK
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "to.be.deleted=" + i);
            assertTrue(published, "Config " + dataId + " should be published");
        }
        Thread.sleep(500);

        // Delete all via Open API
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String response = httpDelete(String.format(
                    "/nacos/v2/cs/config?dataId=%s&group=%s",
                    URLEncoder.encode(dataId, "UTF-8"),
                    URLEncoder.encode(DEFAULT_GROUP, "UTF-8")));
            JsonNode json = objectMapper.readTree(response);
            assertEquals(0, json.get("code").asInt(),
                    "Config delete via API should succeed for " + dataId + ": " + response);
        }

        Thread.sleep(500);

        // Verify all gone via SDK
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String content = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
            assertNull(content, "Config " + dataId + " should be deleted");
        }
    }

    /**
     * NBO-007: Test batch register with different clusters
     *
     * Registers instances across different clusters in one batch and
     * verifies cluster-level isolation.
     */
    @Test
    @Order(7)
    void testBatchRegisterDifferentClusters() throws NacosException, InterruptedException {
        String serviceName = "nbo007-cluster-batch-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances in different clusters
        String[] clusters = {"cluster-a", "cluster-b"};
        for (int c = 0; c < clusters.length; c++) {
            for (int i = 0; i < 2; i++) {
                Instance instance = new Instance();
                instance.setIp("10.0." + (c + 5) + "." + (i + 1));
                instance.setPort(8080);
                instance.setClusterName(clusters[c]);
                instance.setWeight(1.0);
                instance.setHealthy(true);
                namingService.registerInstance(serviceName, instance);
            }
        }

        Thread.sleep(2000);

        // Verify total instance count
        List<Instance> allInstances = namingService.getAllInstances(serviceName);
        assertEquals(4, allInstances.size(), "Should have 4 total instances across clusters");

        // Verify per-cluster counts
        List<Instance> clusterA = namingService.getAllInstances(
                serviceName, DEFAULT_GROUP, Arrays.asList("cluster-a"), false);
        assertEquals(2, clusterA.size(), "cluster-a should have 2 instances");

        List<Instance> clusterB = namingService.getAllInstances(
                serviceName, DEFAULT_GROUP, Arrays.asList("cluster-b"), false);
        assertEquals(2, clusterB.size(), "cluster-b should have 2 instances");

        // Cleanup
        for (Instance inst : allInstances) {
            namingService.deregisterInstance(serviceName, inst.getIp(), inst.getPort(), inst.getClusterName());
        }
    }

    /**
     * NBO-008: Test batch config publish and verify via API round-trip
     *
     * Tests the complete round-trip: publish via SDK, verify via Open API,
     * delete via Open API, verify deletion via SDK.
     */
    @Test
    @Order(8)
    void testBatchConfigApiRoundTrip() throws Exception {
        String prefix = "nbo008-roundtrip-" + UUID.randomUUID().toString().substring(0, 8);
        int configCount = 3;

        // Step 1: Publish via SDK
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "roundtrip.value=" + i);
            assertTrue(published, "SDK publish should succeed for " + dataId);
        }
        Thread.sleep(1000);

        // Step 2: Verify via Open API
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String response = httpGet(String.format(
                    "/nacos/v2/cs/config?dataId=%s&group=%s",
                    URLEncoder.encode(dataId, "UTF-8"),
                    URLEncoder.encode(DEFAULT_GROUP, "UTF-8")));
            JsonNode json = objectMapper.readTree(response);
            assertEquals(0, json.get("code").asInt(),
                    "API get should succeed for " + dataId);
            assertTrue(json.has("data"), "Response should have data for " + dataId);
            String content = json.get("data").asText();
            assertEquals("roundtrip.value=" + i, content,
                    "API response content should match for " + dataId);
        }

        // Step 3: Delete via Open API
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            httpDelete(String.format(
                    "/nacos/v2/cs/config?dataId=%s&group=%s",
                    URLEncoder.encode(dataId, "UTF-8"),
                    URLEncoder.encode(DEFAULT_GROUP, "UTF-8")));
        }
        Thread.sleep(500);

        // Step 4: Verify deletion via SDK
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String content = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
            assertNull(content, "Config " + dataId + " should be deleted after API delete");
        }
    }

    // ==================== Helper Methods ====================

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
            String resp = readResponse(conn);
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
        return readResponse(conn);
    }

    private String httpDelete(String path) throws Exception {
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
