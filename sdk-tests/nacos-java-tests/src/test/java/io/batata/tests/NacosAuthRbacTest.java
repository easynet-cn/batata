package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.maintainer.client.NacosMaintainerFactory;
import com.alibaba.nacos.maintainer.client.config.ConfigMaintainerService;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.*;
import org.junit.jupiter.api.Disabled;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.Properties;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Auth RBAC Tests
 *
 * Tests for user/role/permission CRUD via V3 auth API and permission enforcement via SDK.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosAuthRbacTest {

    private static String serverAddr;
    private static String accessToken;
    private static ConfigMaintainerService maintainerService;
    private static final String TEST_USER = "rbac-user-" + UUID.randomUUID().toString().substring(0, 6);
    private static final String TEST_PASSWORD = "Test123456";
    private static final String TEST_ROLE = "rbac-role-" + UUID.randomUUID().toString().substring(0, 6);
    private static final String TEST_RESOURCE = "public:DEFAULT_GROUP:*";
    private static final ObjectMapper objectMapper = new ObjectMapper();

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        accessToken = loginV3(username, password);
        assertFalse(accessToken.isEmpty(), "Admin login should return a valid access token");

        Properties props = new Properties();
        props.setProperty("serverAddr", serverAddr);
        props.setProperty("username", username);
        props.setProperty("password", password);
        maintainerService = NacosMaintainerFactory.createConfigMaintainerService(props);
    }

    @AfterAll
    static void teardown() throws Exception {
        // Cleanup: delete permission, role, user in reverse order
        try {
            httpDelete("/nacos/v3/auth/permission?role=" + TEST_ROLE
                    + "&resource=" + URLEncoder.encode(TEST_RESOURCE, "UTF-8")
                    + "&action=rw");
        } catch (Exception ignored) {}
        try {
            httpDelete("/nacos/v3/auth/role?role=" + TEST_ROLE + "&username=" + TEST_USER);
        } catch (Exception ignored) {}
        try {
            httpDelete("/nacos/v3/auth/user?username=" + TEST_USER);
        } catch (Exception ignored) {}
    }

    /**
     * Parse a Nacos API response and assert that the response code is 0 (success).
     */
    private static JsonNode assertSuccessResponse(String response) throws Exception {
        assertNotNull(response, "Response should not be null");
        assertFalse(response.isEmpty(), "Response should not be empty");
        JsonNode json = objectMapper.readTree(response);
        assertTrue(json.has("code"), "Response should contain 'code' field: " + response);
        assertEquals(0, json.get("code").asInt(), "Response code should be 0 (success): " + response);
        return json;
    }

    /**
     * Assert that the data field in a response indicates success.
     * Handles both boolean true and string responses like "create user ok!".
     */
    private static void assertDataSuccess(JsonNode json, String operation) {
        JsonNode data = json.get("data");
        assertNotNull(data, operation + " should return data");
        if (data.isBoolean()) {
            assertTrue(data.asBoolean(), operation + " should return true");
        } else if (data.isTextual()) {
            // String responses like "create user ok!", "update user ok!", etc.
            assertFalse(data.asText().isEmpty(), operation + " should return a non-empty string");
        } else {
            // Accept any non-null data as success since code is already 0
            assertTrue(true, operation + " returned data: " + data);
        }
    }

    // ==================== User CRUD Tests ====================

    /**
     * RBAC-001: Test create user
     */
    @Test
    @Order(1)
    void testCreateUser() throws Exception {
        String body = "username=" + URLEncoder.encode(TEST_USER, "UTF-8")
                + "&password=" + URLEncoder.encode(TEST_PASSWORD, "UTF-8");
        String response = httpPost("/nacos/v3/auth/user", body);
        JsonNode json = assertSuccessResponse(response);
        assertDataSuccess(json, "User creation");

        // Verify the user exists by searching
        String searchResponse = httpGet("/nacos/v3/auth/user/list?pageNo=1&pageSize=100");
        JsonNode searchJson = assertSuccessResponse(searchResponse);
        JsonNode data = searchJson.get("data");
        JsonNode pageItems = data.has("pageItems") ? data.get("pageItems") : data;
        boolean found = false;
        if (pageItems.isArray()) {
            for (JsonNode item : pageItems) {
                if (TEST_USER.equals(item.get("username").asText())) {
                    found = true;
                    break;
                }
            }
        }
        assertTrue(found, "Created user '" + TEST_USER + "' should appear in user search results");
    }

    /**
     * RBAC-002: Test search users
     */
    @Test
    @Order(2)
    void testSearchUsers() throws Exception {
        String response = httpGet("/nacos/v3/auth/user/list?pageNo=1&pageSize=100");
        JsonNode json = assertSuccessResponse(response);
        JsonNode data = json.get("data");
        assertNotNull(data, "User search should return data");
        // Should have totalCount and pageItems
        assertTrue(data.has("totalCount") || data.has("pageItems") || data.isArray(),
                "User search should contain pagination fields: " + data);
        int totalCount = data.has("totalCount") ? data.get("totalCount").asInt() : -1;
        if (totalCount >= 0) {
            assertTrue(totalCount >= 2, "Should have at least 2 users (admin + test user), got: " + totalCount);
        }
    }

    /**
     * RBAC-003: Test update user password
     */
    @Test
    @Order(3)
    void testUpdateUserPassword() throws Exception {
        String newPassword = "NewPass123456";
        String body = "username=" + URLEncoder.encode(TEST_USER, "UTF-8")
                + "&newPassword=" + URLEncoder.encode(newPassword, "UTF-8");
        String response = httpPut("/nacos/v3/auth/user", body);
        JsonNode json = assertSuccessResponse(response);
        assertDataSuccess(json, "Password update");

        // Verify login with new password succeeds
        String token = loginV3(TEST_USER, newPassword);
        assertFalse(token.isEmpty(), "Login with new password should succeed and return a token");
        assertTrue(token.length() > 10, "Token should be a non-trivial string, got length: " + token.length());

        // Verify login with old password fails
        String oldToken = loginV3(TEST_USER, TEST_PASSWORD);
        assertTrue(oldToken.isEmpty(), "Login with old password should fail after password change");

        // Revert password for subsequent tests
        body = "username=" + URLEncoder.encode(TEST_USER, "UTF-8")
                + "&newPassword=" + URLEncoder.encode(TEST_PASSWORD, "UTF-8");
        httpPut("/nacos/v3/auth/user", body);
    }

    /**
     * RBAC-004: Test delete user (create a temp user and delete it)
     */
    @Test
    @Order(4)
    void testDeleteUser() throws Exception {
        String tempUser = "temp-del-" + UUID.randomUUID().toString().substring(0, 6);
        String createBody = "username=" + URLEncoder.encode(tempUser, "UTF-8")
                + "&password=" + URLEncoder.encode("TempPass123", "UTF-8");
        String createResponse = httpPost("/nacos/v3/auth/user", createBody);
        JsonNode createJson = assertSuccessResponse(createResponse);
        assertDataSuccess(createJson, "Temp user creation");

        // Delete user
        String response = httpDelete("/nacos/v3/auth/user?username=" + URLEncoder.encode(tempUser, "UTF-8"));
        JsonNode json = assertSuccessResponse(response);
        assertDataSuccess(json, "User deletion");

        // Verify user is gone
        String token = loginV3(tempUser, "TempPass123");
        assertTrue(token.isEmpty(), "Deleted user should not be able to login");
    }

    // ==================== Role CRUD Tests ====================

    /**
     * RBAC-005: Test create role (assign role to user)
     */
    @Test
    @Order(5)
    void testCreateRole() throws Exception {
        String body = "role=" + URLEncoder.encode(TEST_ROLE, "UTF-8")
                + "&username=" + URLEncoder.encode(TEST_USER, "UTF-8");
        String response = httpPost("/nacos/v3/auth/role", body);
        JsonNode json = assertSuccessResponse(response);
        assertDataSuccess(json, "Role creation");

        // Verify role exists in search
        String searchResponse = httpGet("/nacos/v3/auth/role/list?pageNo=1&pageSize=100");
        JsonNode searchJson = assertSuccessResponse(searchResponse);
        JsonNode data = searchJson.get("data");
        JsonNode pageItems = data.has("pageItems") ? data.get("pageItems") : data;
        boolean found = false;
        if (pageItems.isArray()) {
            for (JsonNode item : pageItems) {
                String role = item.has("role") ? item.get("role").asText() : "";
                String user = item.has("username") ? item.get("username").asText() : "";
                if (TEST_ROLE.equals(role) && TEST_USER.equals(user)) {
                    found = true;
                    break;
                }
            }
        }
        assertTrue(found, "Created role '" + TEST_ROLE + "' for user '" + TEST_USER + "' should appear in role search");
    }

    /**
     * RBAC-006: Test search roles
     */
    @Test
    @Order(6)
    void testSearchRoles() throws Exception {
        String response = httpGet("/nacos/v3/auth/role/list?pageNo=1&pageSize=100");
        JsonNode json = assertSuccessResponse(response);
        JsonNode data = json.get("data");
        assertNotNull(data, "Role search should return data");
        assertTrue(data.has("totalCount") || data.has("pageItems") || data.isArray(),
                "Role search should contain pagination fields: " + data);
        int totalCount = data.has("totalCount") ? data.get("totalCount").asInt() : -1;
        if (totalCount >= 0) {
            assertTrue(totalCount >= 1, "Should have at least 1 role, got: " + totalCount);
        }
    }

    /**
     * RBAC-007: Test delete role (create temp and delete)
     */
    @Test
    @Order(7)
    void testDeleteRole() throws Exception {
        String tempRole = "temp-role-" + UUID.randomUUID().toString().substring(0, 6);
        String tempUser = "temp-role-user-" + UUID.randomUUID().toString().substring(0, 6);

        // Create temp user and role
        httpPost("/nacos/v3/auth/user", "username=" + tempUser + "&password=TempPass123");
        String createResponse = httpPost("/nacos/v3/auth/role", "role=" + tempRole + "&username=" + tempUser);
        JsonNode createJson = assertSuccessResponse(createResponse);
        assertDataSuccess(createJson, "Role creation");

        // Delete role
        String response = httpDelete("/nacos/v3/auth/role?role=" + tempRole + "&username=" + tempUser);
        JsonNode json = assertSuccessResponse(response);
        assertDataSuccess(json, "Role deletion");

        // Verify role is gone from search
        String searchResponse = httpGet("/nacos/v3/auth/role/list?pageNo=1&pageSize=100");
        JsonNode searchJson = assertSuccessResponse(searchResponse);
        JsonNode data = searchJson.get("data");
        JsonNode pageItems = data.has("pageItems") ? data.get("pageItems") : data;
        boolean found = false;
        if (pageItems.isArray()) {
            for (JsonNode item : pageItems) {
                if (tempRole.equals(item.has("role") ? item.get("role").asText() : "")) {
                    found = true;
                    break;
                }
            }
        }
        assertFalse(found, "Deleted role '" + tempRole + "' should not appear in role search");

        // Cleanup temp user
        httpDelete("/nacos/v3/auth/user?username=" + tempUser);
    }

    // ==================== Permission CRUD Tests ====================

    /**
     * RBAC-008: Test create permission
     */
    @Test
    @Order(8)
    void testCreatePermission() throws Exception {
        String body = "role=" + URLEncoder.encode(TEST_ROLE, "UTF-8")
                + "&resource=" + URLEncoder.encode(TEST_RESOURCE, "UTF-8")
                + "&action=rw";
        String response = httpPost("/nacos/v3/auth/permission", body);
        JsonNode json = assertSuccessResponse(response);
        assertDataSuccess(json, "Permission creation");

        // Verify permission exists in search
        String searchResponse = httpGet("/nacos/v3/auth/permission/list?pageNo=1&pageSize=100");
        JsonNode searchJson = assertSuccessResponse(searchResponse);
        JsonNode data = searchJson.get("data");
        JsonNode pageItems = data.has("pageItems") ? data.get("pageItems") : data;
        boolean found = false;
        if (pageItems.isArray()) {
            for (JsonNode item : pageItems) {
                String role = item.has("role") ? item.get("role").asText() : "";
                String resource = item.has("resource") ? item.get("resource").asText() : "";
                String action = item.has("action") ? item.get("action").asText() : "";
                if (TEST_ROLE.equals(role) && TEST_RESOURCE.equals(resource) && "rw".equals(action)) {
                    found = true;
                    break;
                }
            }
        }
        assertTrue(found, "Created permission for role '" + TEST_ROLE + "' should appear in permission search");
    }

    /**
     * RBAC-009: Test search permissions
     */
    @Test
    @Order(9)
    void testSearchPermissions() throws Exception {
        String response = httpGet("/nacos/v3/auth/permission/list?pageNo=1&pageSize=100");
        JsonNode json = assertSuccessResponse(response);
        JsonNode data = json.get("data");
        assertNotNull(data, "Permission search should return data");
        assertTrue(data.has("totalCount") || data.has("pageItems") || data.isArray(),
                "Permission search should contain pagination fields: " + data);
        int totalCount = data.has("totalCount") ? data.get("totalCount").asInt() : -1;
        if (totalCount >= 0) {
            assertTrue(totalCount >= 1, "Should have at least 1 permission, got: " + totalCount);
        }
    }

    /**
     * RBAC-010: Test delete permission (create temp and delete)
     */
    @Test
    @Order(10)
    void testDeletePermission() throws Exception {
        String tempRole = "temp-perm-role-" + UUID.randomUUID().toString().substring(0, 6);
        String tempUser = "temp-perm-user-" + UUID.randomUUID().toString().substring(0, 6);
        String tempResource = "public:*:*";

        httpPost("/nacos/v3/auth/user", "username=" + tempUser + "&password=TempPass123");
        httpPost("/nacos/v3/auth/role", "role=" + tempRole + "&username=" + tempUser);
        String createResponse = httpPost("/nacos/v3/auth/permission",
                "role=" + tempRole + "&resource=" + URLEncoder.encode(tempResource, "UTF-8") + "&action=r");
        JsonNode createJson = assertSuccessResponse(createResponse);
        assertDataSuccess(createJson, "Permission creation");

        // Delete permission
        String response = httpDelete("/nacos/v3/auth/permission?role=" + tempRole
                + "&resource=" + URLEncoder.encode(tempResource, "UTF-8")
                + "&action=r");
        JsonNode json = assertSuccessResponse(response);
        assertDataSuccess(json, "Permission deletion");

        // Verify permission is gone from search
        String searchResponse = httpGet("/nacos/v3/auth/permission/list?pageNo=1&pageSize=100");
        JsonNode searchJson = assertSuccessResponse(searchResponse);
        JsonNode data = searchJson.get("data");
        JsonNode pageItems = data.has("pageItems") ? data.get("pageItems") : data;
        boolean found = false;
        if (pageItems.isArray()) {
            for (JsonNode item : pageItems) {
                String role = item.has("role") ? item.get("role").asText() : "";
                String resource = item.has("resource") ? item.get("resource").asText() : "";
                if (tempRole.equals(role) && tempResource.equals(resource)) {
                    found = true;
                    break;
                }
            }
        }
        assertFalse(found, "Deleted permission should not appear in permission search");

        // Cleanup
        httpDelete("/nacos/v3/auth/role?role=" + tempRole + "&username=" + tempUser);
        httpDelete("/nacos/v3/auth/user?username=" + tempUser);
    }

    // ==================== SDK Permission Enforcement Tests ====================

    /**
     * RBAC-011: Test config read with read permission
     */
    @Test
    @Order(11)
    void testConfigReadWithReadPermission() throws Exception {
        String readUser = "read-cfg-user-" + UUID.randomUUID().toString().substring(0, 6);
        String readRole = "read-cfg-role-" + UUID.randomUUID().toString().substring(0, 6);
        String dataId = "rbac-read-test-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // Setup: create user with read permission
            setupUserWithPermission(readUser, readRole, "public:DEFAULT_GROUP:*", "r");

            // Publish config as admin
            ConfigService adminConfig = createConfigService("nacos", System.getProperty("nacos.password", "nacos"));
            boolean published = adminConfig.publishConfig(dataId, "DEFAULT_GROUP", "rbac.read=allowed");
            assertTrue(published, "Admin should be able to publish config");
            Thread.sleep(1000);

            // Read config as read-only user
            ConfigService readOnlyConfig = createConfigService(readUser, TEST_PASSWORD);
            String content = readOnlyConfig.getConfig(dataId, "DEFAULT_GROUP", 5000);
            assertNotNull(content, "Read-only user should be able to read config");
            assertEquals("rbac.read=allowed", content, "Config content should match what was published");

            readOnlyConfig.shutDown();
            adminConfig.removeConfig(dataId, "DEFAULT_GROUP");
            adminConfig.shutDown();
        } finally {
            cleanupUserWithPermission(readUser, readRole);
        }
    }

    /**
     * RBAC-012: Test config write with read permission (should fail)
     */
    @Test
    @Order(12)
    void testConfigWriteWithReadPermission() throws Exception {
        String readUser = "ronly-cfg-user-" + UUID.randomUUID().toString().substring(0, 6);
        String readRole = "ronly-cfg-role-" + UUID.randomUUID().toString().substring(0, 6);

        try {
            setupUserWithPermission(readUser, readRole, "public:DEFAULT_GROUP:*", "r");

            ConfigService readOnlyConfig = createConfigService(readUser, TEST_PASSWORD);
            String dataId = "rbac-write-fail-" + UUID.randomUUID().toString().substring(0, 8);

            boolean writeSucceeded = false;
            try {
                boolean result = readOnlyConfig.publishConfig(dataId, "DEFAULT_GROUP", "should.fail=true");
                writeSucceeded = result;
            } catch (NacosException e) {
                // Expected: write should be denied
                writeSucceeded = false;
            }
            assertFalse(writeSucceeded, "Write with read-only permission should be denied");

            readOnlyConfig.shutDown();
        } finally {
            cleanupUserWithPermission(readUser, readRole);
        }
    }

    /**
     * RBAC-013: Test config write with write permission
     */
    @Test
    @Order(13)
    void testConfigWriteWithWritePermission() throws Exception {
        String writeUser = "write-cfg-user-" + UUID.randomUUID().toString().substring(0, 6);
        String writeRole = "write-cfg-role-" + UUID.randomUUID().toString().substring(0, 6);
        String dataId = "rbac-write-ok-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            setupUserWithPermission(writeUser, writeRole, "public:DEFAULT_GROUP:*", "w");

            ConfigService writeConfig = createConfigService(writeUser, TEST_PASSWORD);
            boolean result = writeConfig.publishConfig(dataId, "DEFAULT_GROUP", "rbac.write=allowed");
            assertTrue(result, "User with write permission should be able to publish config");

            writeConfig.shutDown();

            // Cleanup config as admin
            ConfigService adminConfig = createConfigService("nacos", System.getProperty("nacos.password", "nacos"));
            adminConfig.removeConfig(dataId, "DEFAULT_GROUP");
            adminConfig.shutDown();
        } finally {
            cleanupUserWithPermission(writeUser, writeRole);
        }
    }

    /**
     * RBAC-014: Test config read with write-only permission (should fail)
     */
    @Test
    @Order(14)
    void testConfigReadWithWritePermission() throws Exception {
        String writeUser = "wonly-cfg-user-" + UUID.randomUUID().toString().substring(0, 6);
        String writeRole = "wonly-cfg-role-" + UUID.randomUUID().toString().substring(0, 6);
        String dataId = "rbac-read-fail-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            setupUserWithPermission(writeUser, writeRole, "public:DEFAULT_GROUP:*", "w");

            // Publish config as admin
            ConfigService adminConfig = createConfigService("nacos", System.getProperty("nacos.password", "nacos"));
            adminConfig.publishConfig(dataId, "DEFAULT_GROUP", "rbac.readonly=blocked");
            Thread.sleep(1000);

            // Try to read as write-only user
            ConfigService writeOnlyConfig = createConfigService(writeUser, TEST_PASSWORD);
            String content = null;
            boolean readDenied = false;
            try {
                content = writeOnlyConfig.getConfig(dataId, "DEFAULT_GROUP", 5000);
                // If content is null or empty, it was effectively denied
                readDenied = (content == null || content.isEmpty());
            } catch (NacosException e) {
                readDenied = true;
            }
            assertTrue(readDenied, "Read with write-only permission should be denied, but got: " + content);

            writeOnlyConfig.shutDown();
            adminConfig.removeConfig(dataId, "DEFAULT_GROUP");
            adminConfig.shutDown();
        } finally {
            cleanupUserWithPermission(writeUser, writeRole);
        }
    }

    /**
     * RBAC-015: Test naming read with read permission
     */
    @Test
    @Order(15)
    void testNamingReadWithReadPermission() throws Exception {
        String readUser = "read-ns-user-" + UUID.randomUUID().toString().substring(0, 6);
        String readRole = "read-ns-role-" + UUID.randomUUID().toString().substring(0, 6);
        String serviceName = "rbac-read-svc-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            setupUserWithPermission(readUser, readRole, "public:DEFAULT_GROUP:*", "r");

            // Register instance as admin
            NamingService adminNaming = createNamingService("nacos", System.getProperty("nacos.password", "nacos"));
            adminNaming.registerInstance(serviceName, "192.168.1.1", 8080);
            Thread.sleep(1000);

            // Read instances as read-only user
            NamingService readOnlyNaming = createNamingService(readUser, TEST_PASSWORD);
            var instances = readOnlyNaming.getAllInstances(serviceName);
            assertNotNull(instances, "Read-only user should be able to query instances");
            assertEquals(1, instances.size(), "Should have 1 registered instance");
            assertEquals("192.168.1.1", instances.get(0).getIp(), "Instance IP should match");
            assertEquals(8080, instances.get(0).getPort(), "Instance port should match");

            readOnlyNaming.shutDown();
            adminNaming.deregisterInstance(serviceName, "192.168.1.1", 8080);
            adminNaming.shutDown();
        } finally {
            cleanupUserWithPermission(readUser, readRole);
        }
    }

    /**
     * RBAC-016: Test naming write with read permission (should fail)
     */
    @Test
    @Order(16)
    void testNamingWriteWithReadPermission() throws Exception {
        String readUser = "ronly-ns-user-" + UUID.randomUUID().toString().substring(0, 6);
        String readRole = "ronly-ns-role-" + UUID.randomUUID().toString().substring(0, 6);

        try {
            setupUserWithPermission(readUser, readRole, "public:DEFAULT_GROUP:*", "r");

            NamingService readOnlyNaming = createNamingService(readUser, TEST_PASSWORD);
            String serviceName = "rbac-write-fail-svc-" + UUID.randomUUID().toString().substring(0, 8);

            boolean writeDenied = false;
            try {
                readOnlyNaming.registerInstance(serviceName, "192.168.1.1", 8080);
                // If we get here, try to clean up
                readOnlyNaming.deregisterInstance(serviceName, "192.168.1.1", 8080);
                writeDenied = false;
            } catch (NacosException e) {
                writeDenied = true;
            }
            assertTrue(writeDenied, "Naming write with read-only permission should be denied");

            readOnlyNaming.shutDown();
        } finally {
            cleanupUserWithPermission(readUser, readRole);
        }
    }

    /**
     * RBAC-017: Test naming write with write permission
     */
    @Test
    @Order(17)
    void testNamingWriteWithWritePermission() throws Exception {
        String writeUser = "write-ns-user-" + UUID.randomUUID().toString().substring(0, 6);
        String writeRole = "write-ns-role-" + UUID.randomUUID().toString().substring(0, 6);
        String serviceName = "rbac-write-ok-svc-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            setupUserWithPermission(writeUser, writeRole, "public:DEFAULT_GROUP:*", "w");

            NamingService writeNaming = createNamingService(writeUser, TEST_PASSWORD);
            // Should not throw - write permission is sufficient
            writeNaming.registerInstance(serviceName, "192.168.1.1", 8080);
            Thread.sleep(1000);

            // Verify the instance was registered by querying as admin
            NamingService adminNaming = createNamingService("nacos", System.getProperty("nacos.password", "nacos"));
            var instances = adminNaming.getAllInstances(serviceName);
            assertNotNull(instances, "Admin should be able to query instances");
            assertEquals(1, instances.size(), "Should have 1 registered instance");
            assertEquals("192.168.1.1", instances.get(0).getIp(), "Instance IP should match");

            writeNaming.deregisterInstance(serviceName, "192.168.1.1", 8080);
            writeNaming.shutDown();
            adminNaming.shutDown();
        } finally {
            cleanupUserWithPermission(writeUser, writeRole);
        }
    }

    /**
     * RBAC-018: Test naming read with write-only permission (should fail)
     */
    @Test
    @Order(18)
    void testNamingReadWithWritePermission() throws Exception {
        String writeUser = "wonly-ns-user-" + UUID.randomUUID().toString().substring(0, 6);
        String writeRole = "wonly-ns-role-" + UUID.randomUUID().toString().substring(0, 6);
        String serviceName = "rbac-read-fail-svc-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            setupUserWithPermission(writeUser, writeRole, "public:DEFAULT_GROUP:*", "w");

            // Register as admin
            NamingService adminNaming = createNamingService("nacos", System.getProperty("nacos.password", "nacos"));
            adminNaming.registerInstance(serviceName, "192.168.1.1", 8080);
            Thread.sleep(1000);

            // Try to read as write-only user
            NamingService writeOnlyNaming = createNamingService(writeUser, TEST_PASSWORD);
            boolean readDenied = false;
            try {
                var instances = writeOnlyNaming.getAllInstances(serviceName);
                // If returned empty or null, effectively denied
                readDenied = (instances == null || instances.isEmpty());
            } catch (NacosException e) {
                readDenied = true;
            }
            assertTrue(readDenied,
                    "Naming read with write-only permission should be denied or return empty results");

            writeOnlyNaming.shutDown();
            adminNaming.deregisterInstance(serviceName, "192.168.1.1", 8080);
            adminNaming.shutDown();
        } finally {
            cleanupUserWithPermission(writeUser, writeRole);
        }
    }

    /**
     * RBAC-019: Test full (rw) permission allows both read and write
     */
    @Test
    @Order(19)
    void testFullPermission() throws Exception {
        String rwUser = "rw-user-" + UUID.randomUUID().toString().substring(0, 6);
        String rwRole = "rw-role-" + UUID.randomUUID().toString().substring(0, 6);
        String dataId = "rbac-rw-test-" + UUID.randomUUID().toString().substring(0, 8);
        String configContent = "rbac.full=access";

        try {
            setupUserWithPermission(rwUser, rwRole, "public:DEFAULT_GROUP:*", "rw");

            ConfigService rwConfig = createConfigService(rwUser, TEST_PASSWORD);

            // Write should succeed
            boolean writeResult = rwConfig.publishConfig(dataId, "DEFAULT_GROUP", configContent);
            assertTrue(writeResult, "User with rw permission should be able to write config");
            Thread.sleep(1000);

            // Read should succeed
            String content = rwConfig.getConfig(dataId, "DEFAULT_GROUP", 5000);
            assertNotNull(content, "User with rw permission should be able to read config");
            assertEquals(configContent, content, "Read content should match written content");

            rwConfig.removeConfig(dataId, "DEFAULT_GROUP");
            rwConfig.shutDown();
        } finally {
            cleanupUserWithPermission(rwUser, rwRole);
        }
    }

    /**
     * RBAC-020: Test login returns valid JWT token structure
     */
    @Test
    @Order(20)
    void testLoginTokenStructure() throws Exception {
        String loginUrl = String.format("http://%s/nacos/v3/auth/user/login", serverAddr);
        URL url = new URL(loginUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("POST");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");

        String body = "username=" + URLEncoder.encode("nacos", "UTF-8")
                + "&password=" + URLEncoder.encode(System.getProperty("nacos.password", "nacos"), "UTF-8");
        conn.getOutputStream().write(body.getBytes(StandardCharsets.UTF_8));

        assertEquals(200, conn.getResponseCode(), "Login should return HTTP 200");

        BufferedReader reader = new BufferedReader(new InputStreamReader(conn.getInputStream()));
        StringBuilder response = new StringBuilder();
        String line;
        while ((line = reader.readLine()) != null) {
            response.append(line);
        }
        JsonNode json = objectMapper.readTree(response.toString());

        // Accept both wrapped format {"code":0,"data":{"accessToken":...}}
        // and flat format {"accessToken":"...","tokenTtl":18000,...}
        String token;
        if (json.has("code")) {
            assertEquals(0, json.get("code").asInt(), "Login response code should be 0");
            JsonNode data = json.get("data");
            assertNotNull(data, "Login response should have 'data' field");
            token = data.has("accessToken") ? data.get("accessToken").asText() : "";
        } else if (json.has("accessToken")) {
            token = json.get("accessToken").asText();
        } else {
            fail("Login response should have either 'code' or 'accessToken' field: " + response);
            return; // unreachable but satisfies compiler
        }
        assertFalse(token.isEmpty(), "Access token should not be empty");
        // JWT tokens have 3 parts separated by dots
        String[] parts = token.split("\\.");
        assertEquals(3, parts.length, "JWT token should have 3 parts (header.payload.signature), got: " + parts.length);

        // Check token TTL is present (check in both wrapped and flat format)
        if (json.has("tokenTtl")) {
            int ttl = json.get("tokenTtl").asInt();
            assertTrue(ttl > 0, "Token TTL should be positive, got: " + ttl);
        } else if (json.has("data") && json.get("data").has("tokenTtl")) {
            int ttl = json.get("data").get("tokenTtl").asInt();
            assertTrue(ttl > 0, "Token TTL should be positive, got: " + ttl);
        }
    }

    // ==================== Cross-Namespace Isolation Tests ====================

    /**
     * RBAC-021: Cross-namespace isolation - user can only access authorized namespace
     */
    @Test
    @Order(21)
    void testCrossNamespaceConfigIsolation() throws Exception {
        String nsUser = "ns-user-" + UUID.randomUUID().toString().substring(0, 6);
        String nsRole = "ns-role-" + UUID.randomUUID().toString().substring(0, 6);
        String isolatedNs = "test-ns-isolated-" + UUID.randomUUID().toString().substring(0, 6);
        String isolatedDataId = "isolated-cfg-" + UUID.randomUUID().toString().substring(0, 8);
        String publicDataId = "public-cfg-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // Create namespace via SDK
            maintainerService.createNamespace(isolatedNs, isolatedNs, "Test isolated namespace");
            Thread.sleep(500);

            // Create user with permission only on the isolated namespace
            String nsResource = isolatedNs + ":DEFAULT_GROUP:*";
            setupUserWithPermission(nsUser, nsRole, nsResource, "rw");

            // As admin: publish config in isolated namespace
            ConfigService adminConfigIsolated = createConfigServiceWithNamespace("nacos",
                    System.getProperty("nacos.password", "nacos"), isolatedNs);
            boolean published = adminConfigIsolated.publishConfig(isolatedDataId, "DEFAULT_GROUP", "isolated.data=secret");
            assertTrue(published, "Admin should publish config in isolated namespace");
            Thread.sleep(1000);

            // As admin: publish config in public namespace
            ConfigService adminConfig = createConfigService("nacos", System.getProperty("nacos.password", "nacos"));
            boolean publishedPublic = adminConfig.publishConfig(publicDataId, "DEFAULT_GROUP", "public.data=visible");
            assertTrue(publishedPublic, "Admin should publish config in public namespace");
            Thread.sleep(1000);

            // As ns-user: should be able to read "isolated-cfg" in the isolated namespace
            ConfigService nsConfig = createConfigServiceWithNamespace(nsUser, TEST_PASSWORD, isolatedNs);
            String isolatedContent = nsConfig.getConfig(isolatedDataId, "DEFAULT_GROUP", 5000);
            assertNotNull(isolatedContent, "NS user should be able to read config in authorized namespace");
            assertEquals("isolated.data=secret", isolatedContent, "Config content should match");

            // As ns-user: should NOT be able to read "public-cfg" in public namespace
            ConfigService nsConfigPublic = createConfigService(nsUser, TEST_PASSWORD);
            String publicContent = null;
            boolean readDenied = false;
            try {
                publicContent = nsConfigPublic.getConfig(publicDataId, "DEFAULT_GROUP", 5000);
                readDenied = (publicContent == null || publicContent.isEmpty());
            } catch (NacosException e) {
                readDenied = true;
            }
            assertTrue(readDenied,
                    "NS user should NOT be able to read config in public namespace, but got: " + publicContent);

            // As ns-user: should be able to write config in isolated namespace
            String writeDataId = "ns-write-test-" + UUID.randomUUID().toString().substring(0, 8);
            boolean writeResult = nsConfig.publishConfig(writeDataId, "DEFAULT_GROUP", "ns.write=ok");
            assertTrue(writeResult, "NS user should be able to write config in authorized namespace");

            // As ns-user: should NOT be able to write config in public namespace
            boolean writePublicSucceeded = false;
            try {
                writePublicSucceeded = nsConfigPublic.publishConfig(
                        "ns-write-fail-" + UUID.randomUUID().toString().substring(0, 8),
                        "DEFAULT_GROUP", "should.fail=true");
            } catch (NacosException e) {
                writePublicSucceeded = false;
            }
            assertFalse(writePublicSucceeded,
                    "NS user should NOT be able to write config in public namespace");

            // Cleanup configs
            nsConfig.removeConfig(writeDataId, "DEFAULT_GROUP");
            nsConfig.shutDown();
            nsConfigPublic.shutDown();
            adminConfigIsolated.removeConfig(isolatedDataId, "DEFAULT_GROUP");
            adminConfigIsolated.shutDown();
            adminConfig.removeConfig(publicDataId, "DEFAULT_GROUP");
            adminConfig.shutDown();
        } finally {
            // Cleanup: delete permission, role, user, namespace
            cleanupUserWithPermissionForNamespace(nsUser, nsRole, isolatedNs);
            try {
                maintainerService.deleteNamespace(isolatedNs);
            } catch (Exception ignored) {}
        }
    }

    /**
     * RBAC-022: Cross-namespace isolation for naming service
     */
    @Test
    @Order(22)
    void testCrossNamespaceNamingIsolation() throws Exception {
        String nsUser = "ns-naming-user-" + UUID.randomUUID().toString().substring(0, 6);
        String nsRole = "ns-naming-role-" + UUID.randomUUID().toString().substring(0, 6);
        String namingNs = "test-ns-naming-" + UUID.randomUUID().toString().substring(0, 6);
        String serviceName = "ns-isolation-svc-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // Create namespace via SDK
            maintainerService.createNamespace(namingNs, namingNs, "Test naming namespace");
            Thread.sleep(500);

            // Create user with permission only on the naming namespace
            String nsResource = namingNs + ":DEFAULT_GROUP:*";
            setupUserWithPermission(nsUser, nsRole, nsResource, "rw");

            // As admin: register instance in the naming namespace
            NamingService adminNaming = createNamingServiceWithNamespace("nacos",
                    System.getProperty("nacos.password", "nacos"), namingNs);
            adminNaming.registerInstance(serviceName, "10.0.0.1", 8080);
            Thread.sleep(1000);

            // As admin: register instance in public namespace
            NamingService adminNamingPublic = createNamingService("nacos",
                    System.getProperty("nacos.password", "nacos"));
            adminNamingPublic.registerInstance(serviceName, "10.0.0.2", 8080);
            Thread.sleep(1000);

            // As restricted user: should be able to list instances in the naming namespace
            NamingService nsNaming = createNamingServiceWithNamespace(nsUser, TEST_PASSWORD, namingNs);
            var instances = nsNaming.getAllInstances(serviceName);
            assertNotNull(instances, "NS user should be able to query instances in authorized namespace");
            assertFalse(instances.isEmpty(),
                    "NS user should see instances in authorized namespace");
            boolean foundExpected = false;
            for (var inst : instances) {
                if ("10.0.0.1".equals(inst.getIp()) && inst.getPort() == 8080) {
                    foundExpected = true;
                    break;
                }
            }
            assertTrue(foundExpected, "Should find the instance registered in the naming namespace");

            // As restricted user: should NOT be able to register instance in public namespace
            NamingService nsNamingPublic = createNamingService(nsUser, TEST_PASSWORD);
            boolean writeDenied = false;
            try {
                nsNamingPublic.registerInstance("denied-svc-" + UUID.randomUUID().toString().substring(0, 8),
                        "10.0.0.99", 9999);
                writeDenied = false;
            } catch (NacosException e) {
                writeDenied = true;
            }
            assertTrue(writeDenied,
                    "NS user should NOT be able to register instance in public namespace");

            // Cleanup
            nsNaming.shutDown();
            nsNamingPublic.shutDown();
            adminNaming.deregisterInstance(serviceName, "10.0.0.1", 8080);
            adminNaming.shutDown();
            adminNamingPublic.deregisterInstance(serviceName, "10.0.0.2", 8080);
            adminNamingPublic.shutDown();
        } finally {
            // Cleanup: delete permission, role, user, namespace
            cleanupUserWithPermissionForNamespace(nsUser, nsRole, namingNs);
            try {
                maintainerService.deleteNamespace(namingNs);
            } catch (Exception ignored) {}
        }
    }

    // ==================== Fine-Grained Permission Tests ====================

    /**
     * RBAC-023: Resource type isolation - config permission should NOT grant naming access
     *
     * A user with config:rw should not be able to register instances.
     */
    @Test
    @Order(23)
    void testResourceTypeIsolation() throws Exception {
        String user = "rtype-user-" + UUID.randomUUID().toString().substring(0, 6);
        String role = "rtype-role-" + UUID.randomUUID().toString().substring(0, 6);

        try {
            // Grant only config permissions
            setupUserWithPermission(user, role, "public:DEFAULT_GROUP:config/*", "rw");

            // Config operations should succeed
            ConfigService cfgService = createConfigService(user, TEST_PASSWORD);
            String dataId = "rtype-test-" + UUID.randomUUID().toString().substring(0, 8);
            boolean published = cfgService.publishConfig(dataId, "DEFAULT_GROUP", "rtype=config");
            assertTrue(published, "User with config:rw should be able to publish config");
            Thread.sleep(500);

            String content = cfgService.getConfig(dataId, "DEFAULT_GROUP", 5000);
            assertEquals("rtype=config", content, "User with config:rw should read config");

            // Naming operations should be denied
            NamingService namingService = createNamingService(user, TEST_PASSWORD);
            boolean namingDenied = false;
            try {
                namingService.registerInstance("rtype-svc-" + UUID.randomUUID().toString().substring(0, 8),
                        "10.0.0.1", 8080);
            } catch (NacosException e) {
                namingDenied = true;
            }
            assertTrue(namingDenied,
                    "User with config-only permission should NOT be able to register naming instances");

            cfgService.removeConfig(dataId, "DEFAULT_GROUP");
            cfgService.shutDown();
            namingService.shutDown();
        } finally {
            cleanupUserWithSpecificPermission(user, role, "public:DEFAULT_GROUP:config/*", "rw");
        }
    }

    /**
     * RBAC-024: Specific dataId permission - user can only access one config
     *
     * Grant permission on a specific config dataId, verify other dataIds are denied.
     */
    @Test
    @Order(24)
    void testSpecificDataIdPermission() throws Exception {
        String user = "specid-user-" + UUID.randomUUID().toString().substring(0, 6);
        String role = "specid-role-" + UUID.randomUUID().toString().substring(0, 6);
        String allowedDataId = "allowed-cfg-" + UUID.randomUUID().toString().substring(0, 8);
        String deniedDataId = "denied-cfg-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // Grant permission only on the specific dataId
            String specificResource = "public:DEFAULT_GROUP:config/" + allowedDataId;
            setupUserWithPermission(user, role, specificResource, "rw");

            // Admin publishes both configs
            ConfigService adminConfig = createConfigService("nacos", System.getProperty("nacos.password", "nacos"));
            adminConfig.publishConfig(allowedDataId, "DEFAULT_GROUP", "allowed=true");
            adminConfig.publishConfig(deniedDataId, "DEFAULT_GROUP", "denied=true");
            Thread.sleep(1000);

            // User should be able to read the allowed config
            ConfigService userConfig = createConfigService(user, TEST_PASSWORD);
            String allowedContent = userConfig.getConfig(allowedDataId, "DEFAULT_GROUP", 5000);
            assertNotNull(allowedContent, "User should read the specifically permitted config");
            assertEquals("allowed=true", allowedContent);

            // User should NOT be able to read the denied config
            boolean readDenied = false;
            try {
                String deniedContent = userConfig.getConfig(deniedDataId, "DEFAULT_GROUP", 5000);
                readDenied = (deniedContent == null || deniedContent.isEmpty());
            } catch (NacosException e) {
                readDenied = true;
            }
            assertTrue(readDenied,
                    "User should NOT be able to read config outside specific permission");

            userConfig.shutDown();
            adminConfig.removeConfig(allowedDataId, "DEFAULT_GROUP");
            adminConfig.removeConfig(deniedDataId, "DEFAULT_GROUP");
            adminConfig.shutDown();
        } finally {
            String specificResource = "public:DEFAULT_GROUP:config/" + allowedDataId;
            cleanupUserWithSpecificPermission(user, role, specificResource, "rw");
        }
    }

    /**
     * RBAC-025: Wildcard prefix pattern permission
     *
     * Grant permission on "public:DEFAULT_GROUP:config/app-*", verify only matching
     * dataIds are accessible.
     */
    @Test
    @Order(25)
    void testWildcardPrefixPermission() throws Exception {
        String user = "wildcard-user-" + UUID.randomUUID().toString().substring(0, 6);
        String role = "wildcard-role-" + UUID.randomUUID().toString().substring(0, 6);
        String prefix = "wc-" + UUID.randomUUID().toString().substring(0, 6);
        String matchingDataId = prefix + "-config-a";
        String nonMatchingDataId = "other-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // Grant wildcard prefix permission
            String wildcardResource = "public:DEFAULT_GROUP:config/" + prefix + "*";
            setupUserWithPermission(user, role, wildcardResource, "rw");

            // Admin publishes both configs
            ConfigService adminConfig = createConfigService("nacos", System.getProperty("nacos.password", "nacos"));
            adminConfig.publishConfig(matchingDataId, "DEFAULT_GROUP", "matching=true");
            adminConfig.publishConfig(nonMatchingDataId, "DEFAULT_GROUP", "nonmatching=true");
            Thread.sleep(1000);

            ConfigService userConfig = createConfigService(user, TEST_PASSWORD);

            // Matching dataId should be accessible
            String matchContent = userConfig.getConfig(matchingDataId, "DEFAULT_GROUP", 5000);
            assertNotNull(matchContent, "Wildcard-matching config should be readable");
            assertEquals("matching=true", matchContent);

            // Non-matching dataId should be denied
            boolean denied = false;
            try {
                String nonMatchContent = userConfig.getConfig(nonMatchingDataId, "DEFAULT_GROUP", 5000);
                denied = (nonMatchContent == null || nonMatchContent.isEmpty());
            } catch (NacosException e) {
                denied = true;
            }
            assertTrue(denied, "Non-matching config should NOT be readable with wildcard prefix permission");

            userConfig.shutDown();
            adminConfig.removeConfig(matchingDataId, "DEFAULT_GROUP");
            adminConfig.removeConfig(nonMatchingDataId, "DEFAULT_GROUP");
            adminConfig.shutDown();
        } finally {
            String wildcardResource = "public:DEFAULT_GROUP:config/" + prefix + "*";
            cleanupUserWithSpecificPermission(user, role, wildcardResource, "rw");
        }
    }

    /**
     * RBAC-026: Custom group permission - permission on specific group only
     *
     * Grant permission on CUSTOM_GROUP, verify DEFAULT_GROUP is denied.
     */
    @Test
    @Order(26)
    void testCustomGroupPermission() throws Exception {
        String user = "grp-user-" + UUID.randomUUID().toString().substring(0, 6);
        String role = "grp-role-" + UUID.randomUUID().toString().substring(0, 6);
        String customGroup = "CUSTOM_AUTH_GROUP";
        String dataId = "grp-test-cfg-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // Grant permission only on CUSTOM_AUTH_GROUP
            String groupResource = "public:" + customGroup + ":config/*";
            setupUserWithPermission(user, role, groupResource, "rw");

            // Admin publishes config in both groups
            ConfigService adminConfig = createConfigService("nacos", System.getProperty("nacos.password", "nacos"));
            adminConfig.publishConfig(dataId, customGroup, "custom.group=true");
            adminConfig.publishConfig(dataId, "DEFAULT_GROUP", "default.group=true");
            Thread.sleep(1000);

            ConfigService userConfig = createConfigService(user, TEST_PASSWORD);

            // Custom group config should be accessible
            String customContent = userConfig.getConfig(dataId, customGroup, 5000);
            assertNotNull(customContent, "User should read config in permitted custom group");
            assertEquals("custom.group=true", customContent);

            // Default group config should be denied
            boolean denied = false;
            try {
                String defaultContent = userConfig.getConfig(dataId, "DEFAULT_GROUP", 5000);
                denied = (defaultContent == null || defaultContent.isEmpty());
            } catch (NacosException e) {
                denied = true;
            }
            assertTrue(denied, "User should NOT read config in non-permitted DEFAULT_GROUP");

            userConfig.shutDown();
            adminConfig.removeConfig(dataId, customGroup);
            adminConfig.removeConfig(dataId, "DEFAULT_GROUP");
            adminConfig.shutDown();
        } finally {
            String groupResource = "public:" + customGroup + ":config/*";
            cleanupUserWithSpecificPermission(user, role, groupResource, "rw");
        }
    }

    /**
     * RBAC-027: Multiple roles - user with config:r role + naming:w role
     *
     * User should be able to read config AND write naming, but not write config or read naming.
     */
    @Test
    @Order(27)
    void testMultipleRoles() throws Exception {
        String user = "multi-role-user-" + UUID.randomUUID().toString().substring(0, 6);
        String configRole = "multi-cfg-role-" + UUID.randomUUID().toString().substring(0, 6);
        String namingRole = "multi-ns-role-" + UUID.randomUUID().toString().substring(0, 6);
        String dataId = "multi-role-cfg-" + UUID.randomUUID().toString().substring(0, 8);
        String serviceName = "multi-role-svc-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // Create user
            httpPost("/nacos/v3/auth/user",
                    "username=" + URLEncoder.encode(user, "UTF-8")
                            + "&password=" + URLEncoder.encode(TEST_PASSWORD, "UTF-8"));

            // Assign config:r role
            httpPost("/nacos/v3/auth/role",
                    "role=" + configRole + "&username=" + user);
            httpPost("/nacos/v3/auth/permission",
                    "role=" + configRole
                            + "&resource=" + URLEncoder.encode("public:DEFAULT_GROUP:config/*", "UTF-8")
                            + "&action=r");

            // Assign naming:w role
            httpPost("/nacos/v3/auth/role",
                    "role=" + namingRole + "&username=" + user);
            httpPost("/nacos/v3/auth/permission",
                    "role=" + namingRole
                            + "&resource=" + URLEncoder.encode("public:DEFAULT_GROUP:naming/*", "UTF-8")
                            + "&action=w");

            // Admin publishes config
            ConfigService adminConfig = createConfigService("nacos", System.getProperty("nacos.password", "nacos"));
            adminConfig.publishConfig(dataId, "DEFAULT_GROUP", "multi.role=test");
            Thread.sleep(1000);

            // User can read config (from configRole)
            ConfigService userConfig = createConfigService(user, TEST_PASSWORD);
            String content = userConfig.getConfig(dataId, "DEFAULT_GROUP", 5000);
            assertNotNull(content, "User with config:r role should read config");
            assertEquals("multi.role=test", content);

            // User cannot write config (configRole only has "r")
            boolean writeDenied = false;
            try {
                boolean result = userConfig.publishConfig(
                        "multi-deny-" + UUID.randomUUID().toString().substring(0, 8),
                        "DEFAULT_GROUP", "should.fail=true");
                writeDenied = !result;
            } catch (NacosException e) {
                writeDenied = true;
            }
            assertTrue(writeDenied, "User with config:r should NOT be able to write config");

            // User can write naming (from namingRole)
            NamingService userNaming = createNamingService(user, TEST_PASSWORD);
            userNaming.registerInstance(serviceName, "10.0.0.1", 8080);
            Thread.sleep(1000);

            // Verify via admin
            NamingService adminNaming = createNamingService("nacos", System.getProperty("nacos.password", "nacos"));
            var instances = adminNaming.getAllInstances(serviceName);
            assertNotNull(instances);
            assertFalse(instances.isEmpty(), "Instance should be registered via naming:w permission");

            // Cleanup
            userConfig.shutDown();
            userNaming.deregisterInstance(serviceName, "10.0.0.1", 8080);
            userNaming.shutDown();
            adminConfig.removeConfig(dataId, "DEFAULT_GROUP");
            adminConfig.shutDown();
            adminNaming.shutDown();
        } finally {
            try { httpDelete("/nacos/v3/auth/permission?role=" + configRole
                    + "&resource=" + URLEncoder.encode("public:DEFAULT_GROUP:config/*", "UTF-8")
                    + "&action=r"); } catch (Exception ignored) {}
            try { httpDelete("/nacos/v3/auth/permission?role=" + namingRole
                    + "&resource=" + URLEncoder.encode("public:DEFAULT_GROUP:naming/*", "UTF-8")
                    + "&action=w"); } catch (Exception ignored) {}
            try { httpDelete("/nacos/v3/auth/role?role=" + configRole + "&username=" + user); } catch (Exception ignored) {}
            try { httpDelete("/nacos/v3/auth/role?role=" + namingRole + "&username=" + user); } catch (Exception ignored) {}
            try { httpDelete("/nacos/v3/auth/user?username=" + user); } catch (Exception ignored) {}
        }
    }

    /**
     * RBAC-028: Unauthenticated access should be denied
     *
     * Accessing config/naming without any token should fail.
     */
    @Test
    @Order(28)
    void testUnauthenticatedAccessDenied() throws Exception {
        // Create ConfigService without username/password
        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        // No username/password set

        String dataId = "unauth-test-" + UUID.randomUUID().toString().substring(0, 8);

        // Publish config as admin first
        ConfigService adminConfig = createConfigService("nacos", System.getProperty("nacos.password", "nacos"));
        adminConfig.publishConfig(dataId, "DEFAULT_GROUP", "unauth.test=value");
        Thread.sleep(500);

        // Try to read without auth - should fail
        boolean accessDenied = false;
        ConfigService unauthConfig = null;
        try {
            unauthConfig = NacosFactory.createConfigService(properties);
            String content = unauthConfig.getConfig(dataId, "DEFAULT_GROUP", 5000);
            accessDenied = (content == null || content.isEmpty());
        } catch (NacosException e) {
            accessDenied = true;
        } finally {
            if (unauthConfig != null) {
                try { unauthConfig.shutDown(); } catch (Exception ignored) {}
            }
        }
        assertTrue(accessDenied, "Unauthenticated access should be denied");

        adminConfig.removeConfig(dataId, "DEFAULT_GROUP");
        adminConfig.shutDown();
    }

    /**
     * RBAC-029: Invalid credentials should fail login
     */
    @Test
    @Order(29)
    void testInvalidCredentialsDenied() throws Exception {
        String token = loginV3("nonexistent-user", "wrong-password");
        assertTrue(token.isEmpty(), "Login with invalid credentials should fail");

        // Also test valid user with wrong password
        String wrongPassToken = loginV3("nacos", "definitely-wrong-password");
        assertTrue(wrongPassToken.isEmpty(), "Login with wrong password should fail");
    }

    /**
     * RBAC-030: Permission revocation takes effect
     *
     * Grant permission, verify access, revoke permission, verify denial.
     */
    @Test
    @Order(30)
    void testPermissionRevocation() throws Exception {
        String user = "revoke-user-" + UUID.randomUUID().toString().substring(0, 6);
        String role = "revoke-role-" + UUID.randomUUID().toString().substring(0, 6);
        String dataId = "revoke-cfg-" + UUID.randomUUID().toString().substring(0, 8);
        String resource = "public:DEFAULT_GROUP:config/*";

        try {
            // Setup user with rw permission
            setupUserWithPermission(user, role, resource, "rw");

            // Admin publishes config
            ConfigService adminConfig = createConfigService("nacos", System.getProperty("nacos.password", "nacos"));
            adminConfig.publishConfig(dataId, "DEFAULT_GROUP", "revoke.test=value");
            Thread.sleep(1000);

            // User can read config
            ConfigService userConfig = createConfigService(user, TEST_PASSWORD);
            String content = userConfig.getConfig(dataId, "DEFAULT_GROUP", 5000);
            assertNotNull(content, "User should read config before revocation");
            assertEquals("revoke.test=value", content);
            userConfig.shutDown();

            // Revoke permission
            httpDelete("/nacos/v3/auth/permission?role=" + URLEncoder.encode(role, "UTF-8")
                    + "&resource=" + URLEncoder.encode(resource, "UTF-8")
                    + "&action=rw");
            // Wait for cache to expire or be invalidated
            Thread.sleep(2000);

            // User should no longer be able to read config
            ConfigService userConfig2 = createConfigService(user, TEST_PASSWORD);
            boolean denied = false;
            try {
                String revokedContent = userConfig2.getConfig(dataId, "DEFAULT_GROUP", 5000);
                denied = (revokedContent == null || revokedContent.isEmpty());
            } catch (NacosException e) {
                denied = true;
            }
            assertTrue(denied, "User should NOT be able to read config after permission revocation");

            userConfig2.shutDown();
            adminConfig.removeConfig(dataId, "DEFAULT_GROUP");
            adminConfig.shutDown();
        } finally {
            try { httpDelete("/nacos/v3/auth/role?role=" + role + "&username=" + user); } catch (Exception ignored) {}
            try { httpDelete("/nacos/v3/auth/user?username=" + user); } catch (Exception ignored) {}
        }
    }

    // ==================== Helper Methods ====================

    private void cleanupUserWithSpecificPermission(String username, String role, String resource, String action) {
        try {
            httpDelete("/nacos/v3/auth/permission?role=" + URLEncoder.encode(role, "UTF-8")
                    + "&resource=" + URLEncoder.encode(resource, "UTF-8")
                    + "&action=" + URLEncoder.encode(action, "UTF-8"));
        } catch (Exception ignored) {}
        try {
            httpDelete("/nacos/v3/auth/role?role=" + role + "&username=" + username);
        } catch (Exception ignored) {}
        try {
            httpDelete("/nacos/v3/auth/user?username=" + username);
        } catch (Exception ignored) {}
    }

    private void setupUserWithPermission(String username, String role, String resource, String action) throws Exception {
        String createUserResponse = httpPost("/nacos/v3/auth/user",
                "username=" + URLEncoder.encode(username, "UTF-8")
                        + "&password=" + URLEncoder.encode(TEST_PASSWORD, "UTF-8"));
        JsonNode createUserJson = objectMapper.readTree(createUserResponse);
        assertEquals(0, createUserJson.get("code").asInt(),
                "User creation should succeed: " + createUserResponse);

        String createRoleResponse = httpPost("/nacos/v3/auth/role",
                "role=" + URLEncoder.encode(role, "UTF-8")
                        + "&username=" + URLEncoder.encode(username, "UTF-8"));
        JsonNode createRoleJson = objectMapper.readTree(createRoleResponse);
        assertEquals(0, createRoleJson.get("code").asInt(),
                "Role creation should succeed: " + createRoleResponse);

        String createPermResponse = httpPost("/nacos/v3/auth/permission",
                "role=" + URLEncoder.encode(role, "UTF-8")
                        + "&resource=" + URLEncoder.encode(resource, "UTF-8")
                        + "&action=" + URLEncoder.encode(action, "UTF-8"));
        JsonNode createPermJson = objectMapper.readTree(createPermResponse);
        assertEquals(0, createPermJson.get("code").asInt(),
                "Permission creation should succeed: " + createPermResponse);
    }

    private void cleanupUserWithPermission(String username, String role) {
        try {
            httpDelete("/nacos/v3/auth/permission?role=" + URLEncoder.encode(role, "UTF-8")
                    + "&resource=" + URLEncoder.encode("public:DEFAULT_GROUP:*", "UTF-8")
                    + "&action=rw");
        } catch (Exception ignored) {}
        try {
            httpDelete("/nacos/v3/auth/permission?role=" + URLEncoder.encode(role, "UTF-8")
                    + "&resource=" + URLEncoder.encode("public:DEFAULT_GROUP:*", "UTF-8")
                    + "&action=r");
        } catch (Exception ignored) {}
        try {
            httpDelete("/nacos/v3/auth/permission?role=" + URLEncoder.encode(role, "UTF-8")
                    + "&resource=" + URLEncoder.encode("public:DEFAULT_GROUP:*", "UTF-8")
                    + "&action=w");
        } catch (Exception ignored) {}
        try {
            httpDelete("/nacos/v3/auth/permission?role=" + URLEncoder.encode(role, "UTF-8")
                    + "&resource=" + URLEncoder.encode("public:*:*", "UTF-8")
                    + "&action=r");
        } catch (Exception ignored) {}
        try {
            httpDelete("/nacos/v3/auth/role?role=" + role + "&username=" + username);
        } catch (Exception ignored) {}
        try {
            httpDelete("/nacos/v3/auth/user?username=" + username);
        } catch (Exception ignored) {}
    }

    private ConfigService createConfigService(String username, String password) throws NacosException {
        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);
        return NacosFactory.createConfigService(properties);
    }

    private NamingService createNamingService(String username, String password) throws NacosException {
        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);
        return NacosFactory.createNamingService(properties);
    }

    private ConfigService createConfigServiceWithNamespace(String username, String password, String namespace) throws NacosException {
        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);
        properties.setProperty("namespace", namespace);
        return NacosFactory.createConfigService(properties);
    }

    private NamingService createNamingServiceWithNamespace(String username, String password, String namespace) throws NacosException {
        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);
        properties.setProperty("namespace", namespace);
        return NacosFactory.createNamingService(properties);
    }

    private void cleanupUserWithPermissionForNamespace(String username, String role, String namespace) {
        String nsResource = namespace + ":DEFAULT_GROUP:*";
        try {
            httpDelete("/nacos/v3/auth/permission?role=" + URLEncoder.encode(role, "UTF-8")
                    + "&resource=" + URLEncoder.encode(nsResource, "UTF-8")
                    + "&action=rw");
        } catch (Exception ignored) {}
        try {
            httpDelete("/nacos/v3/auth/permission?role=" + URLEncoder.encode(role, "UTF-8")
                    + "&resource=" + URLEncoder.encode(nsResource, "UTF-8")
                    + "&action=r");
        } catch (Exception ignored) {}
        try {
            httpDelete("/nacos/v3/auth/permission?role=" + URLEncoder.encode(role, "UTF-8")
                    + "&resource=" + URLEncoder.encode(nsResource, "UTF-8")
                    + "&action=w");
        } catch (Exception ignored) {}
        try {
            httpDelete("/nacos/v3/auth/role?role=" + role + "&username=" + username);
        } catch (Exception ignored) {}
        try {
            httpDelete("/nacos/v3/auth/user?username=" + username);
        } catch (Exception ignored) {}
    }

    private static String loginV3(String username, String password) throws Exception {
        String loginUrl = String.format("http://%s/nacos/v3/auth/user/login", serverAddr);
        URL url = new URL(loginUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("POST");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");

        String body = "username=" + URLEncoder.encode(username, "UTF-8")
                + "&password=" + URLEncoder.encode(password, "UTF-8");
        conn.getOutputStream().write(body.getBytes(StandardCharsets.UTF_8));

        if (conn.getResponseCode() == 200) {
            BufferedReader reader = new BufferedReader(new InputStreamReader(conn.getInputStream()));
            StringBuilder response = new StringBuilder();
            String line;
            while ((line = reader.readLine()) != null) {
                response.append(line);
            }
            JsonNode json = objectMapper.readTree(response.toString());
            if (json.has("data") && json.get("data").has("accessToken")) {
                return json.get("data").get("accessToken").asText();
            }
            if (json.has("accessToken")) {
                return json.get("accessToken").asText();
            }
        }
        return "";
    }

    private static String httpGet(String path) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }
        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("GET");
        return readResponse(conn);
    }

    private static String httpPost(String path, String body) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }
        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("POST");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");
        if (body != null && !body.isEmpty()) {
            try (OutputStream os = conn.getOutputStream()) {
                os.write(body.getBytes(StandardCharsets.UTF_8));
            }
        }
        return readResponse(conn);
    }

    private static String httpPut(String path, String body) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }
        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("PUT");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");
        if (body != null && !body.isEmpty()) {
            try (OutputStream os = conn.getOutputStream()) {
                os.write(body.getBytes(StandardCharsets.UTF_8));
            }
        }
        return readResponse(conn);
    }

    private static String httpDelete(String path) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }
        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("DELETE");
        return readResponse(conn);
    }

    private static String readResponse(HttpURLConnection conn) throws Exception {
        int responseCode = conn.getResponseCode();
        java.io.InputStream stream = responseCode >= 400 ? conn.getErrorStream() : conn.getInputStream();
        StringBuilder response = new StringBuilder();
        if (stream != null) {
            BufferedReader reader = new BufferedReader(new InputStreamReader(stream));
            String line;
            while ((line = reader.readLine()) != null) {
                response.append(line);
            }
        }
        return response.toString();
    }
}
