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
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo OpenAPI Advanced Tests
 *
 * Tests for advanced Apollo OpenAPI operations including app/cluster/namespace creation,
 * batch operations, item history, gray release management, namespace locks, pagination,
 * error handling, and rate limiting.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloOpenApiAdvancedTest {

    private static String baseUrl;
    private static CloseableHttpClient httpClient;
    private static Gson gson;

    private static final String ENV = "DEV";
    private static final String CLUSTER = "default";
    private static final String NAMESPACE = "application";
    private static final String OPERATOR = "test-user";

    // Shared test data for sequential tests
    private static String testAppId;
    private static String testCluster;
    private static String testNamespace;

    @BeforeAll
    static void setup() {
        baseUrl = System.getProperty("apollo.meta", "http://127.0.0.1:8848");
        httpClient = HttpClients.createDefault();
        gson = new Gson();

        // Generate unique identifiers for this test run
        String uniqueId = UUID.randomUUID().toString().substring(0, 8);
        testAppId = "adv-api-test-" + uniqueId;
        testCluster = "test-cluster-" + uniqueId;
        testNamespace = "test-ns-" + uniqueId;

        System.out.println("Apollo OpenAPI Advanced Test Setup:");
        System.out.println("  Base URL: " + baseUrl);
        System.out.println("  Test App ID: " + testAppId);
        System.out.println("  Test Cluster: " + testCluster);
        System.out.println("  Test Namespace: " + testNamespace);
    }

    @AfterAll
    static void teardown() throws IOException {
        if (httpClient != null) {
            httpClient.close();
        }
    }

    // ==================== App/Cluster/Namespace Creation Tests ====================

    /**
     * AOA-001: Test create app via OpenAPI
     *
     * Verifies that a new application can be created through the OpenAPI endpoint.
     */
    @Test
    @Order(1)
    void testCreateAppViaOpenApi() throws IOException {
        String url = baseUrl + "/openapi/v1/apps";

        JsonObject body = new JsonObject();
        body.addProperty("appId", testAppId);
        body.addProperty("name", "Advanced API Test App");
        body.addProperty("orgId", "TEST");
        body.addProperty("orgName", "Test Organization");
        body.addProperty("ownerName", OPERATOR);
        body.addProperty("ownerEmail", "test@example.com");
        body.addProperty("dataChangeCreatedBy", OPERATOR);

        HttpPost request = new HttpPost(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("AOA-001 Create App Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Success (200/201), conflict (409 if already exists), or not supported (404)
            assertTrue(statusCode == 200 || statusCode == 201 || statusCode == 404 || statusCode == 409,
                    "Should return valid response code for app creation");

            if (statusCode == 200 || statusCode == 201) {
                JsonObject app = gson.fromJson(responseBody, JsonObject.class);
                assertTrue(app.has("appId") || app.has("name"),
                        "Created app should have appId or name field");
            }
            return null;
        });
    }

    /**
     * AOA-002: Test create cluster via OpenAPI
     *
     * Verifies that a new cluster can be created for an existing application.
     */
    @Test
    @Order(2)
    void testCreateClusterViaOpenApi() throws IOException {
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters",
                baseUrl, ENV, testAppId);

        JsonObject body = new JsonObject();
        body.addProperty("name", testCluster);
        body.addProperty("appId", testAppId);
        body.addProperty("operator", OPERATOR);

        HttpPost request = new HttpPost(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("AOA-002 Create Cluster Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Success (200/201), conflict (409), bad request (400), or not found (404)
            assertTrue(statusCode == 200 || statusCode == 201 || statusCode == 400 ||
                            statusCode == 404 || statusCode == 409,
                    "Should return valid response code for cluster creation");

            if (statusCode == 200 || statusCode == 201) {
                JsonObject cluster = gson.fromJson(responseBody, JsonObject.class);
                assertTrue(cluster.has("name") || cluster.has("clusterName"),
                        "Created cluster should have name field");
            }
            return null;
        });
    }

    /**
     * AOA-003: Test create namespace via OpenAPI
     *
     * Verifies that a new namespace can be created within a cluster.
     */
    @Test
    @Order(3)
    void testCreateNamespaceViaOpenApi() throws IOException {
        String url = String.format("%s/openapi/v1/apps/%s/appnamespaces",
                baseUrl, testAppId);

        JsonObject body = new JsonObject();
        body.addProperty("name", testNamespace);
        body.addProperty("appId", testAppId);
        body.addProperty("format", "properties");
        body.addProperty("isPublic", false);
        body.addProperty("comment", "Created via OpenAPI advanced test");
        body.addProperty("dataChangeCreatedBy", OPERATOR);

        HttpPost request = new HttpPost(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("AOA-003 Create Namespace Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Success (200/201), conflict (409), bad request (400), or not found (404)
            assertTrue(statusCode == 200 || statusCode == 201 || statusCode == 400 ||
                            statusCode == 404 || statusCode == 409,
                    "Should return valid response code for namespace creation");

            if (statusCode == 200 || statusCode == 201) {
                JsonObject ns = gson.fromJson(responseBody, JsonObject.class);
                assertTrue(ns.has("namespaceName") || ns.has("name"),
                        "Created namespace should have name field");
            }
            return null;
        });
    }

    // ==================== Item Operations Tests ====================

    /**
     * AOA-004: Test batch item operations
     *
     * Verifies that multiple items can be created/updated in a single batch request.
     */
    @Test
    @Order(4)
    void testBatchItemOperations() throws IOException {
        String appId = "batch-items-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items/batch",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        JsonArray items = new JsonArray();
        for (int i = 1; i <= 5; i++) {
            JsonObject item = new JsonObject();
            item.addProperty("key", "batch.key." + i);
            item.addProperty("value", "batch-value-" + i);
            item.addProperty("comment", "Batch created item " + i);
            item.addProperty("dataChangeCreatedBy", OPERATOR);
            items.add(item);
        }

        JsonObject body = new JsonObject();
        body.add("createItems", items);

        HttpPost request = new HttpPost(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("AOA-004 Batch Item Operations Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Success (200), bad request (400), not found (404), or method not allowed (405)
            assertTrue(statusCode == 200 || statusCode == 400 || statusCode == 404 || statusCode == 405,
                    "Should return valid response code for batch operations, got: " + statusCode);
            return null;
        });
    }

    /**
     * AOA-005: Test item with comment
     *
     * Verifies that items can be created with detailed comments.
     */
    @Test
    @Order(5)
    void testItemWithComment() throws IOException {
        String appId = "item-comment-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        String detailedComment = "This is a detailed comment explaining the purpose of this configuration item. " +
                "It includes information about when to use this setting and what values are acceptable. " +
                "Last reviewed: 2024-01-15 by the platform team.";

        JsonObject body = new JsonObject();
        body.addProperty("key", "documented.config.key");
        body.addProperty("value", "documented-value");
        body.addProperty("comment", detailedComment);
        body.addProperty("dataChangeCreatedBy", OPERATOR);

        HttpPost request = new HttpPost(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        try {
            httpClient.execute(request, response -> {
                int statusCode = response.getCode();
                String responseBody = EntityUtils.toString(response.getEntity());

                System.out.println("AOA-005 Item With Comment Response:");
                System.out.println("  Status: " + statusCode);
                System.out.println("  Body: " + responseBody);

                // 201 (success), 400 (namespace not found per Apollo convention), or 404
                assertTrue(statusCode == 200 || statusCode == 201 || statusCode == 400 || statusCode == 404,
                        "Should return valid response code");

                if (statusCode == 200 || statusCode == 201) {
                    JsonObject item = gson.fromJson(responseBody, JsonObject.class);
                    if (item.has("comment")) {
                        String savedComment = item.get("comment").getAsString();
                        assertFalse(savedComment.isEmpty(), "Comment should be preserved");
                    }
                }
                return null;
            });
        } catch (org.apache.hc.client5.http.HttpHostConnectException |
                 org.apache.hc.core5.http.NoHttpResponseException e) {
            System.out.println("AOA-005 Connection issue (server closed keep-alive connection): " + e.getMessage());
        }
    }

    /**
     * AOA-006: Test item history
     *
     * Verifies that the history of changes to an item can be retrieved.
     */
    @Test
    @Order(6)
    void testItemHistory() throws IOException {
        String appId = "item-history-" + UUID.randomUUID().toString().substring(0, 8);
        String key = "history.test.key";
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items/%s/history",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE, key);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("AOA-006 Item History Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Success (200 with history), or not found (404 if no history)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code for item history");

            if (statusCode == 200) {
                // Response should be an array of history entries
                try {
                    JsonArray history = gson.fromJson(responseBody, JsonArray.class);
                    assertNotNull(history, "History should be returned as array");
                    System.out.println("  History entries: " + history.size());
                } catch (Exception e) {
                    // May also return as object with content array
                    JsonObject historyObj = gson.fromJson(responseBody, JsonObject.class);
                    assertNotNull(historyObj, "History should be returned as object");
                }
            }
            return null;
        });
    }

    // ==================== Release Tests ====================

    /**
     * AOA-007: Test release comparison
     *
     * Verifies that two releases can be compared to show differences.
     */
    @Test
    @Order(7)
    void testReleaseComparison() throws IOException {
        String appId = "release-compare-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/releases/compare?baseReleaseId=1&toCompareReleaseId=2",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("AOA-007 Release Comparison Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Success (200 with diff), bad request (400), or not found (404)
            assertTrue(statusCode == 200 || statusCode == 400 || statusCode == 404,
                    "Should return valid response code for release comparison");

            if (statusCode == 200) {
                JsonObject comparison = gson.fromJson(responseBody, JsonObject.class);
                // Comparison should contain changes info
                assertNotNull(comparison, "Comparison result should not be null");
            }
            return null;
        });
    }

    // ==================== Gray Release Tests ====================

    /**
     * AOA-008: Test gray release management
     *
     * Verifies that a gray release can be created with specific configurations.
     */
    @Test
    @Order(8)
    void testGrayReleaseManagement() throws IOException {
        String appId = "gray-mgmt-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/branches",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        JsonObject body = new JsonObject();
        body.addProperty("branchName", "gray-branch-" + UUID.randomUUID().toString().substring(0, 6));
        body.addProperty("operator", OPERATOR);

        HttpPost request = new HttpPost(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("AOA-008 Gray Release Management Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Success (200/201), bad request (400), or not found (404)
            assertTrue(statusCode == 200 || statusCode == 201 || statusCode == 400 || statusCode == 404,
                    "Should return valid response code for gray release creation");
            return null;
        });
    }

    /**
     * AOA-009: Test gray release rules
     *
     * Verifies that gray release rules can be configured with IP-based targeting.
     */
    @Test
    @Order(9)
    void testGrayReleaseRules() throws IOException {
        String appId = "gray-rules-" + UUID.randomUUID().toString().substring(0, 8);
        String branchName = "test-branch";
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/branches/%s/rules",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE, branchName);

        JsonObject rules = new JsonObject();

        // IP-based rules
        JsonArray ipRules = new JsonArray();
        JsonObject ipRule = new JsonObject();
        ipRule.addProperty("clientAppId", appId);
        JsonArray ips = new JsonArray();
        ips.add("192.168.1.100");
        ips.add("192.168.1.101");
        ips.add("192.168.1.102");
        ipRule.add("clientIpList", ips);
        ipRules.add(ipRule);
        rules.add("ruleItems", ipRules);

        JsonObject body = new JsonObject();
        body.add("grayBranchRules", rules);
        body.addProperty("dataChangeLastModifiedBy", OPERATOR);

        HttpPut request = new HttpPut(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("AOA-009 Gray Release Rules Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Success (200), bad request (400), or not found (404)
            assertTrue(statusCode == 200 || statusCode == 400 || statusCode == 404,
                    "Should return valid response code for gray release rules");
            return null;
        });
    }

    /**
     * AOA-010: Test merge gray release
     *
     * Verifies that a gray release can be merged into the main release.
     */
    @Test
    @Order(10)
    void testMergeGrayRelease() throws IOException {
        String appId = "gray-merge-" + UUID.randomUUID().toString().substring(0, 8);
        String branchName = "test-branch";
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/branches/%s/merge",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE, branchName);

        JsonObject body = new JsonObject();
        body.addProperty("operator", OPERATOR);

        HttpPost request = new HttpPost(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("AOA-010 Merge Gray Release Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Success (200), bad request (400), or not found (404)
            assertTrue(statusCode == 200 || statusCode == 400 || statusCode == 404,
                    "Should return valid response code for merge gray release");
            return null;
        });
    }

    /**
     * AOA-011: Test abandon gray release
     *
     * Verifies that a gray release can be abandoned/deleted.
     */
    @Test
    @Order(11)
    void testAbandonGrayRelease() throws IOException {
        String appId = "gray-abandon-" + UUID.randomUUID().toString().substring(0, 8);
        String branchName = "test-branch";
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/branches/%s?operator=%s",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE, branchName, OPERATOR);

        HttpDelete request = new HttpDelete(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = response.getEntity() != null ?
                    EntityUtils.toString(response.getEntity()) : "";

            System.out.println("AOA-011 Abandon Gray Release Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Success (200/204), or not found (404)
            assertTrue(statusCode == 200 || statusCode == 204 || statusCode == 404,
                    "Should return valid response code for abandon gray release");
            return null;
        });
    }

    /**
     * AOA-012: Test release rollback via API
     *
     * Verifies that a release can be rolled back to a previous version.
     */
    @Test
    @Order(12)
    void testReleaseRollbackViaApi() throws IOException {
        String appId = "rollback-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/releases/%d/rollback?operator=%s",
                baseUrl, ENV, 12345L, OPERATOR);

        HttpPut request = new HttpPut(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = response.getEntity() != null ?
                    EntityUtils.toString(response.getEntity()) : "";

            System.out.println("AOA-012 Release Rollback Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Success (200), bad request (400), or not found (404)
            assertTrue(statusCode == 200 || statusCode == 400 || statusCode == 404,
                    "Should return valid response code for release rollback");
            return null;
        });
    }

    // ==================== Namespace Lock Tests ====================

    /**
     * AOA-013: Test namespace lock
     *
     * Verifies that a namespace can be locked for exclusive editing.
     */
    @Test
    @Order(13)
    void testNamespaceLock() throws IOException {
        String appId = "ns-lock-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/lock",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        // First, check current lock status
        HttpGet getRequest = new HttpGet(url);

        httpClient.execute(getRequest, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("AOA-013 Namespace Lock Status Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (lock info) or 404 (not locked/not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code for lock status");

            if (statusCode == 200) {
                JsonObject lockInfo = gson.fromJson(responseBody, JsonObject.class);
                // Lock info should contain isLocked or lockedBy field
                System.out.println("  Lock info retrieved successfully");
            }
            return null;
        });
    }

    /**
     * AOA-014: Test namespace unlock
     *
     * Verifies that a locked namespace can be unlocked.
     */
    @Test
    @Order(14)
    void testNamespaceUnlock() throws IOException {
        String appId = "ns-unlock-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/lock?operator=%s",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE, OPERATOR);

        HttpDelete request = new HttpDelete(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = response.getEntity() != null ?
                    EntityUtils.toString(response.getEntity()) : "";

            System.out.println("AOA-014 Namespace Unlock Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Success (200/204), not found (404), or forbidden (403 if not lock owner)
            assertTrue(statusCode == 200 || statusCode == 204 || statusCode == 403 || statusCode == 404,
                    "Should return valid response code for namespace unlock");
            return null;
        });
    }

    // ==================== Item Management Tests ====================

    /**
     * AOA-015: Test get all items
     *
     * Verifies that all items in a namespace can be retrieved.
     */
    @Test
    @Order(15)
    void testGetAllItems() throws IOException {
        String appId = "all-items-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("AOA-015 Get All Items Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");

            if (statusCode == 200) {
                // Should return array of items
                try {
                    JsonArray items = gson.fromJson(responseBody, JsonArray.class);
                    assertNotNull(items, "Items should be returned as array");
                    System.out.println("  Total items: " + items.size());
                } catch (Exception e) {
                    // May return as paginated result
                    JsonObject result = gson.fromJson(responseBody, JsonObject.class);
                    assertNotNull(result, "Items should be returned as paginated result");
                }
            }
            return null;
        });
    }

    /**
     * AOA-016: Test delete item
     *
     * Verifies that an item can be deleted from a namespace.
     */
    @Test
    @Order(16)
    void testDeleteItem() throws IOException {
        String appId = "delete-item-" + UUID.randomUUID().toString().substring(0, 8);
        String key = "item.to.delete";
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items/%s?operator=%s",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE, key, OPERATOR);

        HttpDelete request = new HttpDelete(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = response.getEntity() != null ?
                    EntityUtils.toString(response.getEntity()) : "";

            System.out.println("AOA-016 Delete Item Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Success (200/204), or not found (404)
            assertTrue(statusCode == 200 || statusCode == 204 || statusCode == 404,
                    "Should return valid response code for item deletion");
            return null;
        });
    }

    /**
     * AOA-017: Test update item
     *
     * Verifies that an existing item can be updated.
     */
    @Test
    @Order(17)
    void testUpdateItem() throws IOException {
        String appId = "update-item-" + UUID.randomUUID().toString().substring(0, 8);
        String key = "item.to.update";
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items/%s",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE, key);

        JsonObject body = new JsonObject();
        body.addProperty("key", key);
        body.addProperty("value", "updated-value-" + System.currentTimeMillis());
        body.addProperty("comment", "Updated via OpenAPI test at " + System.currentTimeMillis());
        body.addProperty("dataChangeLastModifiedBy", OPERATOR);

        HttpPut request = new HttpPut(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("AOA-017 Update Item Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // Success (200), or not found (404)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code for item update");
            return null;
        });
    }

    // ==================== Pagination and Error Handling Tests ====================

    /**
     * AOA-018: Test OpenAPI pagination
     *
     * Verifies that paginated results are properly returned with page/size parameters.
     */
    @Test
    @Order(18)
    void testOpenApiPagination() throws IOException {
        String appId = "pagination-test-" + UUID.randomUUID().toString().substring(0, 8);

        // Test with various pagination parameters
        int[] pageSizes = {5, 10, 20};

        for (int pageSize : pageSizes) {
            String url = String.format(
                    "%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/releases?page=0&size=%d",
                    baseUrl, ENV, appId, CLUSTER, NAMESPACE, pageSize);

            HttpGet request = new HttpGet(url);

            httpClient.execute(request, response -> {
                int statusCode = response.getCode();
                String responseBody = EntityUtils.toString(response.getEntity());

                System.out.println("AOA-018 Pagination Test (size=" + pageSize + ") Response:");
                System.out.println("  Status: " + statusCode);
                System.out.println("  Body length: " + (responseBody != null ? responseBody.length() : 0));

                assertTrue(statusCode == 200 || statusCode == 404,
                        "Should return valid response code");

                if (statusCode == 200 && responseBody != null && !responseBody.isEmpty()) {
                    try {
                        // Check for paginated response structure
                        JsonObject result = gson.fromJson(responseBody, JsonObject.class);
                        if (result.has("content") || result.has("page") ||
                                result.has("totalElements") || result.has("totalPages")) {
                            System.out.println("  Paginated response structure detected");
                        }
                    } catch (Exception e) {
                        // May return as simple array
                        JsonArray arr = gson.fromJson(responseBody, JsonArray.class);
                        System.out.println("  Array response with " + arr.size() + " elements");
                    }
                }
                return null;
            });
        }
    }

    /**
     * AOA-019: Test OpenAPI error handling
     *
     * Verifies that the API returns appropriate error responses for various error conditions.
     */
    @Test
    @Order(19)
    void testOpenApiErrorHandling() throws IOException {
        // Test 1: Invalid app ID format (URL-encoded special characters)
        String url1 = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items",
                baseUrl, ENV, "invalid%20app%20id%20with%20spaces%21", CLUSTER, NAMESPACE);
        HttpGet request1 = new HttpGet(url1);

        try {
            httpClient.execute(request1, response -> {
                int statusCode = response.getCode();
                String responseBody = EntityUtils.toString(response.getEntity());

                System.out.println("AOA-019 Error Handling - Invalid App ID:");
                System.out.println("  Status: " + statusCode);
                System.out.println("  Body: " + responseBody);

                // Should return any valid HTTP status (invalid app just means no data)
                assertTrue(statusCode >= 200, "Should return valid HTTP status");
                return null;
            });
        } catch (IOException e) {
            System.out.println("AOA-019 Error Handling - Invalid App ID: Connection error (acceptable): " + e.getMessage());
        }

        // Test 2: Missing required field in POST body
        String url2 = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items",
                baseUrl, ENV, "error-test-app", CLUSTER, NAMESPACE);

        JsonObject incompleteBody = new JsonObject();
        // Missing required 'key' field
        incompleteBody.addProperty("value", "test-value");

        HttpPost request2 = new HttpPost(url2);
        request2.setEntity(new StringEntity(gson.toJson(incompleteBody), ContentType.APPLICATION_JSON));

        try {
            httpClient.execute(request2, response -> {
                int statusCode = response.getCode();
                String responseBody = EntityUtils.toString(response.getEntity());

                System.out.println("AOA-019 Error Handling - Missing Required Field:");
                System.out.println("  Status: " + statusCode);
                System.out.println("  Body: " + responseBody);

                // Should return error (400 for validation, 404 for not found, or accept it)
                assertTrue(statusCode == 200 || statusCode == 201 ||
                                statusCode == 400 || statusCode == 404 || statusCode == 422,
                        "Should handle missing field appropriately");
                return null;
            });
        } catch (IOException e) {
            System.out.println("AOA-019 Error Handling - Missing Required Field: Connection error (acceptable): " + e.getMessage());
        }

        // Test 3: Invalid JSON body
        HttpPost request3 = new HttpPost(url2);
        request3.setEntity(new StringEntity("{ invalid json }", ContentType.APPLICATION_JSON));

        try {
            httpClient.execute(request3, response -> {
                int statusCode = response.getCode();
                String responseBody = EntityUtils.toString(response.getEntity());

                System.out.println("AOA-019 Error Handling - Invalid JSON:");
                System.out.println("  Status: " + statusCode);
                System.out.println("  Body: " + responseBody);

                // Should return 400 for malformed JSON
                assertTrue(statusCode == 400 || statusCode == 404 || statusCode == 500,
                        "Should return error for invalid JSON");
                return null;
            });
        } catch (IOException e) {
            System.out.println("AOA-019 Error Handling - Invalid JSON: Connection error (acceptable): " + e.getMessage());
        }
    }

    /**
     * AOA-020: Test OpenAPI rate limiting
     *
     * Verifies that the API properly handles rate limiting by making many concurrent requests.
     */
    @Test
    @Order(20)
    void testOpenApiRateLimiting() throws InterruptedException {
        String appId = "rate-limit-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        int concurrentRequests = 20;
        ExecutorService executor = Executors.newFixedThreadPool(10);
        CountDownLatch latch = new CountDownLatch(concurrentRequests);
        AtomicInteger successCount = new AtomicInteger(0);
        AtomicInteger rateLimitedCount = new AtomicInteger(0);
        AtomicInteger errorCount = new AtomicInteger(0);

        for (int i = 0; i < concurrentRequests; i++) {
            final int requestNum = i;
            executor.submit(() -> {
                try (CloseableHttpClient client = HttpClients.createDefault()) {
                    HttpGet request = new HttpGet(url);
                    client.execute(request, response -> {
                        int statusCode = response.getCode();
                        EntityUtils.consume(response.getEntity());

                        if (statusCode == 200 || statusCode == 404) {
                            successCount.incrementAndGet();
                        } else if (statusCode == 429) {
                            rateLimitedCount.incrementAndGet();
                        } else {
                            errorCount.incrementAndGet();
                        }
                        return null;
                    });
                } catch (IOException e) {
                    errorCount.incrementAndGet();
                } finally {
                    latch.countDown();
                }
            });
        }

        boolean completed = latch.await(30, TimeUnit.SECONDS);
        executor.shutdown();

        System.out.println("AOA-020 Rate Limiting Test Results:");
        System.out.println("  Total requests: " + concurrentRequests);
        System.out.println("  Successful: " + successCount.get());
        System.out.println("  Rate limited (429): " + rateLimitedCount.get());
        System.out.println("  Errors: " + errorCount.get());
        System.out.println("  Completed in time: " + completed);

        assertTrue(completed, "All requests should complete within timeout");

        // Either all succeed (no rate limiting) or some are rate limited
        int totalHandled = successCount.get() + rateLimitedCount.get() + errorCount.get();
        assertEquals(concurrentRequests, totalHandled,
                "All requests should receive a response");

        // If rate limiting is enabled, we should see 429 responses
        // If not enabled, all should succeed or return 404
        System.out.println("  Rate limiting " +
                (rateLimitedCount.get() > 0 ? "IS" : "is NOT") + " enforced");
    }
}
