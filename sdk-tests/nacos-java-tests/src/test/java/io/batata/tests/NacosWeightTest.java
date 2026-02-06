package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.listener.EventListener;
import com.alibaba.nacos.api.naming.listener.NamingEvent;
import com.alibaba.nacos.api.naming.pojo.Instance;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Weight and Load Balancing Tests
 *
 * Tests for service instance weight configuration and weight-based load balancing:
 * - Default and custom weight values
 * - Weight boundaries (zero, maximum)
 * - Weight updates and persistence
 * - Weighted random selection distribution
 * - Weight interaction with health status and clusters
 * - Concurrent weight updates
 * - Traffic distribution based on weight
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosWeightTest {

    private static NamingService namingService;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";

    @BeforeAll
    static void setup() throws NacosException {
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        namingService = NacosFactory.createNamingService(properties);
        System.out.println("Nacos Weight Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (namingService != null) {
            namingService.shutDown();
        }
    }

    // ==================== Weight Value Tests ====================

    /**
     * NWT-001: Test default weight value
     *
     * Verifies that when no weight is explicitly set, the instance uses the default weight value (1.0).
     */
    @Test
    @Order(1)
    void testDefaultWeightValue() throws NacosException, InterruptedException {
        String serviceName = "nwt001-default-weight-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instance without setting weight
        namingService.registerInstance(serviceName, "192.168.200.1", 8080);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty(), "Should have registered instance");

        Instance instance = instances.get(0);
        assertEquals(1.0, instance.getWeight(), 0.001, "Default weight should be 1.0");

        System.out.println("Default weight: " + instance.getWeight());

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.1", 8080);
    }

    /**
     * NWT-002: Test set custom weight
     *
     * Verifies that custom weight values can be set and retrieved correctly.
     */
    @Test
    @Order(2)
    void testSetCustomWeight() throws NacosException, InterruptedException {
        String serviceName = "nwt002-custom-weight-" + UUID.randomUUID().toString().substring(0, 8);

        // Register with custom weight
        Instance instance = new Instance();
        instance.setIp("192.168.200.2");
        instance.setPort(8080);
        instance.setWeight(5.5);
        instance.setHealthy(true);

        namingService.registerInstance(serviceName, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty());

        Instance retrieved = instances.get(0);
        assertEquals(5.5, retrieved.getWeight(), 0.001, "Custom weight should be 5.5");

        System.out.println("Custom weight set: " + retrieved.getWeight());

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.2", 8080);
    }

    /**
     * NWT-003: Test weight zero (disabled)
     *
     * Verifies that instances with weight 0 are effectively disabled from load balancing selection.
     */
    @Test
    @Order(3)
    void testWeightZeroDisabled() throws NacosException, InterruptedException {
        String serviceName = "nwt003-zero-weight-" + UUID.randomUUID().toString().substring(0, 8);

        // Register zero weight instance
        Instance zeroWeight = new Instance();
        zeroWeight.setIp("192.168.200.3");
        zeroWeight.setPort(8080);
        zeroWeight.setWeight(0.0);
        zeroWeight.setHealthy(true);
        namingService.registerInstance(serviceName, zeroWeight);

        // Register normal weight instance
        Instance normalWeight = new Instance();
        normalWeight.setIp("192.168.200.4");
        normalWeight.setPort(8080);
        normalWeight.setWeight(1.0);
        normalWeight.setHealthy(true);
        namingService.registerInstance(serviceName, normalWeight);

        Thread.sleep(1000);

        // Zero weight instance should rarely or never be selected
        int zeroSelected = 0;
        int normalSelected = 0;
        for (int i = 0; i < 100; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName);
            if (selected != null) {
                if (selected.getIp().equals("192.168.200.3")) {
                    zeroSelected++;
                } else {
                    normalSelected++;
                }
            }
        }

        System.out.println("Zero weight selected: " + zeroSelected + ", Normal weight selected: " + normalSelected);
        assertTrue(zeroSelected == 0 || zeroSelected < normalSelected * 0.1,
                "Zero weight instance should rarely or never be selected");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.3", 8080);
        namingService.deregisterInstance(serviceName, "192.168.200.4", 8080);
    }

    /**
     * NWT-004: Test weight maximum value
     *
     * Verifies that large weight values are supported and work correctly.
     */
    @Test
    @Order(4)
    void testWeightMaximumValue() throws NacosException, InterruptedException {
        String serviceName = "nwt004-max-weight-" + UUID.randomUUID().toString().substring(0, 8);

        // Register with maximum weight
        Instance maxWeight = new Instance();
        maxWeight.setIp("192.168.200.5");
        maxWeight.setPort(8080);
        maxWeight.setWeight(10000.0);
        maxWeight.setHealthy(true);

        namingService.registerInstance(serviceName, maxWeight);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty());

        Instance retrieved = instances.get(0);
        assertEquals(10000.0, retrieved.getWeight(), 0.001, "Maximum weight should be preserved");

        System.out.println("Maximum weight: " + retrieved.getWeight());

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.5", 8080);
    }

    /**
     * NWT-005: Test weight update
     *
     * Verifies that instance weight can be dynamically updated.
     */
    @Test
    @Order(5)
    void testWeightUpdate() throws NacosException, InterruptedException {
        String serviceName = "nwt005-weight-update-" + UUID.randomUUID().toString().substring(0, 8);

        // Register with initial weight
        Instance instance = new Instance();
        instance.setIp("192.168.200.6");
        instance.setPort(8080);
        instance.setWeight(1.0);
        instance.setHealthy(true);

        namingService.registerInstance(serviceName, instance);
        Thread.sleep(1000);

        // Verify initial weight
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertEquals(1.0, instances.get(0).getWeight(), 0.001, "Initial weight should be 1.0");

        // Update weight
        instance.setWeight(8.0);
        namingService.registerInstance(serviceName, instance);
        Thread.sleep(1000);

        // Verify updated weight
        instances = namingService.getAllInstances(serviceName);
        assertEquals(8.0, instances.get(0).getWeight(), 0.001, "Updated weight should be 8.0");

        System.out.println("Weight updated from 1.0 to 8.0");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.6", 8080);
    }

    // ==================== Load Balancing Tests ====================

    /**
     * NWT-006: Test weighted random selection
     *
     * Verifies that selectOneHealthyInstance respects instance weights for load balancing.
     */
    @Test
    @Order(6)
    void testWeightedRandomSelection() throws NacosException, InterruptedException {
        String serviceName = "nwt006-weighted-selection-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instance with high weight
        Instance highWeight = new Instance();
        highWeight.setIp("192.168.200.10");
        highWeight.setPort(8080);
        highWeight.setWeight(9.0);
        highWeight.setHealthy(true);
        namingService.registerInstance(serviceName, highWeight);

        // Register instance with low weight
        Instance lowWeight = new Instance();
        lowWeight.setIp("192.168.200.11");
        lowWeight.setPort(8080);
        lowWeight.setWeight(1.0);
        lowWeight.setHealthy(true);
        namingService.registerInstance(serviceName, lowWeight);

        Thread.sleep(1000);

        // Select many times and count distribution
        Map<String, Integer> selectionCount = new HashMap<>();
        for (int i = 0; i < 1000; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName);
            assertNotNull(selected, "Should select an instance");
            selectionCount.merge(selected.getIp(), 1, Integer::sum);
        }

        int highCount = selectionCount.getOrDefault("192.168.200.10", 0);
        int lowCount = selectionCount.getOrDefault("192.168.200.11", 0);

        System.out.println("High weight (9.0) selected: " + highCount);
        System.out.println("Low weight (1.0) selected: " + lowCount);

        // High weight should be selected significantly more often
        assertTrue(highCount > lowCount, "High weight instance should be selected more often");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.10", 8080);
        namingService.deregisterInstance(serviceName, "192.168.200.11", 8080);
    }

    /**
     * NWT-007: Test weight distribution accuracy
     *
     * Verifies that the selection distribution approximately matches the weight ratio.
     */
    @Test
    @Order(7)
    void testWeightDistributionAccuracy() throws NacosException, InterruptedException {
        String serviceName = "nwt007-distribution-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances with 1:2:3 weight ratio
        Instance weight1 = new Instance();
        weight1.setIp("192.168.200.20");
        weight1.setPort(8080);
        weight1.setWeight(1.0);
        weight1.setHealthy(true);
        namingService.registerInstance(serviceName, weight1);

        Instance weight2 = new Instance();
        weight2.setIp("192.168.200.21");
        weight2.setPort(8080);
        weight2.setWeight(2.0);
        weight2.setHealthy(true);
        namingService.registerInstance(serviceName, weight2);

        Instance weight3 = new Instance();
        weight3.setIp("192.168.200.22");
        weight3.setPort(8080);
        weight3.setWeight(3.0);
        weight3.setHealthy(true);
        namingService.registerInstance(serviceName, weight3);

        Thread.sleep(1000);

        // Run many selections
        Map<String, Integer> counts = new HashMap<>();
        int totalSelections = 6000;
        for (int i = 0; i < totalSelections; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName);
            assertNotNull(selected);
            counts.merge(selected.getIp(), 1, Integer::sum);
        }

        int count1 = counts.getOrDefault("192.168.200.20", 0);
        int count2 = counts.getOrDefault("192.168.200.21", 0);
        int count3 = counts.getOrDefault("192.168.200.22", 0);

        System.out.println("Weight 1.0 selected: " + count1 + " (" + (count1 * 100.0 / totalSelections) + "%)");
        System.out.println("Weight 2.0 selected: " + count2 + " (" + (count2 * 100.0 / totalSelections) + "%)");
        System.out.println("Weight 3.0 selected: " + count3 + " (" + (count3 * 100.0 / totalSelections) + "%)");

        // Expected ratios: 1/6 (~16.7%), 2/6 (~33.3%), 3/6 (~50%)
        // Allow 10% tolerance for randomness
        double ratio1 = (double) count1 / totalSelections;
        double ratio2 = (double) count2 / totalSelections;
        double ratio3 = (double) count3 / totalSelections;

        assertTrue(ratio1 > 0.05 && ratio1 < 0.30, "Weight 1 ratio should be around 16.7%");
        assertTrue(ratio2 > 0.20 && ratio2 < 0.50, "Weight 2 ratio should be around 33.3%");
        assertTrue(ratio3 > 0.35 && ratio3 < 0.65, "Weight 3 ratio should be around 50%");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.20", 8080);
        namingService.deregisterInstance(serviceName, "192.168.200.21", 8080);
        namingService.deregisterInstance(serviceName, "192.168.200.22", 8080);
    }

    /**
     * NWT-008: Test weight with multiple instances
     *
     * Verifies weight-based selection works correctly with many instances.
     */
    @Test
    @Order(8)
    void testWeightWithMultipleInstances() throws NacosException, InterruptedException {
        String serviceName = "nwt008-multi-instance-" + UUID.randomUUID().toString().substring(0, 8);

        // Register 10 instances with varying weights
        List<Instance> instances = new ArrayList<>();
        for (int i = 0; i < 10; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.200." + (30 + i));
            instance.setPort(8080);
            instance.setWeight(1.0 + i); // Weights: 1, 2, 3, ... 10
            instance.setHealthy(true);
            instances.add(instance);
            namingService.registerInstance(serviceName, instance);
        }

        Thread.sleep(1500);

        // Verify all instances registered
        List<Instance> allInstances = namingService.getAllInstances(serviceName);
        assertEquals(10, allInstances.size(), "Should have 10 instances");

        // Select many times
        Map<String, Integer> counts = new HashMap<>();
        for (int i = 0; i < 1000; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName);
            assertNotNull(selected);
            counts.merge(selected.getIp(), 1, Integer::sum);
        }

        // Higher weight instances should generally be selected more
        int lowWeightSelections = 0;
        int highWeightSelections = 0;
        for (int i = 0; i < 5; i++) {
            lowWeightSelections += counts.getOrDefault("192.168.200." + (30 + i), 0);
        }
        for (int i = 5; i < 10; i++) {
            highWeightSelections += counts.getOrDefault("192.168.200." + (30 + i), 0);
        }

        System.out.println("Low weight (1-5) selections: " + lowWeightSelections);
        System.out.println("High weight (6-10) selections: " + highWeightSelections);
        assertTrue(highWeightSelections > lowWeightSelections,
                "Higher weight instances should be selected more often");

        // Cleanup
        for (Instance inst : instances) {
            namingService.deregisterInstance(serviceName, inst.getIp(), inst.getPort());
        }
    }

    /**
     * NWT-009: Test weight persistence after restart
     *
     * Verifies that weight values persist after re-registration.
     */
    @Test
    @Order(9)
    void testWeightPersistenceAfterRestart() throws NacosException, InterruptedException {
        String serviceName = "nwt009-persistence-" + UUID.randomUUID().toString().substring(0, 8);

        // Register with specific weight
        Instance instance = new Instance();
        instance.setIp("192.168.200.40");
        instance.setPort(8080);
        instance.setWeight(7.5);
        instance.setHealthy(true);

        namingService.registerInstance(serviceName, instance);
        Thread.sleep(1000);

        // Verify weight
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertEquals(7.5, instances.get(0).getWeight(), 0.001, "Weight should be 7.5");

        // Deregister
        namingService.deregisterInstance(serviceName, "192.168.200.40", 8080);
        Thread.sleep(500);

        // Re-register with same weight
        namingService.registerInstance(serviceName, instance);
        Thread.sleep(1000);

        // Verify weight persists
        instances = namingService.getAllInstances(serviceName);
        assertEquals(7.5, instances.get(0).getWeight(), 0.001, "Weight should persist as 7.5");

        System.out.println("Weight persisted after re-registration: " + instances.get(0).getWeight());

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.40", 8080);
    }

    /**
     * NWT-010: Test weight with health status
     *
     * Verifies that unhealthy instances are excluded from weighted selection regardless of weight.
     */
    @Test
    @Order(10)
    void testWeightWithHealthStatus() throws NacosException, InterruptedException {
        String serviceName = "nwt010-health-weight-" + UUID.randomUUID().toString().substring(0, 8);

        // Register high weight but unhealthy instance
        Instance unhealthyHighWeight = new Instance();
        unhealthyHighWeight.setIp("192.168.200.50");
        unhealthyHighWeight.setPort(8080);
        unhealthyHighWeight.setWeight(100.0);
        unhealthyHighWeight.setHealthy(false);
        namingService.registerInstance(serviceName, unhealthyHighWeight);

        // Register low weight healthy instance
        Instance healthyLowWeight = new Instance();
        healthyLowWeight.setIp("192.168.200.51");
        healthyLowWeight.setPort(8080);
        healthyLowWeight.setWeight(1.0);
        healthyLowWeight.setHealthy(true);
        namingService.registerInstance(serviceName, healthyLowWeight);

        Thread.sleep(1000);

        // Select healthy instances only
        List<Instance> healthyInstances = namingService.selectInstances(serviceName, DEFAULT_GROUP, true);
        assertEquals(1, healthyInstances.size(), "Should have 1 healthy instance");
        assertEquals("192.168.200.51", healthyInstances.get(0).getIp());

        // selectOneHealthyInstance should only return healthy instance
        int unhealthySelected = 0;
        for (int i = 0; i < 100; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName);
            if (selected != null && selected.getIp().equals("192.168.200.50")) {
                unhealthySelected++;
            }
        }

        assertEquals(0, unhealthySelected, "Unhealthy instance should never be selected");
        System.out.println("Unhealthy high-weight instance never selected (as expected)");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.50", 8080);
        namingService.deregisterInstance(serviceName, "192.168.200.51", 8080);
    }

    /**
     * NWT-011: Test dynamic weight adjustment
     *
     * Verifies that changing weight dynamically affects selection distribution.
     */
    @Test
    @Order(11)
    void testDynamicWeightAdjustment() throws NacosException, InterruptedException {
        String serviceName = "nwt011-dynamic-weight-" + UUID.randomUUID().toString().substring(0, 8);

        // Register two instances with equal weight
        Instance instance1 = new Instance();
        instance1.setIp("192.168.200.60");
        instance1.setPort(8080);
        instance1.setWeight(1.0);
        instance1.setHealthy(true);
        namingService.registerInstance(serviceName, instance1);

        Instance instance2 = new Instance();
        instance2.setIp("192.168.200.61");
        instance2.setPort(8080);
        instance2.setWeight(1.0);
        instance2.setHealthy(true);
        namingService.registerInstance(serviceName, instance2);

        Thread.sleep(1000);

        // Initial selection should be roughly equal
        Map<String, Integer> initialCounts = new HashMap<>();
        for (int i = 0; i < 200; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName);
            initialCounts.merge(selected.getIp(), 1, Integer::sum);
        }

        System.out.println("Initial distribution (equal weights): " + initialCounts);

        // Increase weight of instance1
        instance1.setWeight(9.0);
        namingService.registerInstance(serviceName, instance1);
        Thread.sleep(1000);

        // New selection should favor instance1
        Map<String, Integer> newCounts = new HashMap<>();
        for (int i = 0; i < 200; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName);
            newCounts.merge(selected.getIp(), 1, Integer::sum);
        }

        System.out.println("After weight adjustment (9:1 ratio): " + newCounts);

        int inst1Count = newCounts.getOrDefault("192.168.200.60", 0);
        int inst2Count = newCounts.getOrDefault("192.168.200.61", 0);

        assertTrue(inst1Count > inst2Count, "Instance with higher weight should be selected more");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.60", 8080);
        namingService.deregisterInstance(serviceName, "192.168.200.61", 8080);
    }

    /**
     * NWT-012: Test weight in cluster selection
     *
     * Verifies that weight-based selection works correctly within a specific cluster.
     */
    @Test
    @Order(12)
    void testWeightInClusterSelection() throws NacosException, InterruptedException {
        String serviceName = "nwt012-cluster-weight-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances in cluster-A with different weights
        Instance clusterAHigh = new Instance();
        clusterAHigh.setIp("192.168.200.70");
        clusterAHigh.setPort(8080);
        clusterAHigh.setClusterName("cluster-A");
        clusterAHigh.setWeight(9.0);
        clusterAHigh.setHealthy(true);
        namingService.registerInstance(serviceName, clusterAHigh);

        Instance clusterALow = new Instance();
        clusterALow.setIp("192.168.200.71");
        clusterALow.setPort(8080);
        clusterALow.setClusterName("cluster-A");
        clusterALow.setWeight(1.0);
        clusterALow.setHealthy(true);
        namingService.registerInstance(serviceName, clusterALow);

        // Register instance in cluster-B
        Instance clusterB = new Instance();
        clusterB.setIp("192.168.200.72");
        clusterB.setPort(8080);
        clusterB.setClusterName("cluster-B");
        clusterB.setWeight(5.0);
        clusterB.setHealthy(true);
        namingService.registerInstance(serviceName, clusterB);

        Thread.sleep(1000);

        // Select from cluster-A only
        List<String> clusters = Arrays.asList("cluster-A");
        Map<String, Integer> counts = new HashMap<>();
        for (int i = 0; i < 200; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName, DEFAULT_GROUP, clusters, true);
            assertNotNull(selected);
            assertEquals("cluster-A", selected.getClusterName(), "Should only select from cluster-A");
            counts.merge(selected.getIp(), 1, Integer::sum);
        }

        int highWeightCount = counts.getOrDefault("192.168.200.70", 0);
        int lowWeightCount = counts.getOrDefault("192.168.200.71", 0);

        System.out.println("Cluster-A selection - High weight: " + highWeightCount + ", Low weight: " + lowWeightCount);
        assertTrue(highWeightCount > lowWeightCount, "High weight instance should be selected more in cluster");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.70", 8080);
        namingService.deregisterInstance(serviceName, "192.168.200.71", 8080);
        namingService.deregisterInstance(serviceName, "192.168.200.72", 8080);
    }

    /**
     * NWT-013: Test weight metadata
     *
     * Verifies that weight information can be stored and retrieved alongside instance metadata.
     */
    @Test
    @Order(13)
    void testWeightMetadata() throws NacosException, InterruptedException {
        String serviceName = "nwt013-weight-metadata-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instance with weight and related metadata
        Instance instance = new Instance();
        instance.setIp("192.168.200.80");
        instance.setPort(8080);
        instance.setWeight(5.0);
        instance.setHealthy(true);

        Map<String, String> metadata = new HashMap<>();
        metadata.put("weightCategory", "medium");
        metadata.put("targetWeight", "5.0");
        metadata.put("minWeight", "1.0");
        metadata.put("maxWeight", "10.0");
        metadata.put("autoScale", "true");
        instance.setMetadata(metadata);

        namingService.registerInstance(serviceName, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty());

        Instance retrieved = instances.get(0);
        assertEquals(5.0, retrieved.getWeight(), 0.001);
        assertEquals("medium", retrieved.getMetadata().get("weightCategory"));
        assertEquals("5.0", retrieved.getMetadata().get("targetWeight"));
        assertEquals("true", retrieved.getMetadata().get("autoScale"));

        System.out.println("Weight: " + retrieved.getWeight());
        System.out.println("Weight metadata: " + retrieved.getMetadata());

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.80", 8080);
    }

    /**
     * NWT-014: Test concurrent weight updates
     *
     * Verifies that concurrent weight updates are handled correctly.
     */
    @Test
    @Order(14)
    void testConcurrentWeightUpdates() throws NacosException, InterruptedException {
        String serviceName = "nwt014-concurrent-weight-" + UUID.randomUUID().toString().substring(0, 8);

        // Register initial instance
        Instance instance = new Instance();
        instance.setIp("192.168.200.90");
        instance.setPort(8080);
        instance.setWeight(1.0);
        instance.setHealthy(true);
        namingService.registerInstance(serviceName, instance);
        Thread.sleep(500);

        // Perform concurrent weight updates
        int numThreads = 5;
        int updatesPerThread = 10;
        ExecutorService executor = Executors.newFixedThreadPool(numThreads);
        CountDownLatch latch = new CountDownLatch(numThreads);
        AtomicInteger successCount = new AtomicInteger(0);
        AtomicInteger errorCount = new AtomicInteger(0);

        for (int t = 0; t < numThreads; t++) {
            final int threadId = t;
            executor.submit(() -> {
                try {
                    for (int i = 0; i < updatesPerThread; i++) {
                        Instance updateInstance = new Instance();
                        updateInstance.setIp("192.168.200.90");
                        updateInstance.setPort(8080);
                        updateInstance.setWeight(1.0 + threadId + (i * 0.1));
                        updateInstance.setHealthy(true);
                        namingService.registerInstance(serviceName, updateInstance);
                        successCount.incrementAndGet();
                        Thread.sleep(50);
                    }
                } catch (Exception e) {
                    errorCount.incrementAndGet();
                    e.printStackTrace();
                } finally {
                    latch.countDown();
                }
            });
        }

        latch.await(30, TimeUnit.SECONDS);
        executor.shutdown();

        Thread.sleep(1000);

        // Verify instance still exists and has a valid weight
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty(), "Instance should still exist after concurrent updates");

        Instance finalInstance = instances.get(0);
        assertTrue(finalInstance.getWeight() > 0, "Weight should be positive");

        System.out.println("Concurrent updates - Success: " + successCount.get() + ", Errors: " + errorCount.get());
        System.out.println("Final weight: " + finalInstance.getWeight());

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.90", 8080);
    }

    /**
     * NWT-015: Test weight-based traffic distribution
     *
     * Verifies that weighted load balancing distributes traffic proportionally to weights
     * across multiple service instances.
     */
    @Test
    @Order(15)
    void testWeightBasedTrafficDistribution() throws NacosException, InterruptedException {
        String serviceName = "nwt015-traffic-dist-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances simulating a real deployment scenario:
        // 2 primary instances (high weight), 2 secondary instances (medium weight), 1 backup (low weight)
        Instance primary1 = new Instance();
        primary1.setIp("192.168.201.1");
        primary1.setPort(8080);
        primary1.setWeight(10.0);
        primary1.setHealthy(true);
        primary1.setMetadata(Map.of("tier", "primary", "dc", "us-east"));
        namingService.registerInstance(serviceName, primary1);

        Instance primary2 = new Instance();
        primary2.setIp("192.168.201.2");
        primary2.setPort(8080);
        primary2.setWeight(10.0);
        primary2.setHealthy(true);
        primary2.setMetadata(Map.of("tier", "primary", "dc", "us-west"));
        namingService.registerInstance(serviceName, primary2);

        Instance secondary1 = new Instance();
        secondary1.setIp("192.168.201.3");
        secondary1.setPort(8080);
        secondary1.setWeight(5.0);
        secondary1.setHealthy(true);
        secondary1.setMetadata(Map.of("tier", "secondary", "dc", "eu-west"));
        namingService.registerInstance(serviceName, secondary1);

        Instance secondary2 = new Instance();
        secondary2.setIp("192.168.201.4");
        secondary2.setPort(8080);
        secondary2.setWeight(5.0);
        secondary2.setHealthy(true);
        secondary2.setMetadata(Map.of("tier", "secondary", "dc", "ap-south"));
        namingService.registerInstance(serviceName, secondary2);

        Instance backup = new Instance();
        backup.setIp("192.168.201.5");
        backup.setPort(8080);
        backup.setWeight(1.0);
        backup.setHealthy(true);
        backup.setMetadata(Map.of("tier", "backup", "dc", "backup-dc"));
        namingService.registerInstance(serviceName, backup);

        Thread.sleep(1500);

        // Simulate traffic distribution
        int totalRequests = 10000;
        Map<String, Integer> distribution = new HashMap<>();
        Map<String, Integer> tierDistribution = new HashMap<>();

        for (int i = 0; i < totalRequests; i++) {
            Instance selected = namingService.selectOneHealthyInstance(serviceName);
            assertNotNull(selected);
            distribution.merge(selected.getIp(), 1, Integer::sum);
            tierDistribution.merge(selected.getMetadata().get("tier"), 1, Integer::sum);
        }

        // Calculate and print distribution
        System.out.println("\n=== Traffic Distribution Analysis ===");
        System.out.println("Total requests: " + totalRequests);
        System.out.println("\nPer-instance distribution:");
        for (Map.Entry<String, Integer> entry : distribution.entrySet()) {
            double percentage = (entry.getValue() * 100.0) / totalRequests;
            System.out.printf("  %s: %d (%.2f%%)%n", entry.getKey(), entry.getValue(), percentage);
        }

        System.out.println("\nPer-tier distribution:");
        for (Map.Entry<String, Integer> entry : tierDistribution.entrySet()) {
            double percentage = (entry.getValue() * 100.0) / totalRequests;
            System.out.printf("  %s: %d (%.2f%%)%n", entry.getKey(), entry.getValue(), percentage);
        }

        // Total weight: 10+10+5+5+1 = 31
        // Expected: primary ~64.5% (20/31), secondary ~32.3% (10/31), backup ~3.2% (1/31)
        int primaryCount = tierDistribution.getOrDefault("primary", 0);
        int secondaryCount = tierDistribution.getOrDefault("secondary", 0);
        int backupCount = tierDistribution.getOrDefault("backup", 0);

        double primaryRatio = (double) primaryCount / totalRequests;
        double secondaryRatio = (double) secondaryCount / totalRequests;
        double backupRatio = (double) backupCount / totalRequests;

        // Verify distribution is approximately correct (with 10% tolerance)
        assertTrue(primaryRatio > 0.50 && primaryRatio < 0.80,
                "Primary tier should receive ~64.5% of traffic, got: " + (primaryRatio * 100) + "%");
        assertTrue(secondaryRatio > 0.20 && secondaryRatio < 0.45,
                "Secondary tier should receive ~32.3% of traffic, got: " + (secondaryRatio * 100) + "%");
        assertTrue(backupRatio > 0.00 && backupRatio < 0.10,
                "Backup tier should receive ~3.2% of traffic, got: " + (backupRatio * 100) + "%");

        // Verify primary instances receive more traffic than secondary
        assertTrue(primaryCount > secondaryCount, "Primary tier should receive more traffic than secondary");
        assertTrue(secondaryCount > backupCount, "Secondary tier should receive more traffic than backup");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.201.1", 8080);
        namingService.deregisterInstance(serviceName, "192.168.201.2", 8080);
        namingService.deregisterInstance(serviceName, "192.168.201.3", 8080);
        namingService.deregisterInstance(serviceName, "192.168.201.4", 8080);
        namingService.deregisterInstance(serviceName, "192.168.201.5", 8080);
    }
}
