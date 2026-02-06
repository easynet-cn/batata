package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigChangeListener;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.model.ConfigChangeEvent;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Async Config Tests
 *
 * Tests for asynchronous configuration access patterns:
 * - Async config loading
 * - Parallel namespace access
 * - Async listeners
 * - Thread safety
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloAsyncConfigTest {

    private static ExecutorService executorService;

    @BeforeAll
    static void setupClass() {
        System.setProperty("app.id", System.getProperty("apollo.app.id", "test-app"));
        System.setProperty("apollo.meta", System.getProperty("apollo.meta", "http://127.0.0.1:8848"));
        System.setProperty("apollo.configService", System.getProperty("apollo.configService", "http://127.0.0.1:8848"));
        System.setProperty("env", System.getProperty("apollo.env", "DEV"));
        System.setProperty("apollo.cluster", System.getProperty("apollo.cluster", "default"));

        executorService = Executors.newFixedThreadPool(10);
        System.out.println("Apollo Async Config Test Setup");
    }

    @AfterAll
    static void teardownClass() {
        if (executorService != null) {
            executorService.shutdown();
        }
    }

    // ==================== Async Loading Tests ====================

    /**
     * AAC-001: Test async config loading
     */
    @Test
    @Order(1)
    void testAsyncConfigLoading() throws InterruptedException, ExecutionException {
        CompletableFuture<Config> future = CompletableFuture.supplyAsync(() ->
            ConfigService.getAppConfig(), executorService);

        Config config = future.get(30, TimeUnit.SECONDS);
        assertNotNull(config, "Config should be loaded asynchronously");

        System.out.println("Async config loaded successfully");
    }

    /**
     * AAC-002: Test parallel namespace loading
     */
    @Test
    @Order(2)
    void testParallelNamespaceLoading() throws InterruptedException {
        String[] namespaces = {"application", "common", "database", "redis", "mq"};
        List<CompletableFuture<Config>> futures = new ArrayList<>();

        for (String ns : namespaces) {
            CompletableFuture<Config> future = CompletableFuture.supplyAsync(() -> {
                try {
                    return ConfigService.getConfig(ns);
                } catch (Exception e) {
                    return null;
                }
            }, executorService);
            futures.add(future);
        }

        // Wait for all
        CompletableFuture.allOf(futures.toArray(new CompletableFuture[0])).join();

        int loaded = 0;
        for (CompletableFuture<Config> future : futures) {
            if (future.getNow(null) != null) {
                loaded++;
            }
        }

        System.out.println("Parallel loaded " + loaded + " namespaces");
    }

    /**
     * AAC-003: Test async property access
     */
    @Test
    @Order(3)
    void testAsyncPropertyAccess() throws InterruptedException, ExecutionException {
        Config config = ConfigService.getAppConfig();

        CompletableFuture<String> future = CompletableFuture.supplyAsync(() ->
            config.getProperty("async.test.key", "default"), executorService);

        String value = future.get(10, TimeUnit.SECONDS);
        assertNotNull(value);

        System.out.println("Async property access: " + value);
    }

    /**
     * AAC-004: Test multiple async property access
     */
    @Test
    @Order(4)
    void testMultipleAsyncPropertyAccess() throws InterruptedException {
        Config config = ConfigService.getAppConfig();
        int propertyCount = 20;
        List<CompletableFuture<String>> futures = new ArrayList<>();

        for (int i = 0; i < propertyCount; i++) {
            final int idx = i;
            CompletableFuture<String> future = CompletableFuture.supplyAsync(() ->
                config.getProperty("async.key." + idx, "default-" + idx), executorService);
            futures.add(future);
        }

        CompletableFuture.allOf(futures.toArray(new CompletableFuture[0])).join();

        List<String> values = new ArrayList<>();
        for (CompletableFuture<String> future : futures) {
            values.add(future.getNow("failed"));
        }

        assertEquals(propertyCount, values.size());
        System.out.println("Multiple async property access: " + values.size() + " values");
    }

    // ==================== Thread Safety Tests ====================

    /**
     * AAC-005: Test thread-safe config access
     */
    @Test
    @Order(5)
    void testThreadSafeConfigAccess() throws InterruptedException {
        int threadCount = 50;
        CountDownLatch latch = new CountDownLatch(threadCount);
        AtomicInteger successCount = new AtomicInteger(0);
        AtomicInteger errorCount = new AtomicInteger(0);

        for (int i = 0; i < threadCount; i++) {
            executorService.submit(() -> {
                try {
                    Config config = ConfigService.getAppConfig();
                    config.getProperty("thread.safe.key", "default");
                    successCount.incrementAndGet();
                } catch (Exception e) {
                    errorCount.incrementAndGet();
                } finally {
                    latch.countDown();
                }
            });
        }

        boolean completed = latch.await(60, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete");

        System.out.println("Thread-safe access - Success: " + successCount.get() +
                          ", Errors: " + errorCount.get());
    }

    /**
     * AAC-006: Test concurrent config instance access
     */
    @Test
    @Order(6)
    void testConcurrentConfigInstanceAccess() throws InterruptedException {
        int threadCount = 30;
        CountDownLatch latch = new CountDownLatch(threadCount);
        Set<Integer> configHashes = ConcurrentHashMap.newKeySet();

        for (int i = 0; i < threadCount; i++) {
            executorService.submit(() -> {
                try {
                    Config config = ConfigService.getAppConfig();
                    configHashes.add(System.identityHashCode(config));
                } finally {
                    latch.countDown();
                }
            });
        }

        latch.await(30, TimeUnit.SECONDS);

        // Should be same instance (cached)
        assertEquals(1, configHashes.size(), "Should return same cached instance");
        System.out.println("Concurrent access returned " + configHashes.size() + " unique instance(s)");
    }

    /**
     * AAC-007: Test concurrent property names access
     */
    @Test
    @Order(7)
    void testConcurrentPropertyNamesAccess() throws InterruptedException {
        Config config = ConfigService.getAppConfig();
        int threadCount = 20;
        CountDownLatch latch = new CountDownLatch(threadCount);
        List<Set<String>> results = new CopyOnWriteArrayList<>();

        for (int i = 0; i < threadCount; i++) {
            executorService.submit(() -> {
                try {
                    Set<String> names = config.getPropertyNames();
                    results.add(names);
                } finally {
                    latch.countDown();
                }
            });
        }

        latch.await(30, TimeUnit.SECONDS);

        assertEquals(threadCount, results.size());
        System.out.println("Concurrent property names access completed");
    }

    // ==================== Async Listener Tests ====================

    /**
     * AAC-008: Test async listener registration
     */
    @Test
    @Order(8)
    void testAsyncListenerRegistration() throws InterruptedException {
        Config config = ConfigService.getAppConfig();
        int listenerCount = 10;
        CountDownLatch latch = new CountDownLatch(listenerCount);
        List<ConfigChangeListener> listeners = new CopyOnWriteArrayList<>();

        for (int i = 0; i < listenerCount; i++) {
            executorService.submit(() -> {
                try {
                    ConfigChangeListener listener = changeEvent -> {};
                    config.addChangeListener(listener);
                    listeners.add(listener);
                } finally {
                    latch.countDown();
                }
            });
        }

        latch.await(30, TimeUnit.SECONDS);

        assertEquals(listenerCount, listeners.size());
        System.out.println("Async registered " + listeners.size() + " listeners");

        // Cleanup
        for (ConfigChangeListener listener : listeners) {
            config.removeChangeListener(listener);
        }
    }

    /**
     * AAC-009: Test async listener callback
     */
    @Test
    @Order(9)
    void testAsyncListenerCallback() throws InterruptedException {
        Config config = ConfigService.getAppConfig();
        AtomicInteger callbackCount = new AtomicInteger(0);
        CountDownLatch latch = new CountDownLatch(1);

        ConfigChangeListener listener = changeEvent -> {
            // Process callback asynchronously
            CompletableFuture.runAsync(() -> {
                callbackCount.incrementAndGet();
                System.out.println("Async callback processed");
            }, executorService);
        };

        config.addChangeListener(listener);

        // Wait a bit for any potential callbacks
        Thread.sleep(2000);

        config.removeChangeListener(listener);
        System.out.println("Async listener callback test completed");
    }

    /**
     * AAC-010: Test listener with async interested keys
     */
    @Test
    @Order(10)
    void testListenerAsyncInterestedKeys() throws InterruptedException {
        Config config = ConfigService.getAppConfig();

        Set<String> interestedKeys = ConcurrentHashMap.newKeySet();
        interestedKeys.addAll(Arrays.asList("async.key1", "async.key2", "async.key3"));

        AtomicInteger matchCount = new AtomicInteger(0);

        ConfigChangeListener listener = changeEvent -> {
            for (String key : changeEvent.changedKeys()) {
                if (interestedKeys.contains(key)) {
                    matchCount.incrementAndGet();
                }
            }
        };

        config.addChangeListener(listener, interestedKeys);

        // Access keys
        for (String key : interestedKeys) {
            config.getProperty(key, "default");
        }

        Thread.sleep(1000);

        config.removeChangeListener(listener);
        System.out.println("Async interested keys test completed");
    }

    // ==================== CompletableFuture Tests ====================

    /**
     * AAC-011: Test config with CompletableFuture chain
     */
    @Test
    @Order(11)
    void testConfigCompletableFutureChain() throws InterruptedException, ExecutionException {
        CompletableFuture<String> result = CompletableFuture.supplyAsync(() ->
            ConfigService.getAppConfig(), executorService)
            .thenApply(config -> config.getProperty("chain.key", "default"))
            .thenApply(value -> value.toUpperCase())
            .thenApply(value -> "Result: " + value);

        String finalResult = result.get(30, TimeUnit.SECONDS);
        assertTrue(finalResult.startsWith("Result:"));

        System.out.println("CompletableFuture chain result: " + finalResult);
    }

    /**
     * AAC-012: Test config with CompletableFuture combine
     */
    @Test
    @Order(12)
    void testConfigCompletableFutureCombine() throws InterruptedException, ExecutionException {
        CompletableFuture<String> future1 = CompletableFuture.supplyAsync(() -> {
            Config config = ConfigService.getConfig("application");
            return config.getProperty("combine.key1", "value1");
        }, executorService);

        CompletableFuture<String> future2 = CompletableFuture.supplyAsync(() -> {
            Config config = ConfigService.getConfig("application");
            return config.getProperty("combine.key2", "value2");
        }, executorService);

        CompletableFuture<String> combined = future1.thenCombine(future2,
            (v1, v2) -> v1 + ":" + v2);

        String result = combined.get(30, TimeUnit.SECONDS);
        assertTrue(result.contains(":"));

        System.out.println("Combined result: " + result);
    }

    /**
     * AAC-013: Test config with CompletableFuture exceptionally
     */
    @Test
    @Order(13)
    void testConfigCompletableFutureExceptionally() throws InterruptedException, ExecutionException {
        CompletableFuture<String> result = CompletableFuture.supplyAsync(() -> {
            Config config = ConfigService.getConfig("non-existent-namespace-" + UUID.randomUUID());
            return config.getProperty("key", "default");
        }, executorService)
        .exceptionally(ex -> {
            System.out.println("Exception handled: " + ex.getMessage());
            return "fallback-value";
        });

        String value = result.get(30, TimeUnit.SECONDS);
        assertNotNull(value);

        System.out.println("Exceptionally result: " + value);
    }

    // ==================== Executor Tests ====================

    /**
     * AAC-014: Test config with custom executor
     */
    @Test
    @Order(14)
    void testConfigWithCustomExecutor() throws InterruptedException {
        ExecutorService customExecutor = Executors.newSingleThreadExecutor();
        AtomicReference<String> threadName = new AtomicReference<>();

        try {
            CompletableFuture.runAsync(() -> {
                threadName.set(Thread.currentThread().getName());
                ConfigService.getAppConfig().getProperty("executor.key", "default");
            }, customExecutor).get(10, TimeUnit.SECONDS);

            assertNotNull(threadName.get());
            System.out.println("Custom executor thread: " + threadName.get());
        } catch (Exception e) {
            System.out.println("Custom executor error: " + e.getMessage());
        } finally {
            customExecutor.shutdown();
        }
    }

    /**
     * AAC-015: Test config with scheduled executor
     */
    @Test
    @Order(15)
    void testConfigWithScheduledExecutor() throws InterruptedException {
        ScheduledExecutorService scheduler = Executors.newScheduledThreadPool(2);
        AtomicInteger accessCount = new AtomicInteger(0);

        try {
            ScheduledFuture<?> future = scheduler.scheduleAtFixedRate(() -> {
                Config config = ConfigService.getAppConfig();
                config.getProperty("scheduled.key", "default");
                accessCount.incrementAndGet();
            }, 0, 200, TimeUnit.MILLISECONDS);

            Thread.sleep(1000);
            future.cancel(false);

            assertTrue(accessCount.get() >= 4, "Should have multiple accesses");
            System.out.println("Scheduled access count: " + accessCount.get());
        } finally {
            scheduler.shutdown();
        }
    }

    // ==================== Performance Tests ====================

    /**
     * AAC-016: Test async access performance
     */
    @Test
    @Order(16)
    void testAsyncAccessPerformance() throws InterruptedException {
        Config config = ConfigService.getAppConfig();
        int iterations = 100;
        CountDownLatch latch = new CountDownLatch(iterations);

        long startTime = System.currentTimeMillis();

        for (int i = 0; i < iterations; i++) {
            final int idx = i;
            executorService.submit(() -> {
                try {
                    config.getProperty("perf.key." + (idx % 10), "default");
                } finally {
                    latch.countDown();
                }
            });
        }

        latch.await(60, TimeUnit.SECONDS);

        long duration = System.currentTimeMillis() - startTime;
        double avgMs = (double) duration / iterations;

        System.out.println(iterations + " async accesses in " + duration + "ms");
        System.out.println("Average: " + String.format("%.2f", avgMs) + "ms per access");
    }

    /**
     * AAC-017: Test parallel namespace performance
     */
    @Test
    @Order(17)
    void testParallelNamespacePerformance() throws InterruptedException {
        String[] namespaces = {"ns1", "ns2", "ns3", "ns4", "ns5"};
        int iterations = 50;
        CountDownLatch latch = new CountDownLatch(iterations);

        long startTime = System.currentTimeMillis();

        for (int i = 0; i < iterations; i++) {
            final String ns = namespaces[i % namespaces.length];
            executorService.submit(() -> {
                try {
                    Config config = ConfigService.getConfig(ns);
                    config.getProperty("perf.key", "default");
                } catch (Exception e) {
                    // Ignore
                } finally {
                    latch.countDown();
                }
            });
        }

        latch.await(60, TimeUnit.SECONDS);

        long duration = System.currentTimeMillis() - startTime;
        System.out.println(iterations + " parallel namespace accesses in " + duration + "ms");
    }

    /**
     * AAC-018: Test high concurrency access
     */
    @Test
    @Order(18)
    void testHighConcurrencyAccess() throws InterruptedException {
        Config config = ConfigService.getAppConfig();
        int threadCount = 100;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch endLatch = new CountDownLatch(threadCount);
        AtomicInteger successCount = new AtomicInteger(0);

        for (int i = 0; i < threadCount; i++) {
            final int idx = i;
            new Thread(() -> {
                try {
                    startLatch.await(); // Wait for all threads to be ready
                    config.getProperty("concurrent.key." + (idx % 5), "default");
                    successCount.incrementAndGet();
                } catch (Exception e) {
                    // Ignore
                } finally {
                    endLatch.countDown();
                }
            }).start();
        }

        // Start all threads simultaneously
        startLatch.countDown();

        boolean completed = endLatch.await(60, TimeUnit.SECONDS);
        assertTrue(completed);

        System.out.println("High concurrency: " + successCount.get() + "/" + threadCount + " successful");
    }
}
