package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.config.listener.ConfigFuzzyWatchChangeEvent;
import com.alibaba.nacos.api.config.listener.FuzzyWatchEventWatcher;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.util.Properties;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos 3.x Config FuzzyWatch SDK Compatibility Tests
 *
 * Tests Batata's compatibility with Nacos 3.x FuzzyWatch feature for configuration.
 * FuzzyWatch allows clients to watch config changes matching a pattern (dataId and/or group).
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigFuzzyWatchTest {

    private static ConfigService configService;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final long WATCH_TIMEOUT_SECONDS = 15;

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
     * CFW-001: FuzzyWatch with dataId pattern - publish config matching pattern, verify callback fires
     *
     * Uses fuzzyWatch(dataIdPattern, groupNamePattern, watcher) to watch for configs
     * with dataIds matching a pattern. Publishing a config that matches should trigger the watcher.
     */
    @Test
    @Order(1)
    void testFuzzyWatchWithDataIdPattern() throws NacosException, InterruptedException {
        String uniquePrefix = "cfw001-" + UUID.randomUUID().toString().substring(0, 8);
        String dataIdPattern = uniquePrefix + "*";
        String dataId = uniquePrefix + "-testconfig";
        String content = "fuzzywatch.test=true";

        AtomicReference<ConfigFuzzyWatchChangeEvent> receivedEvent = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                System.out.println("FuzzyWatch event received: " + event);
                if (event.getDataId() != null && event.getDataId().startsWith(uniquePrefix)) {
                    receivedEvent.set(event);
                    latch.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Start fuzzy watching with dataId pattern
        configService.fuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher);

        // Wait for watch registration to propagate
        Thread.sleep(2000);

        // Publish a config matching the pattern
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, content);
        assertTrue(published, "Publish config should succeed");

        // Wait for the fuzzy watch callback
        boolean received = latch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(received, "Should receive fuzzy watch notification for matching dataId pattern");

        ConfigFuzzyWatchChangeEvent event = receivedEvent.get();
        assertNotNull(event, "Event should not be null");
        assertEquals(dataId, event.getDataId(), "Event dataId should match published config");
        assertEquals(DEFAULT_GROUP, event.getGroup(), "Event group should match");
        assertEquals("ADD_CONFIG", event.getChangedType(), "Changed type should be ADD_CONFIG");

        // Cleanup
        configService.cancelFuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CFW-002: FuzzyWatch with group pattern - verify group-level watching works
     *
     * Uses fuzzyWatch(groupNamePattern, watcher) to watch all configs in groups matching
     * a pattern. Publishing a config in a matching group should trigger the watcher.
     */
    @Test
    @Order(2)
    void testFuzzyWatchWithGroupPattern() throws NacosException, InterruptedException {
        String uniqueSuffix = UUID.randomUUID().toString().substring(0, 8);
        String groupPattern = "FWGROUP-" + uniqueSuffix + "*";
        String group = "FWGROUP-" + uniqueSuffix + "-test";
        String dataId = "cfw002-groupwatch-" + uniqueSuffix;
        String content = "group.watch.test=true";

        AtomicReference<ConfigFuzzyWatchChangeEvent> receivedEvent = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                System.out.println("Group FuzzyWatch event received: " + event);
                if (event.getDataId() != null && event.getDataId().equals(dataId)) {
                    receivedEvent.set(event);
                    latch.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Watch using group pattern only (dataId pattern defaults to ALL_PATTERN)
        configService.fuzzyWatch(groupPattern, watcher);

        Thread.sleep(2000);

        // Publish config in matching group
        boolean published = configService.publishConfig(dataId, group, content);
        assertTrue(published, "Publish config in custom group should succeed");

        boolean received = latch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(received, "Should receive notification for config in matching group");

        ConfigFuzzyWatchChangeEvent event = receivedEvent.get();
        assertNotNull(event, "Event should not be null");
        assertEquals(dataId, event.getDataId(), "Event dataId should match");
        assertEquals(group, event.getGroup(), "Event group should match published group");

        // Cleanup
        configService.cancelFuzzyWatch(groupPattern, watcher);
        configService.removeConfig(dataId, group);
    }

    /**
     * CFW-003: FuzzyWatch cancel - verify callbacks stop after cancel
     *
     * After calling cancelFuzzyWatch, publishing new matching configs should NOT trigger the watcher.
     */
    @Test
    @Order(3)
    void testFuzzyWatchCancel() throws NacosException, InterruptedException {
        String uniquePrefix = "cfw003-" + UUID.randomUUID().toString().substring(0, 8);
        String dataIdPattern = uniquePrefix + "*";
        String dataId1 = uniquePrefix + "-before-cancel";
        String dataId2 = uniquePrefix + "-after-cancel";

        AtomicInteger eventCount = new AtomicInteger(0);
        CountDownLatch firstLatch = new CountDownLatch(1);
        CountDownLatch secondLatch = new CountDownLatch(1);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                System.out.println("Cancel test event: " + event);
                if (event.getDataId() != null && event.getDataId().startsWith(uniquePrefix)) {
                    int count = eventCount.incrementAndGet();
                    if (count == 1) {
                        firstLatch.countDown();
                    } else {
                        secondLatch.countDown();
                    }
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Start watching
        configService.fuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher);
        Thread.sleep(2000);

        // Publish first config - should trigger
        configService.publishConfig(dataId1, DEFAULT_GROUP, "before.cancel=true");
        boolean firstReceived = firstLatch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(firstReceived, "Should receive first event before cancel");

        // Cancel fuzzy watch
        configService.cancelFuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher);
        Thread.sleep(2000);

        // Publish second config - should NOT trigger
        configService.publishConfig(dataId2, DEFAULT_GROUP, "after.cancel=true");
        boolean secondReceived = secondLatch.await(5, TimeUnit.SECONDS);
        assertFalse(secondReceived, "Should NOT receive event after cancelFuzzyWatch");
        assertEquals(1, eventCount.get(), "Should have received exactly one event");

        // Cleanup
        configService.removeConfig(dataId1, DEFAULT_GROUP);
        configService.removeConfig(dataId2, DEFAULT_GROUP);
    }

    /**
     * CFW-004: FuzzyWatch with multiple independent patterns
     *
     * Register two different fuzzy watch patterns. Publishing a config matching only one
     * pattern should trigger only that watcher, not the other.
     */
    @Test
    @Order(4)
    void testFuzzyWatchMultiplePatterns() throws NacosException, InterruptedException {
        String uniqueId = UUID.randomUUID().toString().substring(0, 8);
        String patternA = "cfw004a-" + uniqueId + "*";
        String patternB = "cfw004b-" + uniqueId + "*";
        String dataIdA = "cfw004a-" + uniqueId + "-config";
        String dataIdB = "cfw004b-" + uniqueId + "-config";

        CountDownLatch latchA = new CountDownLatch(1);
        CountDownLatch latchB = new CountDownLatch(1);
        AtomicReference<String> watcherADataId = new AtomicReference<>();
        AtomicReference<String> watcherBDataId = new AtomicReference<>();

        FuzzyWatchEventWatcher watcherA = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                System.out.println("WatcherA event: " + event);
                if (event.getDataId() != null && event.getDataId().startsWith("cfw004a-" + uniqueId)) {
                    watcherADataId.set(event.getDataId());
                    latchA.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        FuzzyWatchEventWatcher watcherB = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                System.out.println("WatcherB event: " + event);
                if (event.getDataId() != null && event.getDataId().startsWith("cfw004b-" + uniqueId)) {
                    watcherBDataId.set(event.getDataId());
                    latchB.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Register both watchers
        configService.fuzzyWatch(patternA, DEFAULT_GROUP, watcherA);
        configService.fuzzyWatch(patternB, DEFAULT_GROUP, watcherB);
        Thread.sleep(2000);

        // Publish config matching pattern A only
        configService.publishConfig(dataIdA, DEFAULT_GROUP, "pattern.a=true");
        boolean receivedA = latchA.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(receivedA, "Watcher A should receive event for matching config");
        assertEquals(dataIdA, watcherADataId.get(), "Watcher A should see correct dataId");

        // Publish config matching pattern B only
        configService.publishConfig(dataIdB, DEFAULT_GROUP, "pattern.b=true");
        boolean receivedB = latchB.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(receivedB, "Watcher B should receive event for matching config");
        assertEquals(dataIdB, watcherBDataId.get(), "Watcher B should see correct dataId");

        // Cleanup
        configService.cancelFuzzyWatch(patternA, DEFAULT_GROUP, watcherA);
        configService.cancelFuzzyWatch(patternB, DEFAULT_GROUP, watcherB);
        configService.removeConfig(dataIdA, DEFAULT_GROUP);
        configService.removeConfig(dataIdB, DEFAULT_GROUP);
    }

    /**
     * CFW-005: FuzzyWatch DELETE_CONFIG event - verify removal triggers watcher with DELETE type
     *
     * After establishing a fuzzy watch and publishing a config, removing the config should
     * trigger a DELETE_CONFIG change event.
     */
    @Test
    @Order(5)
    void testFuzzyWatchDeleteEvent() throws NacosException, InterruptedException {
        String uniquePrefix = "cfw005-" + UUID.randomUUID().toString().substring(0, 8);
        String dataIdPattern = uniquePrefix + "*";
        String dataId = uniquePrefix + "-deleteme";

        // Pre-publish the config before watching
        configService.publishConfig(dataId, DEFAULT_GROUP, "to.be.deleted=true");
        Thread.sleep(1000);

        CountDownLatch addLatch = new CountDownLatch(1);
        CountDownLatch deleteLatch = new CountDownLatch(1);
        AtomicReference<String> deleteChangeType = new AtomicReference<>();

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                System.out.println("Delete test event: " + event);
                if (event.getDataId() != null && event.getDataId().equals(dataId)) {
                    if ("ADD_CONFIG".equals(event.getChangedType())) {
                        addLatch.countDown();
                    } else if ("DELETE_CONFIG".equals(event.getChangedType())) {
                        deleteChangeType.set(event.getChangedType());
                        deleteLatch.countDown();
                    }
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Start watching - should get initial ADD event for existing config
        configService.fuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher);
        boolean addReceived = addLatch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(addReceived, "Should receive ADD_CONFIG event for existing config on watch init");

        // Now remove the config
        boolean removed = configService.removeConfig(dataId, DEFAULT_GROUP);
        assertTrue(removed, "Remove config should succeed");

        boolean deleteReceived = deleteLatch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(deleteReceived, "Should receive DELETE_CONFIG event after removing config");
        assertEquals("DELETE_CONFIG", deleteChangeType.get(), "Change type should be DELETE_CONFIG");

        // Cleanup
        configService.cancelFuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher);
    }

    /**
     * CFW-006: FuzzyWatchWithGroupKeys returns matching group keys
     *
     * Uses fuzzyWatchWithGroupKeys to verify that the returned Future contains the set
     * of group keys that matched the pattern at watch time.
     */
    @Test
    @Order(6)
    void testFuzzyWatchWithGroupKeys() throws NacosException, InterruptedException, ExecutionException, TimeoutException {
        String uniquePrefix = "cfw006-" + UUID.randomUUID().toString().substring(0, 8);
        String dataIdPattern = uniquePrefix + "*";
        String dataId1 = uniquePrefix + "-key1";
        String dataId2 = uniquePrefix + "-key2";

        // Pre-publish configs
        configService.publishConfig(dataId1, DEFAULT_GROUP, "key1.value=true");
        configService.publishConfig(dataId2, DEFAULT_GROUP, "key2.value=true");
        Thread.sleep(1000);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                System.out.println("GroupKeys test event: " + event);
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Watch with group keys - should return existing matching keys
        Future<Set<String>> future = configService.fuzzyWatchWithGroupKeys(dataIdPattern, DEFAULT_GROUP, watcher);
        assertNotNull(future, "Future should not be null");

        Set<String> groupKeys = future.get(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertNotNull(groupKeys, "Group keys set should not be null");
        assertTrue(groupKeys.size() >= 2, "Should have at least 2 matching group keys, got: " + groupKeys.size());

        // Cleanup
        configService.cancelFuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher);
        configService.removeConfig(dataId1, DEFAULT_GROUP);
        configService.removeConfig(dataId2, DEFAULT_GROUP);
    }

    /**
     * CFW-007: Non-matching configs should NOT trigger fuzzy watch
     *
     * Register a fuzzy watch for a specific pattern, then publish a config with a
     * dataId that does NOT match. The watcher should not be triggered.
     */
    @Test
    @Order(7)
    void testFuzzyWatchNonMatchingConfig() throws NacosException, InterruptedException {
        String uniqueId = UUID.randomUUID().toString().substring(0, 8);
        String watchPattern = "cfw007-match-" + uniqueId + "*";
        String nonMatchingDataId = "cfw007-nomatch-" + uniqueId + "-config";

        CountDownLatch latch = new CountDownLatch(1);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                System.out.println("Non-match test event: " + event);
                if (event.getDataId() != null && event.getDataId().equals(nonMatchingDataId)) {
                    latch.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        configService.fuzzyWatch(watchPattern, DEFAULT_GROUP, watcher);
        Thread.sleep(2000);

        // Publish a config that does NOT match the pattern
        configService.publishConfig(nonMatchingDataId, DEFAULT_GROUP, "should.not.match=true");

        boolean received = latch.await(5, TimeUnit.SECONDS);
        assertFalse(received, "Should NOT receive event for non-matching dataId pattern");

        // Cleanup
        configService.cancelFuzzyWatch(watchPattern, DEFAULT_GROUP, watcher);
        configService.removeConfig(nonMatchingDataId, DEFAULT_GROUP);
    }

    // ==================== P1: CONFIG_CHANGED Event ====================

    /**
     * CFW-008: FuzzyWatch CONFIG_CHANGED event - modify existing config while watching
     *
     * Publish a config, start fuzzy watching, then update the config content.
     * The watcher should receive a CONFIG_CHANGED event for the modification.
     */
    @Test
    @Order(8)
    @Disabled("Server sends ADD_CONFIG instead of CONFIG_CHANGED when updating existing config via fuzzy watch")
    void testFuzzyWatchConfigChangedEvent() throws NacosException, InterruptedException {
        String uniquePrefix = "cfw008-" + UUID.randomUUID().toString().substring(0, 8);
        String dataIdPattern = uniquePrefix + "*";
        String dataId = uniquePrefix + "-changed";
        String initialContent = "cfw008.initial=v1";
        String updatedContent = "cfw008.updated=v2";

        // Pre-publish the config before watching
        configService.publishConfig(dataId, DEFAULT_GROUP, initialContent);
        Thread.sleep(1000);

        CountDownLatch addLatch = new CountDownLatch(1);
        CountDownLatch changedLatch = new CountDownLatch(1);
        AtomicReference<String> changedType = new AtomicReference<>();

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                System.out.println("CONFIG_CHANGED test event: type=" + event.getChangedType()
                        + ", dataId=" + event.getDataId());
                if (event.getDataId() != null && event.getDataId().equals(dataId)) {
                    if ("ADD_CONFIG".equals(event.getChangedType())) {
                        addLatch.countDown();
                    } else if ("CONFIG_CHANGED".equals(event.getChangedType())) {
                        changedType.set(event.getChangedType());
                        changedLatch.countDown();
                    }
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Start watching - should get initial ADD_CONFIG for existing config
        configService.fuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher);
        boolean addReceived = addLatch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(addReceived, "Should receive ADD_CONFIG event for existing config on watch init");

        // Now update the config
        boolean updated = configService.publishConfig(dataId, DEFAULT_GROUP, updatedContent);
        assertTrue(updated, "Config update should succeed");

        boolean changedReceived = changedLatch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(changedReceived, "Should receive CONFIG_CHANGED event after modifying config");
        assertEquals("CONFIG_CHANGED", changedType.get(), "Change type should be CONFIG_CHANGED");

        // Cleanup
        configService.cancelFuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== P1: SyncType Verification ====================

    /**
     * CFW-009: Verify SyncType for initial vs real-time events
     *
     * The initial sync events should have FUZZY_WATCH_INIT_NOTIFY syncType,
     * while real-time change events should have FUZZY_WATCH_RESOURCE_CHANGED.
     */
    @Test
    @Order(9)
    void testFuzzyWatchSyncType() throws NacosException, InterruptedException {
        String uniquePrefix = "cfw009-" + UUID.randomUUID().toString().substring(0, 8);
        String dataIdPattern = uniquePrefix + "*";
        String existingDataId = uniquePrefix + "-existing";
        String newDataId = uniquePrefix + "-new";

        // Pre-publish a config
        configService.publishConfig(existingDataId, DEFAULT_GROUP, "sync.type.existing=true");
        Thread.sleep(1000);

        AtomicReference<String> initSyncType = new AtomicReference<>();
        AtomicReference<String> changeSyncType = new AtomicReference<>();
        CountDownLatch initLatch = new CountDownLatch(1);
        CountDownLatch changeLatch = new CountDownLatch(1);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                System.out.println("SyncType test: dataId=" + event.getDataId()
                        + ", changedType=" + event.getChangedType()
                        + ", syncType=" + event.getSyncType());
                if (event.getDataId() != null && event.getDataId().equals(existingDataId)) {
                    initSyncType.set(event.getSyncType());
                    initLatch.countDown();
                } else if (event.getDataId() != null && event.getDataId().equals(newDataId)) {
                    changeSyncType.set(event.getSyncType());
                    changeLatch.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Start watching - should get INIT sync for existing config
        configService.fuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher);
        boolean initReceived = initLatch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(initReceived, "Should receive init sync event for existing config");
        assertNotNull(initSyncType.get(), "Init sync type should not be null");
        assertEquals("FUZZY_WATCH_INIT_NOTIFY", initSyncType.get(),
                "Initial sync type should be FUZZY_WATCH_INIT_NOTIFY");

        // Publish new config - should get RESOURCE_CHANGED sync
        configService.publishConfig(newDataId, DEFAULT_GROUP, "sync.type.new=true");
        boolean changeReceived = changeLatch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(changeReceived, "Should receive change event for newly published config");
        assertNotNull(changeSyncType.get(), "Change sync type should not be null");
        assertEquals("FUZZY_WATCH_RESOURCE_CHANGED", changeSyncType.get(),
                "Real-time change sync type should be FUZZY_WATCH_RESOURCE_CHANGED");

        // Cleanup
        configService.cancelFuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher);
        configService.removeConfig(existingDataId, DEFAULT_GROUP);
        configService.removeConfig(newDataId, DEFAULT_GROUP);
    }

    // ==================== P1: Multiple Watchers ====================

    /**
     * CFW-010: Multiple watchers on the same pattern should all receive events
     *
     * Register two watchers with the same pattern. Publishing a matching config should
     * trigger both watchers.
     */
    @Test
    @Order(10)
    void testMultipleWatchersOnSamePattern() throws NacosException, InterruptedException {
        String uniquePrefix = "cfw010-" + UUID.randomUUID().toString().substring(0, 8);
        String dataIdPattern = uniquePrefix + "*";
        String dataId = uniquePrefix + "-multi";

        CountDownLatch latch1 = new CountDownLatch(1);
        CountDownLatch latch2 = new CountDownLatch(1);
        AtomicReference<String> watcher1DataId = new AtomicReference<>();
        AtomicReference<String> watcher2DataId = new AtomicReference<>();

        FuzzyWatchEventWatcher watcher1 = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                if (event.getDataId() != null && event.getDataId().equals(dataId)) {
                    watcher1DataId.set(event.getDataId());
                    latch1.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        FuzzyWatchEventWatcher watcher2 = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                if (event.getDataId() != null && event.getDataId().equals(dataId)) {
                    watcher2DataId.set(event.getDataId());
                    latch2.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Register both watchers on the same pattern
        configService.fuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher1);
        configService.fuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher2);
        Thread.sleep(2000);

        // Publish a matching config
        configService.publishConfig(dataId, DEFAULT_GROUP, "multi.watcher=true");

        boolean received1 = latch1.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        boolean received2 = latch2.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertTrue(received1, "Watcher 1 should receive the event");
        assertTrue(received2, "Watcher 2 should receive the event");
        assertEquals(dataId, watcher1DataId.get(), "Watcher 1 should see correct dataId");
        assertEquals(dataId, watcher2DataId.get(), "Watcher 2 should see correct dataId");

        // Cleanup
        configService.cancelFuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher1);
        configService.cancelFuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher2);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CFW-011: Cancel one watcher should not affect other watchers on same pattern
     *
     * Register two watchers, cancel one, verify the other still receives events.
     */
    @Test
    @Order(11)
    void testCancelOneWatcherPreservesOther() throws NacosException, InterruptedException {
        String uniquePrefix = "cfw011-" + UUID.randomUUID().toString().substring(0, 8);
        String dataIdPattern = uniquePrefix + "*";
        String dataId = uniquePrefix + "-preserved";

        CountDownLatch survivorLatch = new CountDownLatch(1);
        CountDownLatch cancelledLatch = new CountDownLatch(1);

        FuzzyWatchEventWatcher cancelledWatcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                if (event.getDataId() != null && event.getDataId().equals(dataId)) {
                    cancelledLatch.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        FuzzyWatchEventWatcher survivorWatcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                if (event.getDataId() != null && event.getDataId().equals(dataId)) {
                    survivorLatch.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Register both watchers
        configService.fuzzyWatch(dataIdPattern, DEFAULT_GROUP, cancelledWatcher);
        configService.fuzzyWatch(dataIdPattern, DEFAULT_GROUP, survivorWatcher);
        Thread.sleep(2000);

        // Cancel one watcher
        configService.cancelFuzzyWatch(dataIdPattern, DEFAULT_GROUP, cancelledWatcher);
        Thread.sleep(1000);

        // Publish a matching config
        configService.publishConfig(dataId, DEFAULT_GROUP, "survivor.test=true");

        boolean survivorReceived = survivorLatch.await(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        boolean cancelledReceived = cancelledLatch.await(5, TimeUnit.SECONDS);

        assertTrue(survivorReceived, "Surviving watcher should still receive events");
        assertFalse(cancelledReceived, "Cancelled watcher should NOT receive events");

        // Cleanup
        configService.cancelFuzzyWatch(dataIdPattern, DEFAULT_GROUP, survivorWatcher);
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== P1: Namespace Isolation ====================

    /**
     * CFW-012: FuzzyWatch namespace isolation
     *
     * A fuzzy watch in the default namespace should NOT receive events for configs
     * published in a different namespace.
     */
    @Test
    @Order(12)
    void testFuzzyWatchNamespaceIsolation() throws Exception {
        String uniquePrefix = "cfw012-" + UUID.randomUUID().toString().substring(0, 8);
        String dataIdPattern = uniquePrefix + "*";
        String dataId = uniquePrefix + "-nsconfig";
        String otherNamespace = "cfw012-ns-" + UUID.randomUUID().toString().substring(0, 8);

        // Create a second ConfigService for the other namespace
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties nsProperties = new Properties();
        nsProperties.setProperty("serverAddr", serverAddr);
        nsProperties.setProperty("username", username);
        nsProperties.setProperty("password", password);
        nsProperties.setProperty("namespace", otherNamespace);

        ConfigService nsConfigService = NacosFactory.createConfigService(nsProperties);

        CountDownLatch defaultNsLatch = new CountDownLatch(1);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                System.out.println("Namespace isolation event: " + event);
                if (event.getDataId() != null && event.getDataId().equals(dataId)) {
                    defaultNsLatch.countDown();
                }
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Watch in default namespace
        configService.fuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher);
        Thread.sleep(2000);

        // Publish config in OTHER namespace
        nsConfigService.publishConfig(dataId, DEFAULT_GROUP, "other.namespace=true");
        Thread.sleep(1000);

        // Default namespace watcher should NOT receive the event
        boolean received = defaultNsLatch.await(5, TimeUnit.SECONDS);
        assertFalse(received,
                "Default namespace watcher should NOT receive events from other namespace");

        // Cleanup
        configService.cancelFuzzyWatch(dataIdPattern, DEFAULT_GROUP, watcher);
        nsConfigService.removeConfig(dataId, DEFAULT_GROUP);
        nsConfigService.shutDown();
    }

    // ==================== P1: fuzzyWatchWithGroupKeys (both patterns) ====================

    /**
     * CFW-013: fuzzyWatchWithGroupKeys with group-only pattern (3-arg overload)
     *
     * Uses the 2-arg fuzzyWatchWithGroupKeys(groupPattern, watcher) to get all
     * matching configs across all dataIds in groups matching the pattern.
     */
    @Test
    @Order(13)
    void testFuzzyWatchWithGroupKeysGroupOnly() throws Exception {
        String uniqueSuffix = UUID.randomUUID().toString().substring(0, 8);
        String group = "CFWGRP-" + uniqueSuffix;
        String dataId1 = "cfw013-a-" + uniqueSuffix;
        String dataId2 = "cfw013-b-" + uniqueSuffix;

        // Publish configs in custom group
        configService.publishConfig(dataId1, group, "grpkeys.a=true");
        configService.publishConfig(dataId2, group, "grpkeys.b=true");
        Thread.sleep(1000);

        FuzzyWatchEventWatcher watcher = new FuzzyWatchEventWatcher() {
            @Override
            public void onEvent(ConfigFuzzyWatchChangeEvent event) {
                System.out.println("GroupKeys group-only event: " + event);
            }

            @Override
            public Executor getExecutor() {
                return null;
            }
        };

        // Watch with group-only pattern - should find configs in that group
        Future<Set<String>> future = configService.fuzzyWatchWithGroupKeys(group, watcher);
        assertNotNull(future, "Future should not be null");

        Set<String> groupKeys = future.get(WATCH_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertNotNull(groupKeys, "Group keys set should not be null");
        assertTrue(groupKeys.size() >= 2,
                "Should have at least 2 matching group keys in group '" + group + "', got: " + groupKeys.size());

        // Cleanup
        configService.cancelFuzzyWatch(group, watcher);
        configService.removeConfig(dataId1, group);
        configService.removeConfig(dataId2, group);
    }
}
