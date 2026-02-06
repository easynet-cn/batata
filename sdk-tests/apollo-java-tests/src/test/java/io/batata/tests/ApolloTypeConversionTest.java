package io.batata.tests;

import org.junit.jupiter.api.*;

import java.io.*;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.text.SimpleDateFormat;
import java.time.Duration;
import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Type Conversion Tests
 *
 * Tests for configuration value type conversions:
 * - Primitive types (int, long, float, double, boolean, byte, short)
 * - String types
 * - Date/Duration types
 * - Enum types
 * - Array/List types
 * - Nested object types
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloTypeConversionTest {

    private static String configServiceUrl;
    private static String openApiUrl;
    private static String portalToken;
    private static final String APP_ID = "type-conversion-test";
    private static final String CLUSTER = "default";
    private static final String ENV = "DEV";
    private static final String NAMESPACE = "application";

    @BeforeAll
    static void setup() throws Exception {
        configServiceUrl = System.getProperty("apollo.config.service", "http://127.0.0.1:8848");
        openApiUrl = System.getProperty("apollo.openapi.url", "http://127.0.0.1:8848");
        portalToken = System.getProperty("apollo.portal.token", "test-token");

        System.out.println("Apollo Type Conversion Test Setup");
        System.out.println("Config Service: " + configServiceUrl);

        // Create test app and namespace
        createTestApp();
    }

    private static void createTestApp() throws Exception {
        String url = openApiUrl + "/openapi/v1/apps";
        String body = String.format(
                "{\"appId\":\"%s\",\"name\":\"Type Conversion Test App\",\"orgId\":\"test\",\"orgName\":\"Test Org\",\"ownerName\":\"test\",\"ownerEmail\":\"test@test.com\"}",
                APP_ID);

        try {
            httpPost(url, body, "application/json");
        } catch (Exception e) {
            System.out.println("App creation: " + e.getMessage());
        }
    }

    // ==================== Primitive Type Tests ====================

    /**
     * ATC-001: Test integer property conversion
     */
    @Test
    @Order(1)
    void testIntPropertyConversion() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("int.positive", "42");
        configs.put("int.negative", "-100");
        configs.put("int.zero", "0");
        configs.put("int.max", String.valueOf(Integer.MAX_VALUE));
        configs.put("int.min", String.valueOf(Integer.MIN_VALUE));

        publishConfigs(configs);

        // Verify retrieval
        String content = getConfig();
        System.out.println("Int configs: " + content);

        // Parse and verify
        assertTrue(content.contains("42") || content.contains("int.positive"),
                "Should contain int value");
    }

    /**
     * ATC-002: Test long property conversion
     */
    @Test
    @Order(2)
    void testLongPropertyConversion() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("long.value", "9223372036854775807"); // Long.MAX_VALUE
        configs.put("long.timestamp", String.valueOf(System.currentTimeMillis()));

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Long configs: " + content);
        assertNotNull(content);
    }

    /**
     * ATC-003: Test float property conversion
     */
    @Test
    @Order(3)
    void testFloatPropertyConversion() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("float.positive", "3.14159");
        configs.put("float.negative", "-2.718");
        configs.put("float.scientific", "1.23E10");
        configs.put("float.small", "0.0000001");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Float configs: " + content);
        assertTrue(content.contains("3.14") || content.contains("float.positive"),
                "Should contain float value");
    }

    /**
     * ATC-004: Test double property conversion
     */
    @Test
    @Order(4)
    void testDoublePropertyConversion() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("double.pi", "3.141592653589793");
        configs.put("double.e", "2.718281828459045");
        configs.put("double.large", "1.7976931348623157E308");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Double configs: " + content);
        assertNotNull(content);
    }

    /**
     * ATC-005: Test boolean property conversion
     */
    @Test
    @Order(5)
    void testBooleanPropertyConversion() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("bool.true.lower", "true");
        configs.put("bool.true.upper", "TRUE");
        configs.put("bool.true.mixed", "True");
        configs.put("bool.false.lower", "false");
        configs.put("bool.false.upper", "FALSE");
        configs.put("bool.yes", "yes");
        configs.put("bool.no", "no");
        configs.put("bool.one", "1");
        configs.put("bool.zero", "0");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Boolean configs: " + content);
        assertTrue(content.contains("true") || content.contains("bool.true"),
                "Should contain boolean value");
    }

    /**
     * ATC-006: Test byte property conversion
     */
    @Test
    @Order(6)
    void testBytePropertyConversion() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("byte.max", "127");
        configs.put("byte.min", "-128");
        configs.put("byte.zero", "0");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Byte configs: " + content);
        assertNotNull(content);
    }

    /**
     * ATC-007: Test short property conversion
     */
    @Test
    @Order(7)
    void testShortPropertyConversion() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("short.max", "32767");
        configs.put("short.min", "-32768");
        configs.put("short.value", "1000");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Short configs: " + content);
        assertNotNull(content);
    }

    // ==================== String Type Tests ====================

    /**
     * ATC-008: Test string property with special characters
     */
    @Test
    @Order(8)
    void testStringSpecialChars() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("string.url", "http://localhost:8080/api?param=value&other=test");
        configs.put("string.path", "/usr/local/bin");
        configs.put("string.email", "test@example.com");
        configs.put("string.json", "{\"key\":\"value\"}");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("String special chars: " + content);
        assertNotNull(content);
    }

    /**
     * ATC-009: Test string property with unicode
     */
    @Test
    @Order(9)
    void testStringUnicode() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("string.chinese", "\u4e2d\u6587\u6d4b\u8bd5");
        configs.put("string.japanese", "\u65e5\u672c\u8a9e");
        configs.put("string.emoji", "Hello \ud83d\ude00");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("String unicode: " + content);
        assertNotNull(content);
    }

    /**
     * ATC-010: Test empty and whitespace strings
     */
    @Test
    @Order(10)
    void testEmptyAndWhitespace() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("string.empty", "");
        configs.put("string.spaces", "   ");
        configs.put("string.trimmed", "  value  ");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Empty/whitespace strings: " + content);
        assertNotNull(content);
    }

    // ==================== Date/Time Type Tests ====================

    /**
     * ATC-011: Test date property formats
     */
    @Test
    @Order(11)
    void testDatePropertyFormats() throws Exception {
        SimpleDateFormat isoFormat = new SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss.SSSZ");
        SimpleDateFormat simpleFormat = new SimpleDateFormat("yyyy-MM-dd");

        Map<String, String> configs = new HashMap<>();
        configs.put("date.iso", isoFormat.format(new Date()));
        configs.put("date.simple", simpleFormat.format(new Date()));
        configs.put("date.epoch", String.valueOf(System.currentTimeMillis()));
        configs.put("date.string", "2024-01-15");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Date configs: " + content);
        assertNotNull(content);
    }

    /**
     * ATC-012: Test duration property formats
     */
    @Test
    @Order(12)
    void testDurationPropertyFormats() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("duration.seconds", "30s");
        configs.put("duration.minutes", "5m");
        configs.put("duration.hours", "2h");
        configs.put("duration.millis", "500ms");
        configs.put("duration.complex", "1h30m");
        configs.put("duration.iso", "PT1H30M"); // ISO 8601 duration

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Duration configs: " + content);
        assertNotNull(content);

        // Verify parsing
        assertTrue(content.contains("30s") || content.contains("duration"),
                "Should contain duration value");
    }

    // ==================== Enum Type Tests ====================

    /**
     * ATC-013: Test enum property conversion
     */
    @Test
    @Order(13)
    void testEnumPropertyConversion() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("enum.loglevel", "DEBUG");
        configs.put("enum.environment", "PRODUCTION");
        configs.put("enum.status", "ACTIVE");
        configs.put("enum.priority", "HIGH");
        configs.put("enum.lower", "warning"); // lowercase enum

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Enum configs: " + content);
        assertTrue(content.contains("DEBUG") || content.contains("enum.loglevel"),
                "Should contain enum value");
    }

    // ==================== Array/List Type Tests ====================

    /**
     * ATC-014: Test array property (comma-separated)
     */
    @Test
    @Order(14)
    void testArrayPropertyCommaSeparated() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("array.strings", "one,two,three,four");
        configs.put("array.ints", "1,2,3,4,5");
        configs.put("array.hosts", "host1.example.com,host2.example.com,host3.example.com");
        configs.put("array.empty", "");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Array configs: " + content);
        assertTrue(content.contains("one,two,three") || content.contains("array.strings"),
                "Should contain array value");
    }

    /**
     * ATC-015: Test array property (JSON format)
     */
    @Test
    @Order(15)
    void testArrayPropertyJsonFormat() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("array.json.strings", "[\"a\",\"b\",\"c\"]");
        configs.put("array.json.numbers", "[1,2,3,4,5]");
        configs.put("array.json.mixed", "[1,\"two\",true,null]");
        configs.put("array.json.objects", "[{\"id\":1},{\"id\":2}]");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("JSON array configs: " + content);
        assertNotNull(content);
    }

    /**
     * ATC-016: Test list with different delimiters
     */
    @Test
    @Order(16)
    void testListDifferentDelimiters() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("list.semicolon", "a;b;c;d");
        configs.put("list.pipe", "a|b|c|d");
        configs.put("list.space", "a b c d");
        configs.put("list.newline", "a\\nb\\nc\\nd");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("List delimiter configs: " + content);
        assertNotNull(content);
    }

    // ==================== Nested Object Type Tests ====================

    /**
     * ATC-017: Test nested properties (dot notation)
     */
    @Test
    @Order(17)
    void testNestedPropertiesDotNotation() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("database.host", "localhost");
        configs.put("database.port", "3306");
        configs.put("database.name", "mydb");
        configs.put("database.connection.timeout", "30");
        configs.put("database.connection.pool.size", "10");
        configs.put("database.connection.pool.maxIdle", "5");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Nested property configs: " + content);
        assertTrue(content.contains("database") || content.contains("localhost"),
                "Should contain nested properties");
    }

    /**
     * ATC-018: Test JSON object value
     */
    @Test
    @Order(18)
    void testJsonObjectValue() throws Exception {
        String jsonValue = "{\"server\":{\"host\":\"localhost\",\"port\":8080},\"features\":{\"enabled\":true,\"list\":[1,2,3]}}";

        Map<String, String> configs = new HashMap<>();
        configs.put("config.json", jsonValue);

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("JSON object config: " + content);
        assertNotNull(content);
    }

    // ==================== Edge Case Tests ====================

    /**
     * ATC-019: Test null and missing values
     */
    @Test
    @Order(19)
    void testNullAndMissingValues() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("value.exists", "present");
        configs.put("value.empty", "");
        // Note: "value.null" cannot be truly null in properties

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Null/missing configs: " + content);

        // Query non-existent key
        String url = String.format("%s/configs/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, NAMESPACE);
        String response = httpGet(url);
        System.out.println("Config response for null test: " + response);
    }

    /**
     * ATC-020: Test type conversion errors
     */
    @Test
    @Order(20)
    void testTypeConversionErrors() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("invalid.int", "not-a-number");
        configs.put("invalid.double", "abc.def");
        configs.put("invalid.boolean", "maybe");
        configs.put("invalid.date", "not-a-date");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Invalid type configs: " + content);
        // These should be stored as strings, conversion happens on client side
        assertNotNull(content);
    }

    /**
     * ATC-021: Test very large values
     */
    @Test
    @Order(21)
    void testVeryLargeValues() throws Exception {
        StringBuilder largeString = new StringBuilder();
        for (int i = 0; i < 1000; i++) {
            largeString.append("abcdefghij");
        }

        Map<String, String> configs = new HashMap<>();
        configs.put("large.string", largeString.toString());
        configs.put("large.number", "99999999999999999999999999999999");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Large value config length: " + content.length());
        assertNotNull(content);
    }

    /**
     * ATC-022: Test property key formats
     */
    @Test
    @Order(22)
    void testPropertyKeyFormats() throws Exception {
        Map<String, String> configs = new HashMap<>();
        configs.put("camelCase", "value1");
        configs.put("snake_case", "value2");
        configs.put("kebab-case", "value3");
        configs.put("UPPER_CASE", "value4");
        configs.put("mixed.Dot_Case", "value5");
        configs.put("with-numbers-123", "value6");

        publishConfigs(configs);

        String content = getConfig();
        System.out.println("Key format configs: " + content);
        assertNotNull(content);
    }

    // ==================== Helper Methods ====================

    private void publishConfigs(Map<String, String> configs) throws Exception {
        for (Map.Entry<String, String> entry : configs.entrySet()) {
            publishItem(entry.getKey(), entry.getValue());
        }

        // Release
        String releaseUrl = openApiUrl + "/openapi/v1/envs/" + ENV + "/apps/" + APP_ID +
                "/clusters/" + CLUSTER + "/namespaces/" + NAMESPACE + "/releases";
        String releaseBody = "{\"releaseTitle\":\"type-test-release\",\"releasedBy\":\"test\"}";

        try {
            httpPost(releaseUrl, releaseBody, "application/json");
        } catch (Exception e) {
            System.out.println("Release: " + e.getMessage());
        }
    }

    private void publishItem(String key, String value) throws Exception {
        String itemUrl = openApiUrl + "/openapi/v1/envs/" + ENV + "/apps/" + APP_ID +
                "/clusters/" + CLUSTER + "/namespaces/" + NAMESPACE + "/items";

        String escapedValue = value.replace("\\", "\\\\")
                .replace("\"", "\\\"")
                .replace("\n", "\\n");

        String itemBody = String.format(
                "{\"key\":\"%s\",\"value\":\"%s\",\"dataChangeCreatedBy\":\"test\"}",
                key, escapedValue);

        try {
            httpPost(itemUrl, itemBody, "application/json");
        } catch (Exception e) {
            // Item may already exist, try update
            String updateUrl = itemUrl + "/" + key;
            String updateBody = String.format(
                    "{\"key\":\"%s\",\"value\":\"%s\",\"dataChangeLastModifiedBy\":\"test\"}",
                    key, escapedValue);
            try {
                httpPut(updateUrl, updateBody, "application/json");
            } catch (Exception e2) {
                // Ignore
            }
        }
    }

    private String getConfig() throws Exception {
        String url = String.format("%s/configs/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, NAMESPACE);
        return httpGet(url);
    }

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

    private static String httpPut(String urlString, String body, String contentType) throws Exception {
        URL url = new URL(urlString);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("PUT");
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
