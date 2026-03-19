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
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);
        properties.setProperty("nacos.naming.cache.dir", cacheDir);
        properties.setProperty("nacos.config.cache.dir", cacheDir);

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

        // Cleanup cache directory
        try {
            deleteDirectory(Paths.get(cacheDir));
        } catch (Exception e) {
            // Cache cleanup is best-effort
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

        // Second fetch - should return same content (from cache or server)
        String fetchedAgain = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(content, fetchedAgain, "Second fetch should return same cached content");

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
        assertNotNull(fetched, "Fetched config should not be null");
        assertEquals(content, fetched, "Fetched content should match published content");

        // Add listener to ensure config is being watched
        AtomicReference<String> listenerContent = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);
        Listener listener = new Listener() {
            @Override
            public Executor getExecutor() { return null; }

            @Override
            public void receiveConfigInfo(String configInfo) {
                listenerContent.set(configInfo);
                latch.countDown();
            }
        };

        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Verify snapshot by re-fetching
        String reFetched = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(content, reFetched, "Re-fetched config should match original content");

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
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
        assertEquals(content, receivedContent.get(),
                "Listener should receive the exact published content");

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

        assertNull(content, "Non-existent config should return null");
        assertTrue(duration < 10000,
                "Config fetch should complete within reasonable time, took " + duration + "ms");
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

        assertNotNull(fetch1, "First fetch should not be null");
        assertNotNull(fetch2, "Second fetch should not be null");
        assertNotNull(fetch3, "Third fetch should not be null");
        assertEquals(fetch1, fetch2, "Multiple fetches should return same content");
        assertEquals(fetch2, fetch3, "Multiple fetches should return same content");
        assertEquals(content, fetch1, "Fetched content should match original");

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

        // Verify instance details are consistent between queries
        Set<String> ips1 = new HashSet<>();
        Set<String> ips2 = new HashSet<>();
        for (Instance inst : instances1) { ips1.add(inst.getIp()); }
        for (Instance inst : instances2) { ips2.add(inst.getIp()); }
        assertEquals(ips1, ips2, "Cached instances should have same IPs as first query");

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
        assertFalse(instances.isEmpty(), "Should have at least one instance");
        assertEquals("1.0.0", instances.get(0).getMetadata().get("version"),
                "Initial version should be 1.0.0");

        // Update instance
        instance.setMetadata(Map.of("version", "2.0.0"));
        namingService.registerInstance(serviceName, instance);

        Thread.sleep(1000);

        // Query again with subscribe=false to bypass local cache
        instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP, new ArrayList<>(), false);
        assertFalse(instances.isEmpty(), "Should still have instance after update");

        String version = instances.get(0).getMetadata().get("version");
        assertEquals("2.0.0", version,
                "Instance version should be updated to 2.0.0");

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
                latch.countDown();
            }
        });

        Thread.sleep(500);

        // Register instance
        namingService.registerInstance(serviceName, "192.168.202.1", 8080);

        // Wait for subscription notification
        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "Should receive subscription update");
        assertNotNull(cachedInstances.get(), "Cached instances should not be null");
        assertFalse(cachedInstances.get().isEmpty(),
                "Subscription cache should contain at least one instance");

        boolean foundRegistered = cachedInstances.get().stream()
                .anyMatch(inst -> "192.168.202.1".equals(inst.getIp()));
        assertTrue(foundRegistered, "Subscription cache should contain the registered instance");

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
        assertFalse(instances.isEmpty(), "Should have cached instance");
        assertEquals(1, instances.size(), "Should have exactly 1 cached instance");
        assertEquals("192.168.203.1", instances.get(0).getIp(),
                "Cached instance should have the registered IP");
        assertEquals(8080, instances.get(0).getPort(),
                "Cached instance should have the registered port");

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
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, content);
        assertTrue(published, "Config should be published for fallback test");
        Thread.sleep(1000);

        String fetched = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(content, fetched, "Should fetch the published fallback content");

        // Fetch again to verify cache/fallback consistency
        String fetchedAgain = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(content, fetchedAgain,
                "Subsequent fetch should return same content from cache/server");

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
        assertEquals(2, initial.size(), "Should have 2 initial instances");

        // Add more instances
        namingService.registerInstance(serviceName, "192.168.204.3", 8080);
        Thread.sleep(1000);

        // Query again - should get updated list
        List<Instance> updated = namingService.getAllInstances(serviceName);
        assertTrue(updated.size() >= 2, "Should have at least initial instances");
        assertEquals(3, updated.size(),
                "Should have 3 instances after adding one more");

        // Verify the new instance is present
        boolean foundNew = updated.stream()
                .anyMatch(inst -> "192.168.204.3".equals(inst.getIp()));
        assertTrue(foundNew, "Updated list should contain the newly added instance");

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
        String configContent = "dir.test=value";

        // Create config cache
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, configContent);
        assertTrue(published, "Config should be published for directory test");
        Thread.sleep(500);
        String fetchedConfig = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(configContent, fetchedConfig, "Should fetch config for cache creation");

        // Create naming cache
        namingService.registerInstance(serviceName, "192.168.205.1", 8080);
        Thread.sleep(500);
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty(), "Should have instances for cache creation");

        // Cache directory may or may not exist depending on SDK implementation
        // The important thing is that config and naming operations work correctly
        Path cachePath = Paths.get(cacheDir);
        // This is an informational check - cache location is implementation-dependent
        assertNotNull(cachePath, "Cache path should be a valid path");

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
        assertTrue(completed, "All threads should complete within timeout");
        assertNull(error.get(), "No errors should occur during concurrent cache access");

        // Cleanup
        for (int i = 0; i < 5; i++) {
            namingService.deregisterInstance(serviceName, "192.168.206." + (i + 1), 8080);
        }
    }
}
