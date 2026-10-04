package io.batata.tests;

import com.alibaba.nacos.api.config.ConfigService;
import com.alibaba.nacos.api.config.model.ConfigDetailInfo;
import com.alibaba.nacos.api.config.model.ConfigGrayInfo;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.maintainer.client.config.ConfigMaintainerService;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.pojo.Instance;
import com.alibaba.nacos.api.naming.pojo.ServiceInfo;

import java.util.List;
import java.util.function.Supplier;

/**
 * Shared polling helpers for the SDK compatibility test-suite.
 *
 * Replaces fixed {@code Thread.sleep(...)} calls with condition-based polling so
 * tests become both faster (they stop as soon as the condition is met) and more
 * stable on slow CI machines (they no longer fail because a fixed delay was too
 * short). Every helper has a bounded timeout and simply returns when the
 * condition is satisfied or the deadline elapses; the assertion that follows is
 * what actually fails the test with a clear message.
 */
public final class TestSupport {

    /** Default upper bound for polling loops. */
    public static final long DEFAULT_TIMEOUT_MS = 10_000;
    private static final long POLL_INTERVAL_MS = 200;

    private TestSupport() {
    }

    /**
     * Poll {@code condition} until it returns true or {@code timeoutMillis} elapses.
     *
     * @return {@code true} if the condition became true within the timeout.
     */
    public static boolean waitFor(Supplier<Boolean> condition, long timeoutMillis) {
        long deadline = System.currentTimeMillis() + timeoutMillis;
        while (System.currentTimeMillis() < deadline) {
            if (condition.get()) {
                return true;
            }
            try {
                Thread.sleep(POLL_INTERVAL_MS);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                break;
            }
        }
        return condition.get();
    }

    /**
     * Poll until the naming service reports exactly {@code expected} instances
     * for {@code serviceName}, or the default timeout elapses. Waiting for an
     * exact count works for both registration (0 -&gt; N) and deregistration
     * (N -&gt; M) scenarios.
     *
     * @return the last observed instance list (may differ from expected if the
     *         timeout elapsed; the caller's assertion reports the failure).
     */
    public static List<Instance> waitForInstances(NamingService namingService,
                                                  String serviceName,
                                                  String group,
                                                  int expected) throws NacosException {
        long deadline = System.currentTimeMillis() + DEFAULT_TIMEOUT_MS;
        List<Instance> instances = namingService.getAllInstances(serviceName, group);
        while (instances.size() != expected && System.currentTimeMillis() < deadline) {
            try {
                Thread.sleep(POLL_INTERVAL_MS);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                break;
            }
            instances = namingService.getAllInstances(serviceName, group);
        }
        return instances;
    }

    /**
     * Poll until the naming service reports an active subscription for
     * {@code serviceName} in {@code group}, or the default timeout elapses.
     * Used to replace the fixed "wait for the subscription to be registered"
     * sleeps that precede an instance registration in subscribe tests.
     */
    public static boolean waitForSubscribed(NamingService namingService,
                                            String serviceName,
                                            String group) {
        return waitFor(() -> {
            try {
                return namingService.getSubscribeServices().stream()
                        .anyMatch(s -> serviceName.equals(s.getName())
                                && group.equals(s.getGroupName()));
            } catch (Exception e) {
                return false;
            }
        }, DEFAULT_TIMEOUT_MS);
    }

    /**
     * Poll until the naming service no longer reports a subscription for
     * {@code serviceName} in {@code group}, or the default timeout elapses.
     * Used to replace the fixed "wait for the unsubscribe to propagate" sleeps.
     */
    public static boolean waitForUnsubscribed(NamingService namingService,
                                              String serviceName,
                                              String group) {
        return waitFor(() -> {
            try {
                return namingService.getSubscribeServices().stream()
                        .noneMatch(s -> serviceName.equals(s.getName())
                                && group.equals(s.getGroupName()));
            } catch (Exception e) {
                return false;
            }
        }, DEFAULT_TIMEOUT_MS);
    }

    /**
     * Poll until {@code getConfig(dataId, group)} returns a non-null value, or
     * the default timeout elapses. Replaces the fixed "wait for the config to be
     * published before attaching a listener" sleeps in config tests.
     */
    public static boolean waitForConfigPresent(ConfigService configService,
                                               String dataId,
                                               String group) {
        return waitFor(() -> {
            try {
                return configService.getConfig(dataId, group, 1000) != null;
            } catch (NacosException e) {
                return false;
            }
        }, DEFAULT_TIMEOUT_MS);
    }

    /**
     * Poll until {@code getConfig(dataId, group)} returns {@code null}, or the
     * default timeout elapses. Replaces the fixed "wait for the config deletion to
     * propagate" sleeps in batch delete tests.
     */
    public static boolean waitForConfigDeleted(ConfigService configService,
                                               String dataId,
                                               String group) {
        return waitFor(() -> {
            try {
                return configService.getConfig(dataId, group, 1000) == null;
            } catch (NacosException e) {
                return false;
            }
        }, DEFAULT_TIMEOUT_MS);
    }

    /**
     * Poll until {@code getConfig(dataId, group)} returns exactly {@code expected},
     * or the default timeout elapses. Replaces the fixed "wait for the updated
     * config content to propagate" sleeps in batch update tests.
     */
    public static boolean waitForConfigContent(ConfigService configService,
                                               String dataId,
                                               String group,
                                               String expected) {
        return waitFor(() -> {
            try {
                return expected.equals(configService.getConfig(dataId, group, 1000));
            } catch (NacosException e) {
                return false;
            }
        }, DEFAULT_TIMEOUT_MS);
    }

    // ==================== ConfigMaintainerService polling helpers ====================

    /**
     * Poll until the maintainer client reports a non-null, non-empty config for
     * {@code dataId}/{@code group}/{@code namespace}, or the default timeout
     * elapses. Replaces the fixed "wait for the config to persist" sleeps in
     * maintainer-based tests (beta, tag, history, etc.).
     */
    public static boolean waitForConfigPresent(ConfigMaintainerService maintainerService,
                                               String dataId,
                                               String group,
                                               String namespace) {
        return waitFor(() -> {
            try {
                ConfigDetailInfo info = maintainerService.getConfig(dataId, group, namespace);
                return info != null && info.getContent() != null;
            } catch (Exception e) {
                return false;
            }
        }, DEFAULT_TIMEOUT_MS);
    }

    /**
     * Poll until the maintainer client reports exactly {@code expected} content
     * for {@code dataId}/{@code group}/{@code namespace}, or the default timeout
     * elapses.
     */
    public static boolean waitForConfigContent(ConfigMaintainerService maintainerService,
                                               String dataId,
                                               String group,
                                               String namespace,
                                               String expected) {
        return waitFor(() -> {
            try {
                ConfigDetailInfo info = maintainerService.getConfig(dataId, group, namespace);
                return info != null && expected.equals(info.getContent());
            } catch (Exception e) {
                return false;
            }
        }, DEFAULT_TIMEOUT_MS);
    }

    /**
     * Poll until the maintainer client reports a beta (gray) config with non-null
     * content for {@code dataId}/{@code group}/{@code namespace}, or the default
     * timeout elapses. Replaces the fixed "wait for the beta config to be
     * queryable" sleeps.
     */
    public static boolean waitForBetaPresent(ConfigMaintainerService maintainerService,
                                             String dataId,
                                             String group,
                                             String namespace) {
        return waitFor(() -> {
            try {
                ConfigGrayInfo info = maintainerService.queryBeta(dataId, group, namespace);
                return info != null && info.getContent() != null;
            } catch (Exception e) {
                return false;
            }
        }, DEFAULT_TIMEOUT_MS);
    }

    /**
     * Poll until the maintainer client reports a beta (gray) config whose content
     * equals {@code expected}, or the default timeout elapses.
     */
    public static boolean waitForBetaContent(ConfigMaintainerService maintainerService,
                                             String dataId,
                                             String group,
                                             String namespace,
                                             String expected) {
        return waitFor(() -> {
            try {
                ConfigGrayInfo info = maintainerService.queryBeta(dataId, group, namespace);
                return info != null && expected.equals(info.getContent());
            } catch (Exception e) {
                return false;
            }
        }, DEFAULT_TIMEOUT_MS);
    }

    /**
     * Poll until the maintainer client reports no config (null or empty content)
     * for {@code dataId}/{@code group}/{@code namespace}, or the default timeout
     * elapses. Replaces the fixed "wait for the deletion to propagate" sleeps in
     * maintainer-based tests.
     */
    public static boolean waitForConfigDeleted(ConfigMaintainerService maintainerService,
                                               String dataId,
                                               String group,
                                               String namespace) {
        return waitFor(() -> {
            try {
                ConfigDetailInfo info = maintainerService.getConfig(dataId, group, namespace);
                return info == null || info.getContent() == null;
            } catch (Exception e) {
                return false;
            }
        }, DEFAULT_TIMEOUT_MS);
    }
}
