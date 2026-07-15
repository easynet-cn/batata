package io.batata.tests;

import com.ctrip.framework.apollo.openapi.client.ApolloOpenApiClient;
import com.ctrip.framework.apollo.openapi.dto.OpenAppDTO;
import com.ctrip.framework.apollo.openapi.dto.OpenCreateAppDTO;
import com.ctrip.framework.apollo.openapi.dto.OpenEnvClusterDTO;
import org.junit.jupiter.api.*;

import java.util.Collections;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class ApolloAppTest extends ApolloTestBase {

    private String testAppId;

    @BeforeAll
    void setup() {
        String openApiUrl = System.getProperty("apollo.openapi.url", "http://127.0.0.1:8080");
        String token = System.getProperty("apollo.openapi.token", "admin");

        openApiClient = ApolloOpenApiClient.newBuilder()
                .withPortalUrl(openApiUrl)
                .withToken(token)
                .build();
        assertNotNull(openApiClient, "ApolloOpenApiClient should be created successfully");
    }

    @Test
    @Order(1)
    void testCreateApp() {
        testAppId = createTestApp();
        assertNotNull(testAppId, "App ID should not be null");
        assertTrue(testAppId.startsWith("test-app-"), "App ID should start with 'test-app-'");
    }

    @Test
    @Order(2)
    void testGetAppsByIds() {
        assertNotNull(testAppId, "testAppId should be set by testCreateApp");

        List<OpenAppDTO> apps = openApiClient.getAppsByIds(Collections.singletonList(testAppId));
        assertNotNull(apps, "Apps list should not be null");
        assertFalse(apps.isEmpty(), "Apps list should not be empty");

        OpenAppDTO app = apps.stream()
                .filter(a -> testAppId.equals(a.getAppId()))
                .findFirst()
                .orElse(null);
        assertNotNull(app, "App should exist");
        assertEquals(testAppId, app.getAppId(), "App ID should match");
        assertEquals("TEST_ORG", app.getOrgId(), "Org ID should match");
    }

    @Test
    @Order(3)
    void testGetAppsByIdsNotFound() {
        List<OpenAppDTO> apps = openApiClient.getAppsByIds(
                Collections.singletonList("non-existent-app-" + System.currentTimeMillis()));
        assertNotNull(apps, "Apps list should not be null");
        assertTrue(apps.isEmpty(), "Apps list should be empty for non-existent app");
    }

    @Test
    @Order(4)
    void testGetAllApps() {
        List<OpenAppDTO> apps = openApiClient.getAllApps();
        assertNotNull(apps, "Apps list should not be null");

        assertTrue(apps.stream().anyMatch(a -> testAppId.equals(a.getAppId())),
                "Test app should be present in all apps");
    }

    @Test
    @Order(5)
    void testGetEnvClusterInfo() {
        assertNotNull(testAppId, "testAppId should be set");

        List<OpenEnvClusterDTO> envClusters = openApiClient.getEnvClusterInfo(testAppId);
        assertNotNull(envClusters, "Env cluster info should not be null");
        assertFalse(envClusters.isEmpty(), "Env cluster info should not be empty");
    }

    @Test
    @Order(6)
    void testGetEnvClusterInfoNotFound() {
        assertThrows(Exception.class, () -> {
            openApiClient.getEnvClusterInfo("non-existent-app-" + System.currentTimeMillis());
        });
    }

    @Test
    @Order(7)
    void testCreateDuplicateApp() {
        assertNotNull(testAppId, "testAppId should be set");

        assertThrows(Exception.class, () -> {
            OpenAppDTO app = new OpenAppDTO();
            app.setAppId(testAppId);
            app.setName("Duplicate App");
            app.setOrgId("TEST_ORG");
            app.setOrgName("Test Organization");
            app.setOwnerName("Test Owner");
            app.setOwnerEmail("test@example.com");
            app.setDataChangeCreatedBy(OPERATOR);

            OpenCreateAppDTO createAppDTO = new OpenCreateAppDTO();
            createAppDTO.setApp(app);
            openApiClient.createApp(createAppDTO);
        });
    }
}
