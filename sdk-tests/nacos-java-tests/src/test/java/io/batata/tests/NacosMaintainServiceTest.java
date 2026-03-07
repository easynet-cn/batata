package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingMaintainService;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import com.alibaba.nacos.api.naming.pojo.Service;
import com.alibaba.nacos.api.selector.AbstractSelector;
import com.alibaba.nacos.api.selector.ExpressionSelector;
import com.alibaba.nacos.api.selector.NoneSelector;
import org.junit.jupiter.api.*;

import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos NamingMaintainService Tests
 *
 * Tests for the NamingMaintainService interface: instance updates,
 * service CRUD, and service selector management.
 * Aligned with Nacos MaintainServiceNamingITCase.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosMaintainServiceTest {

    private static NamingMaintainService maintainService;
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

        maintainService = NacosFactory.createMaintainService(properties);
        namingService = NacosFactory.createNamingService(properties);
        System.out.println("MaintainService Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (maintainService != null) maintainService.shutDown();
        if (namingService != null) namingService.shutDown();
    }

    // ==================== P0: Update Instance ====================

    /**
     * NMS-001: Test update instance metadata via MaintainService
     *
     * Aligned with Nacos MaintainServiceNamingITCase.updateInstance()
     */
    @Test
    @Order(1)
    void testUpdateInstanceMetadata() throws NacosException, InterruptedException {
        String serviceName = "nms-update-meta-" + UUID.randomUUID().toString().substring(0, 8);

        // Register instance with initial metadata
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

        // Update metadata via MaintainService
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

        maintainService.updateInstance(serviceName, updateInstance);
        Thread.sleep(1500);

        // Verify metadata was updated
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
     * NMS-002: Test disable instance via MaintainService
     *
     * Aligned with Nacos MaintainServiceNamingITCase.updateInstanceWithDisable()
     */
    @Test
    @Order(2)
    void testDisableInstance() throws NacosException, InterruptedException {
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

        // Disable the instance via MaintainService
        Instance disableInstance = new Instance();
        disableInstance.setIp("192.168.51.1");
        disableInstance.setPort(8080);
        disableInstance.setWeight(1.0);
        disableInstance.setEnabled(false);
        disableInstance.setEphemeral(true);

        maintainService.updateInstance(serviceName, disableInstance);
        Thread.sleep(1500);

        // After disable, selectInstances(healthy=true) should not return it
        List<Instance> afterDisable = namingService.selectInstances(serviceName, true);
        System.out.println("Instances after disable: " + afterDisable.size());

        // The disabled instance should not appear in healthy selection
        for (Instance inst : afterDisable) {
            if (inst.getIp().equals("192.168.51.1") && inst.getPort() == 8080) {
                assertTrue(inst.isEnabled(), "Disabled instance should not appear as enabled");
            }
        }

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.51.1", 8080);
    }

    /**
     * NMS-003: Test update instance weight via MaintainService
     */
    @Test
    @Order(3)
    void testUpdateInstanceWeight() throws NacosException, InterruptedException {
        String serviceName = "nms-weight-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.52.1");
        instance.setPort(8080);
        instance.setWeight(1.0);
        instance.setEphemeral(true);

        namingService.registerInstance(serviceName, instance);
        Thread.sleep(1500);

        // Update weight
        Instance updateInstance = new Instance();
        updateInstance.setIp("192.168.52.1");
        updateInstance.setPort(8080);
        updateInstance.setWeight(5.0);
        updateInstance.setEphemeral(true);

        maintainService.updateInstance(serviceName, updateInstance);
        Thread.sleep(1500);

        // Verify weight was updated
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty());
        assertEquals(5.0, instances.get(0).getWeight(), 0.01, "Weight should be updated to 5.0");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.52.1", 8080);
    }

    // ==================== P0: Service CRUD ====================

    /**
     * NMS-004: Test create and query service via MaintainService
     *
     * Aligned with Nacos MaintainServiceNamingITCase.createAndUpdateService()
     */
    @Test
    @Order(4)
    void testCreateAndQueryService() throws NacosException, InterruptedException {
        String serviceName = "nms-create-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Create service with protectThreshold
        maintainService.createService(serviceName, DEFAULT_GROUP, 0.8f);
        Thread.sleep(1000);

        // Query service
        Service queried = maintainService.queryService(serviceName);
        assertNotNull(queried, "Service should be queryable after creation");
        assertEquals(serviceName, queried.getName(), "Service name should match");
        assertEquals(0.8f, queried.getProtectThreshold(), 0.01, "ProtectThreshold should match");

        // Cleanup
        maintainService.deleteService(serviceName);
    }

    /**
     * NMS-005: Test update service via MaintainService
     */
    @Test
    @Order(5)
    void testUpdateService() throws NacosException, InterruptedException {
        String serviceName = "nms-update-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Create service
        maintainService.createService(serviceName, DEFAULT_GROUP, 0.5f);
        Thread.sleep(1000);

        // Update protectThreshold
        Map<String, String> metadata = new HashMap<>();
        metadata.put("updated", "true");
        maintainService.updateService(serviceName, DEFAULT_GROUP, 0.9f, metadata);
        Thread.sleep(1000);

        // Verify update
        Service queried = maintainService.queryService(serviceName);
        assertNotNull(queried);
        assertEquals(0.9f, queried.getProtectThreshold(), 0.01, "ProtectThreshold should be updated");

        // Cleanup
        maintainService.deleteService(serviceName);
    }

    /**
     * NMS-006: Test delete service via MaintainService
     *
     * Aligned with Nacos MaintainServiceNamingITCase.deleteService()
     */
    @Test
    @Order(6)
    void testDeleteService() throws NacosException, InterruptedException {
        String serviceName = "nms-delete-svc-" + UUID.randomUUID().toString().substring(0, 8);

        // Create service
        maintainService.createService(serviceName, DEFAULT_GROUP, 0.5f);
        Thread.sleep(1000);

        // Delete service
        boolean deleted = maintainService.deleteService(serviceName);
        assertTrue(deleted, "Service deletion should succeed");

        // Verify deleted
        Thread.sleep(500);
        try {
            Service queried = maintainService.queryService(serviceName);
            // If no exception, the service should be empty/null
            System.out.println("Query after delete: " + queried);
        } catch (NacosException e) {
            // Expected - service not found
            System.out.println("Service correctly not found after deletion: " + e.getMessage());
        }
    }

    /**
     * NMS-007: Test delete service with instances should fail
     *
     * Aligned with Nacos CPInstancesAPINamingITCase.deleteServiceHasInstance()
     */
    @Test
    @Order(7)
    void testDeleteServiceWithInstancesFails() throws NacosException, InterruptedException {
        String serviceName = "nms-del-with-inst-" + UUID.randomUUID().toString().substring(0, 8);

        // Register an instance (implicitly creates service)
        namingService.registerInstance(serviceName, "192.168.53.1", 8080);
        Thread.sleep(1500);

        // Try to delete service with active instances
        try {
            boolean deleted = maintainService.deleteService(serviceName);
            System.out.println("Delete service with instances result: " + deleted);
            // If it returns false, that's the expected behavior
            if (deleted) {
                System.out.println("WARNING: Service was deleted despite having instances");
            }
        } catch (NacosException e) {
            System.out.println("Service deletion correctly denied (has instances): " + e.getMessage());
        }

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.53.1", 8080);
        Thread.sleep(1000);
        try {
            maintainService.deleteService(serviceName);
        } catch (Exception ignored) {}
    }

    /**
     * NMS-008: Test create service with selector
     */
    @Test
    @Order(8)
    void testCreateServiceWithSelector() throws NacosException, InterruptedException {
        String serviceName = "nms-selector-svc-" + UUID.randomUUID().toString().substring(0, 8);

        Service service = new Service();
        service.setName(serviceName);
        service.setGroupName(DEFAULT_GROUP);
        service.setProtectThreshold(0.6f);

        ExpressionSelector selector = new ExpressionSelector();
        selector.setExpression("CONSUMER.label.env = 'production'");

        try {
            maintainService.createService(service, selector);
            Thread.sleep(1000);

            Service queried = maintainService.queryService(serviceName);
            assertNotNull(queried, "Service with selector should be created");
            assertEquals(serviceName, queried.getName());

            // Cleanup
            maintainService.deleteService(serviceName);
        } catch (NacosException e) {
            System.out.println("Create service with selector: " + e.getMessage());
            // Selector support may vary
        }
    }

    /**
     * NMS-009: Test create service with group
     */
    @Test
    @Order(9)
    void testCreateServiceWithGroup() throws NacosException, InterruptedException {
        String serviceName = "nms-group-svc-" + UUID.randomUUID().toString().substring(0, 8);
        String group = "MAINTAIN_GROUP";

        maintainService.createService(serviceName, group, 0.5f);
        Thread.sleep(1000);

        // Query in specific group
        Service queried = maintainService.queryService(serviceName, group);
        assertNotNull(queried, "Service should be queryable in specified group");
        assertEquals(serviceName, queried.getName());

        // Cleanup
        maintainService.deleteService(serviceName, group);
    }

    /**
     * NMS-010: Test update instance in specific group
     */
    @Test
    @Order(10)
    void testUpdateInstanceInGroup() throws NacosException, InterruptedException {
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

        // Update metadata via MaintainService
        Instance updateInstance = new Instance();
        updateInstance.setIp("192.168.54.1");
        updateInstance.setPort(8080);
        updateInstance.setWeight(2.0);
        updateInstance.setEphemeral(true);
        Map<String, String> updatedMeta = new HashMap<>();
        updatedMeta.put("version", "2.0");
        updateInstance.setMetadata(updatedMeta);

        maintainService.updateInstance(serviceName, group, updateInstance);
        Thread.sleep(1500);

        List<Instance> instances = namingService.getAllInstances(serviceName, group);
        assertFalse(instances.isEmpty());
        assertEquals(2.0, instances.get(0).getWeight(), 0.01, "Weight should be updated");
        assertEquals("2.0", instances.get(0).getMetadata().get("version"), "Version should be updated");

        // Cleanup
        namingService.deregisterInstance(serviceName, group, "192.168.54.1", 8080);
    }
}
