package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.listener.Event;
import com.alibaba.nacos.api.naming.listener.EventListener;
import com.alibaba.nacos.api.naming.listener.NamingEvent;
import com.alibaba.nacos.api.naming.pojo.Instance;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Service Subscribe Tests
 *
 * Tests for service subscription and notification:
 * - Subscribe/unsubscribe operations
 * - Instance change notifications
 * - Multiple subscribers
 * - Cluster-based subscriptions
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosServiceSubscribeTest {

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
        System.out.println("Nacos Service Subscribe Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (namingService != null) {
            namingService.shutDown();
        }
    }

    // ==================== Basic Subscribe Tests ====================

    /**
     * NSS-001: Test basic service subscription
     */
    @Test
    @Order(1)
    void testBasicSubscribe() throws NacosException, InterruptedException {
        String serviceName = "subscribe-basic-" + UUID.randomUUID().toString().substring(0, 8);
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<List<Instance>> receivedInstances = new AtomicReference<>();

        // Subscribe first
        EventListener listener = event -> {
            if (event instanceof NamingEvent) {
                NamingEvent namingEvent = (NamingEvent) event;
                receivedInstances.set(namingEvent.getInstances());
                latch.countDown();
                System.out.println("Received " + namingEvent.getInstances().size() + " instances");
            }
        };

        namingService.subscribe(serviceName, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Register instance
        Instance instance = new Instance();
        instance.setIp("192.168.1.1");
        instance.setPort(8080);
        instance.setHealthy(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);

        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "Should receive subscription notification");
        assertNotNull(receivedInstances.get());

        // Cleanup
        namingService.unsubscribe(serviceName, DEFAULT_GROUP, listener);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NSS-002: Test unsubscribe from service
     */
    @Test
    @Order(2)
    void testUnsubscribe() throws NacosException, InterruptedException {
        String serviceName = "unsubscribe-test-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicInteger notificationCount = new AtomicInteger(0);

        EventListener listener = event -> {
            notificationCount.incrementAndGet();
        };

        // Subscribe
        namingService.subscribe(serviceName, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Unsubscribe
        namingService.unsubscribe(serviceName, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        int countBefore = notificationCount.get();

        // Register instance - should not trigger notification
        namingService.registerInstance(serviceName, "192.168.1.2", 8080);
        Thread.sleep(2000);

        int countAfter = notificationCount.get();
        assertEquals(countBefore, countAfter, "Should not receive notification after unsubscribe");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.1.2", 8080);
    }

    /**
     * NSS-003: Test subscribe to non-existent service
     */
    @Test
    @Order(3)
    void testSubscribeNonExistentService() throws NacosException, InterruptedException {
        String serviceName = "nonexistent-" + UUID.randomUUID().toString().substring(0, 8);
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<List<Instance>> receivedInstances = new AtomicReference<>();

        EventListener listener = event -> {
            if (event instanceof NamingEvent) {
                NamingEvent namingEvent = (NamingEvent) event;
                receivedInstances.set(namingEvent.getInstances());
                latch.countDown();
            }
        };

        // Subscribe to non-existent service
        namingService.subscribe(serviceName, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Now create the service
        namingService.registerInstance(serviceName, "192.168.1.3", 8080);

        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "Should receive notification when service is created");

        // Cleanup
        namingService.unsubscribe(serviceName, DEFAULT_GROUP, listener);
        namingService.deregisterInstance(serviceName, "192.168.1.3", 8080);
    }

    // ==================== Multiple Subscriber Tests ====================

    /**
     * NSS-004: Test multiple subscribers to same service
     */
    @Test
    @Order(4)
    void testMultipleSubscribers() throws NacosException, InterruptedException {
        String serviceName = "multi-sub-" + UUID.randomUUID().toString().substring(0, 8);
        int subscriberCount = 3;
        CountDownLatch latch = new CountDownLatch(subscriberCount);
        List<EventListener> listeners = new ArrayList<>();

        for (int i = 0; i < subscriberCount; i++) {
            final int index = i;
            EventListener listener = event -> {
                System.out.println("Subscriber " + index + " received event");
                latch.countDown();
            };
            listeners.add(listener);
            namingService.subscribe(serviceName, DEFAULT_GROUP, listener);
        }

        Thread.sleep(500);

        // Register instance
        namingService.registerInstance(serviceName, "192.168.1.4", 8080);

        boolean allReceived = latch.await(15, TimeUnit.SECONDS);
        assertTrue(allReceived, "All subscribers should receive notification");

        // Cleanup
        for (EventListener listener : listeners) {
            namingService.unsubscribe(serviceName, DEFAULT_GROUP, listener);
        }
        namingService.deregisterInstance(serviceName, "192.168.1.4", 8080);
    }

    /**
     * NSS-005: Test subscriber receives instance changes
     */
    @Test
    @Order(5)
    void testSubscriberReceivesChanges() throws NacosException, InterruptedException {
        String serviceName = "change-test-" + UUID.randomUUID().toString().substring(0, 8);
        List<Integer> instanceCounts = new CopyOnWriteArrayList<>();
        CountDownLatch latch = new CountDownLatch(3); // Expect 3 notifications

        EventListener listener = event -> {
            if (event instanceof NamingEvent) {
                NamingEvent namingEvent = (NamingEvent) event;
                instanceCounts.add(namingEvent.getInstances().size());
                latch.countDown();
            }
        };

        namingService.subscribe(serviceName, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Add first instance
        namingService.registerInstance(serviceName, "192.168.1.10", 8080);
        Thread.sleep(1500);

        // Add second instance
        namingService.registerInstance(serviceName, "192.168.1.11", 8080);
        Thread.sleep(1500);

        // Remove first instance
        namingService.deregisterInstance(serviceName, "192.168.1.10", 8080);
        Thread.sleep(1500);

        latch.await(15, TimeUnit.SECONDS);

        System.out.println("Instance count changes: " + instanceCounts);

        // Cleanup
        namingService.unsubscribe(serviceName, DEFAULT_GROUP, listener);
        namingService.deregisterInstance(serviceName, "192.168.1.11", 8080);
    }

    // ==================== Cluster Subscribe Tests ====================

    /**
     * NSS-006: Test subscribe with cluster filter
     */
    @Test
    @Order(6)
    void testSubscribeWithCluster() throws NacosException, InterruptedException {
        String serviceName = "cluster-sub-" + UUID.randomUUID().toString().substring(0, 8);
        String targetCluster = "cluster-a";
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<List<Instance>> receivedInstances = new AtomicReference<>();

        EventListener listener = event -> {
            if (event instanceof NamingEvent) {
                NamingEvent namingEvent = (NamingEvent) event;
                receivedInstances.set(namingEvent.getInstances());
                latch.countDown();
            }
        };

        // Subscribe to specific cluster
        namingService.subscribe(serviceName, DEFAULT_GROUP, Arrays.asList(targetCluster), listener);
        Thread.sleep(500);

        // Register in target cluster
        Instance instanceA = new Instance();
        instanceA.setIp("192.168.1.20");
        instanceA.setPort(8080);
        instanceA.setClusterName(targetCluster);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instanceA);

        // Register in different cluster (should not be in notification)
        Instance instanceB = new Instance();
        instanceB.setIp("192.168.1.21");
        instanceB.setPort(8080);
        instanceB.setClusterName("cluster-b");
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instanceB);

        boolean received = latch.await(10, TimeUnit.SECONDS);
        if (received && receivedInstances.get() != null) {
            System.out.println("Received instances: " + receivedInstances.get().size());
            for (Instance inst : receivedInstances.get()) {
                System.out.println("  " + inst.getIp() + " in cluster " + inst.getClusterName());
            }
        }

        // Cleanup
        namingService.unsubscribe(serviceName, DEFAULT_GROUP, Arrays.asList(targetCluster), listener);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instanceA);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instanceB);
    }

    /**
     * NSS-007: Test subscribe to multiple clusters
     */
    @Test
    @Order(7)
    void testSubscribeMultipleClusters() throws NacosException, InterruptedException {
        String serviceName = "multi-cluster-sub-" + UUID.randomUUID().toString().substring(0, 8);
        List<String> clusters = Arrays.asList("cluster-x", "cluster-y");
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<List<Instance>> receivedInstances = new AtomicReference<>();

        EventListener listener = event -> {
            if (event instanceof NamingEvent) {
                NamingEvent namingEvent = (NamingEvent) event;
                receivedInstances.set(namingEvent.getInstances());
                latch.countDown();
            }
        };

        namingService.subscribe(serviceName, DEFAULT_GROUP, clusters, listener);
        Thread.sleep(500);

        // Register in both clusters
        for (String cluster : clusters) {
            Instance instance = new Instance();
            instance.setIp("192.168.2." + cluster.hashCode() % 256);
            instance.setPort(8080);
            instance.setClusterName(cluster);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        boolean received = latch.await(10, TimeUnit.SECONDS);
        if (received && receivedInstances.get() != null) {
            System.out.println("Multi-cluster instances: " + receivedInstances.get().size());
        }

        // Cleanup
        namingService.unsubscribe(serviceName, DEFAULT_GROUP, clusters, listener);
        for (String cluster : clusters) {
            namingService.deregisterInstance(serviceName, "192.168.2." + cluster.hashCode() % 256, 8080, cluster);
        }
    }

    // ==================== Concurrent Subscribe Tests ====================

    /**
     * NSS-008: Test concurrent subscribe operations
     */
    @Test
    @Order(8)
    void testConcurrentSubscribe() throws InterruptedException {
        String serviceName = "concurrent-sub-" + UUID.randomUUID().toString().substring(0, 8);
        int threadCount = 10;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(threadCount);
        List<EventListener> listeners = new CopyOnWriteArrayList<>();

        for (int i = 0; i < threadCount; i++) {
            final int index = i;
            new Thread(() -> {
                try {
                    startLatch.await();
                    EventListener listener = event -> {
                        System.out.println("Concurrent subscriber " + index + " received event");
                    };
                    listeners.add(listener);
                    namingService.subscribe(serviceName, DEFAULT_GROUP, listener);
                } catch (Exception e) {
                    System.err.println("Concurrent subscribe error: " + e.getMessage());
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        startLatch.countDown();
        boolean completed = doneLatch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All concurrent subscribes should complete");

        System.out.println("Concurrent subscribers: " + listeners.size());

        // Cleanup
        for (EventListener listener : listeners) {
            try {
                namingService.unsubscribe(serviceName, DEFAULT_GROUP, listener);
            } catch (Exception e) {
                // Ignore cleanup errors
            }
        }
    }

    /**
     * NSS-009: Test rapid subscribe/unsubscribe
     */
    @Test
    @Order(9)
    void testRapidSubscribeUnsubscribe() throws NacosException, InterruptedException {
        String serviceName = "rapid-sub-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicInteger subscribeCount = new AtomicInteger(0);

        // Register instance first
        namingService.registerInstance(serviceName, "192.168.3.1", 8080);
        Thread.sleep(500);

        // Rapid subscribe/unsubscribe cycles
        for (int i = 0; i < 5; i++) {
            EventListener listener = event -> {
                subscribeCount.incrementAndGet();
            };

            namingService.subscribe(serviceName, DEFAULT_GROUP, listener);
            Thread.sleep(200);
            namingService.unsubscribe(serviceName, DEFAULT_GROUP, listener);
            Thread.sleep(200);
        }

        System.out.println("Rapid subscribe/unsubscribe - notifications received: " + subscribeCount.get());

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.3.1", 8080);
    }

    // ==================== Edge Cases ====================

    /**
     * NSS-010: Test duplicate subscribe
     */
    @Test
    @Order(10)
    void testDuplicateSubscribe() throws NacosException, InterruptedException {
        String serviceName = "duplicate-sub-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicInteger notificationCount = new AtomicInteger(0);
        CountDownLatch latch = new CountDownLatch(1);

        EventListener listener = event -> {
            notificationCount.incrementAndGet();
            latch.countDown();
        };

        // Subscribe multiple times with same listener
        namingService.subscribe(serviceName, DEFAULT_GROUP, listener);
        namingService.subscribe(serviceName, DEFAULT_GROUP, listener);
        namingService.subscribe(serviceName, DEFAULT_GROUP, listener);

        Thread.sleep(500);

        // Register instance
        namingService.registerInstance(serviceName, "192.168.3.2", 8080);

        latch.await(10, TimeUnit.SECONDS);
        Thread.sleep(1000);

        System.out.println("Duplicate subscribe notification count: " + notificationCount.get());

        // Cleanup
        namingService.unsubscribe(serviceName, DEFAULT_GROUP, listener);
        namingService.deregisterInstance(serviceName, "192.168.3.2", 8080);
    }

    /**
     * NSS-011: Test subscribe with empty cluster list
     */
    @Test
    @Order(11)
    void testSubscribeEmptyClusterList() throws NacosException, InterruptedException {
        String serviceName = "empty-cluster-" + UUID.randomUUID().toString().substring(0, 8);
        CountDownLatch latch = new CountDownLatch(1);

        EventListener listener = event -> {
            latch.countDown();
        };

        // Subscribe with empty cluster list (should get all clusters)
        namingService.subscribe(serviceName, DEFAULT_GROUP, Collections.emptyList(), listener);
        Thread.sleep(500);

        // Register instance
        namingService.registerInstance(serviceName, "192.168.3.3", 8080);

        boolean received = latch.await(10, TimeUnit.SECONDS);
        System.out.println("Empty cluster list subscription received: " + received);

        // Cleanup
        namingService.unsubscribe(serviceName, DEFAULT_GROUP, Collections.emptyList(), listener);
        namingService.deregisterInstance(serviceName, "192.168.3.3", 8080);
    }

    /**
     * NSS-012: Test subscribe listener exception handling
     */
    @Test
    @Order(12)
    void testSubscribeListenerException() throws NacosException, InterruptedException {
        String serviceName = "listener-error-" + UUID.randomUUID().toString().substring(0, 8);
        CountDownLatch latch = new CountDownLatch(2);
        AtomicInteger successCount = new AtomicInteger(0);

        // Listener that throws exception
        EventListener badListener = event -> {
            latch.countDown();
            throw new RuntimeException("Simulated listener error");
        };

        // Listener that should still work
        EventListener goodListener = event -> {
            successCount.incrementAndGet();
            latch.countDown();
        };

        namingService.subscribe(serviceName, DEFAULT_GROUP, badListener);
        namingService.subscribe(serviceName, DEFAULT_GROUP, goodListener);
        Thread.sleep(500);

        // Register instance
        namingService.registerInstance(serviceName, "192.168.3.4", 8080);

        boolean bothCalled = latch.await(10, TimeUnit.SECONDS);
        System.out.println("Both listeners called: " + bothCalled + ", good listener count: " + successCount.get());

        // Cleanup
        namingService.unsubscribe(serviceName, DEFAULT_GROUP, badListener);
        namingService.unsubscribe(serviceName, DEFAULT_GROUP, goodListener);
        namingService.deregisterInstance(serviceName, "192.168.3.4", 8080);
    }
}
