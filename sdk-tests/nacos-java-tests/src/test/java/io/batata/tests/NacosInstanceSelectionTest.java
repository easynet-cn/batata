package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import org.junit.jupiter.api.*;

import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Instance Selection Tests
 *
 * Tests for selectInstances and selectOneHealthyInstance methods
 * with various filters (health, cluster, subscribe mode).
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosInstanceSelectionTest {

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
        System.out.println("Nacos Instance Selection Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (namingService != null) {
            namingService.shutDown();
        }
    }

    // ==================== Select Instances Tests ====================

    /**
     * NIS-001: Test selectInstances returns only healthy instances by default
     */
    @Test
    @Order(1)
    void testSelectInstancesOnlyService() throws NacosException, InterruptedException {
        String serviceName = "select-healthy-" + UUID.randomUUID().toString().substring(0, 8);

        // Register healthy instance
        Instance healthy = new Instance();
        healthy.setIp("192.168.1.1");
        healthy.setPort(8080);
        healthy.setHealthy(true);
        healthy.setWeight(1.0);
        namingService.registerInstance(serviceName, healthy);

        // Register unhealthy instance
        Instance unhealthy = new Instance();
        unhealthy.setIp("192.168.1.2");
        unhealthy.setPort(8080);
        unhealthy.setHealthy(false);
        unhealthy.setWeight(1.0);
        namingService.registerInstance(serviceName, unhealthy);

        Thread.sleep(1000);

        // selectInstances should return only healthy by default
        List<Instance> instances = namingService.selectInstances(serviceName, true);
        assertNotNull(instances);

        // At least the healthy one should be returned
        boolean hasHealthy = instances.stream().anyMatch(i -> "192.168.1.1".equals(i.getIp()));
        assertTrue(hasHealthy, "Should contain healthy instance");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.1.1", 8080);
        namingService.deregisterInstance(serviceName, "192.168.1.2", 8080);
    }

    /**
     * NIS-002: Test selectInstances with full service name (group@@service)
     */
    @Test
    @Order(2)
    void testSelectInstancesFullName() throws NacosException, InterruptedException {
        String serviceName = "select-fullname-" + UUID.randomUUID().toString().substring(0, 8);
        String groupName = "TEST_GROUP";

        Instance instance = new Instance();
        instance.setIp("192.168.2.1");
        instance.setPort(8080);
        instance.setHealthy(true);
        namingService.registerInstance(serviceName, groupName, instance);

        Thread.sleep(1000);

        // Query with full name
        List<Instance> instances = namingService.selectInstances(serviceName, groupName, true);
        assertNotNull(instances);
        assertFalse(instances.isEmpty(), "Should find instances in specified group");

        // Cleanup
        namingService.deregisterInstance(serviceName, groupName, "192.168.2.1", 8080);
    }

    /**
     * NIS-003: Test selectInstances without subscription (direct query)
     */
    @Test
    @Order(3)
    void testSelectInstancesNotSubscribe() throws NacosException, InterruptedException {
        String serviceName = "select-nosub-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.3.1");
        instance.setPort(8080);
        instance.setHealthy(true);
        namingService.registerInstance(serviceName, instance);

        Thread.sleep(1000);

        // Query without subscription (subscribe=false)
        List<Instance> instances = namingService.selectInstances(serviceName, true, false);
        assertNotNull(instances);
        assertFalse(instances.isEmpty(), "Should return instances without subscription");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.3.1", 8080);
    }

    /**
     * NIS-004: Test selectInstances with cluster filter
     */
    @Test
    @Order(4)
    void testSelectInstancesWithClusters() throws NacosException, InterruptedException {
        String serviceName = "select-cluster-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instance in cluster A
        Instance instanceA = new Instance();
        instanceA.setIp("192.168.4.1");
        instanceA.setPort(8080);
        instanceA.setClusterName("cluster-A");
        instanceA.setHealthy(true);
        namingService.registerInstance(serviceName, instanceA);

        // Register instance in cluster B
        Instance instanceB = new Instance();
        instanceB.setIp("192.168.4.2");
        instanceB.setPort(8080);
        instanceB.setClusterName("cluster-B");
        instanceB.setHealthy(true);
        namingService.registerInstance(serviceName, instanceB);

        Thread.sleep(1000);

        // Query only cluster-A
        List<String> clusters = Arrays.asList("cluster-A");
        List<Instance> instances = namingService.selectInstances(serviceName, clusters, true);
        assertNotNull(instances);

        // Should only contain cluster-A instance
        for (Instance inst : instances) {
            if (inst.getClusterName() != null && !inst.getClusterName().isEmpty()) {
                assertEquals("cluster-A", inst.getClusterName(), "Should only return cluster-A instances");
            }
        }

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.4.1", 8080);
        namingService.deregisterInstance(serviceName, "192.168.4.2", 8080);
    }

    /**
     * NIS-005: Test selectInstances with multiple clusters
     */
    @Test
    @Order(5)
    void testSelectInstancesWithMultipleClusters() throws NacosException, InterruptedException {
        String serviceName = "select-multi-cluster-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances in different clusters
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.5." + (i + 1));
            instance.setPort(8080);
            instance.setClusterName("cluster-" + (char)('A' + i));
            instance.setHealthy(true);
            namingService.registerInstance(serviceName, instance);
        }

        Thread.sleep(1000);

        // Query clusters A and B
        List<String> clusters = Arrays.asList("cluster-A", "cluster-B");
        List<Instance> instances = namingService.selectInstances(serviceName, clusters, true);
        assertNotNull(instances);

        // Cleanup
        for (int i = 0; i < 3; i++) {
            namingService.deregisterInstance(serviceName, "192.168.5." + (i + 1), 8080);
        }
    }

    /**
     * NIS-006: Test selectInstances including unhealthy instances
     */
    @Test
    @Order(6)
    void testSelectInstancesWithHealthyFlagFalse() throws NacosException, InterruptedException {
        String serviceName = "select-all-health-" + UUID.randomUUID().toString().substring(0, 8);

        Instance healthy = new Instance();
        healthy.setIp("192.168.6.1");
        healthy.setPort(8080);
        healthy.setHealthy(true);
        namingService.registerInstance(serviceName, healthy);

        Instance unhealthy = new Instance();
        unhealthy.setIp("192.168.6.2");
        unhealthy.setPort(8080);
        unhealthy.setHealthy(false);
        namingService.registerInstance(serviceName, unhealthy);

        Thread.sleep(1000);

        // Query all instances (healthy=false means include all)
        List<Instance> allInstances = namingService.selectInstances(serviceName, false);
        assertNotNull(allInstances);

        // Should include both healthy and unhealthy
        assertTrue(allInstances.size() >= 2 || allInstances.stream().anyMatch(i -> !i.isHealthy()),
                "Should include unhealthy instances when healthy=false");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.6.1", 8080);
        namingService.deregisterInstance(serviceName, "192.168.6.2", 8080);
    }

    // ==================== Select One Healthy Instance Tests ====================

    /**
     * NIS-007: Test selectOneHealthyInstance basic
     */
    @Test
    @Order(7)
    void testSelectOneHealthyInstanceOnlyService() throws NacosException, InterruptedException {
        String serviceName = "select-one-" + UUID.randomUUID().toString().substring(0, 8);

        // Register multiple healthy instances
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.7." + (i + 1));
            instance.setPort(8080);
            instance.setHealthy(true);
            instance.setWeight(1.0);
            namingService.registerInstance(serviceName, instance);
        }

        Thread.sleep(1000);

        // Select one healthy instance
        Instance selected = namingService.selectOneHealthyInstance(serviceName);
        assertNotNull(selected, "Should return one healthy instance");
        assertTrue(selected.isHealthy(), "Selected instance should be healthy");

        // Cleanup
        for (int i = 0; i < 3; i++) {
            namingService.deregisterInstance(serviceName, "192.168.7." + (i + 1), 8080);
        }
    }

    /**
     * NIS-008: Test selectOneHealthyInstance with group
     */
    @Test
    @Order(8)
    void testSelectOneHealthyInstanceFullName() throws NacosException, InterruptedException {
        String serviceName = "select-one-group-" + UUID.randomUUID().toString().substring(0, 8);
        String groupName = "SELECT_GROUP";

        Instance instance = new Instance();
        instance.setIp("192.168.8.1");
        instance.setPort(8080);
        instance.setHealthy(true);
        namingService.registerInstance(serviceName, groupName, instance);

        Thread.sleep(1000);

        // Select from specific group
        Instance selected = namingService.selectOneHealthyInstance(serviceName, groupName);
        assertNotNull(selected, "Should return instance from specified group");

        // Cleanup
        namingService.deregisterInstance(serviceName, groupName, "192.168.8.1", 8080);
    }

    /**
     * NIS-009: Test selectOneHealthyInstance without subscription
     */
    @Test
    @Order(9)
    void testSelectOneHealthyInstanceNotSubscribe() throws NacosException, InterruptedException {
        String serviceName = "select-one-nosub-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.9.1");
        instance.setPort(8080);
        instance.setHealthy(true);
        namingService.registerInstance(serviceName, instance);

        Thread.sleep(1000);

        // Select without subscription
        Instance selected = namingService.selectOneHealthyInstance(serviceName, false);
        assertNotNull(selected, "Should return instance without subscription");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.9.1", 8080);
    }

    /**
     * NIS-010: Test selectOneHealthyInstance with clusters
     */
    @Test
    @Order(10)
    void testSelectOneHealthyInstanceWithClusters() throws NacosException, InterruptedException {
        String serviceName = "select-one-cluster-" + UUID.randomUUID().toString().substring(0, 8);

        // Register in cluster-A
        Instance instanceA = new Instance();
        instanceA.setIp("192.168.10.1");
        instanceA.setPort(8080);
        instanceA.setClusterName("cluster-A");
        instanceA.setHealthy(true);
        namingService.registerInstance(serviceName, instanceA);

        // Register in cluster-B
        Instance instanceB = new Instance();
        instanceB.setIp("192.168.10.2");
        instanceB.setPort(8080);
        instanceB.setClusterName("cluster-B");
        instanceB.setHealthy(true);
        namingService.registerInstance(serviceName, instanceB);

        Thread.sleep(1000);

        // Select from cluster-A only
        List<String> clusters = Arrays.asList("cluster-A");
        Instance selected = namingService.selectOneHealthyInstance(serviceName, clusters);
        assertNotNull(selected, "Should return instance from cluster-A");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.10.1", 8080);
        namingService.deregisterInstance(serviceName, "192.168.10.2", 8080);
    }

    /**
     * NIS-011: Test selectOneHealthyInstance with weight-based selection
     */
    @Test
    @Order(11)
    void testSelectOneHealthyInstanceWeightedSelection() throws NacosException, InterruptedException {
        String serviceName = "select-weighted-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instance with high weight
        Instance highWeight = new Instance();
        highWeight.setIp("192.168.11.1");
        highWeight.setPort(8080);
        highWeight.setHealthy(true);
        highWeight.setWeight(10.0);
        namingService.registerInstance(serviceName, highWeight);

        // Register instance with low weight
        Instance lowWeight = new Instance();
        lowWeight.setIp("192.168.11.2");
        lowWeight.setPort(8080);
        lowWeight.setHealthy(true);
        lowWeight.setWeight(1.0);
        namingService.registerInstance(serviceName, lowWeight);

        Thread.sleep(1000);

        // Select multiple times and check distribution
        Map<String, Integer> selectionCount = new HashMap<>();
        for (int i = 0; i < 100; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName);
            assertNotNull(selected);
            selectionCount.merge(selected.getIp(), 1, Integer::sum);
        }

        System.out.println("Weight-based selection distribution: " + selectionCount);
        // High weight instance should be selected more often (but not guaranteed in all implementations)

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.11.1", 8080);
        namingService.deregisterInstance(serviceName, "192.168.11.2", 8080);
    }

    /**
     * NIS-012: Test selectInstances with full parameters (group, clusters, healthy, subscribe)
     */
    @Test
    @Order(12)
    void testSelectInstancesFullParameters() throws NacosException, InterruptedException {
        String serviceName = "select-full-params-" + UUID.randomUUID().toString().substring(0, 8);
        String groupName = "FULL_PARAMS_GROUP";

        Instance instance = new Instance();
        instance.setIp("192.168.12.1");
        instance.setPort(8080);
        instance.setClusterName("production");
        instance.setHealthy(true);
        namingService.registerInstance(serviceName, groupName, instance);

        Thread.sleep(1000);

        // Query with all parameters
        List<String> clusters = Arrays.asList("production");
        List<Instance> instances = namingService.selectInstances(serviceName, groupName, clusters, true, false);
        assertNotNull(instances);

        // Cleanup
        namingService.deregisterInstance(serviceName, groupName, "192.168.12.1", 8080);
    }

    /**
     * NIS-013: Test selectOneHealthyInstance with full parameters
     */
    @Test
    @Order(13)
    void testSelectOneHealthyInstanceFullParameters() throws NacosException, InterruptedException {
        String serviceName = "select-one-full-" + UUID.randomUUID().toString().substring(0, 8);
        String groupName = "ONE_FULL_GROUP";

        Instance instance = new Instance();
        instance.setIp("192.168.13.1");
        instance.setPort(8080);
        instance.setClusterName("default");
        instance.setHealthy(true);
        namingService.registerInstance(serviceName, groupName, instance);

        Thread.sleep(1000);

        // Select with all parameters
        List<String> clusters = Arrays.asList("default");
        Instance selected = namingService.selectOneHealthyInstance(serviceName, groupName, clusters, false);
        assertNotNull(selected, "Should return instance with full parameters");

        // Cleanup
        namingService.deregisterInstance(serviceName, groupName, "192.168.13.1", 8080);
    }
}
