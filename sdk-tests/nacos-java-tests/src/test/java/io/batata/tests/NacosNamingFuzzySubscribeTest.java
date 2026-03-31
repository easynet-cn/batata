package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.listener.FuzzyWatchChangeEvent;
import com.alibaba.nacos.api.naming.listener.FuzzyWatchEventWatcher;
import com.alibaba.nacos.api.naming.pojo.ListView;
import org.junit.jupiter.api.*;

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
     * NOTE: fuzzyWatch(groupName, watcher) sets serviceNamePattern=".*" (ANY_PATTERN).
     * SDK bug: FuzzyGroupKeyPattern.itemMatched(".*", name) treats ".*" as prefix "." match,
     * so client-side filtering drops all notifications for normal service names.
     * This affects both Nacos and Batata servers.
     */
    @Test
    @Order(2)
    @Disabled("SDK bug: FuzzyGroupKeyPattern.itemMatched treats ANY_PATTERN '.*' as prefix '.' match, drops notifications")
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

        // Watch using group pattern - wait for server to complete init sync
        namingService.fuzzyWatch(groupPattern, watcher);
        Thread.sleep(5000);

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

    // ==================== P1: SyncType Verification ====================

    /**
     * NFS-008: Verify SyncType for initial vs real-time events
     *
     * Pre-register a service, start watching, then register a new service.
     * Initial event should have FUZZY_WATCH_INIT_NOTIFY syncType,
     * real-time event should have FUZZY_WATCH_RESOURCE_CHANGED.
     */
    @Test
    @Order(8)
    void testFuzzyWatchSyncType() throws NacosException, InterruptedException {
        String uniquePrefix = "nfs008-" + UUID.randomUUID().toString().substring(0, 8);
        String servicePattern = uniquePrefix + "*";
        String existingService = uniquePrefix + "-existing";
        String newService = uniquePrefix + "-new";

        // Pre-register a service
        namingService.registerInstance(existingService, "192.168.108.1", 8080);
        Thread.sleep(1500);

        AtomicReference<String> initSyncType = new AtomicReference<>();
        AtomicReference<String> changeSyncType = new AtomicReference<>();
        CountDownLatch initLatch = new CountDownLatch(1);
        CountDownLatch changeLatch = new CountDownLatch(1);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                System.out.println("SyncType test: service=" + event.getServiceName()
                        + ", changeType=" + event.getChangeType()
                        + ", syncType=" + event.getSyncType());
                if (event.getServiceName() != null && event.getServiceName().equals(existingService)) {
                    initSyncType.set(event.getSyncType());
                    initLatch.countDown();
                } else if (event.getServiceName() != null && event.getServiceName().equals(newService)) {
                    changeSyncType.set(event.getSyncType());
                    changeLatch.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Start watching - should get INIT sync for existing service
        namingService.fuzzyWatch(servicePattern, DEFAULT_GROUP, watcher);
        boolean initReceived = initLatch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(initReceived, "Should receive init sync for existing service");
        assertEquals("FUZZY_WATCH_INIT_NOTIFY", initSyncType.get(),
                "Initial sync type should be FUZZY_WATCH_INIT_NOTIFY");

        // Register new service - should get RESOURCE_CHANGED sync
        namingService.registerInstance(newService, "192.168.108.2", 8080);
        boolean changeReceived = changeLatch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(changeReceived, "Should receive change event for new service");
        assertEquals("FUZZY_WATCH_RESOURCE_CHANGED", changeSyncType.get(),
                "Real-time sync type should be FUZZY_WATCH_RESOURCE_CHANGED");

        // Cleanup
        namingService.cancelFuzzyWatch(servicePattern, DEFAULT_GROUP, watcher);
        namingService.deregisterInstance(existingService, "192.168.108.1", 8080);
        namingService.deregisterInstance(newService, "192.168.108.2", 8080);
    }

    // ==================== P1: Multiple Watchers ====================

    /**
     * NFS-009: Multiple watchers on the same service pattern should all receive events
     */
    @Test
    @Order(9)
    void testMultipleWatchersOnSamePattern() throws NacosException, InterruptedException {
        String uniquePrefix = "nfs009-" + UUID.randomUUID().toString().substring(0, 8);
        String servicePattern = uniquePrefix + "*";
        String serviceName = uniquePrefix + "-multi";

        CountDownLatch latch1 = new CountDownLatch(1);
        CountDownLatch latch2 = new CountDownLatch(1);
        AtomicReference<String> watcher1Service = new AtomicReference<>();
        AtomicReference<String> watcher2Service = new AtomicReference<>();

        FuzzyWatchEventWatcher watcher1 = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                if (event.getServiceName() != null && event.getServiceName().equals(serviceName)) {
                    watcher1Service.set(event.getServiceName());
                    latch1.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        FuzzyWatchEventWatcher watcher2 = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                if (event.getServiceName() != null && event.getServiceName().equals(serviceName)) {
                    watcher2Service.set(event.getServiceName());
                    latch2.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Register both watchers on the same pattern
        namingService.fuzzyWatch(servicePattern, DEFAULT_GROUP, watcher1);
        namingService.fuzzyWatch(servicePattern, DEFAULT_GROUP, watcher2);
        Thread.sleep(2000);

        // Register a matching service
        namingService.registerInstance(serviceName, "192.168.109.1", 8080);

        boolean received1 = latch1.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        boolean received2 = latch2.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(received1, "Watcher 1 should receive the event");
        assertTrue(received2, "Watcher 2 should receive the event");
        assertEquals(serviceName, watcher1Service.get());
        assertEquals(serviceName, watcher2Service.get());

        // Cleanup
        namingService.cancelFuzzyWatch(servicePattern, DEFAULT_GROUP, watcher1);
        namingService.cancelFuzzyWatch(servicePattern, DEFAULT_GROUP, watcher2);
        namingService.deregisterInstance(serviceName, "192.168.109.1", 8080);
    }

    /**
     * NFS-010: Cancel one watcher should not affect other watchers on same pattern
     */
    @Test
    @Order(10)
    void testCancelOneWatcherPreservesOther() throws NacosException, InterruptedException {
        String uniquePrefix = "nfs010-" + UUID.randomUUID().toString().substring(0, 8);
        String servicePattern = uniquePrefix + "*";
        String serviceName = uniquePrefix + "-preserved";

        CountDownLatch survivorLatch = new CountDownLatch(1);
        CountDownLatch cancelledLatch = new CountDownLatch(1);

        FuzzyWatchEventWatcher cancelledWatcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                if (event.getServiceName() != null && event.getServiceName().equals(serviceName)) {
                    cancelledLatch.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        FuzzyWatchEventWatcher survivorWatcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                if (event.getServiceName() != null && event.getServiceName().equals(serviceName)) {
                    survivorLatch.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Register both watchers
        namingService.fuzzyWatch(servicePattern, DEFAULT_GROUP, cancelledWatcher);
        namingService.fuzzyWatch(servicePattern, DEFAULT_GROUP, survivorWatcher);
        Thread.sleep(2000);

        // Cancel one watcher
        namingService.cancelFuzzyWatch(servicePattern, DEFAULT_GROUP, cancelledWatcher);
        Thread.sleep(1000);

        // Register a matching service
        namingService.registerInstance(serviceName, "192.168.110.1", 8080);

        boolean survivorReceived = survivorLatch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        boolean cancelledReceived = cancelledLatch.await(5, TimeUnit.SECONDS);

        assertTrue(survivorReceived, "Surviving watcher should still receive events");
        assertFalse(cancelledReceived, "Cancelled watcher should NOT receive events");

        // Cleanup
        namingService.cancelFuzzyWatch(servicePattern, DEFAULT_GROUP, survivorWatcher);
        namingService.deregisterInstance(serviceName, "192.168.110.1", 8080);
    }

    // ==================== P1: Namespace Isolation ====================

    /**
     * NFS-011: FuzzyWatch namespace isolation for naming
     *
     * A fuzzy watch in the default namespace should NOT receive events for services
     * registered in a different namespace.
     */
    @Test
    @Order(11)
    void testFuzzyWatchNamespaceIsolation() throws Exception {
        String uniquePrefix = "nfs011-" + UUID.randomUUID().toString().substring(0, 8);
        String servicePattern = uniquePrefix + "*";
        String serviceName = uniquePrefix + "-nssvc";
        String otherNamespace = "nfs011-ns-" + UUID.randomUUID().toString().substring(0, 8);

        // Create a second NamingService for the other namespace
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties nsProperties = new Properties();
        nsProperties.setProperty("serverAddr", serverAddr);
        nsProperties.setProperty("username", username);
        nsProperties.setProperty("password", password);
        nsProperties.setProperty("namespace", otherNamespace);

        NamingService nsNamingService = NacosFactory.createNamingService(nsProperties);

        CountDownLatch defaultNsLatch = new CountDownLatch(1);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                System.out.println("Namespace isolation event: " + event);
                if (event.getServiceName() != null && event.getServiceName().equals(serviceName)) {
                    defaultNsLatch.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Watch in default namespace
        namingService.fuzzyWatch(servicePattern, DEFAULT_GROUP, watcher);
        Thread.sleep(2000);

        // Register service in OTHER namespace
        nsNamingService.registerInstance(serviceName, "192.168.111.1", 8080);
        Thread.sleep(1000);

        // Default namespace watcher should NOT receive the event
        boolean received = defaultNsLatch.await(5, TimeUnit.SECONDS);
        assertFalse(received,
                "Default namespace watcher should NOT receive events from other namespace");

        // Cleanup
        namingService.cancelFuzzyWatch(servicePattern, DEFAULT_GROUP, watcher);
        nsNamingService.deregisterInstance(serviceName, "192.168.111.1", 8080);
        nsNamingService.shutDown();
    }

    // ==================== P1: Watch with Fixed Group ====================

    /**
     * NFS-012: FuzzyWatch with fixed group name (not pattern)
     *
     * The fuzzyWatch(groupName, watcher) overload uses a fixed group name.
     * Registering a service in that exact group should trigger the event.
     */
    @Test
    @Order(12)
    @Disabled("Nacos SDK FuzzyGroupKeyPattern.itemMatched fails for ANY_PATTERN('.*') - also broken in Nacos 3.x")
    void testFuzzyWatchWithFixedGroup() throws NacosException, InterruptedException {
        String uniqueId = UUID.randomUUID().toString().substring(0, 8);
        String fixedGroup = "NFSGROUP-" + uniqueId;
        String serviceName = "nfs012-fixedgrp-" + uniqueId;

        AtomicReference<FuzzyWatchChangeEvent> receivedEvent = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(FuzzyWatchChangeEvent event) {
                System.out.println("Fixed group event: " + event);
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

        // Watch with fixed group name
        namingService.fuzzyWatch(fixedGroup, watcher);
        Thread.sleep(2000);

        // Register service in the exact group
        namingService.registerInstance(serviceName, fixedGroup, "192.168.112.1", 8080);

        boolean received = latch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(received, "Should receive event for service in fixed group");

        FuzzyWatchChangeEvent event = receivedEvent.get();
        assertNotNull(event, "Event should not be null");
        assertEquals(serviceName, event.getServiceName(), "Service name should match");
        assertEquals(fixedGroup, event.getGroupName(), "Group should match the fixed group");
        assertEquals("ADD_SERVICE", event.getChangeType(), "Change type should be ADD_SERVICE");

        // Cleanup
        namingService.cancelFuzzyWatch(fixedGroup, watcher);
        namingService.deregisterInstance(serviceName, fixedGroup, "192.168.112.1", 8080);
    }
}
