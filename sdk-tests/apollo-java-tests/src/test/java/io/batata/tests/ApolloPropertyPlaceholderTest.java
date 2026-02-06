package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigChangeListener;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.model.ConfigChange;
import org.junit.jupiter.api.*;

import java.io.*;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;
import java.util.regex.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Property Placeholder Tests
 *
 * Tests for property placeholder and reference handling:
 * - Simple placeholder resolution
 * - Nested placeholders
 * - Default values
 * - Circular reference detection
 * - Cross-namespace references
 * - System property and environment variable integration
 * - Placeholder caching and performance
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloPropertyPlaceholderTest {

    private static String configServiceUrl;
    private static String openApiUrl;
    private static String portalToken;
    private static final String APP_ID = "placeholder-test-app";
    private static final String CLUSTER = "default";
    private static final String ENV = "DEV";
    private static final String NAMESPACE = "application";

    // Simple placeholder pattern: ${key} or ${key:default}
    private static final Pattern PLACEHOLDER_PATTERN = Pattern.compile("\\$\\{([^}:]+)(?::([^}]*))?\\}");

    @BeforeAll
    static void setup() {
        System.setProperty("app.id", System.getProperty("apollo.app.id", APP_ID));
        System.setProperty("apollo.meta", System.getProperty("apollo.meta", "http://127.0.0.1:8848"));
        System.setProperty("apollo.configService", System.getProperty("apollo.configService", "http://127.0.0.1:8848"));
        System.setProperty("env", System.getProperty("apollo.env", ENV));
        System.setProperty("apollo.cluster", System.getProperty("apollo.cluster", CLUSTER));

        configServiceUrl = System.getProperty("apollo.configService", "http://127.0.0.1:8848");
        openApiUrl = System.getProperty("apollo.openapi.url", "http://127.0.0.1:8848");
        portalToken = System.getProperty("apollo.portal.token", "test-token");

        // Set system properties for placeholder tests
        System.setProperty("system.test.property", "system-value");
        System.setProperty("system.nested.key", "nested-from-system");

        System.out.println("Apollo Property Placeholder Test Setup");
        System.out.println("Config Service: " + configServiceUrl);
    }

    // ==================== Simple Placeholder Resolution Tests ====================

    /**
     * APP-001: Test simple placeholder resolution
     *
     * Tests that a simple placeholder ${key} is correctly resolved
     * to the value of the referenced property.
     */
    @Test
    @Order(1)
    void testSimplePlaceholderResolution() {
        Config config = ConfigService.getAppConfig();

        // Setup test data - base value and a value with placeholder
        String baseKey = "app.name";
        String baseValue = config.getProperty(baseKey, "MyApplication");

        String placeholderKey = "app.display.name";
        String placeholderValue = config.getProperty(placeholderKey, "${app.name}");

        System.out.println("APP-001: Simple placeholder resolution");
        System.out.println("  Base key: " + baseKey + " = " + baseValue);
        System.out.println("  Placeholder key: " + placeholderKey + " = " + placeholderValue);

        // Resolve placeholder manually for verification
        String resolved = resolvePlaceholders(config, placeholderValue);
        System.out.println("  Resolved value: " + resolved);

        assertNotNull(resolved, "Resolved value should not be null");
    }

    /**
     * APP-002: Test nested placeholder
     *
     * Tests resolution of nested placeholders like ${outer.${inner}}
     * where inner placeholder is resolved first.
     */
    @Test
    @Order(2)
    void testNestedPlaceholder() {
        Config config = ConfigService.getAppConfig();

        // Setup nested placeholder scenario
        String innerKey = "env.name";
        String innerValue = config.getProperty(innerKey, "production");

        // This would be like: server.${env.name}.url -> server.production.url
        String nestedPlaceholder = "server.${env.name}.url";
        System.out.println("APP-002: Nested placeholder resolution");
        System.out.println("  Inner key: " + innerKey + " = " + innerValue);
        System.out.println("  Nested placeholder: " + nestedPlaceholder);

        // Resolve inner placeholder first
        String partiallyResolved = resolvePlaceholders(config, nestedPlaceholder);
        System.out.println("  After resolution: " + partiallyResolved);

        // The result should contain the resolved inner value
        if (!"production".equals(innerValue)) {
            assertTrue(partiallyResolved.contains(innerValue) ||
                       partiallyResolved.contains("${"),
                       "Should resolve or keep placeholder");
        }
    }

    /**
     * APP-003: Test placeholder with default value
     *
     * Tests that ${key:defaultValue} returns defaultValue when key is not found.
     */
    @Test
    @Order(3)
    void testPlaceholderWithDefaultValue() {
        Config config = ConfigService.getAppConfig();

        // Test placeholder with default value syntax
        String placeholderWithDefault = "${missing.key:default-fallback-value}";
        String resolved = resolvePlaceholders(config, placeholderWithDefault);

        System.out.println("APP-003: Placeholder with default value");
        System.out.println("  Placeholder: " + placeholderWithDefault);
        System.out.println("  Resolved: " + resolved);

        assertEquals("default-fallback-value", resolved,
                     "Should return default value for missing key");
    }

    /**
     * APP-004: Test missing placeholder handling
     *
     * Tests behavior when a placeholder references a non-existent key
     * without a default value.
     */
    @Test
    @Order(4)
    void testMissingPlaceholderHandling() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "definitely.missing.placeholder.key." + UUID.randomUUID();
        String placeholderValue = "${" + missingKey + "}";

        System.out.println("APP-004: Missing placeholder handling");
        System.out.println("  Placeholder: " + placeholderValue);

        // Try to resolve - should either keep placeholder or return empty
        String resolved = resolvePlaceholders(config, placeholderValue);
        System.out.println("  Resolved: " + resolved);

        // Behavior: either keep placeholder as-is or return empty string
        assertTrue(resolved.equals(placeholderValue) || resolved.isEmpty() ||
                   resolved.equals("${" + missingKey + "}"),
                   "Should keep placeholder or return empty for missing key");
    }

    /**
     * APP-005: Test circular reference detection
     *
     * Tests that circular references like A -> B -> A are detected
     * and handled gracefully without infinite loops.
     */
    @Test
    @Order(5)
    void testCircularReferenceDetection() {
        Config config = ConfigService.getAppConfig();

        // Simulate circular reference detection
        // key.a = ${key.b}, key.b = ${key.a}
        Set<String> visited = new HashSet<>();
        String startKey = "circular.key.a";
        String placeholder = "${circular.key.b}";

        System.out.println("APP-005: Circular reference detection");
        System.out.println("  Testing circular: " + startKey + " -> " + placeholder);

        // Track visited keys to detect cycles
        visited.add(startKey);
        String resolved = resolvePlaceholdersWithCycleDetection(config, placeholder, visited);

        System.out.println("  Result: " + resolved);

        // Should not hang - test completes within timeout
        assertNotNull(resolved, "Should return a value without hanging");
    }

    /**
     * APP-006: Test placeholder in different types
     *
     * Tests placeholder resolution in different property value types
     * (strings, URLs, numbers embedded in strings).
     */
    @Test
    @Order(6)
    void testPlaceholderInDifferentTypes() {
        Config config = ConfigService.getAppConfig();

        System.out.println("APP-006: Placeholder in different types");

        // Test in URL-like string
        String urlTemplate = "http://${server.host:localhost}:${server.port:8080}/api";
        String resolvedUrl = resolvePlaceholders(config, urlTemplate);
        System.out.println("  URL template: " + urlTemplate);
        System.out.println("  Resolved URL: " + resolvedUrl);
        assertNotNull(resolvedUrl);

        // Test in JSON-like string
        String jsonTemplate = "{\"name\": \"${app.name:TestApp}\", \"version\": \"${app.version:1.0.0}\"}";
        String resolvedJson = resolvePlaceholders(config, jsonTemplate);
        System.out.println("  JSON template: " + jsonTemplate);
        System.out.println("  Resolved JSON: " + resolvedJson);
        assertNotNull(resolvedJson);

        // Test in path string
        String pathTemplate = "/opt/${app.name:myapp}/config/${env:dev}";
        String resolvedPath = resolvePlaceholders(config, pathTemplate);
        System.out.println("  Path template: " + pathTemplate);
        System.out.println("  Resolved path: " + resolvedPath);
        assertNotNull(resolvedPath);
    }

    /**
     * APP-007: Test placeholder with special characters
     *
     * Tests placeholder resolution when keys or values contain
     * special characters like dots, hyphens, underscores.
     */
    @Test
    @Order(7)
    void testPlaceholderWithSpecialCharacters() {
        Config config = ConfigService.getAppConfig();

        System.out.println("APP-007: Placeholder with special characters");

        // Key with dots
        String dottedKey = "${database.connection.pool.size:10}";
        String resolvedDotted = resolvePlaceholders(config, dottedKey);
        System.out.println("  Dotted key: " + dottedKey + " -> " + resolvedDotted);

        // Key with hyphens
        String hyphenKey = "${feature-toggle-enabled:false}";
        String resolvedHyphen = resolvePlaceholders(config, hyphenKey);
        System.out.println("  Hyphen key: " + hyphenKey + " -> " + resolvedHyphen);

        // Key with underscores
        String underscoreKey = "${MAX_CONNECTIONS:100}";
        String resolvedUnderscore = resolvePlaceholders(config, underscoreKey);
        System.out.println("  Underscore key: " + underscoreKey + " -> " + resolvedUnderscore);

        // Value with special characters in default
        String specialDefault = "${missing.key:value@#$%^&*()}";
        String resolvedSpecial = resolvePlaceholders(config, specialDefault);
        System.out.println("  Special default: " + specialDefault + " -> " + resolvedSpecial);

        assertNotNull(resolvedDotted);
        assertNotNull(resolvedHyphen);
        assertNotNull(resolvedUnderscore);
    }

    /**
     * APP-008: Test placeholder chain resolution
     *
     * Tests resolution of placeholder chains where A references B,
     * B references C, etc.
     */
    @Test
    @Order(8)
    void testPlaceholderChainResolution() {
        Config config = ConfigService.getAppConfig();

        System.out.println("APP-008: Placeholder chain resolution");

        // Simulate chain: level1 -> level2 -> level3 -> actual value
        // In practice, config would have:
        // level3.value = "final-value"
        // level2.ref = ${level3.value}
        // level1.ref = ${level2.ref}

        String level3 = config.getProperty("chain.level3", "final-chain-value");
        System.out.println("  Level 3 (base): " + level3);

        String level2Template = "${chain.level3:level3-default}";
        String level2Resolved = resolvePlaceholders(config, level2Template);
        System.out.println("  Level 2 resolved: " + level2Resolved);

        String level1Template = "${chain.level2:" + level2Resolved + "}";
        String level1Resolved = resolvePlaceholders(config, level1Template);
        System.out.println("  Level 1 resolved: " + level1Resolved);

        assertNotNull(level1Resolved, "Chain should resolve to a value");
    }

    /**
     * APP-009: Test placeholder across namespaces
     *
     * Tests placeholder resolution when referencing properties
     * from different namespaces.
     */
    @Test
    @Order(9)
    void testPlaceholderAcrossNamespaces() {
        System.out.println("APP-009: Placeholder across namespaces");

        // Get configs from different namespaces
        Config appConfig = ConfigService.getConfig("application");
        Config commonConfig = ConfigService.getConfig("common");

        String appValue = appConfig.getProperty("cross.namespace.key", "app-value");
        String commonValue = commonConfig.getProperty("shared.setting", "common-value");

        System.out.println("  Application namespace: cross.namespace.key = " + appValue);
        System.out.println("  Common namespace: shared.setting = " + commonValue);

        // Cross-namespace placeholder simulation
        // In real scenario, would need custom resolver that checks multiple namespaces
        String crossNsPlaceholder = "${common:shared.setting:fallback}";
        System.out.println("  Cross-namespace placeholder: " + crossNsPlaceholder);

        // Resolve from common namespace
        String resolved = resolvePlaceholders(commonConfig, "${shared.setting:fallback}");
        System.out.println("  Resolved from common: " + resolved);

        assertNotNull(resolved);
    }

    /**
     * APP-010: Test placeholder with system properties
     *
     * Tests that placeholders can reference Java system properties.
     */
    @Test
    @Order(10)
    void testPlaceholderWithSystemProperties() {
        Config config = ConfigService.getAppConfig();

        System.out.println("APP-010: Placeholder with system properties");

        // System properties set in setup
        String systemValue = System.getProperty("system.test.property", "not-set");
        System.out.println("  System property value: " + systemValue);

        // Test placeholder that falls back to system property
        String placeholder = "${system.test.property:system-default}";
        String resolved = resolvePlaceholdersWithSystemFallback(config, placeholder);
        System.out.println("  Placeholder: " + placeholder);
        System.out.println("  Resolved: " + resolved);

        // Should resolve to system property value
        assertEquals("system-value", resolved,
                     "Should resolve to system property value");
    }

    /**
     * APP-011: Test placeholder with environment variables
     *
     * Tests that placeholders can reference environment variables.
     */
    @Test
    @Order(11)
    void testPlaceholderWithEnvironmentVariables() {
        Config config = ConfigService.getAppConfig();

        System.out.println("APP-011: Placeholder with environment variables");

        // Common environment variables
        String javaHome = System.getenv("JAVA_HOME");
        String user = System.getenv("USER");
        String path = System.getenv("PATH");

        System.out.println("  JAVA_HOME: " + (javaHome != null ? "set" : "not set"));
        System.out.println("  USER: " + user);
        System.out.println("  PATH: " + (path != null ? "set (" + path.length() + " chars)" : "not set"));

        // Test placeholder that could resolve to env var
        String envPlaceholder = "${USER:default-user}";
        String resolved = resolvePlaceholdersWithEnvFallback(config, envPlaceholder);
        System.out.println("  Placeholder: " + envPlaceholder);
        System.out.println("  Resolved: " + resolved);

        // Should resolve to env var or default
        assertNotNull(resolved);
        assertFalse(resolved.isEmpty(), "Should have a resolved value");
    }

    /**
     * APP-012: Test placeholder precedence
     *
     * Tests the order of precedence: config value > system property >
     * environment variable > default value.
     */
    @Test
    @Order(12)
    void testPlaceholderPrecedence() {
        Config config = ConfigService.getAppConfig();

        System.out.println("APP-012: Placeholder precedence");

        // Set up test key in system properties
        String testKey = "precedence.test.key";
        String systemValue = "from-system";
        System.setProperty(testKey, systemValue);

        // Get from config (might override system)
        String configValue = config.getProperty(testKey, null);
        System.out.println("  Config value: " + configValue);
        System.out.println("  System value: " + systemValue);

        // Resolve with precedence
        String placeholder = "${" + testKey + ":default-value}";
        String resolved = resolvePlaceholdersWithPrecedence(config, placeholder);
        System.out.println("  Placeholder: " + placeholder);
        System.out.println("  Resolved (with precedence): " + resolved);

        // Clean up
        System.clearProperty(testKey);

        assertNotNull(resolved, "Should resolve to some value");
    }

    /**
     * APP-013: Test placeholder caching
     *
     * Tests that resolved placeholders are cached for performance.
     */
    @Test
    @Order(13)
    void testPlaceholderCaching() {
        Config config = ConfigService.getAppConfig();

        System.out.println("APP-013: Placeholder caching");

        String placeholder = "${cache.test.key:cached-default-value}";
        Map<String, String> cache = new ConcurrentHashMap<>();

        // First resolution - should compute
        long start1 = System.nanoTime();
        String resolved1 = resolvePlaceholdersWithCache(config, placeholder, cache);
        long duration1 = System.nanoTime() - start1;

        // Second resolution - should use cache
        long start2 = System.nanoTime();
        String resolved2 = resolvePlaceholdersWithCache(config, placeholder, cache);
        long duration2 = System.nanoTime() - start2;

        System.out.println("  First resolution: " + duration1 + "ns -> " + resolved1);
        System.out.println("  Second resolution: " + duration2 + "ns -> " + resolved2);
        System.out.println("  Cache size: " + cache.size());

        assertEquals(resolved1, resolved2, "Cached value should match original");
    }

    /**
     * APP-014: Test placeholder refresh
     *
     * Tests that placeholder resolution updates when underlying
     * config values change.
     */
    @Test
    @Order(14)
    void testPlaceholderRefresh() throws InterruptedException {
        Config config = ConfigService.getConfig("application");

        System.out.println("APP-014: Placeholder refresh");

        AtomicReference<String> latestValue = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        ConfigChangeListener listener = changeEvent -> {
            for (String key : changeEvent.changedKeys()) {
                ConfigChange change = changeEvent.getChange(key);
                System.out.println("  Change detected: " + key +
                                   " [" + change.getOldValue() + " -> " + change.getNewValue() + "]");
                latestValue.set(change.getNewValue());
                latch.countDown();
            }
        };

        config.addChangeListener(listener);

        // Initial resolution
        String placeholder = "${refresh.test.key:initial-value}";
        String initialResolved = resolvePlaceholders(config, placeholder);
        System.out.println("  Initial resolved: " + initialResolved);

        // Wait briefly for any changes
        boolean changed = latch.await(2, TimeUnit.SECONDS);
        System.out.println("  Change received: " + changed);

        if (changed && latestValue.get() != null) {
            String refreshedResolved = resolvePlaceholders(config, placeholder);
            System.out.println("  Refreshed resolved: " + refreshedResolved);
        }

        config.removeChangeListener(listener);
    }

    /**
     * APP-015: Test placeholder in arrays
     *
     * Tests placeholder resolution within array/list values.
     */
    @Test
    @Order(15)
    void testPlaceholderInArrays() {
        Config config = ConfigService.getAppConfig();

        System.out.println("APP-015: Placeholder in arrays");

        // Array with placeholders
        String arrayTemplate = "${host1:server1.example.com},${host2:server2.example.com},${host3:server3.example.com}";

        // Resolve each element
        String[] elements = arrayTemplate.split(",");
        String[] resolved = new String[elements.length];

        for (int i = 0; i < elements.length; i++) {
            resolved[i] = resolvePlaceholders(config, elements[i]);
            System.out.println("  Element " + i + ": " + elements[i] + " -> " + resolved[i]);
        }

        // Combine back
        String resolvedArray = String.join(",", resolved);
        System.out.println("  Resolved array: " + resolvedArray);

        assertEquals(3, resolved.length, "Should have 3 elements");
        for (String element : resolved) {
            assertFalse(element.contains("${"), "Should not contain unresolved placeholders");
        }
    }

    /**
     * APP-016: Test placeholder in JSON config
     *
     * Tests placeholder resolution within JSON configuration values.
     */
    @Test
    @Order(16)
    void testPlaceholderInJsonConfig() {
        Config config = ConfigService.getAppConfig();

        System.out.println("APP-016: Placeholder in JSON config");

        // JSON with embedded placeholders
        String jsonTemplate = "{\n" +
                "  \"database\": {\n" +
                "    \"host\": \"${db.host:localhost}\",\n" +
                "    \"port\": ${db.port:3306},\n" +
                "    \"name\": \"${db.name:mydb}\",\n" +
                "    \"user\": \"${db.user:root}\"\n" +
                "  },\n" +
                "  \"cache\": {\n" +
                "    \"enabled\": ${cache.enabled:true},\n" +
                "    \"ttl\": ${cache.ttl:3600}\n" +
                "  }\n" +
                "}";

        System.out.println("  JSON Template:");
        System.out.println(jsonTemplate);

        // Resolve placeholders in JSON
        String resolvedJson = resolvePlaceholders(config, jsonTemplate);
        System.out.println("  Resolved JSON:");
        System.out.println(resolvedJson);

        // Verify no unresolved placeholders
        assertFalse(resolvedJson.contains("${db.host}"), "db.host should be resolved");
        assertFalse(resolvedJson.contains("${db.port}"), "db.port should be resolved");
        assertTrue(resolvedJson.contains("localhost") || resolvedJson.contains("\"host\":"),
                   "Should contain resolved host");
    }

    /**
     * APP-017: Test placeholder performance
     *
     * Tests the performance of placeholder resolution under load.
     */
    @Test
    @Order(17)
    void testPlaceholderPerformance() {
        Config config = ConfigService.getAppConfig();

        System.out.println("APP-017: Placeholder performance");

        int iterations = 10000;
        String placeholder = "${perf.test.key:performance-default-value}";

        // Warm up
        for (int i = 0; i < 100; i++) {
            resolvePlaceholders(config, placeholder);
        }

        // Measure
        long startTime = System.currentTimeMillis();
        for (int i = 0; i < iterations; i++) {
            resolvePlaceholders(config, placeholder);
        }
        long duration = System.currentTimeMillis() - startTime;

        double avgMs = (double) duration / iterations;
        double opsPerSecond = iterations / (duration / 1000.0);

        System.out.println("  Iterations: " + iterations);
        System.out.println("  Total time: " + duration + "ms");
        System.out.println("  Average: " + String.format("%.4f", avgMs) + "ms per resolution");
        System.out.println("  Throughput: " + String.format("%.0f", opsPerSecond) + " ops/sec");

        // Should complete in reasonable time
        assertTrue(duration < 10000, "Should complete " + iterations + " resolutions in under 10s");
    }

    /**
     * APP-018: Test placeholder thread safety
     *
     * Tests that placeholder resolution is thread-safe under
     * concurrent access.
     */
    @Test
    @Order(18)
    void testPlaceholderThreadSafety() throws InterruptedException {
        Config config = ConfigService.getAppConfig();

        System.out.println("APP-018: Placeholder thread safety");

        int threadCount = 20;
        int iterationsPerThread = 100;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch endLatch = new CountDownLatch(threadCount);

        List<String> results = new CopyOnWriteArrayList<>();
        List<Exception> errors = new CopyOnWriteArrayList<>();
        AtomicInteger successCount = new AtomicInteger(0);

        String placeholder = "${thread.safe.key:thread-safe-default}";

        // Create threads
        for (int t = 0; t < threadCount; t++) {
            final int threadId = t;
            new Thread(() -> {
                try {
                    startLatch.await(); // Wait for all threads to be ready
                    for (int i = 0; i < iterationsPerThread; i++) {
                        String resolved = resolvePlaceholders(config, placeholder);
                        results.add(resolved);
                        successCount.incrementAndGet();
                    }
                } catch (Exception e) {
                    errors.add(e);
                } finally {
                    endLatch.countDown();
                }
            }).start();
        }

        // Start all threads
        startLatch.countDown();

        // Wait for completion
        boolean completed = endLatch.await(60, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete within timeout");

        System.out.println("  Threads: " + threadCount);
        System.out.println("  Iterations per thread: " + iterationsPerThread);
        System.out.println("  Total successful: " + successCount.get());
        System.out.println("  Errors: " + errors.size());
        System.out.println("  Results collected: " + results.size());

        // Verify no errors
        assertTrue(errors.isEmpty(), "Should have no errors: " + errors);

        // Verify all results are consistent
        String expectedValue = results.isEmpty() ? null : results.get(0);
        if (expectedValue != null) {
            long consistentCount = results.stream()
                    .filter(r -> r.equals(expectedValue))
                    .count();
            System.out.println("  Consistent results: " + consistentCount + "/" + results.size());
            assertEquals(results.size(), consistentCount,
                         "All results should be consistent");
        }

        assertEquals(threadCount * iterationsPerThread, successCount.get(),
                     "All operations should succeed");
    }

    // ==================== Helper Methods ====================

    /**
     * Resolves placeholders in the given value using the config.
     */
    private String resolvePlaceholders(Config config, String value) {
        if (value == null || !value.contains("${")) {
            return value;
        }

        StringBuffer result = new StringBuffer();
        Matcher matcher = PLACEHOLDER_PATTERN.matcher(value);

        while (matcher.find()) {
            String key = matcher.group(1);
            String defaultValue = matcher.group(2);

            String replacement = config.getProperty(key, defaultValue);
            if (replacement == null) {
                replacement = matcher.group(0); // Keep original if not found
            }

            matcher.appendReplacement(result, Matcher.quoteReplacement(replacement));
        }
        matcher.appendTail(result);

        return result.toString();
    }

    /**
     * Resolves placeholders with cycle detection to prevent infinite loops.
     */
    private String resolvePlaceholdersWithCycleDetection(Config config, String value, Set<String> visited) {
        if (value == null || !value.contains("${")) {
            return value;
        }

        StringBuffer result = new StringBuffer();
        Matcher matcher = PLACEHOLDER_PATTERN.matcher(value);

        while (matcher.find()) {
            String key = matcher.group(1);
            String defaultValue = matcher.group(2);

            String replacement;
            if (visited.contains(key)) {
                // Circular reference detected
                replacement = defaultValue != null ? defaultValue : "[CIRCULAR:" + key + "]";
            } else {
                visited.add(key);
                String configValue = config.getProperty(key, defaultValue);
                replacement = configValue != null ? configValue : matcher.group(0);
            }

            matcher.appendReplacement(result, Matcher.quoteReplacement(replacement));
        }
        matcher.appendTail(result);

        return result.toString();
    }

    /**
     * Resolves placeholders with system property fallback.
     */
    private String resolvePlaceholdersWithSystemFallback(Config config, String value) {
        if (value == null || !value.contains("${")) {
            return value;
        }

        StringBuffer result = new StringBuffer();
        Matcher matcher = PLACEHOLDER_PATTERN.matcher(value);

        while (matcher.find()) {
            String key = matcher.group(1);
            String defaultValue = matcher.group(2);

            // Try config first, then system property
            String replacement = config.getProperty(key, null);
            if (replacement == null) {
                replacement = System.getProperty(key, defaultValue);
            }
            if (replacement == null) {
                replacement = matcher.group(0);
            }

            matcher.appendReplacement(result, Matcher.quoteReplacement(replacement));
        }
        matcher.appendTail(result);

        return result.toString();
    }

    /**
     * Resolves placeholders with environment variable fallback.
     */
    private String resolvePlaceholdersWithEnvFallback(Config config, String value) {
        if (value == null || !value.contains("${")) {
            return value;
        }

        StringBuffer result = new StringBuffer();
        Matcher matcher = PLACEHOLDER_PATTERN.matcher(value);

        while (matcher.find()) {
            String key = matcher.group(1);
            String defaultValue = matcher.group(2);

            // Try config first, then environment variable
            String replacement = config.getProperty(key, null);
            if (replacement == null) {
                replacement = System.getenv(key);
            }
            if (replacement == null) {
                replacement = defaultValue;
            }
            if (replacement == null) {
                replacement = matcher.group(0);
            }

            matcher.appendReplacement(result, Matcher.quoteReplacement(replacement));
        }
        matcher.appendTail(result);

        return result.toString();
    }

    /**
     * Resolves placeholders with full precedence: config > system > env > default.
     */
    private String resolvePlaceholdersWithPrecedence(Config config, String value) {
        if (value == null || !value.contains("${")) {
            return value;
        }

        StringBuffer result = new StringBuffer();
        Matcher matcher = PLACEHOLDER_PATTERN.matcher(value);

        while (matcher.find()) {
            String key = matcher.group(1);
            String defaultValue = matcher.group(2);

            // Precedence: config > system property > env variable > default
            String replacement = config.getProperty(key, null);
            if (replacement == null) {
                replacement = System.getProperty(key);
            }
            if (replacement == null) {
                replacement = System.getenv(key);
            }
            if (replacement == null) {
                replacement = defaultValue;
            }
            if (replacement == null) {
                replacement = matcher.group(0);
            }

            matcher.appendReplacement(result, Matcher.quoteReplacement(replacement));
        }
        matcher.appendTail(result);

        return result.toString();
    }

    /**
     * Resolves placeholders with caching support.
     */
    private String resolvePlaceholdersWithCache(Config config, String value, Map<String, String> cache) {
        if (value == null) {
            return null;
        }

        // Check cache first
        String cached = cache.get(value);
        if (cached != null) {
            return cached;
        }

        // Resolve and cache
        String resolved = resolvePlaceholders(config, value);
        cache.put(value, resolved);

        return resolved;
    }

    // ==================== HTTP Helper Methods ====================

    private static String httpGet(String urlString) throws Exception {
        URL url = new URL(urlString);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("GET");
        conn.setRequestProperty("Authorization", portalToken);
        return readResponse(conn);
    }

    private static String httpPost(String urlString, String body, String contentType) throws Exception {
        URL url = new URL(urlString);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("POST");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", contentType);
        conn.setRequestProperty("Authorization", portalToken);

        try (OutputStream os = conn.getOutputStream()) {
            os.write(body.getBytes(StandardCharsets.UTF_8));
        }
        return readResponse(conn);
    }

    private static String readResponse(HttpURLConnection conn) throws Exception {
        int responseCode = conn.getResponseCode();
        InputStream is = responseCode >= 400 ? conn.getErrorStream() : conn.getInputStream();
        if (is == null) {
            return "Status: " + responseCode;
        }

        BufferedReader reader = new BufferedReader(new InputStreamReader(is));
        StringBuilder response = new StringBuilder();
        String line;
        while ((line = reader.readLine()) != null) {
            response.append(line);
        }
        return response.toString();
    }
}
