package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.Assumptions;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Disabled;
import org.junit.jupiter.api.Test;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.Properties;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Protocol-only check for cipher-prefix config storage.
 *
 * This test deliberately reads the raw HTTP endpoint rather than going through
 * the SDK: it asserts how the SERVER stores cipher content, which the SDK's
 * transparent decryption hides. It therefore belongs in the protocol suite,
 * not in the Nacos Java SDK suite. It remains disabled because batata uses
 * server-side encryption and the HTTP API also returns decrypted content.
 */
public class NacosConfigEncryptionProtocolTest {

    private static ConfigService configService;
    private static String serverAddr;
    private static String accessToken;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");
        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);
        configService = NacosFactory.createConfigService(properties);
        accessToken = obtainAccessToken(serverAddr, username, password);
    }

    @AfterAll
    static void teardown() throws Exception {
        if (configService != null) {
            configService.shutDown();
        }
    }

    private static String obtainAccessToken(String addr, String username, String password) {
        try {
            HttpRequest request = HttpRequest.newBuilder()
                    .uri(URI.create("http://" + addr + "/nacos/v3/auth/user/login"))
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .POST(HttpRequest.BodyPublishers.ofString(
                            "username=" + username + "&password=" + password))
                    .build();
            HttpResponse<String> response = HttpClient.newHttpClient()
                    .send(request, HttpResponse.BodyHandlers.ofString());
            String body = response.body();
            int idx = body.indexOf("accessToken");
            if (idx < 0) {
                return null;
            }
            int start = body.indexOf("\"", idx) + 1;
            int end = body.indexOf("\"", start);
            return body.substring(start, end);
        } catch (Exception e) {
            return null;
        }
    }

    private String getRawConfigFromServer(String dataId, String group) {
        try {
            HttpRequest request = HttpRequest.newBuilder()
                    .uri(URI.create("http://" + serverAddr + "/nacos/v2/cs/config?dataId=" + dataId
                            + "&group=" + group + "&accessToken=" + accessToken))
                    .GET()
                    .build();
            return HttpClient.newHttpClient().send(request, HttpResponse.BodyHandlers.ofString()).body();
        } catch (Exception e) {
            return null;
        }
    }

    @Test
    @Disabled("Batata encrypts on store and decrypts on read for both gRPC and HTTP; "
            + "this protocol-only assertion expects the upstream client-side encryption model.")
    void testCipherConfigStoredEncrypted() throws Exception {
        Assumptions.assumeTrue(accessToken != null && !accessToken.isEmpty(),
                "Access token required for raw HTTP API access");
        String dataId = "cipher-enc003-" + UUID.randomUUID().toString().substring(0, 8);
        String plainContent = "encryption.secret=my_very_secret_value";

        assertTrue(configService.publishConfig(dataId, DEFAULT_GROUP, plainContent));
        TestSupport.waitForConfigPresent(configService, dataId, DEFAULT_GROUP);

        String rawResponse = getRawConfigFromServer(dataId, DEFAULT_GROUP);
        assertNotNull(rawResponse);
        assertFalse(rawResponse.contains(plainContent),
                "Raw server response should NOT contain plain text for cipher- config");
        assertEquals(plainContent, configService.getConfig(dataId, DEFAULT_GROUP, 5000));
        configService.removeConfig(dataId, DEFAULT_GROUP);
    }
}
