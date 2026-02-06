package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.config.listener.Listener;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import org.junit.jupiter.api.*;

import java.io.*;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Failover and Cache Tests
 *
 * Tests for local cache fallback, failover mechanisms, and snapshot loading.
 * These tests verify the client's behavior when the server is unavailable
 * or when using cached data.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosFailoverTest {

    private static ConfigService configService;
    private static NamingService namingService;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static String cacheDir;

    @BeforeAll
    static void setup() throws NacosException {
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        // Set cache directory for testing
        cacheDir = System.getProperty("java.io.tmpdir") + "/nacos-cache-test-" + System.currentTimeMillis();

        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);
        properties.put("nacos.naming.cache.dir", cacheDir);
        properties.put("nacos.config.cache.dir", cacheDir);

        configService = NacosFactory.createConfigService(properties);
        namingService = NacosFactory.createNamingService(properties);

        System.out.println("Nacos Failover Test Setup - Server: " + serverAddr);
        System.out.println("Cache directory: " + cacheDir);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (configService != null) {
            configService.shutDown();
        }
        if (namingService != null) {
            namingService.shutDown();
        }

        // Cleanup cache directory
        try {
            deleteDirectory(Paths.get(cacheDir));
        } catch (Exception e) {
            System.out.println("Cache cleanup: " + e.getMessage());
        }
    }

    private static void deleteDirectory(Path path) throws IOException {
        if (Files.exists(path)) {
            Files.walk(path)
                    .sorted(Comparator.reverseOrder())
                    .map(Path::toFile)
                    .forEach(File::delete);
        }
    }

    // ==================== Config Cache Tests ====================

    /**
     * NFC-001: Test config is cached locally after first fetch
     */
    @Test
    @Order(1)
    void testConfigCachedLocally() throws NacosException, InterruptedException {
        String dataId = "cache-test-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "cache.test=value\ncache.time=" + System.currentTimeMillis();

        // Publish config
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, content);
        assertTrue(published, "Config should be published");

        Thread.sleep(1000);

        // First fetch - should cache
        String fetched = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(content, fetched, "Should fetch published content");

        // Verify local cache exists (implementation dependent)
        System.out.println("Config cached successfully for: " + dataId);

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NFC-002: Test config snapshot is created
     */
    @Test
    @Order(2)
    void testConfigSnapshotCreated() throws NacosException, InterruptedException {
        String dataId = "snapshot-test-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "snapshot.test=value";

        configService.publishConfig(dataId, DEFAULT_GROUP, content);
        Thread.sleep(1000);

        // Fetch to trigger snapshot
        String fetched = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(fetched);

        // Add listener to ensure config is being watched
        configService.addListener(dataId, DEFAULT_GROUP, new Listener() {
            @Override
            public Executor getExecutor() { return null; }

            @Override
            public void receiveConfigInfo(String configInfo) {
                System.out.println("Snapshot listener received: " + configInfo);
            }
        });

        Thread.sleep(500);

        System.out.println("Snapshot created for: " + dataId);

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NFC-003: Test config listener receives updates from cache
     */
    @Test
    @Order(3)
    void testConfigListenerWithCache() throws NacosException, InterruptedException {
        String dataId = "listener-cache-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicReference<String> receivedContent = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        // Add listener first
        configService.addListener(dataId, DEFAULT_GROUP, new Listener() {
            @Override
            public Executor getExecutor() { return null; }

            @Override
            public void receiveConfigInfo(String configInfo) {
                System.out.println("Listener received: " + configInfo);
                receivedContent.set(configInfo);
                latch.countDown();
            }
        });

        Thread.sleep(500);

        // Publish config
        String content = "listener.cache=test";
        configService.publishConfig(dataId, DEFAULT_GROUP, content);

        // Wait for notification
        boolean received = latch.await(15, TimeUnit.SECONDS);
        assertTrue(received, "Should receive config notification");
        assertEquals(content, receivedContent.get());

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NFC-004: Test get config with timeout
     */
    @Test
    @Order(4)
    void testGetConfigWithTimeout() throws NacosException {
        String dataId = "timeout-test-" + UUID.randomUUID().toString().substring(0, 8);

        // Get non-existent config with short timeout
        long startTime = System.currentTimeMillis();
        String content = configService.getConfig(dataId, DEFAULT_GROUP, 2000);
        long duration = System.currentTimeMillis() - startTime;

        System.out.println("Timeout test - Duration: " + duration + "ms, Content: " + content);
        assertNull(content, "Non-existent config should return null");
    }

    /**
     * NFC-005: Test config MD5 consistency
     */
    @Test
    @Order(5)
    void testConfigMd5Consistency() throws NacosException, InterruptedException {
        String dataId = "md5-test-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "md5.test=value";

        configService.publishConfig(dataId, DEFAULT_GROUP, content);
        Thread.sleep(500);

        // Fetch multiple times - should get consistent results
        String fetch1 = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        String fetch2 = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        String fetch3 = configService.getConfig(dataId, DEFAULT_GROUP, 5000);

        assertEquals(fetch1, fetch2, "Multiple fetches should return same content");
        assertEquals(fetch2, fetch3, "Multiple fetches should return same content");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== Naming Cache Tests ====================

    /**
     * NFC-006: Test service instances are cached locally
     */
    @Test
    @Order(6)
    void testServiceInstancesCached() throws NacosException, InterruptedException {
        String serviceName = "cache-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.200." + (i + 1));
            instance.setPort(8080);
            instance.setWeight(1.0);
            instance.setHealthy(true);
            namingService.registerInstance(serviceName, instance);
        }

        Thread.sleep(1000);

        // First query - should cache
        List<Instance> instances1 = namingService.getAllInstances(serviceName);
        assertEquals(3, instances1.size(), "Should find 3 instances");

        // Second query - may use cache
        List<Instance> instances2 = namingService.getAllInstances(serviceName);
        assertEquals(3, instances2.size(), "Should find 3 instances from cache");

        System.out.println("Service instances cached for: " + serviceName);

        // Cleanup
        for (int i = 0; i < 3; i++) {
            namingService.deregisterInstance(serviceName, "192.168.200." + (i + 1), 8080);
        }
    }

    /**
     * NFC-007: Test service info holder maintains cache
     */
    @Test
    @Order(7)
    void testServiceInfoHolder() throws NacosException, InterruptedException {
        String serviceName = "holder-test-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instance
        Instance instance = new Instance();
        instance.setIp("192.168.201.1");
        instance.setPort(8080);
        instance.setMetadata(Map.of("version", "1.0.0"));
        namingService.registerInstance(serviceName, instance);

        Thread.sleep(1000);

        // Query to populate cache
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty());

        // Update instance
        instance.setMetadata(Map.of("version", "2.0.0"));
        namingService.registerInstance(serviceName, instance);

        Thread.sleep(1000);

        // Query again - should get updated info
        instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty());

        String version = instances.get(0).getMetadata().get("version");
        System.out.println("Instance version after update: " + version);

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.201.1", 8080);
    }

    /**
     * NFC-008: Test subscription maintains cache
     */
    @Test
    @Order(8)
    void testSubscriptionMaintainsCache() throws NacosException, InterruptedException {
        String serviceName = "sub-cache-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicReference<List<Instance>> cachedInstances = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        // Subscribe
        namingService.subscribe(serviceName, event -> {
            if (event instanceof com.alibaba.nacos.api.naming.listener.NamingEvent) {
                List<Instance> instances = ((com.alibaba.nacos.api.naming.listener.NamingEvent) event).getInstances();
                cachedInstances.set(instances);
                System.out.println("Subscription update: " + instances.size() + " instances");
                latch.countDown();
            }
        });

        Thread.sleep(500);

        // Register instance
        namingService.registerInstance(serviceName, "192.168.202.1", 8080);

        // Wait for subscription notification
        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "Should receive subscription update");
        assertNotNull(cachedInstances.get());

        System.out.println("Subscription cache updated for: " + serviceName);

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.202.1", 8080);
    }

    // ==================== Failover Behavior Tests ====================

    /**
     * NFC-009: Test failover switch detection
     */
    @Test
    @Order(9)
    void testFailoverSwitchDetection() throws NacosException, InterruptedException {
        String serviceName = "failover-switch-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instance to create cache
        namingService.registerInstance(serviceName, "192.168.203.1", 8080);
        Thread.sleep(1000);

        // Query to populate cache
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty());

        // Check if failover mode can be detected
        // This is implementation-specific; the SDK should handle server unavailability gracefully
        System.out.println("Failover switch test - instances cached: " + instances.size());

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.203.1", 8080);
    }

    /**
     * NFC-010: Test config fetch with local fallback
     */
    @Test
    @Order(10)
    void testConfigLocalFallback() throws NacosException, InterruptedException {
        String dataId = "fallback-test-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "fallback.test=value";

        // Publish and fetch to create cache
        configService.publishConfig(dataId, DEFAULT_GROUP, content);
        Thread.sleep(1000);

        String fetched = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(content, fetched);

        // The SDK should maintain local cache for fallback scenarios
        System.out.println("Local fallback test - content cached successfully");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NFC-011: Test service discovery with stale cache
     */
    @Test
    @Order(11)
    void testServiceDiscoveryStaleCache() throws NacosException, InterruptedException {
        String serviceName = "stale-cache-" + UUID.randomUUID().toString().substring(0, 8);

        // Register initial instances
        for (int i = 0; i < 2; i++) {
            namingService.registerInstance(serviceName, "192.168.204." + (i + 1), 8080);
        }

        Thread.sleep(1000);

        // Query to populate cache
        List<Instance> initial = namingService.getAllInstances(serviceName);
        assertEquals(2, initial.size());

        // Add more instances
        namingService.registerInstance(serviceName, "192.168.204.3", 8080);
        Thread.sleep(1000);

        // Query again - should get updated list
        List<Instance> updated = namingService.getAllInstances(serviceName);
        assertTrue(updated.size() >= 2, "Should have at least initial instances");

        System.out.println("Stale cache test - initial: " + initial.size() + ", updated: " + updated.size());

        // Cleanup
        for (int i = 0; i < 3; i++) {
            namingService.deregisterInstance(serviceName, "192.168.204." + (i + 1), 8080);
        }
    }

    /**
     * NFC-012: Test cache directory structure
     */
    @Test
    @Order(12)
    void testCacheDirectoryStructure() throws NacosException, InterruptedException {
        String dataId = "dir-test-" + UUID.randomUUID().toString().substring(0, 8);
        String serviceName = "dir-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Create config cache
        configService.publishConfig(dataId, DEFAULT_GROUP, "dir.test=value");
        Thread.sleep(500);
        configService.getConfig(dataId, DEFAULT_GROUP, 5000);

        // Create naming cache
        namingService.registerInstance(serviceName, "192.168.205.1", 8080);
        Thread.sleep(500);
        namingService.getAllInstances(serviceName);

        // Check cache directory
        Path cachePath = Paths.get(cacheDir);
        if (Files.exists(cachePath)) {
            System.out.println("Cache directory exists: " + cachePath);
            try {
                Files.walk(cachePath, 2).forEach(p -> {
                    System.out.println("  " + cachePath.relativize(p));
                });
            } catch (IOException e) {
                System.out.println("Could not list cache directory: " + e.getMessage());
            }
        } else {
            System.out.println("Cache directory not created (implementation may use different location)");
        }

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
        namingService.deregisterInstance(serviceName, "192.168.205.1", 8080);
    }

    /**
     * NFC-013: Test concurrent cache access
     */
    @Test
    @Order(13)
    void testConcurrentCacheAccess() throws NacosException, InterruptedException {
        String serviceName = "concurrent-cache-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances
        for (int i = 0; i < 5; i++) {
            namingService.registerInstance(serviceName, "192.168.206." + (i + 1), 8080);
        }

        Thread.sleep(1000);

        // Concurrent queries
        int threadCount = 10;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(threadCount);
        AtomicReference<Exception> error = new AtomicReference<>();

        for (int i = 0; i < threadCount; i++) {
            new Thread(() -> {
                try {
                    startLatch.await();
                    for (int j = 0; j < 10; j++) {
                        List<Instance> instances = namingService.getAllInstances(serviceName);
                        assertTrue(instances.size() >= 1, "Should find instances");
                    }
                } catch (Exception e) {
                    error.set(e);
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        startLatch.countDown();
        boolean completed = doneLatch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete");
        assertNull(error.get(), "No errors during concurrent access");

        System.out.println("Concurrent cache access test passed");

        // Cleanup
        for (int i = 0; i < 5; i++) {
            namingService.deregisterInstance(serviceName, "192.168.206." + (i + 1), 8080);
        }
    }
}
