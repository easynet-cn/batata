package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import com.alibaba.nacos.api.naming.pojo.healthcheck.AbstractHealthChecker;
import com.alibaba.nacos.api.naming.pojo.healthcheck.impl.Http;
import com.alibaba.nacos.api.naming.pojo.healthcheck.impl.Tcp;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Health Check Tests
 *
 * Tests for instance health check functionality:
 * - Health status management
 * - Healthy instance filtering
 * - Health check types (TCP, HTTP)
 * - Beat mechanism
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosHealthCheckTest {

    private static NamingService namingService;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";

    @BeforeAll
    static void setup() throws NacosException {
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);

        namingService = NacosFactory.createNamingService(properties);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (namingService != null) {
            namingService.shutDown();
        }
    }

    // ==================== Health Status Tests ====================

    /**
     * NHC-001: Test register healthy instance
     */
    @Test
    @Order(1)
    void testRegisterHealthyInstance() throws NacosException, InterruptedException {
        String serviceName = "healthy-instance-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.10.1");
        instance.setPort(8080);
        instance.setHealthy(true);
        instance.setWeight(1.0);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have registered instance");

        Instance found = instances.get(0);
        assertTrue(found.isHealthy(), "Instance should be healthy");
        assertEquals("192.168.10.1", found.getIp(), "Instance IP should match");
        assertEquals(8080, found.getPort(), "Instance port should match");
        assertEquals(1.0, found.getWeight(), 0.01, "Instance weight should match");

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NHC-002: Test register unhealthy instance
     */
    @Test
    @Order(2)
    void testRegisterUnhealthyInstance() throws NacosException, InterruptedException {
        String serviceName = "unhealthy-instance-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.10.2");
        instance.setPort(8080);
        instance.setHealthy(false);
        instance.setWeight(1.0);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        // Get all instances including unhealthy
        List<Instance> allInstances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(allInstances.isEmpty(), "Should have registered instance");
        assertEquals(1, allInstances.size(), "Should have exactly 1 instance");
        assertEquals("192.168.10.2", allInstances.get(0).getIp(),
                "Instance IP should match the registered unhealthy instance");

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NHC-003: Test select healthy instances only
     */
    @Test
    @Order(3)
    void testSelectHealthyInstancesOnly() throws NacosException, InterruptedException {
        String serviceName = "select-healthy-" + UUID.randomUUID().toString().substring(0, 8);

        // Register healthy instance
        Instance healthy = new Instance();
        healthy.setIp("192.168.10.10");
        healthy.setPort(8080);
        healthy.setHealthy(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, healthy);

        // Register unhealthy instance
        Instance unhealthy = new Instance();
        unhealthy.setIp("192.168.10.11");
        unhealthy.setPort(8080);
        unhealthy.setHealthy(false);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, unhealthy);

        Thread.sleep(1000);

        // Select only healthy
        List<Instance> healthyInstances = namingService.selectInstances(serviceName, DEFAULT_GROUP, true);

        assertNotNull(healthyInstances, "Healthy instances list should not be null");
        assertFalse(healthyInstances.isEmpty(), "Should find at least one healthy instance");
        for (Instance inst : healthyInstances) {
            assertTrue(inst.isHealthy(), "Selected instance should be healthy");
            assertNotEquals("192.168.10.11", inst.getIp(),
                    "Unhealthy instance should not appear in healthy selection");
        }

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, healthy);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, unhealthy);
    }

    /**
     * NHC-004: Test select all instances including unhealthy
     */
    @Test
    @Order(4)
    void testSelectAllInstances() throws NacosException, InterruptedException {
        String serviceName = "select-all-" + UUID.randomUUID().toString().substring(0, 8);

        // Register mixed health instances with delay between each to allow cache updates
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.10." + (20 + i));
            instance.setPort(8080);
            instance.setHealthy(i % 2 == 0); // Alternate health status
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
            Thread.sleep(1000);
        }

        Thread.sleep(3000);

        // Select all (healthy = false means include unhealthy) using non-subscribe query
        List<Instance> allInstances = namingService.selectInstances(serviceName, DEFAULT_GROUP, new ArrayList<>(), false, false);

        assertNotNull(allInstances, "Instance list should not be null");
        assertTrue(allInstances.size() >= 3,
                "Should have at least 3 instances (including unhealthy), got: " + allInstances.size());

        Set<String> ips = allInstances.stream().map(Instance::getIp).collect(Collectors.toSet());
        assertTrue(ips.contains("192.168.10.20"), "Should include instance 192.168.10.20");
        assertTrue(ips.contains("192.168.10.21"), "Should include instance 192.168.10.21");
        assertTrue(ips.contains("192.168.10.22"), "Should include instance 192.168.10.22");

        // Cleanup
        for (int i = 0; i < 3; i++) {
            namingService.deregisterInstance(serviceName, "192.168.10." + (20 + i), 8080);
        }
    }

    // ==================== Health Check Type Tests ====================

    /**
     * NHC-005: Test instance with TCP health check
     */
    @Test
    @Order(5)
    void testInstanceWithTcpHealthCheck() throws NacosException, InterruptedException {
        String serviceName = "tcp-health-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.10.30");
        instance.setPort(8080);
        instance.setHealthy(true);

        // Add metadata for health check
        Map<String, String> metadata = new HashMap<>();
        metadata.put("healthCheckType", "TCP");
        metadata.put("healthCheckPort", "8080");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have instance with TCP health check");

        Instance found = instances.get(0);
        assertEquals("TCP", found.getMetadata().get("healthCheckType"),
                "Health check type metadata should be TCP");
        assertEquals("8080", found.getMetadata().get("healthCheckPort"),
                "Health check port metadata should be 8080");
        assertEquals("192.168.10.30", found.getIp(), "Instance IP should match");

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NHC-006: Test instance with HTTP health check
     */
    @Test
    @Order(6)
    void testInstanceWithHttpHealthCheck() throws NacosException, InterruptedException {
        String serviceName = "http-health-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.10.31");
        instance.setPort(8080);
        instance.setHealthy(true);

        // Add metadata for HTTP health check
        Map<String, String> metadata = new HashMap<>();
        metadata.put("healthCheckType", "HTTP");
        metadata.put("healthCheckPath", "/health");
        metadata.put("healthCheckPort", "8080");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have instance with HTTP health check");

        Instance found = instances.get(0);
        assertEquals("/health", found.getMetadata().get("healthCheckPath"),
                "Health check path should be /health");
        assertEquals("HTTP", found.getMetadata().get("healthCheckType"),
                "Health check type should be HTTP");
        assertEquals("8080", found.getMetadata().get("healthCheckPort"),
                "Health check port should be 8080");

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    // ==================== Weight and Health Combined Tests ====================

    /**
     * NHC-007: Test weighted selection with health filter
     */
    @Test
    @Order(7)
    void testWeightedSelectionWithHealthFilter() throws NacosException, InterruptedException {
        String serviceName = "weighted-health-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances with different weights
        Instance high = new Instance();
        high.setIp("192.168.10.40");
        high.setPort(8080);
        high.setWeight(100.0);
        high.setHealthy(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, high);

        Instance low = new Instance();
        low.setIp("192.168.10.41");
        low.setPort(8080);
        low.setWeight(1.0);
        low.setHealthy(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, low);

        Instance unhealthy = new Instance();
        unhealthy.setIp("192.168.10.42");
        unhealthy.setPort(8080);
        unhealthy.setWeight(100.0);
        unhealthy.setHealthy(false);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, unhealthy);

        Thread.sleep(1000);

        // Select healthy instances
        List<Instance> healthyInstances = namingService.selectInstances(serviceName, DEFAULT_GROUP, true);

        assertEquals(2, healthyInstances.size(), "Should have 2 healthy instances");
        for (Instance inst : healthyInstances) {
            assertTrue(inst.isHealthy(), "All selected instances should be healthy");
            assertNotEquals("192.168.10.42", inst.getIp(),
                    "Unhealthy instance should not be in healthy selection");
        }

        // Count selections (high weight should be selected more often)
        Map<String, Integer> selectionCount = new HashMap<>();
        for (int i = 0; i < 100; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName, DEFAULT_GROUP);
            assertNotNull(selected, "Should select a healthy instance");
            assertTrue(selected.isHealthy(), "Selected instance must be healthy");
            selectionCount.merge(selected.getIp(), 1, Integer::sum);
        }

        int highCount = selectionCount.getOrDefault("192.168.10.40", 0);
        int lowCount = selectionCount.getOrDefault("192.168.10.41", 0);
        assertTrue(highCount > lowCount,
                "High weight healthy instance should be selected more often, got high=" + highCount + " low=" + lowCount);
        assertFalse(selectionCount.containsKey("192.168.10.42"),
                "Unhealthy instance should never be selected via selectOneHealthyInstance");

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, high);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, low);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, unhealthy);
    }

    /**
     * NHC-008: Test zero weight instance excluded
     */
    @Test
    @Order(8)
    void testZeroWeightInstanceExcluded() throws NacosException, InterruptedException {
        String serviceName = "zero-weight-" + UUID.randomUUID().toString().substring(0, 8);

        // Register zero weight instance
        Instance zero = new Instance();
        zero.setIp("192.168.10.50");
        zero.setPort(8080);
        zero.setWeight(0.0);
        zero.setHealthy(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, zero);

        // Register normal weight instance
        Instance normal = new Instance();
        normal.setIp("192.168.10.51");
        normal.setPort(8080);
        normal.setWeight(1.0);
        normal.setHealthy(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, normal);

        Thread.sleep(1000);

        // Select should prefer non-zero weight
        int zeroSelected = 0;
        int normalSelected = 0;
        for (int i = 0; i < 50; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName, DEFAULT_GROUP);
            assertNotNull(selected, "Should select an instance");
            if (selected.getIp().equals("192.168.10.50")) {
                zeroSelected++;
            } else {
                normalSelected++;
            }
        }

        assertTrue(normalSelected > zeroSelected,
                "Normal weight instance should be selected more than zero weight, got normal=" + normalSelected + " zero=" + zeroSelected);

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, zero);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, normal);
    }

    // ==================== Ephemeral Instance Tests ====================

    /**
     * NHC-009: Test ephemeral instance heartbeat
     */
    @Test
    @Order(9)
    void testEphemeralInstanceHeartbeat() throws NacosException, InterruptedException {
        String serviceName = "ephemeral-hb-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.10.60");
        instance.setPort(8080);
        instance.setHealthy(true);
        instance.setEphemeral(true);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);

        // Wait and check if instance stays healthy (heartbeat working)
        Thread.sleep(2000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Ephemeral instance should still be registered");
        assertTrue(instances.get(0).isEphemeral(), "Instance should be ephemeral");
        assertTrue(instances.get(0).isHealthy(),
                "Ephemeral instance should remain healthy with active gRPC connection");

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NHC-010: Test ephemeral instance stays healthy with connection
     */
    @Test
    @Order(10)
    void testEphemeralInstanceStaysHealthy() throws NacosException, InterruptedException {
        String serviceName = "ephemeral-stay-healthy-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.10.61");
        instance.setPort(8080);
        instance.setHealthy(true);
        instance.setEphemeral(true);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(2000);

        // Instance should remain healthy because the gRPC connection is active
        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have ephemeral instance");
        assertTrue(instances.get(0).isEphemeral(), "Instance should be ephemeral");
        assertTrue(instances.get(0).isHealthy(),
                "Ephemeral instance should stay healthy with active connection after 2s");
        assertEquals("192.168.10.61", instances.get(0).getIp(),
                "Instance IP should match");

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    // ==================== Cluster Health Tests ====================

    /**
     * NHC-011: Test select healthy from specific cluster
     */
    @Test
    @Order(11)
    void testSelectHealthyFromCluster() throws NacosException, InterruptedException {
        String serviceName = "cluster-health-" + UUID.randomUUID().toString().substring(0, 8);

        // Register healthy in cluster-a
        Instance healthyA = new Instance();
        healthyA.setIp("192.168.10.70");
        healthyA.setPort(8080);
        healthyA.setClusterName("cluster-a");
        healthyA.setHealthy(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, healthyA);

        // Register unhealthy in cluster-a
        Instance unhealthyA = new Instance();
        unhealthyA.setIp("192.168.10.71");
        unhealthyA.setPort(8080);
        unhealthyA.setClusterName("cluster-a");
        unhealthyA.setHealthy(false);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, unhealthyA);

        // Register healthy in cluster-b
        Instance healthyB = new Instance();
        healthyB.setIp("192.168.10.72");
        healthyB.setPort(8080);
        healthyB.setClusterName("cluster-b");
        healthyB.setHealthy(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, healthyB);

        Thread.sleep(1000);

        // Select healthy from cluster-a only
        List<Instance> clusterAHealthy = namingService.selectInstances(
                serviceName, DEFAULT_GROUP, Arrays.asList("cluster-a"), true);

        assertNotNull(clusterAHealthy, "Cluster-a healthy list should not be null");
        assertFalse(clusterAHealthy.isEmpty(), "Should find healthy instances in cluster-a");

        for (Instance inst : clusterAHealthy) {
            assertEquals("cluster-a", inst.getClusterName(),
                    "All instances should belong to cluster-a");
            assertTrue(inst.isHealthy(),
                    "All selected instances should be healthy");
            assertNotEquals("192.168.10.71", inst.getIp(),
                    "Unhealthy instance should not be in the selection");
        }

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, healthyA);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, unhealthyA);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, healthyB);
    }

    /**
     * NHC-012: Test all clusters all instances
     */
    @Test
    @Order(12)
    void testAllClustersAllInstances() throws NacosException, InterruptedException {
        String serviceName = "all-clusters-" + UUID.randomUUID().toString().substring(0, 8);

        String[] clusters = {"dc1", "dc2", "dc3"};
        for (String cluster : clusters) {
            Instance instance = new Instance();
            instance.setIp("192.168.11." + Math.abs(cluster.hashCode() % 256));
            instance.setPort(8080);
            instance.setClusterName(cluster);
            instance.setHealthy(true);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        // Get all instances from all clusters
        List<Instance> allInstances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);

        assertNotNull(allInstances, "All instances list should not be null");
        assertEquals(3, allInstances.size(),
                "Should have 3 instances across all clusters");

        Set<String> clusterNames = allInstances.stream()
                .map(Instance::getClusterName)
                .collect(Collectors.toSet());
        assertEquals(3, clusterNames.size(), "Should have instances in 3 different clusters");
        assertTrue(clusterNames.contains("dc1"), "Should contain dc1 cluster");
        assertTrue(clusterNames.contains("dc2"), "Should contain dc2 cluster");
        assertTrue(clusterNames.contains("dc3"), "Should contain dc3 cluster");

        for (Instance inst : allInstances) {
            assertTrue(inst.isHealthy(), "All instances should be healthy");
            assertEquals(8080, inst.getPort(), "All instances should have port 8080");
        }

        // Cleanup
        for (String cluster : clusters) {
            namingService.deregisterInstance(serviceName, "192.168.11." + Math.abs(cluster.hashCode() % 256), 8080, cluster);
        }
    }
}
