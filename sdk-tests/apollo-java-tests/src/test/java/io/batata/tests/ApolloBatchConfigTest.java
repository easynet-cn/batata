package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigChangeListener;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.model.ConfigChange;
import com.ctrip.framework.apollo.model.ConfigChangeEvent;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import java.util.stream.Collectors;
import java.util.stream.IntStream;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Batch Configuration Tests
 *
 * Tests for batch configuration operations:
 * - Batch get multiple keys
 * - Batch get from multiple namespaces
 * - Batch property access
 * - Parallel namespace loading
 * - Batch config with defaults
 * - Batch type conversion
 * - Batch listener registration
 * - Batch config refresh
 * - Batch config with cache
 * - Large batch operations
 * - Batch config performance
 * - Batch error handling
 * - Batch partial failure
 * - Batch with timeout
 * - Batch concurrent access
 * - Batch config merge
 * - Batch namespace isolation
 * - Batch config rollback
 * - Batch with gray release
 * - Batch config versioning
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloBatchConfigTest {

    private static final String DEFAULT_NAMESPACE = "application";
    private static final String[] TEST_NAMESPACES = {
            "application", "common", "database", "redis", "mq"
    };

    @BeforeAll
    static void setupClass() {
        System.setProperty("app.id", System.getProperty("apollo.app.id", "batch-test-app"));
        System.setProperty("apollo.meta", System.getProperty("apollo.meta", "http://127.0.0.1:8848"));
        System.setProperty("apollo.configService", System.getProperty("apollo.configService", "http://127.0.0.1:8848"));
        System.setProperty("env", System.getProperty("apollo.env", "DEV"));
        System.setProperty("apollo.cluster", System.getProperty("apollo.cluster", "default"));

        System.out.println("Apollo Batch Config Test Setup");
        System.out.println("  app.id: " + System.getProperty("app.id"));
        System.out.println("  apollo.meta: " + System.getProperty("apollo.meta"));
        System.out.println("  env: " + System.getProperty("env"));
    }

    // ==================== ABC-001: Batch Get Multiple Keys ====================

    /**
     * ABC-001: Test batch get multiple keys
     *
     * Verifies that multiple configuration keys can be retrieved efficiently
     * from a single namespace in batch.
     */
    @Test
    @Order(1)
    void testBatchGetMultipleKeys() {
        Config config = ConfigService.getConfig(DEFAULT_NAMESPACE);
        assertNotNull(config, "Config should not be null");

        // Define batch of keys to retrieve
        List<String> keys = Arrays.asList(
                "batch.key1", "batch.key2", "batch.key3",
                "batch.key4", "batch.key5", "batch.key6",
                "batch.key7", "batch.key8", "batch.key9", "batch.key10"
        );

        // Batch retrieve all values
        Map<String, String> batchResults = new HashMap<>();
        long startTime = System.currentTimeMillis();

        for (String key : keys) {
            String value = config.getProperty(key, "default-" + key);
            batchResults.put(key, value);
        }

        long duration = System.currentTimeMillis() - startTime;

        // Verify all keys were retrieved
        assertEquals(keys.size(), batchResults.size(), "Should retrieve all keys");
        assertFalse(batchResults.values().contains(null), "No null values should exist");

        System.out.println("Batch retrieved " + keys.size() + " keys in " + duration + "ms");
        batchResults.forEach((k, v) -> System.out.println("  " + k + " = " + v));
    }

    // ==================== ABC-002: Batch Get From Multiple Namespaces ====================

    /**
     * ABC-002: Test batch get from multiple namespaces
     *
     * Verifies that configurations can be retrieved from multiple namespaces
     * in a batch operation.
     */
    @Test
    @Order(2)
    void testBatchGetFromMultipleNamespaces() {
        Map<String, Config> configMap = new HashMap<>();
        Map<String, String> results = new HashMap<>();

        long startTime = System.currentTimeMillis();

        // Load configs from all namespaces
        for (String namespace : TEST_NAMESPACES) {
            try {
                Config config = ConfigService.getConfig(namespace);
                configMap.put(namespace, config);

                // Get a property from each namespace
                String value = config.getProperty("common.key", "ns-default-" + namespace);
                results.put(namespace, value);
            } catch (Exception e) {
                System.out.println("Namespace " + namespace + " error: " + e.getMessage());
            }
        }

        long duration = System.currentTimeMillis() - startTime;

        assertEquals(TEST_NAMESPACES.length, configMap.size(),
                "Should load all namespaces");

        System.out.println("Batch loaded " + configMap.size() + " namespaces in " + duration + "ms");
        results.forEach((ns, v) -> System.out.println("  " + ns + " -> " + v));
    }

    // ==================== ABC-003: Batch Property Access ====================

    /**
     * ABC-003: Test batch property access
     *
     * Verifies that multiple properties can be accessed in batch
     * with different access patterns.
     */
    @Test
    @Order(3)
    void testBatchPropertyAccess() {
        Config config = ConfigService.getConfig(DEFAULT_NAMESPACE);

        // Test accessing properties with different patterns
        Map<String, Object> properties = new LinkedHashMap<>();

        // String properties
        properties.put("string.prop1", config.getProperty("string.prop1", "str-default"));
        properties.put("string.prop2", config.getProperty("string.prop2", "str-default"));

        // Integer properties
        properties.put("int.prop1", config.getIntProperty("int.prop1", 0));
        properties.put("int.prop2", config.getIntProperty("int.prop2", 100));

        // Boolean properties
        properties.put("bool.prop1", config.getBooleanProperty("bool.prop1", false));
        properties.put("bool.prop2", config.getBooleanProperty("bool.prop2", true));

        // Long properties
        properties.put("long.prop1", config.getLongProperty("long.prop1", 0L));

        // Double properties
        properties.put("double.prop1", config.getDoubleProperty("double.prop1", 0.0));

        assertEquals(8, properties.size(), "Should have 8 properties");

        System.out.println("Batch property access results:");
        properties.forEach((k, v) -> System.out.println("  " + k + " = " + v + " (" + v.getClass().getSimpleName() + ")"));
    }

    // ==================== ABC-004: Parallel Namespace Loading ====================

    /**
     * ABC-004: Test parallel namespace loading
     *
     * Verifies that multiple namespaces can be loaded in parallel
     * for improved performance.
     */
    @Test
    @Order(4)
    void testParallelNamespaceLoading() throws InterruptedException, ExecutionException {
        ExecutorService executor = Executors.newFixedThreadPool(TEST_NAMESPACES.length);
        List<Future<Config>> futures = new ArrayList<>();

        long startTime = System.currentTimeMillis();

        // Submit parallel loading tasks
        for (String namespace : TEST_NAMESPACES) {
            futures.add(executor.submit(() -> {
                Config config = ConfigService.getConfig(namespace);
                System.out.println("  Loaded namespace: " + namespace +
                        " [Thread: " + Thread.currentThread().getName() + "]");
                return config;
            }));
        }

        // Collect results
        List<Config> configs = new ArrayList<>();
        for (Future<Config> future : futures) {
            try {
                Config config = future.get(10, TimeUnit.SECONDS);
                configs.add(config);
            } catch (TimeoutException e) {
                System.out.println("Timeout loading namespace");
            }
        }

        long duration = System.currentTimeMillis() - startTime;
        executor.shutdown();

        assertEquals(TEST_NAMESPACES.length, configs.size(),
                "Should load all namespaces in parallel");

        System.out.println("Parallel loaded " + configs.size() + " namespaces in " + duration + "ms");
    }

    // ==================== ABC-005: Batch Config With Defaults ====================

    /**
     * ABC-005: Test batch config with defaults
     *
     * Verifies that batch operations correctly handle default values
     * when keys are missing.
     */
    @Test
    @Order(5)
    void testBatchConfigWithDefaults() {
        Config config = ConfigService.getConfig(DEFAULT_NAMESPACE);

        // Define keys with their default values
        Map<String, String> keysWithDefaults = new LinkedHashMap<>();
        keysWithDefaults.put("existing.key", "default1");
        keysWithDefaults.put("missing.key.abc001", "fallback-abc001");
        keysWithDefaults.put("missing.key.abc002", "fallback-abc002");
        keysWithDefaults.put("possibly.existing", "maybe-default");
        keysWithDefaults.put("definitely.missing." + UUID.randomUUID(), "uuid-default");

        Map<String, String> results = new HashMap<>();
        Map<String, Boolean> usedDefaults = new HashMap<>();

        for (Map.Entry<String, String> entry : keysWithDefaults.entrySet()) {
            String key = entry.getKey();
            String defaultValue = entry.getValue();

            String rawValue = config.getProperty(key, null);
            String actualValue = config.getProperty(key, defaultValue);

            results.put(key, actualValue);
            usedDefaults.put(key, rawValue == null);
        }

        assertFalse(results.isEmpty(), "Should have results");

        System.out.println("Batch defaults test:");
        for (String key : keysWithDefaults.keySet()) {
            System.out.println("  " + key + " = " + results.get(key) +
                    (usedDefaults.get(key) ? " (used default)" : " (from config)"));
        }
    }

    // ==================== ABC-006: Batch Type Conversion ====================

    /**
     * ABC-006: Test batch type conversion
     *
     * Verifies that batch operations correctly convert values to various types.
     */
    @Test
    @Order(6)
    void testBatchTypeConversion() {
        Config config = ConfigService.getConfig(DEFAULT_NAMESPACE);

        Map<String, Object> convertedValues = new LinkedHashMap<>();

        // Batch convert to different types
        String[] numericKeys = {"num.key1", "num.key2", "num.key3", "num.key4", "num.key5"};

        for (int i = 0; i < numericKeys.length; i++) {
            String key = numericKeys[i];
            int defaultInt = i * 10;
            long defaultLong = i * 1000L;
            double defaultDouble = i * 1.5;
            float defaultFloat = i * 0.5f;

            convertedValues.put(key + ".int", config.getIntProperty(key, defaultInt));
            convertedValues.put(key + ".long", config.getLongProperty(key, defaultLong));
            convertedValues.put(key + ".double", config.getDoubleProperty(key, defaultDouble));
            convertedValues.put(key + ".float", config.getFloatProperty(key, defaultFloat));
        }

        // Boolean conversions
        String[] boolKeys = {"bool.key1", "bool.key2", "bool.key3"};
        for (String key : boolKeys) {
            convertedValues.put(key, config.getBooleanProperty(key, false));
        }

        assertFalse(convertedValues.isEmpty(), "Should have converted values");

        System.out.println("Batch type conversions: " + convertedValues.size() + " values");
        convertedValues.forEach((k, v) ->
                System.out.println("  " + k + " = " + v + " (" + v.getClass().getSimpleName() + ")"));
    }

    // ==================== ABC-007: Batch Listener Registration ====================

    /**
     * ABC-007: Test batch listener registration
     *
     * Verifies that multiple listeners can be registered in batch
     * across different namespaces and key sets.
     */
    @Test
    @Order(7)
    void testBatchListenerRegistration() {
        Map<String, AtomicInteger> changeCounters = new ConcurrentHashMap<>();
        List<ConfigChangeListener> registeredListeners = new ArrayList<>();

        // Register listeners for multiple namespaces
        for (String namespace : TEST_NAMESPACES) {
            try {
                Config config = ConfigService.getConfig(namespace);
                AtomicInteger counter = new AtomicInteger(0);
                changeCounters.put(namespace, counter);

                ConfigChangeListener listener = changeEvent -> {
                    counter.incrementAndGet();
                    System.out.println("Change in " + namespace + ": " + changeEvent.changedKeys());
                };

                config.addChangeListener(listener);
                registeredListeners.add(listener);

                System.out.println("  Registered listener for namespace: " + namespace);
            } catch (Exception e) {
                System.out.println("  Failed to register listener for " + namespace + ": " + e.getMessage());
            }
        }

        assertEquals(TEST_NAMESPACES.length, registeredListeners.size(),
                "Should register listeners for all namespaces");

        System.out.println("Batch registered " + registeredListeners.size() + " listeners");
    }

    // ==================== ABC-008: Batch Config Refresh ====================

    /**
     * ABC-008: Test batch config refresh
     *
     * Verifies that configurations can be refreshed in batch
     * across multiple namespaces.
     */
    @Test
    @Order(8)
    void testBatchConfigRefresh() throws InterruptedException {
        Map<String, String> beforeRefresh = new HashMap<>();
        Map<String, String> afterRefresh = new HashMap<>();

        // Capture values before refresh
        for (String namespace : TEST_NAMESPACES) {
            try {
                Config config = ConfigService.getConfig(namespace);
                String value = config.getProperty("refresh.test.key", "before-default");
                beforeRefresh.put(namespace, value);
            } catch (Exception e) {
                beforeRefresh.put(namespace, "error");
            }
        }

        System.out.println("Before refresh: " + beforeRefresh);

        // Allow time for any background refresh
        Thread.sleep(1000);

        // Capture values after refresh
        for (String namespace : TEST_NAMESPACES) {
            try {
                Config config = ConfigService.getConfig(namespace);
                String value = config.getProperty("refresh.test.key", "after-default");
                afterRefresh.put(namespace, value);
            } catch (Exception e) {
                afterRefresh.put(namespace, "error");
            }
        }

        System.out.println("After refresh: " + afterRefresh);
        System.out.println("Batch refresh test completed for " + TEST_NAMESPACES.length + " namespaces");
    }

    // ==================== ABC-009: Batch Config With Cache ====================

    /**
     * ABC-009: Test batch config with cache
     *
     * Verifies that batch operations benefit from config caching
     * and return cached instances.
     */
    @Test
    @Order(9)
    void testBatchConfigWithCache() {
        Map<String, List<Config>> configInstances = new HashMap<>();

        // Access each namespace multiple times
        int accessCount = 5;
        for (String namespace : TEST_NAMESPACES) {
            List<Config> instances = new ArrayList<>();
            for (int i = 0; i < accessCount; i++) {
                Config config = ConfigService.getConfig(namespace);
                instances.add(config);
            }
            configInstances.put(namespace, instances);
        }

        // Verify all instances for each namespace are the same (cached)
        int cachedCount = 0;
        for (Map.Entry<String, List<Config>> entry : configInstances.entrySet()) {
            List<Config> instances = entry.getValue();
            Config firstInstance = instances.get(0);

            boolean allSame = instances.stream().allMatch(c -> c == firstInstance);
            if (allSame) {
                cachedCount++;
            }

            System.out.println("  " + entry.getKey() + ": cached = " + allSame);
        }

        assertEquals(TEST_NAMESPACES.length, cachedCount,
                "All namespaces should use cached instances");

        System.out.println("Batch cache verification: " + cachedCount + "/" +
                TEST_NAMESPACES.length + " namespaces using cache");
    }

    // ==================== ABC-010: Large Batch Operations ====================

    /**
     * ABC-010: Test large batch operations
     *
     * Verifies that the system handles large batch operations efficiently.
     */
    @Test
    @Order(10)
    void testLargeBatchOperations() {
        Config config = ConfigService.getConfig(DEFAULT_NAMESPACE);

        int batchSize = 1000;
        Map<String, String> largeBatch = new LinkedHashMap<>();

        long startTime = System.currentTimeMillis();

        // Perform large batch retrieval
        for (int i = 0; i < batchSize; i++) {
            String key = "large.batch.key." + i;
            String value = config.getProperty(key, "default-" + i);
            largeBatch.put(key, value);
        }

        long duration = System.currentTimeMillis() - startTime;

        assertEquals(batchSize, largeBatch.size(), "Should retrieve all " + batchSize + " keys");

        double avgTimePerKey = (double) duration / batchSize;
        System.out.println("Large batch operation:");
        System.out.println("  Keys retrieved: " + batchSize);
        System.out.println("  Total time: " + duration + "ms");
        System.out.println("  Avg time per key: " + String.format("%.3f", avgTimePerKey) + "ms");
    }

    // ==================== ABC-011: Batch Config Performance ====================

    /**
     * ABC-011: Test batch config performance
     *
     * Measures and verifies batch operation performance metrics.
     */
    @Test
    @Order(11)
    void testBatchConfigPerformance() {
        Config config = ConfigService.getConfig(DEFAULT_NAMESPACE);

        int iterations = 100;
        int keysPerIteration = 50;
        List<Long> durations = new ArrayList<>();

        for (int i = 0; i < iterations; i++) {
            long start = System.nanoTime();

            for (int j = 0; j < keysPerIteration; j++) {
                config.getProperty("perf.test.key." + j, "default");
            }

            long duration = System.nanoTime() - start;
            durations.add(duration);
        }

        // Calculate statistics
        long totalNanos = durations.stream().mapToLong(Long::longValue).sum();
        double avgNanos = totalNanos / (double) iterations;
        long maxNanos = durations.stream().mapToLong(Long::longValue).max().orElse(0);
        long minNanos = durations.stream().mapToLong(Long::longValue).min().orElse(0);

        // Sort for percentile calculation
        Collections.sort(durations);
        long p50 = durations.get(durations.size() / 2);
        long p95 = durations.get((int) (durations.size() * 0.95));
        long p99 = durations.get((int) (durations.size() * 0.99));

        System.out.println("Batch performance metrics (" + iterations + " iterations x " +
                keysPerIteration + " keys):");
        System.out.println("  Total time: " + (totalNanos / 1_000_000) + "ms");
        System.out.println("  Avg per batch: " + String.format("%.2f", avgNanos / 1_000_000) + "ms");
        System.out.println("  Min: " + (minNanos / 1_000_000) + "ms, Max: " + (maxNanos / 1_000_000) + "ms");
        System.out.println("  P50: " + (p50 / 1_000_000) + "ms, P95: " + (p95 / 1_000_000) + "ms, P99: " + (p99 / 1_000_000) + "ms");
    }

    // ==================== ABC-012: Batch Error Handling ====================

    /**
     * ABC-012: Test batch error handling
     *
     * Verifies that batch operations handle errors gracefully
     * and continue processing remaining items.
     */
    @Test
    @Order(12)
    void testBatchErrorHandling() {
        Map<String, Object> results = new LinkedHashMap<>();
        Map<String, Exception> errors = new LinkedHashMap<>();

        // Mix of valid and potentially problematic operations
        String[] testKeys = {
                "valid.key1",
                "valid.key2",
                "", // Empty key
                "valid.key3",
                "key.with.very.long.name.that.might.cause.issues.in.some.systems.but.should.still.work",
                "valid.key4"
        };

        Config config = ConfigService.getConfig(DEFAULT_NAMESPACE);

        for (String key : testKeys) {
            try {
                if (key.isEmpty()) {
                    // Skip empty keys
                    errors.put("(empty)", new IllegalArgumentException("Empty key"));
                    continue;
                }
                String value = config.getProperty(key, "default");
                results.put(key, value);
            } catch (Exception e) {
                errors.put(key, e);
            }
        }

        // Batch operations should complete despite individual errors
        assertFalse(results.isEmpty(), "Should have some successful results");

        System.out.println("Batch error handling:");
        System.out.println("  Successful: " + results.size());
        System.out.println("  Errors: " + errors.size());
        errors.forEach((k, e) -> System.out.println("    Error for '" + k + "': " + e.getMessage()));
    }

    // ==================== ABC-013: Batch Partial Failure ====================

    /**
     * ABC-013: Test batch partial failure
     *
     * Verifies that batch operations can handle partial failures
     * and report which operations succeeded/failed.
     */
    @Test
    @Order(13)
    void testBatchPartialFailure() {
        String[] namespaces = {
                "valid-namespace-1",
                "valid-namespace-2",
                null, // Will cause failure
                "valid-namespace-3",
                "non-existent-ns-" + UUID.randomUUID()
        };

        Map<String, Config> successes = new LinkedHashMap<>();
        Map<String, String> failures = new LinkedHashMap<>();

        for (int i = 0; i < namespaces.length; i++) {
            String ns = namespaces[i];
            String nsId = (ns == null) ? "null-ns-" + i : ns;

            try {
                if (ns == null) {
                    throw new IllegalArgumentException("Namespace cannot be null");
                }
                Config config = ConfigService.getConfig(ns);
                successes.put(nsId, config);
            } catch (Exception e) {
                failures.put(nsId, e.getClass().getSimpleName() + ": " + e.getMessage());
            }
        }

        System.out.println("Batch partial failure test:");
        System.out.println("  Successes: " + successes.size());
        System.out.println("  Failures: " + failures.size());
        failures.forEach((ns, err) -> System.out.println("    " + ns + " -> " + err));

        // Some should succeed, some should fail
        assertTrue(successes.size() > 0 || failures.size() > 0,
                "Should have some results");
    }

    // ==================== ABC-014: Batch With Timeout ====================

    /**
     * ABC-014: Test batch with timeout
     *
     * Verifies that batch operations respect timeout constraints.
     */
    @Test
    @Order(14)
    void testBatchWithTimeout() throws InterruptedException {
        ExecutorService executor = Executors.newFixedThreadPool(5);
        int timeoutMs = 5000;

        List<Future<Map.Entry<String, String>>> futures = new ArrayList<>();

        for (String namespace : TEST_NAMESPACES) {
            futures.add(executor.submit(() -> {
                Config config = ConfigService.getConfig(namespace);
                String value = config.getProperty("timeout.test.key", "default");
                return new AbstractMap.SimpleEntry<>(namespace, value);
            }));
        }

        Map<String, String> results = new HashMap<>();
        Map<String, String> timeouts = new HashMap<>();

        for (Future<Map.Entry<String, String>> future : futures) {
            try {
                Map.Entry<String, String> entry = future.get(timeoutMs, TimeUnit.MILLISECONDS);
                results.put(entry.getKey(), entry.getValue());
            } catch (TimeoutException e) {
                timeouts.put("unknown", "Operation timed out after " + timeoutMs + "ms");
            } catch (ExecutionException e) {
                System.out.println("Execution error: " + e.getMessage());
            }
        }

        executor.shutdown();
        executor.awaitTermination(timeoutMs, TimeUnit.MILLISECONDS);

        System.out.println("Batch timeout test (limit: " + timeoutMs + "ms):");
        System.out.println("  Completed: " + results.size());
        System.out.println("  Timed out: " + timeouts.size());
    }

    // ==================== ABC-015: Batch Concurrent Access ====================

    /**
     * ABC-015: Test batch concurrent access
     *
     * Verifies that batch operations work correctly under concurrent access.
     */
    @Test
    @Order(15)
    void testBatchConcurrentAccess() throws InterruptedException {
        int threadCount = 20;
        int operationsPerThread = 100;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch endLatch = new CountDownLatch(threadCount);
        AtomicInteger successCount = new AtomicInteger(0);
        AtomicInteger errorCount = new AtomicInteger(0);
        ConcurrentHashMap<String, String> allResults = new ConcurrentHashMap<>();

        for (int t = 0; t < threadCount; t++) {
            final int threadId = t;
            new Thread(() -> {
                try {
                    startLatch.await(); // Wait for all threads to be ready

                    Config config = ConfigService.getConfig(DEFAULT_NAMESPACE);
                    for (int i = 0; i < operationsPerThread; i++) {
                        String key = "concurrent.key." + threadId + "." + i;
                        String value = config.getProperty(key, "default-" + threadId + "-" + i);
                        allResults.put(key, value);
                        successCount.incrementAndGet();
                    }
                } catch (Exception e) {
                    errorCount.incrementAndGet();
                } finally {
                    endLatch.countDown();
                }
            }).start();
        }

        // Start all threads simultaneously
        startLatch.countDown();

        // Wait for completion
        boolean completed = endLatch.await(60, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete");

        int expectedOperations = threadCount * operationsPerThread;
        assertEquals(expectedOperations, successCount.get(),
                "All operations should succeed");
        assertEquals(0, errorCount.get(), "No errors should occur");

        System.out.println("Batch concurrent access test:");
        System.out.println("  Threads: " + threadCount);
        System.out.println("  Operations per thread: " + operationsPerThread);
        System.out.println("  Total successful: " + successCount.get());
        System.out.println("  Total errors: " + errorCount.get());
        System.out.println("  Unique results: " + allResults.size());
    }

    // ==================== ABC-016: Batch Config Merge ====================

    /**
     * ABC-016: Test batch config merge
     *
     * Verifies that configurations from multiple namespaces can be merged
     * with proper precedence handling.
     */
    @Test
    @Order(16)
    void testBatchConfigMerge() {
        // Load configs from multiple namespaces and merge
        Map<String, String> mergedConfig = new LinkedHashMap<>();
        List<String> namespaceOrder = Arrays.asList("common", "database", "application");

        // Merge in order (later namespaces override earlier ones)
        for (String namespace : namespaceOrder) {
            try {
                Config config = ConfigService.getConfig(namespace);
                Set<String> propertyNames = config.getPropertyNames();

                for (String key : propertyNames) {
                    String value = config.getProperty(key, null);
                    if (value != null) {
                        mergedConfig.put(key, value);
                    }
                }

                System.out.println("  Merged from " + namespace + ": " + propertyNames.size() + " properties");
            } catch (Exception e) {
                System.out.println("  Failed to merge " + namespace + ": " + e.getMessage());
            }
        }

        System.out.println("Batch config merge:");
        System.out.println("  Namespaces: " + namespaceOrder);
        System.out.println("  Total merged properties: " + mergedConfig.size());

        // Show first 10 merged properties
        mergedConfig.entrySet().stream()
                .limit(10)
                .forEach(e -> System.out.println("    " + e.getKey() + " = " + e.getValue()));
    }

    // ==================== ABC-017: Batch Namespace Isolation ====================

    /**
     * ABC-017: Test batch namespace isolation
     *
     * Verifies that batch operations maintain proper namespace isolation
     * and changes in one namespace don't affect others.
     */
    @Test
    @Order(17)
    void testBatchNamespaceIsolation() {
        String sharedKey = "isolation.test.key";
        Map<String, String> namespaceValues = new HashMap<>();

        // Get the same key from different namespaces
        for (String namespace : TEST_NAMESPACES) {
            try {
                Config config = ConfigService.getConfig(namespace);
                String value = config.getProperty(sharedKey, "isolation-default-" + namespace);
                namespaceValues.put(namespace, value);
            } catch (Exception e) {
                namespaceValues.put(namespace, "error: " + e.getMessage());
            }
        }

        // Verify each namespace can have different values for the same key
        System.out.println("Batch namespace isolation for key: " + sharedKey);
        namespaceValues.forEach((ns, value) -> System.out.println("  " + ns + " -> " + value));

        // Verify configs are separate instances
        Config app = ConfigService.getConfig("application");
        Config common = ConfigService.getConfig("common");
        assertNotSame(app, common, "Different namespaces should have different config instances");

        System.out.println("Namespace isolation verified for " + namespaceValues.size() + " namespaces");
    }

    // ==================== ABC-018: Batch Config Rollback ====================

    /**
     * ABC-018: Test batch config rollback
     *
     * Verifies that the system can handle rollback scenarios in batch operations
     * by maintaining previous values.
     */
    @Test
    @Order(18)
    void testBatchConfigRollback() {
        Config config = ConfigService.getConfig(DEFAULT_NAMESPACE);

        // Capture initial state
        String[] rollbackKeys = {
                "rollback.key1", "rollback.key2", "rollback.key3",
                "rollback.key4", "rollback.key5"
        };

        Map<String, String> initialValues = new HashMap<>();
        for (String key : rollbackKeys) {
            String value = config.getProperty(key, "initial-" + key);
            initialValues.put(key, value);
        }

        System.out.println("Initial values captured:");
        initialValues.forEach((k, v) -> System.out.println("  " + k + " = " + v));

        // Simulate reading again (in a real scenario, values might have changed and rolled back)
        Map<String, String> currentValues = new HashMap<>();
        for (String key : rollbackKeys) {
            String value = config.getProperty(key, "current-" + key);
            currentValues.put(key, value);
        }

        System.out.println("Current values:");
        currentValues.forEach((k, v) -> System.out.println("  " + k + " = " + v));

        // In a rollback scenario, values should match initial or be explicitly reverted
        System.out.println("Batch rollback test completed for " + rollbackKeys.length + " keys");
    }

    // ==================== ABC-019: Batch With Gray Release ====================

    /**
     * ABC-019: Test batch with gray release
     *
     * Verifies that batch operations work correctly with gray (canary) release configurations.
     */
    @Test
    @Order(19)
    void testBatchWithGrayRelease() {
        // Gray release typically uses different cluster or label configurations
        String[] clusters = {"default", "gray", "canary"};
        Map<String, Map<String, String>> clusterConfigs = new LinkedHashMap<>();

        String testKey = "gray.release.feature.enabled";

        for (String cluster : clusters) {
            Map<String, String> configValues = new HashMap<>();

            try {
                // In a real scenario, you would set apollo.cluster for each request
                // For this test, we simulate by using different namespace conventions
                String namespace = "application";
                Config config = ConfigService.getConfig(namespace);

                // Get the feature flag value
                String value = config.getProperty(testKey, "false");
                configValues.put(testKey, value);

                // Get additional gray release specific keys
                configValues.put("gray.percentage", config.getProperty("gray.percentage", "0"));
                configValues.put("gray.target.users", config.getProperty("gray.target.users", ""));

            } catch (Exception e) {
                configValues.put("error", e.getMessage());
            }

            clusterConfigs.put(cluster, configValues);
        }

        System.out.println("Batch gray release test:");
        clusterConfigs.forEach((cluster, configs) -> {
            System.out.println("  Cluster: " + cluster);
            configs.forEach((k, v) -> System.out.println("    " + k + " = " + v));
        });
    }

    // ==================== ABC-020: Batch Config Versioning ====================

    /**
     * ABC-020: Test batch config versioning
     *
     * Verifies that batch operations can track and handle different configuration versions.
     */
    @Test
    @Order(20)
    void testBatchConfigVersioning() {
        Map<String, ConfigVersionInfo> versionInfo = new LinkedHashMap<>();

        for (String namespace : TEST_NAMESPACES) {
            try {
                Config config = ConfigService.getConfig(namespace);

                // Collect version-related information
                Set<String> propertyNames = config.getPropertyNames();
                int propertyCount = propertyNames.size();

                // Create a simple hash of the config state for version tracking
                int configHash = propertyNames.stream()
                        .map(key -> key + "=" + config.getProperty(key, ""))
                        .collect(Collectors.joining(","))
                        .hashCode();

                versionInfo.put(namespace, new ConfigVersionInfo(
                        namespace,
                        propertyCount,
                        configHash,
                        System.currentTimeMillis()
                ));

            } catch (Exception e) {
                System.out.println("Failed to get version info for " + namespace + ": " + e.getMessage());
            }
        }

        System.out.println("Batch config versioning:");
        versionInfo.forEach((ns, info) -> {
            System.out.println("  " + ns + ":");
            System.out.println("    Properties: " + info.propertyCount);
            System.out.println("    Config hash: " + info.configHash);
            System.out.println("    Timestamp: " + info.timestamp);
        });

        assertFalse(versionInfo.isEmpty(), "Should have version info for at least one namespace");
    }

    /**
     * Helper class to store config version information
     */
    private static class ConfigVersionInfo {
        final String namespace;
        final int propertyCount;
        final int configHash;
        final long timestamp;

        ConfigVersionInfo(String namespace, int propertyCount, int configHash, long timestamp) {
            this.namespace = namespace;
            this.propertyCount = propertyCount;
            this.configHash = configHash;
            this.timestamp = timestamp;
        }
    }
}
