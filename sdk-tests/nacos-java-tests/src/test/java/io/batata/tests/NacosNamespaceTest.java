package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.model.response.Namespace;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import com.alibaba.nacos.maintainer.client.NacosMaintainerFactory;
import com.alibaba.nacos.maintainer.client.config.ConfigMaintainerService;
import org.junit.jupiter.api.*;

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
 * and cross-namespace operations using Nacos SDK.
 * All operations use SDK clients (ConfigMaintainerService, ConfigService, NamingService).
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosNamespaceTest {

    private static String serverAddr;
    private static String username;
    private static String password;
    private static ConfigMaintainerService maintainerService;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final String PUBLIC_NAMESPACE = "public";

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        username = System.getProperty("nacos.username", "nacos");
        password = System.getProperty("nacos.password", "nacos");

        Properties props = new Properties();
        props.setProperty("serverAddr", serverAddr);
        props.setProperty("username", username);
        props.setProperty("password", password);
        maintainerService = NacosMaintainerFactory.createConfigMaintainerService(props);

        System.out.println("Namespace Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws Exception {
        if (maintainerService != null) {
            maintainerService.shutdown();
        }
    }

    private ConfigService createConfigService(String namespace) throws NacosException {
        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);
        if (namespace != null && !namespace.isEmpty() && !namespace.equals(PUBLIC_NAMESPACE)) {
            properties.setProperty("namespace", namespace);
        }
        return NacosFactory.createConfigService(properties);
    }

    private NamingService createNamingService(String namespace) throws NacosException {
        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);
        if (namespace != null && !namespace.isEmpty() && !namespace.equals(PUBLIC_NAMESPACE)) {
            properties.setProperty("namespace", namespace);
        }
        return NacosFactory.createNamingService(properties);
    }

    // ==================== Namespace Tests ====================

    /**
     * NNS-001: Test default namespace (public)
     */
    @Test
    @Order(1)
    void testDefaultNamespace() throws Exception {
        // The public namespace should be available by default
        List<Namespace> namespaces = maintainerService.getNamespaceList();
        assertNotNull(namespaces, "Namespace list should not be null");
        assertFalse(namespaces.isEmpty(), "Should have at least one namespace (public)");

        // Create a config service for the default namespace
        ConfigService configService = createConfigService(PUBLIC_NAMESPACE);
        try {
            String dataId = "nns001-default-ns-" + UUID.randomUUID();
            String content = "default.namespace.test=true";

            boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, content);
            assertTrue(published, "Should publish config in default namespace");

            String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals(content, retrieved, "Should retrieve config from default namespace");

            configService.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            configService.shutDown();
        }
    }

    /**
     * NNS-002: Test custom namespace creation
     */
    @Test
    @Order(2)
    void testCustomNamespaceCreation() throws Exception {
        String namespaceId = "nns002-" + UUID.randomUUID().toString().substring(0, 8);
        String namespaceName = "Test Custom Namespace";
        String namespaceDesc = "Created by NNS-002 test";

        try {
            Boolean created = maintainerService.createNamespace(namespaceId, namespaceName, namespaceDesc);
            assertTrue(created, "Namespace creation should succeed");

            // Verify namespace exists via SDK
            Namespace ns = maintainerService.getNamespace(namespaceId);
            assertNotNull(ns, "Should retrieve created namespace");

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
            maintainerService.deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-003: Test namespace isolation for config
     */
    @Test
    @Order(3)
    void testNamespaceIsolationForConfig() throws Exception {
        String namespace1 = "nns003-ns1-" + UUID.randomUUID().toString().substring(0, 6);
        String namespace2 = "nns003-ns2-" + UUID.randomUUID().toString().substring(0, 6);

        maintainerService.createNamespace(namespace1, "Namespace 1", "First isolated namespace");
        maintainerService.createNamespace(namespace2, "Namespace 2", "Second isolated namespace");

        ConfigService configService1 = createConfigService(namespace1);
        ConfigService configService2 = createConfigService(namespace2);

        try {
            String dataId = "isolated-config";
            String content1 = "namespace=ns1";
            String content2 = "namespace=ns2";

            assertTrue(configService1.publishConfig(dataId, DEFAULT_GROUP, content1));
            assertTrue(configService2.publishConfig(dataId, DEFAULT_GROUP, content2));
            Thread.sleep(500);

            String retrieved1 = configService1.getConfig(dataId, DEFAULT_GROUP, 5000);
            String retrieved2 = configService2.getConfig(dataId, DEFAULT_GROUP, 5000);

            assertEquals(content1, retrieved1, "Namespace 1 should see its own content");
            assertEquals(content2, retrieved2, "Namespace 2 should see its own content");
            assertNotEquals(retrieved1, retrieved2, "Configs should be different between namespaces");

            configService1.removeConfig(dataId, DEFAULT_GROUP);
            configService2.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            configService1.shutDown();
            configService2.shutDown();
            maintainerService.deleteNamespace(namespace1);
            maintainerService.deleteNamespace(namespace2);
        }
    }

    /**
     * NNS-004: Test namespace isolation for service
     */
    @Test
    @Order(4)
    void testNamespaceIsolationForService() throws Exception {
        String namespace1 = "nns004-ns1-" + UUID.randomUUID().toString().substring(0, 6);
        String namespace2 = "nns004-ns2-" + UUID.randomUUID().toString().substring(0, 6);

        maintainerService.createNamespace(namespace1, "Service NS 1", "First service namespace");
        maintainerService.createNamespace(namespace2, "Service NS 2", "Second service namespace");

        NamingService namingService1 = createNamingService(namespace1);
        NamingService namingService2 = createNamingService(namespace2);

        try {
            String serviceName = "isolated-service";
            String ip1 = "10.0.1.1";
            String ip2 = "10.0.2.1";
            int port = 8080;

            namingService1.registerInstance(serviceName, ip1, port);
            namingService2.registerInstance(serviceName, ip2, port);
            Thread.sleep(1500);

            List<Instance> instances1 = namingService1.getAllInstances(serviceName);
            List<Instance> instances2 = namingService2.getAllInstances(serviceName);

            assertEquals(1, instances1.size(), "Namespace 1 should have 1 instance");
            assertEquals(1, instances2.size(), "Namespace 2 should have 1 instance");
            assertEquals(ip1, instances1.get(0).getIp(), "Namespace 1 should see its own IP");
            assertEquals(ip2, instances2.get(0).getIp(), "Namespace 2 should see its own IP");

            namingService1.deregisterInstance(serviceName, ip1, port);
            namingService2.deregisterInstance(serviceName, ip2, port);
        } finally {
            namingService1.shutDown();
            namingService2.shutDown();
            maintainerService.deleteNamespace(namespace1);
            maintainerService.deleteNamespace(namespace2);
        }
    }

    /**
     * NNS-005: Test cross-namespace access
     */
    @Test
    @Order(5)
    void testCrossNamespaceAccess() throws Exception {
        String namespace1 = "nns005-ns1-" + UUID.randomUUID().toString().substring(0, 6);
        String namespace2 = "nns005-ns2-" + UUID.randomUUID().toString().substring(0, 6);

        maintainerService.createNamespace(namespace1, "Cross Access NS 1", "Cross access test namespace 1");
        maintainerService.createNamespace(namespace2, "Cross Access NS 2", "Cross access test namespace 2");

        ConfigService configService1 = createConfigService(namespace1);
        ConfigService configService2 = createConfigService(namespace2);

        try {
            String dataId = "cross-ns-config-" + UUID.randomUUID();
            String content = "secret.data=confidential";

            assertTrue(configService1.publishConfig(dataId, DEFAULT_GROUP, content));
            Thread.sleep(500);

            String ns1Content = configService1.getConfig(dataId, DEFAULT_GROUP, 5000);
            assertEquals(content, ns1Content, "Config should exist in namespace 1");

            String ns2Content = configService2.getConfig(dataId, DEFAULT_GROUP, 3000);
            assertNull(ns2Content, "Config from namespace 1 should NOT be visible in namespace 2");

            configService1.removeConfig(dataId, DEFAULT_GROUP);
        } finally {
            configService1.shutDown();
            configService2.shutDown();
            maintainerService.deleteNamespace(namespace1);
            maintainerService.deleteNamespace(namespace2);
        }
    }

    /**
     * NNS-006: Test namespace with special characters
     */
    @Test
    @Order(6)
    void testNamespaceWithSpecialCharacters() throws Exception {
        String namespaceId = "nns006_test-ns_" + UUID.randomUUID().toString().substring(0, 6);
        String namespaceName = "Special Chars NS (Test)";
        String namespaceDesc = "Namespace with special chars: _-";

        try {
            Boolean created = maintainerService.createNamespace(namespaceId, namespaceName, namespaceDesc);
            assertTrue(created, "Should create namespace with special characters");

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
            maintainerService.deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-007: Test namespace capacity
     */
    @Test
    @Order(7)
    void testNamespaceCapacity() throws Exception {
        String namespaceId = "nns007-capacity-" + UUID.randomUUID().toString().substring(0, 6);
        maintainerService.createNamespace(namespaceId, "Capacity Test NS", "Testing capacity limits");

        ConfigService configService = createConfigService(namespaceId);

        try {
            int configCount = 10;
            List<String> dataIds = new ArrayList<>();

            for (int i = 0; i < configCount; i++) {
                String dataId = "capacity-config-" + i;
                dataIds.add(dataId);
                boolean published = configService.publishConfig(dataId, DEFAULT_GROUP, "config.index=" + i);
                assertTrue(published, "Should publish config " + i);
            }
            Thread.sleep(500);

            for (int i = 0; i < configCount; i++) {
                String retrieved = configService.getConfig(dataIds.get(i), DEFAULT_GROUP, 5000);
                assertEquals("config.index=" + i, retrieved, "Config " + i + " should exist");
            }

            for (String dataId : dataIds) {
                configService.removeConfig(dataId, DEFAULT_GROUP);
            }
        } finally {
            configService.shutDown();
            maintainerService.deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-008: Test namespace metadata
     */
    @Test
    @Order(8)
    void testNamespaceMetadata() throws Exception {
        String namespaceId = "nns008-meta-" + UUID.randomUUID().toString().substring(0, 6);
        String namespaceName = "Metadata Test Namespace";
        String namespaceDesc = "Testing namespace metadata retrieval";

        try {
            maintainerService.createNamespace(namespaceId, namespaceName, namespaceDesc);

            Namespace ns = maintainerService.getNamespace(namespaceId);
            assertNotNull(ns, "Should retrieve namespace metadata");
            assertEquals(namespaceId, ns.getNamespace(), "Namespace ID should match");
            assertEquals(namespaceName, ns.getNamespaceShowName(), "Namespace name should match");
        } finally {
            maintainerService.deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-009: Test namespace listing
     */
    @Test
    @Order(9)
    void testNamespaceListing() throws Exception {
        String prefix = "nns009-list-" + UUID.randomUUID().toString().substring(0, 4);
        List<String> createdNamespaces = new ArrayList<>();

        try {
            for (int i = 0; i < 3; i++) {
                String namespaceId = prefix + "-" + i;
                createdNamespaces.add(namespaceId);
                maintainerService.createNamespace(namespaceId, "List Test NS " + i, "For listing test");
            }
            Thread.sleep(500);

            List<Namespace> namespaces = maintainerService.getNamespaceList();
            assertNotNull(namespaces, "Namespace list should not be null");

            for (String nsId : createdNamespaces) {
                boolean found = namespaces.stream()
                        .anyMatch(ns -> nsId.equals(ns.getNamespace()));
                assertTrue(found, "Namespace " + nsId + " should appear in the list");
            }
        } finally {
            for (String nsId : createdNamespaces) {
                try { maintainerService.deleteNamespace(nsId); } catch (Exception ignored) {}
            }
        }
    }

    /**
     * NNS-010: Test namespace deletion
     */
    @Test
    @Order(10)
    void testNamespaceDeletion() throws Exception {
        String namespaceId = "nns010-delete-" + UUID.randomUUID().toString().substring(0, 6);
        maintainerService.createNamespace(namespaceId, "To Be Deleted NS", "Will be deleted");

        // Verify namespace exists
        List<Namespace> before = maintainerService.getNamespaceList();
        assertTrue(before.stream().anyMatch(ns -> namespaceId.equals(ns.getNamespace())),
                "Namespace should exist before deletion");

        // Add some data then clean it
        ConfigService configService = createConfigService(namespaceId);
        try {
            configService.publishConfig("pre-delete-config", DEFAULT_GROUP, "will.be.deleted=true");
            Thread.sleep(500);
            configService.removeConfig("pre-delete-config", DEFAULT_GROUP);
        } finally {
            configService.shutDown();
        }

        // Delete namespace
        Boolean deleted = maintainerService.deleteNamespace(namespaceId);
        assertTrue(deleted, "Namespace deletion should succeed");

        Thread.sleep(500);
        List<Namespace> after = maintainerService.getNamespaceList();
        assertFalse(after.stream().anyMatch(ns -> namespaceId.equals(ns.getNamespace())),
                "Namespace should not exist after deletion");
    }

    /**
     * NNS-011: Test namespace update
     */
    @Test
    @Order(11)
    void testNamespaceUpdate() throws Exception {
        String namespaceId = "nns011-update-" + UUID.randomUUID().toString().substring(0, 6);
        String originalName = "Original Name";
        String updatedName = "Updated Name";
        String updatedDesc = "Updated description for namespace";

        try {
            maintainerService.createNamespace(namespaceId, originalName, "Original description");

            Boolean updated = maintainerService.updateNamespace(namespaceId, updatedName, updatedDesc);
            assertTrue(updated, "Namespace update should succeed");

            Namespace ns = maintainerService.getNamespace(namespaceId);
            assertNotNull(ns, "Should retrieve updated namespace");
            assertEquals(updatedName, ns.getNamespaceShowName(), "Namespace name should be updated");
        } finally {
            maintainerService.deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-012: Test concurrent namespace operations
     */
    @Test
    @Order(12)
    void testConcurrentNamespaceOperations() throws Exception {
        String namespaceId = "nns012-concurrent-" + UUID.randomUUID().toString().substring(0, 6);
        maintainerService.createNamespace(namespaceId, "Concurrent Test NS", "For concurrency testing");

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

            startLatch.countDown();
            boolean completed = doneLatch.await(60, TimeUnit.SECONDS);
            assertTrue(completed, "All threads should complete within timeout");
            assertNull(error.get(), "No errors should occur during concurrent operations");

            int expectedSuccess = threadCount * opsPerThread;
            assertEquals(expectedSuccess, successCount.get(),
                    "All concurrent operations should succeed");
        } finally {
            configService.shutDown();
            maintainerService.deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-013: Test namespace with config operations
     */
    @Test
    @Order(13)
    void testNamespaceWithConfigOperations() throws Exception {
        String namespaceId = "nns013-config-ops-" + UUID.randomUUID().toString().substring(0, 6);
        maintainerService.createNamespace(namespaceId, "Config Ops NS", "For config operation tests");

        ConfigService configService = createConfigService(namespaceId);

        try {
            String dataId = "config-ops-test";
            String group = "CONFIG_OPS_GROUP";

            String initialContent = "version=1.0";
            assertTrue(configService.publishConfig(dataId, group, initialContent));
            Thread.sleep(300);

            String retrieved = configService.getConfig(dataId, group, 5000);
            assertEquals(initialContent, retrieved, "Initial content should match");

            String updatedContent = "version=2.0";
            assertTrue(configService.publishConfig(dataId, group, updatedContent));
            Thread.sleep(300);

            String afterUpdate = configService.getConfig(dataId, group, 5000);
            assertEquals(updatedContent, afterUpdate, "Updated content should match");

            assertTrue(configService.removeConfig(dataId, group));
            Thread.sleep(300);

            String afterDelete = configService.getConfig(dataId, group, 3000);
            assertNull(afterDelete, "Config should be deleted");
        } finally {
            configService.shutDown();
            maintainerService.deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-014: Test namespace with naming operations
     */
    @Test
    @Order(14)
    void testNamespaceWithNamingOperations() throws Exception {
        String namespaceId = "nns014-naming-ops-" + UUID.randomUUID().toString().substring(0, 6);
        maintainerService.createNamespace(namespaceId, "Naming Ops NS", "For naming operation tests");

        NamingService namingService = createNamingService(namespaceId);

        try {
            String serviceName = "naming-ops-service";
            String group = "NAMING_OPS_GROUP";

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

            List<Instance> allInstances = namingService.getAllInstances(serviceName, group);
            assertEquals(2, allInstances.size(), "Should have 2 instances");

            List<Instance> healthyInstances = namingService.selectInstances(serviceName, group, true);
            assertEquals(2, healthyInstances.size(), "All instances should be healthy");

            Set<String> zones = new HashSet<>();
            for (Instance inst : allInstances) {
                zones.add(inst.getMetadata().get("zone"));
            }
            assertTrue(zones.contains("zone-a") && zones.contains("zone-b"),
                    "Both zones should be present");

            namingService.deregisterInstance(serviceName, group, instance1);
            Thread.sleep(1000);

            List<Instance> afterDeregister = namingService.getAllInstances(serviceName, group);
            assertEquals(1, afterDeregister.size(), "Should have 1 instance after deregister");
            assertEquals("172.16.0.2", afterDeregister.get(0).getIp());

            namingService.deregisterInstance(serviceName, group, instance2);
        } finally {
            namingService.shutDown();
            maintainerService.deleteNamespace(namespaceId);
        }
    }

    /**
     * NNS-015: Test namespace migration
     */
    @Test
    @Order(15)
    void testNamespaceMigration() throws Exception {
        String sourceNs = "nns015-source-" + UUID.randomUUID().toString().substring(0, 6);
        String targetNs = "nns015-target-" + UUID.randomUUID().toString().substring(0, 6);

        maintainerService.createNamespace(sourceNs, "Source NS", "Migration source namespace");
        maintainerService.createNamespace(targetNs, "Target NS", "Migration target namespace");

        ConfigService sourceConfig = createConfigService(sourceNs);
        ConfigService targetConfig = createConfigService(targetNs);

        try {
            List<String> dataIds = Arrays.asList("migrate-config-1", "migrate-config-2", "migrate-config-3");
            Map<String, String> contents = new HashMap<>();

            for (String dataId : dataIds) {
                String content = "dataId=" + dataId + ",source=" + sourceNs;
                contents.put(dataId, content);
                assertTrue(sourceConfig.publishConfig(dataId, DEFAULT_GROUP, content));
            }
            Thread.sleep(500);

            for (String dataId : dataIds) {
                String retrieved = sourceConfig.getConfig(dataId, DEFAULT_GROUP, 5000);
                assertEquals(contents.get(dataId), retrieved);
            }

            for (String dataId : dataIds) {
                String content = sourceConfig.getConfig(dataId, DEFAULT_GROUP, 5000);
                assertNotNull(content);
                assertTrue(targetConfig.publishConfig(dataId, DEFAULT_GROUP, content));
            }
            Thread.sleep(500);

            for (String dataId : dataIds) {
                String retrieved = targetConfig.getConfig(dataId, DEFAULT_GROUP, 5000);
                assertEquals(contents.get(dataId), retrieved);
            }

            for (String dataId : dataIds) {
                assertTrue(sourceConfig.removeConfig(dataId, DEFAULT_GROUP));
            }
            Thread.sleep(500);

            for (String dataId : dataIds) {
                assertNull(sourceConfig.getConfig(dataId, DEFAULT_GROUP, 3000));
                assertEquals(contents.get(dataId), targetConfig.getConfig(dataId, DEFAULT_GROUP, 5000));
            }

            for (String dataId : dataIds) {
                targetConfig.removeConfig(dataId, DEFAULT_GROUP);
            }
        } finally {
            sourceConfig.shutDown();
            targetConfig.shutDown();
            maintainerService.deleteNamespace(sourceNs);
            maintainerService.deleteNamespace(targetNs);
        }
    }
}
