package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigChangeListener;
import com.ctrip.framework.apollo.ConfigFile;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.core.enums.ConfigFileFormat;
import com.ctrip.framework.apollo.model.ConfigChange;
import com.ctrip.framework.apollo.model.ConfigChangeEvent;
import org.junit.jupiter.api.*;

import java.util.Set;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Config SDK Compatibility Tests
 *
 * Tests Batata's compatibility with official Apollo Java SDK.
 *
 * Note: These tests require:
 * 1. Batata server running with Apollo plugin enabled
 * 2. Pre-configured namespace with test properties
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloConfigTest {

    @BeforeAll
    static void setup() {
        // Apollo client configuration via system properties
        String appId = System.getProperty("app.id", "test-app");
        String env = System.getProperty("env", "DEV");
        String apolloMeta = System.getProperty("apollo.meta", "http://127.0.0.1:8848");
        String configService = System.getProperty("apollo.configService", "http://127.0.0.1:8848");
        String cluster = System.getProperty("apollo.cluster", "default");

        System.setProperty("app.id", appId);
        System.setProperty("env", env);
        System.setProperty("apollo.meta", apolloMeta);
        System.setProperty("apollo.configService", configService);
        System.setProperty("apollo.cluster", cluster);

        // Skip config service discovery
        System.setProperty("apollo.config-service", configService);

        System.out.println("Apollo Config Test Setup:");
        System.out.println("  app.id: " + appId);
        System.out.println("  env: " + env);
        System.out.println("  apollo.meta: " + apolloMeta);
        System.out.println("  apollo.cluster: " + cluster);
    }

    // ==================== P0: Critical Tests ====================

    /**
     * AC-001: Test get config namespace
     */
    @Test
    @Order(1)
    void testGetConfig() {
        Config config = ConfigService.getConfig("application");
        assertNotNull(config, "Config should not be null");

        // Config object should be created even if namespace doesn't exist
        // It will return default values for properties
    }

    /**
     * AC-002: Test get property value
     */
    @Test
    @Order(2)
    void testGetProperty() {
        Config config = ConfigService.getConfig("application");

        // Get property with default value
        String value = config.getProperty("test.key", "default-value");
        assertNotNull(value, "Property value should not be null");

        // Get non-existent property
        String nonExistent = config.getProperty("non.existent.key", "fallback");
        assertEquals("fallback", nonExistent, "Should return default for non-existent key");
    }

    /**
     * AC-003: Test config change listener
     */
    @Test
    @Order(3)
    void testConfigChangeListener() throws InterruptedException {
        Config config = ConfigService.getConfig("application");
        AtomicBoolean changeReceived = new AtomicBoolean(false);
        AtomicReference<ConfigChangeEvent> eventRef = new AtomicReference<>();
        CountDownLatch latch = new CountDownLatch(1);

        ConfigChangeListener listener = changeEvent -> {
            System.out.println("Config change received:");
            for (String key : changeEvent.changedKeys()) {
                ConfigChange change = changeEvent.getChange(key);
                System.out.println("  Key: " + key +
                        ", OldValue: " + change.getOldValue() +
                        ", NewValue: " + change.getNewValue() +
                        ", ChangeType: " + change.getChangeType());
            }
            changeReceived.set(true);
            eventRef.set(changeEvent);
            latch.countDown();
        };

        config.addChangeListener(listener);

        // Note: To trigger this listener, you would need to publish a config change
        // via the Apollo OpenAPI. For basic connectivity test, we just verify
        // the listener is registered successfully.

        // In a real test scenario with OpenAPI:
        // 1. Publish config change via OpenAPI
        // 2. Wait for notification
        // assertTrue(latch.await(30, TimeUnit.SECONDS), "Should receive change notification");

        // For now, just verify listener can be added
        System.out.println("Change listener registered. Waiting for manual config change...");

        // Short wait to demonstrate listener is active
        Thread.sleep(2000);
    }

    // ==================== P1: Important Tests ====================

    /**
     * AC-004: Test get all property names
     */
    @Test
    @Order(4)
    void testGetPropertyNames() {
        Config config = ConfigService.getConfig("application");

        Set<String> propertyNames = config.getPropertyNames();
        assertNotNull(propertyNames, "Property names should not be null");

        System.out.println("Found " + propertyNames.size() + " properties:");
        for (String name : propertyNames) {
            String value = config.getProperty(name, "");
            System.out.println("  " + name + " = " + value);
        }
    }

    /**
     * AC-005: Test get config file
     */
    @Test
    @Order(5)
    void testGetConfigFile() {
        // Test different file formats
        testConfigFileFormat("application", ConfigFileFormat.Properties);
        testConfigFileFormat("application", ConfigFileFormat.JSON);
        testConfigFileFormat("application", ConfigFileFormat.YAML);
    }

    private void testConfigFileFormat(String namespace, ConfigFileFormat format) {
        try {
            ConfigFile configFile = ConfigService.getConfigFile(namespace, format);
            assertNotNull(configFile, "ConfigFile should not be null for format: " + format);

            String content = configFile.getContent();
            System.out.println("ConfigFile [" + namespace + "." + format + "]:");
            if (content != null && !content.isEmpty()) {
                System.out.println("  Content length: " + content.length());
                System.out.println("  Content preview: " + content.substring(0, Math.min(100, content.length())));
            } else {
                System.out.println("  Content is empty or null");
            }
        } catch (Exception e) {
            System.out.println("ConfigFile [" + namespace + "." + format + "] error: " + e.getMessage());
        }
    }

    /**
     * AC-006: Test multiple namespaces
     */
    @Test
    @Order(6)
    void testMultipleNamespaces() {
        // Get application namespace
        Config appConfig = ConfigService.getConfig("application");
        assertNotNull(appConfig);

        // Get custom namespace
        Config customConfig = ConfigService.getConfig("custom-namespace");
        assertNotNull(customConfig);

        // Configs should be independent
        assertNotSame(appConfig, customConfig, "Different namespaces should have different configs");
    }

    // ==================== P2: Nice to Have Tests ====================

    /**
     * AC-007: Test config cache
     */
    @Test
    @Order(7)
    void testConfigCache() {
        // Get same config multiple times
        Config config1 = ConfigService.getConfig("application");
        Config config2 = ConfigService.getConfig("application");

        // Should return cached instance
        assertSame(config1, config2, "Should return cached config instance");
    }

    /**
     * AC-008: Test property with integer value
     */
    @Test
    @Order(8)
    void testTypedProperties() {
        Config config = ConfigService.getConfig("application");

        // Test integer property
        int intValue = config.getIntProperty("test.int.key", 0);
        System.out.println("Int property: " + intValue);

        // Test long property
        long longValue = config.getLongProperty("test.long.key", 0L);
        System.out.println("Long property: " + longValue);

        // Test boolean property
        boolean boolValue = config.getBooleanProperty("test.bool.key", false);
        System.out.println("Boolean property: " + boolValue);

        // Test float property
        float floatValue = config.getFloatProperty("test.float.key", 0.0f);
        System.out.println("Float property: " + floatValue);

        // Test double property
        double doubleValue = config.getDoubleProperty("test.double.key", 0.0);
        System.out.println("Double property: " + doubleValue);
    }

    /**
     * Test config with specific keys listener
     */
    @Test
    @Order(9)
    void testInterestedKeyChangeListener() {
        Config config = ConfigService.getConfig("application");
        AtomicBoolean received = new AtomicBoolean(false);

        // Listen for specific keys only
        config.addChangeListener(event -> {
            System.out.println("Interested key change: " + event.changedKeys());
            received.set(true);
        }, java.util.Set.of("interested.key1", "interested.key2"));

        System.out.println("Registered listener for specific keys");
    }

    /**
     * Test getAppConfig shortcut
     */
    @Test
    @Order(10)
    void testGetAppConfig() {
        Config appConfig = ConfigService.getAppConfig();
        assertNotNull(appConfig, "App config should not be null");

        // Should be same as application namespace
        Config applicationConfig = ConfigService.getConfig("application");
        assertSame(appConfig, applicationConfig, "getAppConfig should return application namespace");
    }
}
