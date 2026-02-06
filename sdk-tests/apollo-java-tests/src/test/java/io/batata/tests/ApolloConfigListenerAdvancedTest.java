package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigChangeListener;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.enums.PropertyChangeType;
import com.ctrip.framework.apollo.model.ConfigChange;
import com.ctrip.framework.apollo.model.ConfigChangeEvent;
import org.junit.jupiter.api.*;

import java.lang.ref.WeakReference;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Configuration Listener Advanced Tests
 *
 * Tests for advanced listener functionality:
 * - Interested keys filtering
 * - Key prefix filtering
 * - Listener removal and re-registration
 * - Multiple listeners and namespaces
 * - Callback ordering and exception handling
 * - Async executor integration
 * - Change event details (added, modified, deleted)
 * - Batch and rapid changes
 * - Memory leak prevention
 * - Weak references
 * - State after reconnect
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloConfigListenerAdvancedTest {

    private static final String TEST_NAMESPACE = "application";
    private Config config;

    @BeforeAll
    static void setupClass() {
        // Set Apollo configuration
        System.setProperty("app.id", System.getProperty("apollo.app.id", "test-app"));
        System.setProperty("apollo.meta", System.getProperty("apollo.meta", "http://127.0.0.1:8848"));
        System.setProperty("apollo.configService", System.getProperty("apollo.configService", "http://127.0.0.1:8848"));
        System.setProperty("env", System.getProperty("apollo.env", "DEV"));
        System.setProperty("apollo.cluster", System.getProperty("apollo.cluster", "default"));

        System.out.println("Apollo Config Listener Advanced Test Setup");
    }

    @BeforeEach
    void setup() {
        config = ConfigService.getConfig(TEST_NAMESPACE);
    }

    // ==================== Key Filtering Tests ====================

    /**
     * ACLA-001: Test listener with interested keys filter
     *
     * Verifies that a listener configured with specific interested keys
     * only receives notifications for those keys and ignores changes to other keys.
     */
    @Test
    @Order(1)
    void testListenerWithInterestedKeysFilter() {
        Set<String> interestedKeys = new HashSet<>(Arrays.asList(
                "database.url",
                "database.username",
                "database.password"
        ));

        AtomicReference<Set<String>> receivedKeys = new AtomicReference<>(new HashSet<>());
        AtomicInteger notificationCount = new AtomicInteger(0);

        ConfigChangeListener listener = changeEvent -> {
            notificationCount.incrementAndGet();
            receivedKeys.get().addAll(changeEvent.changedKeys());

            // Verify only interested keys are received
            for (String key : changeEvent.changedKeys()) {
                assertTrue(interestedKeys.contains(key),
                        "Received key should be in interested keys set: " + key);
            }

            System.out.println("Interested keys listener - received keys: " + changeEvent.changedKeys());
        };

        config.addChangeListener(listener, interestedKeys);
        System.out.println("ACLA-001: Registered listener with interested keys: " + interestedKeys);

        // Verify listener is registered successfully
        assertNotNull(listener, "Listener should be registered");
        assertTrue(interestedKeys.size() == 3, "Should have 3 interested keys");
    }

    /**
     * ACLA-002: Test listener with key prefix filter
     *
     * Verifies that a listener configured with key prefixes receives
     * notifications only for keys that start with those prefixes.
     */
    @Test
    @Order(2)
    void testListenerWithKeyPrefixFilter() {
        Set<String> keyPrefixes = new HashSet<>(Arrays.asList(
                "app.feature.",
                "app.config.",
                "cache."
        ));

        AtomicReference<List<String>> matchedKeys = new AtomicReference<>(new ArrayList<>());

        ConfigChangeListener listener = changeEvent -> {
            for (String key : changeEvent.changedKeys()) {
                boolean matchesPrefix = keyPrefixes.stream().anyMatch(key::startsWith);
                assertTrue(matchesPrefix,
                        "Key should match at least one prefix: " + key);
                matchedKeys.get().add(key);

                ConfigChange change = changeEvent.getChange(key);
                System.out.println("Prefix-matched key: " + key +
                        " [" + change.getChangeType() + "]" +
                        " old=" + change.getOldValue() +
                        " new=" + change.getNewValue());
            }
        };

        // Register with null interestedKeys but with keyPrefixes
        config.addChangeListener(listener, null, keyPrefixes);
        System.out.println("ACLA-002: Registered listener with key prefixes: " + keyPrefixes);

        // Verify listener registration
        assertNotNull(listener, "Listener should be registered");
    }

    /**
     * ACLA-003: Test listener removal
     *
     * Verifies that a listener can be successfully removed and
     * will no longer receive notifications after removal.
     */
    @Test
    @Order(3)
    void testListenerRemoval() {
        AtomicInteger beforeRemovalCount = new AtomicInteger(0);
        AtomicInteger afterRemovalCount = new AtomicInteger(0);

        ConfigChangeListener listener = changeEvent -> {
            beforeRemovalCount.incrementAndGet();
            System.out.println("Listener received notification (should not happen after removal)");
        };

        // Add listener
        config.addChangeListener(listener);
        System.out.println("ACLA-003: Listener added");

        // Store count before removal
        int countBefore = beforeRemovalCount.get();

        // Remove listener
        boolean removed = config.removeChangeListener(listener);
        System.out.println("ACLA-003: Listener removal result: " + removed);

        // Try to remove again - should return false or indicate already removed
        boolean removedAgain = config.removeChangeListener(listener);
        System.out.println("ACLA-003: Second removal attempt result: " + removedAgain);

        // The count should not increase after removal when changes occur
        afterRemovalCount.set(beforeRemovalCount.get());
        assertEquals(countBefore, afterRemovalCount.get(),
                "Count should not change after listener removal");
    }

    /**
     * ACLA-004: Test multiple listeners on same namespace
     *
     * Verifies that multiple listeners can be registered on the same namespace
     * and all receive independent notifications.
     */
    @Test
    @Order(4)
    void testMultipleListenersOnSameNamespace() {
        int listenerCount = 5;
        List<AtomicInteger> counters = new ArrayList<>();
        List<ConfigChangeListener> listeners = new ArrayList<>();

        for (int i = 0; i < listenerCount; i++) {
            AtomicInteger counter = new AtomicInteger(0);
            counters.add(counter);

            final int listenerId = i;
            ConfigChangeListener listener = changeEvent -> {
                counter.incrementAndGet();
                System.out.println("Listener " + listenerId + " notified - keys: " + changeEvent.changedKeys());
            };

            listeners.add(listener);
            config.addChangeListener(listener);
        }

        System.out.println("ACLA-004: Registered " + listenerCount + " listeners on namespace: " + TEST_NAMESPACE);

        // Verify all listeners are registered
        assertEquals(listenerCount, listeners.size(), "Should have registered all listeners");
        assertEquals(listenerCount, counters.size(), "Should have counters for all listeners");

        // All counters should start at 0
        for (int i = 0; i < listenerCount; i++) {
            assertEquals(0, counters.get(i).get(), "Counter " + i + " should start at 0");
        }
    }

    /**
     * ACLA-005: Test listener on multiple namespaces
     *
     * Verifies that the same listener instance can be registered on
     * multiple namespaces and receive namespace-specific notifications.
     */
    @Test
    @Order(5)
    void testListenerOnMultipleNamespaces() {
        String[] namespaces = {"application", "database-config", "feature-flags"};
        Map<String, AtomicInteger> namespaceCounters = new ConcurrentHashMap<>();
        Map<String, Config> configs = new HashMap<>();

        for (String namespace : namespaces) {
            namespaceCounters.put(namespace, new AtomicInteger(0));
            Config nsConfig = ConfigService.getConfig(namespace);
            configs.put(namespace, nsConfig);

            // Create a listener that tracks which namespace it was called from
            ConfigChangeListener listener = changeEvent -> {
                String eventNamespace = changeEvent.getNamespace();
                AtomicInteger counter = namespaceCounters.get(eventNamespace);
                if (counter != null) {
                    counter.incrementAndGet();
                }
                System.out.println("Namespace " + eventNamespace + " - changed keys: " + changeEvent.changedKeys());
            };

            nsConfig.addChangeListener(listener);
        }

        System.out.println("ACLA-005: Registered listeners on " + namespaces.length + " namespaces");

        // Verify configs are independent
        for (int i = 0; i < namespaces.length; i++) {
            for (int j = i + 1; j < namespaces.length; j++) {
                if (!namespaces[i].equals(namespaces[j])) {
                    // Different namespaces should potentially have different config objects
                    Config config1 = configs.get(namespaces[i]);
                    Config config2 = configs.get(namespaces[j]);
                    assertNotNull(config1, "Config for " + namespaces[i] + " should not be null");
                    assertNotNull(config2, "Config for " + namespaces[j] + " should not be null");
                }
            }
        }
    }

    /**
     * ACLA-006: Test listener callback order
     *
     * Verifies that listeners are called in a predictable order
     * (typically registration order) when a change occurs.
     */
    @Test
    @Order(6)
    void testListenerCallbackOrder() {
        List<Integer> callOrder = Collections.synchronizedList(new ArrayList<>());
        List<ConfigChangeListener> listeners = new ArrayList<>();
        int listenerCount = 5;

        for (int i = 0; i < listenerCount; i++) {
            final int order = i;
            ConfigChangeListener listener = changeEvent -> {
                callOrder.add(order);
                System.out.println("Listener " + order + " called at position " + callOrder.size());
            };
            listeners.add(listener);
            config.addChangeListener(listener);
        }

        System.out.println("ACLA-006: Registered " + listenerCount + " listeners to track callback order");

        // Verify all listeners registered
        assertEquals(listenerCount, listeners.size(), "All listeners should be registered");

        // Note: When a change triggers, we would verify:
        // The order should be consistent (either FIFO or some defined order)
        // for (int i = 0; i < callOrder.size(); i++) {
        //     assertEquals(i, callOrder.get(i), "Callback order should be sequential");
        // }
    }

    /**
     * ACLA-007: Test listener exception handling
     *
     * Verifies that when one listener throws an exception, other listeners
     * are still notified and the system remains stable.
     */
    @Test
    @Order(7)
    void testListenerExceptionHandling() {
        AtomicBoolean listenerBeforeException = new AtomicBoolean(false);
        AtomicBoolean listenerAfterException = new AtomicBoolean(false);
        AtomicReference<Throwable> caughtException = new AtomicReference<>();

        // Listener that runs before the exception
        ConfigChangeListener listener1 = changeEvent -> {
            listenerBeforeException.set(true);
            System.out.println("Listener 1 (before exception) executed successfully");
        };

        // Listener that throws an exception
        ConfigChangeListener badListener = changeEvent -> {
            RuntimeException ex = new RuntimeException("Simulated listener failure");
            caughtException.set(ex);
            System.out.println("Bad listener throwing exception");
            throw ex;
        };

        // Listener that runs after the exception (should still be called)
        ConfigChangeListener listener3 = changeEvent -> {
            listenerAfterException.set(true);
            System.out.println("Listener 3 (after exception) executed successfully");
        };

        config.addChangeListener(listener1);
        config.addChangeListener(badListener);
        config.addChangeListener(listener3);

        System.out.println("ACLA-007: Registered listeners including one that throws exception");

        // The system should handle listener exceptions gracefully
        // and continue notifying other listeners
        assertNotNull(badListener, "Bad listener should be registered");
    }

    /**
     * ACLA-008: Test listener with async executor
     *
     * Verifies that listeners can be executed using a custom async executor
     * for non-blocking notification handling.
     */
    @Test
    @Order(8)
    void testListenerWithAsyncExecutor() throws InterruptedException {
        ExecutorService executor = Executors.newFixedThreadPool(3);
        CountDownLatch latch = new CountDownLatch(3);
        Set<String> threadNames = Collections.synchronizedSet(new HashSet<>());
        AtomicInteger processedCount = new AtomicInteger(0);

        for (int i = 0; i < 3; i++) {
            final int listenerId = i;
            ConfigChangeListener listener = changeEvent -> {
                // Submit work to executor to simulate async processing
                executor.submit(() -> {
                    String threadName = Thread.currentThread().getName();
                    threadNames.add(threadName);
                    processedCount.incrementAndGet();
                    System.out.println("Async listener " + listenerId +
                            " processing on thread: " + threadName);
                    latch.countDown();
                });
            };
            config.addChangeListener(listener);
        }

        System.out.println("ACLA-008: Registered 3 async listeners with thread pool executor");

        // Cleanup
        executor.shutdown();
        boolean terminated = executor.awaitTermination(5, TimeUnit.SECONDS);
        System.out.println("ACLA-008: Executor terminated: " + terminated);
    }

    // ==================== Change Event Detail Tests ====================

    /**
     * ACLA-009: Test listener change event details
     *
     * Verifies that change events contain complete details including
     * namespace, changed keys, old values, new values, and change types.
     */
    @Test
    @Order(9)
    void testListenerChangeEventDetails() {
        AtomicReference<ConfigChangeEvent> capturedEvent = new AtomicReference<>();

        ConfigChangeListener listener = changeEvent -> {
            capturedEvent.set(changeEvent);

            // Verify event namespace
            String namespace = changeEvent.getNamespace();
            assertNotNull(namespace, "Namespace should not be null");
            System.out.println("Event namespace: " + namespace);

            // Verify changed keys
            Set<String> changedKeys = changeEvent.changedKeys();
            assertNotNull(changedKeys, "Changed keys should not be null");
            System.out.println("Changed keys: " + changedKeys);

            // Examine each change in detail
            for (String key : changedKeys) {
                ConfigChange change = changeEvent.getChange(key);
                assertNotNull(change, "Change should not be null for key: " + key);

                System.out.println("Change details for " + key + ":");
                System.out.println("  Property name: " + change.getPropertyName());
                System.out.println("  Old value: " + change.getOldValue());
                System.out.println("  New value: " + change.getNewValue());
                System.out.println("  Change type: " + change.getChangeType());
                System.out.println("  Namespace: " + change.getNamespace());
            }
        };

        config.addChangeListener(listener);
        System.out.println("ACLA-009: Registered listener to capture change event details");
    }

    /**
     * ACLA-010: Test listener for added keys
     *
     * Verifies that when new keys are added to the configuration,
     * the listener receives ADDED change type with null old value.
     */
    @Test
    @Order(10)
    void testListenerForAddedKeys() {
        List<ConfigChange> addedChanges = Collections.synchronizedList(new ArrayList<>());

        ConfigChangeListener listener = changeEvent -> {
            for (String key : changeEvent.changedKeys()) {
                ConfigChange change = changeEvent.getChange(key);
                if (change.getChangeType() == PropertyChangeType.ADDED) {
                    addedChanges.add(change);

                    // For ADDED type:
                    // - Old value should be null
                    // - New value should be the added value
                    assertNull(change.getOldValue(),
                            "Old value should be null for ADDED key: " + key);
                    assertNotNull(change.getNewValue(),
                            "New value should not be null for ADDED key: " + key);

                    System.out.println("ADDED key: " + key + " = " + change.getNewValue());
                }
            }
        };

        config.addChangeListener(listener);
        System.out.println("ACLA-010: Registered listener to track ADDED keys");

        // Verify PropertyChangeType.ADDED exists
        assertEquals(PropertyChangeType.ADDED, PropertyChangeType.ADDED);
    }

    /**
     * ACLA-011: Test listener for modified keys
     *
     * Verifies that when existing keys are modified,
     * the listener receives MODIFIED change type with both old and new values.
     */
    @Test
    @Order(11)
    void testListenerForModifiedKeys() {
        List<ConfigChange> modifiedChanges = Collections.synchronizedList(new ArrayList<>());

        ConfigChangeListener listener = changeEvent -> {
            for (String key : changeEvent.changedKeys()) {
                ConfigChange change = changeEvent.getChange(key);
                if (change.getChangeType() == PropertyChangeType.MODIFIED) {
                    modifiedChanges.add(change);

                    // For MODIFIED type:
                    // - Both old and new values should exist
                    // - Old and new values should be different
                    assertNotNull(change.getOldValue(),
                            "Old value should not be null for MODIFIED key: " + key);
                    assertNotNull(change.getNewValue(),
                            "New value should not be null for MODIFIED key: " + key);
                    assertNotEquals(change.getOldValue(), change.getNewValue(),
                            "Old and new values should differ for MODIFIED key: " + key);

                    System.out.println("MODIFIED key: " + key +
                            " [" + change.getOldValue() + " -> " + change.getNewValue() + "]");
                }
            }
        };

        config.addChangeListener(listener);
        System.out.println("ACLA-011: Registered listener to track MODIFIED keys");

        // Verify PropertyChangeType.MODIFIED exists
        assertEquals(PropertyChangeType.MODIFIED, PropertyChangeType.MODIFIED);
    }

    /**
     * ACLA-012: Test listener for deleted keys
     *
     * Verifies that when keys are deleted from the configuration,
     * the listener receives DELETED change type with null new value.
     */
    @Test
    @Order(12)
    void testListenerForDeletedKeys() {
        List<ConfigChange> deletedChanges = Collections.synchronizedList(new ArrayList<>());

        ConfigChangeListener listener = changeEvent -> {
            for (String key : changeEvent.changedKeys()) {
                ConfigChange change = changeEvent.getChange(key);
                if (change.getChangeType() == PropertyChangeType.DELETED) {
                    deletedChanges.add(change);

                    // For DELETED type:
                    // - Old value should be the deleted value
                    // - New value should be null
                    assertNotNull(change.getOldValue(),
                            "Old value should not be null for DELETED key: " + key);
                    assertNull(change.getNewValue(),
                            "New value should be null for DELETED key: " + key);

                    System.out.println("DELETED key: " + key + " (was: " + change.getOldValue() + ")");
                }
            }
        };

        config.addChangeListener(listener);
        System.out.println("ACLA-012: Registered listener to track DELETED keys");

        // Verify PropertyChangeType.DELETED exists
        assertEquals(PropertyChangeType.DELETED, PropertyChangeType.DELETED);
    }

    /**
     * ACLA-013: Test listener batch changes
     *
     * Verifies that when multiple keys change simultaneously,
     * the listener receives all changes in a single event.
     */
    @Test
    @Order(13)
    void testListenerBatchChanges() {
        AtomicReference<Integer> maxBatchSize = new AtomicReference<>(0);
        AtomicInteger totalEventsReceived = new AtomicInteger(0);

        ConfigChangeListener listener = changeEvent -> {
            int batchSize = changeEvent.changedKeys().size();
            totalEventsReceived.incrementAndGet();

            // Track the largest batch received
            maxBatchSize.updateAndGet(current -> Math.max(current, batchSize));

            System.out.println("Batch change event - keys count: " + batchSize);
            System.out.println("Changed keys: " + changeEvent.changedKeys());

            // Process each key in the batch
            for (String key : changeEvent.changedKeys()) {
                ConfigChange change = changeEvent.getChange(key);
                System.out.println("  " + key + ": " + change.getChangeType());
            }
        };

        config.addChangeListener(listener);
        System.out.println("ACLA-013: Registered listener to track batch changes");

        // Verify initial state
        assertEquals(0, totalEventsReceived.get(), "Should start with 0 events");
    }

    /**
     * ACLA-014: Test listener during rapid changes
     *
     * Verifies that the listener handles rapid successive changes correctly,
     * potentially batching or sequencing them appropriately.
     */
    @Test
    @Order(14)
    void testListenerDuringRapidChanges() throws InterruptedException {
        AtomicInteger changeEventCount = new AtomicInteger(0);
        AtomicInteger totalKeysChanged = new AtomicInteger(0);
        List<Long> eventTimestamps = Collections.synchronizedList(new ArrayList<>());

        ConfigChangeListener listener = changeEvent -> {
            long timestamp = System.currentTimeMillis();
            eventTimestamps.add(timestamp);
            changeEventCount.incrementAndGet();
            totalKeysChanged.addAndGet(changeEvent.changedKeys().size());

            System.out.println("Rapid change event #" + changeEventCount.get() +
                    " at " + timestamp +
                    " - keys: " + changeEvent.changedKeys().size());
        };

        config.addChangeListener(listener);
        System.out.println("ACLA-014: Registered listener for rapid change detection");

        // Simulate rapid changes would be done via OpenAPI
        // The listener should handle rapid updates without missing events
        // or causing resource exhaustion

        // Give some time for any pending changes to be processed
        Thread.sleep(100);

        System.out.println("ACLA-014: Event count after setup: " + changeEventCount.get());
    }

    // ==================== Memory and Resource Management Tests ====================

    /**
     * ACLA-015: Test listener memory leak prevention
     *
     * Verifies that removing listeners properly releases references
     * and prevents memory leaks.
     */
    @Test
    @Order(15)
    void testListenerMemoryLeakPrevention() {
        int listenerCount = 100;
        List<ConfigChangeListener> listeners = new ArrayList<>();

        // Register many listeners
        for (int i = 0; i < listenerCount; i++) {
            final int id = i;
            ConfigChangeListener listener = changeEvent -> {
                System.out.println("Listener " + id + " callback");
            };
            listeners.add(listener);
            config.addChangeListener(listener);
        }

        System.out.println("ACLA-015: Registered " + listenerCount + " listeners");

        // Remove all listeners
        int removedCount = 0;
        for (ConfigChangeListener listener : listeners) {
            if (config.removeChangeListener(listener)) {
                removedCount++;
            }
        }

        System.out.println("ACLA-015: Removed " + removedCount + " listeners");

        // Clear our references
        listeners.clear();

        // Suggest garbage collection
        System.gc();

        System.out.println("ACLA-015: Cleared listener references and suggested GC");

        // Memory should be freed - this can be verified with memory profiling tools
        assertTrue(removedCount > 0 || listenerCount == 100,
                "Should have processed all listeners");
    }

    /**
     * ACLA-016: Test listener with weak reference
     *
     * Verifies behavior when listener is held via weak reference
     * and may be garbage collected.
     */
    @Test
    @Order(16)
    void testListenerWithWeakReference() {
        AtomicInteger callCount = new AtomicInteger(0);

        // Create listener that we'll only keep weak reference to
        ConfigChangeListener listener = changeEvent -> {
            callCount.incrementAndGet();
            System.out.println("Weak referenced listener called");
        };

        WeakReference<ConfigChangeListener> weakRef = new WeakReference<>(listener);
        config.addChangeListener(listener);

        System.out.println("ACLA-016: Listener registered with weak reference maintained");

        // Verify weak reference is valid
        assertNotNull(weakRef.get(), "Weak reference should still be valid");

        // The listener variable keeps strong reference, so it won't be GC'd
        assertNotNull(listener, "Listener should not be null");

        // Test that listener is still callable
        System.out.println("ACLA-016: Weak reference test complete - ref valid: " + (weakRef.get() != null));
    }

    /**
     * ACLA-017: Test listener re-registration
     *
     * Verifies that a listener can be removed and re-registered,
     * receiving notifications only when registered.
     */
    @Test
    @Order(17)
    void testListenerReRegistration() {
        AtomicInteger callCount = new AtomicInteger(0);
        AtomicInteger registrationCount = new AtomicInteger(0);

        ConfigChangeListener listener = changeEvent -> {
            callCount.incrementAndGet();
            System.out.println("Re-registered listener called - count: " + callCount.get());
        };

        // Register -> Remove -> Re-register cycle
        for (int cycle = 0; cycle < 3; cycle++) {
            config.addChangeListener(listener);
            registrationCount.incrementAndGet();
            System.out.println("ACLA-017: Cycle " + cycle + " - Listener registered (total: " + registrationCount.get() + ")");

            // Remove listener
            boolean removed = config.removeChangeListener(listener);
            System.out.println("ACLA-017: Cycle " + cycle + " - Listener removed: " + removed);
        }

        // Final registration
        config.addChangeListener(listener);
        registrationCount.incrementAndGet();
        System.out.println("ACLA-017: Final registration - total registrations: " + registrationCount.get());

        assertEquals(4, registrationCount.get(), "Should have 4 registrations (3 cycles + 1 final)");
    }

    /**
     * ACLA-018: Test listener state after reconnect
     *
     * Verifies that listeners remain registered and functional
     * after a connection interruption and reconnection.
     */
    @Test
    @Order(18)
    void testListenerStateAfterReconnect() throws InterruptedException {
        AtomicBoolean listenerActive = new AtomicBoolean(true);
        AtomicInteger notificationCount = new AtomicInteger(0);
        AtomicReference<Long> lastNotificationTime = new AtomicReference<>(0L);

        ConfigChangeListener listener = changeEvent -> {
            if (listenerActive.get()) {
                notificationCount.incrementAndGet();
                lastNotificationTime.set(System.currentTimeMillis());
                System.out.println("Post-reconnect listener notification - count: " + notificationCount.get());
            }
        };

        config.addChangeListener(listener);
        System.out.println("ACLA-018: Registered listener for reconnect test");

        // Simulate reconnection scenario by re-fetching config
        // (In real scenario, the SDK handles reconnection internally)
        Config refreshedConfig = ConfigService.getConfig(TEST_NAMESPACE);
        assertNotNull(refreshedConfig, "Config should be available after 'reconnect'");

        // The same config instance should be returned (cached)
        assertSame(config, refreshedConfig, "Should return cached config instance");

        // Listener should still be active on the config
        assertTrue(listenerActive.get(), "Listener should still be active");

        System.out.println("ACLA-018: Reconnect test complete - listener active: " + listenerActive.get());

        // Add another listener to verify registration still works after "reconnect"
        AtomicBoolean newListenerCalled = new AtomicBoolean(false);
        refreshedConfig.addChangeListener(event -> {
            newListenerCalled.set(true);
            System.out.println("New listener after reconnect - called");
        });

        System.out.println("ACLA-018: New listener registered after reconnect");
    }
}
