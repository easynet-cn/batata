package io.batata.tests;

import com.ctrip.framework.apollo.openapi.client.ApolloOpenApiClient;
import com.ctrip.framework.apollo.openapi.dto.OpenAppNamespaceDTO;
import com.ctrip.framework.apollo.openapi.dto.OpenNamespaceDTO;
import org.junit.jupiter.api.*;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class ApolloNamespaceTest extends ApolloTestBase {

    private String testAppId;
    private String testNamespaceName;

    @BeforeAll
    void setup() {
        String openApiUrl = System.getProperty("apollo.openapi.url", "http://127.0.0.1:8080");
        String token = System.getProperty("apollo.openapi.token", "admin");

        openApiClient = ApolloOpenApiClient.newBuilder()
                .withPortalUrl(openApiUrl)
                .withToken(token)
                .build();
        assertNotNull(openApiClient);
    }

    @BeforeEach
    void beforeEach() {
        testAppId = createTestApp();
    }

    @Test
    @Order(1)
    void testCreateAppNamespace() {
        testNamespaceName = createTestNamespace(testAppId);
        assertNotNull(testNamespaceName, "Namespace name should not be null");
        assertTrue(testNamespaceName.startsWith("test-ns-"), "Namespace name should start with 'test-ns-'");
    }

    @Test
    @Order(2)
    void testGetNamespace() {
        testNamespaceName = createTestNamespace(testAppId);

        OpenNamespaceDTO namespace = openApiClient.getNamespace(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName);
        assertNotNull(namespace, "Namespace should not be null");
        assertEquals(testNamespaceName, namespace.getNamespaceName(),
                "Namespace name should match");
        assertEquals(testAppId, namespace.getAppId(), "App ID should match");
    }

    @Test
    @Order(3)
    void testGetNamespaceNotFound() {
        assertThrows(Exception.class, () -> {
            openApiClient.getNamespace(
                    testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, "non-existent-namespace");
        });
    }

    @Test
    @Order(4)
    void testGetNamespaces() {
        createTestNamespace(testAppId);
        createTestNamespace(testAppId);

        List<OpenNamespaceDTO> namespaces = openApiClient.getNamespaces(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER);
        assertNotNull(namespaces, "Namespaces list should not be null");
        assertTrue(namespaces.size() >= 2, "Should have at least 2 namespaces");
    }

    @Test
    @Order(5)
    void testGetNamespaceWithItems() {
        testNamespaceName = createTestNamespace(testAppId);
        createConfigItem(testAppId, testNamespaceName, "ns.key1", "value1");
        createConfigItem(testAppId, testNamespaceName, "ns.key2", "value2");

        OpenNamespaceDTO namespace = openApiClient.getNamespace(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName);
        assertNotNull(namespace);
        assertNotNull(namespace.getItems(), "Items list should not be null");
        assertEquals(2, namespace.getItems().size(), "Should have 2 items");
    }

    @Test
    @Order(6)
    void testCreateDuplicateNamespace() {
        testNamespaceName = createTestNamespace(testAppId);

        assertThrows(Exception.class, () -> {
            OpenAppNamespaceDTO appNamespace = new OpenAppNamespaceDTO();
            appNamespace.setAppId(testAppId);
            appNamespace.setName(testNamespaceName);
            appNamespace.setFormat("properties");
            appNamespace.setDataChangeCreatedBy(OPERATOR);
            openApiClient.createAppNamespace(appNamespace);
        });
    }

    @Test
    @Order(7)
    void testGetNamespacesWithGrayDel() {
        testNamespaceName = createTestNamespace(testAppId);

        List<OpenNamespaceDTO> namespaces = openApiClient.getNamespaces(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, true);
        assertNotNull(namespaces, "Namespaces list should not be null");
        assertFalse(namespaces.isEmpty(), "Namespaces list should not be empty");
    }

    @Test
    @Order(8)
    void testGetNamespaceWithGrayDel() {
        testNamespaceName = createTestNamespace(testAppId);

        OpenNamespaceDTO namespace = openApiClient.getNamespace(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName, true);
        assertNotNull(namespace, "Namespace should not be null");
        assertEquals(testNamespaceName, namespace.getNamespaceName(),
                "Namespace name should match");
    }
}
