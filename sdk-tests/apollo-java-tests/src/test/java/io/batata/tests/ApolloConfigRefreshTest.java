package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigChangeListener;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.model.ConfigChange;
import com.ctrip.framework.apollo.model.ConfigChangeEvent;
import com.ctrip.framework.apollo.enums.PropertyChangeType;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Config Refresh Tests
 *
 * Tests for configuration refresh and reload functionality:
 * - Manual refresh
 * - Auto refresh
 * - Refresh callbacks
 * - Refresh intervals
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloConfigRefreshTest {

    @BeforeAll
    static void setupClass() {
        System.setProperty("app.id", System.getProperty("apollo.app.id", "test-app"));
        System.setProperty("apollo.meta", System.getProperty("apollo.meta", "http://127.0.0.1:8848"));
        System.setProperty("apollo.configService", System.getProperty("apollo.configService", "http://127.0.0.1:8848"));
        System.setProperty("env", System.getProperty("apollo.env", "DEV"));
        System.setProperty("apollo.cluster", System.getProperty("apollo.cluster", "default"));

        System.out.println("Apollo Config Refresh Test Setup");
    }

    // ==================== Basic Refresh Tests ====================

    /**
     * ACR-001: Test config initial load
     */
    @Test
    @Order(1)
    void testConfigInitialLoad() {
        Config config = ConfigService.getAppConfig();
        assertNotNull(config, "Config should be loaded");

        Set<String> propertyNames = config.getPropertyNames();
        System.out.println("Initial load - properties: " + propertyNames.size());
    }

    /**
     * ACR-002: Test config reload same instance
     */
    @Test
    @Order(2)
    void testConfigReloadSameInstance() {
        Config config1 = ConfigService.getAppConfig();
        Config config2 = ConfigService.getAppConfig();

        // Should return same instance (cached)
        assertSame(config1, config2, "Should return cached config instance");
        System.out.println("Config instances are same: " + (config1 == config2));
    }

    /**
     * ACR-003: Test multiple namespace refresh
     */
    @Test
    @Order(3)
    void testMultipleNamespaceRefresh() {
        String[] namespaces = {"application", "common", "database"};
        Map<String, Config> configs = new HashMap<>();

        for (String ns : namespaces) {
            try {
                Config config = ConfigService.getConfig(ns);
                configs.put(ns, config);
                System.out.println("Loaded namespace: " + ns);
            } catch (Exception e) {
                System.out.println("Namespace " + ns + ": " + e.getMessage());
            }
        }

        System.out.println("Loaded " + configs.size() + " namespaces");
    }

    // ==================== Change Listener Refresh Tests ====================

    /**
     * ACR-004: Test refresh with change listener
     */
    @Test
    @Order(4)
    void testRefreshWithChangeListener() throws InterruptedException {
        Config config = ConfigService.getConfig("application");

        AtomicInteger changeCount = new AtomicInteger(0);
        CountDownLatch latch = new CountDownLatch(1);

        ConfigChangeListener listener = changeEvent -> {
            changeCount.incrementAndGet();
            System.out.println("Config change detected: " + changeEvent.changedKeys());
            latch.countDown();
        };

        config.addChangeListener(listener);

        // Wait briefly for any pending changes
        boolean changed = latch.await(3, TimeUnit.SECONDS);
        System.out.println("Change detected: " + changed + ", count: " + changeCount.get());

        config.removeChangeListener(listener);
    }

    /**
     * ACR-005: Test refresh listener for specific keys
     */
    @Test
    @Order(5)
    void testRefreshListenerSpecificKeys() {
        Config config = ConfigService.getConfig("application");

        Set<String> interestedKeys = new HashSet<>(Arrays.asList(
                "refresh.key1", "refresh.key2", "refresh.key3"
        ));

        AtomicBoolean triggered = new AtomicBoolean(false);

        ConfigChangeListener listener = changeEvent -> {
            for (String key : changeEvent.changedKeys()) {
                if (interestedKeys.contains(key)) {
                    triggered.set(true);
                    System.out.println("Interested key changed: " + key);
                }
            }
        };

        config.addChangeListener(listener, interestedKeys);

        // Read current values
        for (String key : interestedKeys) {
            String value = config.getProperty(key, "default");
            System.out.println(key + " = " + value);
        }

        config.removeChangeListener(listener);
    }

    /**
     * ACR-006: Test refresh with multiple listeners
     */
    @Test
    @Order(6)
    void testRefreshMultipleListeners() {
        Config config = ConfigService.getConfig("application");

        AtomicInteger listener1Count = new AtomicInteger(0);
        AtomicInteger listener2Count = new AtomicInteger(0);
        AtomicInteger listener3Count = new AtomicInteger(0);

        ConfigChangeListener listener1 = e -> listener1Count.incrementAndGet();
        ConfigChangeListener listener2 = e -> listener2Count.incrementAndGet();
        ConfigChangeListener listener3 = e -> listener3Count.incrementAndGet();

        config.addChangeListener(listener1);
        config.addChangeListener(listener2);
        config.addChangeListener(listener3);

        // Read a value to trigger potential refresh
        config.getProperty("test.key", "default");

        System.out.println("Listener counts: " + listener1Count.get() + ", " +
                          listener2Count.get() + ", " + listener3Count.get());

        config.removeChangeListener(listener1);
        config.removeChangeListener(listener2);
        config.removeChangeListener(listener3);
    }

    // ==================== Property Access Refresh Tests ====================

    /**
     * ACR-007: Test property access triggers refresh check
     */
    @Test
    @Order(7)
    void testPropertyAccessRefreshCheck() {
        Config config = ConfigService.getConfig("application");

        // Multiple property accesses
        for (int i = 0; i < 10; i++) {
            String value = config.getProperty("access.test.key." + i, "default");
            System.out.println("Access " + i + ": " + value);
        }

        System.out.println("Property access refresh check completed");
    }

    /**
     * ACR-008: Test concurrent property access
     */
    @Test
    @Order(8)
    void testConcurrentPropertyAccess() throws InterruptedException {
        Config config = ConfigService.getConfig("application");
        int threadCount = 20;
        CountDownLatch latch = new CountDownLatch(threadCount);
        List<String> results = new CopyOnWriteArrayList<>();

        for (int i = 0; i < threadCount; i++) {
            final int idx = i;
            new Thread(() -> {
                try {
                    String value = config.getProperty("concurrent.key", "default-" + idx);
                    results.add(value);
                } finally {
                    latch.countDown();
                }
            }).start();
        }

        boolean completed = latch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete");

        System.out.println("Concurrent access results: " + results.size());
    }

    // ==================== Refresh Interval Tests ====================

    /**
     * ACR-009: Test config refresh interval
     */
    @Test
    @Order(9)
    void testConfigRefreshInterval() throws InterruptedException {
        Config config = ConfigService.getConfig("application");

        String key = "interval.test.key";
        List<String> values = new ArrayList<>();

        // Sample values over time
        for (int i = 0; i < 5; i++) {
            String value = config.getProperty(key, "default");
            values.add(value);
            Thread.sleep(500);
        }

        System.out.println("Values over time: " + values);
    }

    /**
     * ACR-010: Test refresh under load
     */
    @Test
    @Order(10)
    void testRefreshUnderLoad() throws InterruptedException {
        Config config = ConfigService.getConfig("application");
        int iterations = 100;
        long startTime = System.currentTimeMillis();

        for (int i = 0; i < iterations; i++) {
            config.getProperty("load.test.key." + (i % 10), "default");
        }

        long duration = System.currentTimeMillis() - startTime;
        System.out.println(iterations + " property accesses in " + duration + "ms");
        System.out.println("Average: " + (duration / iterations) + "ms per access");
    }

    // ==================== Change Type Tests ====================

    /**
     * ACR-011: Test change event types
     */
    @Test
    @Order(11)
    void testChangeEventTypes() {
        Config config = ConfigService.getConfig("application");

        ConfigChangeListener listener = changeEvent -> {
            for (String key : changeEvent.changedKeys()) {
                ConfigChange change = changeEvent.getChange(key);
                PropertyChangeType changeType = change.getChangeType();

                System.out.println("Key: " + key);
                System.out.println("  Type: " + changeType);
                System.out.println("  Old: " + change.getOldValue());
                System.out.println("  New: " + change.getNewValue());
            }
        };

        config.addChangeListener(listener);

        // Trigger a read
        config.getProperty("change.type.test", "default");

        config.removeChangeListener(listener);
        System.out.println("Change event types test completed");
    }

    /**
     * ACR-012: Test namespace in change event
     */
    @Test
    @Order(12)
    void testNamespaceInChangeEvent() {
        Config config = ConfigService.getConfig("application");

        ConfigChangeListener listener = changeEvent -> {
            String namespace = changeEvent.getNamespace();
            System.out.println("Change in namespace: " + namespace);
            System.out.println("Changed keys: " + changeEvent.changedKeys());
        };

        config.addChangeListener(listener);

        // Access property
        config.getProperty("namespace.change.key", "default");

        config.removeChangeListener(listener);
    }

    // ==================== Refresh Edge Cases ====================

    /**
     * ACR-013: Test refresh with null values
     */
    @Test
    @Order(13)
    void testRefreshWithNullValues() {
        Config config = ConfigService.getConfig("application");

        // Get non-existent key (should return default, not trigger error on refresh)
        String value = config.getProperty("non.existent.key.for.refresh", null);
        assertNull(value, "Non-existent key should return null default");

        String valueWithDefault = config.getProperty("non.existent.key.for.refresh", "fallback");
        assertEquals("fallback", valueWithDefault);

        System.out.println("Null value refresh test completed");
    }

    /**
     * ACR-014: Test refresh after listener removal
     */
    @Test
    @Order(14)
    void testRefreshAfterListenerRemoval() throws InterruptedException {
        Config config = ConfigService.getConfig("application");

        AtomicInteger changeCount = new AtomicInteger(0);
        ConfigChangeListener listener = e -> changeCount.incrementAndGet();

        config.addChangeListener(listener);
        Thread.sleep(100);

        config.removeChangeListener(listener);

        // Further changes should not increment
        int countAfterRemoval = changeCount.get();
        Thread.sleep(500);

        System.out.println("Changes before removal: " + countAfterRemoval);
        System.out.println("Changes after removal: " + changeCount.get());
    }

    /**
     * ACR-015: Test refresh with empty namespace
     */
    @Test
    @Order(15)
    void testRefreshEmptyNamespace() {
        try {
            Config config = ConfigService.getConfig("empty-namespace-test");
            Set<String> names = config.getPropertyNames();

            System.out.println("Empty namespace properties: " + names.size());

            // Should handle gracefully
            String value = config.getProperty("any.key", "default");
            assertEquals("default", value);
        } catch (Exception e) {
            System.out.println("Empty namespace: " + e.getMessage());
        }
    }

    // ==================== Performance Tests ====================

    /**
     * ACR-016: Test refresh performance
     */
    @Test
    @Order(16)
    void testRefreshPerformance() {
        Config config = ConfigService.getConfig("application");

        int iterations = 1000;
        long startTime = System.currentTimeMillis();

        for (int i = 0; i < iterations; i++) {
            config.getProperty("perf.test.key", "default");
        }

        long duration = System.currentTimeMillis() - startTime;
        double avgMs = (double) duration / iterations;

        System.out.println(iterations + " reads in " + duration + "ms");
        System.out.println("Average: " + String.format("%.3f", avgMs) + "ms per read");
    }

    /**
     * ACR-017: Test listener callback performance
     */
    @Test
    @Order(17)
    void testListenerCallbackPerformance() {
        Config config = ConfigService.getConfig("application");

        AtomicLong totalCallbackTime = new AtomicLong(0);
        AtomicInteger callbackCount = new AtomicInteger(0);

        ConfigChangeListener listener = changeEvent -> {
            long start = System.nanoTime();
            // Simulate some processing
            for (String key : changeEvent.changedKeys()) {
                changeEvent.getChange(key);
            }
            long elapsed = System.nanoTime() - start;
            totalCallbackTime.addAndGet(elapsed);
            callbackCount.incrementAndGet();
        };

        config.addChangeListener(listener);

        // Trigger some activity
        for (int i = 0; i < 10; i++) {
            config.getProperty("callback.perf.key." + i, "default");
        }

        config.removeChangeListener(listener);

        if (callbackCount.get() > 0) {
            long avgNs = totalCallbackTime.get() / callbackCount.get();
            System.out.println("Callbacks: " + callbackCount.get());
            System.out.println("Average callback time: " + avgNs + "ns");
        } else {
            System.out.println("No callbacks triggered");
        }
    }

    /**
     * ACR-018: Test concurrent listener registration
     */
    @Test
    @Order(18)
    void testConcurrentListenerRegistration() throws InterruptedException {
        Config config = ConfigService.getConfig("application");
        int listenerCount = 10;
        CountDownLatch latch = new CountDownLatch(listenerCount);
        List<ConfigChangeListener> listeners = new CopyOnWriteArrayList<>();

        for (int i = 0; i < listenerCount; i++) {
            new Thread(() -> {
                ConfigChangeListener listener = e -> {};
                config.addChangeListener(listener);
                listeners.add(listener);
                latch.countDown();
            }).start();
        }

        boolean completed = latch.await(10, TimeUnit.SECONDS);
        assertTrue(completed);

        System.out.println("Registered " + listeners.size() + " listeners concurrently");

        // Cleanup
        for (ConfigChangeListener listener : listeners) {
            config.removeChangeListener(listener);
        }
    }
}
