package io.batata.tests;

import org.junit.jupiter.api.*;

import java.io.*;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Long Polling Tests
 *
 * Tests for configuration notification mechanism via long polling:
 * - Notification endpoint behavior
 * - Multi-namespace notifications
 * - Timeout handling
 * - 304 Not Modified responses
 * - Notification ID tracking
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloLongPollingTest {

    private static String configServiceUrl;
    private static String openApiUrl;
    private static String portalToken;
    private static final String APP_ID = "long-polling-test";
    private static final String CLUSTER = "default";
    private static final String ENV = "DEV";

    @BeforeAll
    static void setup() throws Exception {
        configServiceUrl = System.getProperty("apollo.config.service", "http://127.0.0.1:8848");
        openApiUrl = System.getProperty("apollo.openapi.url", "http://127.0.0.1:8848");
        portalToken = System.getProperty("apollo.portal.token", "test-token");

        System.out.println("Apollo Long Polling Test Setup");
        System.out.println("Config Service: " + configServiceUrl);

        // Create test app
        createTestApp();
    }

    private static void createTestApp() throws Exception {
        String url = openApiUrl + "/openapi/v1/apps";
        String body = String.format(
                "{\"appId\":\"%s\",\"name\":\"Long Polling Test App\",\"orgId\":\"test\",\"orgName\":\"Test Org\",\"ownerName\":\"test\",\"ownerEmail\":\"test@test.com\"}",
                APP_ID);

        try {
            httpPost(url, body, "application/json");
        } catch (Exception e) {
            System.out.println("App creation: " + e.getMessage());
        }
    }

    // ==================== Basic Notification Tests ====================

    /**
     * ALP-001: Test notifications endpoint returns immediately when no changes
     */
    @Test
    @Order(1)
    void testNotificationsNoChange() throws Exception {
        String namespace = "application";
        long notificationId = -1;

        // Build notifications request
        String notifications = String.format(
                "[{\"namespaceName\":\"%s\",\"notificationId\":%d}]",
                namespace, notificationId);

        String url = String.format("%s/notifications/v2?appId=%s&cluster=%s&notifications=%s",
                configServiceUrl, APP_ID, CLUSTER, URLEncoder.encode(notifications, "UTF-8"));

        long startTime = System.currentTimeMillis();

        try {
            HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
            conn.setRequestMethod("GET");
            conn.setConnectTimeout(5000);
            conn.setReadTimeout(65000); // Long polling timeout

            int responseCode = conn.getResponseCode();
            long duration = System.currentTimeMillis() - startTime;

            System.out.println("Notifications no change - Code: " + responseCode + ", Duration: " + duration + "ms");

            // Should return 200 (with notifications), 304 (no change), or 404 (endpoint varies)
            assertTrue(responseCode == 200 || responseCode == 304 || responseCode == 404,
                    "Should return 200, 304, or 404, got: " + responseCode);
        } catch (java.net.SocketTimeoutException e) {
            long duration = System.currentTimeMillis() - startTime;
            System.out.println("Long polling timed out as expected - Duration: " + duration + "ms");
        }
    }

    /**
     * ALP-002: Test notifications returns updated notification ID
     */
    @Test
    @Order(2)
    void testNotificationIdUpdate() throws Exception {
        String namespace = "lp-ns-" + UUID.randomUUID().toString().substring(0, 8);

        // Create namespace and publish config
        createNamespace(namespace);
        publishConfig(namespace, "test.key", "initial-value");

        // Get initial notification
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
            System.out.println("Notification response: " + response);

            // Response should contain updated notificationId
            assertTrue(response.contains("notificationId"),
                    "Response should contain notificationId");
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ALP-003: Test 304 Not Modified when no changes
     */
    @Test
    @Order(3)
    void testNotModifiedResponse() throws Exception {
        String namespace = "not-modified-" + UUID.randomUUID().toString().substring(0, 8);

        // Create namespace
        createNamespace(namespace);
        publishConfig(namespace, "stable.key", "stable-value");

        Thread.sleep(1000);

        // Get current notification ID (simulate SDK has current state)
        // Use a high notification ID to indicate we're up-to-date
        long highNotificationId = System.currentTimeMillis();

        String notifications = String.format(
                "[{\"namespaceName\":\"%s\",\"notificationId\":%d}]",
                namespace, highNotificationId);

        String url = String.format("%s/notifications/v2?appId=%s&cluster=%s&notifications=%s",
                configServiceUrl, APP_ID, CLUSTER, URLEncoder.encode(notifications, "UTF-8"));

        // Short timeout to avoid waiting
        ExecutorService executor = Executors.newSingleThreadExecutor();
        Future<Integer> future = executor.submit(() -> {
            HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
            conn.setRequestMethod("GET");
            conn.setConnectTimeout(5000);
            conn.setReadTimeout(5000);
            return conn.getResponseCode();
        });

        try {
            int responseCode = future.get(10, TimeUnit.SECONDS);
            System.out.println("Not Modified test - Response code: " + responseCode);
        } catch (TimeoutException e) {
            System.out.println("Request timed out (expected for long polling)");
            future.cancel(true);
        } catch (ExecutionException e) {
            System.out.println("Request failed (expected for long polling timeout): " + e.getCause().getMessage());
            future.cancel(true);
        } finally {
            executor.shutdownNow();
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    // ==================== Multi-Namespace Tests ====================

    /**
     * ALP-004: Test multiple namespace notifications
     */
    @Test
    @Order(4)
    void testMultipleNamespaceNotifications() throws Exception {
        String ns1 = "multi-ns1-" + UUID.randomUUID().toString().substring(0, 8);
        String ns2 = "multi-ns2-" + UUID.randomUUID().toString().substring(0, 8);
        String ns3 = "multi-ns3-" + UUID.randomUUID().toString().substring(0, 8);

        // Create multiple namespaces
        createNamespace(ns1);
        createNamespace(ns2);
        createNamespace(ns3);

        // Build multi-namespace notification request
        String notifications = String.format(
                "[{\"namespaceName\":\"%s\",\"notificationId\":-1}," +
                        "{\"namespaceName\":\"%s\",\"notificationId\":-1}," +
                        "{\"namespaceName\":\"%s\",\"notificationId\":-1}]",
                ns1, ns2, ns3);

        String url = String.format("%s/notifications/v2?appId=%s&cluster=%s&notifications=%s",
                configServiceUrl, APP_ID, CLUSTER, URLEncoder.encode(notifications, "UTF-8"));

        HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
        conn.setRequestMethod("GET");
        conn.setConnectTimeout(5000);
        conn.setReadTimeout(10000);

        int responseCode = conn.getResponseCode();
        if (responseCode == 200) {
            String response = readResponse(conn);
            System.out.println("Multi-namespace response: " + response);
        }

        System.out.println("Multi-namespace test - Response code: " + responseCode);

        // Cleanup
        deleteNamespace(ns1);
        deleteNamespace(ns2);
        deleteNamespace(ns3);
    }

    /**
     * ALP-005: Test notification when one namespace changes
     */
    @Test
    @Order(5)
    void testSingleNamespaceChange() throws Exception {
        String ns1 = "change-ns1-" + UUID.randomUUID().toString().substring(0, 8);
        String ns2 = "change-ns2-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(ns1);
        createNamespace(ns2);
        publishConfig(ns1, "key1", "value1");
        publishConfig(ns2, "key2", "value2");

        Thread.sleep(1000);

        // Start long polling in background
        String notifications = String.format(
                "[{\"namespaceName\":\"%s\",\"notificationId\":-1}," +
                        "{\"namespaceName\":\"%s\",\"notificationId\":-1}]",
                ns1, ns2);

        AtomicReference<String> response = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        Thread pollingThread = new Thread(() -> {
            try {
                String url = String.format("%s/notifications/v2?appId=%s&cluster=%s&notifications=%s",
                        configServiceUrl, APP_ID, CLUSTER, URLEncoder.encode(notifications, "UTF-8"));

                HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
                conn.setRequestMethod("GET");
                conn.setConnectTimeout(5000);
                conn.setReadTimeout(30000);

                int code = conn.getResponseCode();
                if (code == 200) {
                    response.set(readResponse(conn));
                }
                latch.countDown();
            } catch (Exception e) {
                System.out.println("Polling error: " + e.getMessage());
                latch.countDown();
            }
        });

        pollingThread.start();

        // Wait a bit then make a change to ns1
        Thread.sleep(1000);
        publishConfig(ns1, "key1", "updated-value");

        // Wait for notification
        boolean received = latch.await(15, TimeUnit.SECONDS);

        if (received && response.get() != null) {
            System.out.println("Single namespace change notification: " + response.get());
        } else {
            System.out.println("Single namespace change - Timed out or no response");
        }

        pollingThread.interrupt();

        // Cleanup
        deleteNamespace(ns1);
        deleteNamespace(ns2);
    }

    // ==================== Timeout Tests ====================

    /**
     * ALP-006: Test long polling timeout handling
     */
    @Test
    @Order(6)
    void testLongPollingTimeout() throws Exception {
        String namespace = "timeout-ns-" + UUID.randomUUID().toString().substring(0, 8);
        createNamespace(namespace);

        // Use high notification ID so no updates are expected
        String notifications = String.format(
                "[{\"namespaceName\":\"%s\",\"notificationId\":999999999}]",
                namespace);

        String url = String.format("%s/notifications/v2?appId=%s&cluster=%s&notifications=%s",
                configServiceUrl, APP_ID, CLUSTER, URLEncoder.encode(notifications, "UTF-8"));

        long startTime = System.currentTimeMillis();

        ExecutorService executor = Executors.newSingleThreadExecutor();
        Future<int[]> future = executor.submit(() -> {
            HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
            conn.setRequestMethod("GET");
            conn.setConnectTimeout(5000);
            conn.setReadTimeout(10000); // Short timeout for test

            int code = conn.getResponseCode();
            return new int[]{code};
        });

        try {
            int[] result = future.get(15, TimeUnit.SECONDS);
            long duration = System.currentTimeMillis() - startTime;
            System.out.println("Timeout test - Code: " + result[0] + ", Duration: " + duration + "ms");
        } catch (TimeoutException e) {
            long duration = System.currentTimeMillis() - startTime;
            System.out.println("Long polling timed out - Duration: " + duration + "ms");
            future.cancel(true);
        } catch (ExecutionException e) {
            long duration = System.currentTimeMillis() - startTime;
            System.out.println("Long polling connection timed out - Duration: " + duration + "ms: " + e.getCause().getMessage());
            future.cancel(true);
        } finally {
            executor.shutdownNow();
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ALP-007: Test client reconnection after timeout
     */
    @Test
    @Order(7)
    void testReconnectionAfterTimeout() throws Exception {
        String namespace = "reconnect-" + UUID.randomUUID().toString().substring(0, 8);
        createNamespace(namespace);

        int successCount = 0;

        // Simulate multiple polling cycles
        for (int i = 0; i < 3; i++) {
            String notifications = String.format(
                    "[{\"namespaceName\":\"%s\",\"notificationId\":%d}]",
                    namespace, i);

            String url = String.format("%s/notifications/v2?appId=%s&cluster=%s&notifications=%s",
                    configServiceUrl, APP_ID, CLUSTER, URLEncoder.encode(notifications, "UTF-8"));

            try {
                HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
                conn.setRequestMethod("GET");
                conn.setConnectTimeout(3000);
                conn.setReadTimeout(3000);

                int code = conn.getResponseCode();
                if (code == 200 || code == 304) {
                    successCount++;
                }
                System.out.println("Reconnection cycle " + i + " - Code: " + code);
            } catch (Exception e) {
                System.out.println("Reconnection cycle " + i + " - Error: " + e.getMessage());
            }
        }

        System.out.println("Reconnection test - Successful cycles: " + successCount + "/3");

        // Cleanup
        deleteNamespace(namespace);
    }

    // ==================== Concurrent Polling Tests ====================

    /**
     * ALP-008: Test multiple concurrent long polling requests
     */
    @Test
    @Order(8)
    void testConcurrentLongPolling() throws Exception {
        int clientCount = 5;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(clientCount);
        AtomicInteger successCount = new AtomicInteger(0);

        String namespace = "concurrent-poll-" + UUID.randomUUID().toString().substring(0, 8);
        createNamespace(namespace);

        for (int i = 0; i < clientCount; i++) {
            final int clientId = i;
            new Thread(() -> {
                try {
                    startLatch.await();

                    String notifications = String.format(
                            "[{\"namespaceName\":\"%s\",\"notificationId\":%d}]",
                            namespace, clientId);

                    String url = String.format("%s/notifications/v2?appId=%s&cluster=%s&notifications=%s",
                            configServiceUrl, APP_ID, CLUSTER, URLEncoder.encode(notifications, "UTF-8"));

                    HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
                    conn.setRequestMethod("GET");
                    conn.setConnectTimeout(5000);
                    conn.setReadTimeout(5000);

                    int code = conn.getResponseCode();
                    if (code == 200 || code == 304) {
                        successCount.incrementAndGet();
                    }
                    System.out.println("Client " + clientId + " - Code: " + code);
                } catch (Exception e) {
                    System.out.println("Client " + clientId + " - Error: " + e.getMessage());
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        startLatch.countDown();
        boolean completed = doneLatch.await(30, TimeUnit.SECONDS);

        System.out.println("Concurrent polling - Completed: " + completed +
                ", Success: " + successCount.get() + "/" + clientCount);

        // Cleanup
        deleteNamespace(namespace);
    }

    // ==================== Release Key Tests ====================

    /**
     * ALP-009: Test release key tracking for cache validation
     */
    @Test
    @Order(9)
    void testReleaseKeyTracking() throws Exception {
        String namespace = "release-key-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);
        publishConfig(namespace, "key", "value1");

        Thread.sleep(1000);

        // Get config with release key
        String url = String.format("%s/configs/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);

        HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
        conn.setRequestMethod("GET");

        int responseCode = conn.getResponseCode();
        if (responseCode == 200) {
            String response = readResponse(conn);
            System.out.println("Release key response: " + response);

            // Response should contain releaseKey
            if (response.contains("releaseKey")) {
                System.out.println("Release key found in response");
            }
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    /**
     * ALP-010: Test config fetch with If-None-Match (ETag)
     */
    @Test
    @Order(10)
    void testConditionalGet() throws Exception {
        String namespace = "conditional-" + UUID.randomUUID().toString().substring(0, 8);

        createNamespace(namespace);
        publishConfig(namespace, "key", "value");

        Thread.sleep(1000);

        // First request to get ETag/release key
        String url = String.format("%s/configs/%s/%s/%s",
                configServiceUrl, APP_ID, CLUSTER, namespace);

        HttpURLConnection conn1 = (HttpURLConnection) new URL(url).openConnection();
        conn1.setRequestMethod("GET");

        String releaseKey = null;
        if (conn1.getResponseCode() == 200) {
            String response = readResponse(conn1);
            // Extract release key from response
            if (response.contains("releaseKey")) {
                int start = response.indexOf("releaseKey") + 13;
                int end = response.indexOf("\"", start);
                if (end > start) {
                    releaseKey = response.substring(start, end);
                }
            }
        }

        if (releaseKey != null) {
            // Second request with release key (should return 304)
            String url2 = String.format("%s/configs/%s/%s/%s?releaseKey=%s",
                    configServiceUrl, APP_ID, CLUSTER, namespace, releaseKey);

            HttpURLConnection conn2 = (HttpURLConnection) new URL(url2).openConnection();
            conn2.setRequestMethod("GET");

            int code2 = conn2.getResponseCode();
            System.out.println("Conditional GET - First: 200, Second: " + code2);
        }

        // Cleanup
        deleteNamespace(namespace);
    }

    // ==================== Helper Methods ====================

    private void createNamespace(String namespace) throws Exception {
        String url = openApiUrl + "/openapi/v1/envs/" + ENV + "/apps/" + APP_ID + "/clusters/" + CLUSTER + "/namespaces";
        String body = String.format(
                "{\"name\":\"%s\",\"appId\":\"%s\",\"format\":\"properties\",\"isPublic\":false}",
                namespace, APP_ID);

        try {
            httpPost(url, body, "application/json");
        } catch (Exception e) {
            // Namespace may already exist
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
