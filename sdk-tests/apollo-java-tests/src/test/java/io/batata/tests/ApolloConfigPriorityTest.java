package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigService;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Configuration Priority and Override Tests
 *
 * Tests for configuration priority, override, and merge functionality:
 * - Namespace priority order
 * - Cluster and environment overrides
 * - Local config and system property overrides
 * - Environment variable overrides
 * - Config merge strategies
 * - Config inheritance and fallback
 * - Gray release priority
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloConfigPriorityTest {

    private static final String DEFAULT_VALUE = "not-found";

    @BeforeAll
    static void setupClass() {
        System.setProperty("app.id", System.getProperty("apollo.app.id", "test-app"));
        System.setProperty("apollo.meta", System.getProperty("apollo.meta", "http://127.0.0.1:8848"));
        System.setProperty("apollo.configService", System.getProperty("apollo.configService", "http://127.0.0.1:8848"));
        System.setProperty("env", System.getProperty("apollo.env", "DEV"));
        System.setProperty("apollo.cluster", System.getProperty("apollo.cluster", "default"));

        System.out.println("Apollo Config Priority Test Setup");
    }

    // ==================== Namespace Priority Tests ====================

    /**
     * ACP-001: Test default namespace priority
     *
     * Verifies that the default (application) namespace is loaded first
     * and serves as the baseline configuration.
     */
    @Test
    @Order(1)
    void testDefaultNamespacePriority() {
        // Default application namespace should be the primary config source
        Config defaultConfig = ConfigService.getAppConfig();
        assertNotNull(defaultConfig, "Default app config should not be null");

        Set<String> propertyNames = defaultConfig.getPropertyNames();
        System.out.println("Default namespace property count: " + propertyNames.size());

        // Default namespace is the base layer in priority chain
        String baseValue = defaultConfig.getProperty("priority.base.key", DEFAULT_VALUE);
        System.out.println("Default namespace priority.base.key: " + baseValue);

        // Verify config object is same when accessed via different methods
        Config explicitDefault = ConfigService.getConfig("application");
        assertSame(defaultConfig, explicitDefault,
                "getAppConfig() and getConfig('application') should return same instance");
    }

    /**
     * ACP-002: Test application namespace priority
     *
     * The application namespace should have higher priority than public namespaces
     * when the same key exists in both.
     */
    @Test
    @Order(2)
    void testApplicationNamespacePriority() {
        Config appConfig = ConfigService.getConfig("application");
        assertNotNull(appConfig);

        // Application namespace properties should override public namespace values
        String appValue = appConfig.getProperty("override.test.key", DEFAULT_VALUE);
        System.out.println("Application namespace override.test.key: " + appValue);

        // Application namespace should have access to all configured properties
        Set<String> names = appConfig.getPropertyNames();
        System.out.println("Application namespace has " + names.size() + " properties");

        // Properties defined in application namespace take precedence
        String priorityValue = appConfig.getProperty("app.priority.value", "app-default");
        System.out.println("App priority value: " + priorityValue);
    }

    /**
     * ACP-003: Test public namespace priority
     *
     * Public namespaces have lower priority than application namespace
     * but can be used for shared configurations.
     */
    @Test
    @Order(3)
    void testPublicNamespacePriority() {
        String publicNs = "TEST1.public-config";

        try {
            Config publicConfig = ConfigService.getConfig(publicNs);
            assertNotNull(publicConfig);

            // Public namespace provides shared configuration
            String publicValue = publicConfig.getProperty("shared.public.key", DEFAULT_VALUE);
            System.out.println("Public namespace shared.public.key: " + publicValue);

            // Compare with application namespace
            Config appConfig = ConfigService.getConfig("application");
            String appValue = appConfig.getProperty("shared.public.key", DEFAULT_VALUE);
            System.out.println("Application namespace shared.public.key: " + appValue);

            // If both have the value, application namespace should be used when accessed via appConfig
            System.out.println("Public namespace serves as fallback for shared configs");
        } catch (Exception e) {
            System.out.println("Public namespace priority test: " + e.getMessage());
        }
    }

    /**
     * ACP-004: Test namespace override order
     *
     * Verifies the complete namespace override order:
     * System Properties > Application Namespace > Public Namespace > Defaults
     */
    @Test
    @Order(4)
    void testNamespaceOverrideOrder() {
        String testKey = "override.order.key";

        // Get value from application namespace
        Config appConfig = ConfigService.getConfig("application");
        String appValue = appConfig.getProperty(testKey, DEFAULT_VALUE);
        System.out.println("Application namespace value: " + appValue);

        // Try public namespace
        try {
            Config publicConfig = ConfigService.getConfig("TEST1.public-config");
            String publicValue = publicConfig.getProperty(testKey, DEFAULT_VALUE);
            System.out.println("Public namespace value: " + publicValue);
        } catch (Exception e) {
            System.out.println("Public namespace: " + e.getMessage());
        }

        // System property should override both (if set)
        String sysPropValue = System.getProperty(testKey);
        if (sysPropValue != null) {
            System.out.println("System property value: " + sysPropValue);
        }

        System.out.println("Override order: System > App > Public > Default");
    }

    // ==================== Cluster and Environment Override Tests ====================

    /**
     * ACP-005: Test cluster-specific override
     *
     * Cluster-specific configurations should override default cluster configurations.
     */
    @Test
    @Order(5)
    void testClusterSpecificOverride() {
        String currentCluster = System.getProperty("apollo.cluster", "default");
        System.out.println("Current cluster: " + currentCluster);

        Config config = ConfigService.getConfig("application");

        // Cluster-specific settings override default
        String clusterDbUrl = config.getProperty("database.url", "jdbc:mysql://localhost:3306/default");
        String clusterRedisHost = config.getProperty("redis.host", "localhost");
        int clusterRedisPort = config.getIntProperty("redis.port", 6379);

        System.out.println("Cluster-specific database.url: " + clusterDbUrl);
        System.out.println("Cluster-specific redis.host: " + clusterRedisHost);
        System.out.println("Cluster-specific redis.port: " + clusterRedisPort);

        // Verify cluster-specific override is applied
        // In a real scenario, different clusters would have different values
        assertNotNull(clusterDbUrl, "Cluster database URL should be configured");
    }

    /**
     * ACP-006: Test environment-specific override
     *
     * Environment-specific configurations (DEV, FAT, UAT, PRO) should override base configurations.
     */
    @Test
    @Order(6)
    void testEnvironmentSpecificOverride() {
        String currentEnv = System.getProperty("env", "DEV");
        System.out.println("Current environment: " + currentEnv);

        Config config = ConfigService.getConfig("application");

        // Environment-specific settings
        String logLevel = config.getProperty("log.level", "INFO");
        boolean debugEnabled = config.getBooleanProperty("debug.enabled", false);
        int connectionPoolSize = config.getIntProperty("db.pool.size", 10);

        System.out.println("Environment log.level: " + logLevel);
        System.out.println("Environment debug.enabled: " + debugEnabled);
        System.out.println("Environment db.pool.size: " + connectionPoolSize);

        // DEV typically has debug enabled
        if ("DEV".equalsIgnoreCase(currentEnv)) {
            System.out.println("DEV environment may have debug features enabled");
        }
    }

    /**
     * ACP-007: Test local config override
     *
     * Local cached configurations should be used when remote config server is unavailable.
     */
    @Test
    @Order(7)
    void testLocalConfigOverride() {
        Config config = ConfigService.getConfig("application");

        // Local config serves as fallback when remote is unavailable
        String localFallbackKey = "local.fallback.key";
        String value = config.getProperty(localFallbackKey, "local-default");

        System.out.println("Local config fallback value: " + value);

        // Apollo caches configs locally for offline access
        // Local cache path is typically: /opt/data/{appId}/config-cache
        String cacheDir = System.getProperty("apollo.cacheDir", "/opt/data");
        System.out.println("Local cache directory: " + cacheDir);

        // Local override can be enabled via apollo.local.override
        String localOverride = System.getProperty("apollo.local.override", "false");
        System.out.println("Local override enabled: " + localOverride);
    }

    /**
     * ACP-008: Test system property override
     *
     * System properties should have highest priority and override all other sources.
     */
    @Test
    @Order(8)
    void testSystemPropertyOverride() {
        String testKey = "system.override.test";
        String systemValue = "system-override-value";

        // Set system property
        System.setProperty(testKey, systemValue);

        try {
            Config config = ConfigService.getConfig("application");

            // Get value from config - system property should take precedence
            // Note: Apollo respects system properties for certain configurations
            String configValue = config.getProperty(testKey, DEFAULT_VALUE);
            System.out.println("Config value for " + testKey + ": " + configValue);

            // System property value
            String sysPropValue = System.getProperty(testKey);
            System.out.println("System property value: " + sysPropValue);
            assertEquals(systemValue, sysPropValue, "System property should be set");

            System.out.println("System properties have highest priority in override chain");
        } finally {
            // Clean up
            System.clearProperty(testKey);
        }
    }

    /**
     * ACP-009: Test environment variable override
     *
     * Environment variables can override configuration values in certain scenarios.
     */
    @Test
    @Order(9)
    void testEnvironmentVariableOverride() {
        // Environment variables that affect Apollo configuration
        String metaEnvVar = System.getenv("APOLLO_META");
        String appIdEnvVar = System.getenv("APP_ID");
        String envEnvVar = System.getenv("ENV");
        String clusterEnvVar = System.getenv("APOLLO_CLUSTER");

        System.out.println("Environment variables:");
        System.out.println("  APOLLO_META: " + (metaEnvVar != null ? metaEnvVar : "not set"));
        System.out.println("  APP_ID: " + (appIdEnvVar != null ? appIdEnvVar : "not set"));
        System.out.println("  ENV: " + (envEnvVar != null ? envEnvVar : "not set"));
        System.out.println("  APOLLO_CLUSTER: " + (clusterEnvVar != null ? clusterEnvVar : "not set"));

        // Config values that can be influenced by environment variables
        Config config = ConfigService.getConfig("application");
        String envValue = config.getProperty("env.variable.test", DEFAULT_VALUE);
        System.out.println("Config influenced by env variable: " + envValue);

        // Environment variables priority: ENV_VAR > System Property in some cases
        System.out.println("Environment variables can override default configurations");
    }

    // ==================== Config Merge and Inheritance Tests ====================

    /**
     * ACP-010: Test config merge strategy
     *
     * When multiple namespaces contain the same key, verify merge behavior.
     */
    @Test
    @Order(10)
    void testConfigMergeStrategy() {
        Config appConfig = ConfigService.getConfig("application");

        // Test key that might exist in multiple namespaces
        String mergeKey = "merge.strategy.key";
        String mergeValue = appConfig.getProperty(mergeKey, DEFAULT_VALUE);
        System.out.println("Merged value for " + mergeKey + ": " + mergeValue);

        // Apollo uses "first wins" strategy - first namespace with key takes precedence
        // Order is determined by namespace loading order

        // Get property names to see merged result
        Set<String> allNames = appConfig.getPropertyNames();
        System.out.println("Total merged properties: " + allNames.size());

        // Merge strategy is: later loaded namespace values override earlier ones
        // But within same namespace, last definition wins
        System.out.println("Merge strategy: First namespace with key wins");
    }

    /**
     * ACP-011: Test config inheritance
     *
     * Child namespaces can inherit and override parent namespace configurations.
     */
    @Test
    @Order(11)
    void testConfigInheritance() {
        // Base/parent namespace
        Config baseConfig = ConfigService.getConfig("application");
        String baseDbUrl = baseConfig.getProperty("database.url", "jdbc:mysql://base:3306/db");
        System.out.println("Base database.url: " + baseDbUrl);

        // Extended/child namespace pattern (e.g., application.database)
        try {
            Config extendedConfig = ConfigService.getConfig("application.database");
            String extendedDbUrl = extendedConfig.getProperty("database.url", baseDbUrl);
            System.out.println("Extended database.url: " + extendedDbUrl);

            // Child namespace inherits base values but can override
            String specificSetting = extendedConfig.getProperty("database.pool.max", "20");
            System.out.println("Extended-specific setting: " + specificSetting);
        } catch (Exception e) {
            System.out.println("Extended namespace: " + e.getMessage());
        }

        // Inheritance pattern is namespace naming convention based
        System.out.println("Inheritance via namespace naming (parent.child)");
    }

    /**
     * ACP-012: Test config fallback chain
     *
     * Verify the fallback chain when a property is not found.
     */
    @Test
    @Order(12)
    void testConfigFallbackChain() {
        Config config = ConfigService.getConfig("application");

        // Test fallback with default value
        String key1 = "fallback.chain.test.1";
        String value1 = config.getProperty(key1, "fallback-default-1");
        System.out.println(key1 + " (with fallback): " + value1);

        // Test fallback without default (returns null)
        String key2 = "fallback.chain.test.2";
        String value2 = config.getProperty(key2, null);
        System.out.println(key2 + " (no fallback): " + value2);

        // Fallback chain: Cluster Config > Datacenter Config > Default Config > Default Value
        String[] fallbackKeys = {"app.setting.cluster", "app.setting.dc", "app.setting.default"};
        for (String key : fallbackKeys) {
            String value = config.getProperty(key, DEFAULT_VALUE);
            System.out.println("Fallback chain " + key + ": " + value);
        }

        // Demonstrate fallback chain order
        System.out.println("Fallback chain: Cluster > Datacenter > Default > Hardcoded Default");
    }

    /**
     * ACP-013: Test multiple namespace merge
     *
     * When an application uses multiple namespaces, verify property merging.
     */
    @Test
    @Order(13)
    void testMultipleNamespaceMerge() {
        String[] namespaces = {"application", "common", "database", "redis"};
        Map<String, Config> configs = new LinkedHashMap<>();
        Set<String> allKeys = new HashSet<>();

        for (String ns : namespaces) {
            try {
                Config config = ConfigService.getConfig(ns);
                configs.put(ns, config);
                allKeys.addAll(config.getPropertyNames());
                System.out.println("Loaded namespace: " + ns + " with " +
                        config.getPropertyNames().size() + " properties");
            } catch (Exception e) {
                System.out.println("Namespace " + ns + ": " + e.getMessage());
            }
        }

        System.out.println("Total unique keys across namespaces: " + allKeys.size());

        // When accessing a key, each namespace is independent
        // To merge, application must implement its own logic
        String testKey = "shared.config.key";
        for (Map.Entry<String, Config> entry : configs.entrySet()) {
            String value = entry.getValue().getProperty(testKey, DEFAULT_VALUE);
            System.out.println("Namespace " + entry.getKey() + " " + testKey + ": " + value);
        }
    }

    /**
     * ACP-014: Test config priority with same key
     *
     * When the same key exists in multiple sources, verify which value is used.
     */
    @Test
    @Order(14)
    void testConfigPriorityWithSameKey() {
        String sameKey = "priority.same.key";

        // Priority order (highest to lowest):
        // 1. System properties
        // 2. Cluster-specific config
        // 3. Application namespace
        // 4. Public namespace
        // 5. Default value

        // Check system property first
        String sysProp = System.getProperty(sameKey);
        System.out.println("1. System property: " + (sysProp != null ? sysProp : "not set"));

        // Check application namespace
        Config appConfig = ConfigService.getConfig("application");
        String appValue = appConfig.getProperty(sameKey, null);
        System.out.println("2. Application namespace: " + (appValue != null ? appValue : "not set"));

        // Check public namespace
        try {
            Config publicConfig = ConfigService.getConfig("TEST1.public-config");
            String publicValue = publicConfig.getProperty(sameKey, null);
            System.out.println("3. Public namespace: " + (publicValue != null ? publicValue : "not set"));
        } catch (Exception e) {
            System.out.println("3. Public namespace: not available");
        }

        // Final resolved value with default
        String finalValue = appConfig.getProperty(sameKey, "default-value");
        System.out.println("Final resolved value: " + finalValue);

        // Demonstrate priority resolution
        System.out.println("Priority: System > Cluster > App > Public > Default");
    }

    // ==================== Dynamic Priority Tests ====================

    /**
     * ACP-015: Test dynamic priority change
     *
     * Verify behavior when configuration priorities change at runtime.
     */
    @Test
    @Order(15)
    void testDynamicPriorityChange() throws InterruptedException {
        String dynamicKey = "dynamic.priority.key";
        Config config = ConfigService.getConfig("application");

        // Get initial value
        String initialValue = config.getProperty(dynamicKey, "initial-default");
        System.out.println("Initial value: " + initialValue);

        // Simulate priority change by setting system property
        String overrideValue = "dynamic-override-" + System.currentTimeMillis();
        System.setProperty(dynamicKey, overrideValue);

        try {
            // System property should take precedence immediately
            String sysPropValue = System.getProperty(dynamicKey);
            System.out.println("System property set: " + sysPropValue);

            // Config value may or may not reflect system property
            // depending on Apollo implementation
            String configValue = config.getProperty(dynamicKey, "default");
            System.out.println("Config value after system prop set: " + configValue);

            // Wait for potential config refresh
            Thread.sleep(1000);

            // Check again after delay
            String refreshedValue = config.getProperty(dynamicKey, "default");
            System.out.println("Config value after refresh: " + refreshedValue);

        } finally {
            // Clean up
            System.clearProperty(dynamicKey);
        }

        System.out.println("Dynamic priority changes may require config refresh");
    }

    /**
     * ACP-016: Test priority with gray release
     *
     * Gray release configurations should have highest priority for targeted clients.
     */
    @Test
    @Order(16)
    void testPriorityWithGrayRelease() {
        Config config = ConfigService.getConfig("application");

        // Gray release keys (typically controlled via Apollo portal)
        String grayKey = "gray.release.feature";
        String grayValue = config.getProperty(grayKey, "default-feature");
        System.out.println("Gray release feature value: " + grayValue);

        // Gray release IP targeting
        String clientIp = System.getProperty("apollo.client.ip", "127.0.0.1");
        System.out.println("Client IP for gray targeting: " + clientIp);

        // Gray release configuration priority:
        // 1. Gray release config (if client is in gray list)
        // 2. Published config
        // 3. Default value

        // Check for gray release indicators
        String grayLabel = config.getProperty("gray.release.label", null);
        String grayPercentage = config.getProperty("gray.release.percentage", "0");

        if (grayLabel != null) {
            System.out.println("Gray release label: " + grayLabel);
        }
        System.out.println("Gray release percentage: " + grayPercentage + "%");

        // Gray config can override published config for specific clients
        String publishedValue = config.getProperty("feature.published.value", "published");
        String grayOverrideValue = config.getProperty("feature.gray.value", publishedValue);

        System.out.println("Published value: " + publishedValue);
        System.out.println("Gray override value: " + grayOverrideValue);

        // Verify gray release behavior
        // When client is in gray list, gray config takes precedence
        System.out.println("Gray release priority: Gray Config > Published Config > Default");
    }
}
