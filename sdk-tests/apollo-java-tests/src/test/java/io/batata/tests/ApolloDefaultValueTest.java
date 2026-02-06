package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigService;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Default Value Tests
 *
 * Tests for default value handling and configuration precedence:
 * - Default values for missing keys
 * - Type-safe default values
 * - Null handling
 * - Precedence rules
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloDefaultValueTest {

    @BeforeAll
    static void setupClass() {
        System.setProperty("app.id", System.getProperty("apollo.app.id", "test-app"));
        System.setProperty("apollo.meta", System.getProperty("apollo.meta", "http://127.0.0.1:8848"));
        System.setProperty("apollo.configService", System.getProperty("apollo.configService", "http://127.0.0.1:8848"));
        System.setProperty("env", System.getProperty("apollo.env", "DEV"));
        System.setProperty("apollo.cluster", System.getProperty("apollo.cluster", "default"));

        System.out.println("Apollo Default Value Test Setup");
    }

    // ==================== String Default Value Tests ====================

    /**
     * ADV-001: Test string default for missing key
     */
    @Test
    @Order(1)
    void testStringDefaultForMissingKey() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "definitely.missing.key." + UUID.randomUUID().toString();
        String defaultValue = "my-default-value";

        String result = config.getProperty(missingKey, defaultValue);

        assertEquals(defaultValue, result, "Should return default value for missing key");
        System.out.println("Missing key returned default: " + result);
    }

    /**
     * ADV-002: Test null default value
     */
    @Test
    @Order(2)
    void testNullDefaultValue() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "another.missing.key." + UUID.randomUUID().toString();

        String result = config.getProperty(missingKey, null);

        assertNull(result, "Should return null when default is null");
        System.out.println("Null default returned: " + result);
    }

    /**
     * ADV-003: Test empty string default
     */
    @Test
    @Order(3)
    void testEmptyStringDefault() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "empty.default.key." + UUID.randomUUID().toString();

        String result = config.getProperty(missingKey, "");

        assertEquals("", result, "Should return empty string default");
        System.out.println("Empty default length: " + result.length());
    }

    /**
     * ADV-004: Test whitespace default value
     */
    @Test
    @Order(4)
    void testWhitespaceDefaultValue() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "whitespace.key." + UUID.randomUUID().toString();
        String whitespaceDefault = "   ";

        String result = config.getProperty(missingKey, whitespaceDefault);

        assertEquals(whitespaceDefault, result, "Should preserve whitespace in default");
        System.out.println("Whitespace default length: " + result.length());
    }

    // ==================== Integer Default Value Tests ====================

    /**
     * ADV-005: Test integer default for missing key
     */
    @Test
    @Order(5)
    void testIntegerDefaultForMissingKey() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "missing.int.key." + UUID.randomUUID().toString();
        int defaultValue = 42;

        int result = config.getIntProperty(missingKey, defaultValue);

        assertEquals(defaultValue, result, "Should return integer default");
        System.out.println("Integer default: " + result);
    }

    /**
     * ADV-006: Test integer default with negative value
     */
    @Test
    @Order(6)
    void testIntegerDefaultNegative() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "negative.int.key." + UUID.randomUUID().toString();
        int defaultValue = -100;

        int result = config.getIntProperty(missingKey, defaultValue);

        assertEquals(defaultValue, result);
        System.out.println("Negative integer default: " + result);
    }

    /**
     * ADV-007: Test integer default with zero
     */
    @Test
    @Order(7)
    void testIntegerDefaultZero() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "zero.int.key." + UUID.randomUUID().toString();

        int result = config.getIntProperty(missingKey, 0);

        assertEquals(0, result);
        System.out.println("Zero integer default: " + result);
    }

    // ==================== Long Default Value Tests ====================

    /**
     * ADV-008: Test long default for missing key
     */
    @Test
    @Order(8)
    void testLongDefaultForMissingKey() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "missing.long.key." + UUID.randomUUID().toString();
        long defaultValue = 9999999999L;

        long result = config.getLongProperty(missingKey, defaultValue);

        assertEquals(defaultValue, result);
        System.out.println("Long default: " + result);
    }

    /**
     * ADV-009: Test long default max value
     */
    @Test
    @Order(9)
    void testLongDefaultMaxValue() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "max.long.key." + UUID.randomUUID().toString();

        long result = config.getLongProperty(missingKey, Long.MAX_VALUE);

        assertEquals(Long.MAX_VALUE, result);
        System.out.println("Max long default: " + result);
    }

    // ==================== Float/Double Default Value Tests ====================

    /**
     * ADV-010: Test float default for missing key
     */
    @Test
    @Order(10)
    void testFloatDefaultForMissingKey() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "missing.float.key." + UUID.randomUUID().toString();
        float defaultValue = 3.14f;

        float result = config.getFloatProperty(missingKey, defaultValue);

        assertEquals(defaultValue, result, 0.001f);
        System.out.println("Float default: " + result);
    }

    /**
     * ADV-011: Test double default for missing key
     */
    @Test
    @Order(11)
    void testDoubleDefaultForMissingKey() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "missing.double.key." + UUID.randomUUID().toString();
        double defaultValue = 2.718281828;

        double result = config.getDoubleProperty(missingKey, defaultValue);

        assertEquals(defaultValue, result, 0.0000001);
        System.out.println("Double default: " + result);
    }

    // ==================== Boolean Default Value Tests ====================

    /**
     * ADV-012: Test boolean default true
     */
    @Test
    @Order(12)
    void testBooleanDefaultTrue() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "missing.bool.true." + UUID.randomUUID().toString();

        boolean result = config.getBooleanProperty(missingKey, true);

        assertTrue(result);
        System.out.println("Boolean default true: " + result);
    }

    /**
     * ADV-013: Test boolean default false
     */
    @Test
    @Order(13)
    void testBooleanDefaultFalse() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "missing.bool.false." + UUID.randomUUID().toString();

        boolean result = config.getBooleanProperty(missingKey, false);

        assertFalse(result);
        System.out.println("Boolean default false: " + result);
    }

    // ==================== Array Default Value Tests ====================

    /**
     * ADV-014: Test array default for missing key
     */
    @Test
    @Order(14)
    void testArrayDefaultForMissingKey() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "missing.array.key." + UUID.randomUUID().toString();
        String[] defaultValue = {"default1", "default2", "default3"};

        String[] result = config.getArrayProperty(missingKey, ",", defaultValue);

        assertArrayEquals(defaultValue, result);
        System.out.println("Array default: " + Arrays.toString(result));
    }

    /**
     * ADV-015: Test empty array default
     */
    @Test
    @Order(15)
    void testEmptyArrayDefault() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "empty.array.key." + UUID.randomUUID().toString();
        String[] defaultValue = new String[0];

        String[] result = config.getArrayProperty(missingKey, ",", defaultValue);

        assertEquals(0, result.length);
        System.out.println("Empty array default length: " + result.length);
    }

    /**
     * ADV-016: Test null array default
     */
    @Test
    @Order(16)
    void testNullArrayDefault() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "null.array.key." + UUID.randomUUID().toString();

        String[] result = config.getArrayProperty(missingKey, ",", null);

        assertNull(result);
        System.out.println("Null array default: " + result);
    }

    // ==================== Special Character Default Tests ====================

    /**
     * ADV-017: Test default with special characters
     */
    @Test
    @Order(17)
    void testDefaultWithSpecialCharacters() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "special.chars.key." + UUID.randomUUID().toString();
        String defaultValue = "value with spaces & special chars: @#$%^&*()";

        String result = config.getProperty(missingKey, defaultValue);

        assertEquals(defaultValue, result);
        System.out.println("Special chars default: " + result);
    }

    /**
     * ADV-018: Test default with unicode
     */
    @Test
    @Order(18)
    void testDefaultWithUnicode() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "unicode.key." + UUID.randomUUID().toString();
        String defaultValue = "默认值 デフォルト 기본값";

        String result = config.getProperty(missingKey, defaultValue);

        assertEquals(defaultValue, result);
        System.out.println("Unicode default: " + result);
    }

    /**
     * ADV-019: Test default with newlines
     */
    @Test
    @Order(19)
    void testDefaultWithNewlines() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "newline.key." + UUID.randomUUID().toString();
        String defaultValue = "line1\nline2\nline3";

        String result = config.getProperty(missingKey, defaultValue);

        assertEquals(defaultValue, result);
        assertTrue(result.contains("\n"));
        System.out.println("Newline default contains newlines: " + result.contains("\n"));
    }

    // ==================== Performance Tests ====================

    /**
     * ADV-020: Test default value performance
     */
    @Test
    @Order(20)
    void testDefaultValuePerformance() {
        Config config = ConfigService.getAppConfig();

        int iterations = 10000;
        String missingKey = "perf.missing.key";
        String defaultValue = "default";

        long startTime = System.currentTimeMillis();

        for (int i = 0; i < iterations; i++) {
            config.getProperty(missingKey, defaultValue);
        }

        long duration = System.currentTimeMillis() - startTime;
        double avgMs = (double) duration / iterations;

        System.out.println(iterations + " default value lookups in " + duration + "ms");
        System.out.println("Average: " + String.format("%.4f", avgMs) + "ms per lookup");
    }

    /**
     * ADV-021: Test concurrent default value access
     */
    @Test
    @Order(21)
    void testConcurrentDefaultValueAccess() throws InterruptedException {
        Config config = ConfigService.getAppConfig();
        int threadCount = 20;
        CountDownLatch latch = new CountDownLatch(threadCount);
        List<String> results = new CopyOnWriteArrayList<>();

        String missingKey = "concurrent.default.key";
        String defaultValue = "concurrent-default";

        for (int i = 0; i < threadCount; i++) {
            new Thread(() -> {
                try {
                    String result = config.getProperty(missingKey, defaultValue);
                    results.add(result);
                } finally {
                    latch.countDown();
                }
            }).start();
        }

        boolean completed = latch.await(30, TimeUnit.SECONDS);
        assertTrue(completed);

        // All should get the same default
        for (String result : results) {
            assertEquals(defaultValue, result);
        }

        System.out.println("Concurrent access results: " + results.size());
    }

    // ==================== Edge Case Tests ====================

    /**
     * ADV-022: Test very long default value
     */
    @Test
    @Order(22)
    void testVeryLongDefaultValue() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "long.default.key." + UUID.randomUUID().toString();
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < 1000; i++) {
            sb.append("long-default-value-").append(i).append("-");
        }
        String longDefault = sb.toString();

        String result = config.getProperty(missingKey, longDefault);

        assertEquals(longDefault, result);
        System.out.println("Long default value length: " + result.length());
    }

    /**
     * ADV-023: Test default value with equals sign
     */
    @Test
    @Order(23)
    void testDefaultWithEqualsSign() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "equals.key." + UUID.randomUUID().toString();
        String defaultValue = "key=value=another";

        String result = config.getProperty(missingKey, defaultValue);

        assertEquals(defaultValue, result);
        System.out.println("Default with equals: " + result);
    }

    /**
     * ADV-024: Test same key different default values
     */
    @Test
    @Order(24)
    void testSameKeyDifferentDefaults() {
        Config config = ConfigService.getAppConfig();

        String missingKey = "same.key.different.defaults";

        String result1 = config.getProperty(missingKey, "default1");
        String result2 = config.getProperty(missingKey, "default2");

        // Both should return their respective defaults for missing key
        assertEquals("default1", result1);
        assertEquals("default2", result2);

        System.out.println("Result 1: " + result1 + ", Result 2: " + result2);
    }
}
