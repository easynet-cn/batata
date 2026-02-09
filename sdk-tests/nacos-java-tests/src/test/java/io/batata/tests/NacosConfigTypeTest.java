package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.config.ConfigType;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Config Type Tests
 *
 * Tests for different configuration types:
 * - Properties format
 * - YAML format
 * - JSON format
 * - XML format
 * - Text format
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigTypeTest {

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
        System.out.println("Nacos Config Type Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (configService != null) {
            configService.shutDown();
        }
    }

    // ==================== Properties Format Tests ====================

    /**
     * NCT-001: Test publish properties format config
     */
    @Test
    @Order(1)
    void testPublishPropertiesConfig() throws NacosException, InterruptedException {
        String dataId = "config-props-" + UUID.randomUUID().toString().substring(0, 8) + ".properties";

        String content = "database.url=jdbc:mysql://localhost:3306/db\n" +
                        "database.username=root\n" +
                        "database.password=secret\n" +
                        "database.pool.size=10";

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content, ConfigType.PROPERTIES.getType());
        assertTrue(success, "Should publish properties config");

        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(retrieved);
        assertTrue(retrieved.contains("database.url="));
        assertTrue(retrieved.contains("database.pool.size=10"));

        System.out.println("Properties config published successfully");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCT-002: Test properties with special values
     */
    @Test
    @Order(2)
    void testPropertiesSpecialValues() throws NacosException, InterruptedException {
        String dataId = "config-props-special-" + UUID.randomUUID().toString().substring(0, 8) + ".properties";

        String content = "path=/usr/local/bin\n" +
                        "url=http://example.com?param=value&other=123\n" +
                        "message=Hello World with spaces\n" +
                        "unicode=中文内容";

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content, ConfigType.PROPERTIES.getType());
        assertTrue(success);

        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertTrue(retrieved.contains("path=/usr/local/bin"));
        assertTrue(retrieved.contains("unicode="));

        System.out.println("Properties with special values verified");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCT-003: Test properties with multiline values
     */
    @Test
    @Order(3)
    void testPropertiesMultilineValues() throws NacosException, InterruptedException {
        String dataId = "config-props-multiline-" + UUID.randomUUID().toString().substring(0, 8) + ".properties";

        String content = "single.line=simple value\n" +
                        "multi.line=line1\\\n" +
                        "    line2\\\n" +
                        "    line3";

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content, ConfigType.PROPERTIES.getType());
        assertTrue(success);

        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(retrieved);

        System.out.println("Multiline properties config retrieved");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== YAML Format Tests ====================

    /**
     * NCT-004: Test publish YAML format config
     */
    @Test
    @Order(4)
    void testPublishYamlConfig() throws NacosException, InterruptedException {
        String dataId = "config-yaml-" + UUID.randomUUID().toString().substring(0, 8) + ".yaml";

        String content = "server:\n" +
                        "  port: 8080\n" +
                        "  host: localhost\n" +
                        "database:\n" +
                        "  url: jdbc:mysql://localhost:3306/db\n" +
                        "  pool:\n" +
                        "    min: 5\n" +
                        "    max: 20";

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content, ConfigType.YAML.getType());
        assertTrue(success, "Should publish YAML config");

        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(retrieved);
        assertTrue(retrieved.contains("server:"));
        assertTrue(retrieved.contains("port: 8080"));

        System.out.println("YAML config published successfully");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCT-005: Test YAML with arrays
     */
    @Test
    @Order(5)
    void testYamlWithArrays() throws NacosException, InterruptedException {
        String dataId = "config-yaml-arrays-" + UUID.randomUUID().toString().substring(0, 8) + ".yaml";

        String content = "servers:\n" +
                        "  - host: server1.example.com\n" +
                        "    port: 8080\n" +
                        "  - host: server2.example.com\n" +
                        "    port: 8081\n" +
                        "  - host: server3.example.com\n" +
                        "    port: 8082\n" +
                        "tags:\n" +
                        "  - primary\n" +
                        "  - production\n" +
                        "  - v2";

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content, ConfigType.YAML.getType());
        assertTrue(success);

        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertTrue(retrieved.contains("servers:"));
        assertTrue(retrieved.contains("- host: server1"));

        System.out.println("YAML with arrays verified");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCT-006: Test YAML with nested objects
     */
    @Test
    @Order(6)
    void testYamlNestedObjects() throws NacosException, InterruptedException {
        String dataId = "config-yaml-nested-" + UUID.randomUUID().toString().substring(0, 8) + ".yaml";

        String content = "application:\n" +
                        "  name: my-service\n" +
                        "  config:\n" +
                        "    cache:\n" +
                        "      enabled: true\n" +
                        "      ttl: 3600\n" +
                        "      provider:\n" +
                        "        type: redis\n" +
                        "        host: localhost\n" +
                        "        port: 6379";

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content, ConfigType.YAML.getType());
        assertTrue(success);

        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertTrue(retrieved.contains("application:"));
        assertTrue(retrieved.contains("provider:"));

        System.out.println("YAML nested objects verified");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== JSON Format Tests ====================

    /**
     * NCT-007: Test publish JSON format config
     */
    @Test
    @Order(7)
    void testPublishJsonConfig() throws NacosException, InterruptedException {
        String dataId = "config-json-" + UUID.randomUUID().toString().substring(0, 8) + ".json";

        String content = "{\n" +
                        "  \"server\": {\n" +
                        "    \"port\": 8080,\n" +
                        "    \"host\": \"localhost\"\n" +
                        "  },\n" +
                        "  \"database\": {\n" +
                        "    \"url\": \"jdbc:mysql://localhost:3306/db\",\n" +
                        "    \"username\": \"root\"\n" +
                        "  }\n" +
                        "}";

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content, ConfigType.JSON.getType());
        assertTrue(success, "Should publish JSON config");

        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(retrieved);
        assertTrue(retrieved.contains("\"server\""));
        assertTrue(retrieved.contains("\"port\": 8080") || retrieved.contains("\"port\":8080"));

        System.out.println("JSON config published successfully");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCT-008: Test JSON with arrays
     */
    @Test
    @Order(8)
    void testJsonWithArrays() throws NacosException, InterruptedException {
        String dataId = "config-json-arrays-" + UUID.randomUUID().toString().substring(0, 8) + ".json";

        String content = "{\n" +
                        "  \"servers\": [\n" +
                        "    {\"host\": \"server1.com\", \"port\": 8080},\n" +
                        "    {\"host\": \"server2.com\", \"port\": 8081}\n" +
                        "  ],\n" +
                        "  \"tags\": [\"primary\", \"production\"]\n" +
                        "}";

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content, ConfigType.JSON.getType());
        assertTrue(success);

        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertTrue(retrieved.contains("\"servers\""));
        assertTrue(retrieved.contains("\"tags\""));

        System.out.println("JSON with arrays verified");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCT-009: Test JSON with special characters
     */
    @Test
    @Order(9)
    void testJsonSpecialCharacters() throws NacosException, InterruptedException {
        String dataId = "config-json-special-" + UUID.randomUUID().toString().substring(0, 8) + ".json";

        String content = "{\n" +
                        "  \"message\": \"Hello \\\"World\\\"\",\n" +
                        "  \"path\": \"/usr/local/bin\",\n" +
                        "  \"unicode\": \"中文内容\",\n" +
                        "  \"escaped\": \"line1\\nline2\"\n" +
                        "}";

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content, ConfigType.JSON.getType());
        assertTrue(success);

        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertTrue(retrieved.contains("\"message\""));

        System.out.println("JSON with special characters verified");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== XML Format Tests ====================

    /**
     * NCT-010: Test publish XML format config
     */
    @Test
    @Order(10)
    void testPublishXmlConfig() throws NacosException, InterruptedException {
        String dataId = "config-xml-" + UUID.randomUUID().toString().substring(0, 8) + ".xml";

        String content = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n" +
                        "<configuration>\n" +
                        "  <server>\n" +
                        "    <port>8080</port>\n" +
                        "    <host>localhost</host>\n" +
                        "  </server>\n" +
                        "  <database>\n" +
                        "    <url>jdbc:mysql://localhost:3306/db</url>\n" +
                        "  </database>\n" +
                        "</configuration>";

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content, ConfigType.XML.getType());
        assertTrue(success, "Should publish XML config");

        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(retrieved);
        assertTrue(retrieved.contains("<configuration>"));
        assertTrue(retrieved.contains("<port>8080</port>"));

        System.out.println("XML config published successfully");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCT-011: Test XML with attributes
     */
    @Test
    @Order(11)
    void testXmlWithAttributes() throws NacosException, InterruptedException {
        String dataId = "config-xml-attrs-" + UUID.randomUUID().toString().substring(0, 8) + ".xml";

        String content = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n" +
                        "<servers>\n" +
                        "  <server id=\"1\" enabled=\"true\">\n" +
                        "    <host>server1.com</host>\n" +
                        "    <port>8080</port>\n" +
                        "  </server>\n" +
                        "  <server id=\"2\" enabled=\"false\">\n" +
                        "    <host>server2.com</host>\n" +
                        "    <port>8081</port>\n" +
                        "  </server>\n" +
                        "</servers>";

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content, ConfigType.XML.getType());
        assertTrue(success);

        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertTrue(retrieved.contains("id=\"1\""));
        assertTrue(retrieved.contains("enabled=\"true\""));

        System.out.println("XML with attributes verified");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== Text Format Tests ====================

    /**
     * NCT-012: Test publish text format config
     */
    @Test
    @Order(12)
    void testPublishTextConfig() throws NacosException, InterruptedException {
        String dataId = "config-text-" + UUID.randomUUID().toString().substring(0, 8) + ".txt";

        String content = "This is a plain text configuration.\n" +
                        "It can contain any content.\n" +
                        "Line 3 of the configuration.\n" +
                        "Special chars: !@#$%^&*()";

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content, ConfigType.TEXT.getType());
        assertTrue(success, "Should publish text config");

        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(retrieved);
        assertTrue(retrieved.contains("plain text configuration"));

        System.out.println("Text config published successfully");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCT-013: Test text with unicode content (CJK characters)
     */
    @Test
    @Order(13)
    void testTextUnicodeContent() throws NacosException, InterruptedException {
        String dataId = "config-text-unicode-" + UUID.randomUUID().toString().substring(0, 8) + ".txt";

        // CJK characters work with utf8 (3-byte max)
        String content = "English content\n" +
                        "中文内容\n" +
                        "日本語コンテンツ\n" +
                        "한국어 콘텐츠";

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content, ConfigType.TEXT.getType());
        assertTrue(success);

        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertTrue(retrieved.contains("中文内容"));
        assertTrue(retrieved.contains("日本語コンテンツ"));
        assertTrue(retrieved.contains("한국어 콘텐츠"));

        System.out.println("Text with unicode (CJK) verified");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== Format Detection Tests ====================

    /**
     * NCT-014: Test config without explicit type
     */
    @Test
    @Order(14)
    void testConfigWithoutExplicitType() throws NacosException, InterruptedException {
        String dataId = "config-no-type-" + UUID.randomUUID().toString().substring(0, 8);

        String content = "key1=value1\nkey2=value2";

        // Publish without type
        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content);
        assertTrue(success);

        Thread.sleep(500);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(content, retrieved);

        System.out.println("Config without explicit type verified");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCT-015: Test large config content
     */
    @Test
    @Order(15)
    void testLargeConfigContent() throws NacosException, InterruptedException {
        String dataId = "config-large-" + UUID.randomUUID().toString().substring(0, 8) + ".properties";

        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < 1000; i++) {
            sb.append("property.").append(i).append("=value").append(i).append("\n");
        }
        String content = sb.toString();

        boolean success = configService.publishConfig(dataId, DEFAULT_GROUP, content, ConfigType.PROPERTIES.getType());
        assertTrue(success);

        Thread.sleep(1000);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 10000);
        assertNotNull(retrieved);
        assertTrue(retrieved.length() > 10000);

        System.out.println("Large config content size: " + retrieved.length() + " chars");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    // ==================== Mixed Format Tests ====================

    /**
     * NCT-016: Test multiple formats in same group
     */
    @Test
    @Order(16)
    void testMultipleFormatsInGroup() throws NacosException, InterruptedException {
        String prefix = "multi-format-" + UUID.randomUUID().toString().substring(0, 8);

        // Properties
        configService.publishConfig(prefix + ".properties", DEFAULT_GROUP,
                "key=value", ConfigType.PROPERTIES.getType());

        // YAML
        configService.publishConfig(prefix + ".yaml", DEFAULT_GROUP,
                "key: value", ConfigType.YAML.getType());

        // JSON
        configService.publishConfig(prefix + ".json", DEFAULT_GROUP,
                "{\"key\": \"value\"}", ConfigType.JSON.getType());

        Thread.sleep(500);

        // Verify all formats
        String props = configService.getConfig(prefix + ".properties", DEFAULT_GROUP, 5000);
        String yaml = configService.getConfig(prefix + ".yaml", DEFAULT_GROUP, 5000);
        String json = configService.getConfig(prefix + ".json", DEFAULT_GROUP, 5000);

        assertTrue(props.contains("key=value"));
        assertTrue(yaml.contains("key: value"));
        assertTrue(json.contains("\"key\""));

        System.out.println("Multiple formats in same group verified");

        // Cleanup
        configService.removeConfig(prefix + ".properties", DEFAULT_GROUP);
        configService.removeConfig(prefix + ".yaml", DEFAULT_GROUP);
        configService.removeConfig(prefix + ".json", DEFAULT_GROUP);
    }

    /**
     * NCT-017: Test format change on update
     */
    @Test
    @Order(17)
    void testFormatChangeOnUpdate() throws NacosException, InterruptedException {
        String dataId = "format-change-" + UUID.randomUUID().toString().substring(0, 8);

        // Initial: Properties
        configService.publishConfig(dataId, DEFAULT_GROUP, "key=value", ConfigType.PROPERTIES.getType());
        Thread.sleep(500);

        String initial = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertTrue(initial.contains("key=value"));

        // Update: JSON format
        configService.publishConfig(dataId, DEFAULT_GROUP, "{\"key\": \"value\"}", ConfigType.JSON.getType());
        Thread.sleep(500);

        String updated = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertTrue(updated.contains("\"key\""));

        System.out.println("Format change on update verified");

        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * NCT-018: Test concurrent format operations
     */
    @Test
    @Order(18)
    void testConcurrentFormatOperations() throws InterruptedException {
        String prefix = "concurrent-format-" + UUID.randomUUID().toString().substring(0, 8);
        int threadCount = 10;
        CountDownLatch latch = new CountDownLatch(threadCount);
        List<String> dataIds = new CopyOnWriteArrayList<>();

        String[] types = {ConfigType.PROPERTIES.getType(), ConfigType.YAML.getType(),
                         ConfigType.JSON.getType(), ConfigType.TEXT.getType()};

        for (int i = 0; i < threadCount; i++) {
            final int idx = i;
            new Thread(() -> {
                try {
                    String dataId = prefix + "-" + idx;
                    String type = types[idx % types.length];
                    String content = "key" + idx + "=value" + idx;

                    configService.publishConfig(dataId, DEFAULT_GROUP, content, type);
                    dataIds.add(dataId);
                } catch (Exception e) {
                    System.out.println("Concurrent format error: " + e.getMessage());
                } finally {
                    latch.countDown();
                }
            }).start();
        }

        boolean completed = latch.await(30, TimeUnit.SECONDS);
        assertTrue(completed);

        System.out.println("Concurrent format operations: " + dataIds.size() + " configs created");

        // Cleanup
        for (String dataId : dataIds) {
            try {
                configService.removeConfig(dataId, DEFAULT_GROUP);
            } catch (Exception e) {
                // Ignore
            }
        }
    }
}
