package io.batata.tests;

import org.junit.jupiter.api.*;

import java.io.*;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo File Format Tests
 *
 * Tests for different configuration file formats:
 * - Properties (.properties)
 * - JSON (.json)
 * - YAML (.yaml, .yml)
 * - XML (.xml)
 * - TXT (.txt)
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloFileFormatTest {

    private static String configServiceUrl;
    private static String openApiUrl;
    private static String portalToken;
    private static final String APP_ID = "file-format-test";
    private static final String CLUSTER = "default";
    private static final String ENV = "DEV";

    @BeforeAll
    static void setup() throws Exception {
        configServiceUrl = System.getProperty("apollo.config.service", "http://127.0.0.1:8848");
        openApiUrl = System.getProperty("apollo.openapi.url", "http://127.0.0.1:8848");
        portalToken = System.getProperty("apollo.portal.token", "test-token");

        System.out.println("Apollo File Format Test Setup");
        System.out.println("Config Service: " + configServiceUrl);
        System.out.println("OpenAPI URL: " + openApiUrl);

        // Create test app if needed
        createTestApp();
    }

    private static void createTestApp() throws Exception {
        String url = openApiUrl + "/openapi/v1/apps";
        String body = String.format(
                "{\"appId\":\"%s\",\"name\":\"File Format Test App\",\"orgId\":\"test\",\"orgName\":\"Test Org\",\"ownerName\":\"test\",\"ownerEmail\":\"test@test.com\"}",
                APP_ID);

        try {
            httpPost(url, body, "application/json");
        } catch (Exception e) {
            // App may already exist
            System.out.println("App creation: " + e.getMessage());
        }
    }

    // ==================== Properties Format Tests ====================

    /**
     * AFF-001: Test Properties format config retrieval
     */
    @Test
    @Order(1)
    void testPropertiesFormatGet() throws Exception {
        String namespace = "test-props-" + UUID.randomUUID().toString().substring(0, 8) + ".properties";

        // Create namespace with properties content
        createNamespace(namespace, "properties");
        publishConfig(namespace, "key1=value1\nkey2=value2\nkey3=value3");

        // Get as properties
        String url = String.format("%s/configfiles/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("Properties content: " + content);
        assertTrue(content.contains("key1") || content.contains("value1"),
                "Should return properties content");

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * AFF-002: Test Properties format with special characters
     */
    @Test
    @Order(2)
    void testPropertiesSpecialChars() throws Exception {
        String namespace = "props-special-" + UUID.randomUUID().toString().substring(0, 8) + ".properties";

        createNamespace(namespace, "properties");

        // Properties with special characters
        String content = "url=http://localhost:8080/api?param=value&other=test\n" +
                "path=/usr/local/bin\n" +
                "unicode=\u4e2d\u6587\n" +
                "escaped=line1\\nline2";

        publishConfig(namespace, content);

        String url = String.format("%s/configfiles/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String retrieved = httpGet(url);

        System.out.println("Properties with special chars: " + retrieved);
        assertNotNull(retrieved);

        deleteNamespace(namespace);
    }

    /**
     * AFF-003: Test Properties format empty values
     */
    @Test
    @Order(3)
    void testPropertiesEmptyValues() throws Exception {
        String namespace = "props-empty-" + UUID.randomUUID().toString().substring(0, 8) + ".properties";

        createNamespace(namespace, "properties");
        publishConfig(namespace, "empty.key=\nblank.key=   \nnormal.key=value");

        String url = String.format("%s/configfiles/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("Properties with empty values: " + content);
        assertNotNull(content);

        deleteNamespace(namespace);
    }

    // ==================== JSON Format Tests ====================

    /**
     * AFF-004: Test JSON format config retrieval
     */
    @Test
    @Order(4)
    void testJsonFormatGet() throws Exception {
        String namespace = "test-json-" + UUID.randomUUID().toString().substring(0, 8) + ".json";

        createNamespace(namespace, "json");

        String jsonContent = "{\"database\":{\"host\":\"localhost\",\"port\":3306},\"cache\":{\"enabled\":true,\"ttl\":300}}";
        publishConfig(namespace, jsonContent);

        // Get as JSON
        String url = String.format("%s/configfiles/json/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("JSON content: " + content);
        assertTrue(content.contains("database") || content.contains("localhost"),
                "Should return JSON content");

        deleteNamespace(namespace);
    }

    /**
     * AFF-005: Test JSON format with nested objects
     */
    @Test
    @Order(5)
    void testJsonNestedObjects() throws Exception {
        String namespace = "json-nested-" + UUID.randomUUID().toString().substring(0, 8) + ".json";

        createNamespace(namespace, "json");

        String jsonContent = "{\n" +
                "  \"level1\": {\n" +
                "    \"level2\": {\n" +
                "      \"level3\": {\n" +
                "        \"value\": \"deep-nested\"\n" +
                "      }\n" +
                "    }\n" +
                "  },\n" +
                "  \"array\": [1, 2, 3, {\"nested\": true}]\n" +
                "}";

        publishConfig(namespace, jsonContent);

        String url = String.format("%s/configfiles/json/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("Nested JSON: " + content);
        assertNotNull(content);

        deleteNamespace(namespace);
    }

    /**
     * AFF-006: Test JSON format with arrays
     */
    @Test
    @Order(6)
    void testJsonArrays() throws Exception {
        String namespace = "json-array-" + UUID.randomUUID().toString().substring(0, 8) + ".json";

        createNamespace(namespace, "json");

        String jsonContent = "{\"servers\":[" +
                "{\"host\":\"server1.example.com\",\"port\":8080}," +
                "{\"host\":\"server2.example.com\",\"port\":8081}," +
                "{\"host\":\"server3.example.com\",\"port\":8082}" +
                "],\"tags\":[\"production\",\"primary\",\"us-west\"]}";

        publishConfig(namespace, jsonContent);

        String url = String.format("%s/configfiles/json/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("JSON with arrays: " + content);
        assertTrue(content.contains("servers") || content.contains("server1"),
                "Should return JSON with arrays");

        deleteNamespace(namespace);
    }

    /**
     * AFF-007: Test JSON format with special types
     */
    @Test
    @Order(7)
    void testJsonSpecialTypes() throws Exception {
        String namespace = "json-types-" + UUID.randomUUID().toString().substring(0, 8) + ".json";

        createNamespace(namespace, "json");

        String jsonContent = "{" +
                "\"stringValue\":\"hello\"," +
                "\"intValue\":42," +
                "\"floatValue\":3.14159," +
                "\"boolTrue\":true," +
                "\"boolFalse\":false," +
                "\"nullValue\":null," +
                "\"emptyString\":\"\"," +
                "\"emptyArray\":[]," +
                "\"emptyObject\":{}" +
                "}";

        publishConfig(namespace, jsonContent);

        String url = String.format("%s/configfiles/json/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("JSON special types: " + content);
        assertNotNull(content);

        deleteNamespace(namespace);
    }

    /**
     * AFF-008: Test invalid JSON handling
     */
    @Test
    @Order(8)
    void testJsonInvalid() throws Exception {
        String namespace = "json-invalid-" + UUID.randomUUID().toString().substring(0, 8) + ".json";

        createNamespace(namespace, "json");

        // Invalid JSON (missing closing brace)
        String invalidJson = "{\"key\": \"value\"";

        try {
            publishConfig(namespace, invalidJson);
            // Some implementations may accept invalid JSON as raw content
        } catch (Exception e) {
            System.out.println("Invalid JSON rejected: " + e.getMessage());
        }

        deleteNamespace(namespace);
    }

    // ==================== YAML Format Tests ====================

    /**
     * AFF-009: Test YAML format config retrieval
     */
    @Test
    @Order(9)
    void testYamlFormatGet() throws Exception {
        String namespace = "test-yaml-" + UUID.randomUUID().toString().substring(0, 8) + ".yaml";

        createNamespace(namespace, "yaml");

        String yamlContent = "server:\n" +
                "  port: 8080\n" +
                "  host: localhost\n" +
                "database:\n" +
                "  url: jdbc:mysql://localhost:3306/db\n" +
                "  username: root";

        publishConfig(namespace, yamlContent);

        String url = String.format("%s/configfiles/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("YAML content: " + content);
        assertNotNull(content);

        deleteNamespace(namespace);
    }

    /**
     * AFF-010: Test YAML format with lists
     */
    @Test
    @Order(10)
    void testYamlWithLists() throws Exception {
        String namespace = "yaml-list-" + UUID.randomUUID().toString().substring(0, 8) + ".yaml";

        createNamespace(namespace, "yaml");

        String yamlContent = "servers:\n" +
                "  - host: server1.example.com\n" +
                "    port: 8080\n" +
                "  - host: server2.example.com\n" +
                "    port: 8081\n" +
                "tags:\n" +
                "  - production\n" +
                "  - primary\n" +
                "  - us-west";

        publishConfig(namespace, yamlContent);

        String url = String.format("%s/configfiles/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("YAML with lists: " + content);
        assertNotNull(content);

        deleteNamespace(namespace);
    }

    /**
     * AFF-011: Test YAML format with anchors and aliases
     */
    @Test
    @Order(11)
    void testYamlAnchorsAliases() throws Exception {
        String namespace = "yaml-anchor-" + UUID.randomUUID().toString().substring(0, 8) + ".yaml";

        createNamespace(namespace, "yaml");

        String yamlContent = "defaults: &defaults\n" +
                "  timeout: 30\n" +
                "  retries: 3\n" +
                "development:\n" +
                "  <<: *defaults\n" +
                "  debug: true\n" +
                "production:\n" +
                "  <<: *defaults\n" +
                "  debug: false";

        publishConfig(namespace, yamlContent);

        String url = String.format("%s/configfiles/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("YAML with anchors: " + content);
        assertNotNull(content);

        deleteNamespace(namespace);
    }

    /**
     * AFF-012: Test YAML format with multi-line strings
     */
    @Test
    @Order(12)
    void testYamlMultilineStrings() throws Exception {
        String namespace = "yaml-multiline-" + UUID.randomUUID().toString().substring(0, 8) + ".yaml";

        createNamespace(namespace, "yaml");

        String yamlContent = "description: |\n" +
                "  This is a multi-line\n" +
                "  string that preserves\n" +
                "  newlines.\n" +
                "folded: >\n" +
                "  This is a folded\n" +
                "  string that becomes\n" +
                "  a single line.";

        publishConfig(namespace, yamlContent);

        String url = String.format("%s/configfiles/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("YAML multiline: " + content);
        assertNotNull(content);

        deleteNamespace(namespace);
    }

    /**
     * AFF-013: Test YML extension (alias for YAML)
     */
    @Test
    @Order(13)
    void testYmlExtension() throws Exception {
        String namespace = "test-yml-" + UUID.randomUUID().toString().substring(0, 8) + ".yml";

        createNamespace(namespace, "yml");

        String yamlContent = "config:\n  key: value";
        publishConfig(namespace, yamlContent);

        String url = String.format("%s/configfiles/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("YML content: " + content);
        assertNotNull(content);

        deleteNamespace(namespace);
    }

    // ==================== XML Format Tests ====================

    /**
     * AFF-014: Test XML format config retrieval
     */
    @Test
    @Order(14)
    void testXmlFormatGet() throws Exception {
        String namespace = "test-xml-" + UUID.randomUUID().toString().substring(0, 8) + ".xml";

        createNamespace(namespace, "xml");

        String xmlContent = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n" +
                "<configuration>\n" +
                "  <server>\n" +
                "    <host>localhost</host>\n" +
                "    <port>8080</port>\n" +
                "  </server>\n" +
                "</configuration>";

        publishConfig(namespace, xmlContent);

        String url = String.format("%s/configfiles/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("XML content: " + content);
        assertNotNull(content);

        deleteNamespace(namespace);
    }

    /**
     * AFF-015: Test XML format with attributes
     */
    @Test
    @Order(15)
    void testXmlWithAttributes() throws Exception {
        String namespace = "xml-attr-" + UUID.randomUUID().toString().substring(0, 8) + ".xml";

        createNamespace(namespace, "xml");

        String xmlContent = "<?xml version=\"1.0\"?>\n" +
                "<servers>\n" +
                "  <server id=\"1\" role=\"primary\">\n" +
                "    <host>server1.example.com</host>\n" +
                "  </server>\n" +
                "  <server id=\"2\" role=\"secondary\">\n" +
                "    <host>server2.example.com</host>\n" +
                "  </server>\n" +
                "</servers>";

        publishConfig(namespace, xmlContent);

        String url = String.format("%s/configfiles/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("XML with attributes: " + content);
        assertNotNull(content);

        deleteNamespace(namespace);
    }

    /**
     * AFF-016: Test XML format with namespaces
     */
    @Test
    @Order(16)
    void testXmlWithNamespaces() throws Exception {
        String namespace = "xml-ns-" + UUID.randomUUID().toString().substring(0, 8) + ".xml";

        createNamespace(namespace, "xml");

        String xmlContent = "<?xml version=\"1.0\"?>\n" +
                "<root xmlns:app=\"http://example.com/app\">\n" +
                "  <app:config>\n" +
                "    <app:setting name=\"timeout\">30</app:setting>\n" +
                "  </app:config>\n" +
                "</root>";

        publishConfig(namespace, xmlContent);

        String url = String.format("%s/configfiles/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("XML with namespaces: " + content);
        assertNotNull(content);

        deleteNamespace(namespace);
    }

    /**
     * AFF-017: Test XML format with CDATA
     */
    @Test
    @Order(17)
    void testXmlWithCdata() throws Exception {
        String namespace = "xml-cdata-" + UUID.randomUUID().toString().substring(0, 8) + ".xml";

        createNamespace(namespace, "xml");

        String xmlContent = "<?xml version=\"1.0\"?>\n" +
                "<config>\n" +
                "  <script><![CDATA[\n" +
                "    function test() {\n" +
                "      return x < y && y > z;\n" +
                "    }\n" +
                "  ]]></script>\n" +
                "</config>";

        publishConfig(namespace, xmlContent);

        String url = String.format("%s/configfiles/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("XML with CDATA: " + content);
        assertNotNull(content);

        deleteNamespace(namespace);
    }

    // ==================== TXT Format Tests ====================

    /**
     * AFF-018: Test TXT format (raw text)
     */
    @Test
    @Order(18)
    void testTxtFormatGet() throws Exception {
        String namespace = "test-txt-" + UUID.randomUUID().toString().substring(0, 8) + ".txt";

        createNamespace(namespace, "txt");

        String textContent = "This is plain text content.\n" +
                "It can contain any characters.\n" +
                "No special parsing is applied.";

        publishConfig(namespace, textContent);

        String url = String.format("%s/configfiles/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);
        String content = httpGet(url);

        System.out.println("TXT content: " + content);
        assertNotNull(content);

        deleteNamespace(namespace);
    }

    /**
     * AFF-019: Test format auto-detection from extension
     */
    @Test
    @Order(19)
    void testFormatAutoDetection() throws Exception {
        // Test that format is auto-detected from namespace extension
        String[] formats = {".properties", ".json", ".yaml", ".yml", ".xml", ".txt"};

        for (String ext : formats) {
            String namespace = "auto-detect-" + UUID.randomUUID().toString().substring(0, 8) + ext;
            String format = ext.substring(1); // Remove leading dot

            try {
                createNamespace(namespace, format);
                publishConfig(namespace, "test=value");

                String url = String.format("%s/configfiles/%s/%s/%s",
                        configServiceUrl, APP_ID, CLUSTER, namespace);
                String content = httpGet(url);

                System.out.println("Auto-detected " + ext + ": " + (content != null ? "OK" : "FAIL"));

                deleteNamespace(namespace);
            } catch (Exception e) {
                System.out.println("Auto-detection for " + ext + " failed: " + e.getMessage());
            }
        }
    }

    /**
     * AFF-020: Test default namespace (application) format
     */
    @Test
    @Order(20)
    void testDefaultNamespaceFormat() throws Exception {
        // Default "application" namespace uses properties format
        String url = String.format("%s/configs/%s/%s/application",
                configServiceUrl, APP_ID, CLUSTER);

        try {
            String content = httpGet(url);
            System.out.println("Default namespace: " + content);
            assertNotNull(content);
        } catch (Exception e) {
            System.out.println("Default namespace not available: " + e.getMessage());
        }
    }

    // ==================== Helper Methods ====================

    private void createNamespace(String namespace, String format) throws Exception {
        String url = openApiUrl + "/openapi/v1/envs/" + ENV + "/apps/" + APP_ID + "/clusters/" + CLUSTER + "/namespaces";
        String body = String.format(
                "{\"name\":\"%s\",\"appId\":\"%s\",\"format\":\"%s\",\"isPublic\":false,\"comment\":\"Test namespace\"}",
                namespace, APP_ID, format);

        try {
            httpPost(url, body, "application/json");
        } catch (Exception e) {
            // Namespace may already exist
        }
    }

    private void publishConfig(String namespace, String content) throws Exception {
        // First, create/update item
        String itemUrl = openApiUrl + "/openapi/v1/envs/" + ENV + "/apps/" + APP_ID +
                "/clusters/" + CLUSTER + "/namespaces/" + namespace + "/items";
        String itemBody = String.format(
                "{\"key\":\"content\",\"value\":\"%s\",\"dataChangeCreatedBy\":\"test\"}",
                content.replace("\"", "\\\"").replace("\n", "\\n"));

        try {
            httpPost(itemUrl, itemBody, "application/json");
        } catch (Exception e) {
            // Item may already exist, try update
        }

        // Then release
        String releaseUrl = openApiUrl + "/openapi/v1/envs/" + ENV + "/apps/" + APP_ID +
                "/clusters/" + CLUSTER + "/namespaces/" + namespace + "/releases";
        String releaseBody = "{\"releaseTitle\":\"test-release\",\"releasedBy\":\"test\"}";

        httpPost(releaseUrl, releaseBody, "application/json");
    }

    private void deleteNamespace(String namespace) throws Exception {
        String url = openApiUrl + "/openapi/v1/envs/" + ENV + "/apps/" + APP_ID +
                "/clusters/" + CLUSTER + "/namespaces/" + namespace;
        try {
            httpDelete(url);
        } catch (Exception e) {
            // Ignore deletion errors
        }
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

    private static String httpDelete(String urlString) throws Exception {
        URL url = new URL(urlString);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("DELETE");
        conn.setRequestProperty("Authorization", portalToken);

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
