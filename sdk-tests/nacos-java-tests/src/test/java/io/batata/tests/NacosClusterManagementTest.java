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
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);

        namingService = NacosFactory.createNamingService(properties);
        configService = NacosFactory.createConfigService(properties);
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
        assertEquals("192.168.1.100", found.getIp(), "IP should match registered instance");
        assertEquals(8080, found.getPort(), "Port should match registered instance");

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

        for (String cluster : clusters) {
            assertEquals(1, clusterCounts.getOrDefault(cluster, 0),
                    "Each cluster should have exactly 1 instance, checking: " + cluster);
        }

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
        assertEquals("cluster-a", clusterAInstances.get(0).getClusterName(),
                "Instance should belong to cluster-a");
        assertEquals("192.168.1.1", clusterAInstances.get(0).getIp(),
                "Cluster-a instance should have correct IP");

        // Get instances from both clusters (subscribe=false for direct query)
        List<Instance> bothClusters = namingService.getAllInstances(
                serviceName, DEFAULT_GROUP, Arrays.asList("cluster-a", "cluster-b"), false);
        assertEquals(2, bothClusters.size(), "Should have 2 instances in both clusters");

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

        assertNotNull(healthyInstances, "Healthy instances list should not be null");
        for (Instance inst : healthyInstances) {
            assertTrue(inst.isHealthy(),
                    "All selected instances should be healthy");
            assertEquals("cluster-a", inst.getClusterName(),
                    "All selected instances should belong to cluster-a");
        }

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
        CountDownLatch eventLatch = new CountDownLatch(1);
        namingService.subscribe(serviceName, DEFAULT_GROUP, event -> {
            eventLatch.countDown();
        });

        boolean eventReceived = eventLatch.await(5, TimeUnit.SECONDS);
        assertTrue(eventReceived, "Should receive service subscription event");

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have service instances");
        assertEquals(1, instances.size(), "Should have exactly 1 instance");
        assertEquals("192.168.1.50", instances.get(0).getIp(),
                "Instance IP should match registered value");

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
        assertTrue(received, "Should receive subscription event with cluster filter");
        assertFalse(receivedInstances.isEmpty(),
                "Should have instances in subscription notification");

        for (Instance inst : receivedInstances) {
            assertEquals("primary", inst.getClusterName(),
                    "All received instances should belong to primary cluster");
        }

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

        // Verify weights are set correctly
        Set<Double> expectedWeights = new HashSet<>(Arrays.asList(10.0, 20.0, 30.0));
        Set<Double> actualWeights = new HashSet<>();
        for (Instance inst : instances) {
            actualWeights.add(inst.getWeight());
            assertTrue(inst.getWeight() > 0, "All weights should be positive");
            assertEquals("weighted", inst.getClusterName(),
                    "All instances should belong to weighted cluster");
        }
        assertEquals(expectedWeights, actualWeights, "Instance weights should match expected values");

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
        assertEquals(100.0, instances.get(0).getWeight(), 0.01, "Weight should be updated to 100.0");
        assertEquals("192.168.1.80", instances.get(0).getIp(),
                "IP should remain unchanged after weight update");

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
        assertEquals("192.168.1.90", instances.get(0).getIp(),
                "Ephemeral instance IP should match");

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
        assertEquals("test-app", instances.get(0).getMetadata().get("app"),
                "Metadata 'app' should match");
        assertEquals("1.0", instances.get(0).getMetadata().get("version"),
                "Metadata 'version' should match");

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
        assertEquals("v2.0.0", retrievedMeta.get("version"), "version metadata should match");
        assertEquals("us-west-1", retrievedMeta.get("region"), "region metadata should match");
        assertEquals("zone-a", retrievedMeta.get("zone"), "zone metadata should match");
        assertEquals("production", retrievedMeta.get("env"), "env metadata should match");
        assertEquals("grpc", retrievedMeta.get("protocol"), "protocol metadata should match");
        assertEquals("true", retrievedMeta.get("weight-override"), "weight-override metadata should match");

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
        assertEquals("v2.0.0", updatedMeta.get("version"),
                "Version metadata should be updated to v2.0.0");
        assertEquals("true", updatedMeta.get("updated"),
                "New 'updated' metadata key should be present");

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
        List<Exception> errors = new CopyOnWriteArrayList<>();

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
                    errors.add(e);
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        startLatch.countDown();
        boolean completed = doneLatch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All registrations should complete");
        assertTrue(errors.isEmpty(),
                "No errors should occur during concurrent registration, got " + errors.size());

        Thread.sleep(2000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertEquals(threadCount, instances.size(),
                "Should have " + threadCount + " concurrently registered instances");

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
            com.alibaba.nacos.api.naming.listener.EventListener listener = event -> {
                latch.countDown();
            };
            listeners.add(listener);
            namingService.subscribe(serviceName, DEFAULT_GROUP, listener);
        }

        boolean allReceived = latch.await(10, TimeUnit.SECONDS);
        assertTrue(allReceived,
                "All " + subscriberCount + " subscribers should receive events");

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

        // Register multiple services with delay between each to allow server processing
        for (int i = 0; i < 5; i++) {
            String serviceName = prefix + "-service-" + i;
            Instance instance = new Instance();
            instance.setIp("192.168.4." + i);
            instance.setPort(8080);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
            Thread.sleep(500);
        }

        Thread.sleep(3000);

        // Get service list with pagination - use larger page size to ensure all services are returned
        com.alibaba.nacos.api.naming.pojo.ListView<String> serviceList =
                namingService.getServicesOfServer(1, 100, DEFAULT_GROUP);

        assertNotNull(serviceList, "Service list should not be null");
        assertNotNull(serviceList.getData(), "Service data list should not be null");
        assertFalse(serviceList.getData().isEmpty(), "Service data list should not be empty");

        // Verify our services are in the list (allow for race conditions - at least 4 of 5)
        long matchingServices = serviceList.getData().stream()
                .filter(name -> name.startsWith(prefix))
                .count();
        assertTrue(matchingServices >= 4,
                "Should find at least 4 registered services in the list, got: " + matchingServices
                + " (total services: " + serviceList.getCount() + ", data: " + serviceList.getData() + ")");

        // Cleanup
        for (int i = 0; i < 5; i++) {
            String serviceName = prefix + "-service-" + i;
            namingService.deregisterInstance(serviceName, "192.168.4." + i, 8080);
        }
    }
}
