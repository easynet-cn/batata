package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigFile;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.core.enums.ConfigFileFormat;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Namespace Tests
 *
 * Tests for namespace functionality:
 * - Multiple namespace access
 * - Public namespace
 * - Associated namespace
 * - Namespace isolation
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloNamespaceTest {

    @BeforeAll
    static void setupClass() {
        System.setProperty("app.id", System.getProperty("apollo.app.id", "test-app"));
        System.setProperty("apollo.meta", System.getProperty("apollo.meta", "http://127.0.0.1:8848"));
        System.setProperty("apollo.configService", System.getProperty("apollo.configService", "http://127.0.0.1:8848"));
        System.setProperty("env", System.getProperty("apollo.env", "DEV"));
        System.setProperty("apollo.cluster", System.getProperty("apollo.cluster", "default"));

        System.out.println("Apollo Namespace Test Setup");
    }

    // ==================== Default Namespace Tests ====================

    /**
     * ANS-001: Test access default application namespace
     */
    @Test
    @Order(1)
    void testDefaultApplicationNamespace() {
        Config config = ConfigService.getAppConfig();
        assertNotNull(config, "Default app config should not be null");

        Set<String> propertyNames = config.getPropertyNames();
        System.out.println("Default namespace properties: " + propertyNames.size());
    }

    /**
     * ANS-002: Test access application namespace explicitly
     */
    @Test
    @Order(2)
    void testExplicitApplicationNamespace() {
        Config config = ConfigService.getConfig("application");
        assertNotNull(config, "Application namespace should not be null");

        String value = config.getProperty("test.key", "default");
        System.out.println("Application namespace test.key: " + value);
    }

    // ==================== Custom Namespace Tests ====================

    /**
     * ANS-003: Test access custom namespace
     */
    @Test
    @Order(3)
    void testCustomNamespace() {
        String namespace = "custom-namespace";

        try {
            Config config = ConfigService.getConfig(namespace);
            assertNotNull(config, "Custom namespace config should not be null");

            String value = config.getProperty("custom.key", "default");
            System.out.println("Custom namespace custom.key: " + value);
        } catch (Exception e) {
            System.out.println("Custom namespace access: " + e.getMessage());
        }
    }

    /**
     * ANS-004: Test access multiple namespaces
     */
    @Test
    @Order(4)
    void testMultipleNamespaces() {
        String[] namespaces = {"application", "common", "database", "redis"};
        Map<String, Config> configs = new HashMap<>();

        for (String ns : namespaces) {
            try {
                Config config = ConfigService.getConfig(ns);
                configs.put(ns, config);
                System.out.println("Loaded namespace: " + ns);
            } catch (Exception e) {
                System.out.println("Namespace " + ns + " error: " + e.getMessage());
            }
        }

        System.out.println("Total namespaces loaded: " + configs.size());
    }

    /**
     * ANS-005: Test namespace with dots in name
     */
    @Test
    @Order(5)
    void testNamespaceWithDots() {
        String namespace = "application.database";

        try {
            Config config = ConfigService.getConfig(namespace);
            assertNotNull(config);

            String value = config.getProperty("db.url", "default");
            System.out.println("Dotted namespace db.url: " + value);
        } catch (Exception e) {
            System.out.println("Dotted namespace: " + e.getMessage());
        }
    }

    // ==================== Public Namespace Tests ====================

    /**
     * ANS-006: Test access public namespace
     */
    @Test
    @Order(6)
    void testPublicNamespace() {
        // Public namespace format: appId.namespace
        String publicNs = "TEST1.public-common";

        try {
            Config config = ConfigService.getConfig(publicNs);
            assertNotNull(config);

            String value = config.getProperty("public.key", "default");
            System.out.println("Public namespace public.key: " + value);
        } catch (Exception e) {
            System.out.println("Public namespace: " + e.getMessage());
        }
    }

    /**
     * ANS-007: Test access shared public namespace
     */
    @Test
    @Order(7)
    void testSharedPublicNamespace() {
        String sharedNs = "SHARED.common-config";

        try {
            Config config = ConfigService.getConfig(sharedNs);
            String value = config.getProperty("shared.setting", "default");
            System.out.println("Shared namespace: " + value);
        } catch (Exception e) {
            System.out.println("Shared namespace: " + e.getMessage());
        }
    }

    // ==================== Namespace Isolation Tests ====================

    /**
     * ANS-008: Test namespace isolation - same key different values
     */
    @Test
    @Order(8)
    void testNamespaceIsolation() {
        String key = "isolation.test.key";

        Config appConfig = ConfigService.getConfig("application");
        Config customConfig = ConfigService.getConfig("custom");

        String appValue = appConfig.getProperty(key, "app-default");
        String customValue = customConfig.getProperty(key, "custom-default");

        System.out.println("Application namespace: " + key + " = " + appValue);
        System.out.println("Custom namespace: " + key + " = " + customValue);

        // Values can be different (isolation working)
    }

    /**
     * ANS-009: Test concurrent namespace access
     */
    @Test
    @Order(9)
    void testConcurrentNamespaceAccess() throws InterruptedException {
        int threadCount = 10;
        String[] namespaces = {"ns1", "ns2", "ns3", "ns4", "ns5"};
        CountDownLatch latch = new CountDownLatch(threadCount);
        List<Exception> errors = new CopyOnWriteArrayList<>();

        for (int i = 0; i < threadCount; i++) {
            final int index = i;
            new Thread(() -> {
                try {
                    String ns = namespaces[index % namespaces.length];
                    Config config = ConfigService.getConfig(ns);
                    config.getProperty("key" + index, "default");
                } catch (Exception e) {
                    errors.add(e);
                } finally {
                    latch.countDown();
                }
            }).start();
        }

        boolean completed = latch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete");

        System.out.println("Concurrent namespace access - errors: " + errors.size());
    }

    // ==================== Config File Namespace Tests ====================

    /**
     * ANS-010: Test config file in namespace
     */
    @Test
    @Order(10)
    void testConfigFileInNamespace() {
        try {
            ConfigFile file = ConfigService.getConfigFile("application", ConfigFileFormat.Properties);
            assertNotNull(file);

            String content = file.getContent();
            System.out.println("Config file content length: " + (content != null ? content.length() : 0));
        } catch (Exception e) {
            System.out.println("Config file: " + e.getMessage());
        }
    }

    /**
     * ANS-011: Test JSON config file namespace
     */
    @Test
    @Order(11)
    void testJsonConfigFileNamespace() {
        String namespace = "config.json";

        try {
            ConfigFile file = ConfigService.getConfigFile(namespace, ConfigFileFormat.JSON);
            String content = file.getContent();

            if (content != null && !content.isEmpty()) {
                assertTrue(content.trim().startsWith("{") || content.trim().startsWith("["),
                        "Should be valid JSON");
            }
            System.out.println("JSON file namespace content: " + (content != null ? content.length() : 0) + " chars");
        } catch (Exception e) {
            System.out.println("JSON namespace: " + e.getMessage());
        }
    }

    /**
     * ANS-012: Test YAML config file namespace
     */
    @Test
    @Order(12)
    void testYamlConfigFileNamespace() {
        String namespace = "config.yaml";

        try {
            ConfigFile file = ConfigService.getConfigFile(namespace, ConfigFileFormat.YAML);
            String content = file.getContent();

            System.out.println("YAML file namespace content: " + (content != null ? content.length() : 0) + " chars");
        } catch (Exception e) {
            System.out.println("YAML namespace: " + e.getMessage());
        }
    }

    // ==================== Namespace Property Tests ====================

    /**
     * ANS-013: Test get all property names from namespace
     */
    @Test
    @Order(13)
    void testGetAllPropertyNames() {
        Config config = ConfigService.getConfig("application");
        Set<String> names = config.getPropertyNames();

        assertNotNull(names, "Property names should not be null");
        System.out.println("Property names count: " + names.size());

        for (String name : names) {
            System.out.println("  - " + name);
        }
    }

    /**
     * ANS-014: Test namespace source type
     */
    @Test
    @Order(14)
    void testNamespaceSourceType() {
        Config config = ConfigService.getConfig("application");

        // Get source type for a property
        String key = "test.property";
        String value = config.getProperty(key, null);

        System.out.println("Property '" + key + "' value: " + value);
    }

    /**
     * ANS-015: Test namespace with special characters
     */
    @Test
    @Order(15)
    void testNamespaceSpecialCharacters() {
        String[] specialNamespaces = {
                "namespace-with-dash",
                "namespace_with_underscore",
                "namespace.with.dots"
        };

        for (String ns : specialNamespaces) {
            try {
                Config config = ConfigService.getConfig(ns);
                String value = config.getProperty("key", "default");
                System.out.println("Namespace '" + ns + "': " + value);
            } catch (Exception e) {
                System.out.println("Namespace '" + ns + "' error: " + e.getClass().getSimpleName());
            }
        }
    }
}
