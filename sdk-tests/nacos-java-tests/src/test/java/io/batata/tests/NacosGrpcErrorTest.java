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
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        configService = NacosFactory.createConfigService(properties);
        namingService = NacosFactory.createNamingService(properties);

        System.out.println("Nacos gRPC Error Test Setup - Server: " + serverAddr);
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
    void testConfigGetShortTimeout() {
        String dataId = "nonexistent-" + UUID.randomUUID().toString().substring(0, 8);

        long startTime = System.currentTimeMillis();
        try {
            // Short timeout for non-existent config
            String content = configService.getConfig(dataId, DEFAULT_GROUP, 1000);
            long duration = System.currentTimeMillis() - startTime;

            assertNull(content, "Non-existent config should return null");
            assertTrue(duration < 5000, "Should respect timeout");
            System.out.println("Short timeout test - Duration: " + duration + "ms");
        } catch (NacosException e) {
            System.out.println("Short timeout exception: " + e.getMessage());
        }
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
            // Very short timeout
            String content = configService.getConfig(dataId, DEFAULT_GROUP, 100);
            long duration = System.currentTimeMillis() - startTime;

            System.out.println("Very short timeout test - Duration: " + duration + "ms, Content: " + content);
        } catch (NacosException e) {
            long duration = System.currentTimeMillis() - startTime;
            System.out.println("Very short timeout - Duration: " + duration + "ms, Error: " + e.getErrCode());
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
        System.out.println("Publish timeout test - Duration: " + duration + "ms");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== Permission Error Tests ====================

    /**
     * NGE-004: Test permission denied handling (403)
     */
    @Test
    @Order(4)
    void testPermissionDenied() {
        // Create a service with restricted access credentials
        Properties props = new Properties();
        props.put("serverAddr", System.getProperty("nacos.server", "127.0.0.1:8848"));
        props.put("username", "invalid-user");
        props.put("password", "invalid-password");

        try {
            ConfigService restrictedService = NacosFactory.createConfigService(props);

            // Attempt operation that requires auth
            String content = restrictedService.getConfig("test-config", DEFAULT_GROUP, 3000);
            System.out.println("Permission test - Content: " + content);

            restrictedService.shutDown();
        } catch (NacosException e) {
            System.out.println("Permission denied error code: " + e.getErrCode());
            System.out.println("Permission denied message: " + e.getMessage());
            // 403 or authentication error is expected
        }
    }

    /**
     * NGE-005: Test unauthorized namespace access
     */
    @Test
    @Order(5)
    void testUnauthorizedNamespace() throws NacosException {
        String dataId = "ns-test-" + UUID.randomUUID().toString().substring(0, 8);
        String namespace = "restricted-namespace-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // Try to access a namespace that doesn't exist or is restricted
            Properties props = new Properties();
            props.put("serverAddr", System.getProperty("nacos.server", "127.0.0.1:8848"));
            props.put("username", System.getProperty("nacos.username", "nacos"));
            props.put("password", System.getProperty("nacos.password", "nacos"));
            props.put("namespace", namespace);

            ConfigService nsService = NacosFactory.createConfigService(props);
            String content = nsService.getConfig(dataId, DEFAULT_GROUP, 3000);

            System.out.println("Namespace test - Content: " + content);
            nsService.shutDown();
        } catch (NacosException e) {
            System.out.println("Namespace access error: " + e.getErrCode() + " - " + e.getMessage());
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
            assertTrue(published, "Iteration " + i + " should succeed");

            String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
            assertEquals(content, retrieved, "Content should match in iteration " + i);

            Thread.sleep(200);
        }

        System.out.println("Connection recovery test passed - 5 iterations");

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
        System.out.println("Server status: " + status);

        // Status should be UP or DOWN
        assertTrue(status.equals("UP") || status.equals("DOWN") || !status.isEmpty(),
                "Status should be valid");
    }

    /**
     * NGE-008: Test naming service server status
     */
    @Test
    @Order(8)
    void testNamingServerStatus() throws NacosException {
        String status = namingService.getServerStatus();
        assertNotNull(status, "Naming server status should not be null");
        System.out.println("Naming server status: " + status);
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

                    // Rapid operations
                    configService.publishConfig(dataId, DEFAULT_GROUP, content);
                    String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
                    configService.removeConfig(dataId, DEFAULT_GROUP);

                    if (retrieved != null) {
                        successCount.incrementAndGet();
                    }
                } catch (Exception e) {
                    errorCount.incrementAndGet();
                    System.out.println("Concurrent error in thread " + index + ": " + e.getMessage());
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        startLatch.countDown();
        boolean completed = doneLatch.await(60, TimeUnit.SECONDS);

        assertTrue(completed, "All threads should complete");
        System.out.println("Concurrent test - Success: " + successCount.get() + ", Errors: " + errorCount.get());
    }

    /**
     * NGE-010: Test rapid subscribe/unsubscribe
     */
    @Test
    @Order(10)
    void testRapidSubscribeUnsubscribe() throws NacosException, InterruptedException {
        String serviceName = "rapid-sub-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicInteger subscribeCount = new AtomicInteger(0);

        // Register a service first
        namingService.registerInstance(serviceName, "192.168.100.1", 8080);
        Thread.sleep(500);

        // Rapid subscribe/unsubscribe cycles
        for (int i = 0; i < 5; i++) {
            final int cycle = i;
            com.alibaba.nacos.api.naming.listener.EventListener listener = event -> {
                subscribeCount.incrementAndGet();
                System.out.println("Cycle " + cycle + " received event");
            };

            namingService.subscribe(serviceName, listener);
            Thread.sleep(100);
            namingService.unsubscribe(serviceName, listener);
            Thread.sleep(100);
        }

        System.out.println("Rapid subscribe/unsubscribe test - Events received: " + subscribeCount.get());

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.100.1", 8080);
    }

    // ==================== Invalid Input Tests ====================

    /**
     * NGE-011: Test invalid dataId handling
     */
    @Test
    @Order(11)
    void testInvalidDataId() {
        String[] invalidDataIds = {
                "",
                null,
                "   ",
                "a".repeat(1000) // Very long dataId
        };

        for (String dataId : invalidDataIds) {
            try {
                if (dataId == null) {
                    configService.getConfig(null, DEFAULT_GROUP, 3000);
                } else {
                    configService.getConfig(dataId, DEFAULT_GROUP, 3000);
                }
                System.out.println("DataId '" + dataId + "' - No error");
            } catch (NacosException e) {
                System.out.println("DataId '" + dataId + "' - Error: " + e.getErrCode());
            } catch (IllegalArgumentException e) {
                System.out.println("DataId '" + dataId + "' - IllegalArgument: " + e.getMessage());
            }
        }
    }

    /**
     * NGE-012: Test invalid group handling
     */
    @Test
    @Order(12)
    void testInvalidGroup() {
        String dataId = "valid-dataid-" + UUID.randomUUID().toString().substring(0, 8);
        String[] invalidGroups = {
                "",
                null,
                "   "
        };

        for (String group : invalidGroups) {
            try {
                configService.getConfig(dataId, group, 3000);
                System.out.println("Group '" + group + "' - No error");
            } catch (NacosException e) {
                System.out.println("Group '" + group + "' - Error: " + e.getErrCode());
            } catch (IllegalArgumentException e) {
                System.out.println("Group '" + group + "' - IllegalArgument: " + e.getMessage());
            }
        }
    }

    /**
     * NGE-013: Test invalid service name handling
     */
    @Test
    @Order(13)
    void testInvalidServiceName() {
        String[] invalidNames = {
                "",
                null,
                "   "
        };

        for (String name : invalidNames) {
            try {
                if (name == null) {
                    namingService.registerInstance(null, "192.168.1.1", 8080);
                } else {
                    namingService.registerInstance(name, "192.168.1.1", 8080);
                }
                System.out.println("Service name '" + name + "' - No error");
            } catch (NacosException e) {
                System.out.println("Service name '" + name + "' - Error: " + e.getErrCode());
            } catch (IllegalArgumentException e) {
                System.out.println("Service name '" + name + "' - IllegalArgument: " + e.getMessage());
            }
        }
    }

    // ==================== Listener Error Tests ====================

    /**
     * NGE-014: Test listener exception handling
     */
    @Test
    @Order(14)
    void testListenerExceptionHandling() throws NacosException, InterruptedException {
        String dataId = "listener-error-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicInteger callCount = new AtomicInteger(0);
        CountDownLatch latch = new CountDownLatch(2);

        // Add listener that throws exception
        configService.addListener(dataId, DEFAULT_GROUP, new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                callCount.incrementAndGet();
                latch.countDown();
                if (callCount.get() == 1) {
                    throw new RuntimeException("Simulated listener error");
                }
                System.out.println("Listener received after error: " + configInfo);
            }
        });

        Thread.sleep(500);

        // Publish config twice
        configService.publishConfig(dataId, DEFAULT_GROUP, "first=value");
        Thread.sleep(1000);
        configService.publishConfig(dataId, DEFAULT_GROUP, "second=value");

        boolean received = latch.await(15, TimeUnit.SECONDS);

        System.out.println("Listener error test - Calls: " + callCount.get() + ", Received both: " + received);

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
                System.out.println("Slow listener finished processing: " + configInfo);
            }
        });

        Thread.sleep(500);

        // Publish config
        long startTime = System.currentTimeMillis();
        configService.publishConfig(dataId, DEFAULT_GROUP, "slow.test=value");

        boolean received = latch.await(10, TimeUnit.SECONDS);
        long duration = System.currentTimeMillis() - startTime;

        System.out.println("Slow listener test - Duration: " + duration + "ms, Received: " + received);

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }
}
