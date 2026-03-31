package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import org.junit.jupiter.api.*;

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
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";

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
     * NBO-005: Test batch config publish via SDK
     *
     * Publishes multiple configs via SDK and verifies all exist.
     */
    @Test
    @Order(5)
    void testBatchConfigPublishViaSdk() throws Exception {
        String prefix = "nbo005-sdk-pub-" + UUID.randomUUID().toString().substring(0, 8);
        int configCount = 5;

        // Publish multiple configs via SDK
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String content = "batch.sdk.config=" + i + "\nindex=" + i;
            boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, content);
            assertTrue(published, "Config publish should succeed for " + dataId);
        }

        Thread.sleep(1000);

        // Verify all configs exist via SDK
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String content = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertNotNull(content, "Config " + dataId + " should exist after batch publish");
            assertTrue(content.contains("batch.sdk.config=" + i),
                    "Config content should match for " + dataId);
        }

        // Cleanup
        for (int i = 0; i < configCount; i++) {
            configService.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
        }
    }

    /**
     * NBO-006: Test batch config delete via SDK
     *
     * Publishes multiple configs, then deletes all via SDK,
     * and verifies all are gone.
     */
    @Test
    @Order(6)
    void testBatchConfigDeleteViaSdk() throws Exception {
        String prefix = "nbo006-sdk-del-" + UUID.randomUUID().toString().substring(0, 8);
        int configCount = 5;

        // Setup: publish configs via SDK
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "to.be.deleted=" + i);
            assertTrue(published, "Config " + dataId + " should be published");
        }
        Thread.sleep(500);

        // Delete all via SDK
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            boolean deleted = configService.removeConfig(dataId, DEFAULT_GROUP);
            assertTrue(deleted, "Config " + dataId + " should be deleted");
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
     * NBO-008: Test batch config publish, verify, and delete round-trip via SDK
     *
     * Tests the complete round-trip: publish via SDK, verify via SDK,
     * delete via SDK, verify deletion via SDK.
     */
    @Test
    @Order(8)
    void testBatchConfigSdkRoundTrip() throws Exception {
        String prefix = "nbo008-roundtrip-" + UUID.randomUUID().toString().substring(0, 8);
        int configCount = 3;

        // Step 1: Publish via SDK
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "roundtrip.value=" + i);
            assertTrue(published, "SDK publish should succeed for " + dataId);
        }
        Thread.sleep(1000);

        // Step 2: Verify via SDK
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String content = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertNotNull(content, "Config should exist for " + dataId);
            assertEquals("roundtrip.value=" + i, content,
                    "Config content should match for " + dataId);
        }

        // Step 3: Delete via SDK
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            boolean deleted = configService.removeConfig(dataId, DEFAULT_GROUP);
            assertTrue(deleted, "Config should be deleted for " + dataId);
        }
        Thread.sleep(500);

        // Step 4: Verify deletion via SDK
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String content = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
            assertNull(content, "Config " + dataId + " should be deleted");
        }
    }

}
