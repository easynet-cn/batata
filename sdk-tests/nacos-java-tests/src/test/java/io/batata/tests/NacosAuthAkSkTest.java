package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.exception.NacosException;
import org.junit.jupiter.api.*;

import java.util.Properties;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)

/**
 * Nacos AK/SK (accessKey/secretKey) authentication compatibility test.
 *
 * NOTE: Disabled by default. Batata currently implements token-based auth
 * (username/password -&gt; JWT access token) and does NOT implement the Nacos
 * ACM/SPAS-style AK/SK auth plugin driven by client properties
 * {@code accessKey}/{@code secretKey}. This test documents the coverage gap and
 * should be enabled once server-side AK/SK support lands.
 */
@Disabled("Batata uses token-based (username/password) auth; Nacos AK/SK (accessKey/secretKey) auth plugin not implemented")
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosAuthAkSkTest {

    private ConfigService configService;

    @BeforeAll
    void setup() throws NacosException {
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("accessKey", System.getProperty("nacos.username", "nacos"));
        properties.setProperty("secretKey", System.getProperty("nacos.password", "nacos"));
        configService = NacosFactory.createConfigService(properties);
    }

    @AfterAll
    void teardown() throws NacosException {
        if (configService != null) {
            configService.shutDown();
        }
    }

    /**
     * AKSK-001: Publish config using accessKey/secretKey authentication.
     */
    @Test
    @Order(1)
    void testPublishWithAkSk() throws NacosException {
        String dataId = "aksk001-" + UUID.randomUUID();
        String content = "aksk.key=aksk.value";
        boolean ok = configService.publishConfig(dataId, "DEFAULT_GROUP", content);
        assertTrue(ok, "Publish with AK/SK auth should succeed");
        configService.removeConfig(dataId, "DEFAULT_GROUP");
    }
}
