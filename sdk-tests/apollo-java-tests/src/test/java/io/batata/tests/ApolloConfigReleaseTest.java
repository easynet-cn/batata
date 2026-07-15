package io.batata.tests;

import com.ctrip.framework.apollo.openapi.client.ApolloOpenApiClient;
import com.ctrip.framework.apollo.openapi.dto.NamespaceReleaseDTO;
import com.ctrip.framework.apollo.openapi.dto.OpenReleaseDTO;
import org.junit.jupiter.api.*;

import java.util.Map;

import static org.junit.jupiter.api.Assertions.*;

@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class ApolloConfigReleaseTest extends ApolloTestBase {

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
        testNamespaceName = createTestNamespace(testAppId);
    }

    @Test
    @Order(1)
    void testPublishNamespace() {
        createConfigItem(testAppId, testNamespaceName, "release.key", "release.value");
        releaseNamespace(testAppId, testNamespaceName);

        OpenReleaseDTO release = openApiClient.getLatestActiveRelease(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName);
        assertNotNull(release, "Release should not be null after publishing");
        assertEquals(testAppId, release.getAppId(), "App ID should match");
        assertEquals(testNamespaceName, release.getNamespaceName(), "Namespace name should match");
    }

    @Test
    @Order(2)
    void testGetLatestActiveRelease() {
        createConfigItem(testAppId, testNamespaceName, "key1", "value1");
        releaseNamespace(testAppId, testNamespaceName);

        OpenReleaseDTO release = openApiClient.getLatestActiveRelease(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName);
        assertNotNull(release, "Latest active release should not be null");
        assertNotNull(release.getName(), "Release name should not be null");
        assertNotNull(release.getConfigurations(), "Release configurations should not be null");
    }

    @Test
    @Order(3)
    void testReleaseConfigurations() {
        createConfigItem(testAppId, testNamespaceName, "config.key1", "value1");
        createConfigItem(testAppId, testNamespaceName, "config.key2", "value2");
        releaseNamespace(testAppId, testNamespaceName);

        OpenReleaseDTO release = openApiClient.getLatestActiveRelease(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName);
        assertNotNull(release);

        Map<String, String> configurations = release.getConfigurations();
        assertNotNull(configurations, "Configurations should not be null");
        assertEquals("value1", configurations.get("config.key1"),
                "Configuration for config.key1 should match");
        assertEquals("value2", configurations.get("config.key2"),
                "Configuration for config.key2 should match");
    }

    @Test
    @Order(4)
    void testPublishWithComment() {
        createConfigItem(testAppId, testNamespaceName, "comment.key", "comment.value");

        NamespaceReleaseDTO release = new NamespaceReleaseDTO();
        release.setReleaseTitle("Commented release " + System.currentTimeMillis());
        release.setReleaseComment("Custom release comment");
        release.setReleasedBy(OPERATOR);
        release.setEmergencyPublish(false);
        openApiClient.publishNamespace(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName, release);

        OpenReleaseDTO activeRelease = openApiClient.getLatestActiveRelease(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName);
        assertNotNull(activeRelease);
        assertEquals("Custom release comment", activeRelease.getComment(),
                "Release comment should match");
    }

    @Test
    @Order(5)
    void testPublishMultipleReleases() {
        createConfigItem(testAppId, testNamespaceName, "multi.key", "initial");
        releaseNamespace(testAppId, testNamespaceName);

        OpenReleaseDTO firstRelease = openApiClient.getLatestActiveRelease(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName);
        assertNotNull(firstRelease);

        updateConfigItem(testAppId, testNamespaceName, "multi.key", "updated");
        releaseNamespace(testAppId, testNamespaceName);

        OpenReleaseDTO secondRelease = openApiClient.getLatestActiveRelease(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName);
        assertNotNull(secondRelease);

        Map<String, String> configs = secondRelease.getConfigurations();
        assertNotNull(configs);
        assertEquals("updated", configs.get("multi.key"),
                "Latest release should contain updated value");
    }

    @Test
    @Order(6)
    void testGetLatestActiveReleaseWithoutRelease() {
        createConfigItem(testAppId, testNamespaceName, "unreleased.key", "unreleased.value");

        OpenReleaseDTO release = openApiClient.getLatestActiveRelease(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName);
        assertNull(release, "Release should be null when no release has been published");
    }
}
