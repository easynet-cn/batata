package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.config.listener.Listener;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Config Listener Tests
 *
 * Tests for configuration change listener functionality:
 * - Add/remove listeners
 * - Multiple listeners per config
 * - Listener notification timing
 * - Concurrent listener operations
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigListenerTest {

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

        configService = NacosFactory.createConfigService(properties);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (configService != null) {
            configService.shutDown();
        }
    }

    // ==================== Basic Listener Tests ====================

    /**
     * NCL-001: Test add config listener
     */
    @Test
    @Order(1)
    void testAddConfigListener() throws NacosException, InterruptedException {
        String dataId = "listener-test-" + UUID.randomUUID().toString().substring(0, 8);
        String expectedContent = "test.value=1";
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<String> receivedConfig = new AtomicReference<>();

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                receivedConfig.set(configInfo);
                latch.countDown();
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);

        // Publish config
        Thread.sleep(500);
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, expectedContent);
        assertTrue(published, "Config should be published successfully");

        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "Listener should receive config update within timeout");
        assertEquals(expectedContent, receivedConfig.get(),
                "Listener should receive the exact config content that was published");

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCL-002: Test remove config listener
     */
    @Test
    @Order(2)
    void testRemoveConfigListener() throws NacosException, InterruptedException {
        String dataId = "remove-listener-" + UUID.randomUUID().toString().substring(0, 8);
        CountDownLatch firstLatch = new CountDownLatch(1);
        AtomicInteger callCount = new AtomicInteger(0);

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                callCount.incrementAndGet();
                firstLatch.countDown();
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Publish first config and wait for listener to fire
        configService.publishConfig(dataId, DEFAULT_GROUP, "first=value");
        boolean firstReceived = firstLatch.await(10, TimeUnit.SECONDS);
        assertTrue(firstReceived, "Listener should receive the first config update");

        int countAfterFirst = callCount.get();
        assertTrue(countAfterFirst >= 1, "Listener should have been called at least once after first publish");

        // Remove listener
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Publish second config - should not trigger listener
        configService.publishConfig(dataId, DEFAULT_GROUP, "second=value");
        Thread.sleep(3000);

        int countAfterSecond = callCount.get();
        assertEquals(countAfterFirst, countAfterSecond,
                "Listener call count should not increase after removal; was " + countAfterFirst + " now " + countAfterSecond);

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCL-003: Test multiple listeners on same config
     */
    @Test
    @Order(3)
    void testMultipleListeners() throws NacosException, InterruptedException {
        String dataId = "multi-listener-" + UUID.randomUUID().toString().substring(0, 8);
        String expectedContent = "multi.test=value";
        int listenerCount = 3;
        CountDownLatch latch = new CountDownLatch(listenerCount);
        List<AtomicReference<String>> receivedValues = new ArrayList<>();
        List<Listener> listeners = new ArrayList<>();

        for (int i = 0; i < listenerCount; i++) {
            AtomicReference<String> ref = new AtomicReference<>();
            receivedValues.add(ref);
            Listener listener = new Listener() {
                @Override
                public Executor getExecutor() {
                    return null;
                }

                @Override
                public void receiveConfigInfo(String configInfo) {
                    ref.set(configInfo);
                    latch.countDown();
                }
            };
            listeners.add(listener);
            configService.addListener(dataId, DEFAULT_GROUP, listener);
        }

        Thread.sleep(500);

        // Publish config
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, expectedContent);
        assertTrue(published, "Config should be published successfully");

        boolean allReceived = latch.await(15, TimeUnit.SECONDS);
        assertTrue(allReceived, "All " + listenerCount + " listeners should receive the config update");

        for (int i = 0; i < listenerCount; i++) {
            assertEquals(expectedContent, receivedValues.get(i).get(),
                    "Listener " + i + " should have received the correct config content");
        }

        // Cleanup
        for (Listener listener : listeners) {
            configService.removeListener(dataId, DEFAULT_GROUP, listener);
        }
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCL-004: Test listener with custom executor
     */
    @Test
    @Order(4)
    void testListenerWithCustomExecutor() throws NacosException, InterruptedException {
        String dataId = "executor-listener-" + UUID.randomUUID().toString().substring(0, 8);
        String expectedContent = "executor.test=value";
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<String> executorThread = new AtomicReference<>();
        AtomicReference<String> receivedConfig = new AtomicReference<>();
        ExecutorService customExecutor = Executors.newSingleThreadExecutor(r -> {
            Thread t = new Thread(r);
            t.setName("custom-listener-executor");
            return t;
        });

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return customExecutor;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                executorThread.set(Thread.currentThread().getName());
                receivedConfig.set(configInfo);
                latch.countDown();
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, expectedContent);
        assertTrue(published, "Config should be published successfully");

        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "Listener with custom executor should receive config update");
        assertEquals(expectedContent, receivedConfig.get(),
                "Listener should receive the correct config content");
        assertEquals("custom-listener-executor", executorThread.get(),
                "Listener callback should execute on the custom executor thread");

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        configService.removeConfig(dataId, DEFAULT_GROUP);
        customExecutor.shutdown();
    }

    // ==================== Listener Update Tests ====================

    /**
     * NCL-005: Test listener receives multiple updates
     */
    @Test
    @Order(5)
    void testListenerMultipleUpdates() throws NacosException, InterruptedException {
        String dataId = "updates-listener-" + UUID.randomUUID().toString().substring(0, 8);
        int updateCount = 5;
        CountDownLatch latch = new CountDownLatch(updateCount);
        List<String> receivedConfigs = new CopyOnWriteArrayList<>();

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                receivedConfigs.add(configInfo);
                latch.countDown();
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Publish multiple updates with enough delay for each to be detected
        for (int i = 0; i < updateCount; i++) {
            boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "update=" + i);
            assertTrue(published, "Config update " + i + " should be published successfully");
            Thread.sleep(1500);
        }

        boolean allReceived = latch.await(30, TimeUnit.SECONDS);
        assertTrue(allReceived,
                "All " + updateCount + " updates should be received, but got " + receivedConfigs.size());
        assertEquals(updateCount, receivedConfigs.size(),
                "Should have received exactly " + updateCount + " config notifications");

        // Verify the last received config matches the last published value
        String lastReceived = receivedConfigs.get(receivedConfigs.size() - 1);
        assertEquals("update=" + (updateCount - 1), lastReceived,
                "The last received config should match the last published value");

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCL-006: Test listener receives config delete notification
     */
    @Test
    @Order(6)
    void testListenerConfigDelete() throws NacosException, InterruptedException {
        String dataId = "delete-listener-" + UUID.randomUUID().toString().substring(0, 8);
        String configContent = "to.be.deleted=true";
        CountDownLatch createLatch = new CountDownLatch(1);
        CountDownLatch deleteLatch = new CountDownLatch(1);
        AtomicReference<String> lastConfig = new AtomicReference<>();
        AtomicReference<String> deleteNotification = new AtomicReference<>();

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                String previous = lastConfig.getAndSet(configInfo);
                if (previous == null) {
                    createLatch.countDown();
                } else if (configInfo == null || configInfo.isEmpty()) {
                    deleteNotification.set(configInfo);
                    deleteLatch.countDown();
                }
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Create config
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, configContent);
        assertTrue(published, "Config should be published successfully");

        boolean createReceived = createLatch.await(10, TimeUnit.SECONDS);
        assertTrue(createReceived, "Listener should receive the create notification");
        assertEquals(configContent, lastConfig.get(),
                "Listener should receive the exact published content on create");

        Thread.sleep(1000);

        // Delete config
        boolean deleted = configService.removeConfig(dataId, DEFAULT_GROUP);
        assertTrue(deleted, "Config should be removed successfully");

        boolean deleteReceived = deleteLatch.await(10, TimeUnit.SECONDS);
        assertTrue(deleteReceived, "Listener should receive delete notification");
        String deleteValue = deleteNotification.get();
        assertTrue(deleteValue == null || deleteValue.isEmpty(),
                "Delete notification should have null or empty content, got: " + deleteValue);

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
    }

    // ==================== Namespace Listener Tests ====================

    /**
     * NCL-007: Test listener in custom namespace
     */
    @Test
    @Order(7)
    void testListenerInNamespace() throws NacosException, InterruptedException {
        String namespace = "test-ns-" + UUID.randomUUID().toString().substring(0, 8);
        String dataId = "ns-listener-" + UUID.randomUUID().toString().substring(0, 8);
        String expectedContent = "namespace.config=value";

        // Create config service for namespace
        Properties props = new Properties();
        props.put("serverAddr", System.getProperty("nacos.server", "127.0.0.1:8848"));
        props.put("username", System.getProperty("nacos.username", "nacos"));
        props.put("password", System.getProperty("nacos.password", "nacos"));
        props.put("namespace", namespace);

        ConfigService nsConfigService = NacosFactory.createConfigService(props);
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<String> receivedConfig = new AtomicReference<>();

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                receivedConfig.set(configInfo);
                latch.countDown();
            }
        };

        try {
            nsConfigService.addListener(dataId, DEFAULT_GROUP, listener);
            Thread.sleep(500);

            boolean published = nsConfigService.publishConfig(dataId, DEFAULT_GROUP, expectedContent);
            assertTrue(published, "Config should be published in namespace successfully");

            boolean received = latch.await(10, TimeUnit.SECONDS);
            assertTrue(received, "Namespace listener should receive update within timeout");
            assertEquals(expectedContent, receivedConfig.get(),
                    "Namespace listener should receive the correct config content");

            // Verify config in default namespace is not affected
            String defaultNsConfig = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
            assertNull(defaultNsConfig,
                    "Config published in custom namespace should not be visible in default namespace");
        } finally {
            // Cleanup
            nsConfigService.removeListener(dataId, DEFAULT_GROUP, listener);
            nsConfigService.removeConfig(dataId, DEFAULT_GROUP);
            nsConfigService.shutDown();
        }
    }

    /**
     * NCL-008: Test listener in custom group
     */
    @Test
    @Order(8)
    void testListenerInCustomGroup() throws NacosException, InterruptedException {
        String dataId = "group-listener-" + UUID.randomUUID().toString().substring(0, 8);
        String group = "CUSTOM_GROUP";
        String expectedContent = "custom.group.config=value";
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<String> receivedConfig = new AtomicReference<>();

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                receivedConfig.set(configInfo);
                latch.countDown();
            }
        };

        configService.addListener(dataId, group, listener);
        Thread.sleep(500);

        boolean published = configService.publishConfig(dataId, group, expectedContent);
        assertTrue(published, "Config should be published to custom group successfully");

        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "Custom group listener should receive update within timeout");
        assertEquals(expectedContent, receivedConfig.get(),
                "Custom group listener should receive the correct config content");

        // Verify config in DEFAULT_GROUP is not affected
        String defaultGroupConfig = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertNull(defaultGroupConfig,
                "Config published in custom group should not be visible in DEFAULT_GROUP");

        // Cleanup
        configService.removeListener(dataId, group, listener);
        configService.removeConfig(dataId, group);
    }

    // ==================== Concurrent Listener Tests ====================

    /**
     * NCL-009: Test concurrent listener add/remove
     */
    @Test
    @Order(9)
    void testConcurrentListenerOperations() throws NacosException, InterruptedException {
        String dataId = "concurrent-listener-" + UUID.randomUUID().toString().substring(0, 8);
        int threadCount = 10;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(threadCount);
        List<Listener> listeners = new CopyOnWriteArrayList<>();
        AtomicInteger errorCount = new AtomicInteger(0);

        for (int i = 0; i < threadCount; i++) {
            final int index = i;
            new Thread(() -> {
                try {
                    startLatch.await();

                    Listener listener = new Listener() {
                        @Override
                        public Executor getExecutor() {
                            return null;
                        }

                        @Override
                        public void receiveConfigInfo(String configInfo) {
                            // no-op
                        }
                    };

                    configService.addListener(dataId, DEFAULT_GROUP, listener);
                    listeners.add(listener);

                    Thread.sleep(100);

                    // Remove half the listeners
                    if (index % 2 == 0) {
                        configService.removeListener(dataId, DEFAULT_GROUP, listener);
                    }
                } catch (Exception e) {
                    errorCount.incrementAndGet();
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        startLatch.countDown();
        boolean completed = doneLatch.await(30, TimeUnit.SECONDS);

        assertTrue(completed, "All concurrent listener operations should complete within timeout");
        assertEquals(0, errorCount.get(),
                "No errors should occur during concurrent listener add/remove operations");
        assertEquals(threadCount, listeners.size(),
                "All " + threadCount + " listeners should have been added before any removal");

        // Cleanup
        for (Listener listener : listeners) {
            try {
                configService.removeListener(dataId, DEFAULT_GROUP, listener);
            } catch (Exception e) {
                // Ignore cleanup errors for already-removed listeners
            }
        }
    }

    /**
     * NCL-010: Test listener under rapid config changes
     */
    @Test
    @Order(10)
    void testListenerRapidChanges() throws NacosException, InterruptedException {
        String dataId = "rapid-changes-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicInteger receiveCount = new AtomicInteger(0);
        AtomicReference<String> lastReceived = new AtomicReference<>();
        int changeCount = 10;

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                receiveCount.incrementAndGet();
                lastReceived.set(configInfo);
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Rapid config changes
        for (int i = 0; i < changeCount; i++) {
            boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "rapid.change=" + i);
            assertTrue(published, "Rapid config change " + i + " should be published successfully");
            Thread.sleep(500); // Delay between changes to allow each to be detected
        }

        // Wait for notifications to settle
        Thread.sleep(5000);

        int received = receiveCount.get();
        assertTrue(received >= 1,
                "Listener should receive at least 1 notification under rapid changes, got " + received);
        // The last received config should be the final value (some intermediate ones may be coalesced)
        assertEquals("rapid.change=" + (changeCount - 1), lastReceived.get(),
                "The last received config should match the final published value");

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== Edge Cases ====================

    /**
     * NCL-011: Test listener for non-existent config
     */
    @Test
    @Order(11)
    void testListenerForNonExistentConfig() throws NacosException, InterruptedException {
        String dataId = "nonexistent-" + UUID.randomUUID().toString().substring(0, 8);
        String expectedContent = "now.exists=true";
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<String> receivedConfig = new AtomicReference<>();

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                receivedConfig.set(configInfo);
                latch.countDown();
            }
        };

        // Add listener for non-existent config - should not throw
        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Verify config does not exist yet
        String current = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertNull(current, "Config should not exist before publishing");

        // Now create the config
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, expectedContent);
        assertTrue(published, "Config should be published successfully");

        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "Listener should be notified when a previously non-existent config is created");
        assertEquals(expectedContent, receivedConfig.get(),
                "Listener should receive the correct content for newly created config");

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCL-012: Test duplicate listener add
     */
    @Test
    @Order(12)
    void testDuplicateListenerAdd() throws NacosException, InterruptedException {
        String dataId = "duplicate-listener-" + UUID.randomUUID().toString().substring(0, 8);
        String expectedContent = "duplicate.test=value";
        CountDownLatch latch = new CountDownLatch(1);
        AtomicInteger callCount = new AtomicInteger(0);

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                callCount.incrementAndGet();
                latch.countDown();
            }
        };

        // Add same listener multiple times
        configService.addListener(dataId, DEFAULT_GROUP, listener);
        configService.addListener(dataId, DEFAULT_GROUP, listener);
        configService.addListener(dataId, DEFAULT_GROUP, listener);

        Thread.sleep(500);

        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, expectedContent);
        assertTrue(published, "Config should be published successfully");

        latch.await(10, TimeUnit.SECONDS);
        Thread.sleep(3000);

        // Duplicate adds of the same listener instance should result in only one callback
        assertEquals(1, callCount.get(),
                "Adding the same listener instance multiple times should only trigger one callback, got " + callCount.get());

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCL-013: Test large config content notification
     */
    @Test
    @Order(13)
    void testLargeConfigNotification() throws NacosException, InterruptedException {
        String dataId = "large-config-" + UUID.randomUUID().toString().substring(0, 8);
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<String> receivedContent = new AtomicReference<>();

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                receivedContent.set(configInfo);
                latch.countDown();
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Create large config (~100KB)
        StringBuilder largeContent = new StringBuilder();
        for (int i = 0; i < 10000; i++) {
            largeContent.append("property").append(i).append("=value").append(i).append("\n");
        }
        String expectedContent = largeContent.toString();
        int expectedSize = expectedContent.length();

        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, expectedContent);
        assertTrue(published, "Large config should be published successfully");

        boolean received = latch.await(15, TimeUnit.SECONDS);
        assertTrue(received, "Listener should receive large config update within timeout");
        assertNotNull(receivedContent.get(), "Received content should not be null");
        assertEquals(expectedSize, receivedContent.get().length(),
                "Received config size should match published config size");
        assertTrue(receivedContent.get().contains("property0=value0"),
                "Received large config should contain the first property");
        assertTrue(receivedContent.get().contains("property9999=value9999"),
                "Received large config should contain the last property");

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }
}
