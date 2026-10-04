package tests

import (
	"fmt"
	"sync"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Concurrent Safety Tests ====================

// CTEST-001: Concurrent KV writes to different keys — all should succeed
func TestConcurrentKVDifferentKeys(t *testing.T) {
	client := getClient(t)
	prefix := "ctest001/concurrent/" + randomID() + "/"

	numGoroutines := 10
	var wg sync.WaitGroup
	errs := make([]error, numGoroutines)
	results := make([]bool, numGoroutines)

	wg.Add(numGoroutines)
	for i := 0; i < numGoroutines; i++ {
		go func(idx int) {
			defer wg.Done()
			key := fmt.Sprintf("%skey-%d", prefix, idx)
			value := []byte(fmt.Sprintf("value-%d", idx))

			_, errs[idx] = client.KV().Put(&api.KVPair{
				Key:   key,
				Value: value,
			}, nil)
			if errs[idx] != nil {
				return
			}

			// Verify the value is correct
			pair, _, err := client.KV().Get(key, nil)
			if err != nil {
				errs[idx] = err
				return
			}
			results[idx] = pair != nil && string(pair.Value) == string(value)
		}(i)
	}
	wg.Wait()

	// All writes should succeed
	for i := 0; i < numGoroutines; i++ {
		assert.NoError(t, errs[i], "Goroutine %d should not error", i)
		assert.True(t, results[i], "Goroutine %d should have correct value", i)
	}

	// Verify all keys exist after concurrent writes
	pairs, _, err := client.KV().List(prefix, nil)
	require.NoError(t, err)
	assert.Len(t, pairs, numGoroutines,
		"Should have exactly %d keys after concurrent writes", numGoroutines)

	// Cleanup
	client.KV().DeleteTree(prefix, nil)
}

// CTEST-002: Concurrent KV writes to the same key — last write wins or CAS fails
func TestConcurrentKVSameKey(t *testing.T) {
	client := getClient(t)
	key := "ctest002/same-key/" + randomID()

	// Initial value
	_, err := client.KV().Put(&api.KVPair{
		Key:   key,
		Value: []byte("initial"),
	}, nil)
	require.NoError(t, err)
	defer client.KV().Delete(key, nil)

	numGoroutines := 10
	var wg sync.WaitGroup
	errs := make([]error, numGoroutines)
	successCount := 0
	var mu sync.Mutex

	wg.Add(numGoroutines)
	for i := 0; i < numGoroutines; i++ {
		go func(idx int) {
			defer wg.Done()
			_, errs[idx] = client.KV().Put(&api.KVPair{
				Key:   key,
				Value: []byte(fmt.Sprintf("concurrent-%d", idx)),
			}, nil)
			if errs[idx] == nil {
				mu.Lock()
				successCount++
				mu.Unlock()
			}
		}(i)
	}
	wg.Wait()

	// All non-CAS writes should succeed (each overwrites the previous)
	for i := 0; i < numGoroutines; i++ {
		assert.NoError(t, errs[i], "Non-CAS put %d should not error", i)
	}
	assert.Equal(t, numGoroutines, successCount,
		"All concurrent non-CAS puts should succeed")

	// The final value should be one of the concurrent values (last write wins)
	pair, _, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair)
	// Value should start with "concurrent-"
	assert.Contains(t, string(pair.Value), "concurrent-",
		"Final value should be one of the concurrent writes")
}

// CTEST-003: Concurrent session create/destroy — no interference
func TestConcurrentSessionCreateDestroy(t *testing.T) {
	client := getClient(t)
	numGoroutines := 10
	sessions := make([]string, numGoroutines)
	var wg sync.WaitGroup
	errs := make([]error, numGoroutines)

	// Phase 1: Concurrent session creation
	wg.Add(numGoroutines)
	for i := 0; i < numGoroutines; i++ {
		go func(idx int) {
			defer wg.Done()
			id, _, err := client.Session().Create(&api.SessionEntry{
				Name: fmt.Sprintf("ctest003-session-%d", idx),
				TTL:  "30s",
			}, nil)
			errs[idx] = err
			sessions[idx] = id
		}(i)
	}
	wg.Wait()

	// All sessions should be created successfully
	for i := 0; i < numGoroutines; i++ {
		require.NoError(t, errs[i], "Session %d creation should not error", i)
		assert.NotEmpty(t, sessions[i], "Session %d ID should not be empty", i)
	}

	// Verify each session is distinct
	seen := make(map[string]bool)
	for i, id := range sessions {
		assert.False(t, seen[id],
			"Session %d ID %q should be unique", i, id)
		seen[id] = true
	}

	// Phase 2: Concurrent session destruction
	wg.Add(numGoroutines)
	for i := 0; i < numGoroutines; i++ {
		go func(idx int) {
			defer wg.Done()
			_, errs[idx] = client.Session().Destroy(sessions[idx], nil)
		}(i)
	}
	wg.Wait()

	// All destroys should succeed
	for i := 0; i < numGoroutines; i++ {
		assert.NoError(t, errs[i], "Session %d destruction should not error", i)
	}

	// Verify all sessions are gone
	for i, id := range sessions {
		info, _, err := client.Session().Info(id, nil)
		assert.NoError(t, err, "Info for session %d should not error", i)
		assert.Nil(t, info, "Session %d should be destroyed", i)
	}
}

// CTEST-004: Concurrent service registration — no conflicts
func TestConcurrentServiceRegistration(t *testing.T) {
	client := getClient(t)
	prefix := "ctest004-svc-" + randomID()
	numGoroutines := 5
	var wg sync.WaitGroup
	errs := make([]error, numGoroutines)
	serviceIDs := make([]string, numGoroutines)

	// Each goroutine registers a service with a distinct ID and port
	wg.Add(numGoroutines)
	for i := 0; i < numGoroutines; i++ {
		go func(idx int) {
			defer wg.Done()
			serviceID := fmt.Sprintf("%s-%d", prefix, idx)
			serviceIDs[idx] = serviceID
			errs[idx] = client.Agent().ServiceRegister(&api.AgentServiceRegistration{
				ID:      serviceID,
				Name:     fmt.Sprintf("concurrent-svc-%d", idx),
				Port:     9000 + idx,
				Address:  fmt.Sprintf("10.0.%d.1", idx),
			})
		}(i)
	}
	wg.Wait()

	// Cleanup
	defer func() {
		for _, id := range serviceIDs {
			client.Agent().ServiceDeregister(id)
		}
	}()

	// All registrations should succeed
	for i := 0; i < numGoroutines; i++ {
		require.NoError(t, errs[i], "Service %d registration should not error", i)
	}

	// Wait for services to be visible
	for _, id := range serviceIDs {
		require.True(t, waitForAgentService(t, client, id, 5*time.Second),
			"Service %s should be visible within timeout", id)
	}

	// Verify all services are registered with correct data
	services, err := client.Agent().Services()
	require.NoError(t, err)
	for i, id := range serviceIDs {
		svc, exists := services[id]
		assert.True(t, exists, "Service %d should exist in services map", i)
		if exists {
			assert.Equal(t, fmt.Sprintf("concurrent-svc-%d", i), svc.Service,
				"Service %d name should match", i)
			assert.Equal(t, 9000+i, svc.Port,
				"Service %d port should match", i)
			assert.Equal(t, fmt.Sprintf("10.0.%d.1", i), svc.Address,
				"Service %d address should match", i)
		}
	}
}
