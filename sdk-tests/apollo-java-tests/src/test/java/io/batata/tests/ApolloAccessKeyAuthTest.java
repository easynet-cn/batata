package io.batata.tests;

import org.junit.jupiter.api.Test;

import java.util.HashMap;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * A3 - AccessKey signature authentication.
 *
 * <p>Client-facing config endpoints must accept a valid
 * {@code Authorization: Apollo <appId>:<signature>} header and reject an
 * invalid signature with 401.
 */
public class ApolloAccessKeyAuthTest extends ApolloTestBase {

    @Test
    void testSignedConfigFetch() throws Exception {
        String appId = createTestApp();
        String namespace = createTestNamespace(appId);
        createConfigItem(appId, namespace, "ak.key", "ak.value");
        releaseNamespace(appId, namespace);

        String secret = ApolloIntegrationHelper.createAccessKey(appId);
        assertNotNull(secret, "access key secret should be returned");

        String path = "/configs/" + appId + "/default/" + namespace;
        long ts = System.currentTimeMillis();
        String sig = ApolloIntegrationHelper.sign(secret, ts, path);

        Map<String, String> headers = new HashMap<>();
        headers.put("Authorization", "Apollo " + appId + ":" + sig);
        headers.put("Timestamp", String.valueOf(ts));

        ApolloIntegrationHelper.HttpResult ok = ApolloIntegrationHelper.request("GET", path, null, headers);
        assertEquals(200, ok.status, "valid signature should be accepted");
        assertTrue(ok.body.contains("ak.value"), "signed response should contain the config value");

        // invalid signature -> 401
        long ts2 = System.currentTimeMillis();
        String badSig = ApolloIntegrationHelper.sign("definitely-wrong-secret", ts2, path);
        Map<String, String> badHeaders = new HashMap<>();
        badHeaders.put("Authorization", "Apollo " + appId + ":" + badSig);
        badHeaders.put("Timestamp", String.valueOf(ts2));

        ApolloIntegrationHelper.HttpResult bad =
                ApolloIntegrationHelper.request("GET", path, null, badHeaders);
        assertEquals(401, bad.status, "invalid signature should be rejected");
    }
}
