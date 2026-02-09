package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import com.alibaba.nacos.api.naming.pojo.ServiceInfo;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Cluster Management Tests
 *
 * Tests for cluster-related functionality including:
 * - Cluster instance management
 * - Cross-cluster service discovery
 * - Cluster health and status
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosClusterManagementTest {

    private static NamingService namingService;
    private static ConfigService configService;
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
        configService = NacosFactory.createConfigService(properties);

        System.out.println("Nacos Cluster Management Test Setup - Server: " + serverAddr);
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

    // ==================== Cluster Instance Tests ====================

    /**
     * NCM-001: Test register instance with cluster name
     */
    @Test
    @Order(1)
    void testRegisterInstanceWithCluster() throws NacosException, InterruptedException {
        String serviceName = "cluster-service-" + UUID.randomUUID().toString().substring(0, 8);
        String clusterName = "cluster-a";

        Instance instance = new Instance();
        instance.setIp("192.168.1.100");
        instance.setPort(8080);
        instance.setClusterName(clusterName);
        instance.setWeight(1.0);
        instance.setHealthy(true);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);

        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have registered instance");

        Instance found = instances.get(0);
        assertEquals(clusterName, found.getClusterName(), "Cluster name should match");

        System.out.println("Registered instance in cluster: " + found.getClusterName());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NCM-002: Test register instances in multiple clusters
     */
    @Test
    @Order(2)
    void testMultipleClusterInstances() throws NacosException, InterruptedException {
        String serviceName = "multi-cluster-" + UUID.randomUUID().toString().substring(0, 8);
        String[] clusters = {"cluster-a", "cluster-b", "cluster-c"};

        // Register instance in each cluster
        for (int i = 0; i < clusters.length; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.1." + (100 + i));
            instance.setPort(8080);
            instance.setClusterName(clusters[i]);
            instance.setWeight(1.0);
            instance.setHealthy(true);

            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        // Get all instances
        List<Instance> allInstances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertEquals(3, allInstances.size(), "Should have 3 instances across clusters");

        // Count instances per cluster
        Map<String, Integer> clusterCounts = new HashMap<>();
        for (Instance inst : allInstances) {
            clusterCounts.merge(inst.getClusterName(), 1, Integer::sum);
        }

        System.out.println("Cluster distribution: " + clusterCounts);

        // Cleanup
        for (int i = 0; i < clusters.length; i++) {
            namingService.deregisterInstance(serviceName, "192.168.1." + (100 + i), 8080, clusters[i]);
        }
    }

    /**
     * NCM-003: Test get instances by cluster
     */
    @Test
    @Order(3)
    void testGetInstancesByCluster() throws NacosException, InterruptedException {
        String serviceName = "cluster-filter-" + UUID.randomUUID().toString().substring(0, 8);

        // Register in cluster-a
        Instance instanceA = new Instance();
        instanceA.setIp("192.168.1.1");
        instanceA.setPort(8080);
        instanceA.setClusterName("cluster-a");
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instanceA);

        // Register in cluster-b
        Instance instanceB = new Instance();
        instanceB.setIp("192.168.1.2");
        instanceB.setPort(8080);
        instanceB.setClusterName("cluster-b");
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instanceB);

        Thread.sleep(2000);

        // Verify all instances are registered first (no cluster filter)
        List<Instance> allInstances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertEquals(2, allInstances.size(), "Should have 2 total instances registered");

        Thread.sleep(500);

        // Get instances from cluster-a only (subscribe=false for direct query)
        List<Instance> clusterAInstances = namingService.getAllInstances(
                serviceName, DEFAULT_GROUP, Arrays.asList("cluster-a"), false);
        assertEquals(1, clusterAInstances.size(), "Should have 1 instance in cluster-a");
        assertEquals("cluster-a", clusterAInstances.get(0).getClusterName());

        // Get instances from both clusters (subscribe=false for direct query)
        List<Instance> bothClusters = namingService.getAllInstances(
                serviceName, DEFAULT_GROUP, Arrays.asList("cluster-a", "cluster-b"), false);
        assertEquals(2, bothClusters.size(), "Should have 2 instances in both clusters");

        System.out.println("Cluster-a instances: " + clusterAInstances.size());
        System.out.println("Both clusters instances: " + bothClusters.size());

        // Cleanup
        namingService.deregisterInstance(serviceName, instanceA);
        namingService.deregisterInstance(serviceName, instanceB);
    }

    /**
     * NCM-004: Test select healthy instances by cluster
     */
    @Test
    @Order(4)
    void testSelectHealthyInstancesByCluster() throws NacosException, InterruptedException {
        String serviceName = "healthy-cluster-" + UUID.randomUUID().toString().substring(0, 8);

        // Register healthy instance in cluster-a
        Instance healthyA = new Instance();
        healthyA.setIp("192.168.1.10");
        healthyA.setPort(8080);
        healthyA.setClusterName("cluster-a");
        healthyA.setHealthy(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, healthyA);

        // Register unhealthy instance in cluster-a
        Instance unhealthyA = new Instance();
        unhealthyA.setIp("192.168.1.11");
        unhealthyA.setPort(8080);
        unhealthyA.setClusterName("cluster-a");
        unhealthyA.setHealthy(false);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, unhealthyA);

        Thread.sleep(1000);

        // Select only healthy instances from cluster-a
        List<Instance> healthyInstances = namingService.selectInstances(
                serviceName, DEFAULT_GROUP, Arrays.asList("cluster-a"), true);

        System.out.println("Healthy instances in cluster-a: " + healthyInstances.size());

        // Cleanup
        namingService.deregisterInstance(serviceName, healthyA);
        namingService.deregisterInstance(serviceName, unhealthyA);
    }

    // ==================== Service Info Tests ====================

    /**
     * NCM-005: Test get service info with clusters
     */
    @Test
    @Order(5)
    void testGetServiceInfoWithClusters() throws NacosException, InterruptedException {
        String serviceName = "service-info-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.1.50");
        instance.setPort(8080);
        instance.setClusterName("default");
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);

        Thread.sleep(1000);

        // Subscribe to get service info
        namingService.subscribe(serviceName, DEFAULT_GROUP, event -> {
            System.out.println("Service event received");
        });

        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have service instances");

        System.out.println("Service instances: " + instances.size());

        // Cleanup
        namingService.unsubscribe(serviceName, DEFAULT_GROUP, event -> {});
        namingService.deregisterInstance(serviceName, instance);
    }

    /**
     * NCM-006: Test subscribe to service with cluster filter
     */
    @Test
    @Order(6)
    void testSubscribeWithClusterFilter() throws NacosException, InterruptedException {
        String serviceName = "subscribe-cluster-" + UUID.randomUUID().toString().substring(0, 8);
        CountDownLatch latch = new CountDownLatch(1);
        List<Instance> receivedInstances = new ArrayList<>();

        // Register instance
        Instance instance = new Instance();
        instance.setIp("192.168.1.60");
        instance.setPort(8080);
        instance.setClusterName("primary");
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);

        Thread.sleep(500);

        // Subscribe with cluster filter
        namingService.subscribe(serviceName, DEFAULT_GROUP, Arrays.asList("primary"), event -> {
            if (event instanceof com.alibaba.nacos.api.naming.listener.NamingEvent) {
                com.alibaba.nacos.api.naming.listener.NamingEvent namingEvent = (com.alibaba.nacos.api.naming.listener.NamingEvent) event;
                if (namingEvent.getInstances() != null) {
                    receivedInstances.addAll(namingEvent.getInstances());
                    latch.countDown();
                }
            }
        });

        boolean received = latch.await(5, TimeUnit.SECONDS);
        System.out.println("Received subscription event: " + received);
        System.out.println("Instances in subscription: " + receivedInstances.size());

        // Cleanup
        namingService.deregisterInstance(serviceName, instance);
    }

    // ==================== Instance Weight and Priority Tests ====================

    /**
     * NCM-007: Test instance weight in cluster
     */
    @Test
    @Order(7)
    void testInstanceWeightInCluster() throws NacosException, InterruptedException {
        String serviceName = "weight-cluster-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances with different weights
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.1." + (70 + i));
            instance.setPort(8080);
            instance.setClusterName("weighted");
            instance.setWeight((i + 1) * 10.0); // 10, 20, 30
            instance.setHealthy(true);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(
                serviceName, DEFAULT_GROUP, Arrays.asList("weighted"));

        assertEquals(3, instances.size(), "Should have 3 weighted instances");

        for (Instance inst : instances) {
            System.out.println("Instance " + inst.getIp() + " weight: " + inst.getWeight());
        }

        // Cleanup
        for (int i = 0; i < 3; i++) {
            namingService.deregisterInstance(serviceName, "192.168.1." + (70 + i), 8080, "weighted");
        }
    }

    /**
     * NCM-008: Test update instance weight
     */
    @Test
    @Order(8)
    void testUpdateInstanceWeight() throws NacosException, InterruptedException {
        String serviceName = "update-weight-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.1.80");
        instance.setPort(8080);
        instance.setClusterName("default");
        instance.setWeight(50.0);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);

        Thread.sleep(500);

        // Update weight
        instance.setWeight(100.0);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);

        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have instance");
        assertEquals(100.0, instances.get(0).getWeight(), 0.01, "Weight should be updated");

        System.out.println("Updated weight: " + instances.get(0).getWeight());

        // Cleanup
        namingService.deregisterInstance(serviceName, instance);
    }

    // ==================== Ephemeral vs Persistent Tests ====================

    /**
     * NCM-009: Test ephemeral instance registration
     */
    @Test
    @Order(9)
    void testEphemeralInstance() throws NacosException, InterruptedException {
        String serviceName = "ephemeral-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.1.90");
        instance.setPort(8080);
        instance.setClusterName("default");
        instance.setEphemeral(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);

        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have ephemeral instance");
        assertTrue(instances.get(0).isEphemeral(), "Instance should be ephemeral");

        System.out.println("Ephemeral instance registered: " + instances.get(0).isEphemeral());

        // Cleanup
        namingService.deregisterInstance(serviceName, instance);
    }

    /**
     * NCM-010: Test ephemeral instance with metadata in cluster
     */
    @Test
    @Order(10)
    void testEphemeralInstanceWithMetadataInCluster() throws NacosException, InterruptedException {
        String serviceName = "ephemeral-meta-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.1.91");
        instance.setPort(8080);
        instance.setClusterName("default");
        instance.setEphemeral(true);

        Map<String, String> metadata = new HashMap<>();
        metadata.put("app", "test-app");
        metadata.put("version", "1.0");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have ephemeral instance");
        assertTrue(instances.get(0).isEphemeral(), "Instance should be ephemeral");
        assertEquals("test-app", instances.get(0).getMetadata().get("app"));

        System.out.println("Ephemeral instance with metadata: " + instances.get(0).getMetadata());

        // Cleanup
        namingService.deregisterInstance(serviceName, instance);
    }

    // ==================== Metadata Tests ====================

    /**
     * NCM-011: Test instance with rich metadata
     */
    @Test
    @Order(11)
    void testInstanceWithRichMetadata() throws NacosException, InterruptedException {
        String serviceName = "metadata-rich-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.1.100");
        instance.setPort(8080);
        instance.setClusterName("default");

        Map<String, String> metadata = new HashMap<>();
        metadata.put("version", "v2.0.0");
        metadata.put("region", "us-west-1");
        metadata.put("zone", "zone-a");
        metadata.put("env", "production");
        metadata.put("protocol", "grpc");
        metadata.put("weight-override", "true");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);

        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have instance with metadata");

        Map<String, String> retrievedMeta = instances.get(0).getMetadata();
        assertEquals("v2.0.0", retrievedMeta.get("version"));
        assertEquals("us-west-1", retrievedMeta.get("region"));

        System.out.println("Instance metadata: " + retrievedMeta);

        // Cleanup
        namingService.deregisterInstance(serviceName, instance);
    }

    /**
     * NCM-012: Test update instance metadata
     */
    @Test
    @Order(12)
    void testUpdateInstanceMetadata() throws NacosException, InterruptedException {
        String serviceName = "metadata-update-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.1.101");
        instance.setPort(8080);
        instance.setClusterName("default");

        Map<String, String> metadata = new HashMap<>();
        metadata.put("version", "v1.0.0");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(500);

        // Update metadata
        metadata.put("version", "v2.0.0");
        metadata.put("updated", "true");
        instance.setMetadata(metadata);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);

        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have updated instance");

        Map<String, String> updatedMeta = instances.get(0).getMetadata();
        System.out.println("Updated metadata: " + updatedMeta);

        // Cleanup
        namingService.deregisterInstance(serviceName, instance);
    }

    // ==================== Concurrent Operations Tests ====================

    /**
     * NCM-013: Test concurrent instance registration
     */
    @Test
    @Order(13)
    void testConcurrentInstanceRegistration() throws Exception {
        String serviceName = "concurrent-reg-" + UUID.randomUUID().toString().substring(0, 8);
        int threadCount = 10;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(threadCount);

        for (int i = 0; i < threadCount; i++) {
            final int index = i;
            new Thread(() -> {
                try {
                    startLatch.await();

                    Instance instance = new Instance();
                    instance.setIp("192.168.2." + index);
                    instance.setPort(8080);
                    instance.setClusterName("concurrent");
                    instance.setHealthy(true);

                    namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
                } catch (Exception e) {
                    System.err.println("Registration error: " + e.getMessage());
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        startLatch.countDown();
        boolean completed = doneLatch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All registrations should complete");

        Thread.sleep(2000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        System.out.println("Concurrent registered instances: " + instances.size());

        // Cleanup
        for (int i = 0; i < threadCount; i++) {
            try {
                namingService.deregisterInstance(serviceName, "192.168.2." + i, 8080, "concurrent");
            } catch (Exception e) {
                // Ignore cleanup errors
            }
        }
    }

    /**
     * NCM-014: Test concurrent service subscription
     */
    @Test
    @Order(14)
    void testConcurrentSubscription() throws Exception {
        String serviceName = "concurrent-sub-" + UUID.randomUUID().toString().substring(0, 8);
        int subscriberCount = 5;

        // Register a service first
        Instance instance = new Instance();
        instance.setIp("192.168.3.1");
        instance.setPort(8080);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);

        Thread.sleep(500);

        CountDownLatch latch = new CountDownLatch(subscriberCount);
        List<com.alibaba.nacos.api.naming.listener.EventListener> listeners = new ArrayList<>();

        for (int i = 0; i < subscriberCount; i++) {
            final int index = i;
            com.alibaba.nacos.api.naming.listener.EventListener listener = event -> {
                System.out.println("Subscriber " + index + " received event");
                latch.countDown();
            };
            listeners.add(listener);
            namingService.subscribe(serviceName, DEFAULT_GROUP, listener);
        }

        boolean allReceived = latch.await(10, TimeUnit.SECONDS);
        System.out.println("All subscribers received events: " + allReceived);

        // Cleanup
        for (com.alibaba.nacos.api.naming.listener.EventListener listener : listeners) {
            namingService.unsubscribe(serviceName, DEFAULT_GROUP, listener);
        }
        namingService.deregisterInstance(serviceName, instance);
    }

    /**
     * NCM-015: Test service list pagination
     */
    @Test
    @Order(15)
    void testServiceListPagination() throws NacosException, InterruptedException {
        String prefix = "paginate-" + UUID.randomUUID().toString().substring(0, 8);

        // Register multiple services
        for (int i = 0; i < 5; i++) {
            String serviceName = prefix + "-service-" + i;
            Instance instance = new Instance();
            instance.setIp("192.168.4." + i);
            instance.setPort(8080);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        // Get service list with pagination
        com.alibaba.nacos.api.naming.pojo.ListView<String> serviceList =
                namingService.getServicesOfServer(1, 10, DEFAULT_GROUP);

        System.out.println("Service count: " + serviceList.getCount());
        System.out.println("Services: " + serviceList.getData());

        // Cleanup
        for (int i = 0; i < 5; i++) {
            String serviceName = prefix + "-service-" + i;
            namingService.deregisterInstance(serviceName, "192.168.4." + i, 8080);
        }
    }
}
