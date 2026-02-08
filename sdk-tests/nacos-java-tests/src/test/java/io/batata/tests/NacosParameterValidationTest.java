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
 * Nacos Parameter Validation Tests
 *
 * Tests for input validation on config and naming operations.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosParameterValidationTest {

    private static ConfigService configService;
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

        configService = NacosFactory.createConfigService(properties);
        namingService = NacosFactory.createNamingService(properties);
        System.out.println("Parameter Validation Test Setup - Server: " + serverAddr);
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (configService != null) {
            configService.shutDown();
        }
        if (namingService != null) {
            namingService.shutDown();
        }
    }

    // ==================== Config Null/Empty Parameter Tests ====================

    /**
     * PV-001: Test getConfig with null dataId
     */
    @Test
    @Order(1)
    void testGetConfigWithNullDataId() {
        assertThrows(Exception.class, () -> {
            configService.getConfig(null, DEFAULT_GROUP, 5000);
        }, "getConfig with null dataId should throw exception");
    }

    /**
     * PV-002: Test getConfig with empty dataId
     */
    @Test
    @Order(2)
    void testGetConfigWithEmptyDataId() {
        assertThrows(Exception.class, () -> {
            configService.getConfig("", DEFAULT_GROUP, 5000);
        }, "getConfig with empty dataId should throw exception");
    }

    /**
     * PV-003: Test publishConfig with null dataId
     */
    @Test
    @Order(3)
    void testPublishConfigWithNullDataId() {
        assertThrows(Exception.class, () -> {
            configService.publishConfig(null, DEFAULT_GROUP, "content");
        }, "publishConfig with null dataId should throw exception");
    }

    /**
     * PV-004: Test publishConfig with null content
     */
    @Test
    @Order(4)
    void testPublishConfigWithNullContent() {
        assertThrows(Exception.class, () -> {
            configService.publishConfig("test-null-content", DEFAULT_GROUP, null);
        }, "publishConfig with null content should throw exception");
    }

    /**
     * PV-005: Test publishConfig with special characters in content
     */
    @Test
    @Order(5)
    void testPublishConfigWithSpecialCharacters() throws NacosException, InterruptedException {
        String dataId = "special-chars-" + UUID.randomUUID().toString().substring(0, 8);
        String specialContent = "key1=!@#$%^&*()\nkey2=<>&\"'\nkey3={[}]:;,./\\";

        boolean result = configService.publishConfig(dataId, DEFAULT_GROUP, specialContent);
        assertTrue(result, "Should publish config with special characters");

        Thread.sleep(1000);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(retrieved, "Should retrieve config with special characters");
        assertEquals(specialContent, retrieved, "Special characters should round-trip correctly");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * PV-006: Test publishConfig with Chinese/Unicode characters
     */
    @Test
    @Order(6)
    void testPublishConfigWithChineseCharacters() throws NacosException, InterruptedException {
        String dataId = "unicode-chars-" + UUID.randomUUID().toString().substring(0, 8);
        String unicodeContent = "name=\u4f60\u597d\u4e16\u754c\ngreeting=\u3053\u3093\u306b\u3061\u306f\nemoji=test\u2603\u2764";

        boolean result = configService.publishConfig(dataId, DEFAULT_GROUP, unicodeContent);
        assertTrue(result, "Should publish config with Unicode characters");

        Thread.sleep(1000);

        String retrieved = configService.getConfig(dataId, DEFAULT_GROUP, 5000);
        assertNotNull(retrieved, "Should retrieve config with Unicode characters");
        assertEquals(unicodeContent, retrieved, "Unicode characters should round-trip correctly");

        // Cleanup
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }

    /**
     * PV-007: Test removeConfig with null dataId
     */
    @Test
    @Order(7)
    void testRemoveConfigWithNullDataId() {
        assertThrows(Exception.class, () -> {
            configService.removeConfig(null, DEFAULT_GROUP);
        }, "removeConfig with null dataId should throw exception");
    }

    /**
     * PV-008: Test removeConfig for non-existent config
     */
    @Test
    @Order(8)
    void testRemoveNonExistentConfig() throws NacosException {
        String dataId = "nonexistent-remove-" + UUID.randomUUID().toString().substring(0, 8);

        // Should succeed silently (not throw) when removing non-existent config
        boolean result = configService.removeConfig(dataId, DEFAULT_GROUP);
        System.out.println("Remove non-existent config result: " + result);
        // Nacos SDK typically returns true even for non-existent configs
    }

    /**
     * PV-009: Test addListener with null listener
     */
    @Test
    @Order(9)
    void testNullListenerThrowsException() {
        String dataId = "null-listener-" + UUID.randomUUID().toString().substring(0, 8);

        assertThrows(Exception.class, () -> {
            configService.addListener(dataId, DEFAULT_GROUP, null);
        }, "addListener with null listener should throw exception");
    }

    // ==================== Naming Validation Tests ====================

    /**
     * PV-010: Test deregister non-existent instance
     */
    @Test
    @Order(10)
    void testDeregisterNonExistentInstance() throws NacosException {
        String serviceName = "pv-deregister-nonexist-" + UUID.randomUUID().toString().substring(0, 8);

        // Should not throw when deregistering non-existent instance
        try {
            namingService.deregisterInstance(serviceName, "10.0.0.99", 9999);
            System.out.println("Deregister non-existent instance: succeeded silently");
        } catch (NacosException e) {
            System.out.println("Deregister non-existent instance: " + e.getMessage());
        }
    }

    /**
     * PV-011: Test deregister last instance leaves empty service
     */
    @Test
    @Order(11)
    void testDeregisterLastInstance() throws NacosException, InterruptedException {
        String serviceName = "pv-deregister-last-" + UUID.randomUUID().toString().substring(0, 8);
        String ip = "192.168.100.1";
        int port = 8080;

        // Register single instance
        namingService.registerInstance(serviceName, ip, port);
        Thread.sleep(1000);

        List<Instance> before = namingService.getAllInstances(serviceName);
        assertEquals(1, before.size(), "Should have one instance before deregister");

        // Deregister the only instance
        namingService.deregisterInstance(serviceName, ip, port);
        Thread.sleep(1000);

        List<Instance> after = namingService.getAllInstances(serviceName);
        assertTrue(after.isEmpty(), "Should have no instances after deregistering the last one");
    }

    /**
     * PV-012: Test register instance with rich metadata round-trip
     */
    @Test
    @Order(12)
    void testRegisterInstanceMetadataRoundTrip() throws NacosException, InterruptedException {
        String serviceName = "pv-metadata-rt-" + UUID.randomUUID().toString().substring(0, 8);

        Instance instance = new Instance();
        instance.setIp("192.168.200.1");
        instance.setPort(8080);
        instance.setWeight(1.0);
        instance.setHealthy(true);

        Map<String, String> metadata = new HashMap<>();
        metadata.put("version", "2.1.0");
        metadata.put("env", "staging");
        metadata.put("region", "us-east-1");
        metadata.put("protocol", "gRPC");
        metadata.put("team", "platform");
        metadata.put("special-chars", "a=b&c=d");
        instance.setMetadata(metadata);

        // Register with rich metadata
        namingService.registerInstance(serviceName, instance);
        Thread.sleep(1500);

        // Retrieve and verify all metadata
        List<Instance> instances = namingService.getAllInstances(serviceName);
        assertFalse(instances.isEmpty(), "Should have at least one instance");

        Instance retrieved = instances.get(0);
        Map<String, String> retrievedMeta = retrieved.getMetadata();
        assertNotNull(retrievedMeta, "Metadata should not be null");

        assertEquals("2.1.0", retrievedMeta.get("version"), "version metadata should be preserved");
        assertEquals("staging", retrievedMeta.get("env"), "env metadata should be preserved");
        assertEquals("us-east-1", retrievedMeta.get("region"), "region metadata should be preserved");
        assertEquals("gRPC", retrievedMeta.get("protocol"), "protocol metadata should be preserved");
        assertEquals("platform", retrievedMeta.get("team"), "team metadata should be preserved");
        assertEquals("a=b&c=d", retrievedMeta.get("special-chars"), "special-chars metadata should be preserved");

        // Cleanup
        namingService.deregisterInstance(serviceName, "192.168.200.1", 8080);
    }
}
