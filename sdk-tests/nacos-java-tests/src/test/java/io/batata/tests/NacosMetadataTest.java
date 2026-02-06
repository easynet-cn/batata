package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Metadata Tests
 *
 * Tests for instance and service metadata functionality:
 * - Instance metadata CRUD
 * - Service metadata management
 * - Metadata-based filtering
 * - Metadata preservation
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosMetadataTest {

    private static NamingService namingService;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";

    @BeforeAll
    static void setup() throws NacosException {
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        namingService = NacosFactory.createNamingService(properties);
        System.out.println("Nacos Metadata Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (namingService != null) {
            namingService.shutDown();
        }
    }

    // ==================== Instance Metadata Tests ====================

    /**
     * NMT-001: Test register instance with metadata
     */
    @Test
    @Order(1)
    void testRegisterInstanceWithMetadata() throws NacosException, InterruptedException {
        String serviceName = "meta-service-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.20.1");
        instance.setPort(8080);
        instance.setHealthy(true);
        instance.setWeight(1.0);

        Map<String, String> metadata = new HashMap<>();
        metadata.put("version", "1.0.0");
        metadata.put("env", "production");
        metadata.put("region", "us-east-1");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have registered instance");

        Instance found = instances.get(0);
        assertEquals("1.0.0", found.getMetadata().get("version"));
        assertEquals("production", found.getMetadata().get("env"));
        assertEquals("us-east-1", found.getMetadata().get("region"));

        System.out.println("Instance metadata: " + found.getMetadata());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NMT-002: Test update instance metadata
     */
    @Test
    @Order(2)
    void testUpdateInstanceMetadata() throws NacosException, InterruptedException {
        String serviceName = "meta-update-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.20.2");
        instance.setPort(8080);
        instance.setHealthy(true);

        Map<String, String> metadata = new HashMap<>();
        metadata.put("version", "1.0.0");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(500);

        // Update metadata
        Map<String, String> newMetadata = new HashMap<>();
        newMetadata.put("version", "2.0.0");
        newMetadata.put("updated", "true");
        instance.setMetadata(newMetadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        Instance found = instances.get(0);

        System.out.println("Updated metadata: " + found.getMetadata());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NMT-003: Test instance with empty metadata
     */
    @Test
    @Order(3)
    void testInstanceWithEmptyMetadata() throws NacosException, InterruptedException {
        String serviceName = "meta-empty-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.20.3");
        instance.setPort(8080);
        instance.setHealthy(true);
        instance.setMetadata(new HashMap<>());

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty());

        Instance found = instances.get(0);
        assertNotNull(found.getMetadata());
        System.out.println("Empty metadata size: " + found.getMetadata().size());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NMT-004: Test instance with special characters in metadata
     */
    @Test
    @Order(4)
    void testMetadataWithSpecialCharacters() throws NacosException, InterruptedException {
        String serviceName = "meta-special-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.20.4");
        instance.setPort(8080);
        instance.setHealthy(true);

        Map<String, String> metadata = new HashMap<>();
        metadata.put("path", "/api/v1/users");
        metadata.put("query", "name=test&value=123");
        metadata.put("description", "Test service with special chars: @#$%");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        Instance found = instances.get(0);

        assertEquals("/api/v1/users", found.getMetadata().get("path"));
        System.out.println("Special char metadata: " + found.getMetadata());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NMT-005: Test instance with large metadata
     */
    @Test
    @Order(5)
    void testInstanceWithLargeMetadata() throws NacosException, InterruptedException {
        String serviceName = "meta-large-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.20.5");
        instance.setPort(8080);
        instance.setHealthy(true);

        Map<String, String> metadata = new HashMap<>();
        for (int i = 0; i < 50; i++) {
            metadata.put("key" + i, "value" + i + "_" + UUID.randomUUID().toString());
        }
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        Instance found = instances.get(0);

        assertTrue(found.getMetadata().size() >= 50, "Should preserve all metadata");
        System.out.println("Large metadata size: " + found.getMetadata().size());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    // ==================== Metadata Filtering Tests ====================

    /**
     * NMT-006: Test filter instances by metadata
     */
    @Test
    @Order(6)
    void testFilterInstancesByMetadata() throws NacosException, InterruptedException {
        String serviceName = "meta-filter-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances with different metadata
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.20." + (10 + i));
            instance.setPort(8080);
            instance.setHealthy(true);

            Map<String, String> metadata = new HashMap<>();
            metadata.put("version", i < 2 ? "v1" : "v2");
            metadata.put("index", String.valueOf(i));
            instance.setMetadata(metadata);

            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        List<Instance> allInstances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        System.out.println("Total instances: " + allInstances.size());

        // Filter by metadata (client-side)
        long v1Count = allInstances.stream()
                .filter(i -> "v1".equals(i.getMetadata().get("version")))
                .count();
        System.out.println("V1 instances: " + v1Count);

        // Cleanup
        for (int i = 0; i < 3; i++) {
            namingService.deregisterInstance(serviceName, "192.168.20." + (10 + i), 8080);
        }
    }

    /**
     * NMT-007: Test metadata-based service discovery
     */
    @Test
    @Order(7)
    void testMetadataBasedDiscovery() throws NacosException, InterruptedException {
        String serviceName = "meta-discovery-" + UUID.randomUUID().toString().substring(0, 8);

        // Register blue instance
        Instance blue = new Instance();
        blue.setIp("192.168.20.20");
        blue.setPort(8080);
        blue.setHealthy(true);
        Map<String, String> blueMetadata = new HashMap<>();
        blueMetadata.put("color", "blue");
        blueMetadata.put("weight", "high");
        blue.setMetadata(blueMetadata);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, blue);

        // Register green instance
        Instance green = new Instance();
        green.setIp("192.168.20.21");
        green.setPort(8080);
        green.setHealthy(true);
        Map<String, String> greenMetadata = new HashMap<>();
        greenMetadata.put("color", "green");
        greenMetadata.put("weight", "low");
        green.setMetadata(greenMetadata);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, green);

        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);

        // Discover by color
        Instance blueInstance = instances.stream()
                .filter(i -> "blue".equals(i.getMetadata().get("color")))
                .findFirst()
                .orElse(null);

        assertNotNull(blueInstance);
        assertEquals("high", blueInstance.getMetadata().get("weight"));
        System.out.println("Blue instance found: " + blueInstance.getIp());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, blue);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, green);
    }

    // ==================== Metadata Preservation Tests ====================

    /**
     * NMT-008: Test metadata preserved across re-registration
     */
    @Test
    @Order(8)
    void testMetadataPreservedAcrossReregistration() throws NacosException, InterruptedException {
        String serviceName = "meta-preserve-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.20.30");
        instance.setPort(8080);
        instance.setHealthy(true);

        Map<String, String> metadata = new HashMap<>();
        metadata.put("important", "value");
        instance.setMetadata(metadata);

        // First registration
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(500);

        // Re-register with same metadata
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertEquals(1, instances.size(), "Should have one instance");
        assertEquals("value", instances.get(0).getMetadata().get("important"));

        System.out.println("Metadata preserved: " + instances.get(0).getMetadata());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NMT-009: Test metadata with numeric values
     */
    @Test
    @Order(9)
    void testMetadataWithNumericValues() throws NacosException, InterruptedException {
        String serviceName = "meta-numeric-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.20.31");
        instance.setPort(8080);
        instance.setHealthy(true);

        Map<String, String> metadata = new HashMap<>();
        metadata.put("port", "8080");
        metadata.put("connections", "100");
        metadata.put("ratio", "0.85");
        metadata.put("negative", "-1");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        Instance found = instances.get(0);

        assertEquals("8080", found.getMetadata().get("port"));
        assertEquals("0.85", found.getMetadata().get("ratio"));
        System.out.println("Numeric metadata: " + found.getMetadata());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NMT-010: Test metadata with boolean-like values
     */
    @Test
    @Order(10)
    void testMetadataWithBooleanValues() throws NacosException, InterruptedException {
        String serviceName = "meta-boolean-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.20.32");
        instance.setPort(8080);
        instance.setHealthy(true);

        Map<String, String> metadata = new HashMap<>();
        metadata.put("enabled", "true");
        metadata.put("deprecated", "false");
        metadata.put("active", "yes");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        Instance found = instances.get(0);

        assertEquals("true", found.getMetadata().get("enabled"));
        assertEquals("false", found.getMetadata().get("deprecated"));
        System.out.println("Boolean metadata: " + found.getMetadata());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    // ==================== Concurrent Metadata Tests ====================

    /**
     * NMT-011: Test concurrent metadata updates
     */
    @Test
    @Order(11)
    void testConcurrentMetadataUpdates() throws NacosException, InterruptedException {
        String serviceName = "meta-concurrent-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.20.40");
        instance.setPort(8080);
        instance.setHealthy(true);

        Map<String, String> metadata = new HashMap<>();
        metadata.put("counter", "0");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(500);

        int threadCount = 5;
        CountDownLatch latch = new CountDownLatch(threadCount);
        List<Exception> errors = new CopyOnWriteArrayList<>();

        for (int i = 0; i < threadCount; i++) {
            final int idx = i;
            new Thread(() -> {
                try {
                    Instance update = new Instance();
                    update.setIp("192.168.20.40");
                    update.setPort(8080);
                    update.setHealthy(true);

                    Map<String, String> updateMeta = new HashMap<>();
                    updateMeta.put("counter", String.valueOf(idx));
                    updateMeta.put("thread", "thread-" + idx);
                    update.setMetadata(updateMeta);

                    namingService.registerInstance(serviceName, DEFAULT_GROUP, update);
                } catch (Exception e) {
                    errors.add(e);
                } finally {
                    latch.countDown();
                }
            }).start();
        }

        boolean completed = latch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete");

        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        System.out.println("Final metadata: " + instances.get(0).getMetadata());
        System.out.println("Concurrent update errors: " + errors.size());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NMT-012: Test multiple instances with same metadata keys
     */
    @Test
    @Order(12)
    void testMultipleInstancesSameMetadataKeys() throws NacosException, InterruptedException {
        String serviceName = "meta-multi-" + UUID.randomUUID().toString().substring(0, 8);

        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.20." + (50 + i));
            instance.setPort(8080);
            instance.setHealthy(true);

            Map<String, String> metadata = new HashMap<>();
            metadata.put("instance-id", "inst-" + i);
            metadata.put("version", "1.0");
            metadata.put("common", "shared");
            instance.setMetadata(metadata);

            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertEquals(3, instances.size());

        // Verify each instance has unique instance-id
        Set<String> instanceIds = new HashSet<>();
        for (Instance inst : instances) {
            instanceIds.add(inst.getMetadata().get("instance-id"));
            assertEquals("1.0", inst.getMetadata().get("version"));
            assertEquals("shared", inst.getMetadata().get("common"));
        }
        assertEquals(3, instanceIds.size(), "Each instance should have unique instance-id");

        System.out.println("Instance IDs: " + instanceIds);

        // Cleanup
        for (int i = 0; i < 3; i++) {
            namingService.deregisterInstance(serviceName, "192.168.20." + (50 + i), 8080);
        }
    }

    // ==================== Extended Keys Tests ====================

    /**
     * NMT-013: Test metadata with extended instance ID
     */
    @Test
    @Order(13)
    void testMetadataWithExtendedInstanceId() throws NacosException, InterruptedException {
        String serviceName = "meta-extended-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.20.60");
        instance.setPort(8080);
        instance.setHealthy(true);
        instance.setInstanceId("custom-instance-id-12345");

        Map<String, String> metadata = new HashMap<>();
        metadata.put("customId", "12345");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty());

        Instance found = instances.get(0);
        System.out.println("Instance ID: " + found.getInstanceId());
        System.out.println("Custom ID in metadata: " + found.getMetadata().get("customId"));

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NMT-014: Test metadata with cluster name
     */
    @Test
    @Order(14)
    void testMetadataWithClusterName() throws NacosException, InterruptedException {
        String serviceName = "meta-cluster-" + UUID.randomUUID().toString().substring(0, 8);
        String clusterName = "cluster-a";

        Instance instance = new Instance();
        instance.setIp("192.168.20.61");
        instance.setPort(8080);
        instance.setHealthy(true);
        instance.setClusterName(clusterName);

        Map<String, String> metadata = new HashMap<>();
        metadata.put("cluster", clusterName);
        metadata.put("zone", "zone-1");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.selectInstances(
                serviceName, DEFAULT_GROUP, Arrays.asList(clusterName), true);

        assertFalse(instances.isEmpty());
        assertEquals(clusterName, instances.get(0).getClusterName());
        assertEquals("zone-1", instances.get(0).getMetadata().get("zone"));

        System.out.println("Cluster instance metadata: " + instances.get(0).getMetadata());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NMT-015: Test clear metadata
     */
    @Test
    @Order(15)
    void testClearMetadata() throws NacosException, InterruptedException {
        String serviceName = "meta-clear-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.20.62");
        instance.setPort(8080);
        instance.setHealthy(true);

        Map<String, String> metadata = new HashMap<>();
        metadata.put("key1", "value1");
        metadata.put("key2", "value2");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(500);

        // Clear metadata by setting empty map
        instance.setMetadata(new HashMap<>());
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        Instance found = instances.get(0);

        System.out.println("Cleared metadata size: " + found.getMetadata().size());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }
}
