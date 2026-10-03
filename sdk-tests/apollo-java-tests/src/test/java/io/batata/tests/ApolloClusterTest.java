package io.batata.tests;

import com.ctrip.framework.apollo.openapi.client.ApolloOpenApiClient;
import com.ctrip.framework.apollo.openapi.dto.OpenClusterDTO;
import org.junit.jupiter.api.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Cluster management via the Apollo OpenAPI client: createCluster and getCluster.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class ApolloClusterTest extends ApolloTestBase {

    private String testAppId;

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
    void testGetDefaultCluster() {
        OpenClusterDTO cluster = openApiClient.getCluster(testAppId, DEFAULT_ENV, DEFAULT_CLUSTER);
        assertNotNull(cluster, "Default cluster should exist");
        assertEquals(DEFAULT_CLUSTER, cluster.getName(), "Cluster name should be 'default'");
        assertEquals(testAppId, cluster.getAppId(), "App ID should match");
    }

    @Test
    @Order(2)
    void testCreateCluster() {
        String clusterName = "cluster-" + java.util.UUID.randomUUID().toString().substring(0, 8);

        OpenClusterDTO newCluster = new OpenClusterDTO();
        newCluster.setAppId(testAppId);
        newCluster.setName(clusterName);
        newCluster.setDataChangeCreatedBy(OPERATOR);

        OpenClusterDTO created = openApiClient.createCluster(DEFAULT_ENV, newCluster);
        assertNotNull(created, "Created cluster should not be null");
        assertEquals(clusterName, created.getName(), "Created cluster name should match");
        assertEquals(testAppId, created.getAppId(), "Created cluster app id should match");
    }

    @Test
    @Order(3)
    void testGetCreatedCluster() {
        String clusterName = "cluster-" + java.util.UUID.randomUUID().toString().substring(0, 8);

        OpenClusterDTO newCluster = new OpenClusterDTO();
        newCluster.setAppId(testAppId);
        newCluster.setName(clusterName);
        newCluster.setDataChangeCreatedBy(OPERATOR);
        openApiClient.createCluster(DEFAULT_ENV, newCluster);

        OpenClusterDTO fetched = openApiClient.getCluster(testAppId, DEFAULT_ENV, clusterName);
        assertNotNull(fetched, "Created cluster should be retrievable");
        assertEquals(clusterName, fetched.getName());
        assertEquals(testAppId, fetched.getAppId());
    }

    @Test
    @Order(4)
    void testGetClusterNotFound() {
        assertThrows(Exception.class, () -> {
            openApiClient.getCluster(testAppId, DEFAULT_ENV, "non-existent-cluster");
        }, "Fetching a non-existent cluster should throw");
    }
}
