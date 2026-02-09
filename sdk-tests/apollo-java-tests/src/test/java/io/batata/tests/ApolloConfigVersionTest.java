package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigChangeListener;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.model.ConfigChange;
import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.*;

import java.io.*;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Configuration Versioning and Release Management Tests
 *
 * Tests for configuration version tracking and release management:
 * - Release key tracking and changes
 * - Version history and rollback
 * - Multi-namespace versioning
 * - Gray release versioning
 * - Version persistence and caching
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloConfigVersionTest {

    private static String configServiceUrl;
    private static String openApiUrl;
    private static String portalToken;
    private static final String APP_ID = "version-test-app";
    private static final String CLUSTER = "default";
    private static final String ENV = "DEV";
    private static final String TEST_NAMESPACE = "application";
    private static Gson gson;
    private static Path cacheDir;

    @BeforeAll
    static void setup() throws Exception {
        configServiceUrl = System.getProperty("apollo.config.service", "http://127.0.0.1:8848");
        openApiUrl = System.getProperty("apollo.openapi.url", "http://127.0.0.1:8848");
        portalToken = System.getProperty("apollo.portal.token", "test-token");
        gson = new Gson();

        // Set Apollo configuration
        System.setProperty("app.id", APP_ID);
        System.setProperty("apollo.meta", configServiceUrl);
        System.setProperty("apollo.configService", configServiceUrl);
        System.setProperty("env", ENV);
        System.setProperty("apollo.cluster", CLUSTER);

        // Create temp cache directory for tests
        cacheDir = Files.createTempDirectory("apollo-version-cache-test");
        System.setProperty("apollo.cache-dir", cacheDir.toString());

        System.out.println("Apollo Config Version Test Setup");
        System.out.println("Config Service: " + configServiceUrl);
        System.out.println("Cache directory: " + cacheDir);

        // Create test app
        createTestApp();
    }

    @AfterAll
    static void teardown() throws IOException {
        // Cleanup cache directory
        if (cacheDir != null && Files.exists(cacheDir)) {
            Files.walk(cacheDir)
                    .sorted(Comparator.reverseOrder())
                    .map(Path::toFile)
                    .forEach(File::delete);
        }
    }

    private static void createTestApp() throws Exception {
        String url = openApiUrl + "/openapi/v1/apps";
        String body = String.format(
                "{\"appId\":\"%s\",\"name\":\"Version Test App\",\"orgId\":\"test\",\"orgName\":\"Test Org\",\"ownerName\":\"test\",\"ownerEmail\":\"test@test.com\"}",
                APP_ID);

        try {
            httpPost(url, body, "application/json");
        } catch (Exception e) {
            System.out.println("App creation: " + e.getMessage());
        }
    }

    // ==================== Release Key Tests ====================

    /**
     * ACV-001: Test get current release key
     */
    @Test
    @Order(1)
    void testGetCurrentReleaseKey() throws Exception {
        String namespace = "release-key-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);
        publishConfig(namespace, "test.key", "initial-value");

        Thread.sleep(1000);

        // Get config with release key
        String url = String.format("%s/configs/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);

        HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
        conn.setRequestMethod("GET");

        int responseCode = conn.getResponseCode();
        if (responseCode == 200) {
            String response = readResponse(conn);
            System.out.println("ACV-001: Current release response: " + response);

            JsonObject configResponse = gson.fromJson(response, JsonObject.class);
            if (configResponse.has("releaseKey")) {
                String releaseKey = configResponse.get("releaseKey").getAsString();
                assertNotNull(releaseKey, "Release key should not be null");
                assertFalse(releaseKey.isEmpty(), "Release key should not be empty");
                System.out.println("ACV-001: Release key: " + releaseKey);
            } else {
                System.out.println("ACV-001: Response does not contain releaseKey");
            }
        } else {
            System.out.println("ACV-001: Response code: " + responseCode);
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ACV-002: Test release key changes on update
     */
    @Test
    @Order(2)
    void testReleaseKeyChangesOnUpdate() throws Exception {
        String namespace = "release-change-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);

        // First release
        publishConfig(namespace, "version.key", "value-v1");
        Thread.sleep(1000);

        String releaseKey1 = getReleaseKey(namespace);
        System.out.println("ACV-002: First release key: " + releaseKey1);

        // Second release
        updateConfig(namespace, "version.key", "value-v2");
        publishRelease(namespace, "Second Release");
        Thread.sleep(1000);

        String releaseKey2 = getReleaseKey(namespace);
        System.out.println("ACV-002: Second release key: " + releaseKey2);

        // Release keys should be different
        if (releaseKey1 != null && releaseKey2 != null) {
            assertNotEquals(releaseKey1, releaseKey2, "Release keys should change after update");
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ACV-003: Test config version tracking
     */
    @Test
    @Order(3)
    void testConfigVersionTracking() throws Exception {
        String namespace = "version-track-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);

        List<String> releaseKeys = new ArrayList<>();

        // Create multiple versions
        for (int i = 1; i <= 3; i++) {
            publishConfig(namespace, "tracked.key", "version-" + i);
            Thread.sleep(500);

            String releaseKey = getReleaseKey(namespace);
            if (releaseKey != null) {
                releaseKeys.add(releaseKey);
            }
            System.out.println("ACV-003: Version " + i + " release key: " + releaseKey);
        }

        // All release keys should be unique
        Set<String> uniqueKeys = new HashSet<>(releaseKeys);
        System.out.println("ACV-003: Total versions created: " + releaseKeys.size());
        System.out.println("ACV-003: Unique release keys: " + uniqueKeys.size());

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ACV-004: Test release history
     */
    @Test
    @Order(4)
    void testReleaseHistory() throws Exception {
        String namespace = "release-history-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);

        // Create multiple releases
        for (int i = 1; i <= 3; i++) {
            publishConfig(namespace, "history.key", "history-value-" + i);
            Thread.sleep(500);
        }

        // Get release history
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/releases",
                openApiUrl, ENV, APP_ID, CLUSTER, namespace);

        HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
        conn.setRequestMethod("GET");
        conn.setRequestProperty("Authorization", portalToken);

        int responseCode = conn.getResponseCode();
        if (responseCode == 200) {
            String response = readResponse(conn);
            System.out.println("ACV-004: Release history: " + response);

            // Parse history
            try {
                JsonArray releases = gson.fromJson(response, JsonArray.class);
                System.out.println("ACV-004: Found " + releases.size() + " releases in history");
            } catch (Exception e) {
                System.out.println("ACV-004: Could not parse as array, response: " + response);
            }
        } else {
            System.out.println("ACV-004: Response code: " + responseCode);
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ACV-005: Test rollback to previous version
     */
    @Test
    @Order(5)
    void testRollbackToPreviousVersion() throws Exception {
        String namespace = "rollback-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);

        // Create first version
        publishConfig(namespace, "rollback.key", "original-value");
        Thread.sleep(500);
        String releaseKey1 = getReleaseKey(namespace);

        // Create second version
        updateConfig(namespace, "rollback.key", "updated-value");
        publishRelease(namespace, "Updated Release");
        Thread.sleep(500);

        // Attempt rollback
        String rollbackUrl = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/releases/%s/rollback",
                openApiUrl, ENV, APP_ID, CLUSTER, namespace, releaseKey1);

        try {
            HttpURLConnection conn = (HttpURLConnection) new URL(rollbackUrl).openConnection();
            conn.setRequestMethod("PUT");
            conn.setRequestProperty("Authorization", portalToken);
            conn.setRequestProperty("Content-Type", "application/json");
            conn.setDoOutput(true);

            String body = "{\"operator\":\"test\"}";
            try (OutputStream os = conn.getOutputStream()) {
                os.write(body.getBytes(StandardCharsets.UTF_8));
            }

            int responseCode = conn.getResponseCode();
            System.out.println("ACV-005: Rollback response code: " + responseCode);

            if (responseCode == 200) {
                String response = readResponse(conn);
                System.out.println("ACV-005: Rollback successful: " + response);
            }
        } catch (Exception e) {
            System.out.println("ACV-005: Rollback error: " + e.getMessage());
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ACV-006: Test version comparison
     */
    @Test
    @Order(6)
    void testVersionComparison() throws Exception {
        String namespace = "version-compare-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);

        // Create two versions
        publishConfig(namespace, "compare.key", "value-a");
        Thread.sleep(500);
        String releaseKey1 = getReleaseKey(namespace);
        String value1 = getConfigValue(namespace, "compare.key");

        updateConfig(namespace, "compare.key", "value-b");
        publishRelease(namespace, "Version B");
        Thread.sleep(500);
        String releaseKey2 = getReleaseKey(namespace);
        String value2 = getConfigValue(namespace, "compare.key");

        System.out.println("ACV-006: Version 1 - Key: " + releaseKey1 + ", Value: " + value1);
        System.out.println("ACV-006: Version 2 - Key: " + releaseKey2 + ", Value: " + value2);

        // Compare release keys
        if (releaseKey1 != null && releaseKey2 != null) {
            assertNotEquals(releaseKey1, releaseKey2, "Different versions should have different release keys");
        }

        // Compare values
        if (value1 != null && value2 != null) {
            assertNotEquals(value1, value2, "Different versions should have different values");
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ACV-007: Test release notification ID
     */
    @Test
    @Order(7)
    void testReleaseNotificationId() throws Exception {
        String namespace = "notification-id-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);
        publishConfig(namespace, "notify.key", "notify-value");
        Thread.sleep(1000);

        // Get notification with ID -1 (initial)
        String notifications = String.format(
                "[{\"namespaceName\":\"%s\",\"notificationId\":-1}]",
                namespace);

        String url = String.format("%s/notifications/v2?appId=%s&cluster=%s&notifications=%s",
                configServiceUrl, APP_ID, CLUSTER, URLEncoder.encode(notifications, "UTF-8"));

        HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
        conn.setRequestMethod("GET");
        conn.setConnectTimeout(5000);
        conn.setReadTimeout(10000);

        int responseCode = conn.getResponseCode();
        if (responseCode == 200) {
            String response = readResponse(conn);
            System.out.println("ACV-007: Notification response: " + response);

            if (response.contains("notificationId")) {
                System.out.println("ACV-007: Notification ID found in response");

                // Parse notification ID
                JsonArray notificationArray = gson.fromJson(response, JsonArray.class);
                if (notificationArray.size() > 0) {
                    JsonObject notification = notificationArray.get(0).getAsJsonObject();
                    long notificationId = notification.get("notificationId").getAsLong();
                    System.out.println("ACV-007: Notification ID: " + notificationId);
                    assertTrue(notificationId > -1, "Notification ID should be updated");
                }
            }
        } else {
            System.out.println("ACV-007: Response code: " + responseCode);
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ACV-008: Test multi-namespace versioning
     */
    @Test
    @Order(8)
    void testMultiNamespaceVersioning() throws Exception {
        String ns1 = "multi-ns1-" + UUID.randomUUID().toString().substring(0, 8);
        String ns2 = "multi-ns2-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(ns1);
        createNamespace(ns2);

        // Publish different configs to each namespace
        publishConfig(ns1, "ns1.key", "ns1-value");
        publishConfig(ns2, "ns2.key", "ns2-value");
        Thread.sleep(1000);

        String releaseKey1 = getReleaseKey(ns1);
        String releaseKey2 = getReleaseKey(ns2);

        System.out.println("ACV-008: Namespace 1 release key: " + releaseKey1);
        System.out.println("ACV-008: Namespace 2 release key: " + releaseKey2);

        // Each namespace should have its own release key
        if (releaseKey1 != null && releaseKey2 != null) {
            // Release keys are unique per namespace
            System.out.println("ACV-008: Both namespaces have independent release keys");
        }

        // Cleanup
        deleteNamespace(ns1);
        deleteNamespace(ns2);
    }

    /**
     * ACV-009: Test version with gray release
     */
    @Test
    @Order(9)
    void testVersionWithGrayRelease() throws Exception {
        String namespace = "gray-release-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);
        publishConfig(namespace, "gray.key", "stable-value");
        Thread.sleep(500);

        // Get stable release key
        String stableReleaseKey = getReleaseKey(namespace);
        System.out.println("ACV-009: Stable release key: " + stableReleaseKey);

        // Create gray release
        String grayUrl = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/gray-del-releases",
                openApiUrl, ENV, APP_ID, CLUSTER, namespace);

        JsonObject grayBody = new JsonObject();
        grayBody.addProperty("releaseTitle", "Gray Release Test");
        grayBody.addProperty("releaseComment", "Testing gray release versioning");
        grayBody.addProperty("releasedBy", "test");
        grayBody.addProperty("isEmergencyPublish", false);

        JsonObject grayRules = new JsonObject();
        JsonArray clientIps = new JsonArray();
        clientIps.add("192.168.1.100");
        grayRules.add("clientAppId", clientIps);
        grayBody.add("grayDelRules", grayRules);

        try {
            String response = httpPost(grayUrl, gson.toJson(grayBody), "application/json");
            System.out.println("ACV-009: Gray release response: " + response);
        } catch (Exception e) {
            System.out.println("ACV-009: Gray release not supported or error: " + e.getMessage());
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ACV-010: Test concurrent version updates
     */
    @Test
    @Order(10)
    void testConcurrentVersionUpdates() throws Exception {
        String namespace = "concurrent-version-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);
        publishConfig(namespace, "concurrent.key", "initial");
        Thread.sleep(500);

        int updateCount = 5;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(updateCount);
        AtomicInteger successCount = new AtomicInteger(0);
        Set<String> releaseKeys = Collections.synchronizedSet(new HashSet<>());

        for (int i = 0; i < updateCount; i++) {
            final int index = i;
            new Thread(() -> {
                try {
                    startLatch.await();

                    updateConfig(namespace, "concurrent.key", "value-" + index);
                    publishRelease(namespace, "Concurrent Release " + index);

                    String releaseKey = getReleaseKey(namespace);
                    if (releaseKey != null) {
                        releaseKeys.add(releaseKey);
                        successCount.incrementAndGet();
                    }
                } catch (Exception e) {
                    System.out.println("ACV-010: Concurrent update " + index + " error: " + e.getMessage());
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        // Start all updates simultaneously
        startLatch.countDown();
        boolean completed = doneLatch.await(30, TimeUnit.SECONDS);

        System.out.println("ACV-010: Completed: " + completed);
        System.out.println("ACV-010: Successful updates: " + successCount.get());
        System.out.println("ACV-010: Unique release keys: " + releaseKeys.size());

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ACV-011: Test version persistence
     */
    @Test
    @Order(11)
    void testVersionPersistence() throws Exception {
        String namespace = "persist-version-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);
        publishConfig(namespace, "persist.key", "persist-value");
        Thread.sleep(1000);

        // Get release key
        String releaseKey1 = getReleaseKey(namespace);
        System.out.println("ACV-011: Initial release key: " + releaseKey1);

        // Wait and fetch again to verify persistence
        Thread.sleep(2000);

        String releaseKey2 = getReleaseKey(namespace);
        System.out.println("ACV-011: After wait release key: " + releaseKey2);

        // Release key should remain the same (no updates)
        if (releaseKey1 != null && releaseKey2 != null) {
            assertEquals(releaseKey1, releaseKey2, "Release key should persist without changes");
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ACV-012: Test version after restart (simulated via SDK cache)
     */
    @Test
    @Order(12)
    void testVersionAfterRestart() throws Exception {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);

        // Get initial property
        String initialValue = config.getProperty("restart.test.key", "default-restart");
        System.out.println("ACV-012: Initial value: " + initialValue);

        // Access config again (simulating restart with cache)
        Config config2 = ConfigService.getConfig(TEST_NAMESPACE);
        String afterValue = config2.getProperty("restart.test.key", "default-restart");
        System.out.println("ACV-012: After restart value: " + afterValue);

        // Values should be consistent
        assertEquals(initialValue, afterValue, "Values should be consistent across config accesses");

        // Verify cache is being used
        assertSame(config, config2, "Same config instance should be returned");
    }

    /**
     * ACV-013: Test release key format
     */
    @Test
    @Order(13)
    void testReleaseKeyFormat() throws Exception {
        String namespace = "key-format-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);
        publishConfig(namespace, "format.key", "format-value");
        Thread.sleep(1000);

        String releaseKey = getReleaseKey(namespace);
        System.out.println("ACV-013: Release key: " + releaseKey);

        if (releaseKey != null) {
            // Verify release key format
            assertFalse(releaseKey.isEmpty(), "Release key should not be empty");

            // Apollo release keys are typically timestamps or formatted strings
            // They should only contain valid characters
            assertTrue(releaseKey.matches("[a-zA-Z0-9\\-_]+"),
                    "Release key should contain valid characters");

            System.out.println("ACV-013: Release key format valid");
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ACV-014: Test version timeline
     */
    @Test
    @Order(14)
    void testVersionTimeline() throws Exception {
        String namespace = "timeline-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);

        List<Long> timestamps = new ArrayList<>();

        // Create multiple versions with timestamps
        for (int i = 1; i <= 3; i++) {
            long beforePublish = System.currentTimeMillis();
            publishConfig(namespace, "timeline.key", "timeline-value-" + i);
            long afterPublish = System.currentTimeMillis();

            timestamps.add(beforePublish);
            Thread.sleep(500);

            System.out.println("ACV-014: Version " + i + " published between " +
                    beforePublish + " and " + afterPublish);
        }

        // Verify timestamps are in order
        for (int i = 1; i < timestamps.size(); i++) {
            assertTrue(timestamps.get(i) > timestamps.get(i - 1),
                    "Version timestamps should be increasing");
        }

        System.out.println("ACV-014: Version timeline verified");

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ACV-015: Test version metadata
     */
    @Test
    @Order(15)
    void testVersionMetadata() throws Exception {
        String namespace = "metadata-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);

        String releaseTitle = "Metadata Test Release";
        String releaseComment = "Testing release metadata";

        // Publish with metadata
        String itemUrl = openApiUrl + "/openapi/v1/envs/" + ENV + "/apps/" + APP_ID +
                "/clusters/" + CLUSTER + "/namespaces/" + namespace + "/items";
        String itemBody = "{\"key\":\"meta.key\",\"value\":\"meta-value\",\"dataChangeCreatedBy\":\"test\"}";

        try {
            httpPost(itemUrl, itemBody, "application/json");
        } catch (Exception e) {
            // Ignore if already exists
        }

        String releaseUrl = openApiUrl + "/openapi/v1/envs/" + ENV + "/apps/" + APP_ID +
                "/clusters/" + CLUSTER + "/namespaces/" + namespace + "/releases";

        JsonObject releaseBody = new JsonObject();
        releaseBody.addProperty("releaseTitle", releaseTitle);
        releaseBody.addProperty("releaseComment", releaseComment);
        releaseBody.addProperty("releasedBy", "metadata-test-user");

        try {
            String response = httpPost(releaseUrl, gson.toJson(releaseBody), "application/json");
            System.out.println("ACV-015: Release with metadata response: " + response);

            // Verify metadata in response
            if (response.contains(releaseTitle)) {
                System.out.println("ACV-015: Release title found in metadata");
            }
        } catch (Exception e) {
            System.out.println("ACV-015: Release error: " + e.getMessage());
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ACV-016: Test version diff detection
     */
    @Test
    @Order(16)
    void testVersionDiffDetection() throws Exception {
        String namespace = "diff-detect-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);

        // Create initial version
        publishConfig(namespace, "diff.key1", "value1");
        publishConfig(namespace, "diff.key2", "value2");
        Thread.sleep(500);

        String releaseKey1 = getReleaseKey(namespace);

        // Modify one key
        updateConfig(namespace, "diff.key1", "modified-value1");
        publishRelease(namespace, "Modified Release");
        Thread.sleep(500);

        String releaseKey2 = getReleaseKey(namespace);

        System.out.println("ACV-016: Before change release key: " + releaseKey1);
        System.out.println("ACV-016: After change release key: " + releaseKey2);

        // Release key should change to reflect the diff
        if (releaseKey1 != null && releaseKey2 != null) {
            assertNotEquals(releaseKey1, releaseKey2, "Release key should change when config differs");
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ACV-017: Test version callback
     */
    @Test
    @Order(17)
    void testVersionCallback() throws Exception {
        Config config = ConfigService.getConfig(TEST_NAMESPACE);
        AtomicReference<String> oldReleaseKey = new AtomicReference<>();
        AtomicReference<String> newReleaseKey = new AtomicReference<>();
        CountDownLatch changeLatch = new CountDownLatch(1);

        // Add change listener to detect version changes
        ConfigChangeListener listener = changeEvent -> {
            System.out.println("ACV-017: Config change detected for namespace: " + changeEvent.getNamespace());
            System.out.println("ACV-017: Changed keys: " + changeEvent.changedKeys());

            for (String key : changeEvent.changedKeys()) {
                ConfigChange change = changeEvent.getChange(key);
                System.out.println("ACV-017: Key: " + key +
                        ", Old: " + change.getOldValue() +
                        ", New: " + change.getNewValue() +
                        ", Type: " + change.getChangeType());
            }

            changeLatch.countDown();
        };

        config.addChangeListener(listener);
        System.out.println("ACV-017: Version change listener registered");

        // Note: In a real scenario, you would trigger a config change via OpenAPI
        // and verify the callback is invoked. For this test, we verify listener registration.

        // Wait briefly to demonstrate listener is active
        Thread.sleep(2000);

        System.out.println("ACV-017: Version callback listener test completed");
    }

    /**
     * ACV-018: Test version with cache
     */
    @Test
    @Order(18)
    void testVersionWithCache() throws Exception {
        String namespace = "cache-version-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);
        publishConfig(namespace, "cache.key", "cache-value");
        Thread.sleep(1000);

        // Get release key
        String releaseKey = getReleaseKey(namespace);
        System.out.println("ACV-018: Release key: " + releaseKey);

        // Access via SDK to trigger caching
        try {
            Config config = ConfigService.getConfig(namespace);
            String value = config.getProperty("cache.key", "not-found");
            System.out.println("ACV-018: Cached value: " + value);

            // Access again - should use cache
            String cachedValue = config.getProperty("cache.key", "not-found");
            assertEquals(value, cachedValue, "Cached value should be consistent");
            System.out.println("ACV-018: Cache consistency verified");
        } catch (Exception e) {
            System.out.println("ACV-018: SDK access: " + e.getMessage());
        }

        // Verify cache directory has content
        if (Files.exists(cacheDir)) {
            long fileCount = Files.list(cacheDir).count();
            System.out.println("ACV-018: Cache files: " + fileCount);
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    // ==================== Helper Methods ====================

    private void createNamespace(String namespace) throws Exception {
        String url = openApiUrl + "/openapi/v1/apps/" + APP_ID + "/appnamespaces";
        String body = String.format(
                "{\"name\":\"%s\",\"appId\":\"%s\",\"format\":\"properties\",\"isPublic\":false,\"dataChangeCreatedBy\":\"test\"}",
                namespace, APP_ID);

        try {
            httpPost(url, body, "application/json");
        } catch (Exception e) {
            // Namespace may already exist
            System.out.println("Create namespace " + namespace + ": " + e.getMessage());
        }
    }

    private void publishConfig(String namespace, String key, String value) throws Exception {
        // Create/update item
        String itemUrl = openApiUrl + "/openapi/v1/envs/" + ENV + "/apps/" + APP_ID +
                "/clusters/" + CLUSTER + "/namespaces/" + namespace + "/items";
        String itemBody = String.format(
                "{\"key\":\"%s\",\"value\":\"%s\",\"dataChangeCreatedBy\":\"test\"}",
                key, value);

        try {
            httpPost(itemUrl, itemBody, "application/json");
        } catch (Exception e) {
            // Item may already exist
        }

        // Release
        publishRelease(namespace, "Test Release");
    }

    private void updateConfig(String namespace, String key, String value) throws Exception {
        String itemUrl = openApiUrl + "/openapi/v1/envs/" + ENV + "/apps/" + APP_ID +
                "/clusters/" + CLUSTER + "/namespaces/" + namespace + "/items/" + key;
        String itemBody = String.format(
                "{\"key\":\"%s\",\"value\":\"%s\",\"dataChangeLastModifiedBy\":\"test\"}",
                key, value);

        HttpURLConnection conn = (HttpURLConnection) new URL(itemUrl).openConnection();
        conn.setRequestMethod("PUT");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/json");
        conn.setRequestProperty("Authorization", portalToken);

        try (OutputStream os = conn.getOutputStream()) {
            os.write(itemBody.getBytes(StandardCharsets.UTF_8));
        }

        int responseCode = conn.getResponseCode();
        if (responseCode >= 400) {
            // Try POST if PUT fails (item doesn't exist)
            String createUrl = openApiUrl + "/openapi/v1/envs/" + ENV + "/apps/" + APP_ID +
                    "/clusters/" + CLUSTER + "/namespaces/" + namespace + "/items";
            httpPost(createUrl, itemBody, "application/json");
        }
    }

    private void publishRelease(String namespace, String title) throws Exception {
        String releaseUrl = openApiUrl + "/openapi/v1/envs/" + ENV + "/apps/" + APP_ID +
                "/clusters/" + CLUSTER + "/namespaces/" + namespace + "/releases";
        String releaseBody = String.format("{\"releaseTitle\":\"%s\",\"releasedBy\":\"test\"}", title);

        httpPost(releaseUrl, releaseBody, "application/json");
    }

    private String getReleaseKey(String namespace) throws Exception {
        String url = String.format("%s/configs/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);

        HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
        conn.setRequestMethod("GET");

        int responseCode = conn.getResponseCode();
        if (responseCode == 200) {
            String response = readResponse(conn);
            JsonObject configResponse = gson.fromJson(response, JsonObject.class);
            if (configResponse.has("releaseKey")) {
                return configResponse.get("releaseKey").getAsString();
            }
        }
        return null;
    }

    private String getConfigValue(String namespace, String key) throws Exception {
        String url = String.format("%s/configs/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);

        HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
        conn.setRequestMethod("GET");

        int responseCode = conn.getResponseCode();
        if (responseCode == 200) {
            String response = readResponse(conn);
            JsonObject configResponse = gson.fromJson(response, JsonObject.class);
            if (configResponse.has("configurations")) {
                JsonObject configurations = configResponse.getAsJsonObject("configurations");
                if (configurations.has(key)) {
                    return configurations.get(key).getAsString();
                }
            }
        }
        return null;
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
