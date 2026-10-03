package io.batata.tests;

import com.ctrip.framework.apollo.openapi.client.ApolloOpenApiClient;
import com.ctrip.framework.apollo.openapi.dto.OpenNamespaceLockDTO;
import org.junit.jupiter.api.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Namespace lock query via the Apollo OpenAPI client.
 *
 * <p>The OpenAPI client only exposes {@code getNamespaceLock}; the lock itself is
 * acquired through the admin endpoint (exercised via ApolloIntegrationHelper).
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class ApolloNamespaceLockTest extends ApolloTestBase {

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
    void testGetNamespaceLockUnlocked() {
        OpenNamespaceLockDTO lock = openApiClient.getNamespaceLock(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName);
        assertNull(lock, "A freshly created namespace should not be locked");
    }

    @Test
    @Order(2)
    void testGetNamespaceLockAfterLock() {
        ApolloIntegrationHelper.lockNamespace(testAppId, DEFAULT_CLUSTER, testNamespaceName, OPERATOR);

        OpenNamespaceLockDTO lock = openApiClient.getNamespaceLock(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName);
        assertNotNull(lock, "Namespace should be locked after lock operation");
        assertEquals(testNamespaceName, lock.getNamespaceName(), "Namespace name should match");
        assertTrue(lock.isLocked(), "Lock flag should be true");
    }
}
