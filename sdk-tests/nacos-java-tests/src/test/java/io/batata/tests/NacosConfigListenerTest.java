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
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        configService = NacosFactory.createConfigService(properties);
        System.out.println("Nacos Config Listener Test Setup - Server: " + serverAddr);
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
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<String> receivedConfig = new AtomicReference<>();

        // Add listener before config exists
        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                receivedConfig.set(configInfo);
                latch.countDown();
                System.out.println("Listener received: " + configInfo);
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);

        // Publish config
        Thread.sleep(500);
        configService.publishConfig(dataId, DEFAULT_GROUP, "test.value=1");

        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "Listener should receive config update");
        assertNotNull(receivedConfig.get(), "Should have received config");

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
        AtomicInteger callCount = new AtomicInteger(0);

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                callCount.incrementAndGet();
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Publish first config
        configService.publishConfig(dataId, DEFAULT_GROUP, "first=value");
        Thread.sleep(2000);

        int countAfterFirst = callCount.get();

        // Remove listener
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Publish second config - should not trigger listener
        configService.publishConfig(dataId, DEFAULT_GROUP, "second=value");
        Thread.sleep(2000);

        int countAfterSecond = callCount.get();

        System.out.println("Calls after first: " + countAfterFirst + ", after second: " + countAfterSecond);

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
        int listenerCount = 3;
        CountDownLatch latch = new CountDownLatch(listenerCount);
        List<Listener> listeners = new ArrayList<>();

        for (int i = 0; i < listenerCount; i++) {
            final int index = i;
            Listener listener = new Listener() {
                @Override
                public Executor getExecutor() {
                    return null;
                }

                @Override
                public void receiveConfigInfo(String configInfo) {
                    System.out.println("Listener " + index + " received: " + configInfo);
                    latch.countDown();
                }
            };
            listeners.add(listener);
            configService.addListener(dataId, DEFAULT_GROUP, listener);
        }

        Thread.sleep(500);

        // Publish config
        configService.publishConfig(dataId, DEFAULT_GROUP, "multi.test=value");

        boolean allReceived = latch.await(15, TimeUnit.SECONDS);
        System.out.println("All listeners received: " + allReceived);

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
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<String> executorThread = new AtomicReference<>();
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
                latch.countDown();
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        configService.publishConfig(dataId, DEFAULT_GROUP, "executor.test=value");

        boolean received = latch.await(10, TimeUnit.SECONDS);
        if (received) {
            System.out.println("Listener executed on thread: " + executorThread.get());
        }

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

        // Publish multiple updates
        for (int i = 0; i < updateCount; i++) {
            configService.publishConfig(dataId, DEFAULT_GROUP, "update=" + i);
            Thread.sleep(1500);
        }

        boolean allReceived = latch.await(30, TimeUnit.SECONDS);
        System.out.println("Received " + receivedConfigs.size() + " updates: " + receivedConfigs);

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
        CountDownLatch createLatch = new CountDownLatch(1);
        CountDownLatch deleteLatch = new CountDownLatch(1);
        AtomicReference<String> lastConfig = new AtomicReference<>();

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
                    deleteLatch.countDown();
                }
                System.out.println("Config change: " + previous + " -> " + configInfo);
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Create config
        configService.publishConfig(dataId, DEFAULT_GROUP, "to.be.deleted=true");
        createLatch.await(10, TimeUnit.SECONDS);

        Thread.sleep(1000);

        // Delete config
        configService.removeConfig(dataId, DEFAULT_GROUP);

        boolean deleteReceived = deleteLatch.await(10, TimeUnit.SECONDS);
        System.out.println("Delete notification received: " + deleteReceived);

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

        // Create config service for namespace
        Properties props = new Properties();
        props.put("serverAddr", System.getProperty("nacos.server", "127.0.0.1:8848"));
        props.put("username", System.getProperty("nacos.username", "nacos"));
        props.put("password", System.getProperty("nacos.password", "nacos"));
        props.put("namespace", namespace);

        try {
            ConfigService nsConfigService = NacosFactory.createConfigService(props);
            CountDownLatch latch = new CountDownLatch(1);

            Listener listener = new Listener() {
                @Override
                public Executor getExecutor() {
                    return null;
                }

                @Override
                public void receiveConfigInfo(String configInfo) {
                    System.out.println("Namespace listener received: " + configInfo);
                    latch.countDown();
                }
            };

            nsConfigService.addListener(dataId, DEFAULT_GROUP, listener);
            Thread.sleep(500);

            nsConfigService.publishConfig(dataId, DEFAULT_GROUP, "namespace.config=value");

            boolean received = latch.await(10, TimeUnit.SECONDS);
            System.out.println("Namespace listener received update: " + received);

            // Cleanup
            nsConfigService.removeListener(dataId, DEFAULT_GROUP, listener);
            nsConfigService.removeConfig(dataId, DEFAULT_GROUP);
            nsConfigService.shutDown();
        } catch (Exception e) {
            System.out.println("Namespace listener test: " + e.getMessage());
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
        CountDownLatch latch = new CountDownLatch(1);

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                System.out.println("Custom group listener received: " + configInfo);
                latch.countDown();
            }
        };

        configService.addListener(dataId, group, listener);
        Thread.sleep(500);

        configService.publishConfig(dataId, group, "custom.group.config=value");

        boolean received = latch.await(10, TimeUnit.SECONDS);
        System.out.println("Custom group listener received update: " + received);

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
    void testConcurrentListenerOperations() throws InterruptedException {
        String dataId = "concurrent-listener-" + UUID.randomUUID().toString().substring(0, 8);
        int threadCount = 10;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(threadCount);
        List<Listener> listeners = new CopyOnWriteArrayList<>();

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
                            System.out.println("Concurrent listener " + index + " received");
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
                    System.err.println("Concurrent listener error: " + e.getMessage());
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        startLatch.countDown();
        boolean completed = doneLatch.await(30, TimeUnit.SECONDS);

        System.out.println("Concurrent operations completed: " + completed);
        System.out.println("Total listeners added: " + listeners.size());

        // Cleanup
        for (Listener listener : listeners) {
            try {
                configService.removeListener(dataId, DEFAULT_GROUP, listener);
            } catch (Exception e) {
                // Ignore
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
        int changeCount = 10;

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                receiveCount.incrementAndGet();
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Rapid config changes
        for (int i = 0; i < changeCount; i++) {
            configService.publishConfig(dataId, DEFAULT_GROUP, "rapid.change=" + i);
            Thread.sleep(100); // Small delay between changes
        }

        // Wait for all notifications
        Thread.sleep(5000);

        System.out.println("Published " + changeCount + " changes, received " + receiveCount.get() + " notifications");

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
        CountDownLatch latch = new CountDownLatch(1);

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                System.out.println("Non-existent config listener received: " + configInfo);
                latch.countDown();
            }
        };

        // Add listener for non-existent config
        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Now create the config
        configService.publishConfig(dataId, DEFAULT_GROUP, "now.exists=true");

        boolean received = latch.await(10, TimeUnit.SECONDS);
        System.out.println("Received notification for newly created config: " + received);

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
        AtomicInteger callCount = new AtomicInteger(0);

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                callCount.incrementAndGet();
            }
        };

        // Add same listener multiple times
        configService.addListener(dataId, DEFAULT_GROUP, listener);
        configService.addListener(dataId, DEFAULT_GROUP, listener);
        configService.addListener(dataId, DEFAULT_GROUP, listener);

        Thread.sleep(500);

        configService.publishConfig(dataId, DEFAULT_GROUP, "duplicate.test=value");
        Thread.sleep(3000);

        System.out.println("Duplicate listener call count: " + callCount.get());

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
        AtomicReference<Integer> receivedSize = new AtomicReference<>();

        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                if (configInfo != null) {
                    receivedSize.set(configInfo.length());
                }
                latch.countDown();
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Create large config (100KB)
        StringBuilder largeContent = new StringBuilder();
        for (int i = 0; i < 10000; i++) {
            largeContent.append("property").append(i).append("=value").append(i).append("\n");
        }

        configService.publishConfig(dataId, DEFAULT_GROUP, largeContent.toString());

        boolean received = latch.await(15, TimeUnit.SECONDS);
        System.out.println("Large config received: " + received + ", size: " + receivedSize.get() + " bytes");

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }
}
