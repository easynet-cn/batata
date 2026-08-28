package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.openapi.client.ApolloOpenApiClient;

import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

/**
 * A4 - gray release by client label.
 *
 * <p>A client identified with {@code apollo.label=env=gray} must receive the
 * gray release configuration, while the non-labeled release continues to serve
 * the base value.
 */
public class ApolloGrayReleaseTest extends ApolloTestBase {

    @BeforeAll
    static void setup() {
        String openApiUrl = System.getProperty("apollo.openapi.url", "http://127.0.0.1:8080");
        String token = System.getProperty("apollo.openapi.token", "admin");
        openApiClient = ApolloOpenApiClient.newBuilder()
                .withPortalUrl(openApiUrl)
                .withToken(token)
                .build();
        assertNotNull(openApiClient);
    }

    @Test
    void testGrayReleaseByLabel() throws Exception {
        String appId = createTestApp();
        String namespace = createTestNamespace(appId);
        createConfigItem(appId, namespace, "gk", "base");
        releaseNamespace(appId, namespace);

        // Upstream flow: create the BRANCH first, author the gray edit ON the
        // branch namespace, publish the branch, then point rules at it.
        String branch = ApolloIntegrationHelper.createBranch(appId, namespace);
        assertNotNull(branch);
        ApolloIntegrationHelper.createItemOnCluster(appId, branch, namespace, "gk", "gray");

        // Upstream format: flat label tokens; blank clientAppId = wildcard.
        String rules = "[{\"clientIpList\":[\"*\"],\"clientLabelList\":[\"gray\"]}]";
        ApolloIntegrationHelper.createGrayRule(appId, namespace, branch, rules, 0);
        int grayReleaseId = ApolloIntegrationHelper.createGrayRelease(appId, namespace, branch);
        ApolloIntegrationHelper.updateGrayRule(appId, namespace, branch, rules, grayReleaseId);

        // the runtime client sends the label and should receive the gray value
        System.setProperty("apollo.label", "gray");
        Config grayConfig = ConfigService.getConfig(appId, namespace);
        assertEquals("gray", grayConfig.getProperty("gk", null),
                "client with matching label should receive the gray release");
    }
}
