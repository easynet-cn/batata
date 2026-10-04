package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.listener.Event;
import com.alibaba.nacos.api.naming.listener.EventListener;
import com.alibaba.nacos.api.naming.listener.NamingEvent;
import com.alibaba.nacos.api.naming.pojo.Instance;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Naming Service SDK Compatibility Tests
 *
 * Tests Batata's compatibility with official Nacos Java SDK for service discovery.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosNamingServiceTest {

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
        assertNotNull(namingService, "NamingService should be created successfully");
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (namingService != null) {
            namingService.shutDown();
        }
    }

    // ==================== Shared polling helpers (replace fixed sleeps) ====================

    /**
     * Poll until the service reports exactly {@code expected} instances, or 10s elapse.
     * Reduces flakiness compared to a fixed Thread.sleep on slow CI machines.
     */
    private static List<Instance> waitForInstances(String serviceName, int expected) throws NacosException {
        long deadline = System.currentTimeMillis() + 10_000;
        List<Instance> instances = namingService.getAllInstances(serviceName);
        while (instances.size() != expected && System.currentTimeMillis() < deadline) {
            try {
                Thread.sleep(200);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                break;
            }
            instances = namingService.getAllInstances(serviceName);
        }
        return instances;
    }

    /**
     * Poll until at least {@code expectedMin} services are listed, or 10s elapse.
     */
    private static boolean waitForServiceCount(int expectedMin) throws NacosException {
        long deadline = System.currentTimeMillis() + 10_000;
        long count = namingService.getServicesOfServer(1, 100).getCount();
        while (count < expectedMin && System.currentTimeMillis() < deadline) {
            try {
                Thread.sleep(200);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                break;
            }
            count = namingService.getServicesOfServer(1, 100).getCount();
        }
        return count >= expectedMin;
    }

    // ==================== P0: Critical Tests ====================

    /**
     * NN-001: Test register instance
     */
    @Test
    @Order(1)
    void testRegisterInstance() throws NacosException, InterruptedException {
        String serviceName = "nn001-register-" + UUID.randomUUID();
        String ip = "192.168.1.100";
        int port = 8080;

        // Register
        namingService.registerInstance(serviceName, ip, port);

        // Wait for registration to propagate (poll up to 10s)
        List<Instance> instances = waitForInstances(serviceName, 1);
        assertFalse(instances.isEmpty(), "Should have at least one instance");

        Instance instance = instances.get(0);
        assertEquals(ip, instance.getIp());
        assertEquals(port, instance.getPort());

        // Cleanup
        namingService.deregisterInstance(serviceName, ip, port);
    }

    /**
     * NN-002: Test deregister instance
     */
    @Test
    @Order(2)
    void testDeregisterInstance() throws NacosException, InterruptedException {
        String serviceName = "nn002-deregister-" + UUID.randomUUID();
        String ip = "192.168.1.101";
        int port = 8081;

        // Register
        namingService.registerInstance(serviceName, ip, port);

        // Verify registered
        List<Instance> beforeDeregister = waitForInstances(serviceName, 1);
        assertFalse(beforeDeregister.isEmpty());

        // Deregister
        namingService.deregisterInstance(serviceName, ip, port);

        // Verify deregistered (poll until empty)
        List<Instance> afterDeregister = waitForInstances(serviceName, 0);
        assertTrue(afterDeregister.isEmpty(), "Should have no instances after deregister");
    }

    /**
     * NN-003: Test get all instances
     */
    @Test
    @Order(3)
    void testGetAllInstances() throws NacosException, InterruptedException {
        String serviceName = "nn003-getall-" + UUID.randomUUID();

        // Register multiple instances
        namingService.registerInstance(serviceName, "192.168.1.1", 8080);
        namingService.registerInstance(serviceName, "192.168.1.2", 8080);
        namingService.registerInstance(serviceName, "192.168.1.3", 8080);

        // Get all (poll up to 10s)
        List<Instance> instances = waitForInstances(serviceName, 3);
        assertEquals(3, instances.size(), "Should have 3 instances");

        Set<String> ips = new HashSet<>();
        for (Instance inst : instances) {
            ips.add(inst.getIp());
        }
        assertTrue(ips.contains("192.168.1.1"));
        assertTrue(ips.contains("192.168.1.2"));
        assertTrue(ips.contains("192.168.1.3"));

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.1.1", 8080);
        namingService.deregisterInstance(serviceName, "192.168.1.2", 8080);
        namingService.deregisterInstance(serviceName, "192.168.1.3", 8080);
    }

    /**
     * NN-004: Test select healthy instances
     */
    @Test
    @Order(4)
    void testSelectInstances() throws NacosException, InterruptedException {
        String serviceName = "nn004-select-" + UUID.randomUUID();

        // Register healthy instance
        Instance healthyInstance = new Instance();
        healthyInstance.setIp("192.168.1.10");
        healthyInstance.setPort(8080);
        healthyInstance.setHealthy(true);
        healthyInstance.setWeight(1.0);
        namingService.registerInstance(serviceName, healthyInstance);

        // Wait for registration (poll up to 10s)
        waitForInstances(serviceName, 1);

        // Select healthy only
        List<Instance> healthyInstances = namingService.selectInstances(serviceName, true);
        assertFalse(healthyInstances.isEmpty(), "Should have healthy instances");

        for (Instance inst : healthyInstances) {
            assertTrue(inst.isHealthy(), "All instances should be healthy");
        }

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.1.10", 8080);
    }

    /**
     * NN-005: Test select one healthy instance (load balancing)
     */
    @Test
    @Order(5)
    void testSelectOneHealthyInstance() throws NacosException, InterruptedException {
        String serviceName = "nn005-selectone-" + UUID.randomUUID();

        // Register multiple instances
        namingService.registerInstance(serviceName, "192.168.1.20", 8080);
        namingService.registerInstance(serviceName, "192.168.1.21", 8080);

        // Wait for both instances to be registered (poll up to 10s)
        waitForInstances(serviceName, 2);

        // Select one multiple times - should load balance across both instances
        Set<String> selectedIps = new HashSet<>();
        for (int i = 0; i < 100; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName);
            assertNotNull(selected);
            selectedIps.add(selected.getIp());
        }

        // Both instances should be selected at least once (verifies real load balancing)
        assertTrue(selectedIps.contains("192.168.1.20"),
                "Instance 192.168.1.20 should be selected at least once, got: " + selectedIps);
        assertTrue(selectedIps.contains("192.168.1.21"),
                "Instance 192.168.1.21 should be selected at least once, got: " + selectedIps);

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.1.20", 8080);
        namingService.deregisterInstance(serviceName, "192.168.1.21", 8080);
    }

    /**
     * NN-006: Test subscribe to service
     */
    @Test
    @Order(6)
    void testSubscribe() throws NacosException, InterruptedException {
        String serviceName = "nn006-subscribe-" + UUID.randomUUID();
        AtomicReference<List<Instance>> receivedInstances = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        // Subscribe
        EventListener listener = event -> {
            if (event instanceof NamingEvent) {
                NamingEvent namingEvent = (NamingEvent) event;
                System.out.println("Received naming event: " + namingEvent.getInstances().size() + " instances");
                receivedInstances.set(namingEvent.getInstances());
                latch.countDown();
            }
        };
        namingService.subscribe(serviceName, listener);
        TestSupport.waitForSubscribed(namingService, serviceName, DEFAULT_GROUP);

        // Register triggers notification
        namingService.registerInstance(serviceName, "192.168.1.30", 8080);

        // Wait for notification
        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "Should receive service change notification");
        assertNotNull(receivedInstances.get());
        assertFalse(receivedInstances.get().isEmpty());

        // Cleanup
        namingService.unsubscribe(serviceName, listener);
        namingService.deregisterInstance(serviceName, "192.168.1.30", 8080);
    }

    // ==================== P1: Important Tests ====================

    /**
     * NN-007: Test unsubscribe
     */
    @Test
    @Order(7)
    void testUnsubscribe() throws NacosException, InterruptedException {
        String serviceName = "nn007-unsubscribe-" + UUID.randomUUID();
        CountDownLatch latch = new CountDownLatch(1);

        EventListener listener = event -> {
            latch.countDown();
        };

        // Subscribe then unsubscribe
        namingService.subscribe(serviceName, listener);
        TestSupport.waitForSubscribed(namingService, serviceName, DEFAULT_GROUP);
        namingService.unsubscribe(serviceName, listener);
        TestSupport.waitForUnsubscribed(namingService, serviceName, DEFAULT_GROUP);

        // Register should NOT trigger notification
        namingService.registerInstance(serviceName, "192.168.1.40", 8080);

        boolean received = latch.await(3, TimeUnit.SECONDS);
        assertFalse(received, "Should NOT receive notification after unsubscribe");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.1.40", 8080);
    }

    /**
     * NN-008: Test instance with metadata
     */
    @Test
    @Order(8)
    void testInstanceWithMetadata() throws NacosException, InterruptedException {
        String serviceName = "nn008-metadata-" + UUID.randomUUID();

        Instance instance = new Instance();
        instance.setIp("192.168.1.50");
        instance.setPort(8080);
        instance.setWeight(1.0);
        instance.setHealthy(true);

        Map<String, String> metadata = new HashMap<>();
        metadata.put("version", "1.0.0");
        metadata.put("env", "test");
        metadata.put("region", "us-west-2");
        instance.setMetadata(metadata);

        // Register with metadata
        namingService.registerInstance(serviceName, instance);

        // Get and verify metadata (poll up to 10s)
        List<Instance> instances = waitForInstances(serviceName, 1);
        assertFalse(instances.isEmpty());

        Instance retrieved = instances.get(0);
        assertEquals("1.0.0", retrieved.getMetadata().get("version"));
        assertEquals("test", retrieved.getMetadata().get("env"));
        assertEquals("us-west-2", retrieved.getMetadata().get("region"));

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.1.50", 8080);
    }

    /**
     * NN-009: Test instance with weight
     */
    @Test
    @Order(9)
    void testInstanceWithWeight() throws NacosException, InterruptedException {
        String serviceName = "nn009-weight-" + UUID.randomUUID();

        // Register with different weights
        Instance light = new Instance();
        light.setIp("192.168.1.60");
        light.setPort(8080);
        light.setWeight(0.1);
        namingService.registerInstance(serviceName, light);

        Instance heavy = new Instance();
        heavy.setIp("192.168.1.61");
        heavy.setPort(8080);
        heavy.setWeight(0.9);
        namingService.registerInstance(serviceName, heavy);

        // Wait for both instances (poll up to 10s)
        waitForInstances(serviceName, 2);

        // Select many times - heavy should be selected more often
        int lightCount = 0;
        int heavyCount = 0;
        for (int i = 0; i < 1000; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName);
            if (selected.getIp().equals("192.168.1.60")) {
                lightCount++;
            } else {
                heavyCount++;
            }
        }

        System.out.println("Light (0.1): " + lightCount + ", Heavy (0.9): " + heavyCount);
        assertTrue(heavyCount > lightCount, "Heavy weight instance should be selected more often");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.1.60", 8080);
        namingService.deregisterInstance(serviceName, "192.168.1.61", 8080);
    }

    // ==================== P2: Nice to Have Tests ====================

    /**
     * NN-010: Test ephemeral vs persistent instance
     */
    @Test
    @Order(10)
    void testEphemeralInstance() throws NacosException, InterruptedException {
        String serviceName = "nn010-ephemeral-" + UUID.randomUUID();

        Instance ephemeral = new Instance();
        ephemeral.setIp("192.168.1.70");
        ephemeral.setPort(8080);
        ephemeral.setEphemeral(true);
        namingService.registerInstance(serviceName, ephemeral);

        List<Instance> instances = waitForInstances(serviceName, 1);
        assertFalse(instances.isEmpty());
        assertTrue(instances.get(0).isEphemeral(), "Instance should be ephemeral");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.1.70", 8080);
    }

    /**
     * NN-011: Test multiple clusters
     */
    @Test
    @Order(11)
    void testMultipleClusters() throws NacosException, InterruptedException {
        String serviceName = "nn011-cluster-" + UUID.randomUUID();

        // Register in different clusters
        Instance cluster1 = new Instance();
        cluster1.setIp("192.168.1.80");
        cluster1.setPort(8080);
        cluster1.setClusterName("cluster-a");
        namingService.registerInstance(serviceName, cluster1);

        Instance cluster2 = new Instance();
        cluster2.setIp("192.168.1.81");
        cluster2.setPort(8080);
        cluster2.setClusterName("cluster-b");
        namingService.registerInstance(serviceName, cluster2);

        // Verify all instances are registered first (poll up to 10s)
        List<Instance> allInstances = waitForInstances(serviceName, 2);
        assertEquals(2, allInstances.size(), "Should have 2 total instances");

        // Get from specific cluster using direct query (subscribe=false)
        List<Instance> clusterA = namingService.getAllInstances(serviceName,
                DEFAULT_GROUP, Arrays.asList("cluster-a"), false);
        assertEquals(1, clusterA.size(), "Should have 1 instance in cluster-a");
        assertEquals("192.168.1.80", clusterA.get(0).getIp());

        List<Instance> clusterB = namingService.getAllInstances(serviceName,
                DEFAULT_GROUP, Arrays.asList("cluster-b"), false);
        assertEquals(1, clusterB.size(), "Should have 1 instance in cluster-b");
        assertEquals("192.168.1.81", clusterB.get(0).getIp());

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.1.80", 8080);
        namingService.deregisterInstance(serviceName, "192.168.1.81", 8080);
    }

    /**
     * NN-012: Test service list pagination
     */
    @Test
    @Order(12)
    void testServiceListPagination() throws NacosException, InterruptedException {
        String prefix = "nn012-list-" + UUID.randomUUID().toString().substring(0, 8);

        // Register multiple services
        for (int i = 0; i < 5; i++) {
            String serviceName = prefix + "-service-" + i;
            namingService.registerInstance(serviceName, "192.168.1." + (90 + i), 8080);
        }

        // List services with pagination (poll until at least 5 are listed)
        assertTrue(waitForServiceCount(5), "Should have at least 5 services");
        com.alibaba.nacos.api.naming.pojo.ListView<String> listView =
                namingService.getServicesOfServer(1, 10);

        assertNotNull(listView);
        assertTrue(listView.getCount() >= 5, "Should have at least 5 services");

        // Cleanup
        for (int i = 0; i < 5; i++) {
            String serviceName = prefix + "-service-" + i;
            namingService.deregisterInstance(serviceName, "192.168.1." + (90 + i), 8080);
        }
    }
}
