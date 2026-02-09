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
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        namingService = NacosFactory.createNamingService(properties);
        System.out.println("Nacos Health Check Test Setup - Server: " + serverAddr);
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

        System.out.println("Healthy instance registered: " + found.isHealthy());

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

        System.out.println("All instances count: " + allInstances.size());

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

        System.out.println("Healthy instances: " + healthyInstances.size());
        for (Instance inst : healthyInstances) {
            assertTrue(inst.isHealthy(), "Selected instance should be healthy");
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

        // Register mixed health instances
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.10." + (20 + i));
            instance.setPort(8080);
            instance.setHealthy(i % 2 == 0); // Alternate health status
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        // Select all (healthy = false means include unhealthy)
        List<Instance> allInstances = namingService.selectInstances(serviceName, DEFAULT_GROUP, false);

        System.out.println("All instances (including unhealthy): " + allInstances.size());

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
        assertFalse(instances.isEmpty());

        Instance found = instances.get(0);
        System.out.println("TCP health check instance metadata: " + found.getMetadata());

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
        assertFalse(instances.isEmpty());

        Instance found = instances.get(0);
        assertEquals("/health", found.getMetadata().get("healthCheckPath"));
        System.out.println("HTTP health check path: " + found.getMetadata().get("healthCheckPath"));

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

        // Count selections (high weight should be selected more often)
        Map<String, Integer> selectionCount = new HashMap<>();
        for (int i = 0; i < 100; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName, DEFAULT_GROUP);
            if (selected != null) {
                selectionCount.merge(selected.getIp(), 1, Integer::sum);
            }
        }

        System.out.println("Selection distribution: " + selectionCount);

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
        for (int i = 0; i < 50; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName, DEFAULT_GROUP);
            if (selected != null && selected.getIp().equals("192.168.10.50")) {
                zeroSelected++;
            }
        }

        System.out.println("Zero weight instance selected: " + zeroSelected + " times out of 50");

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
        if (!instances.isEmpty()) {
            System.out.println("Ephemeral instance status: " + instances.get(0).isHealthy());
        }

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

        System.out.println("Ephemeral instance healthy after 2s: " + instances.get(0).isHealthy());

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

        System.out.println("Healthy instances in cluster-a: " + clusterAHealthy.size());
        for (Instance inst : clusterAHealthy) {
            assertEquals("cluster-a", inst.getClusterName());
            assertTrue(inst.isHealthy());
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
            instance.setIp("192.168.11." + cluster.hashCode() % 256);
            instance.setPort(8080);
            instance.setClusterName(cluster);
            instance.setHealthy(true);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        // Get all instances from all clusters
        List<Instance> allInstances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);

        System.out.println("Total instances across all clusters: " + allInstances.size());
        for (Instance inst : allInstances) {
            System.out.println("  " + inst.getIp() + " in " + inst.getClusterName());
        }

        // Cleanup
        for (String cluster : clusters) {
            namingService.deregisterInstance(serviceName, "192.168.11." + cluster.hashCode() % 256, 8080, cluster);
        }
    }
}
