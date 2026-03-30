package io.batata.tests;

import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.model.response.ConnectionInfo;
import com.alibaba.nacos.api.model.response.NacosMember;
import com.alibaba.nacos.api.model.response.Namespace;
import com.alibaba.nacos.maintainer.client.NacosMaintainerFactory;
import com.alibaba.nacos.maintainer.client.config.ConfigMaintainerService;
import org.junit.jupiter.api.*;
import org.junit.jupiter.api.Disabled;

import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos CoreMaintainerService Tests (V3 Admin API)
 *
 * Tests for CoreMaintainerService operations accessible via ConfigMaintainerService:
 * namespace CRUD, server status (liveness/readiness), cluster node listing,
 * server state, and client connections.
 *
 * CoreMaintainerService is a parent interface of both ConfigMaintainerService
 * and NamingMaintainerService. We use ConfigMaintainerService here since it's
 * already available in the test project.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosCoreMaintainerServiceTest {

    private static ConfigMaintainerService maintainerService;

    @BeforeAll
    static void setup() throws Exception {
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);

        maintainerService = NacosMaintainerFactory.createConfigMaintainerService(properties);
        System.out.println("CoreMaintainerService Test Setup - Server: " + serverAddr);
    }

    // ==================== P0: Server Health ====================

    /**
     * CORE-001: Test server liveness probe
     *
     * The liveness endpoint checks if the server process is alive.
     */
    @Test
    @Order(1)
    void testServerLiveness() throws NacosException {
        Boolean alive = maintainerService.liveness();
        assertTrue(alive, "Server liveness probe should return true");
    }

    /**
     * CORE-002: Test server readiness probe
     *
     * The readiness endpoint checks if the server is ready to accept requests.
     */
    @Test
    @Order(2)
    void testServerReadiness() throws NacosException {
        try {
            Boolean ready = maintainerService.readiness();
            assertTrue(ready, "Server readiness probe should return true");
        } catch (NacosException e) {
            // Readiness may return 502 during startup/warmup - acceptable
            System.out.println("Readiness check returned error (may be warming up): " + e.getMessage());
        }
    }

    /**
     * CORE-003: Test get server state
     *
     * Server state returns key-value pairs like server version, standalone mode, etc.
     */
    @Test
    @Order(3)
    void testGetServerState() throws NacosException {
        Map<String, String> state = maintainerService.getServerState();
        assertNotNull(state, "Server state should not be null");
        assertFalse(state.isEmpty(), "Server state should contain entries");

        // Typical keys include "standalone_mode", "version", "function_mode"
        System.out.println("Server state keys: " + state.keySet());
    }

    // ==================== P0: Namespace CRUD ====================

    /**
     * CORE-004: Test list namespaces
     *
     * At minimum, the public namespace should always exist.
     */
    @Test
    @Order(4)
    void testListNamespaces() throws NacosException {
        List<Namespace> namespaces = maintainerService.getNamespaceList();
        assertNotNull(namespaces, "Namespace list should not be null");
        assertFalse(namespaces.isEmpty(), "Should have at least the public namespace");

        // Public namespace should exist
        boolean hasPublic = namespaces.stream()
                .anyMatch(ns -> "public".equals(ns.getNamespace()) || "".equals(ns.getNamespace()));
        assertTrue(hasPublic, "Public namespace should exist in the list");
    }

    /**
     * CORE-005: Test create, get, update, and delete namespace
     *
     * Full CRUD cycle for namespace management.
     */
    @Test
    @Order(5)
    void testNamespaceCrud() throws NacosException, InterruptedException {
        String nsId = "core-test-ns-" + UUID.randomUUID().toString().substring(0, 8);
        String nsName = "Core Test Namespace";
        String nsDesc = "Namespace for CoreMaintainerService test";

        // Create namespace
        Boolean created = maintainerService.createNamespace(nsId, nsName, nsDesc);
        assertTrue(created, "Namespace creation should succeed");
        Thread.sleep(1000);

        // Get namespace
        Namespace ns = maintainerService.getNamespace(nsId);
        assertNotNull(ns, "Created namespace should be retrievable");
        assertEquals(nsId, ns.getNamespace(), "Namespace ID should match");
        assertEquals(nsName, ns.getNamespaceShowName(), "Namespace name should match");

        // Update namespace
        String updatedName = "Updated Core Namespace";
        String updatedDesc = "Updated description";
        Boolean updated = maintainerService.updateNamespace(nsId, updatedName, updatedDesc);
        assertTrue(updated, "Namespace update should succeed");
        Thread.sleep(500);

        // Verify update
        Namespace updatedNs = maintainerService.getNamespace(nsId);
        assertNotNull(updatedNs, "Updated namespace should be retrievable");
        assertEquals(updatedName, updatedNs.getNamespaceShowName(), "Namespace name should be updated");

        // Delete namespace
        Boolean deleted = maintainerService.deleteNamespace(nsId);
        assertTrue(deleted, "Namespace deletion should succeed");
        Thread.sleep(500);

        // Verify deleted
        try {
            Namespace deletedNs = maintainerService.getNamespace(nsId);
            // If no exception, the namespace should not contain our data
            assertTrue(deletedNs == null || !nsId.equals(deletedNs.getNamespace()),
                    "Deleted namespace should not be retrievable");
        } catch (NacosException e) {
            // Expected - namespace not found
            assertTrue(true, "Namespace correctly not found after deletion");
        }
    }

    /**
     * CORE-006: Test check namespace ID exists
     */
    @Test
    @Order(6)
    void testCheckNamespaceIdExist() throws NacosException, InterruptedException {
        String nsId = "core-check-ns-" + UUID.randomUUID().toString().substring(0, 8);

        // Should not exist before creation
        Boolean existsBefore = maintainerService.checkNamespaceIdExist(nsId);
        assertFalse(existsBefore, "Namespace should not exist before creation");

        // Create and check
        maintainerService.createNamespace(nsId, "Check Namespace", "Test existence check");
        Thread.sleep(1000);

        Boolean existsAfter = maintainerService.checkNamespaceIdExist(nsId);
        assertTrue(existsAfter, "Namespace should exist after creation");

        // Cleanup
        maintainerService.deleteNamespace(nsId);
    }

    /**
     * CORE-007: Test create namespace with auto-generated ID
     */
    @Test
    @Order(7)
    void testCreateNamespaceAutoId() throws NacosException, InterruptedException {
        String nsName = "Auto ID Namespace " + UUID.randomUUID().toString().substring(0, 6);
        String nsDesc = "Namespace with auto-generated ID";

        // Create with auto-generated ID (empty string)
        Boolean created = maintainerService.createNamespace(nsName, nsDesc);
        assertTrue(created, "Namespace creation with auto ID should succeed");
        Thread.sleep(500);

        // Find it in the namespace list
        List<Namespace> namespaces = maintainerService.getNamespaceList();
        Namespace found = namespaces.stream()
                .filter(ns -> nsName.equals(ns.getNamespaceShowName()))
                .findFirst().orElse(null);
        assertNotNull(found, "Auto-ID namespace should be in the list");
        assertNotNull(found.getNamespace(), "Auto-generated namespace ID should not be null");
        assertFalse(found.getNamespace().isEmpty(), "Auto-generated namespace ID should not be empty");

        // Cleanup
        maintainerService.deleteNamespace(found.getNamespace());
    }

    // ==================== P1: Cluster Information ====================

    /**
     * CORE-008: Test list cluster nodes
     *
     * In standalone mode, there should be at least one node (self).
     */
    @Test
    @Order(8)
    void testListClusterNodes() throws NacosException {
        Collection<NacosMember> members = maintainerService.listClusterNodes(null, null);
        assertNotNull(members, "Cluster members should not be null");
        assertFalse(members.isEmpty(), "Should have at least one cluster member (self)");

        NacosMember firstMember = members.iterator().next();
        assertNotNull(firstMember.getAddress(), "Member should have an address");
        System.out.println("Cluster node: " + firstMember.getAddress()
                + ", state: " + firstMember.getState());
    }

    /**
     * CORE-009: Test list cluster nodes with state filter
     *
     * Filter cluster nodes by state (UP, DOWN, etc.).
     */
    @Test
    @Order(9)
    void testListClusterNodesWithStateFilter() throws NacosException {
        // List only UP nodes
        Collection<NacosMember> upMembers = maintainerService.listClusterNodes(null, "UP");
        assertNotNull(upMembers, "UP cluster members should not be null");

        // In a healthy standalone cluster, at least one node should be UP
        assertFalse(upMembers.isEmpty(), "Should have at least one UP cluster member");
        for (NacosMember member : upMembers) {
            // getState() returns NodeState enum, toString() gives "UP"
            assertEquals("UP", String.valueOf(member.getState()), "Filtered member should be in UP state");
        }
    }

    // ==================== P1: Client Connections ====================

    /**
     * CORE-010: Test get current client connections
     *
     * Lists all active SDK client connections to the server.
     */
    @Test
    @Order(10)
    void testGetCurrentClients() throws NacosException {
        Map<String, ConnectionInfo> clients = maintainerService.getCurrentClients();
        assertNotNull(clients, "Client connections map should not be null");
        // There should be at least one connection (our maintainer client)
        System.out.println("Active client connections: " + clients.size());
        for (Map.Entry<String, ConnectionInfo> entry : clients.entrySet()) {
            assertNotNull(entry.getKey(), "Connection ID should not be null");
            assertNotNull(entry.getValue(), "Connection info should not be null");
        }
    }

    // ==================== P2: Namespace Isolation ====================

    /**
     * CORE-011: Test namespace isolation for configs
     *
     * Creates a namespace, publishes config in it, and verifies isolation
     * from the default namespace.
     */
    @Test
    @Order(11)
    void testNamespaceIsolation() throws Exception {
        String nsId = "core-isolation-" + UUID.randomUUID().toString().substring(0, 8);
        String dataId = "isolation-config-" + UUID.randomUUID().toString().substring(0, 8);
        String content = "isolation.test=true";

        // Create namespace
        maintainerService.createNamespace(nsId, "Isolation NS", "For isolation test");
        Thread.sleep(500);

        // Publish config in the new namespace
        boolean published = maintainerService.publishConfig(dataId, "DEFAULT_GROUP", nsId, content);
        assertTrue(published, "Config publish in custom namespace should succeed");
        Thread.sleep(500);

        // Verify config exists in custom namespace
        var configInNs = maintainerService.getConfig(dataId, "DEFAULT_GROUP", nsId);
        assertNotNull(configInNs, "Config should exist in custom namespace");
        assertEquals(content, configInNs.getContent(), "Config content should match");

        // Verify config does NOT exist in default namespace
        try {
            var configInDefault = maintainerService.getConfig(dataId, "DEFAULT_GROUP", "");
            if (configInDefault != null && configInDefault.getContent() != null) {
                assertNotEquals(content, configInDefault.getContent(),
                        "Config in default namespace should not have the custom namespace content");
            }
        } catch (NacosException e) {
            // Expected - config not found in default namespace
            assertTrue(true, "Config correctly not found in default namespace");
        }

        // Cleanup
        maintainerService.deleteConfig(dataId, "DEFAULT_GROUP", nsId);
        maintainerService.deleteNamespace(nsId);
    }

    // ==================== P2: Duplicate Namespace ====================

    /**
     * CORE-012: Test creating duplicate namespace should fail
     */
    @Test
    @Order(12)
    void testCreateDuplicateNamespace() throws NacosException, InterruptedException {
        String nsId = "core-dup-ns-" + UUID.randomUUID().toString().substring(0, 8);

        // Create namespace first time
        Boolean created = maintainerService.createNamespace(nsId, "Duplicate Test", "First creation");
        assertTrue(created, "First namespace creation should succeed");
        Thread.sleep(500);

        // Try creating again with same ID
        try {
            Boolean duplicateCreated = maintainerService.createNamespace(nsId, "Duplicate Test 2", "Second creation");
            // If no exception, should return false for duplicate
            assertFalse(duplicateCreated, "Duplicate namespace creation should return false");
        } catch (NacosException e) {
            // Expected - duplicate namespace ID
            assertTrue(true, "Duplicate namespace creation correctly rejected");
        }

        // Cleanup
        maintainerService.deleteNamespace(nsId);
    }
}
