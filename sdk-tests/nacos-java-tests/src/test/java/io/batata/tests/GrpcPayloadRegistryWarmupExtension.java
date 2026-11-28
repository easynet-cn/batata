package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.config.ConfigService;
import org.junit.jupiter.api.extension.BeforeAllCallback;
import org.junit.jupiter.api.extension.ExtensionContext;

import java.util.Properties;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * Warms the Nacos SDK gRPC payload registry once per JVM.
 *
 * The raw ServerCheck/SetupAck protocol test used to initialize GrpcUtils as a
 * side effect; after moving it to the protocol suite, SDK-only runs began
 * seeing "Unknown payload type:ConfigBatchListenResponse" in listener threads
 * depending on class order. A normal SDK GetConfig call performs the same
 * initialization without any raw endpoint assertion.
 */
public class GrpcPayloadRegistryWarmupExtension implements BeforeAllCallback {

    private static final AtomicBoolean WARMED = new AtomicBoolean(false);

    @Override
    public void beforeAll(ExtensionContext context) throws Exception {
        if (!WARMED.compareAndSet(false, true)) {
            return;
        }
        Properties props = new Properties();
        props.setProperty("serverAddr", System.getProperty("nacos.server", "127.0.0.1:8848"));
        props.setProperty("username", System.getProperty("nacos.username", "nacos"));
        props.setProperty("password", System.getProperty("nacos.password", "nacos"));
        ConfigService client = NacosFactory.createConfigService(props);
        try {
            client.getConfig("batata-sdk-registry-warmup", "DEFAULT_GROUP", 1000);
        } finally {
            client.shutDown();
        }
    }
}
