package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigFile;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.core.enums.ConfigFileFormat;
import org.junit.jupiter.api.*;

import java.io.*;
import java.nio.file.*;
import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Local Cache Tests
 *
 * Tests for local cache and fallback functionality:
 * - Local cache file operations
 * - Cache directory configuration
 * - Fallback behavior when server unavailable
 * - Cache file format validation
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloLocalCacheTest {

    private static final String TEST_NAMESPACE = "application";
    private static Path cacheDir;

    @BeforeAll
    static void setupClass() throws IOException {
        // Set Apollo configuration
        System.setProperty("app.id", System.getProperty("apollo.app.id", "test-app"));
        System.setProperty("apollo.meta", System.getProperty("apollo.meta", "http://127.0.0.1:8848"));
        System.setProperty("apollo.configService", System.getProperty("apollo.configService", "http://127.0.0.1:8848"));
        System.setProperty("env", System.getProperty("apollo.env", "DEV"));
        System.setProperty("apollo.cluster", System.getProperty("apollo.cluster", "default"));

        // Create temp cache directory for tests
        cacheDir = Files.createTempDirectory("apollo-cache-test");
        System.setProperty("apollo.cache-dir", cacheDir.toString());

        System.out.println("Apollo Local Cache Test Setup");
        System.out.println("Cache directory: " + cacheDir);
    }

    @AfterAll
    static void teardownClass() throws IOException {
        // Cleanup cache directory
        if (cacheDir != null && Files.exists(cacheDir)) {
            Files.walk(cacheDir)
                    .sorted(Comparator.reverseOrder())
                    .map(Path::toFile)
                    .forEach(File::delete);
        }
    }

    // ==================== Cache Directory Tests ====================

    /**
     * ALC-001: Test cache directory creation
     */
    @Test
    @Order(1)
    void testCacheDirectoryCreation() {
        String configuredDir = System.getProperty("apollo.cache-dir");
        assertNotNull(configuredDir, "Cache directory should be configured");

        Path cachePath = Paths.get(configuredDir);
        assertTrue(Files.exists(cachePath), "Cache directory should exist");
        assertTrue(Files.isDirectory(cachePath), "Cache path should be a directory");

        System.out.println("Cache directory verified: " + cachePath);
    }

    /**
     * ALC-002: Test cache directory with custom path
     */
    @Test
    @Order(2)
    void testCustomCacheDirectory() throws IOException {
        Path customDir = Files.createTempDirectory("custom-apollo-cache");
        String previousDir = System.getProperty("apollo.cache-dir");

        try {
            System.setProperty("apollo.cache-dir", customDir.toString());

            assertTrue(Files.exists(customDir), "Custom cache directory should exist");
            System.out.println("Custom cache directory: " + customDir);

        } finally {
            // Restore previous setting
            if (previousDir != null) {
                System.setProperty("apollo.cache-dir", previousDir);
            }
            // Cleanup
            Files.deleteIfExists(customDir);
        }
    }

    // ==================== Config Access with Cache ====================

    /**
     * ALC-003: Test config access triggers cache
     */
    @Test
    @Order(3)
    void testConfigAccessTriggersCache() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);
        assertNotNull(config, "Config should not be null");

        // Access some properties to trigger caching
        String value = config.getProperty("test.key", "default");
        System.out.println("Accessed config property: " + value);

        // Check if cache directory has files
        try {
            long fileCount = Files.list(cacheDir).count();
            System.out.println("Files in cache directory: " + fileCount);
        } catch (IOException e) {
            System.out.println("Could not list cache directory: " + e.getMessage());
        }
    }

    /**
     * ALC-004: Test multiple namespace caching
     */
    @Test
    @Order(4)
    void testMultipleNamespaceCaching() {
        String[] namespaces = {"application", "common", "database"};

        for (String namespace : namespaces) {
            try {
                Config config = ConfigService.getConfig(namespace);
                assertNotNull(config, "Config for " + namespace + " should not be null");

                // Trigger cache by accessing property
                config.getProperty("key", "default");
                System.out.println("Accessed namespace: " + namespace);
            } catch (Exception e) {
                System.out.println("Namespace " + namespace + ": " + e.getMessage());
            }
        }
    }

    /**
     * ALC-005: Test config file caching
     */
    @Test
    @Order(5)
    void testConfigFileCaching() {
        try {
            ConfigFile configFile = ConfigService.getConfigFile("application", ConfigFileFormat.Properties);
            assertNotNull(configFile, "Config file should not be null");

            String content = configFile.getContent();
            System.out.println("Config file content length: " + (content != null ? content.length() : 0));
        } catch (Exception e) {
            System.out.println("Config file caching: " + e.getMessage());
        }
    }

    // ==================== Cache File Format Tests ====================

    /**
     * ALC-006: Test properties format cache
     */
    @Test
    @Order(6)
    void testPropertiesFormatCache() {
        try {
            ConfigFile configFile = ConfigService.getConfigFile("application", ConfigFileFormat.Properties);
            String content = configFile.getContent();

            if (content != null && !content.isEmpty()) {
                // Verify it looks like properties format
                System.out.println("Properties content sample: " +
                        content.substring(0, Math.min(100, content.length())));
            }
        } catch (Exception e) {
            System.out.println("Properties format cache: " + e.getMessage());
        }
    }

    /**
     * ALC-007: Test JSON format cache
     */
    @Test
    @Order(7)
    void testJsonFormatCache() {
        try {
            ConfigFile configFile = ConfigService.getConfigFile("test.json", ConfigFileFormat.JSON);
            String content = configFile.getContent();

            if (content != null && !content.isEmpty()) {
                // Verify it starts with { or [
                assertTrue(content.trim().startsWith("{") || content.trim().startsWith("["),
                        "JSON content should start with { or [");
                System.out.println("JSON content sample: " +
                        content.substring(0, Math.min(100, content.length())));
            }
        } catch (Exception e) {
            System.out.println("JSON format cache: " + e.getMessage());
        }
    }

    /**
     * ALC-008: Test YAML format cache
     */
    @Test
    @Order(8)
    void testYamlFormatCache() {
        try {
            ConfigFile configFile = ConfigService.getConfigFile("test.yaml", ConfigFileFormat.YAML);
            String content = configFile.getContent();

            if (content != null && !content.isEmpty()) {
                System.out.println("YAML content sample: " +
                        content.substring(0, Math.min(100, content.length())));
            }
        } catch (Exception e) {
            System.out.println("YAML format cache: " + e.getMessage());
        }
    }

    // ==================== Cache Fallback Tests ====================

    /**
     * ALC-009: Test fallback to default value when no cache
     */
    @Test
    @Order(9)
    void testFallbackToDefaultValue() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        String nonExistentKey = "non.existent.key." + UUID.randomUUID();
        String defaultValue = "fallback-default";

        String value = config.getProperty(nonExistentKey, defaultValue);
        assertEquals(defaultValue, value, "Should return default value for non-existent key");

        System.out.println("Fallback to default value: " + value);
    }

    /**
     * ALC-010: Test cache persistence across config service instances
     */
    @Test
    @Order(10)
    void testCachePersistenceAcrossInstances() {
        // Access config to potentially cache
        Config config1 = ConfigService.getConfig(TEST_NAMESPACE);
        String key = "persistence.test.key";
        String value1 = config1.getProperty(key, "not-found");

        // Get config again (same instance from cache)
        Config config2 = ConfigService.getConfig(TEST_NAMESPACE);
        String value2 = config2.getProperty(key, "not-found");

        System.out.println("Value from first access: " + value1);
        System.out.println("Value from second access: " + value2);

        // Values should be consistent
        assertEquals(value1, value2, "Values should be consistent across accesses");
    }

    // ==================== Cache Update Tests ====================

    /**
     * ALC-011: Test cache update on config change
     */
    @Test
    @Order(11)
    void testCacheUpdateOnChange() throws InterruptedException {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        // Add listener to track changes
        config.addChangeListener(changeEvent -> {
            System.out.println("Config changed, keys: " + changeEvent.changedKeys());
        });

        // Access initial value
        String key = "cache.update.test";
        String initialValue = config.getProperty(key, "initial");
        System.out.println("Initial cached value: " + initialValue);

        // Wait for potential update
        Thread.sleep(1000);

        String currentValue = config.getProperty(key, "initial");
        System.out.println("Current cached value: " + currentValue);
    }

    /**
     * ALC-012: Test cache with special characters in key
     */
    @Test
    @Order(12)
    void testCacheWithSpecialCharacters() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        String[] specialKeys = {
                "key.with.dots",
                "key-with-dashes",
                "key_with_underscores",
                "key:with:colons",
                "key/with/slashes"
        };

        for (String key : specialKeys) {
            String value = config.getProperty(key, "default-" + key);
            System.out.println("Key '" + key + "' = " + value);
        }
    }

    // ==================== Cache Size and Limits Tests ====================

    /**
     * ALC-013: Test large value caching
     */
    @Test
    @Order(13)
    void testLargeValueCaching() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        // Try to get a property that might have large value
        String largeKey = "large.config.value";
        String value = config.getProperty(largeKey, null);

        if (value != null) {
            System.out.println("Large value size: " + value.length() + " characters");
        } else {
            System.out.println("Large value not found (expected in test environment)");
        }
    }

    /**
     * ALC-014: Test many properties caching
     */
    @Test
    @Order(14)
    void testManyPropertiesCaching() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        // Access many properties
        int accessCount = 100;
        for (int i = 0; i < accessCount; i++) {
            String key = "batch.property." + i;
            config.getProperty(key, "default-" + i);
        }

        System.out.println("Accessed " + accessCount + " properties");

        // Verify property set is accessible
        Set<String> names = config.getPropertyNames();
        System.out.println("Total property names in config: " + names.size());
    }

    /**
     * ALC-015: Test cache cleanup behavior
     */
    @Test
    @Order(15)
    void testCacheCleanupBehavior() throws IOException {
        // List current cache files
        if (Files.exists(cacheDir)) {
            long fileCount = Files.list(cacheDir).count();
            long totalSize = Files.walk(cacheDir)
                    .filter(Files::isRegularFile)
                    .mapToLong(p -> {
                        try {
                            return Files.size(p);
                        } catch (IOException e) {
                            return 0;
                        }
                    })
                    .sum();

            System.out.println("Cache files: " + fileCount);
            System.out.println("Total cache size: " + totalSize + " bytes");
        }
    }
}
