package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigService;

import org.junit.jupiter.api.Test;

import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * A1 - real long polling / change notification.
 *
 * <p>A runtime Apollo client subscribes to a namespace. After the config is
 * changed and re-published through the OpenAPI, the client must observe the
 * new value within its polling cycle (driven by /notifications/v2).
 */
public class ApolloConfigPullTest extends ApolloTestBase {

    @Test
    void testLongPollChangeNotification() throws Exception {
        String appId = createTestApp();
        String namespace = createTestNamespace(appId);
        createConfigItem(appId, namespace, "lp.key", "v1");
        releaseNamespace(appId, namespace);

        Config config = ConfigService.getConfig(appId, namespace);
        assertEquals("v1", config.getProperty("lp.key", null));

        CountDownLatch latch = new CountDownLatch(1);
        config.addChangeListener(changeEvent -> {
            if ("v2".equals(config.getProperty("lp.key", null))) {
                latch.countDown();
            }
        });

        updateConfigItem(appId, namespace, "lp.key", "v2");
        releaseNamespace(appId, namespace);

        assertTrue(latch.await(30, TimeUnit.SECONDS),
                "client should receive the config change via long polling");
        assertEquals("v2", config.getProperty("lp.key", null));
    }
}
