package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Batch Configuration Tests
 *
 * Tests for batch configuration operations:
 * - Batch publish configurations
 * - Batch get configurations
 * - Batch delete configurations
 * - Configuration groups
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosBatchConfigTest {

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
        System.out.println("Nacos Batch Config Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (configService != null) {
            configService.shutDown();
        }
    }

    // ==================== Batch Publish Tests ====================

    /**
     * NBC-001: Test batch publish multiple configs
     */
    @Test
    @Order(1)
    void testBatchPublishConfigs() throws NacosException, InterruptedException {
        String prefix = "batch-pub-" + UUID.randomUUID().toString().substring(0, 8);
        int configCount = 5;

        // Publish multiple configs
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String content = "value=" + i + "\nindex=" + i;
            boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content);
            assertTrue(success, "Should publish config " + dataId);
        }

        Thread.sleep(500);

        // Verify all configs exist
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String content = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertNotNull(content, "Config " + dataId + " should exist");
            assertTrue(content.contains("value=" + i));
        }

        System.out.println("Batch published " + configCount + " configs");

        // Cleanup
        for (int i = 0; i < configCount; i++) {
            configService.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
        }
    }

    /**
     * NBC-002: Test batch publish with different groups
     */
    @Test
    @Order(2)
    void testBatchPublishDifferentGroups() throws NacosException, InterruptedException {
        String dataId = "batch-group-" + UUID.randomUUID().toString().substring(0, 8);
        String[] groups = {"GROUP_A", "GROUP_B", "GROUP_C"};

        // Publish same dataId to different groups
        for (String group : groups) {
            String content = "group=" + group + "\nvalue=test";
            boolean success = configService.publishConfig(dataId, group, content);
            assertTrue(success, "Should publish to group " + group);
        }

        Thread.sleep(500);

        // Verify each group has its own config
        for (String group : groups) {
            String content = configService.getConfig(dataId, group, 5000);
            assertNotNull(content);
            assertTrue(content.contains("group=" + group));
        }

        System.out.println("Published to " + groups.length + " groups");

        // Cleanup
        for (String group : groups) {
            configService.removeConfig(dataId, group);
        }
    }

    /**
     * NBC-003: Test concurrent batch publish
     */
    @Test
    @Order(3)
    void testConcurrentBatchPublish() throws InterruptedException {
        String prefix = "batch-concurrent-" + UUID.randomUUID().toString().substring(0, 8);
        int threadCount = 10;
        CountDownLatch latch = new CountDownLatch(threadCount);
        List<Exception> errors = new CopyOnWriteArrayList<>();
        List<String> dataIds = new CopyOnWriteArrayList<>();

        for (int i = 0; i < threadCount; i++) {
            final int idx = i;
            new Thread(() -> {
                try {
                    String dataId = prefix + "-" + idx;
                    dataIds.add(dataId);
                    boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, "content=" + idx);
                    if (!success) {
                        errors.add(new RuntimeException("Failed to publish " + dataId));
                    }
                } catch (Exception e) {
                    errors.add(e);
                } finally {
                    latch.countDown();
                }
            }).start();
        }

        boolean completed = latch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete");

        System.out.println("Concurrent publish errors: " + errors.size());
        System.out.println("Configs published: " + dataIds.size());

        // Cleanup
        for (String dataId : dataIds) {
            try {
                configService.removeConfig(dataId, DEFAULT_GROUP);
            } catch (Exception e) {
                // Ignore cleanup errors
            }
        }
    }

    // ==================== Batch Get Tests ====================

    /**
     * NBC-004: Test batch get multiple configs
     */
    @Test
    @Order(4)
    void testBatchGetConfigs() throws NacosException, InterruptedException {
        String prefix = "batch-get-" + UUID.randomUUID().toString().substring(0, 8);
        int configCount = 5;

        // Setup
        for (int i = 0; i < configCount; i++) {
            configService.publishConfig(prefix + "-" + i, DEFAULT_GROUP, "data=" + i);
        }
        Thread.sleep(500);

        // Batch get
        long startTime = System.currentTimeMillis();
        Map<String, String> configs = new HashMap<>();
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String content = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
            if (content != null) {
                configs.put(dataId, content);
            }
        }
        long duration = System.currentTimeMillis() - startTime;

        assertEquals(configCount, configs.size(), "Should get all configs");
        System.out.println("Batch get " + configs.size() + " configs in " + duration + "ms");

        // Cleanup
        for (int i = 0; i < configCount; i++) {
            configService.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
        }
    }

    /**
     * NBC-005: Test concurrent batch get
     */
    @Test
    @Order(5)
    void testConcurrentBatchGet() throws NacosException, InterruptedException {
        String prefix = "batch-cget-" + UUID.randomUUID().toString().substring(0, 8);
        int configCount = 5;

        // Setup configs
        for (int i = 0; i < configCount; i++) {
            configService.publishConfig(prefix + "-" + i, DEFAULT_GROUP, "value=" + i);
        }
        Thread.sleep(500);

        // Concurrent get
        int threadCount = 10;
        CountDownLatch latch = new CountDownLatch(threadCount);
        List<String> results = new CopyOnWriteArrayList<>();

        for (int i = 0; i < threadCount; i++) {
            final int idx = i % configCount;
            new Thread(() -> {
                try {
                    String content = configService.getConfig(prefix + "-" + idx, DEFAULT_GROUP, 5000);
                    if (content != null) {
                        results.add(content);
                    }
                } catch (Exception e) {
                    System.out.println("Concurrent get error: " + e.getMessage());
                } finally {
                    latch.countDown();
                }
            }).start();
        }

        boolean completed = latch.await(30, TimeUnit.SECONDS);
        assertTrue(completed);
        System.out.println("Concurrent get results: " + results.size());

        // Cleanup
        for (int i = 0; i < configCount; i++) {
            configService.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
        }
    }

    // ==================== Batch Delete Tests ====================

    /**
     * NBC-006: Test batch delete configs
     */
    @Test
    @Order(6)
    void testBatchDeleteConfigs() throws NacosException, InterruptedException {
        String prefix = "batch-del-" + UUID.randomUUID().toString().substring(0, 8);
        int configCount = 5;

        // Setup
        for (int i = 0; i < configCount; i++) {
            configService.publishConfig(prefix + "-" + i, DEFAULT_GROUP, "temp=" + i);
        }
        Thread.sleep(500);

        // Batch delete
        int deleted = 0;
        for (int i = 0; i < configCount; i++) {
            boolean success = configService.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
            if (success) deleted++;
        }

        assertEquals(configCount, deleted, "Should delete all configs");
        System.out.println("Batch deleted " + deleted + " configs");

        // Verify deletion
        Thread.sleep(500);
        for (int i = 0; i < configCount; i++) {
            String content = configService.getConfig(prefix + "-" + i, DEFAULT_GROUP, 3000);
            assertNull(content, "Config should be deleted");
        }
    }

    /**
     * NBC-007: Test batch delete with mixed existence
     */
    @Test
    @Order(7)
    void testBatchDeleteMixedExistence() throws NacosException, InterruptedException {
        String prefix = "batch-mixed-" + UUID.randomUUID().toString().substring(0, 8);

        // Only create some configs
        configService.publishConfig(prefix + "-0", DEFAULT_GROUP, "exists");
        configService.publishConfig(prefix + "-2", DEFAULT_GROUP, "exists");
        Thread.sleep(500);

        // Try to delete all (some don't exist)
        int successCount = 0;
        for (int i = 0; i < 5; i++) {
            boolean success = configService.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
            if (success) successCount++;
        }

        System.out.println("Delete success count: " + successCount);
    }

    // ==================== Group Operations Tests ====================

    /**
     * NBC-008: Test configs in custom group
     */
    @Test
    @Order(8)
    void testCustomGroupConfigs() throws NacosException, InterruptedException {
        String customGroup = "CUSTOM_GROUP_" + UUID.randomUUID().toString().substring(0, 4);
        String prefix = "custom-grp-" + UUID.randomUUID().toString().substring(0, 8);

        // Publish to custom group
        for (int i = 0; i < 3; i++) {
            String dataId = prefix + "-" + i;
            configService.publishConfig(dataId, customGroup, "custom=" + i);
        }
        Thread.sleep(500);

        // Verify in custom group
        for (int i = 0; i < 3; i++) {
            String content = configService.getConfig(prefix + "-" + i, customGroup, 5000);
            assertNotNull(content);
            assertTrue(content.contains("custom=" + i));
        }

        // Verify NOT in default group
        String defaultContent = configService.getConfig(prefix + "-0", DEFAULT_GROUP, 3000);
        assertNull(defaultContent, "Should not exist in default group");

        System.out.println("Custom group configs verified");

        // Cleanup
        for (int i = 0; i < 3; i++) {
            configService.removeConfig(prefix + "-" + i, customGroup);
        }
    }

    /**
     * NBC-009: Test same dataId in multiple groups
     */
    @Test
    @Order(9)
    void testSameDataIdMultipleGroups() throws NacosException, InterruptedException {
        String dataId = "same-dataid-" + UUID.randomUUID().toString().substring(0, 8);
        String[] groups = {"GROUP_1", "GROUP_2", "GROUP_3"};
        Map<String, String> expected = new HashMap<>();

        // Publish different content to each group
        for (int i = 0; i < groups.length; i++) {
            String content = "group_index=" + i + "\ngroup_name=" + groups[i];
            expected.put(groups[i], content);
            configService.publishConfig(dataId, groups[i], content);
        }
        Thread.sleep(500);

        // Verify each group has its own content
        for (String group : groups) {
            String content = configService.getConfig(dataId, group, 5000);
            assertEquals(expected.get(group), content);
        }

        System.out.println("Same dataId in " + groups.length + " groups verified");

        // Cleanup
        for (String group : groups) {
            configService.removeConfig(dataId, group);
        }
    }

    // ==================== Large Batch Tests ====================

    /**
     * NBC-010: Test large batch publish
     */
    @Test
    @Order(10)
    void testLargeBatchPublish() throws NacosException, InterruptedException {
        String prefix = "large-batch-" + UUID.randomUUID().toString().substring(0, 8);
        int configCount = 20;

        long startTime = System.currentTimeMillis();
        int successCount = 0;

        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String content = generateLargeContent(100); // 100 properties
            boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content);
            if (success) successCount++;
        }

        long duration = System.currentTimeMillis() - startTime;
        System.out.println("Large batch: " + successCount + " configs in " + duration + "ms");

        // Cleanup
        for (int i = 0; i < configCount; i++) {
            configService.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
        }
    }

    /**
     * NBC-011: Test batch with large content
     */
    @Test
    @Order(11)
    void testBatchLargeContent() throws NacosException, InterruptedException {
        String prefix = "large-content-" + UUID.randomUUID().toString().substring(0, 8);
        int configCount = 5;

        // Publish configs with large content
        for (int i = 0; i < configCount; i++) {
            String dataId = prefix + "-" + i;
            String content = generateLargeContent(500); // 500 properties
            boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content);
            assertTrue(success, "Should publish large config");
        }
        Thread.sleep(1000);

        // Verify
        for (int i = 0; i < configCount; i++) {
            String content = configService.getConfig(prefix + "-" + i, DEFAULT_GROUP, 10000);
            assertNotNull(content);
            assertTrue(content.length() > 1000);
        }

        System.out.println("Large content batch verified");

        // Cleanup
        for (int i = 0; i < configCount; i++) {
            configService.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
        }
    }

    // ==================== Update Tests ====================

    /**
     * NBC-012: Test batch update configs
     */
    @Test
    @Order(12)
    void testBatchUpdateConfigs() throws NacosException, InterruptedException {
        String prefix = "batch-update-" + UUID.randomUUID().toString().substring(0, 8);
        int configCount = 5;

        // Initial publish
        for (int i = 0; i < configCount; i++) {
            configService.publishConfig(prefix + "-" + i, DEFAULT_GROUP, "version=1");
        }
        Thread.sleep(500);

        // Update all
        for (int i = 0; i < configCount; i++) {
            boolean success = configService.publishConfig(prefix + "-" + i, DEFAULT_GROUP, "version=2");
            assertTrue(success);
        }
        Thread.sleep(500);

        // Verify updates
        for (int i = 0; i < configCount; i++) {
            String content = configService.getConfig(prefix + "-" + i, DEFAULT_GROUP, 5000);
            assertEquals("version=2", content);
        }

        System.out.println("Batch update verified");

        // Cleanup
        for (int i = 0; i < configCount; i++) {
            configService.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
        }
    }

    /**
     * NBC-013: Test rapid batch updates
     */
    @Test
    @Order(13)
    void testRapidBatchUpdates() throws NacosException, InterruptedException {
        String dataId = "rapid-update-" + UUID.randomUUID().toString().substring(0, 8);
        int updateCount = 10;

        // Rapid updates
        for (int i = 0; i < updateCount; i++) {
            configService.publishConfig(dataId, DEFAULT_GROUP, "iteration=" + i);
        }

        Thread.sleep(1000);

        // Get final value
        String content = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(content);
        System.out.println("Final content after rapid updates: " + content);

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== Error Handling Tests ====================

    /**
     * NBC-014: Test batch get with timeout
     */
    @Test
    @Order(14)
    void testBatchGetWithTimeout() throws NacosException {
        String prefix = "batch-timeout-" + UUID.randomUUID().toString().substring(0, 8);

        // Try to get non-existent configs with short timeout
        long startTime = System.currentTimeMillis();
        int nullCount = 0;

        for (int i = 0; i < 5; i++) {
            String content = configService.getConfig(prefix + "-" + i, DEFAULT_GROUP, 1000);
            if (content == null) nullCount++;
        }

        long duration = System.currentTimeMillis() - startTime;
        System.out.println("Batch timeout test: " + nullCount + " nulls in " + duration + "ms");
    }

    /**
     * NBC-015: Test batch operations resilience
     */
    @Test
    @Order(15)
    void testBatchOperationsResilience() throws InterruptedException {
        String prefix = "batch-resilience-" + UUID.randomUUID().toString().substring(0, 8);
        int operationCount = 20;
        List<String> createdConfigs = new CopyOnWriteArrayList<>();

        // Mix of create, update, get, delete operations
        CountDownLatch latch = new CountDownLatch(operationCount);
        List<Exception> errors = new CopyOnWriteArrayList<>();

        for (int i = 0; i < operationCount; i++) {
            final int idx = i;
            new Thread(() -> {
                try {
                    String dataId = prefix + "-" + (idx % 5);
                    int op = idx % 4;

                    switch (op) {
                        case 0: // Create
                            if (configService.publishConfig(dataId, DEFAULT_GROUP, "create=" + idx)) {
                                createdConfigs.add(dataId);
                            }
                            break;
                        case 1: // Update
                            configService.publishConfig(dataId, DEFAULT_GROUP, "update=" + idx);
                            break;
                        case 2: // Get
                            configService.getConfig(dataId, DEFAULT_GROUP, 3000);
                            break;
                        case 3: // Delete
                            configService.removeConfig(dataId, DEFAULT_GROUP);
                            break;
                    }
                } catch (Exception e) {
                    errors.add(e);
                } finally {
                    latch.countDown();
                }
            }).start();
        }

        boolean completed = latch.await(60, TimeUnit.SECONDS);
        assertTrue(completed, "All operations should complete");

        System.out.println("Resilience test errors: " + errors.size());
        System.out.println("Created configs: " + createdConfigs.size());

        // Cleanup
        for (int i = 0; i < 5; i++) {
            try {
                configService.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
            } catch (Exception e) {
                // Ignore
            }
        }
    }

    // ==================== Helper Methods ====================

    private String generateLargeContent(int propertyCount) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < propertyCount; i++) {
            sb.append("property").append(i).append("=value").append(i).append("\n");
        }
        return sb.toString();
    }
}
