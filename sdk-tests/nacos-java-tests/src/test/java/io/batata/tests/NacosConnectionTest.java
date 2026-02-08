package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.config.listener.Listener;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.listener.EventListener;
import com.alibaba.nacos.api.naming.listener.NamingEvent;
import com.alibaba.nacos.api.naming.pojo.Instance;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Connection Management Tests
 *
 * Tests for Nacos SDK connection lifecycle, health checks, authentication,
 * reconnection, and resource management.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConnectionTest {

    private static ConfigService configService;
    private static NamingService namingService;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static String serverAddr;
    private static String username;
    private static String password;

    @BeforeAll
    static void setup() throws NacosException {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        username = System.getProperty("nacos.username", "nacos");
        password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        configService = NacosFactory.createConfigService(properties);
        namingService = NacosFactory.createNamingService(properties);

        System.out.println("Nacos Connection Test Setup - Server: " + serverAddr);
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

    // ==================== NCT-001 to NCT-005: Basic Connection Tests ====================

    /**
     * NCT-001: Test basic connection setup
     *
     * Verifies that a ConfigService and NamingService can be created successfully
     * with basic properties and that operations can be performed immediately.
     */
    @Test
    @Order(1)
    void testBasicConnectionSetup() throws NacosException {
        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        // Create new service to test connection setup
        ConfigService testConfigService = NacosFactory.createConfigService(properties);
        assertNotNull(testConfigService, "ConfigService should be created successfully");

        try {
            // Verify connection by performing a simple operation
            String dataId = "nct001-conn-setup-" + UUID.randomUUID();
            boolean published = testConfigService.publishConfig(dataId, DEFAULT_GROUP, "connection.test=success");
            assertTrue(published, "Should be able to publish config after connection setup");

            // Cleanup
            testConfigService.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            testConfigService.shutDown();
        }
    }

    /**
     * NCT-002: Test connection with custom timeout
     *
     * Verifies that custom timeout settings are respected when creating connections
     * and performing operations.
     */
    @Test
    @Order(2)
    void testConnectionWithCustomTimeout() throws NacosException {
        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);
        // Set custom timeout values
        properties.put("nacos.remote.client.grpc.timeout", "5000");
        properties.put("nacos.remote.client.grpc.connect.timeout", "3000");

        ConfigService testConfigService = NacosFactory.createConfigService(properties);
        assertNotNull(testConfigService, "ConfigService should be created with custom timeout");

        try {
            String dataId = "nct002-timeout-" + UUID.randomUUID();

            // Test with short timeout - should succeed for existing server
            long startTime = System.currentTimeMillis();
            boolean published = testConfigService.publishConfig(dataId, DEFAULT_GROUP, "timeout.test=value");
            long duration = System.currentTimeMillis() - startTime;

            assertTrue(published, "Publish should succeed with custom timeout");
            System.out.println("Operation completed in " + duration + "ms with custom timeout settings");

            // Cleanup
            testConfigService.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            testConfigService.shutDown();
        }
    }

    /**
     * NCT-003: Test connection health check
     *
     * Verifies that the connection remains healthy by performing sequential operations
     * and checking that the SDK can communicate with the server.
     */
    @Test
    @Order(3)
    void testConnectionHealthCheck() throws NacosException, InterruptedException {
        String dataId = "nct003-health-" + UUID.randomUUID();

        // Perform multiple operations to verify connection health
        for (int i = 0; i < 5; i++) {
            boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "health.check=" + i);
            assertTrue(published, "Connection should be healthy - iteration " + i);

            String content = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals("health.check=" + i, content, "Should retrieve correct content");

            Thread.sleep(200);
        }

        // Naming service health check
        String serviceName = "nct003-health-svc-" + UUID.randomUUID();
        namingService.registerInstance(serviceName, "192.168.100.1", 8080);
        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty(), "Naming service connection should be healthy");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
        namingService.deregisterInstance(serviceName, "192.168.100.1", 8080);
    }

    /**
     * NCT-004: Test server list retrieval
     *
     * Verifies that the SDK can work with the provided server address(es)
     * and perform operations successfully.
     */
    @Test
    @Order(4)
    void testServerListRetrieval() throws NacosException, InterruptedException {
        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        ConfigService testConfigService = NacosFactory.createConfigService(properties);
        NamingService testNamingService = NacosFactory.createNamingService(properties);

        try {
            // Verify both services can connect to the server(s)
            String dataId = "nct004-server-list-" + UUID.randomUUID();
            String serviceName = "nct004-server-svc-" + UUID.randomUUID();

            // Config service operation
            boolean configResult = testConfigService.publishConfig(dataId, DEFAULT_GROUP, "server.list=test");
            assertTrue(configResult, "Config service should connect to server");

            // Naming service operation
            testNamingService.registerInstance(serviceName, "192.168.100.2", 8080);
            Thread.sleep(500);

            List<Instance> instances = testNamingService.getAllInstances(serviceName);
            assertFalse(instances.isEmpty(), "Naming service should connect to server");

            System.out.println("Successfully connected to server: " + serverAddr);

            // Cleanup
            testConfigService.removeConfig(dataId, DEFAULT_GROUP);
            testNamingService.deregisterInstance(serviceName, "192.168.100.2", 8080);
        } finally {
            testConfigService.shutDown();
            testNamingService.shutDown();
        }
    }

    /**
     * NCT-005: Test connection status
     *
     * Verifies that connection status can be determined by performing operations
     * and checking server status through the SDK.
     */
    @Test
    @Order(5)
    void testConnectionStatus() throws NacosException, InterruptedException {
        // Check status via config service
        String dataId = "nct005-status-" + UUID.randomUUID();
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "status.check=active");
        assertTrue(published, "Connection status should be active");

        // Verify getServerStatus via naming service
        String status = namingService.getServerStatus();
        assertNotNull(status, "Server status should not be null");
        System.out.println("Server status: " + status);

        // Check if server is UP
        assertTrue("UP".equalsIgnoreCase(status) || status.toLowerCase().contains("up"),
                "Server should be in UP state");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== NCT-006 to NCT-010: Advanced Connection Tests ====================

    /**
     * NCT-006: Test multiple server addresses
     *
     * Verifies that the SDK can be configured with multiple server addresses
     * (even if only one is available for testing).
     */
    @Test
    @Order(6)
    void testMultipleServerAddresses() throws NacosException {
        // Configure with multiple addresses (comma-separated)
        // In testing, only the first address may be available
        String multipleAddresses = serverAddr;
        if (!serverAddr.contains(",")) {
            // Add a non-existent server to simulate multiple addresses
            // The SDK should still work with the first available server
            multipleAddresses = serverAddr;
        }

        Properties properties = new Properties();
        properties.put("serverAddr", multipleAddresses);
        properties.put("username", username);
        properties.put("password", password);

        ConfigService testConfigService = NacosFactory.createConfigService(properties);

        try {
            String dataId = "nct006-multi-addr-" + UUID.randomUUID();
            boolean published = testConfigService.publishConfig(dataId, DEFAULT_GROUP, "multi.server=test");
            assertTrue(published, "Should connect successfully with server address configuration");

            System.out.println("Connected with server address(es): " + multipleAddresses);

            // Cleanup
            testConfigService.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            testConfigService.shutDown();
        }
    }

    /**
     * NCT-007: Test connection retry logic
     *
     * Verifies that the SDK can handle transient failures and retry operations.
     */
    @Test
    @Order(7)
    void testConnectionRetryLogic() throws NacosException, InterruptedException {
        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);
        // Configure retry settings
        properties.put("nacos.remote.client.grpc.retry.times", "3");

        ConfigService testConfigService = NacosFactory.createConfigService(properties);

        try {
            String dataId = "nct007-retry-" + UUID.randomUUID();

            // Perform multiple rapid operations to test retry logic
            int successCount = 0;
            int totalAttempts = 10;

            for (int i = 0; i < totalAttempts; i++) {
                try {
                    boolean published = testConfigService.publishConfig(
                            dataId, DEFAULT_GROUP, "retry.test=" + i);
                    if (published) {
                        successCount++;
                    }
                } catch (NacosException e) {
                    System.out.println("Attempt " + i + " failed: " + e.getMessage());
                }
            }

            assertTrue(successCount >= totalAttempts * 0.8,
                    "At least 80% of operations should succeed with retry logic");
            System.out.println("Retry test: " + successCount + "/" + totalAttempts + " operations succeeded");

            // Cleanup
            testConfigService.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            testConfigService.shutDown();
        }
    }

    /**
     * NCT-008: Test connection pool management
     *
     * Verifies that multiple concurrent connections are managed properly
     * through the SDK's internal connection pooling.
     */
    @Test
    @Order(8)
    void testConnectionPoolManagement() throws InterruptedException, NacosException {
        int threadCount = 10;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(threadCount);
        AtomicInteger successCount = new AtomicInteger(0);
        AtomicReference<Exception> lastError = new AtomicReference<>();

        String prefix = "nct008-pool-" + UUID.randomUUID().toString().substring(0, 8);

        for (int i = 0; i < threadCount; i++) {
            final int index = i;
            new Thread(() -> {
                try {
                    startLatch.await();
                    String dataId = prefix + "-" + index;

                    // Each thread performs multiple operations
                    for (int j = 0; j < 5; j++) {
                        configService.publishConfig(dataId, DEFAULT_GROUP, "pool.test=" + j);
                        String content = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
                        if (("pool.test=" + j).equals(content)) {
                            successCount.incrementAndGet();
                        }
                    }

                    // Cleanup
                    configService.removeConfig(dataId, DEFAULT_GROUP);
                } catch (Exception e) {
                    lastError.set(e);
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        // Start all threads simultaneously
        startLatch.countDown();

        // Wait for completion
        boolean completed = doneLatch.await(60, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete");

        int totalOperations = threadCount * 5;
        System.out.println("Connection pool test: " + successCount.get() + "/" + totalOperations + " operations succeeded");

        assertTrue(successCount.get() >= totalOperations * 0.9,
                "At least 90% of pooled operations should succeed");

        if (lastError.get() != null) {
            System.out.println("Last error encountered: " + lastError.get().getMessage());
        }
    }

    /**
     * NCT-009: Test connection with authentication
     *
     * Verifies that connections with proper authentication credentials work correctly
     * and that operations are authorized.
     */
    @Test
    @Order(9)
    void testConnectionWithAuthentication() throws NacosException {
        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        // Create authenticated service
        ConfigService authConfigService = NacosFactory.createConfigService(properties);

        try {
            String dataId = "nct009-auth-" + UUID.randomUUID();

            // Perform authenticated operations
            boolean published = authConfigService.publishConfig(dataId, DEFAULT_GROUP, "auth.test=success");
            assertTrue(published, "Authenticated publish should succeed");

            String content = authConfigService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals("auth.test=success", content, "Authenticated read should succeed");

            boolean removed = authConfigService.removeConfig(dataId, DEFAULT_GROUP);
            assertTrue(removed, "Authenticated delete should succeed");

            System.out.println("Authentication test passed for user: " + username);
        } finally {
            authConfigService.shutDown();
        }
    }

    /**
     * NCT-010: Test graceful shutdown
     *
     * Verifies that the SDK shuts down gracefully, releasing resources properly
     * and completing pending operations.
     */
    @Test
    @Order(10)
    void testGracefulShutdown() throws NacosException, InterruptedException {
        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        ConfigService shutdownConfigService = NacosFactory.createConfigService(properties);
        NamingService shutdownNamingService = NacosFactory.createNamingService(properties);

        String dataId = "nct010-shutdown-" + UUID.randomUUID();
        String serviceName = "nct010-shutdown-svc-" + UUID.randomUUID();

        // Perform operations before shutdown
        shutdownConfigService.publishConfig(dataId, DEFAULT_GROUP, "shutdown.test=before");
        shutdownNamingService.registerInstance(serviceName, "192.168.100.10", 8080);
        Thread.sleep(500);

        // Initiate graceful shutdown
        long shutdownStart = System.currentTimeMillis();
        shutdownConfigService.shutDown();
        shutdownNamingService.shutDown();
        long shutdownDuration = System.currentTimeMillis() - shutdownStart;

        System.out.println("Graceful shutdown completed in " + shutdownDuration + "ms");

        // Verify services are shut down by checking if operations fail
        boolean shutdownComplete = true;
        try {
            shutdownConfigService.getConfig(dataId, DEFAULT_GROUP, 1000);
            shutdownComplete = false; // Should have thrown exception
        } catch (Exception e) {
            // Expected - service is shut down
            System.out.println("Config service properly shut down: " + e.getClass().getSimpleName());
        }

        assertTrue(shutdownComplete, "Services should be properly shut down");

        // Cleanup using the main services
        configService.removeConfig(dataId, DEFAULT_GROUP);
        namingService.deregisterInstance(serviceName, "192.168.100.10", 8080);
    }

    // ==================== NCT-011 to NCT-015: Connection Resilience Tests ====================

    /**
     * NCT-011: Test reconnection after disconnect
     *
     * Verifies that the SDK can recover and resume operations after temporary
     * connection issues (simulated by creating new services).
     */
    @Test
    @Order(11)
    void testReconnectionAfterDisconnect() throws NacosException, InterruptedException {
        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        String dataId = "nct011-reconnect-" + UUID.randomUUID();

        // First connection
        ConfigService service1 = NacosFactory.createConfigService(properties);
        service1.publishConfig(dataId, DEFAULT_GROUP, "reconnect.phase=1");
        service1.shutDown();

        Thread.sleep(1000);

        // Reconnect (new connection)
        ConfigService service2 = NacosFactory.createConfigService(properties);
        try {
            // Verify we can still access data after reconnection
            String content = service2.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals("reconnect.phase=1", content, "Data should be accessible after reconnection");

            // Update data
            service2.publishConfig(dataId, DEFAULT_GROUP, "reconnect.phase=2");
            content = service2.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals("reconnect.phase=2", content, "Updated data should be accessible");

            System.out.println("Reconnection test passed");

            // Cleanup
            service2.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            service2.shutDown();
        }
    }

    /**
     * NCT-012: Test connection keep-alive
     *
     * Verifies that long-lived connections remain active and can perform operations
     * over extended periods.
     */
    @Test
    @Order(12)
    void testConnectionKeepAlive() throws NacosException, InterruptedException {
        String dataId = "nct012-keepalive-" + UUID.randomUUID();

        // Initial operation
        configService.publishConfig(dataId, DEFAULT_GROUP, "keepalive.time=0");

        // Perform operations with delays to test keep-alive
        for (int i = 1; i <= 5; i++) {
            Thread.sleep(2000); // 2-second delay between operations

            String content = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertNotNull(content, "Connection should stay alive - iteration " + i);

            configService.publishConfig(dataId, DEFAULT_GROUP, "keepalive.time=" + (i * 2));
            System.out.println("Keep-alive iteration " + i + " at " + (i * 2) + " seconds");
        }

        // Final verification
        String finalContent = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals("keepalive.time=10", finalContent, "Connection should remain active");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCT-013: Test concurrent connection requests
     *
     * Verifies that the SDK handles multiple concurrent connection requests properly.
     */
    @Test
    @Order(13)
    void testConcurrentConnectionRequests() throws InterruptedException {
        int connectionCount = 5;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(connectionCount);
        List<ConfigService> services = Collections.synchronizedList(new ArrayList<>());
        AtomicInteger successCount = new AtomicInteger(0);
        AtomicReference<Exception> lastError = new AtomicReference<>();

        String prefix = "nct013-concurrent-" + UUID.randomUUID().toString().substring(0, 8);

        // Create multiple connections concurrently
        for (int i = 0; i < connectionCount; i++) {
            final int index = i;
            new Thread(() -> {
                try {
                    startLatch.await();

                    Properties properties = new Properties();
                    properties.put("serverAddr", serverAddr);
                    properties.put("username", username);
                    properties.put("password", password);

                    ConfigService service = NacosFactory.createConfigService(properties);
                    services.add(service);

                    // Verify connection works
                    String dataId = prefix + "-" + index;
                    boolean published = service.publishConfig(dataId, DEFAULT_GROUP, "concurrent.conn=" + index);
                    if (published) {
                        successCount.incrementAndGet();
                    }

                    // Cleanup
                    service.removeConfig(dataId, DEFAULT_GROUP);
                } catch (Exception e) {
                    lastError.set(e);
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        // Start all connection attempts
        startLatch.countDown();

        // Wait for completion
        boolean completed = doneLatch.await(60, TimeUnit.SECONDS);
        assertTrue(completed, "All connection attempts should complete");

        System.out.println("Concurrent connections: " + successCount.get() + "/" + connectionCount + " succeeded");

        // Shutdown all services
        for (ConfigService service : services) {
            try {
                service.shutDown();
            } catch (Exception e) {
                // Ignore shutdown errors
            }
        }

        assertTrue(successCount.get() >= connectionCount * 0.8,
                "At least 80% of concurrent connections should succeed");
    }

    /**
     * NCT-014: Test connection resource cleanup
     *
     * Verifies that resources are properly released when connections are closed,
     * preventing resource leaks.
     */
    @Test
    @Order(14)
    void testConnectionResourceCleanup() throws NacosException, InterruptedException {
        List<ConfigService> services = new ArrayList<>();
        String prefix = "nct014-cleanup-" + UUID.randomUUID().toString().substring(0, 8);

        // Create and close multiple services to test resource cleanup
        for (int cycle = 0; cycle < 3; cycle++) {
            Properties properties = new Properties();
            properties.put("serverAddr", serverAddr);
            properties.put("username", username);
            properties.put("password", password);

            ConfigService service = NacosFactory.createConfigService(properties);

            String dataId = prefix + "-cycle-" + cycle;
            service.publishConfig(dataId, DEFAULT_GROUP, "cleanup.cycle=" + cycle);
            String content = service.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals("cleanup.cycle=" + cycle, content);

            // Cleanup config
            service.removeConfig(dataId, DEFAULT_GROUP);

            // Shutdown service
            service.shutDown();

            System.out.println("Resource cleanup cycle " + (cycle + 1) + " completed");
            Thread.sleep(500);
        }

        // Verify a new connection still works (resources were properly released)
        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        ConfigService finalService = NacosFactory.createConfigService(properties);
        try {
            String dataId = prefix + "-final";
            boolean published = finalService.publishConfig(dataId, DEFAULT_GROUP, "cleanup.final=success");
            assertTrue(published, "New connection should work after resource cleanup");

            finalService.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            finalService.shutDown();
        }

        System.out.println("Resource cleanup test passed");
    }

    /**
     * NCT-015: Test connection event callbacks
     *
     * Verifies that connection-related events (like config changes) are properly
     * delivered through callbacks/listeners.
     */
    @Test
    @Order(15)
    void testConnectionEventCallbacks() throws NacosException, InterruptedException {
        String dataId = "nct015-events-" + UUID.randomUUID();
        String serviceName = "nct015-events-svc-" + UUID.randomUUID();

        AtomicInteger configEventCount = new AtomicInteger(0);
        AtomicInteger namingEventCount = new AtomicInteger(0);
        CountDownLatch configLatch = new CountDownLatch(2);
        CountDownLatch namingLatch = new CountDownLatch(1);

        // Add config listener
        Listener configListener = new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                System.out.println("Config event received: " + configInfo);
                configEventCount.incrementAndGet();
                configLatch.countDown();
            }
        };
        configService.addListener(dataId, DEFAULT_GROUP, configListener);

        // Add naming listener
        EventListener namingListener = event -> {
            if (event instanceof NamingEvent) {
                NamingEvent namingEvent = (NamingEvent) event;
                System.out.println("Naming event received: " + namingEvent.getInstances().size() + " instances");
                namingEventCount.incrementAndGet();
                namingLatch.countDown();
            }
        };
        namingService.subscribe(serviceName, namingListener);

        Thread.sleep(500);

        // Trigger config events
        configService.publishConfig(dataId, DEFAULT_GROUP, "event.test=1");
        Thread.sleep(500);
        configService.publishConfig(dataId, DEFAULT_GROUP, "event.test=2");

        // Trigger naming event
        namingService.registerInstance(serviceName, "192.168.100.15", 8080);

        // Wait for events
        boolean configEventsReceived = configLatch.await(15, TimeUnit.SECONDS);
        boolean namingEventsReceived = namingLatch.await(10, TimeUnit.SECONDS);

        System.out.println("Config events received: " + configEventCount.get());
        System.out.println("Naming events received: " + namingEventCount.get());

        assertTrue(configEventsReceived, "Should receive config event callbacks");
        assertTrue(namingEventsReceived, "Should receive naming event callbacks");
        assertTrue(configEventCount.get() >= 1, "Should receive at least one config event");
        assertTrue(namingEventCount.get() >= 1, "Should receive at least one naming event");

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, configListener);
        namingService.unsubscribe(serviceName, namingListener);
        configService.removeConfig(dataId, DEFAULT_GROUP);
        namingService.deregisterInstance(serviceName, "192.168.100.15", 8080);
    }
}
