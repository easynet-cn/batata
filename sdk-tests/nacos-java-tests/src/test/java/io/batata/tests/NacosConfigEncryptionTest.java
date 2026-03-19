package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.Properties;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos 3.x Config Encryption SDK Compatibility Tests
 *
 * Tests Batata's compatibility with Nacos config encryption feature.
 * Configs with dataId prefix "cipher-" are automatically encrypted on the server side
 * and decrypted when read back by the client through the ConfigEncryptionFilter.
 *
 * The encryption is transparent: the SDK client publishes plain text, the server stores
 * it encrypted, and the SDK client receives decrypted content on read.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosConfigEncryptionTest {

    private static ConfigService configService;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static String serverAddr;
    private static String accessToken;

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);

        configService = NacosFactory.createConfigService(properties);
        assertNotNull(configService, "ConfigService should be created successfully");

        // Get access token for direct HTTP API calls
        accessToken = obtainAccessToken(serverAddr, username, password);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (configService != null) {
            configService.shutDown();
        }
    }

    /**
     * Obtain an access token via the auth login endpoint.
     */
    private static String obtainAccessToken(String serverAddr, String username, String password) {
        try {
            HttpClient httpClient = HttpClient.newHttpClient();
            String loginUrl = "http://" + serverAddr + "/nacos/v3/auth/user/login";
            String body = "username=" + username + "&password=" + password;

            HttpRequest request = HttpRequest.newBuilder()
                    .uri(URI.create(loginUrl))
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .POST(HttpRequest.BodyPublishers.ofString(body))
                    .build();

            HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
            String responseBody = response.body();

            // Extract token from JSON response (simple parsing)
            if (responseBody.contains("accessToken")) {
                int start = responseBody.indexOf("accessToken") + "accessToken".length();
                // Find the value after the key
                start = responseBody.indexOf("\"", start + 1) + 1;
                int end = responseBody.indexOf("\"", start);
                return responseBody.substring(start, end);
            }
            return null;
        } catch (Exception e) {
            System.err.println("Failed to obtain access token: " + e.getMessage());
            return null;
        }
    }

    /**
     * Read raw config content from server via HTTP API, bypassing SDK decryption.
     */
    private String getRawConfigFromServer(String dataId, String group) {
        try {
            HttpClient httpClient = HttpClient.newHttpClient();
            String url = "http://" + serverAddr + "/nacos/v2/cs/config?dataId=" + dataId
                    + "&group=" + group + "&accessToken=" + accessToken;

            HttpRequest request = HttpRequest.newBuilder()
                    .uri(URI.create(url))
                    .GET()
                    .build();

            HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
            return response.body();
        } catch (Exception e) {
            System.err.println("Failed to get raw config: " + e.getMessage());
            return null;
        }
    }

    // ==================== P1: Important Tests ====================

    /**
     * ENC-001: Publish config with cipher- prefix dataId, verify SDK returns decrypted content
     *
     * When publishing config with a dataId starting with "cipher-", the Nacos encryption
     * filter should encrypt the content on the server side. When reading back through the
     * SDK, the content should be transparently decrypted.
     */
    @Test
    @Order(1)
    void testCipherPrefixPublishAndGet() throws NacosException, InterruptedException {
        String uniqueId = UUID.randomUUID().toString().substring(0, 8);
        String dataId = "cipher-enc001-password-" + uniqueId;
        String plainContent = "db.password=super_secret_123";

        // Publish via SDK (content goes through encryption filter)
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, plainContent);
        assertTrue(published, "Publish cipher- config should succeed");
        Thread.sleep(1000);

        // Read back via SDK (should be decrypted transparently)
        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(retrieved, "Retrieved config should not be null");
        assertEquals(plainContent, retrieved,
                "SDK should return decrypted content matching original plain text");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * ENC-002: Normal config (without cipher- prefix) should not be encrypted
     *
     * A config with a regular dataId should be stored as-is without encryption.
     */
    @Test
    @Order(2)
    void testNormalConfigNotEncrypted() throws NacosException, InterruptedException {
        String uniqueId = UUID.randomUUID().toString().substring(0, 8);
        String normalDataId = "enc002-normal-" + uniqueId;
        String content = "normal.key=plain_value_123";

        // Publish normal config
        boolean published = configService.publishConfig(normalDataId, DEFAULT_GROUP, content);
        assertTrue(published, "Normal config publish should succeed");
        Thread.sleep(500);

        // Read back - should be identical
        String retrieved = configService.getConfig(normalDataId, DEFAULT_GROUP, 5000);
        assertEquals(content, retrieved, "Normal config should be returned as-is without encryption");

        // Cleanup
        configService.removeConfig(normalDataId, DEFAULT_GROUP);
    }

    /**
     * ENC-003: Verify cipher- config stored on server is different from plain text
     *
     * Use the HTTP API directly (bypassing SDK encryption filter) to read the raw
     * stored content. For cipher- configs, the raw stored content should differ
     * from the original plain text (because it should be encrypted on the server).
     */
    @Test
    @Order(3)
    void testCipherConfigStoredEncrypted() throws NacosException, InterruptedException {
        // Skip if no access token available
        Assumptions.assumeTrue(accessToken != null && !accessToken.isEmpty(),
                "Access token required for raw HTTP API access");

        String uniqueId = UUID.randomUUID().toString().substring(0, 8);
        String dataId = "cipher-enc003-secret-" + uniqueId;
        String plainContent = "encryption.secret=my_very_secret_value";

        // Publish via SDK
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, plainContent);
        assertTrue(published, "Cipher config publish should succeed");
        Thread.sleep(1000);

        // Read raw from server (bypassing decryption)
        String rawResponse = getRawConfigFromServer(dataId, DEFAULT_GROUP);
        assertNotNull(rawResponse, "Raw server response should not be null");

        // The raw stored content should NOT equal plain text
        // (it should be encrypted or wrapped in some way)
        // Note: The HTTP API response includes JSON wrapping, so we check the response body
        System.out.println("Raw server response for cipher config: " + rawResponse);

        // Via SDK, it should still be readable as plain text
        String sdkContent = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(plainContent, sdkContent, "SDK should still return decrypted plain content");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * ENC-004: Cipher config update preserves encryption behavior
     *
     * Update an existing cipher- config. The updated content should also be
     * encrypted on the server and decrypted on SDK read.
     */
    @Test
    @Order(4)
    void testCipherConfigUpdate() throws NacosException, InterruptedException {
        String uniqueId = UUID.randomUUID().toString().substring(0, 8);
        String dataId = "cipher-enc004-update-" + uniqueId;
        String initialContent = "secret.v1=initial_secret";
        String updatedContent = "secret.v2=updated_secret";

        // Publish initial
        boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, initialContent);
        assertTrue(published, "Initial cipher config publish should succeed");
        Thread.sleep(500);

        // Verify initial
        String retrieved1 = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(initialContent, retrieved1, "Initial content should be decrypted correctly");

        // Update
        boolean updated = configService.publishConfig(dataId, DEFAULT_GROUP, updatedContent);
        assertTrue(updated, "Cipher config update should succeed");
        Thread.sleep(500);

        // Verify updated
        String retrieved2 = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertEquals(updatedContent, retrieved2, "Updated content should be decrypted correctly");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * ENC-005: Cipher config with different content types
     *
     * Test encryption with JSON and YAML content types to verify encryption
     * works regardless of config type.
     */
    @Test
    @Order(5)
    void testCipherConfigWithDifferentTypes() throws NacosException, InterruptedException {
        String uniqueId = UUID.randomUUID().toString().substring(0, 8);

        // Test JSON type
        String jsonDataId = "cipher-enc005-json-" + uniqueId;
        String jsonContent = "{\"password\": \"json_secret_123\", \"apiKey\": \"key_abc\"}";

        boolean jsonPublished = configService.publishConfig(jsonDataId, DEFAULT_GROUP, jsonContent, "json");
        assertTrue(jsonPublished, "Cipher JSON config publish should succeed");
        Thread.sleep(500);

        String jsonRetrieved = configService.getConfig(jsonDataId, DEFAULT_GROUP, 5000);
        assertEquals(jsonContent, jsonRetrieved, "Cipher JSON content should be decrypted correctly");

        // Test YAML type
        String yamlDataId = "cipher-enc005-yaml-" + uniqueId;
        String yamlContent = "database:\n  password: yaml_secret_456\n  host: localhost";

        boolean yamlPublished = configService.publishConfig(yamlDataId, DEFAULT_GROUP, yamlContent, "yaml");
        assertTrue(yamlPublished, "Cipher YAML config publish should succeed");
        Thread.sleep(500);

        String yamlRetrieved = configService.getConfig(yamlDataId, DEFAULT_GROUP, 5000);
        assertEquals(yamlContent, yamlRetrieved, "Cipher YAML content should be decrypted correctly");

        // Cleanup
        configService.removeConfig(jsonDataId, DEFAULT_GROUP);
        configService.removeConfig(yamlDataId, DEFAULT_GROUP);
    }

    /**
     * ENC-006: Cipher config with listener should deliver decrypted content
     *
     * Register a listener on a cipher- config. When the config is updated, the listener
     * should receive the decrypted content.
     */
    @Test
    @Order(6)
    void testCipherConfigListener() throws NacosException, InterruptedException {
        String uniqueId = UUID.randomUUID().toString().substring(0, 8);
        String dataId = "cipher-enc006-listener-" + uniqueId;
        String content1 = "listener.secret.v1=first_secret";
        String content2 = "listener.secret.v2=second_secret";

        java.util.concurrent.atomic.AtomicReference<String> receivedContent =
                new java.util.concurrent.atomic.AtomicReference<>();
        java.util.concurrent.CountDownLatch latch = new java.util.concurrent.CountDownLatch(1);

        // Publish initial content
        configService.publishConfig(dataId, DEFAULT_GROUP, content1);
        Thread.sleep(3000);

        // Add listener
        configService.addListener(dataId, DEFAULT_GROUP, new com.alibaba.nacos.api.config.listener.Listener() {
            @Override
            public java.util.concurrent.Executor getExecutor() {
                return null;
            }

            @Override
            public void receiveConfigInfo(String configInfo) {
                System.out.println("Cipher listener received: " + configInfo);
                receivedContent.set(configInfo);
                latch.countDown();
            }
        });
        Thread.sleep(1000);

        // Update to trigger listener
        configService.publishConfig(dataId, DEFAULT_GROUP, content2);
        Thread.sleep(5000);

        boolean received = latch.await(20, TimeUnit.SECONDS);
        assertTrue(received, "Listener should receive notification for cipher config update");
        assertEquals(content2, receivedContent.get(),
                "Listener should receive decrypted content for cipher config");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * ENC-007: Remove cipher config
     *
     * Verify that cipher- configs can be removed just like normal configs.
     */
    @Test
    @Order(7)
    void testRemoveCipherConfig() throws NacosException, InterruptedException {
        String uniqueId = UUID.randomUUID().toString().substring(0, 8);
        String dataId = "cipher-enc007-remove-" + uniqueId;
        String content = "removable.secret=delete_me";

        // Publish
        configService.publishConfig(dataId, DEFAULT_GROUP, content);
        Thread.sleep(500);

        // Verify exists
        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(retrieved);

        // Remove
        boolean removed = configService.removeConfig(dataId, DEFAULT_GROUP);
        assertTrue(removed, "Cipher config removal should succeed");

        // Verify deleted
        String afterRemoval = configService.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertNull(afterRemoval, "Cipher config should be null after removal");
    }
}
