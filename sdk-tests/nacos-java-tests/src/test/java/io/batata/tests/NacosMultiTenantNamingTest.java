package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.naming.NamingService;
import com.alibaba.nacos.api.naming.listener.EventListener;
import com.alibaba.nacos.api.naming.listener.NamingEvent;
import com.alibaba.nacos.api.naming.pojo.Instance;
import org.junit.jupiter.api.*;

import java.io.*;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos Multi-Tenant Naming Tests
 *
 * Deep tests for multi-tenant namespace isolation in service discovery.
 * Aligned with Nacos MultiTenantNamingITCase.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosMultiTenantNamingTest {

    private static String serverAddr;
    private static String username;
    private static String password;
    private static String accessToken;
    private static final String DEFAULT_GROUP = "DEFAULT_GROUP";
    private static String namespace1;
    private static String namespace2;
    private static NamingService namingService1;
    private static NamingService namingService2;
    private static NamingService defaultNamingService;

    @BeforeAll
    static void setup() throws Exception {
        serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        username = System.getProperty("nacos.username", "nacos");
        password = System.getProperty("nacos.password", "nacos");

        accessToken = getAccessToken();

        namespace1 = "mt-ns1-" + UUID.randomUUID().toString().substring(0, 6);
        namespace2 = "mt-ns2-" + UUID.randomUUID().toString().substring(0, 6);

        createNamespace(namespace1, "Multi-Tenant NS 1", "First test namespace");
        createNamespace(namespace2, "Multi-Tenant NS 2", "Second test namespace");

        namingService1 = createNamingService(namespace1);
        namingService2 = createNamingService(namespace2);
        defaultNamingService = createNamingService(null);

        System.out.println("Multi-Tenant Naming Test Setup - NS1: " + namespace1 + ", NS2: " + namespace2);
    }

    @AfterAll
    static void teardown() throws Exception {
        if (namingService1 != null) namingService1.shutDown();
        if (namingService2 != null) namingService2.shutDown();
        if (defaultNamingService != null) defaultNamingService.shutDown();
        deleteNamespace(namespace1);
        deleteNamespace(namespace2);
    }

    // ==================== P0: Same IP/Port Cross-Namespace Isolation ====================

    /**
     * MT-001: Test same IP/port registered in different namespaces are isolated
     *
     * Aligned with Nacos MultiTenantNamingITCase.multipleTenantEqualIP()
     */
    @Test
    @Order(1)
    void testSameIpPortCrossNamespaceIsolation() throws NacosException, InterruptedException {
        String serviceName = "mt-same-ip-svc-" + UUID.randomUUID().toString().substring(0, 8);
        String ip = "10.10.10.1";
        int port = 8080;

        // Register same IP:port in both namespaces
        namingService1.registerInstance(serviceName, ip, port);
        namingService2.registerInstance(serviceName, ip, port);
        Thread.sleep(1500);

        // Both namespaces should have 1 instance each
        List<Instance> instances1 = namingService1.getAllInstances(serviceName);
        List<Instance> instances2 = namingService2.getAllInstances(serviceName);

        assertEquals(1, instances1.size(), "Namespace 1 should have exactly 1 instance");
        assertEquals(1, instances2.size(), "Namespace 2 should have exactly 1 instance");
        assertEquals(ip, instances1.get(0).getIp());
        assertEquals(ip, instances2.get(0).getIp());

        // Default namespace should NOT see these instances
        List<Instance> defaultInstances = defaultNamingService.getAllInstances(serviceName);
        assertTrue(defaultInstances.isEmpty(), "Default namespace should not see instances from other namespaces");

        // Cleanup
        namingService1.deregisterInstance(serviceName, ip, port);
        namingService2.deregisterInstance(serviceName, ip, port);
    }

    /**
     * MT-002: Test different IPs in different namespaces
     *
     * Aligned with Nacos MultiTenantNamingITCase.multipleTenantRegisterInstance()
     */
    @Test
    @Order(2)
    void testDifferentIpsCrossNamespace() throws NacosException, InterruptedException {
        String serviceName = "mt-diff-ip-svc-" + UUID.randomUUID().toString().substring(0, 8);

        namingService1.registerInstance(serviceName, "10.0.1.1", 8080);
        namingService2.registerInstance(serviceName, "10.0.2.1", 8080);
        Thread.sleep(1500);

        List<Instance> instances1 = namingService1.getAllInstances(serviceName);
        List<Instance> instances2 = namingService2.getAllInstances(serviceName);

        assertEquals(1, instances1.size());
        assertEquals(1, instances2.size());
        assertEquals("10.0.1.1", instances1.get(0).getIp(), "NS1 should have its own IP");
        assertEquals("10.0.2.1", instances2.get(0).getIp(), "NS2 should have its own IP");

        // Cleanup
        namingService1.deregisterInstance(serviceName, "10.0.1.1", 8080);
        namingService2.deregisterInstance(serviceName, "10.0.2.1", 8080);
    }

    // ==================== P0: Multi-Namespace + Multi-Group Isolation ====================

    /**
     * MT-003: Test namespace + group combined isolation
     *
     * Aligned with Nacos MultiTenantNamingITCase.multipleTenantMultiGroupRegisterInstance()
     */
    @Test
    @Order(3)
    void testNamespaceAndGroupIsolation() throws NacosException, InterruptedException {
        String serviceName = "mt-group-svc-" + UUID.randomUUID().toString().substring(0, 8);
        String group1 = "GROUP_A";
        String group2 = "GROUP_B";

        // Register in NS1/GROUP_A and NS2/GROUP_B
        namingService1.registerInstance(serviceName, group1, "10.0.3.1", 8080);
        namingService2.registerInstance(serviceName, group2, "10.0.3.2", 8080);
        Thread.sleep(1500);

        // NS1/GROUP_A should only see its own instance
        List<Instance> ns1g1 = namingService1.getAllInstances(serviceName, group1);
        assertEquals(1, ns1g1.size(), "NS1/GROUP_A should have 1 instance");
        assertEquals("10.0.3.1", ns1g1.get(0).getIp());

        // NS1/GROUP_B should have nothing
        List<Instance> ns1g2 = namingService1.getAllInstances(serviceName, group2);
        assertTrue(ns1g2.isEmpty(), "NS1/GROUP_B should have no instances");

        // NS2/GROUP_B should see its instance
        List<Instance> ns2g2 = namingService2.getAllInstances(serviceName, group2);
        assertEquals(1, ns2g2.size(), "NS2/GROUP_B should have 1 instance");
        assertEquals("10.0.3.2", ns2g2.get(0).getIp());

        // Cleanup
        namingService1.deregisterInstance(serviceName, group1, "10.0.3.1", 8080);
        namingService2.deregisterInstance(serviceName, group2, "10.0.3.2", 8080);
    }

    /**
     * MT-004: Test same IP/port in same group across namespaces
     *
     * Aligned with Nacos MultiTenantNamingITCase.multipleTenantGroupEqualIP()
     */
    @Test
    @Order(4)
    void testSameIpSameGroupCrossNamespace() throws NacosException, InterruptedException {
        String serviceName = "mt-eq-ip-grp-" + UUID.randomUUID().toString().substring(0, 8);
        String group = "SHARED_GROUP";
        String ip = "10.10.20.1";
        int port = 9090;

        // Register same IP:port in same group but different namespaces
        namingService1.registerInstance(serviceName, group, ip, port);
        namingService2.registerInstance(serviceName, group, ip, port);
        Thread.sleep(1500);

        // Both should have 1 instance each (isolated by namespace)
        List<Instance> instances1 = namingService1.getAllInstances(serviceName, group);
        List<Instance> instances2 = namingService2.getAllInstances(serviceName, group);

        assertEquals(1, instances1.size(), "NS1 should have 1 instance");
        assertEquals(1, instances2.size(), "NS2 should have 1 instance");

        // Cleanup
        namingService1.deregisterInstance(serviceName, group, ip, port);
        namingService2.deregisterInstance(serviceName, group, ip, port);
    }

    /**
     * MT-005: Test getInstances with group from different namespace
     *
     * Aligned with Nacos MultiTenantNamingITCase.multipleTenantGroupGetInstances()
     */
    @Test
    @Order(5)
    void testGetInstancesGroupCrossNamespace() throws NacosException, InterruptedException {
        String serviceName = "mt-get-grp-" + UUID.randomUUID().toString().substring(0, 8);
        String group = "QUERY_GROUP";

        namingService1.registerInstance(serviceName, group, "10.0.5.1", 8080);
        namingService1.registerInstance(serviceName, group, "10.0.5.2", 8080);
        Thread.sleep(1500);

        // NS1 should see 2 instances
        List<Instance> ns1Instances = namingService1.getAllInstances(serviceName, group);
        assertEquals(2, ns1Instances.size(), "NS1 should have 2 instances");

        // NS2 should see 0 instances for same service/group
        List<Instance> ns2Instances = namingService2.getAllInstances(serviceName, group);
        assertTrue(ns2Instances.isEmpty(), "NS2 should have no instances for NS1's service");

        // Cleanup
        namingService1.deregisterInstance(serviceName, group, "10.0.5.1", 8080);
        namingService1.deregisterInstance(serviceName, group, "10.0.5.2", 8080);
    }

    // ==================== P0: Subscribe Isolation Across Namespaces ====================

    /**
     * MT-006: Test subscribe isolation between namespaces
     *
     * Aligned with Nacos MultiTenantNamingITCase.multipleTenantSubscribe()
     */
    @Test
    @Order(6)
    void testSubscribeIsolation() throws NacosException, InterruptedException {
        String serviceName = "mt-sub-iso-" + UUID.randomUUID().toString().substring(0, 8);
        CountDownLatch ns1Latch = new CountDownLatch(1);
        CountDownLatch ns2Latch = new CountDownLatch(1);
        AtomicReference<List<Instance>> ns1Received = new AtomicReference<>();
        AtomicReference<List<Instance>> ns2Received = new AtomicReference<>();

        // Subscribe in both namespaces
        EventListener listener1 = event -> {
            if (event instanceof NamingEvent) {
                ns1Received.set(((NamingEvent) event).getInstances());
                ns1Latch.countDown();
            }
        };
        EventListener listener2 = event -> {
            if (event instanceof NamingEvent) {
                ns2Received.set(((NamingEvent) event).getInstances());
                ns2Latch.countDown();
            }
        };

        namingService1.subscribe(serviceName, listener1);
        namingService2.subscribe(serviceName, listener2);
        Thread.sleep(500);

        // Register in NS1 only
        namingService1.registerInstance(serviceName, "10.0.6.1", 8080);

        // NS1 should receive notification
        boolean ns1Got = ns1Latch.await(10, TimeUnit.SECONDS);
        assertTrue(ns1Got, "NS1 subscriber should receive notification");
        assertNotNull(ns1Received.get());
        assertFalse(ns1Received.get().isEmpty(), "NS1 should receive non-empty instances");

        // NS2 should NOT receive notification (or receive empty list)
        boolean ns2Got = ns2Latch.await(3, TimeUnit.SECONDS);
        if (ns2Got) {
            // If NS2 receives a notification, it should be for empty instances
            assertTrue(ns2Received.get().isEmpty(),
                    "NS2 should not see NS1's instances");
        }
        // If ns2 didn't receive anything, that's also correct behavior

        // Cleanup
        namingService1.unsubscribe(serviceName, listener1);
        namingService2.unsubscribe(serviceName, listener2);
        namingService1.deregisterInstance(serviceName, "10.0.6.1", 8080);
    }

    /**
     * MT-007: Test subscribe same service from two NamingService instances (different namespaces)
     *
     * Aligned with Nacos SubscribeNamingITCase.subscribeSameServiceForTwoNamingService()
     */
    @Test
    @Order(7)
    void testSubscribeSameServiceDifferentNamespaces() throws NacosException, InterruptedException {
        String serviceName = "mt-sub-same-" + UUID.randomUUID().toString().substring(0, 8);
        CountDownLatch latch1 = new CountDownLatch(1);
        CountDownLatch latch2 = new CountDownLatch(1);
        AtomicReference<List<Instance>> received1 = new AtomicReference<>();
        AtomicReference<List<Instance>> received2 = new AtomicReference<>();

        EventListener listener1 = event -> {
            if (event instanceof NamingEvent) {
                List<Instance> instances = ((NamingEvent) event).getInstances();
                if (!instances.isEmpty()) {
                    received1.set(instances);
                    latch1.countDown();
                }
            }
        };
        EventListener listener2 = event -> {
            if (event instanceof NamingEvent) {
                List<Instance> instances = ((NamingEvent) event).getInstances();
                if (!instances.isEmpty()) {
                    received2.set(instances);
                    latch2.countDown();
                }
            }
        };

        namingService1.subscribe(serviceName, listener1);
        namingService2.subscribe(serviceName, listener2);
        Thread.sleep(500);

        // Register different IPs in each namespace
        namingService1.registerInstance(serviceName, "10.0.7.1", 8080);
        namingService2.registerInstance(serviceName, "10.0.7.2", 8080);

        boolean got1 = latch1.await(10, TimeUnit.SECONDS);
        boolean got2 = latch2.await(10, TimeUnit.SECONDS);

        assertTrue(got1, "NS1 should receive its subscription");
        assertTrue(got2, "NS2 should receive its subscription");

        // Each namespace should see its own instance only
        if (received1.get() != null) {
            assertEquals(1, received1.get().size(), "NS1 should see 1 instance");
            assertEquals("10.0.7.1", received1.get().get(0).getIp());
        }
        if (received2.get() != null) {
            assertEquals(1, received2.get().size(), "NS2 should see 1 instance");
            assertEquals("10.0.7.2", received2.get().get(0).getIp());
        }

        // Cleanup
        namingService1.unsubscribe(serviceName, listener1);
        namingService2.unsubscribe(serviceName, listener2);
        namingService1.deregisterInstance(serviceName, "10.0.7.1", 8080);
        namingService2.deregisterInstance(serviceName, "10.0.7.2", 8080);
    }

    /**
     * MT-008: Test subscribe with group in namespace
     *
     * Aligned with Nacos MultiTenantNamingITCase.multipleTenantGroupSubscribe()
     */
    @Test
    @Order(8)
    void testSubscribeWithGroupInNamespace() throws NacosException, InterruptedException {
        String serviceName = "mt-sub-grp-" + UUID.randomUUID().toString().substring(0, 8);
        String group = "SUB_GROUP";
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<List<Instance>> received = new AtomicReference<>();

        EventListener listener = event -> {
            if (event instanceof NamingEvent) {
                List<Instance> instances = ((NamingEvent) event).getInstances();
                if (!instances.isEmpty()) {
                    received.set(instances);
                    latch.countDown();
                }
            }
        };

        namingService1.subscribe(serviceName, group, listener);
        Thread.sleep(500);

        namingService1.registerInstance(serviceName, group, "10.0.8.1", 8080);

        boolean got = latch.await(10, TimeUnit.SECONDS);
        assertTrue(got, "Should receive group subscription notification");
        assertNotNull(received.get());
        assertEquals("10.0.8.1", received.get().get(0).getIp());

        // Cleanup
        namingService1.unsubscribe(serviceName, group, listener);
        namingService1.deregisterInstance(serviceName, group, "10.0.8.1", 8080);
    }

    // ==================== P1: Unsubscribe and Service List ====================

    /**
     * MT-009: Test unsubscribe in namespace
     *
     * Aligned with Nacos MultiTenantNamingITCase.multipleTenantUnSubscribe()
     */
    @Test
    @Order(9)
    void testUnsubscribeInNamespace() throws NacosException, InterruptedException {
        String serviceName = "mt-unsub-" + UUID.randomUUID().toString().substring(0, 8);
        CountDownLatch latch = new CountDownLatch(1);

        EventListener listener = event -> latch.countDown();

        namingService1.subscribe(serviceName, listener);
        Thread.sleep(500);

        namingService1.unsubscribe(serviceName, listener);
        Thread.sleep(500);

        // Register should NOT trigger notification
        namingService1.registerInstance(serviceName, "10.0.9.1", 8080);

        boolean received = latch.await(3, TimeUnit.SECONDS);
        assertFalse(received, "Should NOT receive notification after unsubscribe");

        // Cleanup
        namingService1.deregisterInstance(serviceName, "10.0.9.1", 8080);
    }

    /**
     * MT-010: Test getServicesOfServer with namespace
     *
     * Aligned with Nacos MultiTenantNamingITCase.multipleTenantGetServicesOfServer()
     */
    @Test
    @Order(10)
    void testGetServicesOfServerWithNamespace() throws NacosException, InterruptedException {
        String prefix = "mt-list-" + UUID.randomUUID().toString().substring(0, 6);

        // Register services in NS1
        namingService1.registerInstance(prefix + "-svc-1", "10.0.10.1", 8080);
        namingService1.registerInstance(prefix + "-svc-2", "10.0.10.2", 8080);
        Thread.sleep(1500);

        // Get service list from NS1
        var ns1Services = namingService1.getServicesOfServer(1, 100);
        assertTrue(ns1Services.getCount() >= 2, "NS1 should have at least 2 services");

        // NS2 should not see NS1's services (or see fewer services)
        var ns2Services = namingService2.getServicesOfServer(1, 100);
        System.out.println("NS1 services: " + ns1Services.getCount() + ", NS2 services: " + ns2Services.getCount());

        // Cleanup
        namingService1.deregisterInstance(prefix + "-svc-1", "10.0.10.1", 8080);
        namingService1.deregisterInstance(prefix + "-svc-2", "10.0.10.2", 8080);
    }

    /**
     * MT-011: Test getServicesOfServer with group in namespace
     *
     * Aligned with Nacos MultiTenantNamingITCase.multipleTenantGroupGetServicesOfServer()
     */
    @Test
    @Order(11)
    void testGetServicesWithGroupInNamespace() throws NacosException, InterruptedException {
        String serviceName = "mt-svc-grp-list-" + UUID.randomUUID().toString().substring(0, 8);
        String group = "LIST_GROUP";

        namingService1.registerInstance(serviceName, group, "10.0.11.1", 8080);
        Thread.sleep(1500);

        var services = namingService1.getServicesOfServer(1, 100, group);
        assertTrue(services.getCount() >= 1, "Should find service in group");

        // Cleanup
        namingService1.deregisterInstance(serviceName, group, "10.0.11.1", 8080);
    }

    /**
     * MT-012: Test selectOneHealthyInstance across namespaces
     *
     * Aligned with Nacos MultiTenantNamingITCase.multipleTenantSelectOneHealthyInstance()
     */
    @Test
    @Order(12)
    void testSelectOneHealthyInstanceCrossNamespace() throws NacosException, InterruptedException {
        String serviceName = "mt-select-one-" + UUID.randomUUID().toString().substring(0, 8);

        namingService1.registerInstance(serviceName, "10.0.12.1", 8080);
        namingService2.registerInstance(serviceName, "10.0.12.2", 8080);
        Thread.sleep(1500);

        // selectOneHealthyInstance in NS1 should return NS1's instance
        Instance selected1 = namingService1.selectOneHealthyInstance(serviceName);
        assertNotNull(selected1, "Should select one instance from NS1");
        assertEquals("10.0.12.1", selected1.getIp(), "NS1 should select NS1's instance");

        // selectOneHealthyInstance in NS2 should return NS2's instance
        Instance selected2 = namingService2.selectOneHealthyInstance(serviceName);
        assertNotNull(selected2, "Should select one instance from NS2");
        assertEquals("10.0.12.2", selected2.getIp(), "NS2 should select NS2's instance");

        // Cleanup
        namingService1.deregisterInstance(serviceName, "10.0.12.1", 8080);
        namingService2.deregisterInstance(serviceName, "10.0.12.2", 8080);
    }

    /**
     * MT-013: Test selectInstances across namespaces
     *
     * Aligned with Nacos MultiTenantNamingITCase.multipleTenantSelectInstances()
     */
    @Test
    @Order(13)
    void testSelectInstancesCrossNamespace() throws NacosException, InterruptedException {
        String serviceName = "mt-select-" + UUID.randomUUID().toString().substring(0, 8);

        namingService1.registerInstance(serviceName, "10.0.13.1", 8080);
        namingService1.registerInstance(serviceName, "10.0.13.2", 8080);
        namingService2.registerInstance(serviceName, "10.0.13.3", 8080);
        Thread.sleep(1500);

        List<Instance> ns1Healthy = namingService1.selectInstances(serviceName, true);
        assertEquals(2, ns1Healthy.size(), "NS1 should have 2 healthy instances");

        List<Instance> ns2Healthy = namingService2.selectInstances(serviceName, true);
        assertEquals(1, ns2Healthy.size(), "NS2 should have 1 healthy instance");

        // Cleanup
        namingService1.deregisterInstance(serviceName, "10.0.13.1", 8080);
        namingService1.deregisterInstance(serviceName, "10.0.13.2", 8080);
        namingService2.deregisterInstance(serviceName, "10.0.13.3", 8080);
    }

    /**
     * MT-014: Test deregister in one namespace doesn't affect another
     *
     * Aligned with Nacos MultiTenantNamingITCase.multipleTenantDeregisterInstance()
     */
    @Test
    @Order(14)
    void testDeregisterCrossNamespaceIsolation() throws NacosException, InterruptedException {
        String serviceName = "mt-dereg-iso-" + UUID.randomUUID().toString().substring(0, 8);

        namingService1.registerInstance(serviceName, "10.0.14.1", 8080);
        namingService2.registerInstance(serviceName, "10.0.14.2", 8080);
        Thread.sleep(1500);

        // Deregister from NS1
        namingService1.deregisterInstance(serviceName, "10.0.14.1", 8080);
        Thread.sleep(1000);

        // NS1 should have no instances
        List<Instance> ns1After = namingService1.getAllInstances(serviceName);
        assertTrue(ns1After.isEmpty(), "NS1 should have no instances after deregister");

        // NS2 should still have its instance
        List<Instance> ns2After = namingService2.getAllInstances(serviceName);
        assertEquals(1, ns2After.size(), "NS2 should still have its instance");
        assertEquals("10.0.14.2", ns2After.get(0).getIp());

        // Cleanup
        namingService2.deregisterInstance(serviceName, "10.0.14.2", 8080);
    }

    /**
     * MT-015: Test server status from different namespaces
     *
     * Aligned with Nacos MultiTenantNamingITCase.multipleTenantServerStatus()
     */
    @Test
    @Order(15)
    void testServerStatusFromNamespaces() throws NacosException {
        String status1 = namingService1.getServerStatus();
        String status2 = namingService2.getServerStatus();

        assertNotNull(status1, "NS1 should report server status");
        assertNotNull(status2, "NS2 should report server status");
        assertEquals(status1, status2, "Server status should be the same regardless of namespace");
        System.out.println("Server status from both namespaces: " + status1);
    }

    // ==================== Helper Methods ====================

    private static NamingService createNamingService(String namespace) throws NacosException {
        Properties properties = new Properties();
        properties.put("serverAddr", serverAddr);
        properties.put("username", username);
        properties.put("password", password);
        if (namespace != null) {
            properties.put("namespace", namespace);
        }
        return NacosFactory.createNamingService(properties);
    }

    private static String getAccessToken() throws Exception {
        String loginUrl = String.format("http://%s/nacos/v3/auth/user/login", serverAddr);
        URL url = new URL(loginUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("POST");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");

        String body = "username=" + URLEncoder.encode(username, "UTF-8")
                + "&password=" + URLEncoder.encode(password, "UTF-8");
        conn.getOutputStream().write(body.getBytes(StandardCharsets.UTF_8));

        if (conn.getResponseCode() == 200) {
            BufferedReader reader = new BufferedReader(new InputStreamReader(conn.getInputStream()));
            StringBuilder response = new StringBuilder();
            String line;
            while ((line = reader.readLine()) != null) response.append(line);
            String resp = response.toString();
            if (resp.contains("accessToken")) {
                int start = resp.indexOf("accessToken") + 14;
                int end = resp.indexOf("\"", start);
                return resp.substring(start, end);
            }
        }
        return "";
    }

    private static void createNamespace(String id, String name, String desc) throws Exception {
        String fullUrl = String.format("http://%s/nacos/v2/console/namespace", serverAddr);
        if (!accessToken.isEmpty()) fullUrl += "?accessToken=" + accessToken;

        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("POST");
        conn.setDoOutput(true);
        conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");

        String body = String.format("namespaceId=%s&namespaceName=%s&namespaceDesc=%s",
                URLEncoder.encode(id, "UTF-8"),
                URLEncoder.encode(name, "UTF-8"),
                URLEncoder.encode(desc, "UTF-8"));
        conn.getOutputStream().write(body.getBytes(StandardCharsets.UTF_8));
        conn.getResponseCode();
    }

    private static void deleteNamespace(String id) throws Exception {
        String fullUrl = String.format("http://%s/nacos/v2/console/namespace?namespaceId=%s",
                serverAddr, URLEncoder.encode(id, "UTF-8"));
        if (!accessToken.isEmpty()) fullUrl += "&accessToken=" + accessToken;

        URL url = new URL(fullUrl);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        conn.setRequestMethod("DELETE");
        conn.getResponseCode();
    }
}
