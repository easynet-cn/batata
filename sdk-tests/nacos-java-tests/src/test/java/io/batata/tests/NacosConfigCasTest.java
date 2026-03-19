package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Properties;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.*;
import static org.junit.jupiter.api.Assumptions.*;

/**
 * Nacos 3.x Config CAS (Compare-And-Swap) SDK Compatibility Tests
 *
 * Tests Batata's compatibility with Nacos publishConfigCas API, which allows
 * optimistic concurrency control by requiring the MD5 of the previous content.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigCasTest {

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
        assertNotNull(configService, "ConfigService should be created successfully");
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (configService != null) {
            configService.shutDown();
        }
    }

    /**
     * Compute MD5 hash of a string, matching Nacos server-side MD5 calculation.
     */
    private static String md5(String content) {
        try {
            MessageDigest md = MessageDigest.getInstance("MD5");
            byte[] digest = md.digest(content.getBytes(java.nio.charset.StandardCharsets.UTF_8));
            StringBuilder sb = new StringBuilder();
            for (byte b : digest) {
                sb.append(String.format("%02x", b));
            }
            return sb.toString();
        } catch (NoSuchAlgorithmException e) {
            throw new RuntimeException("MD5 not available", e);
        }
    }

    // ==================== P0: Critical Tests ====================

    /**
     * CAS-001: publishConfigCas with correct MD5 should succeed
     *
     * Publish initial content, compute its MD5, then use publishConfigCas with that MD5
     * to update. The update should succeed because the MD5 matches.
     */
    @Test
    @Order(1)
    @Disabled("publishConfigCas API not yet implemented in Batata")
    void testPublishConfigCasWithCorrectMd5() throws NacosException, InterruptedException {
        String dataId = "cas001-correct-md5-" + UUID.randomUUID();
        String initialContent = "cas.initial.value=100";
        String updatedContent = "cas.updated.value=200";

        // Publish initial config
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, initialContent);
        assertTrue(published, "Initial publish should succeed");
        Thread.sleep(500);

        // Verify initial content
        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(initialContent, retrieved, "Should retrieve initial content");

        // Compute MD5 of current content
        String currentMd5 = md5(initialContent);

        // CAS update with correct MD5
        boolean casResult = configService.publishConfigCas(dataId, DEFAULT_GROUP, updatedContent, currentMd5);
        assertTrue(casResult, "CAS publish with correct MD5 should succeed");

        // Verify the update took effect
        Thread.sleep(500);
        String afterCas = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(updatedContent, afterCas, "Content should be updated after successful CAS");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CAS-002: publishConfigCas with wrong MD5 should fail
     *
     * Publish content, then attempt CAS update with a deliberately wrong MD5.
     * The update should fail, and the original content should remain.
     */
    @Test
    @Order(2)
    @Disabled("publishConfigCas API not yet implemented in Batata")
    void testPublishConfigCasWithWrongMd5() throws NacosException, InterruptedException {
        String dataId = "cas002-wrong-md5-" + UUID.randomUUID();
        String initialContent = "cas.stable.value=keep_me";
        String attemptedContent = "cas.wrong.value=should_not_appear";

        // Publish initial config
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, initialContent);
        assertTrue(published, "Initial publish should succeed");
        Thread.sleep(500);

        // Attempt CAS update with a completely wrong MD5
        String wrongMd5 = "00000000000000000000000000000000";
        boolean casResult = configService.publishConfigCas(dataId, DEFAULT_GROUP, attemptedContent, wrongMd5);
        assertFalse(casResult, "CAS publish with wrong MD5 should fail");

        // Verify original content is unchanged
        String afterFailedCas = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(initialContent, afterFailedCas, "Content should remain unchanged after failed CAS");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CAS-003: publishConfigCas after another update should fail (stale MD5)
     *
     * Client A reads config and computes MD5. Before Client A does CAS, another update
     * changes the content. Client A's CAS should fail because the MD5 is stale.
     */
    @Test
    @Order(3)
    @Disabled("publishConfigCas API not yet implemented in Batata")
    void testPublishConfigCasWithStaleMd5() throws NacosException, InterruptedException {
        String dataId = "cas003-stale-md5-" + UUID.randomUUID();
        String originalContent = "cas.original=v1";
        String interveningContent = "cas.intervening=v2";
        String staleUpdateContent = "cas.stale.update=v3";

        // Step 1: Publish original config
        configService.publishConfig(dataId, DEFAULT_GROUP, originalContent);
        Thread.sleep(500);

        // Step 2: "Client A" reads and computes MD5
        String clientAContent = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(originalContent, clientAContent);
        String clientAMd5 = md5(clientAContent);

        // Step 3: Another update happens (simulating Client B)
        boolean interveningPublished = configService.publishConfig(dataId, DEFAULT_GROUP, interveningContent);
        assertTrue(interveningPublished, "Intervening publish should succeed");
        Thread.sleep(500);

        // Verify the intervening update took effect
        String currentContent = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(interveningContent, currentContent, "Intervening content should be active");

        // Step 4: "Client A" tries CAS with stale MD5 - should fail
        boolean staleCasResult = configService.publishConfigCas(dataId, DEFAULT_GROUP, staleUpdateContent, clientAMd5);
        assertFalse(staleCasResult, "CAS with stale MD5 should fail because content was modified");

        // Verify the intervening content is still there
        String afterStaleCas = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(interveningContent, afterStaleCas, "Intervening content should remain after stale CAS");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CAS-004: Concurrent CAS updates - only one should succeed
     *
     * Multiple threads read the same config, compute the same MD5, and try CAS simultaneously.
     * Exactly one should succeed; the rest should fail.
     */
    @Test
    @Order(4)
    @Disabled("publishConfigCas API not yet implemented in Batata")
    void testConcurrentCasUpdates() throws NacosException, InterruptedException {
        String dataId = "cas004-concurrent-" + UUID.randomUUID();
        String initialContent = "cas.concurrent.initial=start";
        int threadCount = 5;

        // Publish initial config
        configService.publishConfig(dataId, DEFAULT_GROUP, initialContent);
        Thread.sleep(500);

        // All threads read the same content and compute same MD5
        String currentContent = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(initialContent, currentContent);
        String sharedMd5 = md5(currentContent);

        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(threadCount);
        AtomicInteger successCount = new AtomicInteger(0);
        AtomicInteger failCount = new AtomicInteger(0);

        for (int i = 0; i < threadCount; i++) {
            final int index = i;
            new Thread(() -> {
                try {
                    startLatch.await();
                    String threadContent = "cas.concurrent.winner=thread-" + index;
                    boolean result = configService.publishConfigCas(dataId, DEFAULT_GROUP, threadContent, sharedMd5);
                    if (result) {
                        successCount.incrementAndGet();
                    } else {
                        failCount.incrementAndGet();
                    }
                } catch (Exception e) {
                    System.err.println("Thread " + index + " error: " + e.getMessage());
                    failCount.incrementAndGet();
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        // Release all threads simultaneously
        startLatch.countDown();

        boolean completed = doneLatch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All CAS threads should complete within timeout");

        // Exactly one should succeed
        assertEquals(1, successCount.get(),
                "Exactly one CAS should succeed, but got " + successCount.get() + " successes");
        assertEquals(threadCount - 1, failCount.get(),
                "The rest should fail, but got " + failCount.get() + " failures");

        // Verify the winner's content is stored
        String finalContent = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(finalContent, "Final content should not be null");
        assertTrue(finalContent.startsWith("cas.concurrent.winner=thread-"),
                "Final content should be from the winning thread");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CAS-005: publishConfigCas with type parameter
     *
     * Verify that publishConfigCas also works when specifying a config type (e.g., "json").
     */
    @Test
    @Order(5)
    @Disabled("publishConfigCas API not yet implemented in Batata")
    void testPublishConfigCasWithType() throws NacosException, InterruptedException {
        String dataId = "cas005-typed-" + UUID.randomUUID();
        String initialContent = "{\"key\": \"initial\"}";
        String updatedContent = "{\"key\": \"updated\"}";

        // Publish initial JSON config
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, initialContent, "json");
        assertTrue(published, "Initial typed publish should succeed");
        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(initialContent, retrieved);

        // CAS update with correct MD5 and type
        String md5 = md5(initialContent);
        boolean casResult = configService.publishConfigCas(dataId, DEFAULT_GROUP, updatedContent, md5, "json");
        assertTrue(casResult, "CAS publish with type should succeed");

        Thread.sleep(500);
        String afterCas = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(updatedContent, afterCas, "Typed content should be updated after CAS");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CAS-006: Sequential CAS updates should work when each uses the correct MD5
     *
     * Perform multiple successive CAS updates, each using the MD5 of the previous content.
     * All should succeed.
     */
    @Test
    @Order(6)
    @Disabled("publishConfigCas API not yet implemented in Batata")
    void testSequentialCasUpdates() throws NacosException, InterruptedException {
        String dataId = "cas006-sequential-" + UUID.randomUUID();
        String content = "cas.seq.v0=initial";

        // Publish initial
        configService.publishConfig(dataId, DEFAULT_GROUP, content);
        Thread.sleep(500);

        // Perform 5 sequential CAS updates
        for (int i = 1; i <= 5; i++) {
            String currentContent = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertNotNull(currentContent, "Content should exist at iteration " + i);

            String currentMd5 = md5(currentContent);
            String newContent = "cas.seq.v" + i + "=updated";

            boolean casResult = configService.publishConfigCas(dataId, DEFAULT_GROUP, newContent, currentMd5);
            assertTrue(casResult, "Sequential CAS update " + i + " should succeed");
            Thread.sleep(300);

            content = newContent;
        }

        // Verify final content
        String finalContent = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals("cas.seq.v5=updated", finalContent, "Final content should be from last CAS update");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }
}
