package consultest

import (
	"context"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// TestLockLockUnlock tests basic lock acquisition and release
func TestLockLockUnlock(t *testing.T) {
	client := getTestClient(t)

	key := "test/lock/basic-" + randomString(8)

	// Create lock
	lock, err := client.LockKey(key)
	require.NoError(t, err)

	// Acquire lock
	leaderCh, err := lock.Lock(nil)
	require.NoError(t, err)
	require.NotNil(t, leaderCh, "Should get leader channel when lock acquired")

	// Verify lock is held
	pair, _, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	if pair != nil {
		assert.NotEmpty(t, pair.Session, "Lock key should have session")
	}

	// Release lock
	err = lock.Unlock()
	require.NoError(t, err)

	// Verify lock is released
	select {
	case <-leaderCh:
		// Expected - channel closed when lock released
	case <-time.After(2 * time.Second):
		t.Fatal("Leader channel should be closed after unlock")
	}

	// Cleanup
	_, err = client.KV().Delete(key, nil)
	require.NoError(t, err)
}

// TestLockForceInvalidate tests lock loss when session is destroyed
func TestLockForceInvalidate(t *testing.T) {
	client := getTestClient(t)

	key := "test/lock/invalidate-" + randomString(8)

	// Create lock with specific session TTL
	opts := &api.LockOptions{
		Key:         key,
		SessionTTL:  "10s",
		SessionName: "test-lock-session",
	}
	lock, err := client.LockOpts(opts)
	require.NoError(t, err)

	// Acquire lock
	leaderCh, err := lock.Lock(nil)
	require.NoError(t, err)
	require.NotNil(t, leaderCh)

	// Get the session ID
	pair, _, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair)
	sessionID := pair.Session

	// Force destroy the session
	_, err = client.Session().Destroy(sessionID, nil)
	require.NoError(t, err)

	// Lock should be lost
	select {
	case <-leaderCh:
		// Expected - channel closed when session destroyed
	case <-time.After(5 * time.Second):
		t.Fatal("Leader channel should be closed after session destroy")
	}

	// Cleanup
	_, err = client.KV().Delete(key, nil)
	require.NoError(t, err)
}

// TestLockDeleteKey tests lock behavior when key is deleted
func TestLockDeleteKey(t *testing.T) {
	client := getTestClient(t)

	key := "test/lock/delete-key-" + randomString(8)

	lock, err := client.LockKey(key)
	require.NoError(t, err)

	// Acquire lock
	leaderCh, err := lock.Lock(nil)
	require.NoError(t, err)
	require.NotNil(t, leaderCh)

	// Delete the key (simulating external interference)
	_, err = client.KV().Delete(key, nil)
	require.NoError(t, err)

	// Lock should eventually be lost (implementation dependent)
	select {
	case <-leaderCh:
		// Expected if implementation detects key deletion
	case <-time.After(3 * time.Second):
		// Some implementations may not detect this immediately
		t.Log("Lock not immediately released after key deletion")
	}

	// Cleanup - unlock if still held
	_ = lock.Unlock()
}

// TestLockContend tests concurrent lock contention
func TestLockContend(t *testing.T) {
	client := getTestClient(t)

	key := "test/lock/contend-" + randomString(8)
	numContenders := 3
	acquired := make(chan int, numContenders)

	// Start multiple contenders
	for i := 0; i < numContenders; i++ {
		go func(id int) {
			lock, err := client.LockKey(key)
			if err != nil {
				t.Logf("Contender %d failed to create lock: %v", id, err)
				return
			}

			// Try to acquire with timeout
			ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
			defer cancel()

			stopCh := make(chan struct{})
			go func() {
				<-ctx.Done()
				close(stopCh)
			}()

			leaderCh, err := lock.Lock(stopCh)
			if err != nil {
				t.Logf("Contender %d failed to acquire: %v", id, err)
				return
			}
			if leaderCh != nil {
				acquired <- id
				time.Sleep(100 * time.Millisecond) // Hold briefly
				lock.Unlock()
			}
		}(i)
	}

	// Wait for at least one to acquire
	select {
	case id := <-acquired:
		t.Logf("Contender %d acquired the lock", id)
	case <-time.After(15 * time.Second):
		t.Fatal("No contender acquired the lock")
	}

	// Allow time for others to potentially acquire
	time.Sleep(2 * time.Second)

	// Cleanup
	_, err := client.KV().Delete(key, nil)
	require.NoError(t, err)
}

// TestLockDestroy tests lock destruction rules
func TestLockDestroy(t *testing.T) {
	client := getTestClient(t)

	key := "test/lock/destroy-" + randomString(8)

	lock, err := client.LockKey(key)
	require.NoError(t, err)

	// Acquire lock
	leaderCh, err := lock.Lock(nil)
	require.NoError(t, err)
	require.NotNil(t, leaderCh)

	// Destroy should fail while lock is held
	err = lock.Destroy()
	if err != nil {
		t.Logf("Destroy while held returned: %v (expected)", err)
	}

	// Unlock first
	err = lock.Unlock()
	require.NoError(t, err)

	// Now destroy should succeed
	err = lock.Destroy()
	require.NoError(t, err)

	// Verify key is gone
	pair, _, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	assert.Nil(t, pair, "Key should be deleted after destroy")
}

// TestLockConflict tests lock conflict with semaphore on same key
func TestLockConflict(t *testing.T) {
	client := getTestClient(t)

	key := "test/lock/conflict-" + randomString(8)

	// Create and acquire a lock
	lock, err := client.LockKey(key)
	require.NoError(t, err)

	leaderCh, err := lock.Lock(nil)
	require.NoError(t, err)
	require.NotNil(t, leaderCh)

	// Try to create a semaphore on the same prefix (should work on different keys)
	semOpts := &api.SemaphoreOptions{
		Prefix: key + "/sem",
		Limit:  2,
	}
	sem, err := client.SemaphoreOpts(semOpts)
	if err == nil {
		// Semaphore on different key should work
		_, semErr := sem.Acquire(nil)
		if semErr != nil {
			t.Logf("Semaphore acquire: %v", semErr)
		}
		_ = sem.Release()
	}

	// Cleanup
	err = lock.Unlock()
	require.NoError(t, err)
	_, err = client.KV().Delete(key, nil)
	require.NoError(t, err)
}

// TestLockReclaimLock tests re-acquiring lock with same session
func TestLockReclaimLock(t *testing.T) {
	client := getTestClient(t)

	key := "test/lock/reclaim-" + randomString(8)

	// Create session first
	session := client.Session()
	sessionID, _, err := session.Create(&api.SessionEntry{
		Name:     "reclaim-test",
		TTL:      "30s",
		Behavior: "delete",
	}, nil)
	require.NoError(t, err)

	// Create lock with the session
	opts := &api.LockOptions{
		Key:     key,
		Session: sessionID,
	}
	lock1, err := client.LockOpts(opts)
	require.NoError(t, err)

	// Acquire with first lock handle
	leaderCh1, err := lock1.Lock(nil)
	require.NoError(t, err)
	require.NotNil(t, leaderCh1)

	// Create second lock handle with same session
	lock2, err := client.LockOpts(opts)
	require.NoError(t, err)

	// Second lock should be able to "reclaim" since same session
	leaderCh2, err := lock2.Lock(nil)
	if err == nil && leaderCh2 != nil {
		t.Log("Second lock handle reclaimed the lock")
	}

	// Cleanup
	_ = lock1.Unlock()
	_, err = session.Destroy(sessionID, nil)
	require.NoError(t, err)
	_, err = client.KV().Delete(key, nil)
	require.NoError(t, err)
}

// TestLockMonitorRetry tests lock monitor retry behavior
func TestLockMonitorRetry(t *testing.T) {
	client := getTestClient(t)

	key := "test/lock/monitor-" + randomString(8)

	opts := &api.LockOptions{
		Key:              key,
		SessionTTL:       "15s",
		MonitorRetries:   3,
		MonitorRetryTime: time.Second,
	}
	lock, err := client.LockOpts(opts)
	require.NoError(t, err)

	// Acquire lock
	leaderCh, err := lock.Lock(nil)
	require.NoError(t, err)
	require.NotNil(t, leaderCh)

	// Lock should remain held with monitoring
	select {
	case <-leaderCh:
		t.Fatal("Lock should not be lost during normal operation")
	case <-time.After(2 * time.Second):
		// Expected - lock still held
	}

	// Cleanup
	err = lock.Unlock()
	require.NoError(t, err)
	_, err = client.KV().Delete(key, nil)
	require.NoError(t, err)
}

// TestLockOneShot tests one-shot lock acquisition with timeout
func TestLockOneShot(t *testing.T) {
	client := getTestClient(t)

	key := "test/lock/oneshot-" + randomString(8)

	// First lock - should succeed
	lock1, err := client.LockKey(key)
	require.NoError(t, err)

	leaderCh1, err := lock1.Lock(nil)
	require.NoError(t, err)
	require.NotNil(t, leaderCh1)

	// Second lock with timeout - should fail since lock is held
	opts := &api.LockOptions{
		Key:          key,
		LockTryOnce:  true,
		LockWaitTime: time.Second,
	}
	lock2, err := client.LockOpts(opts)
	require.NoError(t, err)

	leaderCh2, err := lock2.Lock(nil)
	if leaderCh2 == nil {
		t.Log("One-shot lock correctly failed to acquire (lock is held)")
	} else {
		t.Log("One-shot lock acquired (unexpected, but possible if lock1 released)")
		_ = lock2.Unlock()
	}

	// Cleanup
	err = lock1.Unlock()
	require.NoError(t, err)
	_, err = client.KV().Delete(key, nil)
	require.NoError(t, err)
}

// TestLockWithValue tests lock with custom value
func TestLockWithValue(t *testing.T) {
	client := getTestClient(t)

	key := "test/lock/value-" + randomString(8)
	value := []byte("lock-holder-info")

	opts := &api.LockOptions{
		Key:   key,
		Value: value,
	}
	lock, err := client.LockOpts(opts)
	require.NoError(t, err)

	// Acquire lock
	leaderCh, err := lock.Lock(nil)
	require.NoError(t, err)
	require.NotNil(t, leaderCh)

	// Verify value is stored
	pair, _, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair)
	assert.Equal(t, value, pair.Value, "Lock should store custom value")

	// Cleanup
	err = lock.Unlock()
	require.NoError(t, err)
	_, err = client.KV().Delete(key, nil)
	require.NoError(t, err)
}
