package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.openapi.dto.OpenAppNamespaceDTO;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

/**
 * A2 - public namespace merge.
 *
 * <p>A namespace owned as a *public* namespace by another app must be served to
 * a requesting app that does not own a namespace of the same name.
 */
public class ApolloPublicNamespaceTest extends ApolloTestBase {

    @Test
    void testPublicNamespaceMerge() {
        String consumerApp = createTestApp();
        String ownerApp = createTestApp();
        String namespace = "shared-" + java.util.UUID.randomUUID().toString().substring(0, 8);

        OpenAppNamespaceDTO publicNs = new OpenAppNamespaceDTO();
        publicNs.setAppId(ownerApp);
        publicNs.setName(namespace);
        publicNs.setFormat("properties");
        publicNs.setPublic(true);
        publicNs.setDataChangeCreatedBy(OPERATOR);
        openApiClient.createAppNamespace(publicNs);

        createConfigItem(ownerApp, namespace, "pub.key", "pub.value");
        releaseNamespace(ownerApp, namespace);

        Config config = ConfigService.getConfig(consumerApp, namespace);
        assertNotNull(config, "config for public namespace should be available");
        assertEquals("pub.value", config.getProperty("pub.key", null),
                "consumer should receive the public app's configuration");
    }
}
