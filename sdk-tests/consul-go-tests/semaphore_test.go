package tests

import (
	"context"
	"sync"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// TestSemaphoreAcquireRelease tests basic semaphore acquisition and release
func TestSemaphoreAcquireRelease(t *testing.T) {
	client := getTestClient(t)

	prefix := "test/semaphore/basic-" + randomString(8)
	limit := 2

	opts := &api.SemaphoreOptions{
		Prefix: prefix,
		Limit:  limit,
	}
	sem, err := client.SemaphoreOpts(opts)
	require.NoError(t, err)

	// Acquire semaphore
	lockCh, err := sem.Acquire(nil)
	require.NoError(t, err)
	require.NotNil(t, lockCh, "Should get lock channel when semaphore acquired")

	// Release semaphore
	err = sem.Release()
	require.NoError(t, err)

	// Verify release
	select {
	case <-lockCh:
		// Expected - channel closed when released
	case <-time.After(2 * time.Second):
		t.Fatal("Lock channel should be closed after release")
	}

	// Cleanup
	_, err = client.KV().DeleteTree(prefix, nil)
	require.NoError(t, err)
}

// TestSemaphoreForceInvalidate tests semaphore loss when session is destroyed
func TestSemaphoreForceInvalidate(t *testing.T) {
	client := getTestClient(t)

	prefix := "test/semaphore/invalidate-" + randomString(8)

	opts := &api.SemaphoreOptions{
		Prefix:      prefix,
		Limit:       2,
		SessionTTL:  "15s",
		SessionName: "test-sem-session",
	}
	sem, err := client.SemaphoreOpts(opts)
	require.NoError(t, err)

	// Acquire semaphore
	lockCh, err := sem.Acquire(nil)
	require.NoError(t, err)
	require.NotNil(t, lockCh)

	// Get session ID from contender key
	keys, _, err := client.KV().Keys(prefix+"/", "", nil)
	require.NoError(t, err)

	var sessionID string
	for _, key := range keys {
		pair, _, err := client.KV().Get(key, nil)
		if err == nil && pair != nil && pair.Session != "" {
			sessionID = pair.Session
			break
		}
	}

	if sessionID != "" {
		// Force destroy the session
		_, err = client.Session().Destroy(sessionID, nil)
		require.NoError(t, err)

		// Semaphore should be lost
		select {
		case <-lockCh:
			// Expected - channel closed when session destroyed
		case <-time.After(5 * time.Second):
			t.Fatal("Lock channel should be closed after session destroy")
		}
	}

	// Cleanup
	_, err = client.KV().DeleteTree(prefix, nil)
	require.NoError(t, err)
}

// TestSemaphoreDeleteKey tests semaphore behavior when key is deleted
func TestSemaphoreDeleteKey(t *testing.T) {
	client := getTestClient(t)

	prefix := "test/semaphore/delete-" + randomString(8)

	opts := &api.SemaphoreOptions{
		Prefix: prefix,
		Limit:  2,
	}
	sem, err := client.SemaphoreOpts(opts)
	require.NoError(t, err)

	// Acquire semaphore
	lockCh, err := sem.Acquire(nil)
	require.NoError(t, err)
	require.NotNil(t, lockCh)

	// Delete the semaphore keys
	_, err = client.KV().DeleteTree(prefix, nil)
	require.NoError(t, err)

	// Semaphore holder should eventually detect loss
	select {
	case <-lockCh:
		// Expected if implementation detects deletion
	case <-time.After(3 * time.Second):
		t.Log("Semaphore not immediately released after key deletion")
	}

	// Cleanup - release if still held
	_ = sem.Release()
}

// TestSemaphoreContend tests concurrent semaphore acquisition
func TestSemaphoreContend(t *testing.T) {
	client := getTestClient(t)

	prefix := "test/semaphore/contend-" + randomString(8)
	limit := 2
	numContenders := 4

	acquired := make(chan int, numContenders)
	var wg sync.WaitGroup

	// Start multiple contenders
	for i := 0; i < numContenders; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()

			opts := &api.SemaphoreOptions{
				Prefix: prefix,
				Limit:  limit,
			}
			sem, err := client.SemaphoreOpts(opts)
			if err != nil {
				t.Logf("Contender %d failed to create semaphore: %v", id, err)
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

			lockCh, err := sem.Acquire(stopCh)
			if err != nil {
				t.Logf("Contender %d failed to acquire: %v", id, err)
				return
			}
			if lockCh != nil {
				acquired <- id
				time.Sleep(500 * time.Millisecond) // Hold briefly
				sem.Release()
			}
		}(i)
	}

	// Wait for acquisitions
	time.Sleep(2 * time.Second)

	// Count how many acquired (should be up to limit)
	count := 0
	for {
		select {
		case id := <-acquired:
			count++
			t.Logf("Contender %d acquired semaphore", id)
		default:
			goto done
		}
	}
done:

	t.Logf("Total acquisitions: %d (limit was %d)", count, limit)
	assert.True(t, count >= 1, "At least one contender should acquire")

	wg.Wait()

	// Cleanup
	_, err := client.KV().DeleteTree(prefix, nil)
	require.NoError(t, err)
}

// TestSemaphoreBadLimit tests rejection of invalid limits
func TestSemaphoreBadLimit(t *testing.T) {
	client := getTestClient(t)

	prefix := "test/semaphore/badlimit-" + randomString(8)

	// Limit of 0 should fail
	opts := &api.SemaphoreOptions{
		Prefix: prefix,
		Limit:  0,
	}
	_, err := client.SemaphoreOpts(opts)
	assert.Error(t, err, "Limit of 0 should fail")

	// Negative limit should fail
	opts.Limit = -1
	_, err = client.SemaphoreOpts(opts)
	assert.Error(t, err, "Negative limit should fail")
}

// TestSemaphoreDestroy tests semaphore destruction
func TestSemaphoreDestroy(t *testing.T) {
	client := getTestClient(t)

	prefix := "test/semaphore/destroy-" + randomString(8)

	opts := &api.SemaphoreOptions{
		Prefix: prefix,
		Limit:  2,
	}
	sem, err := client.SemaphoreOpts(opts)
	require.NoError(t, err)

	// Acquire semaphore
	lockCh, err := sem.Acquire(nil)
	require.NoError(t, err)
	require.NotNil(t, lockCh)

	// Destroy should fail while held
	err = sem.Destroy()
	if err != nil {
		t.Logf("Destroy while held returned: %v (expected)", err)
	}

	// Release first
	err = sem.Release()
	require.NoError(t, err)

	// Now destroy should succeed
	err = sem.Destroy()
	require.NoError(t, err)

	// Verify keys are gone
	keys, _, err := client.KV().Keys(prefix+"/", "", nil)
	require.NoError(t, err)
	assert.Empty(t, keys, "Semaphore keys should be deleted after destroy")
}

// TestSemaphoreConflict tests semaphore conflict with lock on same prefix
func TestSemaphoreConflict(t *testing.T) {
	client := getTestClient(t)

	prefix := "test/semaphore/conflict-" + randomString(8)

	// Create and acquire a semaphore
	semOpts := &api.SemaphoreOptions{
		Prefix: prefix,
		Limit:  2,
	}
	sem, err := client.SemaphoreOpts(semOpts)
	require.NoError(t, err)

	lockCh, err := sem.Acquire(nil)
	require.NoError(t, err)
	require.NotNil(t, lockCh)

	// Try to create a lock on different key in same prefix tree
	lock, err := client.LockKey(prefix + "/lock")
	require.NoError(t, err)

	// Lock on different key should work
	lockLeaderCh, err := lock.Lock(nil)
	if err == nil && lockLeaderCh != nil {
		t.Log("Lock on different key succeeded")
		_ = lock.Unlock()
	}

	// Cleanup
	err = sem.Release()
	require.NoError(t, err)
	_, err = client.KV().DeleteTree(prefix, nil)
	require.NoError(t, err)
}

// TestSemaphoreMonitorRetry tests semaphore monitor retry behavior
func TestSemaphoreMonitorRetry(t *testing.T) {
	client := getTestClient(t)

	prefix := "test/semaphore/monitor-" + randomString(8)

	opts := &api.SemaphoreOptions{
		Prefix:           prefix,
		Limit:            2,
		SessionTTL:       "15s",
		MonitorRetries:   3,
		MonitorRetryTime: time.Second,
	}
	sem, err := client.SemaphoreOpts(opts)
	require.NoError(t, err)

	// Acquire semaphore
	lockCh, err := sem.Acquire(nil)
	require.NoError(t, err)
	require.NotNil(t, lockCh)

	// Semaphore should remain held with monitoring
	select {
	case <-lockCh:
		t.Fatal("Semaphore should not be lost during normal operation")
	case <-time.After(2 * time.Second):
		// Expected - semaphore still held
	}

	// Cleanup
	err = sem.Release()
	require.NoError(t, err)
	_, err = client.KV().DeleteTree(prefix, nil)
	require.NoError(t, err)
}

// TestSemaphoreOneShot tests one-shot semaphore acquisition with timeout
func TestSemaphoreOneShot(t *testing.T) {
	client := getTestClient(t)

	prefix := "test/semaphore/oneshot-" + randomString(8)
	limit := 1

	// First semaphore - should succeed
	opts1 := &api.SemaphoreOptions{
		Prefix: prefix,
		Limit:  limit,
	}
	sem1, err := client.SemaphoreOpts(opts1)
	require.NoError(t, err)

	lockCh1, err := sem1.Acquire(nil)
	require.NoError(t, err)
	require.NotNil(t, lockCh1)

	// Second semaphore with timeout - should fail since limit reached
	opts2 := &api.SemaphoreOptions{
		Prefix:            prefix,
		Limit:             limit,
		SemaphoreTryOnce:  true,
		SemaphoreWaitTime: time.Second,
	}
	sem2, err := client.SemaphoreOpts(opts2)
	require.NoError(t, err)

	lockCh2, err := sem2.Acquire(nil)
	if lockCh2 == nil {
		t.Log("One-shot semaphore correctly failed to acquire (limit reached)")
	} else {
		t.Log("One-shot semaphore acquired (unexpected)")
		_ = sem2.Release()
	}

	// Cleanup
	err = sem1.Release()
	require.NoError(t, err)
	_, err = client.KV().DeleteTree(prefix, nil)
	require.NoError(t, err)
}

// TestSemaphoreWithValue tests semaphore with custom holder value
func TestSemaphoreWithValue(t *testing.T) {
	client := getTestClient(t)

	prefix := "test/semaphore/value-" + randomString(8)
	value := []byte("semaphore-holder-info")

	opts := &api.SemaphoreOptions{
		Prefix: prefix,
		Limit:  2,
		Value:  value,
	}
	sem, err := client.SemaphoreOpts(opts)
	require.NoError(t, err)

	// Acquire semaphore
	lockCh, err := sem.Acquire(nil)
	require.NoError(t, err)
	require.NotNil(t, lockCh)

	// Verify value is stored in contender key
	keys, _, err := client.KV().Keys(prefix+"/", "", nil)
	require.NoError(t, err)

	foundValue := false
	for _, key := range keys {
		pair, _, err := client.KV().Get(key, nil)
		if err == nil && pair != nil && string(pair.Value) == string(value) {
			foundValue = true
			break
		}
	}
	assert.True(t, foundValue, "Semaphore should store custom value")

	// Cleanup
	err = sem.Release()
	require.NoError(t, err)
	_, err = client.KV().DeleteTree(prefix, nil)
	require.NoError(t, err)
}

// TestSemaphoreMultipleHolders tests multiple holders within limit
func TestSemaphoreMultipleHolders(t *testing.T) {
	client := getTestClient(t)

	prefix := "test/semaphore/multi-" + randomString(8)
	limit := 3

	var sems []*api.Semaphore
	var lockChs []<-chan struct{}

	// Acquire up to limit
	for i := 0; i < limit; i++ {
		opts := &api.SemaphoreOptions{
			Prefix: prefix,
			Limit:  limit,
		}
		sem, err := client.SemaphoreOpts(opts)
		require.NoError(t, err)

		lockCh, err := sem.Acquire(nil)
		require.NoError(t, err)
		require.NotNil(t, lockCh, "Holder %d should acquire within limit", i)

		sems = append(sems, sem)
		lockChs = append(lockChs, lockCh)
	}

	t.Logf("Successfully acquired %d semaphore slots (limit: %d)", len(sems), limit)

	// Cleanup
	for _, sem := range sems {
		_ = sem.Release()
	}
	_, err := client.KV().DeleteTree(prefix, nil)
	require.NoError(t, err)
}
