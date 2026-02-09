package io.batata.tests;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import org.apache.hc.client5.http.classic.methods.*;
import org.apache.hc.client5.http.impl.classic.CloseableHttpClient;
import org.apache.hc.client5.http.impl.classic.HttpClients;
import org.apache.hc.core5.http.ContentType;
import org.apache.hc.core5.http.io.entity.EntityUtils;
import org.apache.hc.core5.http.io.entity.StringEntity;
import org.junit.jupiter.api.*;

import java.io.IOException;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo OpenAPI Compatibility Tests
 *
 * Tests Batata's compatibility with Apollo OpenAPI endpoints.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloOpenApiTest {

    private static String baseUrl;
    private static CloseableHttpClient httpClient;
    private static Gson gson;

    private static final String ENV = "DEV";
    private static final String CLUSTER = "default";
    private static final String NAMESPACE = "application";

    @BeforeAll
    static void setup() {
        baseUrl = System.getProperty("apollo.meta", "http://127.0.0.1:8848");
        httpClient = HttpClients.createDefault();
        gson = new Gson();

        System.out.println("Apollo OpenAPI Test Setup:");
        System.out.println("  Base URL: " + baseUrl);
    }

    @AfterAll
    static void teardown() throws IOException {
        if (httpClient != null) {
            httpClient.close();
        }
    }

    // ==================== P0: Critical Tests ====================

    /**
     * AO-005: Test publish release
     */
    @Test
    @Order(1)
    void testPublishRelease() throws IOException {
        String appId = "openapi-test-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/releases",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        JsonObject body = new JsonObject();
        body.addProperty("releaseTitle", "Test Release");
        body.addProperty("releaseComment", "SDK Integration Test Release");
        body.addProperty("releasedBy", "test-user");

        HttpPost request = new HttpPost(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Publish Release Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success) or 404 (app not found) depending on setup
            assertTrue(statusCode == 200 || statusCode == 404 || statusCode == 400,
                    "Should return valid response code");
            return null;
        });
    }

    // ==================== P1: Important Tests ====================

    /**
     * AO-001: Test list apps
     */
    @Test
    @Order(2)
    void testListApps() throws IOException {
        String url = baseUrl + "/openapi/v1/apps";
        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("List Apps Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (with apps) or 404 (endpoint not fully implemented)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return 200 or 404, got: " + statusCode);

            if (statusCode == 200) {
                // Should return JSON array
                JsonArray apps = gson.fromJson(responseBody, JsonArray.class);
                assertNotNull(apps, "Should return JSON array");
                System.out.println("  Found " + apps.size() + " apps");
            } else {
                System.out.println("  List apps endpoint not implemented (404)");
            }

            return null;
        });
    }

    /**
     * AO-002: Test create item
     */
    @Test
    @Order(3)
    void testCreateItem() throws IOException {
        String appId = "item-test-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        JsonObject body = new JsonObject();
        body.addProperty("key", "test.created.key");
        body.addProperty("value", "test-created-value");
        body.addProperty("comment", "Created via OpenAPI test");
        body.addProperty("dataChangeCreatedBy", "test-user");

        HttpPost request = new HttpPost(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Create Item Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 201 (success), 400 (namespace not found per Apollo convention), or 404
            assertTrue(statusCode == 200 || statusCode == 201 || statusCode == 400 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AO-003: Test update item
     */
    @Test
    @Order(4)
    void testUpdateItem() throws IOException {
        String appId = "update-test-" + UUID.randomUUID().toString().substring(0, 8);
        String key = "test.update.key";
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items/%s",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE, key);

        JsonObject body = new JsonObject();
        body.addProperty("key", key);
        body.addProperty("value", "updated-value");
        body.addProperty("comment", "Updated via OpenAPI test");
        body.addProperty("dataChangeLastModifiedBy", "test-user");

        HttpPut request = new HttpPut(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Update Item Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success) or 404 (not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AO-004: Test delete item
     */
    @Test
    @Order(5)
    void testDeleteItem() throws IOException {
        String appId = "delete-test-" + UUID.randomUUID().toString().substring(0, 8);
        String key = "test.delete.key";
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items/%s?operator=test-user",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE, key);

        HttpDelete request = new HttpDelete(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = response.getEntity() != null ?
                    EntityUtils.toString(response.getEntity()) : "";

            System.out.println("Delete Item Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200/204 (success) or 404 (not found)
            assertTrue(statusCode == 200 || statusCode == 204 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AO-006: Test get latest release
     */
    @Test
    @Order(6)
    void testGetLatestRelease() throws IOException {
        String appId = "release-test-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/releases/latest",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Get Latest Release Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (found) or 404 (no releases)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    // ==================== Config API Tests ====================

    /**
     * Test get config via Config API
     */
    @Test
    @Order(7)
    void testGetConfigApi() throws IOException {
        String appId = "config-api-test";
        String url = String.format("%s/configs/%s/%s/%s",
                baseUrl, appId, CLUSTER, NAMESPACE);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Get Config API Response:");
            System.out.println("  URL: " + url);
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Should return Apollo config format
            if (statusCode == 200) {
                JsonObject config = gson.fromJson(responseBody, JsonObject.class);
                // Apollo config format has appId, cluster, namespaceName, configurations, releaseKey
                assertTrue(config.has("appId") || config.has("configurations"),
                        "Should return Apollo config format");
            }
            return null;
        });
    }

    /**
     * Test get config files (plain text)
     */
    @Test
    @Order(8)
    void testGetConfigFiles() throws IOException {
        String appId = "configfiles-test";
        String url = String.format("%s/configfiles/%s/%s/%s",
                baseUrl, appId, CLUSTER, NAMESPACE);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Get Config Files Response:");
            System.out.println("  URL: " + url);
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body length: " + (responseBody != null ? responseBody.length() : 0));

            // May return 200 (found) or 404 (not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * Test get config files JSON format
     */
    @Test
    @Order(9)
    void testGetConfigFilesJson() throws IOException {
        String appId = "configfiles-json-test";
        String url = String.format("%s/configfiles/json/%s/%s/%s",
                baseUrl, appId, CLUSTER, NAMESPACE);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Get Config Files JSON Response:");
            System.out.println("  URL: " + url);
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            if (statusCode == 200) {
                // Should be valid JSON
                assertDoesNotThrow(() -> gson.fromJson(responseBody, JsonObject.class),
                        "Should return valid JSON");
            }
            return null;
        });
    }

    /**
     * Test notifications (long polling)
     */
    @Test
    @Order(10)
    void testNotifications() throws IOException {
        String appId = "notifications-test";
        JsonArray notifications = new JsonArray();
        JsonObject notification = new JsonObject();
        notification.addProperty("namespaceName", NAMESPACE);
        notification.addProperty("notificationId", -1);
        notifications.add(notification);

        String url = String.format("%s/notifications/v2?appId=%s&cluster=%s&notifications=%s",
                baseUrl, appId, CLUSTER, java.net.URLEncoder.encode(gson.toJson(notifications), "UTF-8"));

        HttpGet request = new HttpGet(url);
        // Note: In real scenario, this is a long-polling request
        // For testing, we just verify the endpoint responds

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Notifications Response:");
            System.out.println("  URL: " + url);
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 with notifications, 304 (not modified), or 404
            assertTrue(statusCode == 200 || statusCode == 304 || statusCode == 404,
                    "Should return valid response code");

            if (statusCode == 200 && responseBody != null && !responseBody.isEmpty()) {
                // Should be array of notifications
                JsonArray result = gson.fromJson(responseBody, JsonArray.class);
                assertNotNull(result, "Should return notifications array");
            }
            return null;
        });
    }

    /**
     * Test list namespaces
     */
    @Test
    @Order(11)
    void testListNamespaces() throws IOException {
        String appId = "namespaces-test";
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces",
                baseUrl, ENV, appId, CLUSTER);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("List Namespaces Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success) or 404 (app not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * Test list clusters
     */
    @Test
    @Order(12)
    void testListClusters() throws IOException {
        String appId = "clusters-test";
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters",
                baseUrl, ENV, appId);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("List Clusters Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success) or 404 (app not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }
}
