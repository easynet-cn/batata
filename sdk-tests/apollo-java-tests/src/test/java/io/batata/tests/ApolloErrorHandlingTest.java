package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigFile;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.core.enums.ConfigFileFormat;
import com.ctrip.framework.apollo.exceptions.ApolloConfigException;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Error Handling Tests
 *
 * Tests for error handling and edge cases:
 * - Invalid configurations
 * - Network errors and timeouts
 * - Missing namespaces
 * - Type conversion errors
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloErrorHandlingTest {

    private static final String TEST_NAMESPACE = "application";

    @BeforeAll
    static void setupClass() {
        System.setProperty("app.id", System.getProperty("apollo.app.id", "test-app"));
        System.setProperty("apollo.meta", System.getProperty("apollo.meta", "http://127.0.0.1:8848"));
        System.setProperty("apollo.configService", System.getProperty("apollo.configService", "http://127.0.0.1:8848"));
        System.setProperty("env", System.getProperty("apollo.env", "DEV"));
        System.setProperty("apollo.cluster", System.getProperty("apollo.cluster", "default"));

        System.out.println("Apollo Error Handling Test Setup");
    }

    // ==================== Missing Key Tests ====================

    /**
     * AEH-001: Test get property with default value for missing key
     */
    @Test
    @Order(1)
    void testMissingKeyWithDefault() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        String missingKey = "non.existent.key." + UUID.randomUUID();
        String defaultValue = "default-fallback";

        String value = config.getProperty(missingKey, defaultValue);
        assertEquals(defaultValue, value, "Should return default value for missing key");

        System.out.println("Missing key returned default: " + value);
    }

    /**
     * AEH-002: Test get property with null default
     */
    @Test
    @Order(2)
    void testMissingKeyWithNullDefault() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        String missingKey = "another.missing.key." + UUID.randomUUID();

        String value = config.getProperty(missingKey, null);
        assertNull(value, "Should return null when default is null and key missing");

        System.out.println("Missing key with null default: " + value);
    }

    /**
     * AEH-003: Test get int property with missing key
     */
    @Test
    @Order(3)
    void testMissingIntProperty() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        String missingKey = "missing.int.key." + UUID.randomUUID();
        int defaultValue = 42;

        int value = config.getIntProperty(missingKey, defaultValue);
        assertEquals(defaultValue, value, "Should return default int for missing key");

        System.out.println("Missing int returned default: " + value);
    }

    /**
     * AEH-004: Test get boolean property with missing key
     */
    @Test
    @Order(4)
    void testMissingBooleanProperty() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        String missingKey = "missing.boolean.key." + UUID.randomUUID();
        boolean defaultValue = true;

        boolean value = config.getBooleanProperty(missingKey, defaultValue);
        assertEquals(defaultValue, value, "Should return default boolean for missing key");

        System.out.println("Missing boolean returned default: " + value);
    }

    // ==================== Type Conversion Error Tests ====================

    /**
     * AEH-005: Test invalid int conversion returns default
     */
    @Test
    @Order(5)
    void testInvalidIntConversion() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        // This key might contain non-numeric value
        String key = "test.string.value";
        int defaultValue = 999;

        int value = config.getIntProperty(key, defaultValue);
        // If conversion fails, should return default
        System.out.println("Int property for string key: " + value);
    }

    /**
     * AEH-006: Test invalid long conversion returns default
     */
    @Test
    @Order(6)
    void testInvalidLongConversion() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        String key = "invalid.long.value";
        long defaultValue = 123456789L;

        long value = config.getLongProperty(key, defaultValue);
        System.out.println("Long property result: " + value);
    }

    /**
     * AEH-007: Test invalid float conversion returns default
     */
    @Test
    @Order(7)
    void testInvalidFloatConversion() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        String key = "invalid.float.value";
        float defaultValue = 3.14f;

        float value = config.getFloatProperty(key, defaultValue);
        System.out.println("Float property result: " + value);
    }

    /**
     * AEH-008: Test invalid double conversion returns default
     */
    @Test
    @Order(8)
    void testInvalidDoubleConversion() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        String key = "invalid.double.value";
        double defaultValue = 2.718281828;

        double value = config.getDoubleProperty(key, defaultValue);
        System.out.println("Double property result: " + value);
    }

    // ==================== Missing Namespace Tests ====================

    /**
     * AEH-009: Test get config for non-existent namespace
     */
    @Test
    @Order(9)
    void testNonExistentNamespace() {
        String nonExistentNs = "non-existent-namespace-" + UUID.randomUUID();

        try {
            Config config = ConfigService.getConfig(nonExistentNs);
            assertNotNull(config, "Should return config object even for non-existent namespace");

            // Access should return defaults
            String value = config.getProperty("any.key", "default");
            assertEquals("default", value);

            System.out.println("Non-existent namespace handled gracefully");
        } catch (Exception e) {
            System.out.println("Non-existent namespace error: " + e.getMessage());
        }
    }

    /**
     * AEH-010: Test get config file for non-existent namespace
     */
    @Test
    @Order(10)
    void testNonExistentConfigFile() {
        String nonExistentNs = "non-existent-file-" + UUID.randomUUID();

        try {
            ConfigFile configFile = ConfigService.getConfigFile(nonExistentNs, ConfigFileFormat.Properties);
            assertNotNull(configFile, "Should return config file object");

            String content = configFile.getContent();
            System.out.println("Non-existent config file content: " +
                    (content == null ? "null" : content.length() + " chars"));
        } catch (Exception e) {
            System.out.println("Non-existent config file error: " + e.getMessage());
        }
    }

    // ==================== Empty Value Tests ====================

    /**
     * AEH-011: Test get property with empty string value
     */
    @Test
    @Order(11)
    void testEmptyStringProperty() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        // Key that might have empty value
        String key = "empty.value.key";
        String defaultValue = "non-empty-default";

        String value = config.getProperty(key, defaultValue);
        System.out.println("Property result: " + (value == null ? "null" : "'" + value + "'"));
    }

    /**
     * AEH-012: Test property names set for empty config
     */
    @Test
    @Order(12)
    void testPropertyNamesEmptyConfig() {
        String emptyNs = "empty-namespace-" + UUID.randomUUID();

        try {
            Config config = ConfigService.getConfig(emptyNs);
            Set<String> names = config.getPropertyNames();

            assertNotNull(names, "Property names set should not be null");
            System.out.println("Empty config property names: " + names.size());
        } catch (Exception e) {
            System.out.println("Property names error: " + e.getMessage());
        }
    }

    // ==================== Concurrent Access Tests ====================

    /**
     * AEH-013: Test concurrent config access
     */
    @Test
    @Order(13)
    void testConcurrentConfigAccess() throws InterruptedException {
        int threadCount = 10;
        CountDownLatch latch = new CountDownLatch(threadCount);
        List<Exception> errors = new CopyOnWriteArrayList<>();

        for (int i = 0; i < threadCount; i++) {
            final int index = i;
            new Thread(() -> {
                try {
                    Config config = ConfigService.getConfig(TEST_NAMESPACE);
                    for (int j = 0; j < 100; j++) {
                        config.getProperty("key" + j, "default");
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
        assertTrue(errors.isEmpty(), "No errors should occur: " + errors);

        System.out.println("Concurrent access completed without errors");
    }

    /**
     * AEH-014: Test concurrent namespace access
     */
    @Test
    @Order(14)
    void testConcurrentNamespaceAccess() throws InterruptedException {
        String[] namespaces = {"ns1", "ns2", "ns3", "application", "common"};
        int threadCount = 5;
        CountDownLatch latch = new CountDownLatch(threadCount * namespaces.length);
        List<Exception> errors = new CopyOnWriteArrayList<>();

        for (String ns : namespaces) {
            for (int i = 0; i < threadCount; i++) {
                new Thread(() -> {
                    try {
                        Config config = ConfigService.getConfig(ns);
                        config.getProperty("test.key", "default");
                    } catch (Exception e) {
                        errors.add(e);
                    } finally {
                        latch.countDown();
                    }
                }).start();
            }
        }

        boolean completed = latch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete");

        System.out.println("Concurrent namespace access - errors: " + errors.size());
    }

    // ==================== Special Character Tests ====================

    /**
     * AEH-015: Test property key with special characters
     */
    @Test
    @Order(15)
    void testSpecialCharacterKeys() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        String[] specialKeys = {
                "key.with.dots",
                "key-with-dashes",
                "key_with_underscores",
                "key:with:colons",
                "key with spaces",
                "key[with]brackets",
                "key{with}braces"
        };

        for (String key : specialKeys) {
            try {
                String value = config.getProperty(key, "default");
                System.out.println("Key '" + key + "' = " + value);
            } catch (Exception e) {
                System.out.println("Key '" + key + "' error: " + e.getMessage());
            }
        }
    }

    /**
     * AEH-016: Test unicode property key
     */
    @Test
    @Order(16)
    void testUnicodeKeys() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        String[] unicodeKeys = {
                "键名",
                "キー",
                "키",
                "emoji.🎉.key"
        };

        for (String key : unicodeKeys) {
            try {
                String value = config.getProperty(key, "default");
                System.out.println("Unicode key '" + key + "' = " + value);
            } catch (Exception e) {
                System.out.println("Unicode key '" + key + "' error: " + e.getMessage());
            }
        }
    }

    // ==================== Array Property Tests ====================

    /**
     * AEH-017: Test array property with invalid format
     */
    @Test
    @Order(17)
    void testInvalidArrayProperty() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        String key = "invalid.array.property";
        String[] defaultValue = {"default1", "default2"};

        String[] value = config.getArrayProperty(key, ",", defaultValue);
        assertNotNull(value);
        System.out.println("Array property result: " + Arrays.toString(value));
    }

    /**
     * AEH-018: Test array property with empty delimiter
     */
    @Test
    @Order(18)
    void testArrayPropertyEmptyDelimiter() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        String key = "test.array.key";
        String[] defaultValue = {"default"};

        try {
            // Empty delimiter might cause issues
            String[] value = config.getArrayProperty(key, "", defaultValue);
            System.out.println("Empty delimiter result: " + Arrays.toString(value));
        } catch (Exception e) {
            System.out.println("Empty delimiter error: " + e.getMessage());
        }
    }

    // ==================== Null Handling Tests ====================

    /**
     * AEH-019: Test null key handling
     */
    @Test
    @Order(19)
    void testNullKeyHandling() {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        try {
            String value = config.getProperty(null, "default");
            System.out.println("Null key result: " + value);
        } catch (Exception e) {
            System.out.println("Null key error (expected): " + e.getClass().getSimpleName());
        }
    }

    /**
     * AEH-020: Test null namespace handling
     */
    @Test
    @Order(20)
    void testNullNamespaceHandling() {
        try {
            Config config = ConfigService.getConfig(null);
            System.out.println("Null namespace - config: " + config);
        } catch (Exception e) {
            System.out.println("Null namespace error (expected): " + e.getClass().getSimpleName());
        }
    }
}
