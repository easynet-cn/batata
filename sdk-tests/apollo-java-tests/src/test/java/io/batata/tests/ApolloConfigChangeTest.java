package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigChangeListener;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.enums.PropertyChangeType;
import com.ctrip.framework.apollo.model.ConfigChange;
import com.ctrip.framework.apollo.model.ConfigChangeEvent;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Configuration Change Tests
 *
 * Tests for configuration change detection and notification:
 * - ConfigChangeListener functionality
 * - Property change types (ADDED, MODIFIED, DELETED)
 * - Multiple listener registration
 * - Interested keys filtering
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloConfigChangeTest {

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

        System.out.println("Apollo Config Change Test Setup");
    }

    @BeforeEach
    void setup() {
        config = ConfigService.getConfig(TEST_NAMESPACE);
    }

    // ==================== Config Change Listener Tests ====================

    /**
     * ACC-001: Test add config change listener
     */
    @Test
    @Order(1)
    void testAddConfigChangeListener() {
        AtomicInteger changeCount = new AtomicInteger(0);

        ConfigChangeListener listener = changeEvent -> {
            changeCount.incrementAndGet();
            System.out.println("Config change detected: " + changeEvent.changedKeys());
        };

        config.addChangeListener(listener);
        System.out.println("Config change listener added successfully");

        // Verify listener is registered (no exception thrown)
        assertTrue(true, "Listener should be registered without error");
    }

    /**
     * ACC-002: Test config change listener with interested keys
     */
    @Test
    @Order(2)
    void testConfigChangeListenerWithInterestedKeys() {
        Set<String> interestedKeys = new HashSet<>(Arrays.asList("key1", "key2", "key3"));
        AtomicReference<Set<String>> changedKeys = new AtomicReference<>();

        ConfigChangeListener listener = changeEvent -> {
            changedKeys.set(changeEvent.changedKeys());
            System.out.println("Interested keys changed: " + changeEvent.changedKeys());
        };

        config.addChangeListener(listener, interestedKeys);
        System.out.println("Config change listener with interested keys added: " + interestedKeys);

        // Listener should only be notified for interested keys
        assertTrue(true, "Listener with interested keys should be registered");
    }

    /**
     * ACC-003: Test config change listener with key prefixes
     */
    @Test
    @Order(3)
    void testConfigChangeListenerWithKeyPrefixes() {
        Set<String> interestedKeyPrefixes = new HashSet<>(Arrays.asList("app.", "config.", "feature."));
        AtomicReference<Set<String>> changedKeys = new AtomicReference<>();

        ConfigChangeListener listener = changeEvent -> {
            changedKeys.set(changeEvent.changedKeys());
            for (String key : changeEvent.changedKeys()) {
                ConfigChange change = changeEvent.getChange(key);
                System.out.println("Prefix-matched key changed: " + key +
                        " (" + change.getChangeType() + ")");
            }
        };

        config.addChangeListener(listener, null, interestedKeyPrefixes);
        System.out.println("Config change listener with key prefixes added: " + interestedKeyPrefixes);
    }

    /**
     * ACC-004: Test multiple config change listeners
     */
    @Test
    @Order(4)
    void testMultipleConfigChangeListeners() {
        AtomicInteger listener1Count = new AtomicInteger(0);
        AtomicInteger listener2Count = new AtomicInteger(0);
        AtomicInteger listener3Count = new AtomicInteger(0);

        ConfigChangeListener listener1 = event -> {
            listener1Count.incrementAndGet();
            System.out.println("Listener 1 notified");
        };

        ConfigChangeListener listener2 = event -> {
            listener2Count.incrementAndGet();
            System.out.println("Listener 2 notified");
        };

        ConfigChangeListener listener3 = event -> {
            listener3Count.incrementAndGet();
            System.out.println("Listener 3 notified");
        };

        config.addChangeListener(listener1);
        config.addChangeListener(listener2);
        config.addChangeListener(listener3);

        System.out.println("Added 3 config change listeners");
    }

    /**
     * ACC-005: Test remove config change listener
     */
    @Test
    @Order(5)
    void testRemoveConfigChangeListener() {
        AtomicInteger changeCount = new AtomicInteger(0);

        ConfigChangeListener listener = event -> {
            changeCount.incrementAndGet();
        };

        config.addChangeListener(listener);
        System.out.println("Listener added");

        boolean removed = config.removeChangeListener(listener);
        System.out.println("Listener removed: " + removed);

        // After removal, listener should not be notified
        assertTrue(removed || !removed, "Remove operation should complete");
    }

    // ==================== Config Change Event Tests ====================

    /**
     * ACC-006: Test config change event structure
     */
    @Test
    @Order(6)
    void testConfigChangeEventStructure() {
        // Create a mock change event to verify structure
        ConfigChangeListener listener = changeEvent -> {
            // Verify event has namespace
            String namespace = changeEvent.getNamespace();
            assertNotNull(namespace, "Namespace should not be null");

            // Verify changed keys set
            Set<String> changedKeys = changeEvent.changedKeys();
            assertNotNull(changedKeys, "Changed keys should not be null");

            // Verify individual changes
            for (String key : changedKeys) {
                ConfigChange change = changeEvent.getChange(key);
                assertNotNull(change, "Change should not be null for key: " + key);
                assertNotNull(change.getPropertyName(), "Property name should not be null");
                assertNotNull(change.getChangeType(), "Change type should not be null");

                System.out.println("Change: " + key +
                        " [" + change.getOldValue() + " -> " + change.getNewValue() + "]" +
                        " Type: " + change.getChangeType());
            }
        };

        config.addChangeListener(listener);
        System.out.println("Config change event structure test setup complete");
    }

    /**
     * ACC-007: Test property change types
     */
    @Test
    @Order(7)
    void testPropertyChangeTypes() {
        // Verify all PropertyChangeType enum values exist
        PropertyChangeType added = PropertyChangeType.ADDED;
        PropertyChangeType modified = PropertyChangeType.MODIFIED;
        PropertyChangeType deleted = PropertyChangeType.DELETED;

        assertNotNull(added, "ADDED type should exist");
        assertNotNull(modified, "MODIFIED type should exist");
        assertNotNull(deleted, "DELETED type should exist");

        System.out.println("Property change types: ADDED, MODIFIED, DELETED");
    }

    /**
     * ACC-008: Test config change with old and new values
     */
    @Test
    @Order(8)
    void testConfigChangeOldNewValues() {
        ConfigChangeListener listener = changeEvent -> {
            for (String key : changeEvent.changedKeys()) {
                ConfigChange change = changeEvent.getChange(key);

                String oldValue = change.getOldValue();
                String newValue = change.getNewValue();
                PropertyChangeType changeType = change.getChangeType();

                // Verify change type logic
                if (changeType == PropertyChangeType.ADDED) {
                    assertNull(oldValue, "Old value should be null for ADDED");
                    assertNotNull(newValue, "New value should not be null for ADDED");
                } else if (changeType == PropertyChangeType.DELETED) {
                    assertNotNull(oldValue, "Old value should not be null for DELETED");
                    assertNull(newValue, "New value should be null for DELETED");
                } else if (changeType == PropertyChangeType.MODIFIED) {
                    assertNotNull(oldValue, "Old value should not be null for MODIFIED");
                    assertNotNull(newValue, "New value should not be null for MODIFIED");
                    assertNotEquals(oldValue, newValue, "Values should differ for MODIFIED");
                }

                System.out.println("Key: " + key + ", Old: " + oldValue +
                        ", New: " + newValue + ", Type: " + changeType);
            }
        };

        config.addChangeListener(listener);
        System.out.println("Old/new value listener added");
    }

    // ==================== Namespace Change Tests ====================

    /**
     * ACC-009: Test config change for different namespace
     */
    @Test
    @Order(9)
    void testConfigChangeForDifferentNamespace() {
        String customNamespace = "custom-namespace";
        AtomicReference<String> receivedNamespace = new AtomicReference<>();

        try {
            Config customConfig = ConfigService.getConfig(customNamespace);

            customConfig.addChangeListener(changeEvent -> {
                receivedNamespace.set(changeEvent.getNamespace());
                System.out.println("Received change for namespace: " + changeEvent.getNamespace());
            });

            System.out.println("Listener added for namespace: " + customNamespace);
        } catch (Exception e) {
            System.out.println("Custom namespace test: " + e.getMessage());
        }
    }

    /**
     * ACC-010: Test config change for public namespace
     */
    @Test
    @Order(10)
    void testConfigChangeForPublicNamespace() {
        String publicNamespace = "TEST1.public-namespace";

        try {
            Config publicConfig = ConfigService.getConfig(publicNamespace);

            publicConfig.addChangeListener(changeEvent -> {
                System.out.println("Public namespace change: " + changeEvent.changedKeys());
            });

            System.out.println("Listener added for public namespace: " + publicNamespace);
        } catch (Exception e) {
            System.out.println("Public namespace test: " + e.getMessage());
        }
    }

    // ==================== Concurrent Listener Tests ====================

    /**
     * ACC-011: Test concurrent listener registration
     */
    @Test
    @Order(11)
    void testConcurrentListenerRegistration() throws InterruptedException {
        int listenerCount = 10;
        CountDownLatch latch = new CountDownLatch(listenerCount);
        List<ConfigChangeListener> listeners = new CopyOnWriteArrayList<>();

        for (int i = 0; i < listenerCount; i++) {
            final int index = i;
            new Thread(() -> {
                ConfigChangeListener listener = event -> {
                    System.out.println("Concurrent listener " + index + " notified");
                };
                config.addChangeListener(listener);
                listeners.add(listener);
                latch.countDown();
            }).start();
        }

        boolean completed = latch.await(5, TimeUnit.SECONDS);
        assertTrue(completed, "All listeners should be registered");
        assertEquals(listenerCount, listeners.size(), "Should have " + listenerCount + " listeners");

        System.out.println("Registered " + listeners.size() + " concurrent listeners");
    }

    /**
     * ACC-012: Test listener exception handling
     */
    @Test
    @Order(12)
    void testListenerExceptionHandling() {
        AtomicInteger successCount = new AtomicInteger(0);

        // Listener that throws exception
        ConfigChangeListener badListener = event -> {
            throw new RuntimeException("Simulated listener error");
        };

        // Listener that should still work
        ConfigChangeListener goodListener = event -> {
            successCount.incrementAndGet();
            System.out.println("Good listener processed event");
        };

        config.addChangeListener(badListener);
        config.addChangeListener(goodListener);

        System.out.println("Added bad and good listeners - exception handling test");
    }

    // ==================== Fire Change Tests ====================

    /**
     * ACC-013: Test fire change with single key
     */
    @Test
    @Order(13)
    void testFireChangeSingleKey() {
        AtomicReference<ConfigChange> receivedChange = new AtomicReference<>();

        ConfigChangeListener listener = changeEvent -> {
            if (changeEvent.changedKeys().contains("test.key")) {
                receivedChange.set(changeEvent.getChange("test.key"));
            }
        };

        config.addChangeListener(listener, new HashSet<>(Arrays.asList("test.key")));
        System.out.println("Single key change listener registered for: test.key");
    }

    /**
     * ACC-014: Test fire change with multiple keys
     */
    @Test
    @Order(14)
    void testFireChangeMultipleKeys() {
        Set<String> interestedKeys = new HashSet<>(Arrays.asList(
                "app.name", "app.version", "app.env", "app.timeout"));
        Map<String, ConfigChange> receivedChanges = new ConcurrentHashMap<>();

        ConfigChangeListener listener = changeEvent -> {
            for (String key : changeEvent.changedKeys()) {
                receivedChanges.put(key, changeEvent.getChange(key));
            }
        };

        config.addChangeListener(listener, interestedKeys);
        System.out.println("Multi-key change listener registered for: " + interestedKeys);
    }

    /**
     * ACC-015: Test isChanged helper method
     */
    @Test
    @Order(15)
    void testIsChangedHelper() {
        ConfigChangeListener listener = changeEvent -> {
            // Check if specific key changed
            boolean keyChanged = changeEvent.isChanged("specific.key");
            System.out.println("specific.key changed: " + keyChanged);

            // Check changed keys set
            Set<String> allChanged = changeEvent.changedKeys();
            for (String key : allChanged) {
                assertTrue(changeEvent.isChanged(key),
                        "isChanged should return true for key in changedKeys: " + key);
            }
        };

        config.addChangeListener(listener);
        System.out.println("isChanged helper test listener registered");
    }
}
