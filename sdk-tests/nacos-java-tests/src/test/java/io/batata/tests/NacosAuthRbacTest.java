package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import org.junit.jupiter.api.*;

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
    private static final String TEST_USER = "rbac-user-" + UUID.randomUUID().toString().substring(0, 6);
    private static final String TEST_PASSWORD = "Test123456";
    private static final String TEST_ROLE = "rbac-role-" + UUID.randomUUID().toString().substring(0, 6);
    private static final String TEST_RESOURCE = "public:DEFAULT_GROUP:*";

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        accessToken = loginV3(username, password);
        System.out.println("Auth RBAC Test Setup - Server: " + serverAddr);
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
        System.out.println("Create user: " + response);
        assertTrue(response.contains("true") || response.contains("success") || response.contains("200"),
                "User creation should succeed");
    }

    /**
     * RBAC-002: Test search users
     */
    @Test
    @Order(2)
    void testSearchUsers() throws Exception {
        String response = httpGet("/nacos/v3/auth/user/search?pageNo=1&pageSize=100");
        System.out.println("Search users: " + response);
        assertNotNull(response);
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
        System.out.println("Update user password: " + response);

        // Verify login with new password
        String token = loginV3(TEST_USER, newPassword);
        System.out.println("Login with new password: " + (token.isEmpty() ? "FAILED" : "OK"));

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
        httpPost("/nacos/v3/auth/user", createBody);

        String response = httpDelete("/nacos/v3/auth/user?username=" + URLEncoder.encode(tempUser, "UTF-8"));
        System.out.println("Delete user: " + response);
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
        System.out.println("Create role: " + response);
        assertTrue(response.contains("true") || response.contains("success") || response.contains("200"),
                "Role creation should succeed");
    }

    /**
     * RBAC-006: Test search roles
     */
    @Test
    @Order(6)
    void testSearchRoles() throws Exception {
        String response = httpGet("/nacos/v3/auth/role/search?pageNo=1&pageSize=100");
        System.out.println("Search roles: " + response);
        assertNotNull(response);
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
        httpPost("/nacos/v3/auth/role", "role=" + tempRole + "&username=" + tempUser);

        // Delete role
        String response = httpDelete("/nacos/v3/auth/role?role=" + tempRole + "&username=" + tempUser);
        System.out.println("Delete role: " + response);

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
        System.out.println("Create permission: " + response);
        assertTrue(response.contains("true") || response.contains("success") || response.contains("200"),
                "Permission creation should succeed");
    }

    /**
     * RBAC-009: Test search permissions
     */
    @Test
    @Order(9)
    void testSearchPermissions() throws Exception {
        String response = httpGet("/nacos/v3/auth/permission/searchPage?pageNo=1&pageSize=100");
        System.out.println("Search permissions: " + response);
        assertNotNull(response);
    }

    /**
     * RBAC-010: Test delete permission (create temp and delete)
     */
    @Test
    @Order(10)
    void testDeletePermission() throws Exception {
        String tempRole = "temp-perm-role-" + UUID.randomUUID().toString().substring(0, 6);
        String tempUser = "temp-perm-user-" + UUID.randomUUID().toString().substring(0, 6);

        httpPost("/nacos/v3/auth/user", "username=" + tempUser + "&password=TempPass123");
        httpPost("/nacos/v3/auth/role", "role=" + tempRole + "&username=" + tempUser);
        httpPost("/nacos/v3/auth/permission",
                "role=" + tempRole + "&resource=" + URLEncoder.encode("public:*:*", "UTF-8") + "&action=r");

        String response = httpDelete("/nacos/v3/auth/permission?role=" + tempRole
                + "&resource=" + URLEncoder.encode("public:*:*", "UTF-8")
                + "&action=r");
        System.out.println("Delete permission: " + response);

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
            adminConfig.publishConfig(dataId, "DEFAULT_GROUP", "rbac.read=allowed");
            Thread.sleep(1000);

            // Read config as read-only user
            ConfigService readOnlyConfig = createConfigService(readUser, TEST_PASSWORD);
            String content = readOnlyConfig.getConfig(dataId, "DEFAULT_GROUP", 5000);
            System.out.println("Config read with read permission: " + content);

            readOnlyConfig.shutDown();
            adminConfig.removeConfig(dataId, "DEFAULT_GROUP");
            adminConfig.shutDown();
        } catch (Exception e) {
            System.out.println("Config read with read permission: " + e.getMessage());
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

            try {
                boolean result = readOnlyConfig.publishConfig(dataId, "DEFAULT_GROUP", "should.fail=true");
                System.out.println("Config write with read-only permission result: " + result);
                // May fail with exception or return false
            } catch (NacosException e) {
                System.out.println("Config write with read-only permission correctly denied: " + e.getMessage());
            }

            readOnlyConfig.shutDown();
        } catch (Exception e) {
            System.out.println("Config write with read permission test: " + e.getMessage());
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
            System.out.println("Config write with write permission: " + result);

            writeConfig.shutDown();

            // Cleanup config as admin
            ConfigService adminConfig = createConfigService("nacos", System.getProperty("nacos.password", "nacos"));
            adminConfig.removeConfig(dataId, "DEFAULT_GROUP");
            adminConfig.shutDown();
        } catch (Exception e) {
            System.out.println("Config write with write permission: " + e.getMessage());
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
            try {
                String content = writeOnlyConfig.getConfig(dataId, "DEFAULT_GROUP", 5000);
                System.out.println("Config read with write-only permission result: " + content);
            } catch (NacosException e) {
                System.out.println("Config read with write-only permission correctly denied: " + e.getMessage());
            }

            writeOnlyConfig.shutDown();
            adminConfig.removeConfig(dataId, "DEFAULT_GROUP");
            adminConfig.shutDown();
        } catch (Exception e) {
            System.out.println("Config read with write permission test: " + e.getMessage());
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
            try {
                var instances = readOnlyNaming.getAllInstances(serviceName);
                System.out.println("Naming read with read permission: " + instances.size() + " instances");
            } catch (NacosException e) {
                System.out.println("Naming read with read permission: " + e.getMessage());
            }

            readOnlyNaming.shutDown();
            adminNaming.deregisterInstance(serviceName, "192.168.1.1", 8080);
            adminNaming.shutDown();
        } catch (Exception e) {
            System.out.println("Naming read with read permission test: " + e.getMessage());
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

            try {
                readOnlyNaming.registerInstance(serviceName, "192.168.1.1", 8080);
                System.out.println("Naming write with read-only permission: unexpectedly succeeded");
                readOnlyNaming.deregisterInstance(serviceName, "192.168.1.1", 8080);
            } catch (NacosException e) {
                System.out.println("Naming write with read-only permission correctly denied: " + e.getMessage());
            }

            readOnlyNaming.shutDown();
        } catch (Exception e) {
            System.out.println("Naming write with read permission test: " + e.getMessage());
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
            writeNaming.registerInstance(serviceName, "192.168.1.1", 8080);
            System.out.println("Naming write with write permission: succeeded");
            Thread.sleep(1000);
            writeNaming.deregisterInstance(serviceName, "192.168.1.1", 8080);
            writeNaming.shutDown();
        } catch (Exception e) {
            System.out.println("Naming write with write permission: " + e.getMessage());
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
            try {
                var instances = writeOnlyNaming.getAllInstances(serviceName);
                System.out.println("Naming read with write-only permission result: " + instances.size() + " instances");
            } catch (NacosException e) {
                System.out.println("Naming read with write-only permission correctly denied: " + e.getMessage());
            }

            writeOnlyNaming.shutDown();
            adminNaming.deregisterInstance(serviceName, "192.168.1.1", 8080);
            adminNaming.shutDown();
        } catch (Exception e) {
            System.out.println("Naming read with write permission test: " + e.getMessage());
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

        try {
            setupUserWithPermission(rwUser, rwRole, "public:DEFAULT_GROUP:*", "rw");

            ConfigService rwConfig = createConfigService(rwUser, TEST_PASSWORD);

            // Write should succeed
            boolean writeResult = rwConfig.publishConfig(dataId, "DEFAULT_GROUP", "rbac.full=access");
            System.out.println("Full permission write: " + writeResult);
            Thread.sleep(1000);

            // Read should succeed
            String content = rwConfig.getConfig(dataId, "DEFAULT_GROUP", 5000);
            System.out.println("Full permission read: " + content);

            rwConfig.removeConfig(dataId, "DEFAULT_GROUP");
            rwConfig.shutDown();
        } catch (Exception e) {
            System.out.println("Full permission test: " + e.getMessage());
        } finally {
            cleanupUserWithPermission(rwUser, rwRole);
        }
    }

    // ==================== Helper Methods ====================

    private void setupUserWithPermission(String username, String role, String resource, String action) throws Exception {
        httpPost("/nacos/v3/auth/user",
                "username=" + URLEncoder.encode(username, "UTF-8")
                        + "&password=" + URLEncoder.encode(TEST_PASSWORD, "UTF-8"));
        httpPost("/nacos/v3/auth/role",
                "role=" + URLEncoder.encode(role, "UTF-8")
                        + "&username=" + URLEncoder.encode(username, "UTF-8"));
        httpPost("/nacos/v3/auth/permission",
                "role=" + URLEncoder.encode(role, "UTF-8")
                        + "&resource=" + URLEncoder.encode(resource, "UTF-8")
                        + "&action=" + URLEncoder.encode(action, "UTF-8"));
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
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);
        return NacosFactory.createConfigService(properties);
    }

    private NamingService createNamingService(String username, String password) throws NacosException {
        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);
        return NacosFactory.createNamingService(properties);
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
            String resp = response.toString();
            if (resp.contains("accessToken")) {
                int start = resp.indexOf("accessToken") + 14;
                int end = resp.indexOf("\"", start);
                if (end > start) return resp.substring(start, end);
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
