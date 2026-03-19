package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import com.alibaba.nacos.api.naming.selector.NamingSelector;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Service Selector Tests
 *
 * Tests for service instance selection strategies:
 * - Random selection
 * - Round robin
 * - Weighted selection
 * - Custom selectors
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosServiceSelectorTest {

    private static NamingService namingService;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";

    @BeforeAll
    static void setup() throws NacosException {
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);

        namingService = NacosFactory.createNamingService(properties);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (namingService != null) {
            namingService.shutDown();
        }
    }

    // ==================== Basic Selection Tests ====================

    /**
     * NSS-001: Test select one healthy instance
     */
    @Test
    @Order(1)
    void testSelectOneHealthyInstance() throws NacosException, InterruptedException {
        String serviceName = "selector-basic-" + UUID.randomUUID().toString().substring(0, 8);

        // Register multiple instances
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.30." + i);
            instance.setPort(8080);
            instance.setHealthy(true);
            instance.setWeight(1.0);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        // Select one healthy instance
        Instance selected = namingService.selectOneHealthyInstance(serviceName, DEFAULT_GROUP);
        assertNotNull(selected, "Should select an instance");
        assertTrue(selected.isHealthy(), "Selected instance should be healthy");
        assertTrue(selected.getIp().startsWith("192.168.30."),
                "Selected instance IP should be from registered range");
        assertEquals(8080, selected.getPort(), "Selected instance port should be 8080");

        // Cleanup
        for (int i = 0; i < 3; i++) {
            namingService.deregisterInstance(serviceName, "192.168.30." + i, 8080);
        }
    }

    /**
     * NSS-002: Test select multiple times returns different instances
     */
    @Test
    @Order(2)
    void testSelectMultipleTimes() throws NacosException, InterruptedException {
        String serviceName = "selector-multi-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances
        for (int i = 0; i < 5; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.31." + i);
            instance.setPort(8080);
            instance.setHealthy(true);
            instance.setWeight(1.0);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        // Select multiple times
        Set<String> selectedIps = new HashSet<>();
        for (int i = 0; i < 20; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName, DEFAULT_GROUP);
            assertNotNull(selected, "Selection should not return null");
            assertTrue(selected.isHealthy(), "Each selected instance should be healthy");
            selectedIps.add(selected.getIp());
        }

        assertTrue(selectedIps.size() > 1, "Should select different instances over 20 selections");
        for (String ip : selectedIps) {
            assertTrue(ip.startsWith("192.168.31."),
                    "All selected IPs should be from registered range, got: " + ip);
        }

        // Cleanup
        for (int i = 0; i < 5; i++) {
            namingService.deregisterInstance(serviceName, "192.168.31." + i, 8080);
        }
    }

    // ==================== Weighted Selection Tests ====================

    /**
     * NSS-003: Test weighted selection favors higher weight
     */
    @Test
    @Order(3)
    void testWeightedSelectionFavorsHigherWeight() throws NacosException, InterruptedException {
        String serviceName = "selector-weighted-" + UUID.randomUUID().toString().substring(0, 8);

        // High weight instance
        Instance highWeight = new Instance();
        highWeight.setIp("192.168.32.1");
        highWeight.setPort(8080);
        highWeight.setHealthy(true);
        highWeight.setWeight(100.0);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, highWeight);

        // Low weight instance
        Instance lowWeight = new Instance();
        lowWeight.setIp("192.168.32.2");
        lowWeight.setPort(8080);
        lowWeight.setHealthy(true);
        lowWeight.setWeight(1.0);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, lowWeight);

        Thread.sleep(1000);

        // Count selections
        Map<String, Integer> selectionCount = new HashMap<>();
        for (int i = 0; i < 100; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName, DEFAULT_GROUP);
            assertNotNull(selected, "Selection should not return null");
            selectionCount.merge(selected.getIp(), 1, Integer::sum);
        }

        int highCount = selectionCount.getOrDefault("192.168.32.1", 0);
        int lowCount = selectionCount.getOrDefault("192.168.32.2", 0);

        assertTrue(highCount > lowCount,
                "High weight instance should be selected more often, but got high=" + highCount + " low=" + lowCount);
        assertTrue(highCount > 50,
                "High weight (100x) instance should be selected majority of the time, got " + highCount + "/100");

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, highWeight);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, lowWeight);
    }

    /**
     * NSS-004: Test zero weight instance rarely selected
     */
    @Test
    @Order(4)
    void testZeroWeightRarelySelected() throws NacosException, InterruptedException {
        String serviceName = "selector-zero-" + UUID.randomUUID().toString().substring(0, 8);

        // Zero weight instance
        Instance zeroWeight = new Instance();
        zeroWeight.setIp("192.168.33.1");
        zeroWeight.setPort(8080);
        zeroWeight.setHealthy(true);
        zeroWeight.setWeight(0.0);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, zeroWeight);

        // Normal weight instance
        Instance normalWeight = new Instance();
        normalWeight.setIp("192.168.33.2");
        normalWeight.setPort(8080);
        normalWeight.setHealthy(true);
        normalWeight.setWeight(1.0);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, normalWeight);

        Thread.sleep(1000);

        int zeroSelected = 0;
        int normalSelected = 0;
        for (int i = 0; i < 50; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName, DEFAULT_GROUP);
            assertNotNull(selected, "Selection should not return null");
            if (selected.getIp().equals("192.168.33.1")) {
                zeroSelected++;
            } else {
                normalSelected++;
            }
        }

        assertTrue(zeroSelected < normalSelected,
                "Zero weight instance should be selected less than normal weight, got zero=" + zeroSelected + " normal=" + normalSelected);

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, zeroWeight);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, normalWeight);
    }

    /**
     * NSS-005: Test equal weights distribution
     */
    @Test
    @Order(5)
    void testEqualWeightsDistribution() throws NacosException, InterruptedException {
        String serviceName = "selector-equal-" + UUID.randomUUID().toString().substring(0, 8);
        int instanceCount = 4;

        // Register instances with equal weights
        for (int i = 0; i < instanceCount; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.34." + i);
            instance.setPort(8080);
            instance.setHealthy(true);
            instance.setWeight(1.0);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        Map<String, Integer> distribution = new HashMap<>();
        int totalSelections = 200;

        for (int i = 0; i < totalSelections; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName, DEFAULT_GROUP);
            assertNotNull(selected, "Selection should not return null");
            distribution.merge(selected.getIp(), 1, Integer::sum);
        }

        // With equal weights, each instance should get at least some selections
        for (int i = 0; i < instanceCount; i++) {
            String ip = "192.168.34." + i;
            int count = distribution.getOrDefault(ip, 0);
            assertTrue(count > 0,
                    "Instance " + ip + " should be selected at least once with equal weights, but got 0 out of " + totalSelections);
        }

        // Verify all selected IPs belong to registered range
        for (String ip : distribution.keySet()) {
            assertTrue(ip.startsWith("192.168.34."),
                    "Selected IP should be from registered range, got: " + ip);
        }

        // Cleanup
        for (int i = 0; i < instanceCount; i++) {
            namingService.deregisterInstance(serviceName, "192.168.34." + i, 8080);
        }
    }

    // ==================== Cluster Selection Tests ====================

    /**
     * NSS-006: Test select from specific cluster
     */
    @Test
    @Order(6)
    void testSelectFromSpecificCluster() throws NacosException, InterruptedException {
        String serviceName = "selector-cluster-" + UUID.randomUUID().toString().substring(0, 8);

        // Cluster A instances
        for (int i = 0; i < 2; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.35." + i);
            instance.setPort(8080);
            instance.setClusterName("cluster-a");
            instance.setHealthy(true);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        // Cluster B instances
        for (int i = 0; i < 2; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.36." + i);
            instance.setPort(8080);
            instance.setClusterName("cluster-b");
            instance.setHealthy(true);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        // Select from cluster-a only
        List<Instance> clusterAInstances = namingService.selectInstances(
                serviceName, DEFAULT_GROUP, Arrays.asList("cluster-a"), true);

        assertFalse(clusterAInstances.isEmpty(), "Should find instances in cluster-a");
        assertEquals(2, clusterAInstances.size(), "Should find exactly 2 instances in cluster-a");

        for (Instance inst : clusterAInstances) {
            assertEquals("cluster-a", inst.getClusterName(),
                    "All instances should belong to cluster-a");
            assertTrue(inst.getIp().startsWith("192.168.35."),
                    "Cluster-a instance IP should start with 192.168.35., got: " + inst.getIp());
            assertTrue(inst.isHealthy(), "All selected instances should be healthy");
        }

        // Cleanup
        for (int i = 0; i < 2; i++) {
            namingService.deregisterInstance(serviceName, "192.168.35." + i, 8080, "cluster-a");
            namingService.deregisterInstance(serviceName, "192.168.36." + i, 8080, "cluster-b");
        }
    }

    /**
     * NSS-007: Test select one from specific cluster
     */
    @Test
    @Order(7)
    void testSelectOneFromCluster() throws NacosException, InterruptedException {
        String serviceName = "selector-one-cluster-" + UUID.randomUUID().toString().substring(0, 8);
        String targetCluster = "target-cluster";

        // Target cluster instances
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.37." + i);
            instance.setPort(8080);
            instance.setClusterName(targetCluster);
            instance.setHealthy(true);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        // Select one from target cluster
        Instance selected = namingService.selectOneHealthyInstance(
                serviceName, DEFAULT_GROUP, Arrays.asList(targetCluster), true);

        assertNotNull(selected, "Should select an instance from the target cluster");
        assertEquals(targetCluster, selected.getClusterName(),
                "Selected instance should belong to target cluster");
        assertTrue(selected.getIp().startsWith("192.168.37."),
                "Selected IP should be from registered range, got: " + selected.getIp());
        assertTrue(selected.isHealthy(), "Selected instance should be healthy");

        // Cleanup
        for (int i = 0; i < 3; i++) {
            namingService.deregisterInstance(serviceName, "192.168.37." + i, 8080, targetCluster);
        }
    }

    // ==================== Health Filter Selection Tests ====================

    /**
     * NSS-008: Test select only healthy instances
     */
    @Test
    @Order(8)
    void testSelectOnlyHealthy() throws NacosException, InterruptedException {
        String serviceName = "selector-healthy-" + UUID.randomUUID().toString().substring(0, 8);

        // Healthy instances
        Instance healthy1 = new Instance();
        healthy1.setIp("192.168.38.1");
        healthy1.setPort(8080);
        healthy1.setHealthy(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, healthy1);

        Instance healthy2 = new Instance();
        healthy2.setIp("192.168.38.2");
        healthy2.setPort(8080);
        healthy2.setHealthy(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, healthy2);

        // Unhealthy instance
        Instance unhealthy = new Instance();
        unhealthy.setIp("192.168.38.3");
        unhealthy.setPort(8080);
        unhealthy.setHealthy(false);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, unhealthy);

        Thread.sleep(1000);

        // Select healthy only
        List<Instance> healthyInstances = namingService.selectInstances(serviceName, DEFAULT_GROUP, true);

        assertFalse(healthyInstances.isEmpty(), "Should find healthy instances");
        for (Instance inst : healthyInstances) {
            assertTrue(inst.isHealthy(), "All selected instances should be healthy");
            assertNotEquals("192.168.38.3", inst.getIp(), "Unhealthy instance should not be selected");
        }
        assertTrue(healthyInstances.size() >= 2,
                "Should have at least 2 healthy instances, got: " + healthyInstances.size());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, healthy1);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, healthy2);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, unhealthy);
    }

    /**
     * NSS-009: Test select including unhealthy
     */
    @Test
    @Order(9)
    void testSelectIncludingUnhealthy() throws NacosException, InterruptedException {
        String serviceName = "selector-all-" + UUID.randomUUID().toString().substring(0, 8);

        Instance healthy = new Instance();
        healthy.setIp("192.168.39.1");
        healthy.setPort(8080);
        healthy.setHealthy(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, healthy);
        Thread.sleep(2000);

        Instance unhealthy = new Instance();
        unhealthy.setIp("192.168.39.2");
        unhealthy.setPort(8080);
        unhealthy.setHealthy(false);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, unhealthy);

        Thread.sleep(3000);

        // Select all (including unhealthy) using non-subscribe query
        List<Instance> allInstances = namingService.selectInstances(serviceName, DEFAULT_GROUP, new ArrayList<>(), false, false);

        assertNotNull(allInstances, "Instance list should not be null");
        assertTrue(allInstances.size() >= 2,
                "Should have at least 2 instances (healthy + unhealthy), got: " + allInstances.size());

        Set<String> ips = allInstances.stream().map(Instance::getIp).collect(Collectors.toSet());
        assertTrue(ips.contains("192.168.39.1"), "Should include the healthy instance");
        assertTrue(ips.contains("192.168.39.2"), "Should include the unhealthy instance");

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, healthy);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, unhealthy);
    }

    // ==================== Empty Service Selection Tests ====================

    /**
     * NSS-010: Test select from empty service
     */
    @Test
    @Order(10)
    void testSelectFromEmptyService() throws NacosException {
        String serviceName = "selector-empty-" + UUID.randomUUID().toString().substring(0, 8);

        // selectOneHealthyInstance on an empty/non-existent service throws an exception.
        // Nacos throws NacosException or IllegalStateException with "no host to srv" message.
        Exception thrown = assertThrows(Exception.class, () -> {
            namingService.selectOneHealthyInstance(serviceName, DEFAULT_GROUP);
        }, "selectOneHealthyInstance on empty service should throw an exception");
        assertNotNull(thrown.getMessage(), "Exception should have a message");
        System.out.println("Expected exception for empty service: " + thrown.getClass().getSimpleName() + ": " + thrown.getMessage());
    }

    /**
     * NSS-011: Test select when all unhealthy
     */
    @Test
    @Order(11)
    void testSelectWhenAllUnhealthy() throws NacosException, InterruptedException {
        String serviceName = "selector-all-unhealthy-" + UUID.randomUUID().toString().substring(0, 8);

        // Register only unhealthy instances
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.40." + i);
            instance.setPort(8080);
            instance.setHealthy(false);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        // Try to select healthy - should return empty list
        List<Instance> healthyInstances = namingService.selectInstances(serviceName, DEFAULT_GROUP, true);
        assertNotNull(healthyInstances, "Instance list should not be null");
        assertEquals(0, healthyInstances.size(),
                "Should find no healthy instances when all are unhealthy");

        // Cleanup
        for (int i = 0; i < 3; i++) {
            namingService.deregisterInstance(serviceName, "192.168.40." + i, 8080);
        }
    }

    // ==================== Concurrent Selection Tests ====================

    /**
     * NSS-012: Test concurrent selections
     */
    @Test
    @Order(12)
    void testConcurrentSelections() throws NacosException, InterruptedException {
        String serviceName = "selector-concurrent-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances
        for (int i = 0; i < 5; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.41." + i);
            instance.setPort(8080);
            instance.setHealthy(true);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        int threadCount = 20;
        CountDownLatch latch = new CountDownLatch(threadCount);
        List<Instance> selections = new CopyOnWriteArrayList<>();
        List<Exception> errors = new CopyOnWriteArrayList<>();

        for (int i = 0; i < threadCount; i++) {
            new Thread(() -> {
                try {
                    Instance selected = namingService.selectOneHealthyInstance(serviceName, DEFAULT_GROUP);
                    if (selected != null) {
                        selections.add(selected);
                    }
                } catch (Exception e) {
                    errors.add(e);
                } finally {
                    latch.countDown();
                }
            }).start();
        }

        boolean completed = latch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete within timeout");
        assertTrue(errors.isEmpty(),
                "No errors should occur during concurrent selections, but got " + errors.size() + " errors");
        assertFalse(selections.isEmpty(),
                "Should have successful selections from concurrent threads");

        for (Instance selected : selections) {
            assertTrue(selected.isHealthy(), "All concurrently selected instances should be healthy");
            assertTrue(selected.getIp().startsWith("192.168.41."),
                    "All selected IPs should be from registered range");
        }

        // Cleanup
        for (int i = 0; i < 5; i++) {
            namingService.deregisterInstance(serviceName, "192.168.41." + i, 8080);
        }
    }

    /**
     * NSS-013: Test selection during registration changes
     */
    @Test
    @Order(13)
    void testSelectionDuringChanges() throws NacosException, InterruptedException {
        String serviceName = "selector-changes-" + UUID.randomUUID().toString().substring(0, 8);

        // Initial instances
        for (int i = 0; i < 3; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.42." + i);
            instance.setPort(8080);
            instance.setHealthy(true);
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(500);

        // Concurrent selection and registration
        CountDownLatch latch = new CountDownLatch(10);
        List<Instance> selections = new CopyOnWriteArrayList<>();
        List<Exception> selectionErrors = new CopyOnWriteArrayList<>();

        // Selection threads
        for (int i = 0; i < 5; i++) {
            new Thread(() -> {
                try {
                    for (int j = 0; j < 10; j++) {
                        Instance selected = namingService.selectOneHealthyInstance(serviceName, DEFAULT_GROUP);
                        if (selected != null) {
                            selections.add(selected);
                        }
                        Thread.sleep(50);
                    }
                } catch (Exception e) {
                    selectionErrors.add(e);
                } finally {
                    latch.countDown();
                }
            }).start();
        }

        // Registration threads
        for (int i = 0; i < 5; i++) {
            final int idx = i;
            new Thread(() -> {
                try {
                    Instance instance = new Instance();
                    instance.setIp("192.168.43." + idx);
                    instance.setPort(8080);
                    instance.setHealthy(true);
                    namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
                    Thread.sleep(100);
                    namingService.deregisterInstance(serviceName, "192.168.43." + idx, 8080);
                } catch (Exception e) {
                    // Registration errors during concurrent changes are expected
                } finally {
                    latch.countDown();
                }
            }).start();
        }

        boolean completed = latch.await(60, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete within timeout");
        assertFalse(selections.isEmpty(),
                "Should have successful selections even during concurrent registration changes");

        for (Instance selected : selections) {
            assertNotNull(selected.getIp(), "Selected instance should have an IP");
            assertTrue(selected.isHealthy(), "Selected instances should be healthy");
        }

        // Cleanup
        for (int i = 0; i < 3; i++) {
            try {
                namingService.deregisterInstance(serviceName, "192.168.42." + i, 8080);
            } catch (Exception e) {
                // Ignore
            }
        }
    }

    // ==================== Metadata Based Selection Tests ====================

    /**
     * NSS-014: Test select by metadata filter (client-side)
     */
    @Test
    @Order(14)
    void testSelectByMetadataFilter() throws NacosException, InterruptedException {
        String serviceName = "selector-metadata-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances with different metadata
        for (int i = 0; i < 4; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.44." + i);
            instance.setPort(8080);
            instance.setHealthy(true);

            Map<String, String> metadata = new HashMap<>();
            metadata.put("version", i < 2 ? "v1" : "v2");
            metadata.put("region", i % 2 == 0 ? "east" : "west");
            instance.setMetadata(metadata);

            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        // Get all and filter by metadata
        List<Instance> allInstances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertEquals(4, allInstances.size(), "Should have 4 total instances");

        // Filter v1 instances
        List<Instance> v1Instances = allInstances.stream()
                .filter(i -> "v1".equals(i.getMetadata().get("version")))
                .collect(Collectors.toList());

        assertEquals(2, v1Instances.size(), "Should have 2 v1 instances");
        for (Instance inst : v1Instances) {
            assertEquals("v1", inst.getMetadata().get("version"),
                    "Filtered instance should have version=v1");
        }

        // Filter east region
        List<Instance> eastInstances = allInstances.stream()
                .filter(i -> "east".equals(i.getMetadata().get("region")))
                .collect(Collectors.toList());

        assertEquals(2, eastInstances.size(), "Should have 2 east region instances");
        for (Instance inst : eastInstances) {
            assertEquals("east", inst.getMetadata().get("region"),
                    "Filtered instance should have region=east");
        }

        // Cleanup
        for (int i = 0; i < 4; i++) {
            namingService.deregisterInstance(serviceName, "192.168.44." + i, 8080);
        }
    }

    /**
     * NSS-015: Test select with combined filters
     */
    @Test
    @Order(15)
    void testSelectWithCombinedFilters() throws NacosException, InterruptedException {
        String serviceName = "selector-combined-" + UUID.randomUUID().toString().substring(0, 8);
        String cluster = "prod-cluster";

        // Register instances
        for (int i = 0; i < 6; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.45." + i);
            instance.setPort(8080);
            instance.setClusterName(i < 3 ? cluster : "dev-cluster");
            instance.setHealthy(i % 2 == 0);
            instance.setWeight(i < 3 ? 10.0 : 1.0);

            Map<String, String> metadata = new HashMap<>();
            metadata.put("env", i < 3 ? "prod" : "dev");
            instance.setMetadata(metadata);

            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        }

        Thread.sleep(1000);

        // Combined filter: healthy + cluster + metadata
        List<Instance> filtered = namingService.selectInstances(
                        serviceName, DEFAULT_GROUP, Arrays.asList(cluster), true)
                .stream()
                .filter(i -> "prod".equals(i.getMetadata().get("env")))
                .collect(Collectors.toList());

        assertNotNull(filtered, "Filtered list should not be null");
        assertFalse(filtered.isEmpty(), "Should find instances matching combined filters");

        for (Instance inst : filtered) {
            assertEquals(cluster, inst.getClusterName(),
                    "All instances should belong to prod-cluster");
            assertTrue(inst.isHealthy(),
                    "All instances should be healthy");
            assertEquals("prod", inst.getMetadata().get("env"),
                    "All instances should have env=prod metadata");
        }

        // Cleanup
        for (int i = 0; i < 6; i++) {
            try {
                namingService.deregisterInstance(serviceName, "192.168.45." + i, 8080,
                        i < 3 ? cluster : "dev-cluster");
            } catch (Exception e) {
                // Ignore
            }
        }
    }
}
