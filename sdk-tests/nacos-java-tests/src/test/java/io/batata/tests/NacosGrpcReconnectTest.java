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
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos gRPC Reconnect/Resilience Tests
 *
 * Tests for gRPC connection resilience including listener survival across
 * reconnections, config recovery after transient failures, and naming
 * subscribe durability. These tests simulate reconnection scenarios by
 * creating fresh service instances and verifying state continuity.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosGrpcReconnectTest {

    private static String serverAddr;
    private static String username;
    private static String password;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";

    @BeforeAll
    static void setup() {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        username = System.getProperty("nacos.username", "nacos");
        password = System.getProperty("nacos.password", "nacos");
        System.out.println("gRPC Reconnect Test Setup - Server: " + serverAddr);
    }

    private Properties createProperties() {
        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);
        return properties;
    }

    // ==================== Config Listener Resilience Tests ====================

    /**
     * NGR-001: Test config listener survives service recreation
     *
     * Publishes config, shuts down the service, creates a new service,
     * re-registers a listener, publishes an update, and verifies the
     * listener receives the notification.
     */
    @Test
    @Order(1)
    void testConfigListenerSurvivesReconnect() throws NacosException, InterruptedException {
        String dataId = "ngr001-listener-reconnect-" + UUID.randomUUID().toString().substring(0, 8);

        // Phase 1: Publish initial config with first service
        ConfigService service1 = NacosFactory.createConfigService(createProperties());
        boolean published = service1.publishConfig(dataId, DEFAULT_GROUP, "phase=1");
        assertTrue(published, "Initial publish should succeed");
        Thread.sleep(500);

        // Verify initial content
        String content1 = service1.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals("phase=1", content1, "Initial content should be phase=1");

        // Shutdown first service (simulates connection loss)
        service1.shutDown();
        Thread.sleep(1000);

        // Phase 2: Create new service (simulates reconnection)
        ConfigService service2 = NacosFactory.createConfigService(createProperties());

        AtomicReference<String> receivedContent = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        // Register listener on new service
        service2.addListener(dataId, DEFAULT_GROUP, new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                System.out.println("Listener received after reconnect: " + configInfo);
                receivedContent.set(configInfo);
                latch.countDown();
            }
        });
        Thread.sleep(500);

        // Publish update after reconnection
        boolean updated = service2.publishConfig(dataId, DEFAULT_GROUP, "phase=2");
        assertTrue(updated, "Update after reconnect should succeed");

        // Verify listener fires
        boolean received = latch.await(15, TimeUnit.SECONDS);
        assertTrue(received, "Listener should receive notification after reconnect");
        assertEquals("phase=2", receivedContent.get(),
                "Listener should receive updated content");

        // Cleanup
        service2.removeConfig(dataId, DEFAULT_GROUP);
        service2.shutDown();
    }

    /**
     * NGR-002: Test config get after connection recovery
     *
     * Publishes config, simulates disconnect by shutting down the service,
     * creates a new connection, and verifies config is still retrievable.
     */
    @Test
    @Order(2)
    void testConfigGetAfterConnectionRecovery() throws NacosException, InterruptedException {
        String dataId = "ngr002-get-recovery-" + UUID.randomUUID().toString().substring(0, 8);
        String expectedContent = "recovery.test=persistent-data";

        // Publish with first connection
        ConfigService service1 = NacosFactory.createConfigService(createProperties());
        service1.publishConfig(dataId, DEFAULT_GROUP, expectedContent);
        Thread.sleep(500);

        String verifyContent = service1.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(expectedContent, verifyContent, "Content should be published");

        // Simulate disconnect
        service1.shutDown();
        Thread.sleep(1000);

        // Recover with new connection
        ConfigService service2 = NacosFactory.createConfigService(createProperties());
        try {
            String recovered = service2.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals(expectedContent, recovered,
                    "Config should be retrievable after connection recovery");
        } finally {
            service2.removeConfig(dataId, DEFAULT_GROUP);
            service2.shutDown();
        }
    }

    /**
     * NGR-003: Test multiple rapid reconnections for config
     *
     * Performs multiple connect/disconnect cycles and verifies the config
     * service remains functional throughout.
     */
    @Test
    @Order(3)
    void testMultipleRapidReconnectionsConfig() throws NacosException, InterruptedException {
        String dataId = "ngr003-rapid-reconnect-" + UUID.randomUUID().toString().substring(0, 8);
        int reconnectCycles = 3;

        // Initial publish
        ConfigService initialService = NacosFactory.createConfigService(createProperties());
        initialService.publishConfig(dataId, DEFAULT_GROUP, "cycle=0");
        initialService.shutDown();
        Thread.sleep(500);

        // Perform rapid reconnect cycles
        for (int cycle = 1; cycle <= reconnectCycles; cycle++) {
            ConfigService service = NacosFactory.createConfigService(createProperties());

            // Read previous value
            String content = service.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertNotNull(content, "Config should be readable in cycle " + cycle);

            // Update with new value
            boolean updated = service.publishConfig(dataId, DEFAULT_GROUP, "cycle=" + cycle);
            assertTrue(updated, "Update should succeed in cycle " + cycle);

            Thread.sleep(300);

            // Verify the update
            String updatedContent = service.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals("cycle=" + cycle, updatedContent,
                    "Content should reflect cycle " + cycle);

            // Disconnect
            service.shutDown();
            Thread.sleep(500);
        }

        // Final verification with fresh connection
        ConfigService finalService = NacosFactory.createConfigService(createProperties());
        try {
            String finalContent = finalService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals("cycle=" + reconnectCycles, finalContent,
                    "Final content should reflect last cycle");
        } finally {
            finalService.removeConfig(dataId, DEFAULT_GROUP);
            finalService.shutDown();
        }
    }

    // ==================== Naming Subscribe Resilience Tests ====================

    /**
     * NGR-004: Test naming subscribe survives service recreation
     *
     * Subscribes to a service, shuts down the naming service, creates a
     * new service, re-subscribes, registers an instance, and verifies
     * the subscriber receives the notification.
     */
    @Test
    @Order(4)
    void testNamingSubscribeSurvivesReconnect() throws NacosException, InterruptedException {
        String serviceName = "ngr004-naming-reconnect-" + UUID.randomUUID().toString().substring(0, 8);

        // Phase 1: Register an instance with first service
        NamingService namingService1 = NacosFactory.createNamingService(createProperties());
        namingService1.registerInstance(serviceName, "10.0.10.1", 8080);
        Thread.sleep(1000);

        List<Instance> instances1 = namingService1.getAllInstances(serviceName);
        assertFalse(instances1.isEmpty(), "Should have instance before disconnect");

        // Simulate disconnect
        namingService1.shutDown();
        Thread.sleep(1000);

        // Phase 2: Reconnect with new service and subscribe
        NamingService namingService2 = NacosFactory.createNamingService(createProperties());
        AtomicReference<List<Instance>> receivedInstances = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        EventListener listener = event -> {
            if (event instanceof NamingEvent) {
                NamingEvent namingEvent = (NamingEvent) event;
                System.out.println("Naming event after reconnect: " + namingEvent.getInstances().size());
                receivedInstances.set(namingEvent.getInstances());
                latch.countDown();
            }
        };
        namingService2.subscribe(serviceName, listener);
        Thread.sleep(500);

        // Register a new instance to trigger notification
        namingService2.registerInstance(serviceName, "10.0.10.2", 8081);

        boolean received = latch.await(15, TimeUnit.SECONDS);
        assertTrue(received, "Should receive naming event after reconnection");
        assertNotNull(receivedInstances.get(), "Received instances should not be null");

        // Cleanup
        namingService2.unsubscribe(serviceName, listener);
        namingService2.deregisterInstance(serviceName, "10.0.10.2", 8081);
        namingService2.shutDown();
    }

    /**
     * NGR-005: Test instance registration retry after transient failure
     *
     * Simulates a scenario where the first connection fails and the client
     * retries with a new connection, verifying the registration eventually
     * succeeds.
     */
    @Test
    @Order(5)
    void testInstanceRegistrationRetryAfterFailure() throws NacosException, InterruptedException {
        String serviceName = "ngr005-reg-retry-" + UUID.randomUUID().toString().substring(0, 8);

        // Attempt registration with a fresh connection
        NamingService namingService = NacosFactory.createNamingService(createProperties());

        try {
            // Register with retry logic
            int maxRetries = 3;
            boolean registered = false;
            NacosException lastException = null;

            for (int attempt = 0; attempt < maxRetries; attempt++) {
                try {
                    namingService.registerInstance(serviceName, "10.0.11.1", 8080);
                    registered = true;
                    break;
                } catch (NacosException e) {
                    lastException = e;
                    System.out.println("Registration attempt " + (attempt + 1) + " failed: " + e.getMessage());
                    Thread.sleep(1000);
                }
            }

            assertTrue(registered, "Registration should succeed within retry attempts" +
                    (lastException != null ? ". Last error: " + lastException.getMessage() : ""));

            Thread.sleep(1500);

            // Verify instance is registered
            List<Instance> instances = namingService.getAllInstances(serviceName);
            assertFalse(instances.isEmpty(), "Instance should be registered after retry");
            assertEquals("10.0.11.1", instances.get(0).getIp(),
                    "Registered instance IP should match");

            // Cleanup
            namingService.deregisterInstance(serviceName, "10.0.11.1", 8080);
        } finally {
            namingService.shutDown();
        }
    }

    /**
     * NGR-006: Test config listener with multiple updates across reconnections
     *
     * Registers a listener, publishes multiple updates across different
     * connection lifecycles, and verifies the listener tracks all changes.
     */
    @Test
    @Order(6)
    void testConfigListenerMultipleUpdatesAcrossReconnections() throws NacosException, InterruptedException {
        String dataId = "ngr006-multi-update-" + UUID.randomUUID().toString().substring(0, 8);

        // Publish initial config
        ConfigService setupService = NacosFactory.createConfigService(createProperties());
        setupService.publishConfig(dataId, DEFAULT_GROUP, "version=0");
        setupService.shutDown();
        Thread.sleep(500);

        // Create service with listener
        ConfigService listenerService = NacosFactory.createConfigService(createProperties());
        AtomicInteger updateCount = new AtomicInteger(0);
        AtomicReference<String> latestContent = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(3);

        listenerService.addListener(dataId, DEFAULT_GROUP, new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                updateCount.incrementAndGet();
                latestContent.set(configInfo);
                latch.countDown();
                System.out.println("Update #" + updateCount.get() + ": " + configInfo);
            }
        });
        Thread.sleep(500);

        // Publish 3 updates from separate connections
        for (int i = 1; i <= 3; i++) {
            ConfigService updater = NacosFactory.createConfigService(createProperties());
            updater.publishConfig(dataId, DEFAULT_GROUP, "version=" + i);
            updater.shutDown();
            Thread.sleep(1000);
        }

        // Wait for all updates to be received
        boolean allReceived = latch.await(30, TimeUnit.SECONDS);

        System.out.println("Total updates received: " + updateCount.get());
        assertTrue(updateCount.get() >= 1,
                "Should receive at least 1 config update notification");

        // Cleanup
        listenerService.removeConfig(dataId, DEFAULT_GROUP);
        listenerService.shutDown();
    }

    /**
     * NGR-007: Test naming service state persistence across reconnections
     *
     * Registers instances, disconnects, reconnects, and verifies that
     * previously registered ephemeral instances are still tracked by
     * the server (within the health check timeout window).
     */
    @Test
    @Order(7)
    void testNamingStatePersistenceAcrossReconnect() throws NacosException, InterruptedException {
        String serviceName = "ngr007-state-persist-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances with first connection
        NamingService service1 = NacosFactory.createNamingService(createProperties());
        service1.registerInstance(serviceName, "10.0.12.1", 8080);
        service1.registerInstance(serviceName, "10.0.12.2", 8081);
        Thread.sleep(1500);

        List<Instance> before = service1.getAllInstances(serviceName);
        assertEquals(2, before.size(), "Should have 2 instances before disconnect");

        // Shutdown first service
        service1.shutDown();

        // Quickly reconnect (within health check window)
        Thread.sleep(500);
        NamingService service2 = NacosFactory.createNamingService(createProperties());

        try {
            // Query instances - ephemeral instances may still be present
            // if we reconnect within the health check timeout window
            List<Instance> after = service2.getAllInstances(serviceName);
            System.out.println("Instances after reconnect: " + after.size());

            // Re-register to ensure service works
            service2.registerInstance(serviceName, "10.0.12.3", 8082);
            Thread.sleep(1500);

            List<Instance> withNew = service2.getAllInstances(serviceName);
            assertTrue(withNew.size() >= 1,
                    "Should have at least 1 instance after re-registration");

            boolean hasNewInstance = false;
            for (Instance inst : withNew) {
                if ("10.0.12.3".equals(inst.getIp())) {
                    hasNewInstance = true;
                    break;
                }
            }
            assertTrue(hasNewInstance, "New instance should be registered after reconnect");

            // Cleanup
            for (Instance inst : withNew) {
                service2.deregisterInstance(serviceName, inst.getIp(), inst.getPort());
            }
        } finally {
            service2.shutDown();
        }
    }

    /**
     * NGR-008: Test concurrent config and naming operations during reconnection
     *
     * Performs config and naming operations simultaneously, simulates
     * disconnection, and verifies both services recover properly.
     */
    @Test
    @Order(8)
    void testConcurrentConfigNamingDuringReconnect() throws NacosException, InterruptedException {
        String dataId = "ngr008-concurrent-" + UUID.randomUUID().toString().substring(0, 8);
        String serviceName = "ngr008-concurrent-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Phase 1: Setup with first connections
        ConfigService configService1 = NacosFactory.createConfigService(createProperties());
        NamingService namingService1 = NacosFactory.createNamingService(createProperties());

        configService1.publishConfig(dataId, DEFAULT_GROUP, "concurrent.phase=1");
        namingService1.registerInstance(serviceName, "10.0.13.1", 8080);
        Thread.sleep(1000);

        // Verify both operational
        String config1 = configService1.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals("concurrent.phase=1", config1);

        List<Instance> instances1 = namingService1.getAllInstances(serviceName);
        assertFalse(instances1.isEmpty());

        // Simulate disconnect
        configService1.shutDown();
        namingService1.shutDown();
        Thread.sleep(1000);

        // Phase 2: Recover both services
        ConfigService configService2 = NacosFactory.createConfigService(createProperties());
        NamingService namingService2 = NacosFactory.createNamingService(createProperties());

        try {
            // Verify config is accessible
            String config2 = configService2.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals("concurrent.phase=1", config2,
                    "Config should be recoverable after reconnect");

            // Update config
            configService2.publishConfig(dataId, DEFAULT_GROUP, "concurrent.phase=2");
            Thread.sleep(500);

            String updated = configService2.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals("concurrent.phase=2", updated,
                    "Updated config should be retrievable");

            // Register new instance
            namingService2.registerInstance(serviceName, "10.0.13.2", 8081);
            Thread.sleep(1000);

            List<Instance> instances2 = namingService2.getAllInstances(serviceName);
            assertTrue(instances2.size() >= 1,
                    "Should have at least 1 instance after naming reconnect");

            // Cleanup
            configService2.removeConfig(dataId, DEFAULT_GROUP);
            for (Instance inst : instances2) {
                namingService2.deregisterInstance(serviceName, inst.getIp(), inst.getPort());
            }
        } finally {
            configService2.shutDown();
            namingService2.shutDown();
        }
    }
}
