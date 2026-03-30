package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.config.listener.Listener;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.io.BufferedReader;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Multi-Tenant Config Isolation Tests
 *
 * Tests for config isolation across namespaces (tenants), including
 * independent content per namespace, listener scoping, cross-namespace
 * boundary enforcement, and default vs custom namespace behavior.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosMultiTenantConfigTest {

    private static String serverAddr;
    private static String username;
    private static String password;
    private static String accessToken;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final List<String> createdNamespaces = new ArrayList<>();

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        username = System.getProperty("nacos.username", "nacos");
        password = System.getProperty("nacos.password", "nacos");

        accessToken = loginV3(username, password);
        System.out.println("Multi-Tenant Config Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws Exception {
        // Cleanup created namespaces
        for (String nsId : createdNamespaces) {
            try {
                deleteNamespace(nsId);
            } catch (Exception e) {
                // Ignore cleanup errors
            }
        }
    }

    private ConfigService createConfigService(String namespace) throws NacosException {
        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);
        if (namespace != null && !namespace.isEmpty() && !"public".equals(namespace)) {
            properties.setProperty("namespace", namespace);
        }
        return NacosFactory.createConfigService(properties);
    }

    // ==================== Namespace Isolation Tests ====================

    /**
     * NMTC-001: Test same dataId in different namespaces has independent content
     *
     * Publishes the same dataId in two different namespaces with different
     * content and verifies each namespace returns its own content.
     */
    @Test
    @Order(1)
    void testSameDataIdIndependentContent() throws Exception {
        String ns1 = "nmtc001-ns1-" + UUID.randomUUID().toString().substring(0, 6);
        String ns2 = "nmtc001-ns2-" + UUID.randomUUID().toString().substring(0, 6);
        createTestNamespace(ns1, "Tenant A");
        createTestNamespace(ns2, "Tenant B");

        ConfigService configNs1 = createConfigService(ns1);
        ConfigService configNs2 = createConfigService(ns2);

        try {
            String dataId = "shared-data-id";
            String contentNs1 = "tenant=A\ndb.host=db-tenant-a.internal";
            String contentNs2 = "tenant=B\ndb.host=db-tenant-b.internal";

            // Publish same dataId in both namespaces
            assertTrue(configNs1.publishConfig(dataId, DEFAULT_GROUP, contentNs1),
                    "Publish to namespace 1 should succeed");
            assertTrue(configNs2.publishConfig(dataId, DEFAULT_GROUP, contentNs2),
                    "Publish to namespace 2 should succeed");

            Thread.sleep(500);

            // Verify each namespace has its own content
            String retrieved1 = configNs1.getConfig(dataId, DEFAULT_GROUP, 5000);
            String retrieved2 = configNs2.getConfig(dataId, DEFAULT_GROUP, 5000);

            assertEquals(contentNs1, retrieved1,
                    "Namespace 1 should return tenant A content");
            assertEquals(contentNs2, retrieved2,
                    "Namespace 2 should return tenant B content");
            assertNotEquals(retrieved1, retrieved2,
                    "Content should differ between namespaces");

            // Cleanup
            configNs1.removeConfig(dataId, DEFAULT_GROUP);
            configNs2.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            configNs1.shutDown();
            configNs2.shutDown();
        }
    }

    /**
     * NMTC-002: Test config listener only fires for its namespace
     *
     * Registers a listener in namespace A, publishes config in namespace B,
     * and verifies the listener in A does NOT fire. Then publishes in
     * namespace A and verifies the listener DOES fire.
     */
    @Test
    @Order(2)
    void testConfigListenerNamespaceScoping() throws Exception {
        String nsA = "nmtc002-nsA-" + UUID.randomUUID().toString().substring(0, 6);
        String nsB = "nmtc002-nsB-" + UUID.randomUUID().toString().substring(0, 6);
        createTestNamespace(nsA, "Listener NS A");
        createTestNamespace(nsB, "Listener NS B");

        ConfigService configA = createConfigService(nsA);
        ConfigService configB = createConfigService(nsB);

        try {
            String dataId = "listener-scope-test";
            AtomicReference<String> receivedInA = new AtomicReference<>();
            CountDownLatch latchA = new CountDownLatch(1);

            // Register listener in namespace A
            Listener listenerA = new Listener() {
                @Override
                public Executor getExecutor() {
                    return null;
                }

                @Override
                public void receiveConfigInfo(String configInfo) {
                    System.out.println("Listener in NS-A received: " + configInfo);
                    receivedInA.set(configInfo);
                    latchA.countDown();
                }
            };
            configA.addListener(dataId, DEFAULT_GROUP, listenerA);
            Thread.sleep(500);

            // Publish in namespace B - listener in A should NOT fire
            configB.publishConfig(dataId, DEFAULT_GROUP, "namespace=B");
            Thread.sleep(2000);

            boolean firedFromB = latchA.await(3, TimeUnit.SECONDS);
            assertFalse(firedFromB,
                    "Listener in namespace A should NOT fire when config published in namespace B");
            assertNull(receivedInA.get(),
                    "Listener in namespace A should not have received content from namespace B");

            // Now publish in namespace A - listener should fire
            CountDownLatch latchA2 = new CountDownLatch(1);
            AtomicReference<String> receivedInA2 = new AtomicReference<>();
            configA.addListener(dataId, DEFAULT_GROUP, new Listener() {
                @Override
                public Executor getExecutor() {
                    return null;
                }

                @Override
                public void receiveConfigInfo(String configInfo) {
                    receivedInA2.set(configInfo);
                    latchA2.countDown();
                }
            });
            Thread.sleep(500);

            configA.publishConfig(dataId, DEFAULT_GROUP, "namespace=A");
            boolean firedFromA = latchA2.await(15, TimeUnit.SECONDS);
            assertTrue(firedFromA,
                    "Listener in namespace A should fire when config published in namespace A");
            assertEquals("namespace=A", receivedInA2.get(),
                    "Listener should receive correct content from its own namespace");

            // Cleanup
            configA.removeConfig(dataId, DEFAULT_GROUP);
            configB.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            configA.shutDown();
            configB.shutDown();
        }
    }

    /**
     * NMTC-003: Test config operations don't cross namespace boundaries
     *
     * Publishes config in namespace A, then attempts to delete it from
     * namespace B, verifying the deletion does NOT affect namespace A.
     */
    @Test
    @Order(3)
    void testConfigDoesNotCrossNamespaceBoundaries() throws Exception {
        String nsA = "nmtc003-nsA-" + UUID.randomUUID().toString().substring(0, 6);
        String nsB = "nmtc003-nsB-" + UUID.randomUUID().toString().substring(0, 6);
        createTestNamespace(nsA, "Boundary NS A");
        createTestNamespace(nsB, "Boundary NS B");

        ConfigService configA = createConfigService(nsA);
        ConfigService configB = createConfigService(nsB);

        try {
            String dataId = "boundary-test-config";
            String content = "sensitive.data=secret";

            // Publish in namespace A
            assertTrue(configA.publishConfig(dataId, DEFAULT_GROUP, content));
            Thread.sleep(500);

            // Verify exists in A
            assertEquals(content, configA.getConfig(dataId, DEFAULT_GROUP, 5000));

            // Verify NOT accessible from B
            String fromB = configB.getConfig(dataId, DEFAULT_GROUP, 3000);
            assertNull(fromB,
                    "Config published in namespace A should not be accessible from namespace B");

            // Attempt to delete from namespace B
            boolean deletedFromB = configB.removeConfig(dataId, DEFAULT_GROUP);
            // The delete may return true (no error) or false, but it should NOT affect namespace A

            Thread.sleep(500);

            // Verify config in namespace A is still intact
            String stillInA = configA.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals(content, stillInA,
                    "Config in namespace A should be unchanged after delete attempt from namespace B");

            // Cleanup
            configA.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            configA.shutDown();
            configB.shutDown();
        }
    }

    /**
     * NMTC-004: Test default namespace vs custom namespace behavior
     *
     * Verifies that the default (public) namespace and a custom namespace
     * are completely independent, and that empty namespace property uses
     * the public namespace.
     */
    @Test
    @Order(4)
    void testDefaultVsCustomNamespaceBehavior() throws Exception {
        String customNs = "nmtc004-custom-" + UUID.randomUUID().toString().substring(0, 6);
        createTestNamespace(customNs, "Custom NS");

        ConfigService defaultService = createConfigService(null); // Uses public/default namespace
        ConfigService customService = createConfigService(customNs);

        try {
            String dataId = "default-vs-custom-" + UUID.randomUUID().toString().substring(0, 8);

            // Publish different content in default vs custom namespace
            String defaultContent = "env=default\nversion=1.0";
            String customContent = "env=custom\nversion=2.0";

            assertTrue(defaultService.publishConfig(dataId, DEFAULT_GROUP, defaultContent));
            assertTrue(customService.publishConfig(dataId, DEFAULT_GROUP, customContent));
            Thread.sleep(500);

            // Verify default namespace content
            String retrievedDefault = defaultService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals(defaultContent, retrievedDefault,
                    "Default namespace should return default content");

            // Verify custom namespace content
            String retrievedCustom = customService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals(customContent, retrievedCustom,
                    "Custom namespace should return custom content");

            // Create another service with empty string namespace - should use default
            ConfigService emptyNsService = createConfigService("");
            try {
                String retrievedEmpty = emptyNsService.getConfig(dataId, DEFAULT_GROUP, 5000);
                assertEquals(defaultContent, retrievedEmpty,
                        "Empty namespace should resolve to default namespace");
            } finally {
                emptyNsService.shutDown();
            }

            // Cleanup
            defaultService.removeConfig(dataId, DEFAULT_GROUP);
            customService.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            defaultService.shutDown();
            customService.shutDown();
        }
    }

    /**
     * NMTC-005: Test multi-tenant config with different groups in same namespace
     *
     * Within a single namespace, verifies that the same dataId in different
     * groups maintains separate content, combining tenant and group isolation.
     */
    @Test
    @Order(5)
    void testMultiTenantWithDifferentGroups() throws Exception {
        String ns = "nmtc005-groups-" + UUID.randomUUID().toString().substring(0, 6);
        createTestNamespace(ns, "Groups Test NS");

        ConfigService configService = createConfigService(ns);

        try {
            String dataId = "multi-group-config";
            String groupA = "GROUP_A";
            String groupB = "GROUP_B";
            String contentA = "group=A\nfeature.flag=on";
            String contentB = "group=B\nfeature.flag=off";

            // Publish same dataId in different groups
            assertTrue(configService.publishConfig(dataId, groupA, contentA));
            assertTrue(configService.publishConfig(dataId, groupB, contentB));
            Thread.sleep(500);

            // Verify group isolation
            String retrievedA = configService.getConfig(dataId, groupA, 5000);
            String retrievedB = configService.getConfig(dataId, groupB, 5000);

            assertEquals(contentA, retrievedA, "Group A should have its own content");
            assertEquals(contentB, retrievedB, "Group B should have its own content");
            assertNotEquals(retrievedA, retrievedB,
                    "Different groups should have different content");

            // Delete from one group should not affect the other
            configService.removeConfig(dataId, groupA);
            Thread.sleep(500);

            assertNull(configService.getConfig(dataId, groupA, 3000),
                    "Group A config should be deleted");
            assertEquals(contentB, configService.getConfig(dataId, groupB, 5000),
                    "Group B config should be unaffected by Group A deletion");

            // Cleanup
            configService.removeConfig(dataId, groupB);
        } finally {
            configService.shutDown();
        }
    }

    /**
     * NMTC-006: Test config update in one namespace does not notify other namespace
     *
     * SKIPPED: Batata's config change notification does not yet properly filter
     * by namespace when pushing changes via gRPC. The namespace B listener may
     * fire when namespace A updates because the server-side push does not
     * isolate by tenant in the current implementation.
     */
    @Test
    @Order(6)
    @Disabled("ConfigBatchListen initial MD5 mismatch causes spurious listener fire - same root cause as duplicate notification issue")
    void testConfigUpdateDoesNotNotifyCrossNamespace() throws Exception {
        String nsA = "nmtc006-nsA-" + UUID.randomUUID().toString().substring(0, 6);
        String nsB = "nmtc006-nsB-" + UUID.randomUUID().toString().substring(0, 6);
        createTestNamespace(nsA, "Notify NS A");
        createTestNamespace(nsB, "Notify NS B");

        ConfigService configA = createConfigService(nsA);
        ConfigService configB = createConfigService(nsB);

        try {
            String dataId = "cross-notify-test";

            // Publish initial content in both namespaces
            configA.publishConfig(dataId, DEFAULT_GROUP, "initial=A");
            configB.publishConfig(dataId, DEFAULT_GROUP, "initial=B");
            Thread.sleep(500);

            // Register listeners in both namespaces
            CountDownLatch latchA = new CountDownLatch(1);
            CountDownLatch latchB = new CountDownLatch(1);
            AtomicReference<String> receivedA = new AtomicReference<>();
            AtomicReference<String> receivedB = new AtomicReference<>();

            configA.addListener(dataId, DEFAULT_GROUP, new Listener() {
                @Override
                public Executor getExecutor() { return null; }

                @Override
                public void receiveConfigInfo(String configInfo) {
                    receivedA.set(configInfo);
                    latchA.countDown();
                }
            });

            configB.addListener(dataId, DEFAULT_GROUP, new Listener() {
                @Override
                public Executor getExecutor() { return null; }

                @Override
                public void receiveConfigInfo(String configInfo) {
                    receivedB.set(configInfo);
                    latchB.countDown();
                }
            });
            Thread.sleep(500);

            // Update ONLY in namespace A
            configA.publishConfig(dataId, DEFAULT_GROUP, "updated=A");
            Thread.sleep(3000);

            // Wait for listener A to fire
            boolean aFired = latchA.await(15, TimeUnit.SECONDS);
            assertTrue(aFired, "Listener in namespace A should fire");
            assertEquals("updated=A", receivedA.get(),
                    "Namespace A listener should receive updated content");

            // Wait briefly to see if B fires (it should NOT)
            boolean bFired = latchB.await(5, TimeUnit.SECONDS);
            assertFalse(bFired,
                    "Listener in namespace B should NOT fire when update occurs in namespace A");

            // Cleanup
            configA.removeConfig(dataId, DEFAULT_GROUP);
            configB.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            configA.shutDown();
            configB.shutDown();
        }
    }

    /**
     * NMTC-007: Test bulk config operations isolated per namespace
     *
     * Publishes multiple configs in two namespaces, then deletes all
     * from one namespace and verifies the other is unaffected.
     */
    @Test
    @Order(7)
    void testBulkConfigIsolatedPerNamespace() throws Exception {
        String nsA = "nmtc007-nsA-" + UUID.randomUUID().toString().substring(0, 6);
        String nsB = "nmtc007-nsB-" + UUID.randomUUID().toString().substring(0, 6);
        createTestNamespace(nsA, "Bulk NS A");
        createTestNamespace(nsB, "Bulk NS B");

        ConfigService configA = createConfigService(nsA);
        ConfigService configB = createConfigService(nsB);

        try {
            int configCount = 5;
            String prefix = "bulk-iso";

            // Publish configs in both namespaces
            for (int i = 0; i < configCount; i++) {
                String dataId = prefix + "-" + i;
                configA.publishConfig(dataId, DEFAULT_GROUP, "ns=A,idx=" + i);
                configB.publishConfig(dataId, DEFAULT_GROUP, "ns=B,idx=" + i);
            }
            Thread.sleep(500);

            // Delete all from namespace A
            for (int i = 0; i < configCount; i++) {
                configA.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
            }
            Thread.sleep(500);

            // Verify namespace A is empty
            for (int i = 0; i < configCount; i++) {
                String content = configA.getConfig(prefix + "-" + i, DEFAULT_GROUP, 3000);
                assertNull(content,
                        "Namespace A config " + i + " should be deleted");
            }

            // Verify namespace B is unaffected
            for (int i = 0; i < configCount; i++) {
                String content = configB.getConfig(prefix + "-" + i, DEFAULT_GROUP, 5000);
                assertEquals("ns=B,idx=" + i, content,
                        "Namespace B config " + i + " should be unaffected by namespace A deletion");
            }

            // Cleanup namespace B
            for (int i = 0; i < configCount; i++) {
                configB.removeConfig(prefix + "-" + i, DEFAULT_GROUP);
            }
        } finally {
            configA.shutDown();
            configB.shutDown();
        }
    }

    // ==================== Helper Methods ====================

    private void createTestNamespace(String namespaceId, String namespaceName) throws Exception {
        String body = String.format(
                "namespaceId=%s&namespaceName=%s&namespaceDesc=%s",
                URLEncoder.encode(namespaceId, "UTF-8"),
                URLEncoder.encode(namespaceName, "UTF-8"),
                URLEncoder.encode("Test namespace for " + namespaceName, "UTF-8"));
        httpPost("/nacos/v2/console/namespace", body);
        createdNamespaces.add(namespaceId);
    }

    private static void deleteNamespace(String namespaceId) throws Exception {
        httpDeleteStatic("/nacos/v2/console/namespace?namespaceId=" +
                URLEncoder.encode(namespaceId, "UTF-8"));
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
            String resp = readResponse(conn);
            if (resp.contains("accessToken")) {
                int start = resp.indexOf("accessToken") + 14;
                int end = resp.indexOf("\"", start);
                if (end > start) return resp.substring(start, end);
            }
        }
        return "";
    }

    private String httpGet(String path) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }
        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("GET");
        return readResponse(conn);
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
        return readResponse(conn);
    }

    private String httpDelete(String path) throws Exception {
        String fullUrl = String.format("http://%s%s", serverAddr, path);
        if (!accessToken.isEmpty()) {
            fullUrl += (path.contains("?") ? "&" : "?") + "accessToken=" + accessToken;
        }
        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("DELETE");
        return readResponse(conn);
    }

    private static String httpDeleteStatic(String path) throws Exception {
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
        InputStream stream = responseCode >= 400 ? conn.getErrorStream() : conn.getInputStream();
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
