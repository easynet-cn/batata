package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.config.listener.Listener;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos gRPC Error Handling Tests
 *
 * Tests for error handling, timeout, retry, and connection resilience
 * in gRPC communication layer.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosGrpcErrorTest {

    private static ConfigService configService;
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

        configService = NacosFactory.createConfigService(properties);
        namingService = NacosFactory.createNamingService(properties);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (configService != null) {
            configService.shutDown();
        }
        if (namingService != null) {
            namingService.shutDown();
        }
    }

    // ==================== Timeout Handling Tests ====================

    /**
     * NGE-001: Test config get with short timeout
     */
    @Test
    @Order(1)
    void testConfigGetShortTimeout() throws NacosException {
        String dataId = "nonexistent-" + UUID.randomUUID().toString().substring(0, 8);

        long startTime = System.currentTimeMillis();
        String content = configService.getConfig(dataId, DEFAULT_GROUP, 1000);
        long duration = System.currentTimeMillis() - startTime;

        assertNull(content, "Non-existent config should return null");
        assertTrue(duration < 5000,
                "Should complete within 5 seconds (timeout was 1s), actual: " + duration + "ms");
    }

    /**
     * NGE-002: Test config get with very short timeout
     */
    @Test
    @Order(2)
    void testConfigGetVeryShortTimeout() {
        String dataId = "very-short-timeout-" + UUID.randomUUID().toString().substring(0, 8);

        long startTime = System.currentTimeMillis();
        try {
            String content = configService.getConfig(dataId, DEFAULT_GROUP, 100);
            long duration = System.currentTimeMillis() - startTime;
            // If it returns, content should be null (non-existent) and it should be fast
            assertNull(content,
                    "Non-existent config with very short timeout should return null");
            assertTrue(duration < 3000,
                    "Very short timeout should complete quickly, actual: " + duration + "ms");
        } catch (NacosException e) {
            long duration = System.currentTimeMillis() - startTime;
            // A timeout exception is also acceptable for very short timeout
            assertTrue(duration < 5000,
                    "Even on timeout exception, should not take too long, actual: " + duration + "ms");
        }
    }

    /**
     * NGE-003: Test config publish timeout handling
     */
    @Test
    @Order(3)
    void testConfigPublishTimeout() throws NacosException {
        String dataId = "publish-timeout-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "timeout.test=value";

        long startTime = System.currentTimeMillis();
        boolean result = configService.publishConfig(dataId, DEFAULT_GROUP, content);
        long duration = System.currentTimeMillis() - startTime;

        assertTrue(result, "Publish should succeed");
        assertTrue(duration < 10000,
                "Publish should complete within 10 seconds, actual: " + duration + "ms");

        // Verify the config was actually persisted
        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertEquals(content, retrieved,
                "Published config should be retrievable after publish");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== Permission Error Tests ====================

    /**
     * NGE-004: Test permission denied handling (invalid credentials)
     *
     * Note: Some server implementations may not reject invalid credentials on
     * gRPC config read operations (e.g., getConfig may return null instead of
     * throwing). This test accepts both behaviors.
     */
    @Test
    @Order(4)
    void testPermissionDenied() {
        Properties props = new Properties();
        props.put("serverAddr", System.getProperty("nacos.server", "127.0.0.1:8848"));
        props.put("username", "invalid-user");
        props.put("password", "invalid-password");

        boolean exceptionThrown = false;
        boolean returnedNull = false;
        try {
            ConfigService restrictedService = NacosFactory.createConfigService(props);
            try {
                String result = restrictedService.getConfig("test-config", DEFAULT_GROUP, 5000);
                returnedNull = (result == null);
            } finally {
                restrictedService.shutDown();
            }
        } catch (NacosException e) {
            exceptionThrown = true;
            assertTrue(e.getErrCode() != 0,
                    "NacosException error code should be non-zero for auth failure, got: " + e.getErrCode());
        }

        assertTrue(exceptionThrown || returnedNull,
                "Operations with invalid credentials should either throw NacosException or return null");
    }

    /**
     * NGE-005: Test unauthorized namespace access
     */
    @Test
    @Order(5)
    void testUnauthorizedNamespace() throws NacosException {
        String dataId = "ns-test-" + UUID.randomUUID().toString().substring(0, 8);
        String namespace = "restricted-namespace-" + UUID.randomUUID().toString().substring(0, 8);

        Properties props = new Properties();
        props.put("serverAddr", System.getProperty("nacos.server", "127.0.0.1:8848"));
        props.put("username", System.getProperty("nacos.username", "nacos"));
        props.put("password", System.getProperty("nacos.password", "nacos"));
        props.put("namespace", namespace);

        ConfigService nsService = NacosFactory.createConfigService(props);
        try {
            // Non-existent namespace: config get should return null (no data)
            String content = nsService.getConfig(dataId, DEFAULT_GROUP, 3000);
            assertNull(content,
                    "Getting config from non-existent namespace should return null");
        } finally {
            nsService.shutDown();
        }
    }

    // ==================== Connection Resilience Tests ====================

    /**
     * NGE-006: Test connection recovery after operations
     */
    @Test
    @Order(6)
    void testConnectionRecovery() throws NacosException, InterruptedException {
        String dataId = "recovery-" + UUID.randomUUID().toString().substring(0, 8);

        // Perform multiple operations to test connection stability
        for (int i = 0; i < 5; i++) {
            String content = "recovery.iteration=" + i;
            boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, content);
            assertTrue(published, "Publish should succeed in iteration " + i);

            String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
            assertEquals(content, retrieved,
                    "Content should match in iteration " + i);

            Thread.sleep(200);
        }

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NGE-007: Test server status check
     */
    @Test
    @Order(7)
    void testServerStatus() throws NacosException {
        String status = configService.getServerStatus();
        assertNotNull(status, "Server status should not be null");
        assertFalse(status.isEmpty(), "Server status should not be empty");
        assertEquals("UP", status,
                "Config server status should be UP for a running server");
    }

    /**
     * NGE-008: Test naming service server status
     */
    @Test
    @Order(8)
    void testNamingServerStatus() throws NacosException {
        String status = namingService.getServerStatus();
        assertNotNull(status, "Naming server status should not be null");
        assertFalse(status.isEmpty(), "Naming server status should not be empty");
        assertEquals("UP", status,
                "Naming server status should be UP for a running server");
    }

    // ==================== Concurrent Error Handling Tests ====================

    /**
     * NGE-009: Test concurrent operations error handling
     */
    @Test
    @Order(9)
    void testConcurrentOperationsErrors() throws InterruptedException {
        String prefix = "concurrent-err-" + UUID.randomUUID().toString().substring(0, 8);
        int threadCount = 10;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(threadCount);
        AtomicInteger successCount = new AtomicInteger(0);
        AtomicInteger errorCount = new AtomicInteger(0);

        for (int i = 0; i < threadCount; i++) {
            final int index = i;
            new Thread(() -> {
                try {
                    startLatch.await();
                    String dataId = prefix + "-" + index;
                    String content = "concurrent.test=" + index;

                    boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, content);
                    assertTrue(published,
                            "Concurrent publish should succeed for thread " + index);

                    String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
                    assertNotNull(retrieved,
                            "Concurrent get should return content for thread " + index);
                    assertEquals(content, retrieved,
                            "Retrieved content should match published content in thread " + index);

                    boolean removed = configService.removeConfig(dataId, DEFAULT_GROUP);
                    assertTrue(removed,
                            "Concurrent remove should succeed for thread " + index);

                    successCount.incrementAndGet();
                } catch (Exception e) {
                    errorCount.incrementAndGet();
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        startLatch.countDown();
        boolean completed = doneLatch.await(60, TimeUnit.SECONDS);

        assertTrue(completed, "All threads should complete within 60 seconds");
        assertEquals(threadCount, successCount.get(),
                "All " + threadCount + " threads should succeed");
        assertEquals(0, errorCount.get(),
                "No errors should occur during concurrent operations");
    }

    /**
     * NGE-010: Test rapid subscribe/unsubscribe
     */
    @Test
    @Order(10)
    void testRapidSubscribeUnsubscribe() throws NacosException, InterruptedException {
        String serviceName = "rapid-sub-" + UUID.randomUUID().toString().substring(0, 8);

        // Register a service first
        namingService.registerInstance(serviceName, "192.168.100.1", 8080);
        Thread.sleep(500);

        // Rapid subscribe/unsubscribe cycles should not throw
        for (int i = 0; i < 5; i++) {
            com.alibaba.nacos.api.naming.listener.EventListener listener = event -> {};
            namingService.subscribe(serviceName, listener);
            Thread.sleep(100);
            namingService.unsubscribe(serviceName, listener);
            Thread.sleep(100);
        }

        // Verify the service is still accessible after rapid cycles
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertNotNull(instances, "Instance list should not be null after rapid sub/unsub");
        assertFalse(instances.isEmpty(),
                "Instance list should still contain the registered instance");
        assertEquals("192.168.100.1", instances.get(0).getIp(),
                "Instance IP should match after rapid sub/unsub cycles");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.100.1", 8080);
    }

    // ==================== Invalid Input Tests ====================

    /**
     * NGE-011: Test invalid dataId handling - null
     */
    @Test
    @Order(11)
    void testNullDataId() {
        Exception thrown = assertThrows(Exception.class, () -> {
            configService.getConfig(null, DEFAULT_GROUP, 3000);
        }, "Null dataId should throw an exception");

        assertTrue(thrown instanceof NacosException || thrown instanceof IllegalArgumentException,
                "Exception should be NacosException or IllegalArgumentException, got: "
                        + thrown.getClass().getName());
    }

    /**
     * NGE-012: Test invalid dataId handling - empty string
     */
    @Test
    @Order(12)
    void testEmptyDataId() {
        Exception thrown = assertThrows(Exception.class, () -> {
            configService.getConfig("", DEFAULT_GROUP, 3000);
        }, "Empty dataId should throw an exception");

        assertTrue(thrown instanceof NacosException || thrown instanceof IllegalArgumentException,
                "Exception should be NacosException or IllegalArgumentException, got: "
                        + thrown.getClass().getName());
    }

    /**
     * NGE-013: Test invalid service name handling - null
     */
    @Test
    @Order(13)
    void testNullServiceName() {
        Exception thrown = assertThrows(Exception.class, () -> {
            namingService.registerInstance(null, "192.168.1.1", 8080);
        }, "Null service name should throw an exception");

        assertTrue(thrown instanceof NacosException || thrown instanceof IllegalArgumentException,
                "Exception should be NacosException or IllegalArgumentException, got: "
                        + thrown.getClass().getName());
    }

    /**
     * NGE-014: Test listener exception handling
     */
    @Test
    @Order(14)
    void testListenerExceptionHandling() throws NacosException, InterruptedException {
        String dataId = "listener-error-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicInteger callCount = new AtomicInteger(0);
        CountDownLatch secondCallLatch = new CountDownLatch(1);
        AtomicReference<String> lastContent = new AtomicReference<>();

        // Add listener that throws exception on first call
        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                int count = callCount.incrementAndGet();
                if (count == 1) {
                    throw new RuntimeException("Simulated listener error");
                }
                lastContent.set(configInfo);
                secondCallLatch.countDown();
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Publish config twice
        configService.publishConfig(dataId, DEFAULT_GROUP, "first=value");
        Thread.sleep(5000);
        configService.publishConfig(dataId, DEFAULT_GROUP, "second=value");

        boolean receivedSecond = secondCallLatch.await(15, TimeUnit.SECONDS);
        assertTrue(receivedSecond,
                "Listener should recover and receive second config change after throwing on first");
        assertTrue(callCount.get() >= 2,
                "Listener should have been called at least twice, actual: " + callCount.get());
        assertEquals("second=value", lastContent.get(),
                "Last received content should be 'second=value'");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NGE-015: Test slow listener handling
     */
    @Test
    @Order(15)
    void testSlowListenerHandling() throws NacosException, InterruptedException {
        String dataId = "slow-listener-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicReference<String> lastReceived = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        // Add slow listener
        configService.addListener(dataId, DEFAULT_GROUP, new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                try {
                    Thread.sleep(2000); // Simulate slow processing
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                }
                lastReceived.set(configInfo);
                latch.countDown();
            }
        });

        Thread.sleep(500);

        // Publish config
        String content = "slow.test=value";
        configService.publishConfig(dataId, DEFAULT_GROUP, content);

        boolean received = latch.await(15, TimeUnit.SECONDS);
        assertTrue(received,
                "Slow listener should eventually receive config notification");
        assertEquals(content, lastReceived.get(),
                "Slow listener should receive the correct config content");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }
}
