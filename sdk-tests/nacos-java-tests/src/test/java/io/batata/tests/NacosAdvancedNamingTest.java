package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.listener.EventListener;
import com.alibaba.nacos.api.naming.listener.NamingEvent;
import com.alibaba.nacos.api.naming.pojo.Instance;
import com.alibaba.nacos.api.naming.pojo.ListView;
import com.alibaba.nacos.api.naming.pojo.ServiceInfo;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Advanced Naming Service Tests
 *
 * Tests for batch operations, custom selectors, and advanced naming features.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosAdvancedNamingTest {

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
        System.out.println("Nacos Advanced Naming Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (namingService != null) {
            namingService.shutDown();
        }
    }

    // ==================== Batch Register Tests ====================

    /**
     * NAN-001: Test batch register instances
     */
    @Test
    @Order(1)
    void testBatchRegisterInstance() throws NacosException, InterruptedException {
        String serviceName = "batch-register-" + UUID.randomUUID().toString().substring(0, 8);

        List<Instance> instances = new ArrayList<>();
        for (int i = 0; i < 5; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.100." + (i + 1));
            instance.setPort(8080);
            instance.setWeight(1.0);
            instance.setHealthy(true);
            instance.setMetadata(Map.of("index", String.valueOf(i)));
            instances.add(instance);
        }

        // Batch register
        namingService.batchRegisterInstance(serviceName, DEFAULT_GROUP, instances);
        Thread.sleep(1000);

        // Verify all registered
        List<Instance> allInstances = namingService.getAllInstances(serviceName);
        assertEquals(5, allInstances.size(), "All 5 instances should be registered");

        // Cleanup
        for (Instance inst : instances) {
            namingService.deregisterInstance(serviceName, inst.getIp(), inst.getPort());
        }
    }

    /**
     * NAN-002: Test batch register with group name prefix
     */
    @Test
    @Order(2)
    void testBatchRegisterInstanceWithGroupNamePrefix() throws NacosException, InterruptedException {
        String serviceName = "batch-group-" + UUID.randomUUID().toString().substring(0, 8);
        String groupName = "BATCH_GROUP";

        List<Instance> instances = new ArrayList<>();
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.101." + (i + 1));
            instance.setPort(8080);
            instances.add(instance);
        }

        // Batch register with group
        namingService.batchRegisterInstance(serviceName, groupName, instances);
        Thread.sleep(1000);

        // Verify in correct group
        List<Instance> allInstances = namingService.getAllInstances(serviceName, groupName);
        assertEquals(3, allInstances.size(), "All 3 instances should be in BATCH_GROUP");

        // Cleanup
        for (Instance inst : instances) {
            namingService.deregisterInstance(serviceName, groupName, inst.getIp(), inst.getPort());
        }
    }

    /**
     * NAN-003: Test batch register with different clusters
     */
    @Test
    @Order(3)
    void testBatchRegisterWithClusters() throws NacosException, InterruptedException {
        String serviceName = "batch-cluster-" + UUID.randomUUID().toString().substring(0, 8);

        List<Instance> instances = new ArrayList<>();

        // Instances in cluster-A
        for (int i = 0; i < 2; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.102." + (i + 1));
            instance.setPort(8080);
            instance.setClusterName("cluster-A");
            instances.add(instance);
        }

        // Instances in cluster-B
        for (int i = 0; i < 2; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.102." + (i + 10));
            instance.setPort(8080);
            instance.setClusterName("cluster-B");
            instances.add(instance);
        }

        namingService.batchRegisterInstance(serviceName, DEFAULT_GROUP, instances);
        Thread.sleep(1000);

        // Query by cluster
        List<Instance> clusterA = namingService.selectInstances(serviceName,
                Arrays.asList("cluster-A"), true);
        List<Instance> clusterB = namingService.selectInstances(serviceName,
                Arrays.asList("cluster-B"), true);

        System.out.println("Cluster-A instances: " + clusterA.size());
        System.out.println("Cluster-B instances: " + clusterB.size());

        // Cleanup
        for (Instance inst : instances) {
            namingService.deregisterInstance(serviceName, inst.getIp(), inst.getPort());
        }
    }

    // ==================== Batch Deregister Tests ====================

    /**
     * NAN-004: Test batch deregister instances
     */
    @Test
    @Order(4)
    void testBatchDeregisterInstance() throws NacosException, InterruptedException {
        String serviceName = "batch-dereg-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances using batch register (required for batch deregister)
        List<Instance> instances = new ArrayList<>();
        for (int i = 0; i < 4; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.103." + (i + 1));
            instance.setPort(8080);
            instances.add(instance);
        }
        namingService.batchRegisterInstance(serviceName, DEFAULT_GROUP, instances);

        Thread.sleep(1000);

        // Verify registered
        List<Instance> before = namingService.getAllInstances(serviceName);
        assertEquals(4, before.size(), "Should have 4 instances before deregister");

        // Batch deregister
        namingService.batchDeregisterInstance(serviceName, DEFAULT_GROUP, instances);
        Thread.sleep(1000);

        // Verify deregistered
        List<Instance> after = namingService.getAllInstances(serviceName);
        assertTrue(after.isEmpty(), "All instances should be deregistered");
    }

    /**
     * NAN-005: Test partial batch deregister
     */
    @Test
    @Order(5)
    void testPartialBatchDeregister() throws NacosException, InterruptedException {
        String serviceName = "partial-dereg-" + UUID.randomUUID().toString().substring(0, 8);

        // Register 5 instances using batch register (required for batch deregister)
        List<Instance> instances = new ArrayList<>();
        for (int i = 0; i < 5; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.104." + (i + 1));
            instance.setPort(8080);
            instances.add(instance);
        }
        namingService.batchRegisterInstance(serviceName, DEFAULT_GROUP, instances);

        Thread.sleep(1000);

        // Deregister only 3
        List<Instance> toDeregister = new ArrayList<>();
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.104." + (i + 1));
            instance.setPort(8080);
            toDeregister.add(instance);
        }

        namingService.batchDeregisterInstance(serviceName, DEFAULT_GROUP, toDeregister);
        Thread.sleep(1000);

        // Verify 2 remaining
        List<Instance> remaining = namingService.getAllInstances(serviceName);
        assertEquals(2, remaining.size(), "Should have 2 instances remaining");

        // Cleanup
        for (Instance inst : remaining) {
            namingService.deregisterInstance(serviceName, inst.getIp(), inst.getPort());
        }
    }

    // ==================== Subscribe Tests ====================

    /**
     * NAN-006: Test subscribe with clusters
     */
    @Test
    @Order(6)
    void testSubscribeWithClusters() throws NacosException, InterruptedException {
        String serviceName = "subscribe-cluster-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicReference<List<Instance>> received = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        // Subscribe to specific cluster
        List<String> clusters = Arrays.asList("production");
        EventListener listener = event -> {
            if (event instanceof NamingEvent) {
                received.set(((NamingEvent) event).getInstances());
                latch.countDown();
            }
        };

        namingService.subscribe(serviceName, clusters, listener);
        Thread.sleep(500);

        // Register instance in production cluster
        Instance instance = new Instance();
        instance.setIp("192.168.105.1");
        instance.setPort(8080);
        instance.setClusterName("production");
        namingService.registerInstance(serviceName, instance);

        boolean notified = latch.await(10, TimeUnit.SECONDS);
        assertTrue(notified, "Should receive cluster-specific notification");
        assertNotNull(received.get());

        // Cleanup
        namingService.unsubscribe(serviceName, clusters, listener);
        namingService.deregisterInstance(serviceName, "192.168.105.1", 8080);
    }

    /**
     * NAN-007: Test subscribe duplicate handling
     */
    @Test
    @Order(7)
    void testSubscribeDuplicate() throws NacosException, InterruptedException {
        String serviceName = "subscribe-dup-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicInteger notifyCount = new AtomicInteger(0);

        EventListener listener = event -> {
            if (event instanceof NamingEvent) {
                notifyCount.incrementAndGet();
            }
        };

        // Subscribe twice with same listener
        namingService.subscribe(serviceName, listener);
        namingService.subscribe(serviceName, listener);

        Thread.sleep(500);

        // Register instance
        namingService.registerInstance(serviceName, "192.168.106.1", 8080);
        Thread.sleep(2000);

        // Should not receive duplicate notifications
        System.out.println("Notification count after duplicate subscribe: " + notifyCount.get());

        // Cleanup
        namingService.unsubscribe(serviceName, listener);
        namingService.deregisterInstance(serviceName, "192.168.106.1", 8080);
    }

    /**
     * NAN-008: Test unsubscribe removes listener
     */
    @Test
    @Order(8)
    void testUnsubscribeRemovesListener() throws NacosException, InterruptedException {
        String serviceName = "unsubscribe-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicInteger notifyCount = new AtomicInteger(0);

        EventListener listener = event -> {
            if (event instanceof NamingEvent) {
                notifyCount.incrementAndGet();
            }
        };

        // Subscribe
        namingService.subscribe(serviceName, listener);
        Thread.sleep(500);

        // Register first instance (should notify)
        namingService.registerInstance(serviceName, "192.168.107.1", 8080);
        Thread.sleep(1000);

        int countAfterFirst = notifyCount.get();

        // Unsubscribe
        namingService.unsubscribe(serviceName, listener);
        Thread.sleep(500);

        // Register second instance (should not notify)
        namingService.registerInstance(serviceName, "192.168.107.2", 8080);
        Thread.sleep(1000);

        int countAfterSecond = notifyCount.get();
        assertEquals(countAfterFirst, countAfterSecond, "Should not receive notifications after unsubscribe");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.107.1", 8080);
        namingService.deregisterInstance(serviceName, "192.168.107.2", 8080);
    }

    // ==================== Server Status Tests ====================

    /**
     * NAN-009: Test get server status
     */
    @Test
    @Order(9)
    void testGetServerStatus() throws NacosException {
        String status = namingService.getServerStatus();
        assertNotNull(status, "Server status should not be null");
        System.out.println("Server status: " + status);
        assertTrue(status.equals("UP") || status.equals("DOWN"),
                "Status should be UP or DOWN");
    }

    /**
     * NAN-010: Test get services of server with pagination
     */
    @Test
    @Order(10)
    void testGetServicesOfServerPagination() throws NacosException, InterruptedException {
        String prefix = "pagination-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Register multiple services
        for (int i = 0; i < 5; i++) {
            String serviceName = prefix + "-" + i;
            namingService.registerInstance(serviceName, "192.168.108." + (i + 1), 8080);
        }

        Thread.sleep(1000);

        // Test pagination
        ListView<String> page1 = namingService.getServicesOfServer(1, 2);
        assertNotNull(page1);
        System.out.println("Page 1 count: " + page1.getCount() + ", data: " + page1.getData());

        ListView<String> page2 = namingService.getServicesOfServer(2, 2);
        assertNotNull(page2);
        System.out.println("Page 2 count: " + page2.getCount() + ", data: " + page2.getData());

        // Cleanup
        for (int i = 0; i < 5; i++) {
            String serviceName = prefix + "-" + i;
            namingService.deregisterInstance(serviceName, "192.168.108." + (i + 1), 8080);
        }
    }

    /**
     * NAN-011: Test get services of server with group
     */
    @Test
    @Order(11)
    void testGetServicesOfServerWithGroup() throws NacosException, InterruptedException {
        String prefix = "group-svc-" + UUID.randomUUID().toString().substring(0, 8);
        String groupName = "TEST_SERVICES_GROUP";

        // Register services in specific group
        for (int i = 0; i < 3; i++) {
            String serviceName = prefix + "-" + i;
            namingService.registerInstance(serviceName, groupName, "192.168.109." + (i + 1), 8080);
        }

        Thread.sleep(1000);

        // Get services in group
        ListView<String> services = namingService.getServicesOfServer(1, 10, groupName);
        assertNotNull(services);
        System.out.println("Services in group: " + services.getCount() + ", data: " + services.getData());

        // Cleanup
        for (int i = 0; i < 3; i++) {
            String serviceName = prefix + "-" + i;
            namingService.deregisterInstance(serviceName, groupName, "192.168.109." + (i + 1), 8080);
        }
    }

    /**
     * NAN-012: Test get subscribe services
     */
    @Test
    @Order(12)
    void testGetSubscribeServices() throws NacosException, InterruptedException {
        String serviceName1 = "subscribed-svc-1-" + UUID.randomUUID().toString().substring(0, 8);
        String serviceName2 = "subscribed-svc-2-" + UUID.randomUUID().toString().substring(0, 8);

        EventListener listener = event -> {};

        // Subscribe to multiple services
        namingService.subscribe(serviceName1, listener);
        namingService.subscribe(serviceName2, listener);

        Thread.sleep(500);

        // Get subscribed services
        List<ServiceInfo> subscribed = namingService.getSubscribeServices();
        assertNotNull(subscribed);
        System.out.println("Subscribed services count: " + subscribed.size());

        // Cleanup
        namingService.unsubscribe(serviceName1, listener);
        namingService.unsubscribe(serviceName2, listener);
    }

    // ==================== Instance Metadata Tests ====================

    /**
     * NAN-013: Test register instance with metadata
     */
    @Test
    @Order(13)
    void testRegisterWithMetadata() throws NacosException, InterruptedException {
        String serviceName = "metadata-svc-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.110.1");
        instance.setPort(8080);
        instance.setMetadata(Map.of(
                "version", "1.0.0",
                "region", "us-west-2",
                "env", "production",
                "team", "platform"
        ));

        namingService.registerInstance(serviceName, instance);
        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty());

        Instance retrieved = instances.get(0);
        assertEquals("1.0.0", retrieved.getMetadata().get("version"));
        assertEquals("us-west-2", retrieved.getMetadata().get("region"));

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.110.1", 8080);
    }

    /**
     * NAN-014: Test update instance metadata
     */
    @Test
    @Order(14)
    void testUpdateInstanceMetadata() throws NacosException, InterruptedException {
        String serviceName = "update-meta-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Register with initial metadata
        Instance instance = new Instance();
        instance.setIp("192.168.111.1");
        instance.setPort(8080);
        instance.setMetadata(Map.of("version", "1.0.0"));

        namingService.registerInstance(serviceName, instance);
        Thread.sleep(500);

        // Update metadata
        instance.setMetadata(Map.of("version", "2.0.0", "updated", "true"));
        namingService.registerInstance(serviceName, instance);
        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty());

        Instance retrieved = instances.get(0);
        assertEquals("2.0.0", retrieved.getMetadata().get("version"));
        assertEquals("true", retrieved.getMetadata().get("updated"));

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.111.1", 8080);
    }

    /**
     * NAN-015: Test instance with ephemeral flag
     */
    @Test
    @Order(15)
    void testInstanceEphemeralFlag() throws NacosException, InterruptedException {
        String serviceName = "ephemeral-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Ephemeral instance (default)
        Instance ephemeral = new Instance();
        ephemeral.setIp("192.168.112.1");
        ephemeral.setPort(8080);
        ephemeral.setEphemeral(true);

        namingService.registerInstance(serviceName, ephemeral);
        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty());
        assertTrue(instances.get(0).isEphemeral(), "Instance should be ephemeral");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.112.1", 8080);
    }

    /**
     * NAN-016: Test instance with weight
     */
    @Test
    @Order(16)
    void testInstanceWeight() throws NacosException, InterruptedException {
        String serviceName = "weight-svc-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.113.1");
        instance.setPort(8080);
        instance.setWeight(5.0);

        namingService.registerInstance(serviceName, instance);
        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty());
        assertEquals(5.0, instances.get(0).getWeight(), 0.01, "Weight should be 5.0");

        // Update weight
        instance.setWeight(10.0);
        namingService.registerInstance(serviceName, instance);
        Thread.sleep(500);

        instances = namingService.getAllInstances(serviceName);
        assertEquals(10.0, instances.get(0).getWeight(), 0.01, "Weight should be updated to 10.0");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.113.1", 8080);
    }
}
