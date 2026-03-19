package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.listener.FuzzyWatchChangeEvent;
import com.alibaba.nacos.api.naming.listener.FuzzyWatchEventWatcher;
import com.alibaba.nacos.api.naming.pojo.ListView;
import org.junit.jupiter.api.*;
import org.junit.jupiter.api.Disabled;

import java.util.Properties;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos 3.x Naming FuzzySubscribe (FuzzyWatch) SDK Compatibility Tests
 *
 * Tests Batata's compatibility with Nacos 3.x FuzzyWatch feature for naming/service discovery.
 * FuzzyWatch allows clients to watch service changes matching a pattern (serviceName and/or group).
 * When a matching service is registered or deregistered, the watcher callback fires.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosNamingFuzzySubscribeTest {

    private static NamingService namingService;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final long WATCH_TIMEOUT_SECONDS = 15;

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

    // ==================== P0: Critical Tests ====================

    /**
     * NFS-001: FuzzyWatch with service name pattern - register service matching pattern, verify callback
     *
     * Uses fuzzyWatch(serviceNamePattern, groupNamePattern, watcher) to watch for services
     * matching a name pattern. Registering an instance for a matching service should trigger
     * the ADD_SERVICE event in the watcher.
     */
    @Test
    @Order(1)
    @Disabled("Naming FuzzyWatch push notifications not yet fully implemented")
    void testFuzzyWatchWithServicePattern() throws NacosException, InterruptedException {
        String uniquePrefix = "nfs001-" + UUID.randomUUID().toString().substring(0, 8);
        String servicePattern = uniquePrefix + "*";
        String serviceName = uniquePrefix + "-myservice";

        AtomicReference<FuzzyWatchChangeEvent> receivedEvent = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                System.out.println("Naming FuzzyWatch event: " + event);
                if (event.getServiceName() != null && event.getServiceName().startsWith(uniquePrefix)) {
                    receivedEvent.set(event);
                    latch.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Start fuzzy watching with service name pattern
        namingService.fuzzyWatch(servicePattern, DEFAULT_GROUP, watcher);

        // Wait for watch registration to propagate
        Thread.sleep(2000);

        // Register an instance for a service matching the pattern
        namingService.registerInstance(serviceName, "192.168.100.1", 8080);

        // Wait for the fuzzy watch callback
        boolean received = latch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(received, "Should receive fuzzy watch notification for matching service");

        FuzzyWatchChangeEvent event = receivedEvent.get();
        assertNotNull(event, "Event should not be null");
        assertEquals(serviceName, event.getServiceName(), "Event service name should match registered service");
        assertEquals(DEFAULT_GROUP, event.getGroupName(), "Event group should match");
        assertEquals("ADD_SERVICE", event.getChangeType(), "Change type should be ADD_SERVICE");

        // Cleanup
        namingService.cancelFuzzyWatch(servicePattern, DEFAULT_GROUP, watcher);
        namingService.deregisterInstance(serviceName, "192.168.100.1", 8080);
    }

    /**
     * NFS-002: FuzzyWatch with group pattern - watch all services in matching groups
     *
     * Uses fuzzyWatch(groupNamePattern, watcher) with a group pattern.
     * Registering a service in a matching group should trigger the watcher.
     */
    @Test
    @Order(2)
    @Disabled("Naming FuzzyWatch push notifications not yet fully implemented")
    void testFuzzyWatchWithGroupPattern() throws NacosException, InterruptedException {
        String uniqueSuffix = UUID.randomUUID().toString().substring(0, 8);
        String groupPattern = "NFSGROUP-" + uniqueSuffix + "*";
        String group = "NFSGROUP-" + uniqueSuffix + "-test";
        String serviceName = "nfs002-groupwatch-" + uniqueSuffix;

        AtomicReference<FuzzyWatchChangeEvent> receivedEvent = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                System.out.println("Group FuzzyWatch event: " + event);
                if (event.getServiceName() != null && event.getServiceName().equals(serviceName)) {
                    receivedEvent.set(event);
                    latch.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Watch using group pattern
        namingService.fuzzyWatch(groupPattern, watcher);
        Thread.sleep(2000);

        // Register service in matching group
        namingService.registerInstance(serviceName, group, "192.168.100.2", 8080);

        boolean received = latch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(received, "Should receive notification for service in matching group");

        FuzzyWatchChangeEvent event = receivedEvent.get();
        assertNotNull(event, "Event should not be null");
        assertEquals(serviceName, event.getServiceName(), "Service name should match");
        assertEquals(group, event.getGroupName(), "Group should match");

        // Cleanup
        namingService.cancelFuzzyWatch(groupPattern, watcher);
        namingService.deregisterInstance(serviceName, group, "192.168.100.2", 8080);
    }

    /**
     * NFS-003: FuzzyWatch cancel - verify callbacks stop after cancel
     *
     * After calling cancelFuzzyWatch, registering new matching services should NOT trigger the watcher.
     */
    @Test
    @Order(3)
    @Disabled("Naming FuzzyWatch push notifications not yet fully implemented")
    void testFuzzyWatchCancel() throws NacosException, InterruptedException {
        String uniquePrefix = "nfs003-" + UUID.randomUUID().toString().substring(0, 8);
        String servicePattern = uniquePrefix + "*";
        String service1 = uniquePrefix + "-before";
        String service2 = uniquePrefix + "-after";

        AtomicInteger eventCount = new AtomicInteger(0);
        CountDownLatch firstLatch = new CountDownLatch(1);
        CountDownLatch secondLatch = new CountDownLatch(1);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                System.out.println("Cancel test event: " + event);
                if (event.getServiceName() != null && event.getServiceName().startsWith(uniquePrefix)) {
                    int count = eventCount.incrementAndGet();
                    if (count == 1) {
                        firstLatch.countDown();
                    } else {
                        secondLatch.countDown();
                    }
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Start watching
        namingService.fuzzyWatch(servicePattern, DEFAULT_GROUP, watcher);
        Thread.sleep(2000);

        // Register first service - should trigger
        namingService.registerInstance(service1, "192.168.100.3", 8080);
        boolean firstReceived = firstLatch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(firstReceived, "Should receive first event before cancel");

        // Cancel fuzzy watch
        namingService.cancelFuzzyWatch(servicePattern, DEFAULT_GROUP, watcher);
        Thread.sleep(2000);

        // Register second service - should NOT trigger
        namingService.registerInstance(service2, "192.168.100.4", 8080);
        boolean secondReceived = secondLatch.await(5, TimeUnit.SECONDS);
        assertFalse(secondReceived, "Should NOT receive event after cancelFuzzyWatch");
        assertEquals(1, eventCount.get(), "Should have received exactly one event");

        // Cleanup
        namingService.deregisterInstance(service1, "192.168.100.3", 8080);
        namingService.deregisterInstance(service2, "192.168.100.4", 8080);
    }

    /**
     * NFS-004: Non-matching services should NOT trigger fuzzy watch
     *
     * Register a fuzzy watch for a specific pattern, then register a service with a
     * name that does NOT match. The watcher should not be triggered.
     */
    @Test
    @Order(4)
    @Disabled("Naming FuzzyWatch push notifications not yet fully implemented")
    void testFuzzyWatchNonMatchingService() throws NacosException, InterruptedException {
        String uniqueId = UUID.randomUUID().toString().substring(0, 8);
        String watchPattern = "nfs004-match-" + uniqueId + "*";
        String nonMatchingService = "nfs004-nomatch-" + uniqueId + "-service";

        CountDownLatch latch = new CountDownLatch(1);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                System.out.println("Non-match test event: " + event);
                if (event.getServiceName() != null && event.getServiceName().equals(nonMatchingService)) {
                    latch.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        namingService.fuzzyWatch(watchPattern, DEFAULT_GROUP, watcher);
        Thread.sleep(2000);

        // Register a service that does NOT match the pattern
        namingService.registerInstance(nonMatchingService, "192.168.100.5", 8080);

        boolean received = latch.await(5, TimeUnit.SECONDS);
        assertFalse(received, "Should NOT receive event for non-matching service name pattern");

        // Cleanup
        namingService.cancelFuzzyWatch(watchPattern, DEFAULT_GROUP, watcher);
        namingService.deregisterInstance(nonMatchingService, "192.168.100.5", 8080);
    }

    /**
     * NFS-005: FuzzyWatch DELETE_SERVICE event - verify deregistration triggers watcher
     *
     * Register a service, set up fuzzy watch, then deregister the service.
     * The watcher should receive a DELETE_SERVICE change event.
     */
    @Test
    @Order(5)
    @Disabled("Naming FuzzyWatch push notifications not yet fully implemented")
    void testFuzzyWatchDeleteServiceEvent() throws NacosException, InterruptedException {
        String uniquePrefix = "nfs005-" + UUID.randomUUID().toString().substring(0, 8);
        String servicePattern = uniquePrefix + "*";
        String serviceName = uniquePrefix + "-deleteme";

        // Pre-register a service
        namingService.registerInstance(serviceName, "192.168.100.6", 8080);
        Thread.sleep(1000);

        CountDownLatch addLatch = new CountDownLatch(1);
        CountDownLatch deleteLatch = new CountDownLatch(1);
        AtomicReference<String> deleteChangeType = new AtomicReference<>();

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                System.out.println("Delete test event: " + event);
                if (event.getServiceName() != null && event.getServiceName().equals(serviceName)) {
                    if ("ADD_SERVICE".equals(event.getChangeType())) {
                        addLatch.countDown();
                    } else if ("DELETE_SERVICE".equals(event.getChangeType())) {
                        deleteChangeType.set(event.getChangeType());
                        deleteLatch.countDown();
                    }
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Start watching - should get initial ADD event for existing service
        namingService.fuzzyWatch(servicePattern, DEFAULT_GROUP, watcher);
        boolean addReceived = addLatch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(addReceived, "Should receive ADD_SERVICE event for existing service on watch init");

        // Now deregister the service
        namingService.deregisterInstance(serviceName, "192.168.100.6", 8080);

        boolean deleteReceived = deleteLatch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(deleteReceived, "Should receive DELETE_SERVICE event after deregistering service");
        assertEquals("DELETE_SERVICE", deleteChangeType.get(), "Change type should be DELETE_SERVICE");

        // Cleanup
        namingService.cancelFuzzyWatch(servicePattern, DEFAULT_GROUP, watcher);
    }

    /**
     * NFS-006: FuzzyWatch with multiple independent patterns
     *
     * Register two different fuzzy watch patterns. Registering a service matching only one
     * pattern should trigger only that watcher.
     */
    @Test
    @Order(6)
    @Disabled("Naming FuzzyWatch push notifications not yet fully implemented")
    void testFuzzyWatchMultiplePatterns() throws NacosException, InterruptedException {
        String uniqueId = UUID.randomUUID().toString().substring(0, 8);
        String patternA = "nfs006a-" + uniqueId + "*";
        String patternB = "nfs006b-" + uniqueId + "*";
        String serviceA = "nfs006a-" + uniqueId + "-svc";
        String serviceB = "nfs006b-" + uniqueId + "-svc";

        CountDownLatch latchA = new CountDownLatch(1);
        CountDownLatch latchB = new CountDownLatch(1);
        AtomicReference<String> watcherAService = new AtomicReference<>();
        AtomicReference<String> watcherBService = new AtomicReference<>();

        FuzzyWatchEventWatcher watcherA = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                System.out.println("WatcherA event: " + event);
                if (event.getServiceName() != null && event.getServiceName().startsWith("nfs006a-" + uniqueId)) {
                    watcherAService.set(event.getServiceName());
                    latchA.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        FuzzyWatchEventWatcher watcherB = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                System.out.println("WatcherB event: " + event);
                if (event.getServiceName() != null && event.getServiceName().startsWith("nfs006b-" + uniqueId)) {
                    watcherBService.set(event.getServiceName());
                    latchB.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Register both watchers
        namingService.fuzzyWatch(patternA, DEFAULT_GROUP, watcherA);
        namingService.fuzzyWatch(patternB, DEFAULT_GROUP, watcherB);
        Thread.sleep(2000);

        // Register service matching pattern A only
        namingService.registerInstance(serviceA, "192.168.100.7", 8080);
        boolean receivedA = latchA.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(receivedA, "Watcher A should receive event for matching service");
        assertEquals(serviceA, watcherAService.get());

        // Register service matching pattern B only
        namingService.registerInstance(serviceB, "192.168.100.8", 8080);
        boolean receivedB = latchB.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(receivedB, "Watcher B should receive event for matching service");
        assertEquals(serviceB, watcherBService.get());

        // Cleanup
        namingService.cancelFuzzyWatch(patternA, DEFAULT_GROUP, watcherA);
        namingService.cancelFuzzyWatch(patternB, DEFAULT_GROUP, watcherB);
        namingService.deregisterInstance(serviceA, "192.168.100.7", 8080);
        namingService.deregisterInstance(serviceB, "192.168.100.8", 8080);
    }

    /**
     * NFS-007: FuzzyWatchWithServiceKeys returns matching service keys
     *
     * Uses fuzzyWatchWithServiceKeys to verify that the returned Future contains the set
     * of service keys that matched the pattern at watch time.
     */
    @Test
    @Order(7)
    @Disabled("Naming FuzzyWatch push notifications not yet fully implemented")
    void testFuzzyWatchWithServiceKeys() throws NacosException, InterruptedException, ExecutionException, TimeoutException {
        String uniquePrefix = "nfs007-" + UUID.randomUUID().toString().substring(0, 8);
        String servicePattern = uniquePrefix + "*";
        String service1 = uniquePrefix + "-svc1";
        String service2 = uniquePrefix + "-svc2";

        // Pre-register services
        namingService.registerInstance(service1, "192.168.100.9", 8080);
        namingService.registerInstance(service2, "192.168.100.10", 8080);
        Thread.sleep(2000);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                System.out.println("ServiceKeys test event: " + event);
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Watch with service keys - should return existing matching keys
        Future<ListView<String>> future = namingService.fuzzyWatchWithServiceKeys(servicePattern, DEFAULT_GROUP, watcher);
        assertNotNull(future, "Future should not be null");

        ListView<String> serviceKeys = future.get(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertNotNull(serviceKeys, "Service keys list should not be null");
        assertTrue(serviceKeys.getCount() >= 2,
                "Should have at least 2 matching service keys, got: " + serviceKeys.getCount());

        // Cleanup
        namingService.cancelFuzzyWatch(servicePattern, DEFAULT_GROUP, watcher);
        namingService.deregisterInstance(service1, "192.168.100.9", 8080);
        namingService.deregisterInstance(service2, "192.168.100.10", 8080);
    }
}
