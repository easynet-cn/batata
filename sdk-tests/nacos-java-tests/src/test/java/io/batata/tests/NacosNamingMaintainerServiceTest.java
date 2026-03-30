package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.model.Page;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import com.alibaba.nacos.api.naming.pojo.maintainer.ServiceDetailInfo;
import com.alibaba.nacos.api.naming.pojo.maintainer.ServiceView;
import com.alibaba.nacos.maintainer.client.naming.NamingMaintainerFactory;
import com.alibaba.nacos.maintainer.client.naming.NamingMaintainerService;
import org.junit.jupiter.api.*;
import org.junit.jupiter.api.Disabled;

import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Extended NamingMaintainerService Tests (V3 Admin API)
 *
 * Tests for advanced NamingMaintainerService operations: service listing with pagination,
 * instance batch metadata, subscribers, service detail with instances, and multi-namespace.
 * Aligned with Nacos MaintainServiceNamingITCase and SubscribeNamingITCase.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosNamingMaintainerServiceTest {

    private static NamingMaintainerService maintainerService;
    private static NamingService namingService;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static final String DEFAULT_NAMESPACE = "";

    @BeforeAll
    static void setup() throws Exception {
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);

        maintainerService = NamingMaintainerFactory.createNamingMaintainerService(properties);
        namingService = NacosFactory.createNamingService(properties);
        System.out.println("NamingMaintainerService Extended Tests - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (maintainerService != null) maintainerService.shutdown();
        if (namingService != null) namingService.shutDown();
    }

    // ==================== P1: Service Listing ====================

    /**
     * NMMS-001: Test list services with pagination
     *
     * Creates multiple services and verifies they appear in the paginated list.
     */
    @Test
    @Order(1)
    void testListServicesWithPagination() throws Exception {
        String prefix = "nmms-list-" + UUID.randomUUID().toString().substring(0, 6);
        int serviceCount = 5;
        List<String> serviceNames = new ArrayList<>();

        // Create multiple services
        for (int i = 0; i < serviceCount; i++) {
            String serviceName = prefix + "-svc-" + i;
            serviceNames.add(serviceName);
            maintainerService.createService(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName, true, 0.5f);
        }
        Thread.sleep(1500);

        // List services with pagination
        Page<ServiceView> page = maintainerService.listServices(DEFAULT_NAMESPACE, DEFAULT_GROUP, prefix,
                false, 1, 10);
        assertNotNull(page, "Service list page should not be null");
        assertNotNull(page.getPageItems(), "Page items should not be null");
        assertTrue(page.getPageItems().size() >= serviceCount,
                "Should have at least " + serviceCount + " services, got: " + page.getPageItems().size());

        // Verify our services are in the list
        List<String> foundNames = new ArrayList<>();
        for (ServiceView view : page.getPageItems()) {
            foundNames.add(view.getName());
        }
        for (String name : serviceNames) {
            assertTrue(foundNames.contains(name),
                    "Service '" + name + "' should be in the service list");
        }

        // Cleanup
        for (String name : serviceNames) {
            try {
                maintainerService.removeService(DEFAULT_NAMESPACE, DEFAULT_GROUP, name);
            } catch (Exception ignored) {
            }
        }
    }

    /**
     * NMMS-002: Test list services with detail
     *
     * Creates services and verifies detail information in the list response.
     */
    @Test
    @Order(2)
    void testListServicesWithDetail() throws Exception {
        String serviceName = "nmms-detail-list-" + UUID.randomUUID().toString().substring(0, 8);

        maintainerService.createService(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName, true, 0.7f);
        Thread.sleep(1000);

        // Register an instance to have non-empty service
        namingService.registerInstance(serviceName, "10.0.100.1", 8080);
        Thread.sleep(1500);

        // List services with detail
        Page<ServiceDetailInfo> detailPage = maintainerService.listServicesWithDetail(
                DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName, 1, 10);
        assertNotNull(detailPage, "Detail page should not be null");
        assertNotNull(detailPage.getPageItems(), "Detail page items should not be null");
        assertFalse(detailPage.getPageItems().isEmpty(), "Detail page should have at least 1 service");

        ServiceDetailInfo detail = detailPage.getPageItems().stream()
                .filter(d -> serviceName.equals(d.getServiceName()))
                .findFirst().orElse(null);
        assertNotNull(detail, "Our service should be in the detail list");
        assertEquals(0.7f, detail.getProtectThreshold(), 0.01, "ProtectThreshold should match");

        // Cleanup
        namingService.deregisterInstance(serviceName, "10.0.100.1", 8080);
        Thread.sleep(500);
        try {
            maintainerService.removeService(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName);
        } catch (Exception ignored) {
        }
    }

    // ==================== P1: Instance Batch Operations ====================

    /**
     * NMMS-003: Test batch update instance metadata
     *
     * Registers multiple instances, batch updates their metadata,
     * and verifies all were updated.
     */
    @Test
    @Order(3)
    void testBatchUpdateInstanceMetadata() throws Exception {
        String serviceName = "nmms-batch-meta-" + UUID.randomUUID().toString().substring(0, 8);

        // Register multiple instances
        for (int i = 1; i <= 3; i++) {
            Instance inst = new Instance();
            inst.setIp("10.0.200." + i);
            inst.setPort(8080);
            inst.setWeight(1.0);
            inst.setEphemeral(true);
            Map<String, String> meta = new HashMap<>();
            meta.put("version", "1.0");
            inst.setMetadata(meta);
            namingService.registerInstance(serviceName, inst);
        }
        Thread.sleep(2000);

        // Build service and instance list for batch update
        com.alibaba.nacos.api.naming.pojo.Service service = new com.alibaba.nacos.api.naming.pojo.Service();
        service.setName(serviceName);
        service.setGroupName(DEFAULT_GROUP);
        service.setNamespaceId(DEFAULT_NAMESPACE);

        List<Instance> instList = new ArrayList<>();
        for (int i = 1; i <= 3; i++) {
            Instance inst = new Instance();
            inst.setIp("10.0.200." + i);
            inst.setPort(8080);
            inst.setEphemeral(true);
            instList.add(inst);
        }

        Map<String, String> newMeta = new HashMap<>();
        newMeta.put("version", "2.0");
        newMeta.put("batch-updated", "true");

        maintainerService.batchUpdateInstanceMetadata(service, instList, newMeta);
        Thread.sleep(2000);

        // Verify all instances have updated metadata
        List<Instance> allInstances = namingService.getAllInstances(serviceName);
        assertTrue(allInstances.size() >= 3, "Should have at least 3 instances");

        for (Instance inst : allInstances) {
            assertEquals("2.0", inst.getMetadata().get("version"),
                    "Instance " + inst.getIp() + " version should be updated to 2.0");
            assertEquals("true", inst.getMetadata().get("batch-updated"),
                    "Instance " + inst.getIp() + " should have batch-updated metadata");
        }

        // Cleanup
        for (int i = 1; i <= 3; i++) {
            namingService.deregisterInstance(serviceName, "10.0.200." + i, 8080);
        }
    }

    /**
     * NMMS-004: Test batch delete instance metadata
     *
     * Registers instances with metadata, then batch deletes specific metadata keys.
     */
    @Test
    @Order(4)
    void testBatchDeleteInstanceMetadata() throws Exception {
        String serviceName = "nmms-batch-del-meta-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instances with metadata
        for (int i = 1; i <= 2; i++) {
            Instance inst = new Instance();
            inst.setIp("10.0.201." + i);
            inst.setPort(8080);
            inst.setWeight(1.0);
            inst.setEphemeral(true);
            Map<String, String> meta = new HashMap<>();
            meta.put("version", "1.0");
            meta.put("env", "test");
            meta.put("temp-key", "to-be-removed");
            inst.setMetadata(meta);
            namingService.registerInstance(serviceName, inst);
        }
        Thread.sleep(2000);

        // Batch delete the "temp-key" metadata
        com.alibaba.nacos.api.naming.pojo.Service service = new com.alibaba.nacos.api.naming.pojo.Service();
        service.setName(serviceName);
        service.setGroupName(DEFAULT_GROUP);
        service.setNamespaceId(DEFAULT_NAMESPACE);

        List<Instance> instList = new ArrayList<>();
        for (int i = 1; i <= 2; i++) {
            Instance inst = new Instance();
            inst.setIp("10.0.201." + i);
            inst.setPort(8080);
            inst.setEphemeral(true);
            instList.add(inst);
        }

        Map<String, String> keysToDelete = new HashMap<>();
        keysToDelete.put("temp-key", "to-be-removed");

        maintainerService.batchDeleteInstanceMetadata(service, instList, keysToDelete);
        Thread.sleep(2000);

        // Verify temp-key was removed but other metadata remains
        List<Instance> allInstances = namingService.getAllInstances(serviceName);
        for (Instance inst : allInstances) {
            assertNull(inst.getMetadata().get("temp-key"),
                    "Instance " + inst.getIp() + " should not have temp-key after batch delete");
            assertNotNull(inst.getMetadata().get("version"),
                    "Instance " + inst.getIp() + " should still have version metadata");
        }

        // Cleanup
        for (int i = 1; i <= 2; i++) {
            namingService.deregisterInstance(serviceName, "10.0.201." + i, 8080);
        }
    }

    // ==================== P1: Subscribers ====================

    /**
     * NMMS-005: Test get subscribers for a service
     *
     * Subscribes to a service via SDK, then queries subscribers via MaintainerService.
     */
    @Test
    @Order(5)
    void testGetSubscribers() throws Exception {
        String serviceName = "nmms-subscribers-" + UUID.randomUUID().toString().substring(0, 8);

        // Register an instance
        namingService.registerInstance(serviceName, "10.0.210.1", 8080);
        Thread.sleep(1000);

        // Subscribe to the service via SDK
        namingService.subscribe(serviceName, event -> {
            // Listener just to create a subscription
        });
        Thread.sleep(2000);

        // Query subscribers via MaintainerService
        Page<?> subscribers = maintainerService.getSubscribers(
                DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName, 1, 10, false);
        assertNotNull(subscribers, "Subscribers page should not be null");
        // There should be at least one subscriber (our SDK client)
        assertTrue(subscribers.getTotalCount() >= 1,
                "Should have at least 1 subscriber, got: " + subscribers.getTotalCount());

        // Cleanup
        namingService.unsubscribe(serviceName, event -> {});
        namingService.deregisterInstance(serviceName, "10.0.210.1", 8080);
    }

    // ==================== P1: Register via MaintainerService ====================

    /**
     * NMMS-006: Test register instance with cluster name via MaintainerService
     */
    @Test
    @Order(6)
    void testRegisterInstanceWithCluster() throws Exception {
        String serviceName = "nmms-cluster-reg-" + UUID.randomUUID().toString().substring(0, 8);
        String clusterName = "TEST_CLUSTER";

        // Register via MaintainerService with cluster
        Instance instance = new Instance();
        instance.setIp("10.0.220.1");
        instance.setPort(8080);
        instance.setWeight(1.0);
        instance.setClusterName(clusterName);
        instance.setEphemeral(false); // persistent for maintainer registration
        instance.setHealthy(true);
        instance.setEnabled(true);

        maintainerService.registerInstance(serviceName, instance);
        Thread.sleep(1500);

        // Verify via MaintainerService list
        List<Instance> instances = maintainerService.listInstances(serviceName, clusterName, false);
        assertNotNull(instances, "Instance list should not be null");
        assertFalse(instances.isEmpty(), "Should have at least 1 instance in cluster");

        Instance found = instances.stream()
                .filter(i -> i.getIp().equals("10.0.220.1") && i.getPort() == 8080)
                .findFirst().orElse(null);
        assertNotNull(found, "Instance should be found in the specified cluster");
        assertEquals(clusterName, found.getClusterName(), "Cluster name should match");

        // Cleanup
        maintainerService.deregisterInstance(serviceName, "10.0.220.1", 8080, clusterName);
    }

    // ==================== P2: List Selector Types ====================

    /**
     * NMMS-007: Test list selector types
     *
     * Queries the available selector types from the server.
     */
    @Test
    @Order(7)
    @Disabled("SDK-side: NacosNamingMaintainerServiceImpl.listSelectorTypes() tries to deserialize Result wrapper as List<String>")
    void testListSelectorTypes() throws Exception {
        List<String> selectorTypes = maintainerService.listSelectorTypes();
        assertNotNull(selectorTypes, "Selector types list should not be null");
        assertFalse(selectorTypes.isEmpty(), "Should have at least one selector type");
        // Nacos supports at least "none" and "label" selectors
        assertTrue(selectorTypes.stream().anyMatch(s -> s.toLowerCase().contains("none")),
                "Should have 'none' selector type, got: " + selectorTypes);
    }

    // ==================== P1: Partial Update Instance ====================

    /**
     * NMMS-008: Test partial update instance metadata
     *
     * Registers instance with metadata, then partially updates only specific fields
     * without affecting other metadata.
     */
    @Test
    @Order(8)
    void testPartialUpdateInstance() throws Exception {
        String serviceName = "nmms-partial-upd-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instance with initial metadata
        Instance instance = new Instance();
        instance.setIp("10.0.230.1");
        instance.setPort(8080);
        instance.setWeight(1.0);
        instance.setEphemeral(true);
        Map<String, String> meta = new HashMap<>();
        meta.put("version", "1.0");
        meta.put("env", "test");
        meta.put("region", "us-east");
        instance.setMetadata(meta);

        namingService.registerInstance(serviceName, instance);
        Thread.sleep(1500);

        // Partial update - only change weight and add new metadata
        com.alibaba.nacos.api.naming.pojo.Service service = new com.alibaba.nacos.api.naming.pojo.Service();
        service.setName(serviceName);
        service.setGroupName(DEFAULT_GROUP);
        service.setNamespaceId(DEFAULT_NAMESPACE);

        Instance partialInstance = new Instance();
        partialInstance.setIp("10.0.230.1");
        partialInstance.setPort(8080);
        partialInstance.setEphemeral(true);
        Map<String, String> partialMeta = new HashMap<>();
        partialMeta.put("version", "2.0");
        partialMeta.put("deployed-by", "ci");
        partialInstance.setMetadata(partialMeta);

        maintainerService.partialUpdateInstance(service, partialInstance);
        Thread.sleep(1500);

        // Verify partial update - version should be updated, env and region should remain
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty(), "Should have at least 1 instance");
        Instance updated = instances.get(0);
        assertEquals("2.0", updated.getMetadata().get("version"), "Version should be updated");
        assertEquals("ci", updated.getMetadata().get("deployed-by"), "New metadata should be added");
        // Original metadata that wasn't in the partial update should be preserved
        assertEquals("test", updated.getMetadata().get("env"), "Env should be preserved");
        assertEquals("us-east", updated.getMetadata().get("region"), "Region should be preserved");

        // Cleanup
        namingService.deregisterInstance(serviceName, "10.0.230.1", 8080);
    }

    // ==================== P2: Service Ignore Empty ====================

    /**
     * NMMS-009: Test list services ignoring empty services
     *
     * Creates services with and without instances, then lists with ignoreEmptyService=true.
     */
    @Test
    @Order(9)
    void testListServicesIgnoreEmpty() throws Exception {
        String prefix = "nmms-ignore-" + UUID.randomUUID().toString().substring(0, 6);
        String emptyService = prefix + "-empty";
        String nonEmptyService = prefix + "-nonempty";

        // Create two services
        maintainerService.createService(DEFAULT_NAMESPACE, DEFAULT_GROUP, emptyService, true, 0.5f);
        maintainerService.createService(DEFAULT_NAMESPACE, DEFAULT_GROUP, nonEmptyService, true, 0.5f);
        Thread.sleep(500);

        // Register instance only in non-empty service
        namingService.registerInstance(nonEmptyService, "10.0.240.1", 8080);
        Thread.sleep(1500);

        // List with ignoreEmptyService=true
        Page<ServiceView> filteredPage = maintainerService.listServices(
                DEFAULT_NAMESPACE, DEFAULT_GROUP, prefix, true, 1, 100);
        assertNotNull(filteredPage, "Filtered page should not be null");

        boolean foundEmpty = false;
        boolean foundNonEmpty = false;
        for (ServiceView view : filteredPage.getPageItems()) {
            if (emptyService.equals(view.getName())) foundEmpty = true;
            if (nonEmptyService.equals(view.getName())) foundNonEmpty = true;
        }
        assertTrue(foundNonEmpty, "Non-empty service should appear in filtered list");
        assertFalse(foundEmpty, "Empty service should NOT appear when ignoreEmptyService=true");

        // Cleanup
        namingService.deregisterInstance(nonEmptyService, "10.0.240.1", 8080);
        Thread.sleep(500);
        try {
            maintainerService.removeService(DEFAULT_NAMESPACE, DEFAULT_GROUP, emptyService);
        } catch (Exception ignored) {
        }
        try {
            maintainerService.removeService(DEFAULT_NAMESPACE, DEFAULT_GROUP, nonEmptyService);
        } catch (Exception ignored) {
        }
    }

    // ==================== P2: List Healthy Only ====================

    /**
     * NMMS-010: Test list instances with healthy filter
     *
     * Registers healthy instances and verifies the healthyOnly filter works.
     */
    @Test
    @Order(10)
    void testListInstancesHealthyOnly() throws Exception {
        String serviceName = "nmms-healthy-filter-" + UUID.randomUUID().toString().substring(0, 8);

        // Register healthy instances
        namingService.registerInstance(serviceName, "10.0.250.1", 8080);
        namingService.registerInstance(serviceName, "10.0.250.2", 8081);
        Thread.sleep(2000);

        // List all instances (healthyOnly=false)
        List<Instance> all = maintainerService.listInstances(serviceName, null, false);
        assertNotNull(all, "All instances list should not be null");
        int allCount = all.size();
        assertTrue(allCount >= 2, "Should have at least 2 instances total");

        // List only healthy instances
        List<Instance> healthy = maintainerService.listInstances(serviceName, null, true);
        assertNotNull(healthy, "Healthy instances list should not be null");
        for (Instance inst : healthy) {
            assertTrue(inst.isHealthy(), "Instance " + inst.getIp() + " should be healthy");
        }

        // Cleanup
        namingService.deregisterInstance(serviceName, "10.0.250.1", 8080);
        namingService.deregisterInstance(serviceName, "10.0.250.2", 8081);
    }
}
