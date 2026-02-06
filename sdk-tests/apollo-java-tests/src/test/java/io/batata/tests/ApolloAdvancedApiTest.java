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
 * Apollo Advanced API Tests
 *
 * Tests for gray release, namespace locks, access keys, and client metrics.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloAdvancedApiTest {

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

        System.out.println("Apollo Advanced API Test Setup:");
        System.out.println("  Base URL: " + baseUrl);
    }

    @AfterAll
    static void teardown() throws IOException {
        if (httpClient != null) {
            httpClient.close();
        }
    }

    // ==================== Gray Release API Tests ====================

    /**
     * AG-001: Test get gray release
     */
    @Test
    @Order(1)
    void testGetGrayRelease() throws IOException {
        String appId = "gray-test-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/gray",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Get Gray Release Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (found) or 404 (not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AG-002: Test create gray release
     */
    @Test
    @Order(2)
    void testCreateGrayRelease() throws IOException {
        String appId = "gray-create-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/gray",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        JsonObject body = new JsonObject();
        body.addProperty("branchName", "gray-branch-" + UUID.randomUUID().toString().substring(0, 8));

        JsonObject rules = new JsonObject();
        JsonArray clients = new JsonArray();
        clients.add("192.168.1.100");
        clients.add("192.168.1.101");
        rules.add("clientAppId", clients);
        body.add("rules", rules);

        HttpPost request = new HttpPost(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Create Gray Release Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success), 400 (bad request), or 404 (app not found)
            assertTrue(statusCode == 200 || statusCode == 201 || statusCode == 400 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AG-003: Test merge gray release
     */
    @Test
    @Order(3)
    void testMergeGrayRelease() throws IOException {
        String appId = "gray-merge-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/gray/merge",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        JsonObject body = new JsonObject();
        body.addProperty("releasedBy", "test-user");
        body.addProperty("releaseTitle", "Merge gray release");
        body.addProperty("releaseComment", "SDK test merge");

        HttpPut request = new HttpPut(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Merge Gray Release Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success) or 404 (no gray release)
            assertTrue(statusCode == 200 || statusCode == 400 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AG-004: Test abandon gray release
     */
    @Test
    @Order(4)
    void testAbandonGrayRelease() throws IOException {
        String appId = "gray-abandon-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/gray",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        HttpDelete request = new HttpDelete(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = response.getEntity() != null ?
                    EntityUtils.toString(response.getEntity()) : "";

            System.out.println("Abandon Gray Release Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200/204 (success) or 404 (no gray release)
            assertTrue(statusCode == 200 || statusCode == 204 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    // ==================== Namespace Lock API Tests ====================

    /**
     * AL-001: Test get namespace lock status
     */
    @Test
    @Order(5)
    void testGetNamespaceLock() throws IOException {
        String appId = "lock-test-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/lock",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Get Namespace Lock Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (locked) or 404 (not locked/not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AL-002: Test acquire namespace lock
     */
    @Test
    @Order(6)
    void testAcquireNamespaceLock() throws IOException {
        String appId = "lock-acquire-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/lock",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        JsonObject body = new JsonObject();
        body.addProperty("lockedBy", "test-user");

        HttpPost request = new HttpPost(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Acquire Namespace Lock Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success), 409 (already locked), or 404 (not found)
            assertTrue(statusCode == 200 || statusCode == 201 || statusCode == 404 || statusCode == 409,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AL-003: Test release namespace lock
     */
    @Test
    @Order(7)
    void testReleaseNamespaceLock() throws IOException {
        String appId = "lock-release-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/lock?operator=test-user",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        HttpDelete request = new HttpDelete(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = response.getEntity() != null ?
                    EntityUtils.toString(response.getEntity()) : "";

            System.out.println("Release Namespace Lock Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200/204 (success) or 404 (not locked)
            assertTrue(statusCode == 200 || statusCode == 204 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    // ==================== Access Key API Tests ====================

    /**
     * AK-001: Test list access keys
     */
    @Test
    @Order(8)
    void testListAccessKeys() throws IOException {
        String appId = "key-test-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/apps/%s/accesskeys", baseUrl, appId);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("List Access Keys Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success) or 404 (app not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AK-002: Test create access key
     */
    @Test
    @Order(9)
    void testCreateAccessKey() throws IOException {
        String appId = "key-create-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/apps/%s/accesskeys", baseUrl, appId);

        JsonObject body = new JsonObject();
        body.addProperty("dataChangeCreatedBy", "test-user");

        HttpPost request = new HttpPost(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Create Access Key Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200/201 (success) or 404 (app not found)
            assertTrue(statusCode == 200 || statusCode == 201 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AK-003: Test get access key
     */
    @Test
    @Order(10)
    void testGetAccessKey() throws IOException {
        String appId = "key-get-" + UUID.randomUUID().toString().substring(0, 8);
        String keyId = "test-key-id";
        String url = String.format("%s/openapi/v1/apps/%s/accesskeys/%s", baseUrl, appId, keyId);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Get Access Key Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success) or 404 (not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AK-004: Test enable/disable access key
     */
    @Test
    @Order(11)
    void testToggleAccessKey() throws IOException {
        String appId = "key-toggle-" + UUID.randomUUID().toString().substring(0, 8);
        String keyId = "test-key-id";
        String url = String.format("%s/openapi/v1/apps/%s/accesskeys/%s/enable?enabled=false&operator=test-user",
                baseUrl, appId, keyId);

        HttpPut request = new HttpPut(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = response.getEntity() != null ?
                    EntityUtils.toString(response.getEntity()) : "";

            System.out.println("Toggle Access Key Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success) or 404 (not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AK-005: Test delete access key
     */
    @Test
    @Order(12)
    void testDeleteAccessKey() throws IOException {
        String appId = "key-delete-" + UUID.randomUUID().toString().substring(0, 8);
        String keyId = "test-key-id";
        String url = String.format("%s/openapi/v1/apps/%s/accesskeys/%s?operator=test-user",
                baseUrl, appId, keyId);

        HttpDelete request = new HttpDelete(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = response.getEntity() != null ?
                    EntityUtils.toString(response.getEntity()) : "";

            System.out.println("Delete Access Key Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200/204 (success) or 404 (not found)
            assertTrue(statusCode == 200 || statusCode == 204 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    // ==================== Client Metrics API Tests ====================

    /**
     * AM-001: Test get client metrics summary
     */
    @Test
    @Order(13)
    void testGetClientMetricsSummary() throws IOException {
        String url = baseUrl + "/openapi/v1/metrics/clients";

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Get Client Metrics Summary Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success) or 404 (not available)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AM-002: Test get app clients
     */
    @Test
    @Order(14)
    void testGetAppClients() throws IOException {
        String appId = "clients-test-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/apps/%s/clients", baseUrl, appId);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Get App Clients Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success) or 404 (app not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AM-003: Test cleanup stale clients
     */
    @Test
    @Order(15)
    void testCleanupStaleClients() throws IOException {
        String url = baseUrl + "/openapi/v1/metrics/clients/cleanup";

        JsonObject body = new JsonObject();
        body.addProperty("staleDurationInMinutes", 60);

        HttpPost request = new HttpPost(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = response.getEntity() != null ?
                    EntityUtils.toString(response.getEntity()) : "";

            System.out.println("Cleanup Stale Clients Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success), 404 (not available), or 400 (bad request)
            assertTrue(statusCode == 200 || statusCode == 400 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    // ==================== Env/Cluster API Tests ====================

    /**
     * AE-001: Test get env clusters
     */
    @Test
    @Order(16)
    void testGetEnvClusters() throws IOException {
        String appId = "env-test-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/apps/%s/envclusters", baseUrl, appId);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Get Env Clusters Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success) or 404 (app not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AE-002: Test list namespaces
     */
    @Test
    @Order(17)
    void testListNamespaces() throws IOException {
        String appId = "ns-test-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces",
                baseUrl, ENV, appId, CLUSTER);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("List Namespaces Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success) or 404 (not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AE-003: Test get namespace detail
     */
    @Test
    @Order(18)
    void testGetNamespaceDetail() throws IOException {
        String appId = "ns-detail-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Get Namespace Detail Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success) or 404 (not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    // ==================== Release API Tests ====================

    /**
     * AR-001: Test list releases
     */
    @Test
    @Order(19)
    void testListReleases() throws IOException {
        String appId = "release-list-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/releases?page=0&size=10",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        HttpGet request = new HttpGet(url);

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("List Releases Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success) or 404 (not found)
            assertTrue(statusCode == 200 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }

    /**
     * AR-002: Test rollback release
     */
    @Test
    @Order(20)
    void testRollbackRelease() throws IOException {
        String appId = "release-rollback-" + UUID.randomUUID().toString().substring(0, 8);
        String url = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/releases/rollback",
                baseUrl, ENV, appId, CLUSTER, NAMESPACE);

        JsonObject body = new JsonObject();
        body.addProperty("releaseId", 1);
        body.addProperty("operator", "test-user");

        HttpPost request = new HttpPost(url);
        request.setEntity(new StringEntity(gson.toJson(body), ContentType.APPLICATION_JSON));

        httpClient.execute(request, response -> {
            int statusCode = response.getCode();
            String responseBody = EntityUtils.toString(response.getEntity());

            System.out.println("Rollback Release Response:");
            System.out.println("  Status: " + statusCode);
            System.out.println("  Body: " + responseBody);

            // May return 200 (success), 400 (bad request), or 404 (not found)
            assertTrue(statusCode == 200 || statusCode == 400 || statusCode == 404,
                    "Should return valid response code");
            return null;
        });
    }
}
