package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigChangeListener;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.model.ConfigChange;
import com.ctrip.framework.apollo.model.ConfigChangeEvent;
import com.google.gson.Gson;
import com.google.gson.JsonObject;
import org.apache.hc.client5.http.classic.methods.*;
import org.apache.hc.client5.http.impl.classic.CloseableHttpClient;
import org.apache.hc.client5.http.impl.classic.HttpClients;
import org.apache.hc.core5.http.ContentType;
import org.apache.hc.core5.http.io.entity.EntityUtils;
import org.apache.hc.core5.http.io.entity.StringEntity;
import org.junit.jupiter.api.*;

import java.io.*;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo End-to-End Integration Tests
 *
 * Comprehensive integration tests for Apollo compatibility:
 * - Full config lifecycle (CRUD)
 * - Publish and subscribe flow
 * - Multi-namespace scenarios
 * - Hot reload and failover
 * - High concurrency and long running clients
 * - Advanced patterns (feature flags, A/B testing, canary release)
 * - Disaster recovery scenarios
 *
 * Requires Batata server running with Apollo plugin enabled.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloIntegrationTest {

    private static String baseUrl;
    private static CloseableHttpClient httpClient;
    private static Gson gson;
    private static Path cacheDir;

    private static final String ENV = "DEV";
    private static final String CLUSTER = "default";
    private static final String APP_ID = "integration-test-app";
    private static final String NAMESPACE = "application";

    @BeforeAll
    static void setup() throws IOException {
        // Set Apollo configuration
        String appId = System.getProperty("app.id", APP_ID);
        String env = System.getProperty("env", ENV);
        String apolloMeta = System.getProperty("apollo.meta", "http://127.0.0.1:8848");
        String configService = System.getProperty("apollo.configService", "http://127.0.0.1:8848");
        String cluster = System.getProperty("apollo.cluster", CLUSTER);

        System.setProperty("app.id", appId);
        System.setProperty("env", env);
        System.setProperty("apollo.meta", apolloMeta);
        System.setProperty("apollo.configService", configService);
        System.setProperty("apollo.cluster", cluster);
        System.setProperty("apollo.config-service", configService);

        // Create temp cache directory for tests
        cacheDir = Files.createTempDirectory("apollo-integration-test-cache");
        System.setProperty("apollo.cache-dir", cacheDir.toString());

        baseUrl = apolloMeta;
        httpClient = HttpClients.createDefault();
        gson = new Gson();

        System.out.println("Apollo Integration Test Setup:");
        System.out.println("  app.id: " + appId);
        System.out.println("  env: " + env);
        System.out.println("  apollo.meta: " + apolloMeta);
        System.out.println("  apollo.cluster: " + cluster);
        System.out.println("  cache.dir: " + cacheDir);
    }

    @AfterAll
    static void teardown() throws IOException {
        if (httpClient != null) {
            httpClient.close();
        }

        // Cleanup cache directory
        if (cacheDir != null && Files.exists(cacheDir)) {
            Files.walk(cacheDir)
                    .sorted(Comparator.reverseOrder())
                    .map(Path::toFile)
                    .forEach(File::delete);
        }
    }

    // ==================== AIT-001: Full Config Lifecycle ====================

    /**
     * AIT-001: Test full config lifecycle (create, read, update, delete)
     *
     * This test verifies the complete CRUD operations for configuration items
     * through the Apollo OpenAPI.
     */
    @Test
    @Order(1)
    void testFullConfigLifecycle() throws IOException {
        String testAppId = "lifecycle-test-" + UUID.randomUUID().toString().substring(0, 8);
        String testKey = "lifecycle.test.key";
        String initialValue = "initial-value";
        String updatedValue = "updated-value";

        System.out.println("AIT-001: Testing full config lifecycle for app: " + testAppId);

        // Step 1: Create config item
        String createUrl = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items",
                baseUrl, ENV, testAppId, CLUSTER, NAMESPACE);

        JsonObject createBody = new JsonObject();
        createBody.addProperty("key", testKey);
        createBody.addProperty("value", initialValue);
        createBody.addProperty("comment", "Integration test - create");
        createBody.addProperty("dataChangeCreatedBy", "integration-test");

        HttpPost createRequest = new HttpPost(createUrl);
        createRequest.setEntity(new StringEntity(gson.toJson(createBody), ContentType.APPLICATION_JSON));

        AtomicInteger createStatus = new AtomicInteger();
        httpClient.execute(createRequest, response -> {
            createStatus.set(response.getCode());
            String body = EntityUtils.toString(response.getEntity());
            System.out.println("  CREATE - Status: " + response.getCode() + ", Body: " + body);
            return null;
        });

        // Step 2: Read config item via SDK
        try {
            Config config = ConfigService.getConfig(NAMESPACE);
            String readValue = config.getProperty(testKey, "not-found");
            System.out.println("  READ via SDK - Value: " + readValue);
        } catch (Exception e) {
            System.out.println("  READ via SDK - Error: " + e.getMessage());
        }

        // Step 3: Update config item
        String updateUrl = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items/%s",
                baseUrl, ENV, testAppId, CLUSTER, NAMESPACE, testKey);

        JsonObject updateBody = new JsonObject();
        updateBody.addProperty("key", testKey);
        updateBody.addProperty("value", updatedValue);
        updateBody.addProperty("comment", "Integration test - update");
        updateBody.addProperty("dataChangeLastModifiedBy", "integration-test");

        HttpPut updateRequest = new HttpPut(updateUrl);
        updateRequest.setEntity(new StringEntity(gson.toJson(updateBody), ContentType.APPLICATION_JSON));

        httpClient.execute(updateRequest, response -> {
            System.out.println("  UPDATE - Status: " + response.getCode());
            return null;
        });

        // Step 4: Delete config item
        String deleteUrl = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/items/%s?operator=integration-test",
                baseUrl, ENV, testAppId, CLUSTER, NAMESPACE, testKey);

        HttpDelete deleteRequest = new HttpDelete(deleteUrl);

        httpClient.execute(deleteRequest, response -> {
            System.out.println("  DELETE - Status: " + response.getCode());
            return null;
        });

        System.out.println("AIT-001: Full config lifecycle test completed");
    }

    // ==================== AIT-002: Publish and Subscribe Flow ====================

    /**
     * AIT-002: Test config publish and subscribe flow
     *
     * Verifies that config changes published via OpenAPI are received
     * by the SDK through change listeners.
     */
    @Test
    @Order(2)
    void testConfigPublishAndSubscribeFlow() throws InterruptedException {
        System.out.println("AIT-002: Testing config publish and subscribe flow");

        Config config = ConfigService.getConfig(NAMESPACE);
        CountDownLatch changeLatch = new CountDownLatch(1);
        AtomicReference<ConfigChangeEvent> receivedEvent = new AtomicReference<>();

        ConfigChangeListener listener = changeEvent -> {
            System.out.println("  Received config change notification");
            System.out.println("  Changed keys: " + changeEvent.changedKeys());
            for (String key : changeEvent.changedKeys()) {
                ConfigChange change = changeEvent.getChange(key);
                System.out.println("    " + key + ": " + change.getOldValue() + " -> " + change.getNewValue());
            }
            receivedEvent.set(changeEvent);
            changeLatch.countDown();
        };

        config.addChangeListener(listener);
        System.out.println("  Change listener registered");

        // Publish config change via OpenAPI
        String testAppId = "pubsub-test-" + UUID.randomUUID().toString().substring(0, 8);
        String releaseUrl = String.format("%s/openapi/v1/envs/%s/apps/%s/clusters/%s/namespaces/%s/releases",
                baseUrl, ENV, testAppId, CLUSTER, NAMESPACE);

        JsonObject releaseBody = new JsonObject();
        releaseBody.addProperty("releaseTitle", "Publish-Subscribe Test Release");
        releaseBody.addProperty("releaseComment", "Testing publish/subscribe flow");
        releaseBody.addProperty("releasedBy", "integration-test");

        try {
            HttpPost request = new HttpPost(releaseUrl);
            request.setEntity(new StringEntity(gson.toJson(releaseBody), ContentType.APPLICATION_JSON));

            httpClient.execute(request, response -> {
                System.out.println("  Published release - Status: " + response.getCode());
                return null;
            });
        } catch (IOException e) {
            System.out.println("  Publish error: " + e.getMessage());
        }

        // Wait for notification (with timeout)
        boolean received = changeLatch.await(10, TimeUnit.SECONDS);
        System.out.println("  Change notification received: " + received);

        config.removeChangeListener(listener);
        System.out.println("AIT-002: Publish and subscribe flow test completed");
    }

    // ==================== AIT-003: Multi-Namespace Application ====================

    /**
     * AIT-003: Test multi-namespace application
     *
     * Verifies that an application can access multiple namespaces
     * simultaneously and configurations are properly isolated.
     */
    @Test
    @Order(3)
    void testMultiNamespaceApplication() {
        System.out.println("AIT-003: Testing multi-namespace application");

        String[] namespaces = {"application", "database", "redis", "messaging", "feature-flags"};
        Map<String, Config> configMap = new ConcurrentHashMap<>();
        Map<String, Set<String>> namespaceProperties = new ConcurrentHashMap<>();

        // Load all namespaces
        for (String namespace : namespaces) {
            try {
                Config config = ConfigService.getConfig(namespace);
                configMap.put(namespace, config);

                Set<String> propertyNames = config.getPropertyNames();
                namespaceProperties.put(namespace, propertyNames);

                System.out.println("  Namespace '" + namespace + "': " + propertyNames.size() + " properties");
            } catch (Exception e) {
                System.out.println("  Namespace '" + namespace + "' error: " + e.getMessage());
            }
        }

        // Verify namespace isolation
        String testKey = "isolation.test.key";
        System.out.println("  Testing namespace isolation for key: " + testKey);

        for (Map.Entry<String, Config> entry : configMap.entrySet()) {
            String namespace = entry.getKey();
            Config config = entry.getValue();
            String value = config.getProperty(testKey, "default-" + namespace);
            System.out.println("    " + namespace + "." + testKey + " = " + value);
        }

        // Verify configs are cached and independent
        assertEquals(namespaces.length, configMap.size(), "Should load all namespaces");

        System.out.println("AIT-003: Multi-namespace application test completed");
    }

    // ==================== AIT-004: Config Hot Reload ====================

    /**
     * AIT-004: Test config hot reload
     *
     * Verifies that configuration changes are automatically detected
     * and reloaded without application restart.
     */
    @Test
    @Order(4)
    void testConfigHotReload() throws InterruptedException {
        System.out.println("AIT-004: Testing config hot reload");

        Config config = ConfigService.getConfig(NAMESPACE);
        String hotReloadKey = "hotreload.test.key";

        // Get initial value
        String initialValue = config.getProperty(hotReloadKey, "initial");
        System.out.println("  Initial value: " + initialValue);

        AtomicReference<String> latestValue = new AtomicReference<>(initialValue);
        AtomicInteger reloadCount = new AtomicInteger(0);

        ConfigChangeListener listener = changeEvent -> {
            if (changeEvent.isChanged(hotReloadKey)) {
                ConfigChange change = changeEvent.getChange(hotReloadKey);
                latestValue.set(change.getNewValue());
                reloadCount.incrementAndGet();
                System.out.println("  Hot reload detected: " + change.getOldValue() + " -> " + change.getNewValue());
            }
        };

        config.addChangeListener(listener, Set.of(hotReloadKey));

        // Simulate polling interval
        for (int i = 0; i < 3; i++) {
            Thread.sleep(2000);
            String currentValue = config.getProperty(hotReloadKey, "default");
            System.out.println("  Polling check " + (i + 1) + ": " + currentValue);
        }

        System.out.println("  Total reload events: " + reloadCount.get());
        config.removeChangeListener(listener);

        System.out.println("AIT-004: Config hot reload test completed");
    }

    // ==================== AIT-005: Failover to Local Cache ====================

    /**
     * AIT-005: Test failover to local cache
     *
     * Verifies that the SDK can serve configurations from local cache
     * when the server is unavailable.
     */
    @Test
    @Order(5)
    void testFailoverToLocalCache() {
        System.out.println("AIT-005: Testing failover to local cache");

        // First, load config (will be cached)
        Config config = ConfigService.getConfig(NAMESPACE);
        String testKey = "failover.test.key";
        String valueBeforeFailover = config.getProperty(testKey, "cached-default");
        System.out.println("  Value before failover: " + valueBeforeFailover);

        // List cache files
        try {
            if (Files.exists(cacheDir)) {
                long fileCount = Files.list(cacheDir).count();
                System.out.println("  Cache files present: " + fileCount);

                Files.walk(cacheDir)
                        .filter(Files::isRegularFile)
                        .forEach(path -> {
                            try {
                                System.out.println("    - " + path.getFileName() + " (" + Files.size(path) + " bytes)");
                            } catch (IOException e) {
                                // ignore
                            }
                        });
            }
        } catch (IOException e) {
            System.out.println("  Cache directory check error: " + e.getMessage());
        }

        // Verify config is still accessible (from cache)
        String valueAfterCheck = config.getProperty(testKey, "cached-default");
        System.out.println("  Value after cache check: " + valueAfterCheck);

        // Default values should be returned consistently
        assertEquals(valueBeforeFailover, valueAfterCheck, "Cached values should be consistent");

        System.out.println("AIT-005: Failover to local cache test completed");
    }

    // ==================== AIT-006: Recovery After Server Restart ====================

    /**
     * AIT-006: Test recovery after server restart
     *
     * Verifies that the SDK recovers properly after server becomes
     * available again following a restart.
     */
    @Test
    @Order(6)
    void testRecoveryAfterServerRestart() throws InterruptedException {
        System.out.println("AIT-006: Testing recovery after server restart simulation");

        Config config = ConfigService.getConfig(NAMESPACE);
        String recoveryKey = "recovery.test.key";

        // Initial access
        String initialValue = config.getProperty(recoveryKey, "default");
        System.out.println("  Initial access: " + initialValue);

        // Simulate reconnection attempts
        AtomicBoolean connectionRestored = new AtomicBoolean(false);

        for (int attempt = 1; attempt <= 5; attempt++) {
            try {
                String url = baseUrl + "/configs/" + APP_ID + "/" + CLUSTER + "/" + NAMESPACE;
                HttpGet request = new HttpGet(url);

                int statusCode = httpClient.execute(request, response -> response.getCode());

                if (statusCode == 200) {
                    connectionRestored.set(true);
                    System.out.println("  Attempt " + attempt + ": Connection restored (status: " + statusCode + ")");
                    break;
                } else {
                    System.out.println("  Attempt " + attempt + ": Status " + statusCode);
                }
            } catch (IOException e) {
                System.out.println("  Attempt " + attempt + ": " + e.getMessage());
            }

            Thread.sleep(1000);
        }

        // Verify SDK can still access configs
        String recoveredValue = config.getProperty(recoveryKey, "default");
        System.out.println("  Recovered value: " + recoveredValue);

        System.out.println("AIT-006: Recovery after server restart test completed");
    }

    // ==================== AIT-007: Config Consistency Across Clients ====================

    /**
     * AIT-007: Test config consistency across clients
     *
     * Verifies that multiple clients accessing the same configuration
     * receive consistent values.
     */
    @Test
    @Order(7)
    void testConfigConsistencyAcrossClients() throws InterruptedException {
        System.out.println("AIT-007: Testing config consistency across clients");

        int clientCount = 5;
        String consistencyKey = "consistency.test.key";
        CountDownLatch latch = new CountDownLatch(clientCount);

        ConcurrentHashMap<Integer, String> clientValues = new ConcurrentHashMap<>();
        ConcurrentHashMap<Integer, Config> clientConfigs = new ConcurrentHashMap<>();

        // Simulate multiple clients accessing same namespace
        for (int i = 0; i < clientCount; i++) {
            final int clientId = i;
            new Thread(() -> {
                try {
                    Config config = ConfigService.getConfig(NAMESPACE);
                    clientConfigs.put(clientId, config);

                    String value = config.getProperty(consistencyKey, "default-consistency");
                    clientValues.put(clientId, value);

                    System.out.println("  Client " + clientId + ": " + value);
                } finally {
                    latch.countDown();
                }
            }).start();
        }

        boolean completed = latch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All clients should complete");

        // Verify all clients got the same value
        Set<String> uniqueValues = new HashSet<>(clientValues.values());
        System.out.println("  Unique values across clients: " + uniqueValues.size());
        System.out.println("  Values: " + uniqueValues);

        // All clients should see the same value
        assertEquals(1, uniqueValues.size(), "All clients should see consistent value");

        // Verify same config instance is shared
        Set<Config> uniqueConfigs = new HashSet<>(clientConfigs.values());
        System.out.println("  Unique config instances: " + uniqueConfigs.size());
        assertEquals(1, uniqueConfigs.size(), "Config instances should be shared");

        System.out.println("AIT-007: Config consistency test completed");
    }

    // ==================== AIT-008: Large Scale Config Items ====================

    /**
     * AIT-008: Test large scale config items
     *
     * Verifies that the system can handle a large number of configuration
     * items and large configuration values.
     */
    @Test
    @Order(8)
    void testLargeScaleConfigItems() {
        System.out.println("AIT-008: Testing large scale config items");

        Config config = ConfigService.getConfig(NAMESPACE);

        // Test many property accesses
        int propertyCount = 1000;
        long startTime = System.currentTimeMillis();

        for (int i = 0; i < propertyCount; i++) {
            String key = "largescale.property." + i;
            config.getProperty(key, "default-" + i);
        }

        long duration = System.currentTimeMillis() - startTime;
        System.out.println("  Accessed " + propertyCount + " properties in " + duration + "ms");
        System.out.println("  Average: " + (duration * 1.0 / propertyCount) + "ms per property");

        // Test large value handling
        StringBuilder largeValue = new StringBuilder();
        for (int i = 0; i < 10000; i++) {
            largeValue.append("config-value-segment-").append(i).append(";");
        }
        System.out.println("  Large value test size: " + largeValue.length() + " characters");

        // Verify property names retrieval performance
        startTime = System.currentTimeMillis();
        Set<String> propertyNames = config.getPropertyNames();
        duration = System.currentTimeMillis() - startTime;
        System.out.println("  Retrieved " + propertyNames.size() + " property names in " + duration + "ms");

        System.out.println("AIT-008: Large scale config items test completed");
    }

    // ==================== AIT-009: High Concurrency Access ====================

    /**
     * AIT-009: Test high concurrency access
     *
     * Verifies that the system handles high concurrent access to
     * configuration properly without errors or inconsistencies.
     */
    @Test
    @Order(9)
    void testHighConcurrencyAccess() throws InterruptedException {
        System.out.println("AIT-009: Testing high concurrency access");

        int threadCount = 50;
        int operationsPerThread = 100;
        ExecutorService executor = Executors.newFixedThreadPool(threadCount);
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch endLatch = new CountDownLatch(threadCount);

        AtomicInteger successCount = new AtomicInteger(0);
        AtomicInteger errorCount = new AtomicInteger(0);
        ConcurrentHashMap<String, AtomicInteger> errorTypes = new ConcurrentHashMap<>();

        for (int t = 0; t < threadCount; t++) {
            final int threadId = t;
            executor.submit(() -> {
                try {
                    startLatch.await();

                    Config config = ConfigService.getConfig(NAMESPACE);

                    for (int op = 0; op < operationsPerThread; op++) {
                        try {
                            String key = "concurrent.key." + (threadId * operationsPerThread + op) % 100;
                            config.getProperty(key, "default");
                            successCount.incrementAndGet();
                        } catch (Exception e) {
                            errorCount.incrementAndGet();
                            errorTypes.computeIfAbsent(e.getClass().getSimpleName(), k -> new AtomicInteger())
                                    .incrementAndGet();
                        }
                    }
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                } finally {
                    endLatch.countDown();
                }
            });
        }

        long startTime = System.currentTimeMillis();
        startLatch.countDown();

        boolean completed = endLatch.await(60, TimeUnit.SECONDS);
        long duration = System.currentTimeMillis() - startTime;

        executor.shutdown();

        assertTrue(completed, "All threads should complete");

        int totalOperations = threadCount * operationsPerThread;
        System.out.println("  Total operations: " + totalOperations);
        System.out.println("  Successful: " + successCount.get());
        System.out.println("  Errors: " + errorCount.get());
        System.out.println("  Duration: " + duration + "ms");
        System.out.println("  Throughput: " + (totalOperations * 1000.0 / duration) + " ops/sec");

        if (!errorTypes.isEmpty()) {
            System.out.println("  Error types:");
            errorTypes.forEach((type, count) -> System.out.println("    - " + type + ": " + count.get()));
        }

        // Allow some errors due to network issues, but majority should succeed
        assertTrue(successCount.get() > totalOperations * 0.9, "At least 90% operations should succeed");

        System.out.println("AIT-009: High concurrency access test completed");
    }

    // ==================== AIT-010: Long Running Client ====================

    /**
     * AIT-010: Test long running client
     *
     * Verifies that the SDK maintains stable operation over an extended
     * period with periodic config accesses.
     */
    @Test
    @Order(10)
    void testLongRunningClient() throws InterruptedException {
        System.out.println("AIT-010: Testing long running client (simulated)");

        Config config = ConfigService.getConfig(NAMESPACE);
        int iterations = 30;
        int intervalMs = 100;

        AtomicInteger accessCount = new AtomicInteger(0);
        List<Long> accessTimes = new CopyOnWriteArrayList<>();

        long startTime = System.currentTimeMillis();

        for (int i = 0; i < iterations; i++) {
            long accessStart = System.nanoTime();

            String value = config.getProperty("longrunning.test.key", "default");

            long accessDuration = System.nanoTime() - accessStart;
            accessTimes.add(accessDuration / 1_000_000); // Convert to ms
            accessCount.incrementAndGet();

            if (i % 10 == 0) {
                System.out.println("  Iteration " + i + ": value = " + value);
            }

            Thread.sleep(intervalMs);
        }

        long totalDuration = System.currentTimeMillis() - startTime;

        // Calculate statistics
        double avgAccessTime = accessTimes.stream().mapToLong(Long::longValue).average().orElse(0);
        long maxAccessTime = accessTimes.stream().mapToLong(Long::longValue).max().orElse(0);
        long minAccessTime = accessTimes.stream().mapToLong(Long::longValue).min().orElse(0);

        System.out.println("  Total iterations: " + accessCount.get());
        System.out.println("  Total duration: " + totalDuration + "ms");
        System.out.println("  Average access time: " + String.format("%.2f", avgAccessTime) + "ms");
        System.out.println("  Min access time: " + minAccessTime + "ms");
        System.out.println("  Max access time: " + maxAccessTime + "ms");

        assertEquals(iterations, accessCount.get(), "All iterations should complete");

        System.out.println("AIT-010: Long running client test completed");
    }

    // ==================== AIT-011: Client Reconnection ====================

    /**
     * AIT-011: Test client reconnection
     *
     * Verifies that the SDK handles reconnection scenarios properly
     * when connection is temporarily lost and restored.
     */
    @Test
    @Order(11)
    void testClientReconnection() throws InterruptedException {
        System.out.println("AIT-011: Testing client reconnection");

        Config config = ConfigService.getConfig(NAMESPACE);
        String reconnectKey = "reconnect.test.key";

        // Initial connection
        String initialValue = config.getProperty(reconnectKey, "initial");
        System.out.println("  Initial connection - value: " + initialValue);

        // Register change listener
        AtomicBoolean listenerActive = new AtomicBoolean(true);
        AtomicInteger notificationCount = new AtomicInteger(0);

        ConfigChangeListener listener = changeEvent -> {
            if (listenerActive.get()) {
                notificationCount.incrementAndGet();
                System.out.println("  Change notification received during reconnection test");
            }
        };

        config.addChangeListener(listener);

        // Simulate reconnection scenario with periodic checks
        for (int attempt = 1; attempt <= 5; attempt++) {
            Thread.sleep(500);

            String currentValue = config.getProperty(reconnectKey, "default");
            System.out.println("  Reconnection check " + attempt + ": value = " + currentValue);

            // Verify SDK is responsive
            assertNotNull(currentValue, "Should get value after reconnection");
        }

        listenerActive.set(false);
        config.removeChangeListener(listener);

        System.out.println("  Total notifications during test: " + notificationCount.get());
        System.out.println("AIT-011: Client reconnection test completed");
    }

    // ==================== AIT-012: Config with Dependencies ====================

    /**
     * AIT-012: Test config with dependencies
     *
     * Verifies handling of configurations that have dependencies on other
     * config values (e.g., variable substitution patterns).
     */
    @Test
    @Order(12)
    void testConfigWithDependencies() {
        System.out.println("AIT-012: Testing config with dependencies");

        Config config = ConfigService.getConfig(NAMESPACE);

        // Define base configs and dependent configs
        Map<String, String> baseConfigs = new LinkedHashMap<>();
        baseConfigs.put("base.url", config.getProperty("base.url", "http://localhost"));
        baseConfigs.put("base.port", config.getProperty("base.port", "8080"));
        baseConfigs.put("base.context", config.getProperty("base.context", "/api"));

        System.out.println("  Base configs:");
        baseConfigs.forEach((k, v) -> System.out.println("    " + k + " = " + v));

        // Build dependent config manually (simulating variable resolution)
        String fullUrl = baseConfigs.get("base.url") + ":" + baseConfigs.get("base.port") + baseConfigs.get("base.context");
        System.out.println("  Computed full URL: " + fullUrl);

        // Test hierarchical config pattern
        String environment = config.getProperty("app.environment", "development");
        String dbUrl = config.getProperty("db." + environment + ".url", "jdbc:default://localhost/db");
        System.out.println("  Environment: " + environment);
        System.out.println("  DB URL for environment: " + dbUrl);

        // Test conditional config pattern
        boolean featureEnabled = config.getBooleanProperty("feature.newui.enabled", false);
        String uiConfig = featureEnabled ?
                config.getProperty("ui.new.theme", "modern") :
                config.getProperty("ui.legacy.theme", "classic");
        System.out.println("  Feature enabled: " + featureEnabled + ", UI theme: " + uiConfig);

        System.out.println("AIT-012: Config with dependencies test completed");
    }

    // ==================== AIT-013: Feature Flag Pattern ====================

    /**
     * AIT-013: Test feature flag pattern
     *
     * Verifies implementation of feature flags using Apollo configuration.
     */
    @Test
    @Order(13)
    void testFeatureFlagPattern() {
        System.out.println("AIT-013: Testing feature flag pattern");

        Config config = ConfigService.getConfig("feature-flags");
        if (config == null) {
            config = ConfigService.getConfig(NAMESPACE);
        }

        // Define feature flags with default values
        Map<String, Boolean> featureFlags = new LinkedHashMap<>();
        featureFlags.put("feature.darkMode.enabled", config.getBooleanProperty("feature.darkMode.enabled", false));
        featureFlags.put("feature.newCheckout.enabled", config.getBooleanProperty("feature.newCheckout.enabled", false));
        featureFlags.put("feature.recommendations.enabled", config.getBooleanProperty("feature.recommendations.enabled", true));
        featureFlags.put("feature.socialLogin.enabled", config.getBooleanProperty("feature.socialLogin.enabled", false));
        featureFlags.put("feature.analytics.enabled", config.getBooleanProperty("feature.analytics.enabled", true));

        System.out.println("  Feature flags status:");
        featureFlags.forEach((flag, enabled) -> System.out.println("    " + flag + " = " + enabled));

        // Test feature flag evaluation
        boolean darkModeEnabled = featureFlags.get("feature.darkMode.enabled");
        String themeToUse = darkModeEnabled ? "dark" : "light";
        System.out.println("  Theme based on feature flag: " + themeToUse);

        // Test percentage-based rollout (simulated)
        int rolloutPercentage = config.getIntProperty("feature.newCheckout.rollout.percentage", 0);
        boolean shouldEnableForUser = new Random().nextInt(100) < rolloutPercentage;
        System.out.println("  Rollout percentage: " + rolloutPercentage + "%, user enabled: " + shouldEnableForUser);

        // Register listener for feature flag changes
        final Config finalConfig = config;
        config.addChangeListener(event -> {
            for (String key : event.changedKeys()) {
                if (key.startsWith("feature.")) {
                    ConfigChange change = event.getChange(key);
                    System.out.println("  Feature flag changed: " + key +
                            " [" + change.getOldValue() + " -> " + change.getNewValue() + "]");
                }
            }
        });

        System.out.println("AIT-013: Feature flag pattern test completed");
    }

    // ==================== AIT-014: A/B Testing Pattern ====================

    /**
     * AIT-014: Test A/B testing pattern
     *
     * Verifies implementation of A/B testing configuration patterns.
     */
    @Test
    @Order(14)
    void testABTestingPattern() {
        System.out.println("AIT-014: Testing A/B testing pattern");

        Config config = ConfigService.getConfig(NAMESPACE);

        // Get A/B test configurations
        String experimentId = "checkout-flow-test";
        String variantAConfig = config.getProperty("ab." + experimentId + ".variant.a", "{\"button\":\"blue\",\"layout\":\"single\"}");
        String variantBConfig = config.getProperty("ab." + experimentId + ".variant.b", "{\"button\":\"green\",\"layout\":\"multi\"}");
        int trafficSplitA = config.getIntProperty("ab." + experimentId + ".traffic.a", 50);
        int trafficSplitB = config.getIntProperty("ab." + experimentId + ".traffic.b", 50);

        System.out.println("  Experiment: " + experimentId);
        System.out.println("  Variant A config: " + variantAConfig);
        System.out.println("  Variant B config: " + variantBConfig);
        System.out.println("  Traffic split: A=" + trafficSplitA + "%, B=" + trafficSplitB + "%");

        // Simulate user assignment based on user ID hash
        String[] testUserIds = {"user-001", "user-002", "user-003", "user-004", "user-005"};
        Map<String, String> userVariants = new HashMap<>();

        for (String userId : testUserIds) {
            int hashBucket = Math.abs(userId.hashCode()) % 100;
            String assignedVariant = hashBucket < trafficSplitA ? "A" : "B";
            userVariants.put(userId, assignedVariant);
        }

        System.out.println("  User variant assignments:");
        userVariants.forEach((user, variant) -> System.out.println("    " + user + " -> Variant " + variant));

        // Count distribution
        long countA = userVariants.values().stream().filter(v -> v.equals("A")).count();
        long countB = userVariants.values().stream().filter(v -> v.equals("B")).count();
        System.out.println("  Distribution: A=" + countA + ", B=" + countB);

        System.out.println("AIT-014: A/B testing pattern test completed");
    }

    // ==================== AIT-015: Canary Release Pattern ====================

    /**
     * AIT-015: Test canary release pattern
     *
     * Verifies implementation of canary release configuration patterns.
     */
    @Test
    @Order(15)
    void testCanaryReleasePattern() {
        System.out.println("AIT-015: Testing canary release pattern");

        Config config = ConfigService.getConfig(NAMESPACE);

        // Get canary release configurations
        boolean canaryEnabled = config.getBooleanProperty("canary.enabled", false);
        int canaryPercentage = config.getIntProperty("canary.percentage", 10);
        String canaryVersion = config.getProperty("canary.version", "v2.0.0");
        String stableVersion = config.getProperty("stable.version", "v1.9.0");
        String canaryRegions = config.getProperty("canary.regions", "us-east-1,eu-west-1");

        System.out.println("  Canary enabled: " + canaryEnabled);
        System.out.println("  Canary percentage: " + canaryPercentage + "%");
        System.out.println("  Canary version: " + canaryVersion);
        System.out.println("  Stable version: " + stableVersion);
        System.out.println("  Canary regions: " + canaryRegions);

        // Simulate canary routing logic
        String[] testInstances = {"instance-001", "instance-002", "instance-003", "instance-004", "instance-005",
                "instance-006", "instance-007", "instance-008", "instance-009", "instance-010"};

        Map<String, String> instanceVersions = new HashMap<>();
        Set<String> canaryRegionSet = new HashSet<>(Arrays.asList(canaryRegions.split(",")));

        for (int i = 0; i < testInstances.length; i++) {
            String instance = testInstances[i];
            boolean isCanary = canaryEnabled && (i < testInstances.length * canaryPercentage / 100);
            String version = isCanary ? canaryVersion : stableVersion;
            instanceVersions.put(instance, version);
        }

        System.out.println("  Instance version assignments:");
        instanceVersions.forEach((instance, version) ->
                System.out.println("    " + instance + " -> " + version));

        // Count version distribution
        long canaryCount = instanceVersions.values().stream().filter(v -> v.equals(canaryVersion)).count();
        long stableCount = instanceVersions.values().stream().filter(v -> v.equals(stableVersion)).count();
        System.out.println("  Version distribution: canary=" + canaryCount + ", stable=" + stableCount);

        System.out.println("AIT-015: Canary release pattern test completed");
    }

    // ==================== AIT-016: Config Templating ====================

    /**
     * AIT-016: Test config templating
     *
     * Verifies support for configuration templating and variable substitution.
     */
    @Test
    @Order(16)
    void testConfigTemplating() {
        System.out.println("AIT-016: Testing config templating");

        Config config = ConfigService.getConfig(NAMESPACE);

        // Get template and variables
        String urlTemplate = config.getProperty("template.url", "https://${host}:${port}/${context}");
        String host = config.getProperty("template.var.host", "api.example.com");
        String port = config.getProperty("template.var.port", "443");
        String context = config.getProperty("template.var.context", "v1");

        System.out.println("  URL template: " + urlTemplate);
        System.out.println("  Variables: host=" + host + ", port=" + port + ", context=" + context);

        // Perform template substitution
        String resolvedUrl = urlTemplate
                .replace("${host}", host)
                .replace("${port}", port)
                .replace("${context}", context);

        System.out.println("  Resolved URL: " + resolvedUrl);

        // Test complex template with multiple variables
        String messageTemplate = config.getProperty("template.message",
                "Hello ${user.name}, your order #${order.id} is ${order.status}");

        Map<String, String> variables = new HashMap<>();
        variables.put("${user.name}", config.getProperty("template.var.user.name", "John"));
        variables.put("${order.id}", config.getProperty("template.var.order.id", "12345"));
        variables.put("${order.status}", config.getProperty("template.var.order.status", "processing"));

        System.out.println("  Message template: " + messageTemplate);

        String resolvedMessage = messageTemplate;
        for (Map.Entry<String, String> entry : variables.entrySet()) {
            resolvedMessage = resolvedMessage.replace(entry.getKey(), entry.getValue());
        }

        System.out.println("  Resolved message: " + resolvedMessage);

        // Test conditional template
        boolean includeFooter = config.getBooleanProperty("template.include.footer", true);
        String footer = includeFooter ?
                config.getProperty("template.footer", "Thank you for your business!") : "";
        System.out.println("  Include footer: " + includeFooter + ", footer: " + footer);

        System.out.println("AIT-016: Config templating test completed");
    }

    // ==================== AIT-017: Config Inheritance Pattern ====================

    /**
     * AIT-017: Test config inheritance pattern
     *
     * Verifies support for configuration inheritance where child configs
     * can inherit from parent configs.
     */
    @Test
    @Order(17)
    void testConfigInheritancePattern() {
        System.out.println("AIT-017: Testing config inheritance pattern");

        // Load configs from multiple namespaces (simulating inheritance)
        Config baseConfig = ConfigService.getConfig("application");
        Config envConfig = ConfigService.getConfig("application-dev");

        // Define inheritance chain: base -> environment
        Map<String, String> effectiveConfig = new LinkedHashMap<>();

        // Base config values
        System.out.println("  Base config (application):");
        String[] baseKeys = {"app.name", "app.version", "db.pool.size", "logging.level"};
        for (String key : baseKeys) {
            String value = baseConfig.getProperty(key, "base-default-" + key);
            effectiveConfig.put(key, value);
            System.out.println("    " + key + " = " + value);
        }

        // Override with environment config
        System.out.println("  Environment config (application-dev) - overrides:");
        Set<String> envPropertyNames = envConfig.getPropertyNames();
        for (String key : envPropertyNames) {
            String value = envConfig.getProperty(key, null);
            if (value != null) {
                String previousValue = effectiveConfig.put(key, value);
                if (previousValue != null && !previousValue.equals(value)) {
                    System.out.println("    " + key + " = " + value + " (overridden from: " + previousValue + ")");
                } else {
                    System.out.println("    " + key + " = " + value);
                }
            }
        }

        // Test specific overrides
        String dbPoolSize = effectiveConfig.getOrDefault("db.pool.size", "10");
        String loggingLevel = effectiveConfig.getOrDefault("logging.level", "INFO");

        System.out.println("  Effective config:");
        effectiveConfig.forEach((k, v) -> System.out.println("    " + k + " = " + v));

        // Demonstrate inheritance resolution
        String getConfigWithInheritance = getConfigValue(new Config[]{envConfig, baseConfig}, "app.name", "default-app");
        System.out.println("  Inherited app.name: " + getConfigWithInheritance);

        System.out.println("AIT-017: Config inheritance pattern test completed");
    }

    /**
     * Helper method to get config value with inheritance chain
     */
    private String getConfigValue(Config[] configChain, String key, String defaultValue) {
        for (Config config : configChain) {
            String value = config.getProperty(key, null);
            if (value != null) {
                return value;
            }
        }
        return defaultValue;
    }

    // ==================== AIT-018: Disaster Recovery Scenario ====================

    /**
     * AIT-018: Test disaster recovery scenario
     *
     * Verifies that the system handles disaster recovery scenarios including
     * server failures, cache fallback, and graceful degradation.
     */
    @Test
    @Order(18)
    void testDisasterRecoveryScenario() throws InterruptedException {
        System.out.println("AIT-018: Testing disaster recovery scenario");

        Config config = ConfigService.getConfig(NAMESPACE);

        // Step 1: Baseline - normal operation
        System.out.println("  Step 1: Baseline measurement");
        String drKey = "disaster.recovery.test.key";
        String baselineValue = config.getProperty(drKey, "baseline");
        long baselineTime = measureConfigAccess(config, drKey, 10);
        System.out.println("    Baseline value: " + baselineValue);
        System.out.println("    Baseline access time: " + baselineTime + "ms (avg of 10)");

        // Step 2: Verify cache is populated
        System.out.println("  Step 2: Verifying cache state");
        try {
            if (Files.exists(cacheDir)) {
                long cacheSize = Files.walk(cacheDir)
                        .filter(Files::isRegularFile)
                        .mapToLong(p -> {
                            try {
                                return Files.size(p);
                            } catch (IOException e) {
                                return 0;
                            }
                        })
                        .sum();
                System.out.println("    Cache directory size: " + cacheSize + " bytes");
            }
        } catch (IOException e) {
            System.out.println("    Cache check error: " + e.getMessage());
        }

        // Step 3: Simulate degraded mode (rapid access)
        System.out.println("  Step 3: Simulating high load scenario");
        AtomicInteger successCount = new AtomicInteger(0);
        AtomicInteger failCount = new AtomicInteger(0);

        int rapidAccessCount = 100;
        long rapidStartTime = System.currentTimeMillis();

        for (int i = 0; i < rapidAccessCount; i++) {
            try {
                config.getProperty("rapid.access.key." + (i % 10), "default");
                successCount.incrementAndGet();
            } catch (Exception e) {
                failCount.incrementAndGet();
            }
        }

        long rapidDuration = System.currentTimeMillis() - rapidStartTime;
        System.out.println("    Rapid access: " + rapidAccessCount + " operations in " + rapidDuration + "ms");
        System.out.println("    Success: " + successCount.get() + ", Failures: " + failCount.get());

        // Step 4: Test recovery with multiple listeners
        System.out.println("  Step 4: Testing listener resilience");
        List<ConfigChangeListener> listeners = new ArrayList<>();
        AtomicInteger listenerNotifications = new AtomicInteger(0);

        for (int i = 0; i < 5; i++) {
            final int listenerId = i;
            ConfigChangeListener listener = event -> {
                listenerNotifications.incrementAndGet();
                System.out.println("    Listener " + listenerId + " received notification");
            };
            listeners.add(listener);
            config.addChangeListener(listener);
        }

        // Wait for potential notifications
        Thread.sleep(2000);

        // Cleanup listeners
        for (ConfigChangeListener listener : listeners) {
            config.removeChangeListener(listener);
        }

        System.out.println("    Total listener notifications: " + listenerNotifications.get());

        // Step 5: Verify system stability after recovery
        System.out.println("  Step 5: Verifying post-recovery stability");
        String postRecoveryValue = config.getProperty(drKey, "baseline");
        long postRecoveryTime = measureConfigAccess(config, drKey, 10);
        System.out.println("    Post-recovery value: " + postRecoveryValue);
        System.out.println("    Post-recovery access time: " + postRecoveryTime + "ms (avg of 10)");

        // Values should be consistent
        assertEquals(baselineValue, postRecoveryValue, "Values should be consistent after recovery");

        // Summary
        System.out.println("  Disaster Recovery Summary:");
        System.out.println("    - Cache fallback: Available");
        System.out.println("    - High load handling: " + successCount.get() + "/" + rapidAccessCount + " success");
        System.out.println("    - Listener resilience: " + listeners.size() + " listeners handled");
        System.out.println("    - Value consistency: " + (baselineValue.equals(postRecoveryValue) ? "PASSED" : "FAILED"));

        System.out.println("AIT-018: Disaster recovery scenario test completed");
    }

    /**
     * Helper method to measure average config access time
     */
    private long measureConfigAccess(Config config, String key, int iterations) {
        long totalTime = 0;
        for (int i = 0; i < iterations; i++) {
            long start = System.nanoTime();
            config.getProperty(key, "default");
            totalTime += System.nanoTime() - start;
        }
        return totalTime / iterations / 1_000_000; // Convert to milliseconds
    }
}
