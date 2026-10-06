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
 * Nacos Config Change Notification Tests
 *
 * Tests for config change notification semantics via SDK listeners.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigChangeNotifyTest {

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

    // ==================== Config Change Notification Tests ====================

    /**
     * CCN-001: Test listener fires on config add (first publish)
     */
    @Test
    @Order(1)
    void testListenerOnConfigAdd() throws NacosException, InterruptedException {
        String dataId = "ccn-add-" + UUID.randomUUID().toString().substring(0, 8);
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<String> receivedContent = new AtomicReference<>();

        Listener listener = createListener(configInfo -> {
            receivedContent.set(configInfo);
            latch.countDown();
        });

        configService.addListener(dataId, DEFAULT_GROUP, listener);

        // First publish should trigger listener
        String content = "ccn.add.key=first-value";
        configService.publishConfig(dataId, DEFAULT_GROUP, content);

        boolean received = latch.await(10, TimeUnit.SECONDS);
        assertTrue(received, "Listener should fire when config is first published");
        assertNotNull(receivedContent.get(), "Should receive config content");
        assertEquals(content, receivedContent.get(), "Should receive the published content");

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CCN-002: Test listener fires on config modify
     */
    @Test
    @Order(2)
    void testListenerOnConfigModify() throws NacosException, InterruptedException {
        String dataId = "ccn-modify-" + UUID.randomUUID().toString().substring(0, 8);
        CountDownLatch addLatch = new CountDownLatch(1);
        CountDownLatch modifyLatch = new CountDownLatch(1);
        AtomicReference<String> lastContent = new AtomicReference<>();
        AtomicInteger callCount = new AtomicInteger(0);

        Listener listener = createListener(configInfo -> {
            lastContent.set(configInfo);
            int count = callCount.incrementAndGet();
            if (count == 1) {
                addLatch.countDown();
            } else if (count >= 2) {
                modifyLatch.countDown();
            }
        });

        // Register the listener BEFORE the config exists. When a listener is registered
        // after the config is already published, the client starts from the content/md5 it
        // already knows, so nacos delivers no initial callback in that case — only later
        // changes fire (hence a single notification for add+modify collapsed together).
        // Registering first is also how the sibling CCN-001 case (testListenerOnConfigAdd)
        // observes its "add" notification.
        // Note: no waitForConfigPresent() here — an earlier getConfig() would settle the
        // client's md5 and suppress exactly this initial notification.
        configService.addListener(dataId, DEFAULT_GROUP, listener);

        // Publish initial config
        configService.publishConfig(dataId, DEFAULT_GROUP, "ccn.modify=initial");
        assertTrue(addLatch.await(10, TimeUnit.SECONDS),
                "Listener should fire when config is first published");

        // Modify config
        String modifiedContent = "ccn.modify=updated";
        configService.publishConfig(dataId, DEFAULT_GROUP, modifiedContent);

        boolean received = modifyLatch.await(15, TimeUnit.SECONDS);
        assertTrue(received, "Listener should fire when config is modified");
        assertTrue(callCount.get() >= 2,
                "Listener should have been called at least twice (add + modify), got: " + callCount.get());
        assertEquals(modifiedContent, lastContent.get(),
                "Should receive modified content as the last notification");

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CCN-003: Test listener fires on config delete
     */
    @Test
    @Order(3)
    void testListenerOnConfigDelete() throws NacosException, InterruptedException {
        String dataId = "ccn-delete-" + UUID.randomUUID().toString().substring(0, 8);
        String createdContent = "ccn.delete=to-be-removed";
        CountDownLatch createLatch = new CountDownLatch(1);
        CountDownLatch deleteLatch = new CountDownLatch(1);
        AtomicReference<String> lastContent = new AtomicReference<>("initial");
        AtomicInteger callCount = new AtomicInteger(0);

        Listener listener = createListener(configInfo -> {
            lastContent.set(configInfo);
            int count = callCount.incrementAndGet();
            if (count == 1) {
                createLatch.countDown();
            } else if (count >= 2) {
                deleteLatch.countDown();
            }
        });

        configService.addListener(dataId, DEFAULT_GROUP, listener);

        // Create config
        configService.publishConfig(dataId, DEFAULT_GROUP, createdContent);
        createLatch.await(10, TimeUnit.SECONDS);
        TestSupport.waitFor(() -> createdContent.equals(lastContent.get()), 2000);

        // Delete config
        configService.removeConfig(dataId, DEFAULT_GROUP);

        boolean deleteReceived = deleteLatch.await(10, TimeUnit.SECONDS);
        assertTrue(deleteReceived, "Listener should fire when config is deleted");

        // After delete, content should be null or empty
        String content = lastContent.get();
        assertTrue(content == null || content.isEmpty(),
                "Content should be null or empty after delete, but was: " + content);

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
    }

    /**
     * CCN-004: Test multiple listeners on same config all fire
     */
    @Test
    @Order(4)
    void testMultipleListenersOnSameConfig() throws NacosException, InterruptedException {
        String dataId = "ccn-multi-" + UUID.randomUUID().toString().substring(0, 8);
        int listenerCount = 3;
        CountDownLatch latch = new CountDownLatch(listenerCount);
        List<Listener> listeners = new ArrayList<>();
        List<AtomicReference<String>> receivedContents = new ArrayList<>();

        for (int i = 0; i < listenerCount; i++) {
            AtomicReference<String> content = new AtomicReference<>();
            receivedContents.add(content);

            Listener listener = createListener(configInfo -> {
                content.set(configInfo);
                latch.countDown();
            });
            listeners.add(listener);
            configService.addListener(dataId, DEFAULT_GROUP, listener);
        }

        // Publish config
        String expectedContent = "ccn.multi=all-listeners-should-fire";
        configService.publishConfig(dataId, DEFAULT_GROUP, expectedContent);

        boolean allReceived = latch.await(15, TimeUnit.SECONDS);
        assertTrue(allReceived, "All " + listenerCount + " listeners should fire");

        for (int i = 0; i < listenerCount; i++) {
            assertEquals(expectedContent, receivedContents.get(i).get(),
                    "Listener " + i + " should receive correct content");
        }

        // Cleanup
        for (Listener listener : listeners) {
            configService.removeListener(dataId, DEFAULT_GROUP, listener);
        }
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CCN-005: Test removed listener does not fire
     */
    @Test
    @Order(5)
    void testListenerNotFiresAfterRemoval() throws NacosException, InterruptedException {
        String dataId = "ccn-removed-" + UUID.randomUUID().toString().substring(0, 8);
        AtomicInteger callCount = new AtomicInteger(0);

        Listener listener = createListener(configInfo -> {
            callCount.incrementAndGet();
        });

        configService.addListener(dataId, DEFAULT_GROUP, listener);

        // Trigger once
        configService.publishConfig(dataId, DEFAULT_GROUP, "ccn.removed=first");
        TestSupport.waitFor(() -> callCount.get() >= 1, 3000);

        int countAfterFirst = callCount.get();
        assertTrue(countAfterFirst >= 1,
                "Should have received at least 1 notification before removal, got: " + countAfterFirst);

        // Remove listener
        configService.removeListener(dataId, DEFAULT_GROUP, listener);

        // Publish again - should NOT trigger
        configService.publishConfig(dataId, DEFAULT_GROUP, "ccn.removed=second");
        assertFalse(TestSupport.waitFor(() -> callCount.get() != countAfterFirst, 3000),
                "Removed listener should not receive further notifications");

        int countAfterSecond = callCount.get();
        assertEquals(countAfterFirst, countAfterSecond,
                "Removed listener should not receive further notifications");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CCN-006: Test listener handles rapid config changes
     */
    @Test
    @Order(6)
    void testRapidConfigChanges() throws NacosException, InterruptedException {
        String dataId = "ccn-rapid-" + UUID.randomUUID().toString().substring(0, 8);
        int changeCount = 10;
        AtomicInteger receiveCount = new AtomicInteger(0);
        CountDownLatch latch = new CountDownLatch(1);

        Listener listener = createListener(configInfo -> {
            int count = receiveCount.incrementAndGet();
            if (count >= changeCount) {
                latch.countDown();
            }
        });

        configService.addListener(dataId, DEFAULT_GROUP, listener);

        // Rapid config changes
        for (int i = 0; i < changeCount; i++) {
            configService.publishConfig(dataId, DEFAULT_GROUP, "ccn.rapid.change=" + i);
            Thread.sleep(200); // Small delay between changes
        }

        // Wait for notifications
        latch.await(30, TimeUnit.SECONDS);

        int finalCount = receiveCount.get();
        // Should receive at least some notifications (may coalesce rapid changes)
        assertTrue(finalCount >= 1,
                "Should receive at least one notification for rapid changes, got: " + finalCount);
        assertTrue(finalCount <= changeCount * 2,
                "Should not receive more notifications than changes (with some tolerance), got: " + finalCount);

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CCN-007: Test listener receives latest value after multiple changes
     */
    @Test
    @Order(7)
    void testListenerReceivesLatestValue() throws NacosException, InterruptedException {
        String dataId = "ccn-latest-" + UUID.randomUUID().toString().substring(0, 8);
        int changeCount = 5;
        AtomicReference<String> lastReceived = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        Listener listener = createListener(configInfo -> {
            lastReceived.set(configInfo);
            // Only count down when we get the final value
            if (configInfo != null && configInfo.contains("change=" + (changeCount - 1))) {
                latch.countDown();
            }
        });

        configService.addListener(dataId, DEFAULT_GROUP, listener);

        // Publish multiple changes, waiting for each notification before the next
        String finalContent = null;
        for (int i = 0; i < changeCount; i++) {
            final int expectedChange = i;
            finalContent = "ccn.latest.change=" + i;
            configService.publishConfig(dataId, DEFAULT_GROUP, finalContent);
            TestSupport.waitFor(() -> lastReceived.get() != null
                    && lastReceived.get().contains("change=" + expectedChange), 5000);
        }

        boolean received = latch.await(60, TimeUnit.SECONDS);
        assertTrue(received, "Should eventually receive the final config value");
        assertNotNull(lastReceived.get(), "Last received content should not be null");
        assertEquals(finalContent, lastReceived.get(),
                "Last received value should match the final published content");

        // Cleanup
        configService.removeListener(dataId, DEFAULT_GROUP, listener);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== Helper Methods ====================

    private interface ConfigCallback {
        void onReceive(String configInfo);
    }

    private Listener createListener(ConfigCallback callback) {
        return new Listener() {
            @Override
            public java.util.concurrent.Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                callback.onReceive(configInfo);
            }
        };
    }
}
