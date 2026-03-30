package io.batata.tests;

import com.alibaba.nacos.api.NacosFactory;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.lock.LockService;
import com.alibaba.nacos.api.lock.model.LockInstance;
import org.junit.jupiter.api.*;

import java.util.Properties;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos 3.x Lock Service SDK Compatibility Tests
 *
 * Tests Batata's compatibility with the Nacos LockService API introduced in Nacos 3.x.
 * LockService provides distributed locking via gRPC with key-based lock instances
 * that support TTL (expiredTime).
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosLockServiceTest {

    private static LockService lockService;
    private static LockService lockService2;
    private static final String LOCK_TYPE = "nacosMutexLock";

    @BeforeAll
    static void setup() throws NacosException {
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);

        lockService = NacosFactory.createLockService(properties);
        assertNotNull(lockService, "LockService should be created successfully");

        // Create a second lock service to simulate another client
        Properties properties2 = new Properties();
        properties2.put("serverAddr", serverAddr);
        properties2.put("username", username);
        properties2.put("password", password);
        lockService2 = NacosFactory.createLockService(properties2);
        assertNotNull(lockService2, "Second LockService should be created successfully");
    }

    @AfterAll
    static void teardown() throws NacosException {
        if (lockService != null) {
            lockService.shutdown();
        }
        if (lockService2 != null) {
            lockService2.shutdown();
        }
    }

    // ==================== P1: Important Tests ====================

    /**
     * LOCK-001: Acquire lock should succeed
     *
     * Create a LockInstance with a unique key and TTL, then call lock().
     * The lock acquisition should return true.
     */
    @Test
    @Order(1)
    void testAcquireLock() throws NacosException {
        String lockKey = "lock001-acquire-" + UUID.randomUUID();
        LockInstance lockInstance = new LockInstance(lockKey, 30000L, LOCK_TYPE);

        Boolean acquired = lockService.lock(lockInstance);
        assertTrue(acquired, "Lock acquisition should succeed");

        // Release the lock
        Boolean released = lockService.unLock(lockInstance);
        assertTrue(released, "Lock release after acquisition should succeed");
    }

    /**
     * LOCK-002: Release lock should succeed
     *
     * Acquire a lock and then release it. Both operations should succeed.
     * After release, the lock should be available for others.
     */
    @Test
    @Order(2)
    void testReleaseLock() throws NacosException {
        String lockKey = "lock002-release-" + UUID.randomUUID();
        LockInstance lockInstance = new LockInstance(lockKey, 30000L, LOCK_TYPE);

        // Acquire
        Boolean acquired = lockService.lock(lockInstance);
        assertTrue(acquired, "Lock acquisition should succeed");

        // Release
        Boolean released = lockService.unLock(lockInstance);
        assertTrue(released, "Lock release should succeed");

        // After release, another lock attempt on same key should succeed
        LockInstance lockInstance2 = new LockInstance(lockKey, 30000L, LOCK_TYPE);
        Boolean reacquired = lockService2.lock(lockInstance2);
        assertTrue(reacquired, "Lock should be acquirable after release");

        // Cleanup
        lockService2.unLock(lockInstance2);
    }

    /**
     * LOCK-003: Lock contention - second acquire should fail
     *
     * Client 1 holds a lock. Client 2 attempts to acquire the same lock key.
     * Client 2 should fail (return false) because the lock is already held.
     */
    @Test
    @Order(3)
    void testLockContention() throws NacosException {
        String lockKey = "lock003-contention-" + UUID.randomUUID();
        LockInstance lockInstance1 = new LockInstance(lockKey, 30000L, LOCK_TYPE);
        LockInstance lockInstance2 = new LockInstance(lockKey, 30000L, LOCK_TYPE);

        // Client 1 acquires the lock
        Boolean acquired1 = lockService.lock(lockInstance1);
        assertTrue(acquired1, "First client should acquire the lock");

        // Client 2 tries to acquire the same lock - should fail
        Boolean acquired2 = lockService2.lock(lockInstance2);
        assertFalse(acquired2, "Second client should fail to acquire held lock");

        // Client 1 releases
        lockService.unLock(lockInstance1);

        // Now client 2 should be able to acquire
        Boolean acquired2Retry = lockService2.lock(lockInstance2);
        assertTrue(acquired2Retry, "Second client should acquire lock after first releases");

        // Cleanup
        lockService2.unLock(lockInstance2);
    }

    /**
     * LOCK-004: Lock with TTL - should auto-release after timeout
     *
     * Acquire a lock with a short TTL (expiredTime). Wait for the TTL to expire.
     * After expiration, another client should be able to acquire the same lock.
     */
    @Test
    @Order(4)
    void testLockWithTtl() throws NacosException, InterruptedException {
        String lockKey = "lock004-ttl-" + UUID.randomUUID();
        // Short TTL: 3 seconds
        long ttlMs = 3000L;
        LockInstance lockInstance1 = new LockInstance(lockKey, ttlMs, LOCK_TYPE);

        // Client 1 acquires with short TTL
        Boolean acquired1 = lockService.lock(lockInstance1);
        assertTrue(acquired1, "Lock with TTL should be acquired");

        // Immediately, client 2 should fail
        LockInstance lockInstance2 = new LockInstance(lockKey, 30000L, LOCK_TYPE);
        Boolean acquired2Early = lockService2.lock(lockInstance2);
        assertFalse(acquired2Early, "Second client should fail while lock is held");

        // Wait for TTL to expire plus buffer
        Thread.sleep(ttlMs + 2000);

        // After TTL expiry, client 2 should succeed
        LockInstance lockInstance2Retry = new LockInstance(lockKey, 30000L, LOCK_TYPE);
        Boolean acquired2AfterTtl = lockService2.lock(lockInstance2Retry);
        assertTrue(acquired2AfterTtl, "Second client should acquire lock after TTL expiry");

        // Cleanup
        lockService2.unLock(lockInstance2Retry);
    }

    /**
     * LOCK-005: Concurrent lock attempts from different clients - only one connection should win
     *
     * Two separate lock service clients (different gRPC connections, different owners)
     * attempt to lock the same key simultaneously. Only one connection should win.
     * Note: Threads sharing the same LockService share the same gRPC connectionId,
     * so reentrant locking would allow them all to succeed. We use exactly 2 threads
     * with different clients to test true contention.
     */
    @Test
    @Order(5)
    void testConcurrentLockAttempts() throws InterruptedException {
        String lockKey = "lock005-concurrent-" + UUID.randomUUID();
        int threadCount = 2;

        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch doneLatch = new CountDownLatch(threadCount);
        AtomicInteger successCount = new AtomicInteger(0);
        AtomicInteger failCount = new AtomicInteger(0);

        // Thread 0 uses lockService, Thread 1 uses lockService2 (different connections)
        LockService[] services = {lockService, lockService2};
        for (int i = 0; i < threadCount; i++) {
            final int index = i;
            new Thread(() -> {
                try {
                    startLatch.await();
                    LockInstance lockInstance = new LockInstance(lockKey, 30000L, LOCK_TYPE);
                    Boolean result = services[index].lock(lockInstance);
                    if (Boolean.TRUE.equals(result)) {
                        successCount.incrementAndGet();
                    } else {
                        failCount.incrementAndGet();
                    }
                } catch (Exception e) {
                    System.err.println("Thread " + index + " error: " + e.getMessage());
                    failCount.incrementAndGet();
                } finally {
                    doneLatch.countDown();
                }
            }).start();
        }

        startLatch.countDown();
        boolean completed = doneLatch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete within timeout");

        assertEquals(1, successCount.get(),
                "Exactly one concurrent lock attempt should succeed, got: " + successCount.get());
        assertEquals(1, failCount.get(),
                "The other should fail, got failures: " + failCount.get());

        // Cleanup - try to unlock with both services
        try {
            lockService.unLock(new LockInstance(lockKey, 30000L, LOCK_TYPE));
        } catch (Exception ignored) {
        }
        try {
            lockService2.unLock(new LockInstance(lockKey, 30000L, LOCK_TYPE));
        } catch (Exception ignored) {
        }
    }

    /**
     * LOCK-006: Different lock keys should be independent
     *
     * Acquiring a lock on key A should not prevent acquiring a lock on key B.
     */
    @Test
    @Order(6)
    void testDifferentKeysIndependent() throws NacosException {
        String lockKeyA = "lock006-keyA-" + UUID.randomUUID();
        String lockKeyB = "lock006-keyB-" + UUID.randomUUID();

        LockInstance lockA = new LockInstance(lockKeyA, 30000L, LOCK_TYPE);
        LockInstance lockB = new LockInstance(lockKeyB, 30000L, LOCK_TYPE);

        // Acquire lock A with client 1
        Boolean acquiredA = lockService.lock(lockA);
        assertTrue(acquiredA, "Lock A should be acquired");

        // Acquire lock B with client 2 - should succeed (different key)
        Boolean acquiredB = lockService2.lock(lockB);
        assertTrue(acquiredB, "Lock B should be acquired independently of lock A");

        // Cleanup
        lockService.unLock(lockA);
        lockService2.unLock(lockB);
    }

    /**
     * LOCK-007: Lock and unlock idempotency
     *
     * Unlocking an already-unlocked lock should not cause an error.
     * Locking an already-held lock by the same client should be handled gracefully.
     */
    @Test
    @Order(7)
    void testLockUnlockIdempotency() throws NacosException {
        String lockKey = "lock007-idempotent-" + UUID.randomUUID();
        LockInstance lockInstance = new LockInstance(lockKey, 30000L, LOCK_TYPE);

        // Acquire
        Boolean acquired = lockService.lock(lockInstance);
        assertTrue(acquired, "Initial lock should succeed");

        // Release
        Boolean released = lockService.unLock(lockInstance);
        assertTrue(released, "First unlock should succeed");

        // Double release should not throw (may return true or false depending on implementation)
        assertDoesNotThrow(() -> {
            lockService.unLock(lockInstance);
        }, "Double unlock should not throw an exception");
    }

    /**
     * LOCK-008: Lock reentry - same client acquires the same lock twice
     *
     * Depending on Nacos implementation, re-acquiring the same lock from the same client
     * may succeed (reentrant) or fail (non-reentrant). This test documents the behavior.
     */
    @Test
    @Order(8)
    void testLockReentry() throws NacosException {
        String lockKey = "lock008-reentry-" + UUID.randomUUID();
        LockInstance lockInstance1 = new LockInstance(lockKey, 30000L, LOCK_TYPE);

        // First acquire
        Boolean acquired1 = lockService.lock(lockInstance1);
        assertTrue(acquired1, "First lock acquisition should succeed");

        // Second acquire from same service (reentrant attempt)
        LockInstance lockInstance2 = new LockInstance(lockKey, 30000L, LOCK_TYPE);
        try {
            Boolean acquired2 = lockService.lock(lockInstance2);
            // Document whether reentrant locking is supported
            System.out.println("Lock reentry result: " + acquired2 +
                    " (true = reentrant supported, false = non-reentrant)");
            // Either outcome is acceptable - we just verify no crash
            assertNotNull(acquired2, "Reentrant lock attempt should return a result");
        } finally {
            // Cleanup - release the lock
            lockService.unLock(lockInstance1);
        }
    }
}
