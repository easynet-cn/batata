package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.config.listener.Listener;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.util.Properties;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Config Service SDK Compatibility Tests
 *
 * Tests Batata's compatibility with official Nacos Java SDK for configuration management.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigServiceTest {

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
        assertNotNull(configService, "ConfigService should be created successfully");
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (configService != null) {
            configService.shutDown();
        }
    }

    // ==================== P0: Critical Tests ====================

    /**
     * NC-001: Test publish configuration
     */
    @Test
    @Order(1)
    void testPublishConfig() throws NacosException {
        String dataId = "nc001-publish-" + UUID.randomUUID();
        String content = "test.key=test.value\ntest.number=123";

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content);
        assertTrue(success, "Publish config should succeed");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NC-002: Test get configuration
     */
    @Test
    @Order(2)
    void testGetConfig() throws NacosException {
        String dataId = "nc002-get-" + UUID.randomUUID();
        String expectedContent = "get.test.key=get.test.value";

        // Publish first
        configService.publishConfig(dataId, DEFAULT_GROUP, expectedContent);

        // Get with timeout
        String retrievedContent = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(expectedContent, retrievedContent, "Retrieved content should match published content");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NC-003: Test delete configuration
     */
    @Test
    @Order(3)
    void testRemoveConfig() throws NacosException {
        String dataId = "nc003-remove-" + UUID.randomUUID();
        String content = "to.be.removed=true";

        // Publish
        configService.publishConfig(dataId, DEFAULT_GROUP, content);

        // Verify exists
        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertNotNull(retrieved);

        // Remove
        boolean removed = configService.removeConfig(dataId, DEFAULT_GROUP);
        assertTrue(removed, "Remove config should succeed");

        // Verify deleted
        String afterRemove = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertNull(afterRemove, "Config should be null after removal");
    }

    /**
     * NC-004: Test configuration listener
     */
    @Test
    @Order(4)
    void testConfigListener() throws NacosException, InterruptedException {
        String dataId = "nc004-listener-" + UUID.randomUUID();
        AtomicReference<String> receivedContent = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        // Add listener first
        configService.addListener(dataId, DEFAULT_GROUP, new Listener() {
            @Override
            public Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                System.out.println("Received config update: " + configInfo);
                receivedContent.set(configInfo);
                latch.countDown();
            }
        });

        // Wait a bit for listener registration
        Thread.sleep(500);

        // Publish triggers notification
        String content = "listener.updated=true";
        configService.publishConfig(dataId, DEFAULT_GROUP, content);

        // Wait for notification
        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "Should receive config notification within timeout");
        assertEquals(content, receivedContent.get(), "Received content should match published content");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== P1: Important Tests ====================

    /**
     * NC-005: Test remove listener
     */
    @Test
    @Order(5)
    void testRemoveListener() throws NacosException, InterruptedException {
        String dataId = "nc005-remove-listener-" + UUID.randomUUID();
        AtomicReference<String> receivedContent = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

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

        // Add then remove listener
        configService.addListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        Thread.sleep(500);

        // Publish should not trigger removed listener
        configService.publishConfig(dataId, DEFAULT_GROUP, "should.not.receive=true");

        // Wait briefly - should NOT receive notification
        boolean received = latch.await(3, TimeUnit.SECONDS);
        assertFalse(received, "Should NOT receive notification after listener removed");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NC-006: Test get config and sign listener
     */
    @Test
    @Order(6)
    void testGetConfigAndSignListener() throws NacosException, InterruptedException {
        String dataId = "nc006-sign-listener-" + UUID.randomUUID();
        String initialContent = "initial.content=true";
        AtomicReference<String> receivedContent = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        // Publish initial
        configService.publishConfig(dataId, DEFAULT_GROUP, initialContent);
        Thread.sleep(500);

        // Get and sign listener
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

        String content = configService.getConfigAndSignListener(dataId, DEFAULT_GROUP, 5000, listener);
        assertEquals(initialContent, content);

        // Update should trigger listener
        Thread.sleep(500);
        String updatedContent = "updated.content=true";
        configService.publishConfig(dataId, DEFAULT_GROUP, updatedContent);

        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "Should receive update notification");
        assertEquals(updatedContent, receivedContent.get());

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NC-007: Test config with namespace
     */
    @Test
    @Order(7)
    void testConfigWithNamespace() throws NacosException {
        String namespace = "test-namespace-" + UUID.randomUUID().toString().substring(0, 8);
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("namespace", namespace);
        properties.put("username", username);
        properties.put("password", password);

        ConfigService nsConfigService = NacosFactory.createConfigService(properties);

        try {
            String dataId = "nc007-namespace-" + UUID.randomUUID();
            String content = "namespace.specific=true";

            // Publish in namespace
            boolean published = nsConfigService.publishConfig(dataId, DEFAULT_GROUP, content);
            assertTrue(published);

            // Get from namespace
            String retrieved = nsConfigService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals(content, retrieved);

            // Should NOT exist in default namespace
            String defaultNsContent = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
            assertNull(defaultNsContent, "Config should not exist in default namespace");

            // Cleanup
            nsConfigService.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            nsConfigService.shutDown();
        }
    }

    /**
     * NC-008: Test config with custom group
     */
    @Test
    @Order(8)
    void testConfigWithGroup() throws NacosException {
        String dataId = "nc008-group-" + UUID.randomUUID();
        String customGroup = "CUSTOM_GROUP";
        String content = "group.specific=true";

        // Publish in custom group
        boolean published = configService.publishConfig(dataId, customGroup, content);
        assertTrue(published);

        // Get from custom group
        String retrieved = configService.getConfig(dataId, customGroup, 5000);
        assertEquals(content, retrieved);

        // Should NOT exist in default group
        String defaultGroupContent = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertNull(defaultGroupContent, "Config should not exist in DEFAULT_GROUP");

        // Cleanup
        configService.removeConfig(dataId, customGroup);
    }

    // ==================== P2: Nice to Have Tests ====================

    /**
     * NC-009: Test large configuration (>100KB)
     */
    @Test
    @Order(9)
    void testLargeConfig() throws NacosException {
        String dataId = "nc009-large-" + UUID.randomUUID();

        // Generate ~100KB content
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < 5000; i++) {
            sb.append("config.key.").append(i).append("=value.").append(i).append("\n");
        }
        String largeContent = sb.toString();
        assertTrue(largeContent.length() > 100000, "Content should be > 100KB");

        // Publish large config
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, largeContent);
        assertTrue(published, "Large config publish should succeed");

        // Retrieve and verify
        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 10000);
        assertEquals(largeContent, retrieved, "Retrieved large content should match");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NC-010: Test concurrent publish
     */
    @Test
    @Order(10)
    void testConcurrentPublish() throws InterruptedException {
        String dataId = "nc010-concurrent-" + UUID.randomUUID();
        int threadCount = 10;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(threadCount);
        AtomicReference<Exception> error = new AtomicReference<>();

        for (int i = 0; i < threadCount; i++) {
            final int index = i;
            new Thread(() -> {
                try {
                    startLatch.await();
                    String content = "concurrent.value=" + index;
                    configService.publishConfig(dataId, DEFAULT_GROUP, content);
                } catch (Exception e) {
                    error.set(e);
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        // Start all threads simultaneously
        startLatch.countDown();

        // Wait for completion
        boolean completed = doneLatch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete");
        assertNull(error.get(), "No errors should occur during concurrent publish");

        // Verify config exists (content will be one of the values)
        try {
            String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertNotNull(retrieved);
            assertTrue(retrieved.startsWith("concurrent.value="));

            // Cleanup
            configService.removeConfig(dataId, DEFAULT_GROUP);
        } catch (NacosException e) {
            fail("Should be able to read config after concurrent writes");
        }
    }
}
