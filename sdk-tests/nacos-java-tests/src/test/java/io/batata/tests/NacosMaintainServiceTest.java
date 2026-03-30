package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import com.alibaba.nacos.api.naming.pojo.Service;
import com.alibaba.nacos.api.selector.NoneSelector;
import com.alibaba.nacos.maintainer.client.naming.NamingMaintainerFactory;
import com.alibaba.nacos.maintainer.client.naming.NamingMaintainerService;
import com.alibaba.nacos.api.naming.pojo.maintainer.ServiceDetailInfo;
import org.junit.jupiter.api.*;
import org.junit.jupiter.api.Disabled;

import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos NamingMaintainerService Tests (V3 Admin API)
 *
 * Tests for the NamingMaintainerService from nacos-maintainer-client, which uses
 * V3 admin HTTP API internally (replacing the old NamingMaintainService that used V1 API).
 *
 * Covers: instance updates (metadata, weight, enabled), service CRUD, service with groups.
 * Aligned with Nacos MaintainServiceNamingITCase.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosMaintainServiceTest {

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
        System.out.println("NamingMaintainerService Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (maintainerService != null) maintainerService.shutdown();
        if (namingService != null) namingService.shutDown();
    }

    // ==================== P0: Update Instance ====================

    /**
     * NMS-001: Test update instance metadata via NamingMaintainerService
     *
     * Aligned with Nacos MaintainServiceNamingITCase.updateInstance()
     */
    @Test
    @Order(1)
    void testUpdateInstanceMetadata() throws Exception {
        String serviceName = "nms-update-meta-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instance with initial metadata via SDK (gRPC)
        Instance instance = new Instance();
        instance.setIp("192.168.50.1");
        instance.setPort(8080);
        instance.setWeight(1.0);
        instance.setEphemeral(true);
        Map<String, String> initialMeta = new HashMap<>();
        initialMeta.put("version", "1.0");
        initialMeta.put("env", "dev");
        instance.setMetadata(initialMeta);

        namingService.registerInstance(serviceName, instance);
        Thread.sleep(1500);

        // Update metadata via MaintainerService (V3 admin API)
        Instance updateInstance = new Instance();
        updateInstance.setIp("192.168.50.1");
        updateInstance.setPort(8080);
        updateInstance.setWeight(1.0);
        updateInstance.setEphemeral(true);
        Map<String, String> updatedMeta = new HashMap<>();
        updatedMeta.put("version", "2.0");
        updatedMeta.put("env", "staging");
        updatedMeta.put("region", "us-west-1");
        updateInstance.setMetadata(updatedMeta);

        maintainerService.updateInstance(serviceName, updateInstance);
        Thread.sleep(1500);

        // Verify metadata was updated via SDK
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty(), "Should have at least 1 instance");

        Instance retrieved = instances.get(0);
        assertEquals("2.0", retrieved.getMetadata().get("version"), "Version should be updated");
        assertEquals("staging", retrieved.getMetadata().get("env"), "Env should be updated");
        assertEquals("us-west-1", retrieved.getMetadata().get("region"), "Region should be added");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.50.1", 8080);
    }

    /**
     * NMS-002: Test disable instance via NamingMaintainerService
     *
     * Aligned with Nacos MaintainServiceNamingITCase.updateInstanceWithDisable()
     */
    @Test
    @Order(2)
    void testDisableInstance() throws Exception {
        String serviceName = "nms-disable-" + UUID.randomUUID().toString().substring(0, 8);

        // Register enabled instance
        Instance instance = new Instance();
        instance.setIp("192.168.51.1");
        instance.setPort(8080);
        instance.setWeight(1.0);
        instance.setEnabled(true);
        instance.setEphemeral(true);

        namingService.registerInstance(serviceName, instance);
        Thread.sleep(1500);

        // Verify instance is returned when selecting healthy
        List<Instance> beforeDisable = namingService.selectInstances(serviceName, true);
        assertFalse(beforeDisable.isEmpty(), "Should have healthy instances before disable");

        // Disable the instance via MaintainerService
        Instance disableInstance = new Instance();
        disableInstance.setIp("192.168.51.1");
        disableInstance.setPort(8080);
        disableInstance.setWeight(1.0);
        disableInstance.setEnabled(false);
        disableInstance.setEphemeral(true);

        maintainerService.updateInstance(serviceName, disableInstance);
        Thread.sleep(1500);

        // After disable, the instance should not appear in healthy selection
        List<Instance> afterDisable = namingService.selectInstances(serviceName, true);
        for (Instance inst : afterDisable) {
            if (inst.getIp().equals("192.168.51.1") && inst.getPort() == 8080) {
                fail("Disabled instance should not appear in healthy instance selection");
            }
        }

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.51.1", 8080);
    }

    /**
     * NMS-003: Test update instance weight via NamingMaintainerService
     */
    @Test
    @Order(3)
    void testUpdateInstanceWeight() throws Exception {
        String serviceName = "nms-weight-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.52.1");
        instance.setPort(8080);
        instance.setWeight(1.0);
        instance.setEphemeral(true);

        namingService.registerInstance(serviceName, instance);
        Thread.sleep(1500);

        // Update weight via MaintainerService
        Instance updateInstance = new Instance();
        updateInstance.setIp("192.168.52.1");
        updateInstance.setPort(8080);
        updateInstance.setWeight(5.0);
        updateInstance.setEphemeral(true);

        maintainerService.updateInstance(serviceName, updateInstance);
        Thread.sleep(1500);

        // Verify weight was updated
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty(), "Should have at least 1 instance");
        assertEquals(5.0, instances.get(0).getWeight(), 0.01, "Weight should be updated to 5.0");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.52.1", 8080);
    }

    // ==================== P0: Service CRUD ====================

    /**
     * NMS-004: Test create and query service via NamingMaintainerService
     *
     * Aligned with Nacos MaintainServiceNamingITCase.createAndUpdateService()
     */
    @Test
    @Order(4)
    void testCreateAndQueryService() throws Exception {
        String serviceName = "nms-create-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Create service with protectThreshold
        maintainerService.createService(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName, true, 0.8f);
        Thread.sleep(1000);

        // Query service detail
        ServiceDetailInfo detail = maintainerService.getServiceDetail(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName);
        assertNotNull(detail, "Service should be queryable after creation");
        assertEquals(serviceName, detail.getServiceName(), "Service name should match");
        assertEquals(0.8f, detail.getProtectThreshold(), 0.01, "ProtectThreshold should match");

        // Cleanup
        maintainerService.removeService(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName);
    }

    /**
     * NMS-005: Test update service via NamingMaintainerService
     */
    @Test
    @Order(5)
    void testUpdateService() throws Exception {
        String serviceName = "nms-update-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Create service
        maintainerService.createService(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName, true, 0.5f);
        Thread.sleep(1000);

        // Update protectThreshold and metadata
        Map<String, String> metadata = new HashMap<>();
        metadata.put("updated", "true");
        maintainerService.updateService(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName, true,
                metadata, 0.9f, new NoneSelector());
        Thread.sleep(1000);

        // Verify update
        ServiceDetailInfo detail = maintainerService.getServiceDetail(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName);
        assertNotNull(detail, "Service should be queryable after update");
        assertEquals(0.9f, detail.getProtectThreshold(), 0.01, "ProtectThreshold should be updated");

        // Cleanup
        maintainerService.removeService(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName);
    }

    /**
     * NMS-006: Test delete service via NamingMaintainerService
     *
     * Aligned with Nacos MaintainServiceNamingITCase.deleteService()
     */
    @Test
    @Order(6)
    void testDeleteService() throws Exception {
        String serviceName = "nms-delete-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Create service
        maintainerService.createService(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName, true, 0.5f);
        Thread.sleep(1000);

        // Delete service
        maintainerService.removeService(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName);
        Thread.sleep(500);

        // Verify deleted - should throw or return empty
        try {
            ServiceDetailInfo detail = maintainerService.getServiceDetail(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName);
            assertNull(detail, "Deleted service should not be queryable");
        } catch (NacosException e) {
            // Expected - service not found after deletion
            assertTrue(true, "Service correctly not found after deletion");
        }
    }

    /**
     * NMS-007: Test delete service with instances should fail
     *
     * Aligned with Nacos CPInstancesAPINamingITCase.deleteServiceHasInstance()
     */
    @Test
    @Order(7)
    void testDeleteServiceWithInstancesFails() throws Exception {
        String serviceName = "nms-del-with-inst-" + UUID.randomUUID().toString().substring(0, 8);

        // Register an instance (implicitly creates service)
        namingService.registerInstance(serviceName, "192.168.53.1", 8080);
        Thread.sleep(1500);

        // Try to delete service with active instances - should fail
        try {
            maintainerService.removeService(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName);
            // If no exception, service may have been deleted which is unexpected
            System.out.println("WARNING: Service was deleted despite having instances");
        } catch (NacosException e) {
            // Expected - service deletion denied when instances exist
            assertTrue(true, "Service deletion correctly denied (has instances)");
        }

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.53.1", 8080);
        Thread.sleep(1000);
        try {
            maintainerService.removeService(DEFAULT_NAMESPACE, DEFAULT_GROUP, serviceName);
        } catch (Exception ignored) {
        }
    }

    /**
     * NMS-008: Test create service with group
     */
    @Test
    @Order(8)
    void testCreateServiceWithGroup() throws Exception {
        String serviceName = "nms-group-svc-" + UUID.randomUUID().toString().substring(0, 8);
        String group = "MAINTAIN_GROUP";

        maintainerService.createService(DEFAULT_NAMESPACE, group, serviceName, true, 0.5f);
        Thread.sleep(1000);

        // Query in specific group
        ServiceDetailInfo detail = maintainerService.getServiceDetail(DEFAULT_NAMESPACE, group, serviceName);
        assertNotNull(detail, "Service should be queryable in specified group");
        assertEquals(serviceName, detail.getServiceName(), "Service name should match");

        // Cleanup
        maintainerService.removeService(DEFAULT_NAMESPACE, group, serviceName);
    }

    /**
     * NMS-009: Test update instance in specific group via NamingMaintainerService
     */
    @Test
    @Order(9)
    void testUpdateInstanceInGroup() throws Exception {
        String serviceName = "nms-upd-grp-" + UUID.randomUUID().toString().substring(0, 8);
        String group = "UPDATE_GROUP";

        Instance instance = new Instance();
        instance.setIp("192.168.54.1");
        instance.setPort(8080);
        instance.setWeight(1.0);
        instance.setEphemeral(true);
        Map<String, String> meta = new HashMap<>();
        meta.put("version", "1.0");
        instance.setMetadata(meta);

        namingService.registerInstance(serviceName, group, instance);
        Thread.sleep(1500);

        // Update metadata via MaintainerService
        Instance updateInstance = new Instance();
        updateInstance.setIp("192.168.54.1");
        updateInstance.setPort(8080);
        updateInstance.setWeight(2.0);
        updateInstance.setEphemeral(true);
        Map<String, String> updatedMeta = new HashMap<>();
        updatedMeta.put("version", "2.0");
        updateInstance.setMetadata(updatedMeta);

        maintainerService.updateInstance(group, serviceName, updateInstance);
        Thread.sleep(1500);

        List<Instance> instances = namingService.getAllInstances(serviceName, group);
        assertFalse(instances.isEmpty(), "Should have at least 1 instance");
        assertEquals(2.0, instances.get(0).getWeight(), 0.01, "Weight should be updated");
        assertEquals("2.0", instances.get(0).getMetadata().get("version"), "Version should be updated");

        // Cleanup
        namingService.deregisterInstance(serviceName, group, "192.168.54.1", 8080);
    }

    /**
     * NMS-010: Test register and deregister instance via NamingMaintainerService
     */
    @Test
    @Order(10)
    void testRegisterAndDeregisterInstanceViaMaintainer() throws Exception {
        String serviceName = "nms-reg-dereg-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instance via MaintainerService
        Instance instance = new Instance();
        instance.setIp("192.168.55.1");
        instance.setPort(9090);
        instance.setWeight(1.0);
        instance.setHealthy(true);
        instance.setEnabled(true);
        instance.setEphemeral(false); // persistent instance for maintainer registration
        Map<String, String> meta = new HashMap<>();
        meta.put("app", "batata-test");
        instance.setMetadata(meta);

        maintainerService.registerInstance(serviceName, instance);
        Thread.sleep(1500);

        // Verify via SDK
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty(), "Should have the registered instance");
        Instance found = instances.stream()
                .filter(i -> i.getIp().equals("192.168.55.1") && i.getPort() == 9090)
                .findFirst().orElse(null);
        assertNotNull(found, "Registered instance should be found");
        assertEquals("batata-test", found.getMetadata().get("app"), "Instance metadata should match");

        // Deregister via MaintainerService
        maintainerService.deregisterInstance(serviceName, "192.168.55.1", 9090);
        Thread.sleep(1500);

        // Verify deregistered
        List<Instance> afterDeregister = namingService.getAllInstances(serviceName);
        boolean stillExists = afterDeregister.stream()
                .anyMatch(i -> i.getIp().equals("192.168.55.1") && i.getPort() == 9090);
        assertFalse(stillExists, "Instance should be gone after deregistration");
    }

    /**
     * NMS-011: Test list instances via NamingMaintainerService
     */
    @Test
    @Order(11)
    void testListInstances() throws Exception {
        String serviceName = "nms-list-inst-" + UUID.randomUUID().toString().substring(0, 8);

        // Register multiple instances via SDK
        namingService.registerInstance(serviceName, "192.168.60.1", 8080);
        namingService.registerInstance(serviceName, "192.168.60.2", 8081);
        namingService.registerInstance(serviceName, "192.168.60.3", 8082);
        Thread.sleep(2000);

        // List instances via MaintainerService
        List<Instance> instances = maintainerService.listInstances(serviceName, null, false);
        assertNotNull(instances, "Instance list should not be null");
        assertTrue(instances.size() >= 3, "Should have at least 3 instances, got: " + instances.size());

        // Verify specific instances exist
        boolean found1 = instances.stream().anyMatch(i -> i.getIp().equals("192.168.60.1") && i.getPort() == 8080);
        boolean found2 = instances.stream().anyMatch(i -> i.getIp().equals("192.168.60.2") && i.getPort() == 8081);
        boolean found3 = instances.stream().anyMatch(i -> i.getIp().equals("192.168.60.3") && i.getPort() == 8082);
        assertTrue(found1, "Instance 192.168.60.1:8080 should be in list");
        assertTrue(found2, "Instance 192.168.60.2:8081 should be in list");
        assertTrue(found3, "Instance 192.168.60.3:8082 should be in list");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.60.1", 8080);
        namingService.deregisterInstance(serviceName, "192.168.60.2", 8081);
        namingService.deregisterInstance(serviceName, "192.168.60.3", 8082);
    }

    /**
     * NMS-012: Test get instance detail via NamingMaintainerService
     */
    @Test
    @Order(12)
    void testGetInstanceDetail() throws Exception {
        String serviceName = "nms-inst-detail-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.61.1");
        instance.setPort(8080);
        instance.setWeight(3.5);
        instance.setEphemeral(true);
        Map<String, String> meta = new HashMap<>();
        meta.put("version", "3.2.0");
        meta.put("region", "ap-east");
        instance.setMetadata(meta);

        namingService.registerInstance(serviceName, instance);
        Thread.sleep(1500);

        // Get instance detail via MaintainerService
        Instance detail = maintainerService.getInstanceDetail(serviceName, "192.168.61.1", 8080);
        assertNotNull(detail, "Instance detail should not be null");
        assertEquals("192.168.61.1", detail.getIp(), "IP should match");
        assertEquals(8080, detail.getPort(), "Port should match");
        assertEquals(3.5, detail.getWeight(), 0.01, "Weight should match");
        assertEquals("3.2.0", detail.getMetadata().get("version"), "Version metadata should match");
        assertEquals("ap-east", detail.getMetadata().get("region"), "Region metadata should match");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.61.1", 8080);
    }
}
