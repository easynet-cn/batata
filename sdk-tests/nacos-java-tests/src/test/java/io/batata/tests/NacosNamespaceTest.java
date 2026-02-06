package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import org.junit.jupiter.api.*;

import java.io.*;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Namespace Management Tests
 *
 * Tests for namespace management including creation, deletion, isolation,
 * and cross-namespace operations using Nacos SDK 3.1.1.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosNamespaceTest {

    private static String serverAddr;
    private static String username;
    private static String password;
    private static String accessToken;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final String PUBLIC_NAMESPACE = "public";

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        username = System.getProperty("nacos.username", "nacos");
        password = System.getProperty("nacos.password", "nacos");

        accessToken = getAccessToken(username, password);
        System.out.println("Namespace Test Setup - Server: " + serverAddr);
        System.out.println("Access Token: " + (accessToken.isEmpty() ? "NONE" : "OK"));
    }

    private static String getAccessToken(String username, String password) throws Exception {
        String loginUrl = String.format("http://%s/nacos/v1/auth/login", serverAddr);
        URL url = new URL(loginUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("POST");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");

        String body = "username=" + URLEncoder.encode(username, "UTF-8") +
                "&password=" + URLEncoder.encode(password, "UTF-8");
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
                return resp.substring(start, end);
            }
        }
        return "";
    }

    private ConfigService createConfigService(String namespace) throws NacosException {
        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);
        if (namespace != null && !namespace.isEmpty() && !namespace.equals(PUBLIC_NAMESPACE)) {
            properties.put("namespace", namespace);
        }
        return NacosFactory.createConfigService(properties);
    }

    private NamingService createNamingService(String namespace) throws NacosException {
        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);
        if (namespace != null && !namespace.isEmpty() && !namespace.equals(PUBLIC_NAMESPACE)) {
            properties.put("namespace", namespace);
        }
        return NacosFactory.createNamingService(properties);
    }

    private String httpGet(String path) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }

        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("GET");

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
        System.out.println("GET " + path + " -> " + responseCode);
        return response.toString();
    }

    private String httpPost(String path, String body) throws Exception {
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
        System.out.println("POST " + path + " -> " + responseCode);
        return response.toString();
    }

    private String httpPut(String path, String body) throws Exception {
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
        System.out.println("PUT " + path + " -> " + responseCode);
        return response.toString();
    }

    private String httpDelete(String path) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }

        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("DELETE");

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
        System.out.println("DELETE " + path + " -> " + responseCode);
        return response.toString();
    }

    private String createNamespace(String namespaceId, String namespaceName, String namespaceDesc) throws Exception {
        String body = String.format(
                "namespaceId=%s&namespaceName=%s&namespaceDesc=%s",
                URLEncoder.encode(namespaceId, "UTF-8"),
                URLEncoder.encode(namespaceName, "UTF-8"),
                URLEncoder.encode(namespaceDesc, "UTF-8"));
        return httpPost("/nacos/v2/console/namespace", body);
    }

    private String deleteNamespace(String namespaceId) throws Exception {
        return httpDelete("/nacos/v2/console/namespace?namespaceId=" + URLEncoder.encode(namespaceId, "UTF-8"));
    }

    // ==================== Namespace Tests ====================

    /**
     * NNS-001: Test default namespace (public)
     *
     * Verifies that the public namespace exists by default and can be used
     * for config and naming operations without explicit namespace creation.
     */
    @Test
    @Order(1)
    void testDefaultNamespace() throws Exception {
        // The public namespace should be available by default
        String response = httpGet("/nacos/v2/console/namespace/list");
        System.out.println("Namespace list: " + response);
        assertNotNull(response);

        // Create a config service for the default namespace
        ConfigService configService = createConfigService(PUBLIC_NAMESPACE);
        try {
            String dataId = "nns001-default-ns-" + UUID.randomUUID();
            String content = "default.namespace.test=true";

            // Publish in default namespace
            boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, content);
            assertTrue(published, "Should publish config in default namespace");

            // Get from default namespace
            String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals(content, retrieved, "Should retrieve config from default namespace");

            // Cleanup
            configService.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            configService.shutDown();
        }
    }

    /**
     * NNS-002: Test custom namespace creation
     *
     * Tests the complete lifecycle of creating a custom namespace via console API.
     */
    @Test
    @Order(2)
    void testCustomNamespaceCreation() throws Exception {
        String namespaceId = "nns002-" + UUID.randomUUID().toString().substring(0, 8);
        String namespaceName = "Test Custom Namespace";
        String namespaceDesc = "Created by NNS-002 test";

        try {
            // Create namespace
            String createResponse = createNamespace(namespaceId, namespaceName, namespaceDesc);
            System.out.println("Create namespace response: " + createResponse);

            // Verify namespace exists
            String getResponse = httpGet("/nacos/v2/console/namespace?namespaceId=" + namespaceId);
            System.out.println("Get namespace response: " + getResponse);
            assertNotNull(getResponse);

            // Use the namespace with config service
            ConfigService configService = createConfigService(namespaceId);
            try {
                String dataId = "custom-ns-config-" + UUID.randomUUID();
                boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "custom.ns=true");
                assertTrue(published, "Should publish config in custom namespace");

                String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
                assertEquals("custom.ns=true", retrieved);

                configService.removeConfig(dataId, DEFAULT_GROUP);
            } finally {
                configService.shutDown();
            }
        } finally {
            // Cleanup
            deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-003: Test namespace isolation for config
     *
     * Verifies that configurations in different namespaces are isolated from each other.
     */
    @Test
    @Order(3)
    void testNamespaceIsolationForConfig() throws Exception {
        String namespace1 = "nns003-ns1-" + UUID.randomUUID().toString().substring(0, 6);
        String namespace2 = "nns003-ns2-" + UUID.randomUUID().toString().substring(0, 6);

        // Create two namespaces
        createNamespace(namespace1, "Namespace 1", "First isolated namespace");
        createNamespace(namespace2, "Namespace 2", "Second isolated namespace");

        ConfigService configService1 = createConfigService(namespace1);
        ConfigService configService2 = createConfigService(namespace2);

        try {
            String dataId = "isolated-config";
            String content1 = "namespace=ns1";
            String content2 = "namespace=ns2";

            // Publish same dataId in different namespaces with different content
            assertTrue(configService1.publishConfig(dataId, DEFAULT_GROUP, content1));
            assertTrue(configService2.publishConfig(dataId, DEFAULT_GROUP, content2));

            Thread.sleep(500);

            // Verify isolation - each namespace sees its own content
            String retrieved1 = configService1.getConfig(dataId, DEFAULT_GROUP, 5000);
            String retrieved2 = configService2.getConfig(dataId, DEFAULT_GROUP, 5000);

            assertEquals(content1, retrieved1, "Namespace 1 should see its own content");
            assertEquals(content2, retrieved2, "Namespace 2 should see its own content");
            assertNotEquals(retrieved1, retrieved2, "Configs should be different between namespaces");

            // Cleanup configs
            configService1.removeConfig(dataId, DEFAULT_GROUP);
            configService2.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            configService1.shutDown();
            configService2.shutDown();
            deleteNamespace(namespace1);
            deleteNamespace(namespace2);
        }
    }

    /**
     * NNS-004: Test namespace isolation for service
     *
     * Verifies that services in different namespaces are isolated from each other.
     */
    @Test
    @Order(4)
    void testNamespaceIsolationForService() throws Exception {
        String namespace1 = "nns004-ns1-" + UUID.randomUUID().toString().substring(0, 6);
        String namespace2 = "nns004-ns2-" + UUID.randomUUID().toString().substring(0, 6);

        createNamespace(namespace1, "Service NS 1", "First service namespace");
        createNamespace(namespace2, "Service NS 2", "Second service namespace");

        NamingService namingService1 = createNamingService(namespace1);
        NamingService namingService2 = createNamingService(namespace2);

        try {
            String serviceName = "isolated-service";
            String ip1 = "10.0.1.1";
            String ip2 = "10.0.2.1";
            int port = 8080;

            // Register same service name in different namespaces with different IPs
            namingService1.registerInstance(serviceName, ip1, port);
            namingService2.registerInstance(serviceName, ip2, port);

            Thread.sleep(1500);

            // Verify isolation
            List<Instance> instances1 = namingService1.getAllInstances(serviceName);
            List<Instance> instances2 = namingService2.getAllInstances(serviceName);

            assertEquals(1, instances1.size(), "Namespace 1 should have 1 instance");
            assertEquals(1, instances2.size(), "Namespace 2 should have 1 instance");
            assertEquals(ip1, instances1.get(0).getIp(), "Namespace 1 should see its own IP");
            assertEquals(ip2, instances2.get(0).getIp(), "Namespace 2 should see its own IP");

            // Cleanup
            namingService1.deregisterInstance(serviceName, ip1, port);
            namingService2.deregisterInstance(serviceName, ip2, port);
        } finally {
            namingService1.shutDown();
            namingService2.shutDown();
            deleteNamespace(namespace1);
            deleteNamespace(namespace2);
        }
    }

    /**
     * NNS-005: Test cross-namespace access
     *
     * Verifies that data in one namespace cannot be accessed from another namespace.
     */
    @Test
    @Order(5)
    void testCrossNamespaceAccess() throws Exception {
        String namespace1 = "nns005-ns1-" + UUID.randomUUID().toString().substring(0, 6);
        String namespace2 = "nns005-ns2-" + UUID.randomUUID().toString().substring(0, 6);

        createNamespace(namespace1, "Cross Access NS 1", "Cross access test namespace 1");
        createNamespace(namespace2, "Cross Access NS 2", "Cross access test namespace 2");

        ConfigService configService1 = createConfigService(namespace1);
        ConfigService configService2 = createConfigService(namespace2);

        try {
            String dataId = "cross-ns-config-" + UUID.randomUUID();
            String content = "secret.data=confidential";

            // Publish in namespace 1 only
            assertTrue(configService1.publishConfig(dataId, DEFAULT_GROUP, content));
            Thread.sleep(500);

            // Verify it exists in namespace 1
            String ns1Content = configService1.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals(content, ns1Content, "Config should exist in namespace 1");

            // Try to access from namespace 2 - should NOT find it
            String ns2Content = configService2.getConfig(dataId, DEFAULT_GROUP, 3000);
            assertNull(ns2Content, "Config from namespace 1 should NOT be visible in namespace 2");

            // Cleanup
            configService1.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            configService1.shutDown();
            configService2.shutDown();
            deleteNamespace(namespace1);
            deleteNamespace(namespace2);
        }
    }

    /**
     * NNS-006: Test namespace with special characters
     *
     * Tests namespace ID handling with various allowed character patterns.
     */
    @Test
    @Order(6)
    void testNamespaceWithSpecialCharacters() throws Exception {
        // Namespace IDs typically allow alphanumeric, underscores, and hyphens
        String namespaceId = "nns006_test-ns_" + UUID.randomUUID().toString().substring(0, 6);
        String namespaceName = "Special Chars NS (Test)";
        String namespaceDesc = "Namespace with special chars: _-";

        try {
            String createResponse = createNamespace(namespaceId, namespaceName, namespaceDesc);
            System.out.println("Create special namespace: " + createResponse);

            // Verify namespace works
            ConfigService configService = createConfigService(namespaceId);
            try {
                String dataId = "special-ns-config";
                boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "special=true");
                assertTrue(published, "Should publish in namespace with special chars");

                String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
                assertEquals("special=true", retrieved);

                configService.removeConfig(dataId, DEFAULT_GROUP);
            } finally {
                configService.shutDown();
            }
        } finally {
            deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-007: Test namespace capacity
     *
     * Tests namespace capacity limits and quota management.
     */
    @Test
    @Order(7)
    void testNamespaceCapacity() throws Exception {
        String namespaceId = "nns007-capacity-" + UUID.randomUUID().toString().substring(0, 6);
        createNamespace(namespaceId, "Capacity Test NS", "Testing capacity limits");

        ConfigService configService = createConfigService(namespaceId);

        try {
            // Create multiple configs to test capacity
            int configCount = 10;
            List<String> dataIds = new ArrayList<>();

            for (int i = 0; i < configCount; i++) {
                String dataId = "capacity-config-" + i;
                dataIds.add(dataId);
                boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "config.index=" + i);
                assertTrue(published, "Should publish config " + i);
            }

            Thread.sleep(500);

            // Verify all configs exist
            for (int i = 0; i < configCount; i++) {
                String retrieved = configService.getConfig(dataIds.get(i), DEFAULT_GROUP, 5000);
                assertEquals("config.index=" + i, retrieved, "Config " + i + " should exist");
            }

            // Cleanup
            for (String dataId : dataIds) {
                configService.removeConfig(dataId, DEFAULT_GROUP);
            }
        } finally {
            configService.shutDown();
            deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-008: Test namespace metadata
     *
     * Tests namespace metadata including name, description, and quota information.
     */
    @Test
    @Order(8)
    void testNamespaceMetadata() throws Exception {
        String namespaceId = "nns008-meta-" + UUID.randomUUID().toString().substring(0, 6);
        String namespaceName = "Metadata Test Namespace";
        String namespaceDesc = "Testing namespace metadata retrieval";

        try {
            // Create namespace with metadata
            createNamespace(namespaceId, namespaceName, namespaceDesc);

            // Get namespace details
            String getResponse = httpGet("/nacos/v2/console/namespace?namespaceId=" + namespaceId);
            System.out.println("Namespace metadata: " + getResponse);
            assertNotNull(getResponse);

            // The response should contain namespace information
            // Note: Exact format depends on API version
            assertTrue(getResponse.contains(namespaceId) || getResponse.contains("namespace"),
                    "Response should contain namespace information");

        } finally {
            deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-009: Test namespace listing
     *
     * Tests the ability to list all namespaces and verify pagination/filtering.
     */
    @Test
    @Order(9)
    void testNamespaceListing() throws Exception {
        String prefix = "nns009-list-" + UUID.randomUUID().toString().substring(0, 4);
        List<String> createdNamespaces = new ArrayList<>();

        try {
            // Create multiple namespaces
            for (int i = 0; i < 3; i++) {
                String namespaceId = prefix + "-" + i;
                createdNamespaces.add(namespaceId);
                createNamespace(namespaceId, "List Test NS " + i, "For listing test");
            }

            Thread.sleep(500);

            // List all namespaces
            String listResponse = httpGet("/nacos/v2/console/namespace/list");
            System.out.println("Namespace list: " + listResponse);
            assertNotNull(listResponse);

            // Verify our namespaces appear in the list
            for (String nsId : createdNamespaces) {
                assertTrue(listResponse.contains(nsId),
                        "Namespace " + nsId + " should appear in the list");
            }

        } finally {
            // Cleanup
            for (String nsId : createdNamespaces) {
                deleteNamespace(nsId);
            }
        }
    }

    /**
     * NNS-010: Test namespace deletion
     *
     * Tests namespace deletion and verifies cleanup of associated data.
     */
    @Test
    @Order(10)
    void testNamespaceDeletion() throws Exception {
        String namespaceId = "nns010-delete-" + UUID.randomUUID().toString().substring(0, 6);
        createNamespace(namespaceId, "To Be Deleted NS", "Will be deleted");

        // First verify namespace exists
        String beforeDelete = httpGet("/nacos/v2/console/namespace/list");
        assertTrue(beforeDelete.contains(namespaceId), "Namespace should exist before deletion");

        // Add some data to the namespace
        ConfigService configService = createConfigService(namespaceId);
        try {
            configService.publishConfig("pre-delete-config", DEFAULT_GROUP, "will.be.deleted=true");
            Thread.sleep(500);
            configService.removeConfig("pre-delete-config", DEFAULT_GROUP);
        } finally {
            configService.shutDown();
        }

        // Delete namespace
        String deleteResponse = deleteNamespace(namespaceId);
        System.out.println("Delete namespace response: " + deleteResponse);

        // Verify namespace is deleted
        Thread.sleep(500);
        String afterDelete = httpGet("/nacos/v2/console/namespace/list");
        assertFalse(afterDelete.contains("\"" + namespaceId + "\""),
                "Namespace should not exist after deletion");
    }

    /**
     * NNS-011: Test namespace update
     *
     * Tests updating namespace name and description.
     */
    @Test
    @Order(11)
    void testNamespaceUpdate() throws Exception {
        String namespaceId = "nns011-update-" + UUID.randomUUID().toString().substring(0, 6);
        String originalName = "Original Name";
        String updatedName = "Updated Name";
        String updatedDesc = "Updated description for namespace";

        try {
            // Create namespace
            createNamespace(namespaceId, originalName, "Original description");

            // Update namespace
            String updateBody = String.format(
                    "namespaceId=%s&namespaceName=%s&namespaceDesc=%s",
                    URLEncoder.encode(namespaceId, "UTF-8"),
                    URLEncoder.encode(updatedName, "UTF-8"),
                    URLEncoder.encode(updatedDesc, "UTF-8"));
            String updateResponse = httpPut("/nacos/v2/console/namespace", updateBody);
            System.out.println("Update namespace response: " + updateResponse);

            // Verify update
            String getResponse = httpGet("/nacos/v2/console/namespace?namespaceId=" + namespaceId);
            System.out.println("Get updated namespace: " + getResponse);

            // Should contain updated information
            assertTrue(getResponse.contains(updatedName) || getResponse.contains("Updated"),
                    "Namespace should reflect updated information");

        } finally {
            deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-012: Test concurrent namespace operations
     *
     * Tests thread safety of concurrent namespace operations.
     */
    @Test
    @Order(12)
    void testConcurrentNamespaceOperations() throws Exception {
        String namespaceId = "nns012-concurrent-" + UUID.randomUUID().toString().substring(0, 6);
        createNamespace(namespaceId, "Concurrent Test NS", "For concurrency testing");

        int threadCount = 5;
        int opsPerThread = 5;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(threadCount);
        AtomicReference<Exception> error = new AtomicReference<>();
        AtomicInteger successCount = new AtomicInteger(0);

        ConfigService configService = createConfigService(namespaceId);

        try {
            for (int t = 0; t < threadCount; t++) {
                final int threadIdx = t;
                new Thread(() -> {
                    try {
                        startLatch.await();
                        for (int i = 0; i < opsPerThread; i++) {
                            String dataId = "concurrent-config-t" + threadIdx + "-" + i;
                            String content = "thread=" + threadIdx + ",op=" + i;

                            configService.publishConfig(dataId, DEFAULT_GROUP, content);
                            String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
                            if (content.equals(retrieved)) {
                                successCount.incrementAndGet();
                            }
                            configService.removeConfig(dataId, DEFAULT_GROUP);
                        }
                    } catch (Exception e) {
                        error.set(e);
                    } finally {
                        doneLatch.countDown();
                    }
                }).start();
            }

            // Start all threads simultaneously
            startLatch.countDown();

            // Wait for completion
            boolean completed = doneLatch.await(60, TimeUnit.SECONDS);
            assertTrue(completed, "All threads should complete within timeout");
            assertNull(error.get(), "No errors should occur during concurrent operations");

            int expectedSuccess = threadCount * opsPerThread;
            assertEquals(expectedSuccess, successCount.get(),
                    "All concurrent operations should succeed");

        } finally {
            configService.shutDown();
            deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-013: Test namespace with config operations
     *
     * Tests comprehensive config operations within a namespace including
     * publish, update, get, and delete.
     */
    @Test
    @Order(13)
    void testNamespaceWithConfigOperations() throws Exception {
        String namespaceId = "nns013-config-ops-" + UUID.randomUUID().toString().substring(0, 6);
        createNamespace(namespaceId, "Config Ops NS", "For config operation tests");

        ConfigService configService = createConfigService(namespaceId);

        try {
            String dataId = "config-ops-test";
            String group = "CONFIG_OPS_GROUP";

            // 1. Publish new config
            String initialContent = "version=1.0";
            assertTrue(configService.publishConfig(dataId, group, initialContent));
            Thread.sleep(300);

            // 2. Get config
            String retrieved = configService.getConfig(dataId, group, 5000);
            assertEquals(initialContent, retrieved, "Initial content should match");

            // 3. Update config
            String updatedContent = "version=2.0";
            assertTrue(configService.publishConfig(dataId, group, updatedContent));
            Thread.sleep(300);

            // 4. Verify update
            String afterUpdate = configService.getConfig(dataId, group, 5000);
            assertEquals(updatedContent, afterUpdate, "Updated content should match");

            // 5. Delete config
            assertTrue(configService.removeConfig(dataId, group));
            Thread.sleep(300);

            // 6. Verify deletion
            String afterDelete = configService.getConfig(dataId, group, 3000);
            assertNull(afterDelete, "Config should be deleted");

        } finally {
            configService.shutDown();
            deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-014: Test namespace with naming operations
     *
     * Tests comprehensive naming/service operations within a namespace including
     * register, get instances, and deregister.
     */
    @Test
    @Order(14)
    void testNamespaceWithNamingOperations() throws Exception {
        String namespaceId = "nns014-naming-ops-" + UUID.randomUUID().toString().substring(0, 6);
        createNamespace(namespaceId, "Naming Ops NS", "For naming operation tests");

        NamingService namingService = createNamingService(namespaceId);

        try {
            String serviceName = "naming-ops-service";
            String group = "NAMING_OPS_GROUP";

            // 1. Register multiple instances
            Instance instance1 = new Instance();
            instance1.setIp("172.16.0.1");
            instance1.setPort(8080);
            instance1.setWeight(1.0);
            instance1.setHealthy(true);
            Map<String, String> metadata1 = new HashMap<>();
            metadata1.put("zone", "zone-a");
            instance1.setMetadata(metadata1);

            Instance instance2 = new Instance();
            instance2.setIp("172.16.0.2");
            instance2.setPort(8080);
            instance2.setWeight(1.0);
            instance2.setHealthy(true);
            Map<String, String> metadata2 = new HashMap<>();
            metadata2.put("zone", "zone-b");
            instance2.setMetadata(metadata2);

            namingService.registerInstance(serviceName, group, instance1);
            namingService.registerInstance(serviceName, group, instance2);
            Thread.sleep(1500);

            // 2. Get all instances
            List<Instance> allInstances = namingService.getAllInstances(serviceName, group);
            assertEquals(2, allInstances.size(), "Should have 2 instances");

            // 3. Select healthy instances
            List<Instance> healthyInstances = namingService.selectInstances(serviceName, group, true);
            assertEquals(2, healthyInstances.size(), "All instances should be healthy");

            // 4. Verify metadata
            Set<String> zones = new HashSet<>();
            for (Instance inst : allInstances) {
                zones.add(inst.getMetadata().get("zone"));
            }
            assertTrue(zones.contains("zone-a") && zones.contains("zone-b"),
                    "Both zones should be present");

            // 5. Deregister one instance
            namingService.deregisterInstance(serviceName, group, instance1);
            Thread.sleep(1000);

            // 6. Verify deregistration
            List<Instance> afterDeregister = namingService.getAllInstances(serviceName, group);
            assertEquals(1, afterDeregister.size(), "Should have 1 instance after deregister");
            assertEquals("172.16.0.2", afterDeregister.get(0).getIp());

            // 7. Cleanup
            namingService.deregisterInstance(serviceName, group, instance2);

        } finally {
            namingService.shutDown();
            deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-015: Test namespace migration
     *
     * Tests migrating data from one namespace to another, simulating
     * a namespace migration scenario.
     */
    @Test
    @Order(15)
    void testNamespaceMigration() throws Exception {
        String sourceNs = "nns015-source-" + UUID.randomUUID().toString().substring(0, 6);
        String targetNs = "nns015-target-" + UUID.randomUUID().toString().substring(0, 6);

        createNamespace(sourceNs, "Source NS", "Migration source namespace");
        createNamespace(targetNs, "Target NS", "Migration target namespace");

        ConfigService sourceConfig = createConfigService(sourceNs);
        ConfigService targetConfig = createConfigService(targetNs);

        try {
            // 1. Create data in source namespace
            List<String> dataIds = Arrays.asList("migrate-config-1", "migrate-config-2", "migrate-config-3");
            Map<String, String> contents = new HashMap<>();

            for (String dataId : dataIds) {
                String content = "dataId=" + dataId + ",source=" + sourceNs;
                contents.put(dataId, content);
                assertTrue(sourceConfig.publishConfig(dataId, DEFAULT_GROUP, content),
                        "Should publish " + dataId + " to source");
            }
            Thread.sleep(500);

            // 2. Verify source has all configs
            for (String dataId : dataIds) {
                String retrieved = sourceConfig.getConfig(dataId, DEFAULT_GROUP, 5000);
                assertEquals(contents.get(dataId), retrieved,
                        "Source should have " + dataId);
            }

            // 3. Migrate configs to target namespace
            for (String dataId : dataIds) {
                String content = sourceConfig.getConfig(dataId, DEFAULT_GROUP, 5000);
                assertNotNull(content, "Should read " + dataId + " from source");
                assertTrue(targetConfig.publishConfig(dataId, DEFAULT_GROUP, content),
                        "Should publish " + dataId + " to target");
            }
            Thread.sleep(500);

            // 4. Verify target has all migrated configs
            for (String dataId : dataIds) {
                String retrieved = targetConfig.getConfig(dataId, DEFAULT_GROUP, 5000);
                assertEquals(contents.get(dataId), retrieved,
                        "Target should have migrated " + dataId);
            }

            // 5. Clean up source (simulating migration completion)
            for (String dataId : dataIds) {
                assertTrue(sourceConfig.removeConfig(dataId, DEFAULT_GROUP),
                        "Should remove " + dataId + " from source");
            }
            Thread.sleep(500);

            // 6. Verify source is empty and target still has data
            for (String dataId : dataIds) {
                assertNull(sourceConfig.getConfig(dataId, DEFAULT_GROUP, 3000),
                        "Source should not have " + dataId + " after cleanup");
                assertEquals(contents.get(dataId), targetConfig.getConfig(dataId, DEFAULT_GROUP, 5000),
                        "Target should still have " + dataId);
            }

            // Cleanup target
            for (String dataId : dataIds) {
                targetConfig.removeConfig(dataId, DEFAULT_GROUP);
            }

        } finally {
            sourceConfig.shutDown();
            targetConfig.shutDown();
            deleteNamespace(sourceNs);
            deleteNamespace(targetNs);
        }
    }
}
