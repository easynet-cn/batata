package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import org.junit.jupiter.api.*;

import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Cluster Mode SDK Tests
 *
 * Tests SDK compatibility against a 3-node batata cluster.
 * Verifies cross-node data replication (Raft for config, Distro for naming).
 *
 * Prerequisites:
 *   - 3-node cluster running (use ./scripts/start-cluster.sh)
 *   - Node 1: 127.0.0.1:8848, Node 2: 127.0.0.1:8858, Node 3: 127.0.0.1:8868
 *   - Admin user initialized (nacos/nacos)
 *
 * Run with:
 *   mvn test -Dtest=NacosClusterTest \
 *       -Dnacos.cluster.node1=127.0.0.1:8848 \
 *       -Dnacos.cluster.node2=127.0.0.1:8858 \
 *       -Dnacos.cluster.node3=127.0.0.1:8868
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosClusterTest {

    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final String USERNAME = System.getProperty("nacos.username", "nacos");
    private static final String PASSWORD = System.getProperty("nacos.password", "nacos");

    private static String node1Addr;
    private static String node2Addr;
    private static String node3Addr;

    private static ConfigService configNode1;
    private static ConfigService configNode2;
    private static ConfigService configNode3;
    private static NamingService namingNode1;
    private static NamingService namingNode2;
    private static NamingService namingNode3;

    @BeforeAll
    static void setup() throws NacosException {
        node1Addr = System.getProperty("nacos.cluster.node1", "127.0.0.1:8848");
        node2Addr = System.getProperty("nacos.cluster.node2", "127.0.0.1:8858");
        node3Addr = System.getProperty("nacos.cluster.node3", "127.0.0.1:8868");

        // Skip unless explicitly configured via system properties
        // (prevents accidental execution against single-node deployment)
        String explicitNode2 = System.getProperty("nacos.cluster.node2");
        String explicitNode3 = System.getProperty("nacos.cluster.node3");
        if (explicitNode2 == null || explicitNode3 == null) {
            Assumptions.assumeTrue(false,
                    "Cluster tests require -Dnacos.cluster.node2 and -Dnacos.cluster.node3");
        }

        try {
            configNode1 = createConfigService(node1Addr);
            configNode2 = createConfigService(node2Addr);
            configNode3 = createConfigService(node3Addr);

            // Verify all 3 nodes are actually reachable by doing a real request
            configNode1.getConfig("__cluster_probe__", DEFAULT_GROUP, 3000);
            configNode2.getConfig("__cluster_probe__", DEFAULT_GROUP, 3000);
            configNode3.getConfig("__cluster_probe__", DEFAULT_GROUP, 3000);

            namingNode1 = createNamingService(node1Addr);
            namingNode2 = createNamingService(node2Addr);
            namingNode3 = createNamingService(node3Addr);
        } catch (Exception e) {
            Assumptions.assumeTrue(false,
                    "Cluster not available, skipping cluster tests: " + e.getMessage());
        }
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (configNode1 != null) configNode1.shutDown();
        if (configNode2 != null) configNode2.shutDown();
        if (configNode3 != null) configNode3.shutDown();
        if (namingNode1 != null) namingNode1.shutDown();
        if (namingNode2 != null) namingNode2.shutDown();
        if (namingNode3 != null) namingNode3.shutDown();
    }

    private static ConfigService createConfigService(String serverAddr) throws NacosException {
        Properties props = new Properties();
        props.setProperty("serverAddr", serverAddr);
        props.setProperty("username", USERNAME);
        props.setProperty("password", PASSWORD);
        return NacosFactory.createConfigService(props);
    }

    private static NamingService createNamingService(String serverAddr) throws NacosException {
        Properties props = new Properties();
        props.setProperty("serverAddr", serverAddr);
        props.setProperty("username", USERNAME);
        props.setProperty("password", PASSWORD);
        return NacosFactory.createNamingService(props);
    }

    // ==================== Config Raft Replication Tests ====================

    /**
     * CLU-001: Write config on Node 1, read from Node 2 and Node 3
     */
    @Test
    @Order(1)
    void testConfigWriteNode1ReadOtherNodes() throws Exception {
        String dataId = "cluster-cfg-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "config.from.node1=true\ncluster.test=replication";

        // Write on Node 1
        boolean published = configNode1.publishConfig(dataId, DEFAULT_GROUP, content);
        assertTrue(published, "Node 1 should publish config successfully");

        // Wait for Raft replication
        Thread.sleep(3000);

        // Read from Node 2
        String contentNode2 = configNode2.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(contentNode2, "Node 2 should read replicated config");
        assertEquals(content, contentNode2, "Node 2 content should match Node 1");

        // Read from Node 3
        String contentNode3 = configNode3.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(contentNode3, "Node 3 should read replicated config");
        assertEquals(content, contentNode3, "Node 3 content should match Node 1");

        // Cleanup
        configNode1.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CLU-002: Write config on follower node, verify replication
     */
    @Test
    @Order(2)
    void testConfigWriteFromFollower() throws Exception {
        String dataId = "cluster-follower-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "written.by.follower=true";

        // Write on Node 3 (likely a follower)
        boolean published = configNode3.publishConfig(dataId, DEFAULT_GROUP, content);
        assertTrue(published, "Follower should publish config (forwarded to leader)");

        Thread.sleep(3000);

        // Read from Node 1
        String contentNode1 = configNode1.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(contentNode1, "Node 1 should see follower-written config");
        assertEquals(content, contentNode1, "Content should match across nodes");

        // Read from Node 2
        String contentNode2 = configNode2.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(contentNode2, "Node 2 should see follower-written config");
        assertEquals(content, contentNode2, "Content should match across nodes");

        // Cleanup
        configNode1.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CLU-003: Update config on one node, verify update propagated
     */
    @Test
    @Order(3)
    void testConfigUpdateReplication() throws Exception {
        String dataId = "cluster-update-" + UUID.randomUUID().toString().substring(0, 8);

        // Create initial config on Node 1
        configNode1.publishConfig(dataId, DEFAULT_GROUP, "version=1");
        Thread.sleep(2000);

        // Update on Node 2
        configNode2.publishConfig(dataId, DEFAULT_GROUP, "version=2");
        Thread.sleep(3000);

        // Verify update on Node 3
        String content = configNode3.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(content, "Node 3 should have updated config");
        assertEquals("version=2", content, "Node 3 should see the updated version");

        // Cleanup
        configNode1.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * CLU-004: Delete config on one node, verify deletion across cluster
     */
    @Test
    @Order(4)
    void testConfigDeleteReplication() throws Exception {
        String dataId = "cluster-delete-" + UUID.randomUUID().toString().substring(0, 8);

        // Create config
        configNode1.publishConfig(dataId, DEFAULT_GROUP, "to-be-deleted");
        Thread.sleep(2000);

        // Verify exists on Node 2
        String before = configNode2.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(before, "Config should exist on Node 2 before deletion");

        // Delete on Node 3
        boolean removed = configNode3.removeConfig(dataId, DEFAULT_GROUP);
        assertTrue(removed, "Node 3 should delete config successfully");

        Thread.sleep(3000);

        // Verify gone on Node 1
        String after = configNode1.getConfig(dataId, DEFAULT_GROUP, 3000);
        assertNull(after, "Config should be deleted on Node 1 after cluster deletion");
    }

    // ==================== Naming Distro Sync Tests ====================

    /**
     * CLU-005: Register instance on Node 1, query from Node 2 and Node 3
     */
    @Test
    @Order(5)
    void testInstanceRegisterNode1QueryOtherNodes() throws Exception {
        String serviceName = "cluster-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Register on Node 1
        Instance inst = new Instance();
        inst.setIp("10.0.1.1");
        inst.setPort(8080);
        inst.setHealthy(true);
        inst.setWeight(1.0);
        namingNode1.registerInstance(serviceName, inst);

        // Wait for Distro sync
        Thread.sleep(3000);

        // Query from Node 2
        List<Instance> node2Instances = namingNode2.getAllInstances(serviceName);
        assertNotNull(node2Instances, "Node 2 should return instances");
        assertTrue(node2Instances.size() >= 1,
                "Node 2 should see at least 1 instance, got: " + node2Instances.size());
        assertEquals("10.0.1.1", node2Instances.get(0).getIp(),
                "Node 2 should see the correct instance IP");

        // Query from Node 3
        List<Instance> node3Instances = namingNode3.getAllInstances(serviceName);
        assertNotNull(node3Instances, "Node 3 should return instances");
        assertTrue(node3Instances.size() >= 1,
                "Node 3 should see at least 1 instance, got: " + node3Instances.size());

        // Cleanup
        namingNode1.deregisterInstance(serviceName, "10.0.1.1", 8080);
    }

    /**
     * CLU-006: Register instances on different nodes, verify all visible everywhere
     */
    @Test
    @Order(6)
    void testInstanceRegisterOnDifferentNodes() throws Exception {
        String serviceName = "cluster-multi-" + UUID.randomUUID().toString().substring(0, 8);

        // Register on each node
        namingNode1.registerInstance(serviceName, "10.0.2.1", 8080);
        namingNode2.registerInstance(serviceName, "10.0.2.2", 8080);
        namingNode3.registerInstance(serviceName, "10.0.2.3", 8080);

        // Wait for Distro sync
        Thread.sleep(5000);

        // All nodes should see all 3 instances
        for (int nodeIdx = 1; nodeIdx <= 3; nodeIdx++) {
            NamingService ns = nodeIdx == 1 ? namingNode1 : (nodeIdx == 2 ? namingNode2 : namingNode3);
            List<Instance> instances = ns.getAllInstances(serviceName);
            assertNotNull(instances, "Node " + nodeIdx + " should return instances");

            Set<String> ips = new HashSet<>();
            for (Instance inst : instances) {
                ips.add(inst.getIp());
            }
            assertTrue(ips.contains("10.0.2.1"),
                    "Node " + nodeIdx + " should see instance from Node 1");
            assertTrue(ips.contains("10.0.2.2"),
                    "Node " + nodeIdx + " should see instance from Node 2");
            assertTrue(ips.contains("10.0.2.3"),
                    "Node " + nodeIdx + " should see instance from Node 3");
        }

        // Cleanup
        namingNode1.deregisterInstance(serviceName, "10.0.2.1", 8080);
        namingNode2.deregisterInstance(serviceName, "10.0.2.2", 8080);
        namingNode3.deregisterInstance(serviceName, "10.0.2.3", 8080);
    }

    /**
     * CLU-007: Deregister instance on one node, verify removal across cluster
     */
    @Test
    @Order(7)
    void testInstanceDeregisterReplication() throws Exception {
        String serviceName = "cluster-dereg-" + UUID.randomUUID().toString().substring(0, 8);

        // Register on Node 1
        namingNode1.registerInstance(serviceName, "10.0.3.1", 8080);
        Thread.sleep(3000);

        // Verify visible on Node 3
        List<Instance> before = namingNode3.getAllInstances(serviceName);
        assertTrue(before.size() >= 1, "Node 3 should see instance before deregister");

        // Deregister on Node 1
        namingNode1.deregisterInstance(serviceName, "10.0.3.1", 8080);
        Thread.sleep(5000);

        // Verify gone on Node 3 using non-subscribe mode to bypass SDK cache
        List<Instance> after = namingNode3.selectInstances(serviceName,
                new java.util.ArrayList<>(), true, false);
        assertEquals(0, after.size(),
                "Node 3 should see 0 instances after deregister via distro sync, got: " + after.size());
    }

    /**
     * CLU-008: Config and naming work concurrently across cluster
     */
    @Test
    @Order(8)
    void testConcurrentConfigAndNaming() throws Exception {
        String dataId = "cluster-concurrent-" + UUID.randomUUID().toString().substring(0, 8);
        String serviceName = "cluster-concurrent-" + UUID.randomUUID().toString().substring(0, 8);

        // Write config on Node 1 and register instance on Node 3 concurrently
        configNode1.publishConfig(dataId, DEFAULT_GROUP, "concurrent=true");
        namingNode3.registerInstance(serviceName, "10.0.4.1", 8080);

        Thread.sleep(5000);

        // Node 2 should see both
        String config = configNode2.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(config, "Node 2 should see config written on Node 1");
        assertEquals("concurrent=true", config);

        List<Instance> instances = namingNode2.getAllInstances(serviceName);
        assertTrue(instances.size() >= 1,
                "Node 2 should see instance registered on Node 3");

        // Cleanup
        configNode1.removeConfig(dataId, DEFAULT_GROUP);
        namingNode3.deregisterInstance(serviceName, "10.0.4.1", 8080);
    }
}
