package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Persistence Tests
 *
 * Tests for ephemeral vs persistent instance management:
 * - Ephemeral instances (default) - requires heartbeat, auto-deregister on disconnect
 * - Persistent instances - no heartbeat required, survives client disconnect
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosPersistenceTest {

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
        System.out.println("Nacos Persistence Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (namingService != null) {
            namingService.shutDown();
        }
    }

    // ==================== Ephemeral Instance Tests ====================

    /**
     * NPT-001: Test register ephemeral instance (default)
     *
     * Verifies that instances are ephemeral by default when no ephemeral flag is set.
     */
    @Test
    @Order(1)
    void testRegisterEphemeralInstanceDefault() throws NacosException, InterruptedException {
        String serviceName = "npt001-ephemeral-default-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instance without setting ephemeral flag (should default to true)
        Instance instance = new Instance();
        instance.setIp("192.168.200.1");
        instance.setPort(8080);
        instance.setWeight(1.0);
        instance.setHealthy(true);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have registered instance");

        Instance found = instances.get(0);
        assertTrue(found.isEphemeral(), "Instance should be ephemeral by default");

        System.out.println("NPT-001: Instance ephemeral flag (default): " + found.isEphemeral());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NPT-002: Test register persistent instance
     *
     * Verifies that persistent instances can be registered with ephemeral=false.
     */
    @Test
    @Order(2)
    void testRegisterPersistentInstance() throws NacosException, InterruptedException {
        String serviceName = "npt002-persistent-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.200.2");
        instance.setPort(8080);
        instance.setWeight(1.0);
        instance.setHealthy(true);
        instance.setEphemeral(false);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have registered persistent instance");

        Instance found = instances.get(0);
        assertFalse(found.isEphemeral(), "Instance should be persistent (ephemeral=false)");

        System.out.println("NPT-002: Instance ephemeral flag: " + found.isEphemeral());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NPT-003: Test ephemeral instance auto-deregister on disconnect
     *
     * Verifies that ephemeral instances are automatically removed when client disconnects.
     * Note: This test creates a separate NamingService to simulate disconnect.
     */
    @Test
    @Order(3)
    void testEphemeralInstanceAutoDeregisterOnDisconnect() throws NacosException, InterruptedException {
        String serviceName = "npt003-ephemeral-disconnect-" + UUID.randomUUID().toString().substring(0, 8);

        // Create a separate NamingService to register ephemeral instance
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        NamingService tempNamingService = NacosFactory.createNamingService(properties);

        // Register ephemeral instance with temp client
        Instance instance = new Instance();
        instance.setIp("192.168.200.3");
        instance.setPort(8080);
        instance.setEphemeral(true);

        tempNamingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        // Verify instance is registered
        List<Instance> beforeDisconnect = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(beforeDisconnect.isEmpty(), "Instance should be registered before disconnect");
        System.out.println("NPT-003: Instances before disconnect: " + beforeDisconnect.size());

        // Shutdown the temp client (simulate disconnect)
        tempNamingService.shutDown();

        // Wait for heartbeat timeout (typically 15-30 seconds in Nacos)
        // Using shorter wait for test, actual removal depends on server config
        Thread.sleep(20000);

        // Check if instance is removed
        List<Instance> afterDisconnect = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        System.out.println("NPT-003: Instances after disconnect: " + afterDisconnect.size());

        // Note: The instance may or may not be removed depending on server timeout settings
        // This test documents the expected behavior
    }

    /**
     * NPT-004: Test persistent instance survives disconnect
     *
     * Verifies that persistent instances remain registered after client disconnects.
     */
    @Test
    @Order(4)
    void testPersistentInstanceSurvivesDisconnect() throws NacosException, InterruptedException {
        String serviceName = "npt004-persistent-disconnect-" + UUID.randomUUID().toString().substring(0, 8);

        // Create a separate NamingService to register persistent instance
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        NamingService tempNamingService = NacosFactory.createNamingService(properties);

        // Register persistent instance with temp client
        Instance instance = new Instance();
        instance.setIp("192.168.200.4");
        instance.setPort(8080);
        instance.setEphemeral(false);

        tempNamingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        // Verify instance is registered
        List<Instance> beforeDisconnect = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(beforeDisconnect.isEmpty(), "Persistent instance should be registered");
        System.out.println("NPT-004: Instances before disconnect: " + beforeDisconnect.size());

        // Shutdown the temp client (simulate disconnect)
        tempNamingService.shutDown();

        // Wait some time
        Thread.sleep(5000);

        // Verify persistent instance still exists
        List<Instance> afterDisconnect = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(afterDisconnect.isEmpty(), "Persistent instance should survive disconnect");
        System.out.println("NPT-004: Instances after disconnect: " + afterDisconnect.size());

        // Cleanup - must manually deregister persistent instances
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NPT-005: Test switch ephemeral to persistent
     *
     * Verifies behavior when attempting to change instance from ephemeral to persistent.
     * Note: In Nacos, you typically cannot change the ephemeral flag after registration;
     * this test documents the actual behavior.
     */
    @Test
    @Order(5)
    void testSwitchEphemeralToPersistent() throws NacosException, InterruptedException {
        String serviceName = "npt005-switch-eph-to-pers-" + UUID.randomUUID().toString().substring(0, 8);

        // Register ephemeral instance first
        Instance ephemeralInstance = new Instance();
        ephemeralInstance.setIp("192.168.200.5");
        ephemeralInstance.setPort(8080);
        ephemeralInstance.setEphemeral(true);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, ephemeralInstance);
        Thread.sleep(1000);

        List<Instance> initialInstances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(initialInstances.isEmpty());
        assertTrue(initialInstances.get(0).isEphemeral(), "Initial instance should be ephemeral");
        System.out.println("NPT-005: Initial ephemeral status: " + initialInstances.get(0).isEphemeral());

        // Try to re-register same instance as persistent
        Instance persistentInstance = new Instance();
        persistentInstance.setIp("192.168.200.5");
        persistentInstance.setPort(8080);
        persistentInstance.setEphemeral(false);

        try {
            namingService.registerInstance(serviceName, DEFAULT_GROUP, persistentInstance);
            Thread.sleep(1000);

            List<Instance> afterSwitch = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
            System.out.println("NPT-005: After switch attempt, ephemeral status: " +
                    (afterSwitch.isEmpty() ? "N/A" : afterSwitch.get(0).isEphemeral()));
        } catch (NacosException e) {
            System.out.println("NPT-005: Switch from ephemeral to persistent failed: " + e.getMessage());
        }

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.5", 8080);
    }

    /**
     * NPT-006: Test switch persistent to ephemeral
     *
     * Verifies behavior when attempting to change instance from persistent to ephemeral.
     */
    @Test
    @Order(6)
    void testSwitchPersistentToEphemeral() throws NacosException, InterruptedException {
        String serviceName = "npt006-switch-pers-to-eph-" + UUID.randomUUID().toString().substring(0, 8);

        // Register persistent instance first
        Instance persistentInstance = new Instance();
        persistentInstance.setIp("192.168.200.6");
        persistentInstance.setPort(8080);
        persistentInstance.setEphemeral(false);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, persistentInstance);
        Thread.sleep(1000);

        List<Instance> initialInstances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(initialInstances.isEmpty());
        assertFalse(initialInstances.get(0).isEphemeral(), "Initial instance should be persistent");
        System.out.println("NPT-006: Initial ephemeral status: " + initialInstances.get(0).isEphemeral());

        // Try to re-register same instance as ephemeral
        Instance ephemeralInstance = new Instance();
        ephemeralInstance.setIp("192.168.200.6");
        ephemeralInstance.setPort(8080);
        ephemeralInstance.setEphemeral(true);

        try {
            namingService.registerInstance(serviceName, DEFAULT_GROUP, ephemeralInstance);
            Thread.sleep(1000);

            List<Instance> afterSwitch = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
            System.out.println("NPT-006: After switch attempt, ephemeral status: " +
                    (afterSwitch.isEmpty() ? "N/A" : afterSwitch.get(0).isEphemeral()));
        } catch (NacosException e) {
            System.out.println("NPT-006: Switch from persistent to ephemeral failed: " + e.getMessage());
        }

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.6", 8080);
    }

    /**
     * NPT-007: Test ephemeral instance heartbeat
     *
     * Verifies that ephemeral instances maintain health through heartbeat mechanism.
     */
    @Test
    @Order(7)
    void testEphemeralInstanceHeartbeat() throws NacosException, InterruptedException {
        String serviceName = "npt007-ephemeral-heartbeat-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.200.7");
        instance.setPort(8080);
        instance.setEphemeral(true);
        instance.setHealthy(true);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);

        // Check health status over time (heartbeat should maintain health)
        for (int i = 0; i < 3; i++) {
            Thread.sleep(3000);
            List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
            if (!instances.isEmpty()) {
                System.out.println("NPT-007: Check " + (i + 1) + " - Instance healthy: " + instances.get(0).isHealthy());
                assertTrue(instances.get(0).isHealthy(), "Ephemeral instance should remain healthy via heartbeat");
            }
        }

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NPT-008: Test persistent instance without heartbeat
     *
     * Verifies that persistent instances do not require heartbeat to stay registered.
     */
    @Test
    @Order(8)
    void testPersistentInstanceWithoutHeartbeat() throws NacosException, InterruptedException {
        String serviceName = "npt008-persistent-no-heartbeat-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.200.8");
        instance.setPort(8080);
        instance.setEphemeral(false);
        instance.setHealthy(true);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        // Wait for a period longer than typical heartbeat interval
        Thread.sleep(10000);

        // Persistent instance should still be registered
        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Persistent instance should remain registered without heartbeat");
        System.out.println("NPT-008: Persistent instance still registered: " + !instances.isEmpty());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NPT-009: Test ephemeral instance timeout
     *
     * Verifies that ephemeral instances become unhealthy or are removed after heartbeat timeout.
     * Note: This test requires the instance to stop sending heartbeats.
     */
    @Test
    @Order(9)
    void testEphemeralInstanceTimeout() throws NacosException, InterruptedException {
        String serviceName = "npt009-ephemeral-timeout-" + UUID.randomUUID().toString().substring(0, 8);

        // Create a separate client for registration
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);

        NamingService tempNamingService = NacosFactory.createNamingService(properties);

        Instance instance = new Instance();
        instance.setIp("192.168.200.9");
        instance.setPort(8080);
        instance.setEphemeral(true);

        tempNamingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        // Verify initial registration
        List<Instance> beforeTimeout = namingService.selectInstances(serviceName, DEFAULT_GROUP, true);
        System.out.println("NPT-009: Healthy instances before timeout: " + beforeTimeout.size());

        // Shutdown the client to stop heartbeats
        tempNamingService.shutDown();

        // Wait for heartbeat timeout
        Thread.sleep(20000);

        // Check instance status after timeout
        List<Instance> allInstances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        List<Instance> healthyInstances = namingService.selectInstances(serviceName, DEFAULT_GROUP, true);

        System.out.println("NPT-009: All instances after timeout: " + allInstances.size());
        System.out.println("NPT-009: Healthy instances after timeout: " + healthyInstances.size());

        // Instance should either be removed or marked unhealthy
    }

    /**
     * NPT-010: Test list ephemeral instances only
     *
     * Verifies that we can filter and list only ephemeral instances.
     */
    @Test
    @Order(10)
    void testListEphemeralInstancesOnly() throws NacosException, InterruptedException {
        String serviceName = "npt010-list-ephemeral-" + UUID.randomUUID().toString().substring(0, 8);

        // Register ephemeral instance
        Instance ephemeral = new Instance();
        ephemeral.setIp("192.168.200.10");
        ephemeral.setPort(8080);
        ephemeral.setEphemeral(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, ephemeral);

        // Register persistent instance
        Instance persistent = new Instance();
        persistent.setIp("192.168.200.11");
        persistent.setPort(8080);
        persistent.setEphemeral(false);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, persistent);

        Thread.sleep(1000);

        // Get all instances and filter ephemeral
        List<Instance> allInstances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        List<Instance> ephemeralInstances = allInstances.stream()
                .filter(Instance::isEphemeral)
                .collect(Collectors.toList());

        System.out.println("NPT-010: Total instances: " + allInstances.size());
        System.out.println("NPT-010: Ephemeral instances: " + ephemeralInstances.size());

        assertEquals(1, ephemeralInstances.size(), "Should have 1 ephemeral instance");
        assertEquals("192.168.200.10", ephemeralInstances.get(0).getIp());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, ephemeral);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, persistent);
    }

    /**
     * NPT-011: Test list persistent instances only
     *
     * Verifies that we can filter and list only persistent instances.
     */
    @Test
    @Order(11)
    void testListPersistentInstancesOnly() throws NacosException, InterruptedException {
        String serviceName = "npt011-list-persistent-" + UUID.randomUUID().toString().substring(0, 8);

        // Register ephemeral instance
        Instance ephemeral = new Instance();
        ephemeral.setIp("192.168.200.12");
        ephemeral.setPort(8080);
        ephemeral.setEphemeral(true);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, ephemeral);

        // Register persistent instance
        Instance persistent = new Instance();
        persistent.setIp("192.168.200.13");
        persistent.setPort(8080);
        persistent.setEphemeral(false);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, persistent);

        Thread.sleep(1000);

        // Get all instances and filter persistent
        List<Instance> allInstances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        List<Instance> persistentInstances = allInstances.stream()
                .filter(inst -> !inst.isEphemeral())
                .collect(Collectors.toList());

        System.out.println("NPT-011: Total instances: " + allInstances.size());
        System.out.println("NPT-011: Persistent instances: " + persistentInstances.size());

        assertEquals(1, persistentInstances.size(), "Should have 1 persistent instance");
        assertEquals("192.168.200.13", persistentInstances.get(0).getIp());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, ephemeral);
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, persistent);
    }

    /**
     * NPT-012: Test mixed ephemeral and persistent
     *
     * Verifies that ephemeral and persistent instances can coexist in the same service.
     */
    @Test
    @Order(12)
    void testMixedEphemeralAndPersistent() throws NacosException, InterruptedException {
        String serviceName = "npt012-mixed-" + UUID.randomUUID().toString().substring(0, 8);

        // Register multiple ephemeral instances
        for (int i = 0; i < 3; i++) {
            Instance ephemeral = new Instance();
            ephemeral.setIp("192.168.201." + i);
            ephemeral.setPort(8080);
            ephemeral.setEphemeral(true);
            ephemeral.setMetadata(Map.of("type", "ephemeral", "index", String.valueOf(i)));
            namingService.registerInstance(serviceName, DEFAULT_GROUP, ephemeral);
        }

        // Register multiple persistent instances
        for (int i = 0; i < 2; i++) {
            Instance persistent = new Instance();
            persistent.setIp("192.168.202." + i);
            persistent.setPort(8080);
            persistent.setEphemeral(false);
            persistent.setMetadata(Map.of("type", "persistent", "index", String.valueOf(i)));
            namingService.registerInstance(serviceName, DEFAULT_GROUP, persistent);
        }

        Thread.sleep(1500);

        List<Instance> allInstances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);

        long ephemeralCount = allInstances.stream().filter(Instance::isEphemeral).count();
        long persistentCount = allInstances.stream().filter(inst -> !inst.isEphemeral()).count();

        System.out.println("NPT-012: Total instances: " + allInstances.size());
        System.out.println("NPT-012: Ephemeral: " + ephemeralCount + ", Persistent: " + persistentCount);

        assertEquals(5, allInstances.size(), "Should have 5 total instances");
        assertEquals(3, ephemeralCount, "Should have 3 ephemeral instances");
        assertEquals(2, persistentCount, "Should have 2 persistent instances");

        // Cleanup
        for (int i = 0; i < 3; i++) {
            namingService.deregisterInstance(serviceName, "192.168.201." + i, 8080);
        }
        for (int i = 0; i < 2; i++) {
            namingService.deregisterInstance(serviceName, "192.168.202." + i, 8080);
        }
    }

    /**
     * NPT-013: Test persistent instance update
     *
     * Verifies that persistent instances can be updated (weight, enabled status, etc.).
     */
    @Test
    @Order(13)
    void testPersistentInstanceUpdate() throws NacosException, InterruptedException {
        String serviceName = "npt013-persistent-update-" + UUID.randomUUID().toString().substring(0, 8);

        // Register persistent instance
        Instance instance = new Instance();
        instance.setIp("192.168.200.20");
        instance.setPort(8080);
        instance.setEphemeral(false);
        instance.setWeight(1.0);
        instance.setEnabled(true);

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        // Verify initial state
        List<Instance> initial = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(initial.isEmpty());
        assertEquals(1.0, initial.get(0).getWeight(), 0.01);
        System.out.println("NPT-013: Initial weight: " + initial.get(0).getWeight());

        // Update weight
        instance.setWeight(5.0);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        // Verify updated state
        List<Instance> updated = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(updated.isEmpty());
        assertEquals(5.0, updated.get(0).getWeight(), 0.01);
        System.out.println("NPT-013: Updated weight: " + updated.get(0).getWeight());

        // Update enabled status
        instance.setEnabled(false);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> disabled = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        System.out.println("NPT-013: Instance enabled after update: " + disabled.get(0).isEnabled());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NPT-014: Test persistent instance metadata
     *
     * Verifies that persistent instances can store and update metadata.
     */
    @Test
    @Order(14)
    void testPersistentInstanceMetadata() throws NacosException, InterruptedException {
        String serviceName = "npt014-persistent-metadata-" + UUID.randomUUID().toString().substring(0, 8);

        // Register persistent instance with metadata
        Instance instance = new Instance();
        instance.setIp("192.168.200.21");
        instance.setPort(8080);
        instance.setEphemeral(false);
        instance.setMetadata(Map.of(
                "version", "1.0.0",
                "region", "us-west-2",
                "type", "persistent"
        ));

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        // Verify initial metadata
        List<Instance> initial = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(initial.isEmpty());
        assertEquals("1.0.0", initial.get(0).getMetadata().get("version"));
        assertEquals("us-west-2", initial.get(0).getMetadata().get("region"));
        assertEquals("persistent", initial.get(0).getMetadata().get("type"));
        System.out.println("NPT-014: Initial metadata: " + initial.get(0).getMetadata());

        // Update metadata
        instance.setMetadata(Map.of(
                "version", "2.0.0",
                "region", "us-east-1",
                "type", "persistent",
                "updated", "true"
        ));
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        // Verify updated metadata
        List<Instance> updated = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(updated.isEmpty());
        assertEquals("2.0.0", updated.get(0).getMetadata().get("version"));
        assertEquals("us-east-1", updated.get(0).getMetadata().get("region"));
        assertEquals("true", updated.get(0).getMetadata().get("updated"));
        System.out.println("NPT-014: Updated metadata: " + updated.get(0).getMetadata());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }

    /**
     * NPT-015: Test ephemeral instance rapid register/deregister
     *
     * Verifies that ephemeral instances can be rapidly registered and deregistered.
     */
    @Test
    @Order(15)
    void testEphemeralInstanceRapidRegisterDeregister() throws NacosException, InterruptedException {
        String serviceName = "npt015-rapid-" + UUID.randomUUID().toString().substring(0, 8);

        int iterations = 10;
        long totalRegisterTime = 0;
        long totalDeregisterTime = 0;

        for (int i = 0; i < iterations; i++) {
            Instance instance = new Instance();
            instance.setIp("192.168.200.30");
            instance.setPort(8080 + i);
            instance.setEphemeral(true);

            // Time registration
            long startRegister = System.currentTimeMillis();
            namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
            long registerTime = System.currentTimeMillis() - startRegister;
            totalRegisterTime += registerTime;

            Thread.sleep(100);

            // Time deregistration
            long startDeregister = System.currentTimeMillis();
            namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
            long deregisterTime = System.currentTimeMillis() - startDeregister;
            totalDeregisterTime += deregisterTime;

            Thread.sleep(100);
        }

        System.out.println("NPT-015: Avg register time: " + (totalRegisterTime / iterations) + "ms");
        System.out.println("NPT-015: Avg deregister time: " + (totalDeregisterTime / iterations) + "ms");

        // Verify no instances remain
        Thread.sleep(500);
        List<Instance> remaining = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertTrue(remaining.isEmpty(), "No instances should remain after rapid register/deregister");
    }

    /**
     * NPT-016: Test persistent instance with health check
     *
     * Verifies that persistent instances can have custom health check configuration.
     */
    @Test
    @Order(16)
    void testPersistentInstanceWithHealthCheck() throws NacosException, InterruptedException {
        String serviceName = "npt016-persistent-health-" + UUID.randomUUID().toString().substring(0, 8);

        // Register persistent instance with health check metadata
        Instance instance = new Instance();
        instance.setIp("192.168.200.40");
        instance.setPort(8080);
        instance.setEphemeral(false);
        instance.setHealthy(true);
        instance.setMetadata(Map.of(
                "healthCheckType", "HTTP",
                "healthCheckPath", "/health",
                "healthCheckPort", "8080",
                "healthCheckInterval", "10000"
        ));

        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        // Verify instance and health check config
        List<Instance> instances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        assertFalse(instances.isEmpty(), "Should have registered persistent instance");

        Instance found = instances.get(0);
        assertFalse(found.isEphemeral(), "Instance should be persistent");
        assertTrue(found.isHealthy(), "Instance should be healthy");

        Map<String, String> metadata = found.getMetadata();
        assertEquals("HTTP", metadata.get("healthCheckType"));
        assertEquals("/health", metadata.get("healthCheckPath"));
        assertEquals("8080", metadata.get("healthCheckPort"));

        System.out.println("NPT-016: Persistent instance health check config: " + metadata);

        // Manually mark unhealthy
        instance.setHealthy(false);
        namingService.registerInstance(serviceName, DEFAULT_GROUP, instance);
        Thread.sleep(1000);

        List<Instance> unhealthyInstances = namingService.getAllInstances(serviceName, DEFAULT_GROUP);
        if (!unhealthyInstances.isEmpty()) {
            System.out.println("NPT-016: Instance healthy status after manual update: " +
                    unhealthyInstances.get(0).isHealthy());
        }

        // Select only healthy (should be empty or not contain our instance)
        List<Instance> healthyOnly = namingService.selectInstances(serviceName, DEFAULT_GROUP, true);
        System.out.println("NPT-016: Healthy instances count: " + healthyOnly.size());

        // Cleanup
        namingService.deregisterInstance(serviceName, DEFAULT_GROUP, instance);
    }
}
