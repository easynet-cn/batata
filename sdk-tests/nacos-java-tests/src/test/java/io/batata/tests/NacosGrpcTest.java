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
 * Nacos gRPC Connection and Handler Tests
 *
 * These tests exercise the gRPC transport layer of the Nacos SDK.
 * The Nacos Java SDK uses gRPC by default for communication.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosGrpcTest {

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

        System.out.println("Nacos gRPC Test Setup - Server: " + serverAddr);
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

    // ==================== gRPC Connection Tests ====================

    /**
     * NG-001: Test gRPC health check (implicit via SDK operations)
     */
    @Test
    @Order(1)
    void testGrpcHealthCheck() throws NacosException {
        // gRPC health check is performed implicitly when SDK connects
        // A successful config operation indicates healthy gRPC connection
        String dataId = "grpc-health-" + UUID.randomUUID();

        boolean result = configService.publishConfig(dataId, DEFAULT_GROUP, "health=check");
        assertTrue(result, "gRPC connection should be healthy");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NG-002: Test gRPC server check (implicit via naming service)
     */
    @Test
    @Order(2)
    void testGrpcServerCheck() throws NacosException, InterruptedException {
        // Server check is performed when naming service connects
        String serviceName = "grpc-server-check-" + UUID.randomUUID();

        namingService.registerInstance(serviceName, "192.168.1.1", 8080);
        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty(), "gRPC server should respond to naming requests");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.1.1", 8080);
    }

    /**
     * NG-003: Test gRPC connection setup
     */
    @Test
    @Order(3)
    void testGrpcConnectionSetup() throws NacosException {
        // Test connection by performing multiple operations
        String prefix = "grpc-conn-" + UUID.randomUUID().toString().substring(0, 8);

        for (int i = 0; i < 5; i++) {
            String dataId = prefix + "-" + i;
            configService.publishConfig(dataId, DEFAULT_GROUP, "conn.test=" + i);
            String content = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals("conn.test=" + i, content);
            configService.removeConfig(dataId, DEFAULT_GROUP);
        }
    }

    // ==================== Config gRPC Handler Tests ====================

    /**
     * NG-004: Test ConfigQueryHandler via getConfig
     */
    @Test
    @Order(4)
    void testConfigQueryHandler() throws NacosException {
        String dataId = "grpc-query-" + UUID.randomUUID();
        String content = "query.test=value";

        configService.publishConfig(dataId, DEFAULT_GROUP, content);

        // ConfigQueryHandler handles this request
        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(content, retrieved, "ConfigQueryHandler should return correct content");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NG-005: Test ConfigPublishHandler via publishConfig
     */
    @Test
    @Order(5)
    void testConfigPublishHandler() throws NacosException {
        String dataId = "grpc-publish-" + UUID.randomUUID();

        // ConfigPublishHandler handles this request
        boolean result = configService.publishConfig(dataId, DEFAULT_GROUP, "publish.test=value");
        assertTrue(result, "ConfigPublishHandler should succeed");

        // Verify
        String content = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(content);

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NG-006: Test ConfigRemoveHandler via removeConfig
     */
    @Test
    @Order(6)
    void testConfigRemoveHandler() throws NacosException {
        String dataId = "grpc-remove-" + UUID.randomUUID();

        configService.publishConfig(dataId, DEFAULT_GROUP, "remove.test=value");

        // ConfigRemoveHandler handles this request
        boolean result = configService.removeConfig(dataId, DEFAULT_GROUP);
        assertTrue(result, "ConfigRemoveHandler should succeed");

        // Verify removed
        String content = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertNull(content, "Config should be removed");
    }

    /**
     * NG-007: Test ConfigBatchListenHandler via multiple listeners
     */
    @Test
    @Order(7)
    void testConfigBatchListenHandler() throws NacosException, InterruptedException {
        String prefix = "grpc-batch-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicInteger notificationCount = new AtomicInteger(0);
        CountDownLatch latch = new CountDownLatch(3);

        // Add multiple listeners (triggers batch listen)
        for (int i = 0; i < 3; i++) {
            final String dataId = prefix + "-" + i;
            configService.addListener(dataId, DEFAULT_GROUP, new Listener() {
                @Override
                public Executor getExecutor() { return null; }

                @Override
                public void receiveConfigInfo(String configInfo) {
                    System.out.println("Batch listener received: " + dataId + " = " + configInfo);
                    notificationCount.incrementAndGet();
                    latch.countDown();
                }
            });
        }

        Thread.sleep(1000);

        // Publish to trigger notifications
        for (int i = 0; i < 3; i++) {
            configService.publishConfig(prefix + "-" + i, DEFAULT_GROUP, "batch=" + i);
        }

        boolean received = latch.await(15, TimeUnit.SECONDS);
        assertTrue(received, "Should receive batch notifications");
        assertEquals(3, notificationCount.get(), "Should receive all 3 notifications");

        // Cleanup
        for (int i = 0; i < 3; i++) {
            configService.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
        }
    }

    /**
     * NG-008: Test ConfigChangeNotifyHandler via listener notification
     */
    @Test
    @Order(8)
    void testConfigChangeNotifyHandler() throws NacosException, InterruptedException {
        String dataId = "grpc-notify-" + UUID.randomUUID();
        AtomicReference<String> receivedContent = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        configService.addListener(dataId, DEFAULT_GROUP, new Listener() {
            @Override
            public Executor getExecutor() { return null; }

            @Override
            public void receiveConfigInfo(String configInfo) {
                System.out.println("Change notification: " + configInfo);
                receivedContent.set(configInfo);
                latch.countDown();
            }
        });

        Thread.sleep(500);

        // Publish triggers ConfigChangeNotifyHandler on server
        configService.publishConfig(dataId, DEFAULT_GROUP, "notify.test=value");

        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "ConfigChangeNotifyHandler should push notification");
        assertEquals("notify.test=value", receivedContent.get());

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== Naming gRPC Handler Tests ====================

    /**
     * NG-009: Test InstanceRequestHandler via register/deregister
     */
    @Test
    @Order(9)
    void testInstanceRequestHandler() throws NacosException, InterruptedException {
        String serviceName = "grpc-instance-" + UUID.randomUUID();

        // InstanceRequestHandler handles register
        namingService.registerInstance(serviceName, "192.168.1.10", 8080);
        Thread.sleep(500);

        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertEquals(1, instances.size(), "InstanceRequestHandler should register instance");

        // InstanceRequestHandler handles deregister
        namingService.deregisterInstance(serviceName, "192.168.1.10", 8080);
        Thread.sleep(500);

        instances = namingService.getAllInstances(serviceName);
        assertTrue(instances.isEmpty(), "InstanceRequestHandler should deregister instance");
    }

    /**
     * NG-010: Test BatchInstanceRequestHandler via multiple registrations
     */
    @Test
    @Order(10)
    void testBatchInstanceRequestHandler() throws NacosException, InterruptedException {
        String serviceName = "grpc-batch-instance-" + UUID.randomUUID();

        // Register multiple instances rapidly
        for (int i = 0; i < 5; i++) {
            namingService.registerInstance(serviceName, "192.168.1." + (20 + i), 8080);
        }
        Thread.sleep(1500);

        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertEquals(5, instances.size(), "BatchInstanceRequestHandler should handle multiple instances");

        // Cleanup
        for (int i = 0; i < 5; i++) {
            namingService.deregisterInstance(serviceName, "192.168.1." + (20 + i), 8080);
        }
    }

    /**
     * NG-011: Test ServiceListRequestHandler via getServicesOfServer
     */
    @Test
    @Order(11)
    void testServiceListRequestHandler() throws NacosException, InterruptedException {
        String prefix = "grpc-svclist-" + UUID.randomUUID().toString().substring(0, 8);

        // Register some services
        for (int i = 0; i < 3; i++) {
            namingService.registerInstance(prefix + "-" + i, "192.168.1." + (30 + i), 8080);
        }
        Thread.sleep(1000);

        // ServiceListRequestHandler handles this
        var listView = namingService.getServicesOfServer(1, 100);
        assertNotNull(listView);
        assertTrue(listView.getCount() >= 3, "ServiceListRequestHandler should return services");

        // Cleanup
        for (int i = 0; i < 3; i++) {
            namingService.deregisterInstance(prefix + "-" + i, "192.168.1." + (30 + i), 8080);
        }
    }

    /**
     * NG-012: Test ServiceQueryRequestHandler via getAllInstances
     */
    @Test
    @Order(12)
    void testServiceQueryRequestHandler() throws NacosException, InterruptedException {
        String serviceName = "grpc-query-svc-" + UUID.randomUUID();

        Instance instance = new Instance();
        instance.setIp("192.168.1.40");
        instance.setPort(8080);
        instance.setWeight(1.0);
        instance.setMetadata(Map.of("version", "1.0.0", "region", "us-west"));

        namingService.registerInstance(serviceName, instance);
        Thread.sleep(500);

        // ServiceQueryRequestHandler handles this
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty());

        Instance retrieved = instances.get(0);
        assertEquals("192.168.1.40", retrieved.getIp());
        assertEquals("1.0.0", retrieved.getMetadata().get("version"));

        namingService.deregisterInstance(serviceName, "192.168.1.40", 8080);
    }

    /**
     * NG-013: Test SubscribeServiceRequestHandler via subscribe
     */
    @Test
    @Order(13)
    void testSubscribeServiceRequestHandler() throws NacosException, InterruptedException {
        String serviceName = "grpc-subscribe-" + UUID.randomUUID();
        AtomicReference<List<Instance>> receivedInstances = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        // SubscribeServiceRequestHandler handles this
        EventListener listener = event -> {
            if (event instanceof NamingEvent) {
                List<Instance> instances = ((NamingEvent) event).getInstances();
                System.out.println("Subscription update: " + instances.size() + " instances");
                receivedInstances.set(instances);
                latch.countDown();
            }
        };

        namingService.subscribe(serviceName, listener);
        Thread.sleep(500);

        // Register triggers notification
        namingService.registerInstance(serviceName, "192.168.1.50", 8080);

        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "SubscribeServiceRequestHandler should push updates");
        assertNotNull(receivedInstances.get());

        // Cleanup
        namingService.unsubscribe(serviceName, listener);
        namingService.deregisterInstance(serviceName, "192.168.1.50", 8080);
    }

    // ==================== Connection Management Tests ====================

    /**
     * NG-014: Test connection reconnection
     */
    @Test
    @Order(14)
    void testConnectionReconnection() throws NacosException, InterruptedException {
        String dataId = "grpc-reconnect-" + UUID.randomUUID();

        // Perform operations to ensure connection is established
        configService.publishConfig(dataId, DEFAULT_GROUP, "initial=value");
        Thread.sleep(100);

        // Perform multiple operations to test connection stability
        for (int i = 0; i < 10; i++) {
            configService.publishConfig(dataId, DEFAULT_GROUP, "iteration=" + i);
            String content = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
            assertEquals("iteration=" + i, content);
            Thread.sleep(100);
        }

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NG-015: Test concurrent gRPC operations
     */
    @Test
    @Order(15)
    void testConcurrentGrpcOperations() throws InterruptedException {
        String prefix = "grpc-concurrent-" + UUID.randomUUID().toString().substring(0, 8);
        int threadCount = 10;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(threadCount);
        AtomicReference<Exception> error = new AtomicReference<>();

        for (int i = 0; i < threadCount; i++) {
            final int index = i;
            new Thread(() -> {
                try {
                    startLatch.await();
                    String dataId = prefix + "-" + index;

                    configService.publishConfig(dataId, DEFAULT_GROUP, "concurrent=" + index);
                    String content = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
                    assertEquals("concurrent=" + index, content);
                    configService.removeConfig(dataId, DEFAULT_GROUP);
                } catch (Exception e) {
                    error.set(e);
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        // Start all threads
        startLatch.countDown();

        // Wait for completion
        boolean completed = doneLatch.await(60, TimeUnit.SECONDS);
        assertTrue(completed, "All concurrent operations should complete");
        assertNull(error.get(), "No errors should occur: " + (error.get() != null ? error.get().getMessage() : ""));
    }

    /**
     * NG-016: Test gRPC stream with multiple messages
     */
    @Test
    @Order(16)
    void testGrpcStreamMultipleMessages() throws NacosException, InterruptedException {
        String dataId = "grpc-stream-" + UUID.randomUUID();
        AtomicInteger messageCount = new AtomicInteger(0);
        CountDownLatch latch = new CountDownLatch(5);

        configService.addListener(dataId, DEFAULT_GROUP, new Listener() {
            @Override
            public Executor getExecutor() { return null; }

            @Override
            public void receiveConfigInfo(String configInfo) {
                System.out.println("Stream message #" + messageCount.incrementAndGet() + ": " + configInfo);
                latch.countDown();
            }
        });

        Thread.sleep(500);

        // Publish multiple updates to test stream handling
        for (int i = 0; i < 5; i++) {
            configService.publishConfig(dataId, DEFAULT_GROUP, "stream.message=" + i);
            Thread.sleep(500);
        }

        boolean received = latch.await(30, TimeUnit.SECONDS);
        assertTrue(received, "Should receive all stream messages");
        assertEquals(5, messageCount.get(), "Should receive 5 messages");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }
}
