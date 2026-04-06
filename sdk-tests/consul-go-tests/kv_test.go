package tests

import (
	"fmt"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== P0: Critical Tests ====================

// CK-001: Test KV put and get with full field validation
func TestKVPut(t *testing.T) {
	client := getClient(t)
	key := "ck001/put/" + randomID()
	value := []byte("test-value")

	// Put
	wm, err := client.KV().Put(&api.KVPair{
		Key:   key,
		Value: value,
	}, nil)
	require.NoError(t, err, "KV put should succeed")
	// WriteMeta should have valid request time
	assert.Greater(t, wm.RequestTime, time.Duration(0),
		"WriteMeta.RequestTime should be positive")

	// Get and validate ALL fields
	pair, meta, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair, "Pair should not be nil")

	// Value correctness
	assert.Equal(t, key, pair.Key, "Key must match")
	assert.Equal(t, value, pair.Value, "Value must match")

	// Index fields
	assert.Greater(t, pair.CreateIndex, uint64(0),
		"CreateIndex must be > 0 after put")
	assert.Greater(t, pair.ModifyIndex, uint64(0),
		"ModifyIndex must be > 0 after put")
	assert.Equal(t, uint64(0), pair.LockIndex,
		"LockIndex should be 0 (not locked)")
	assert.Equal(t, uint64(0), pair.Flags,
		"Flags should be 0 (not set)")
	assert.Empty(t, pair.Session,
		"Session should be empty (no lock)")

	// QueryMeta validation (matches Consul's setMeta headers)
	assert.Greater(t, meta.LastIndex, uint64(0),
		"QueryMeta.LastIndex must be > 0")
	assert.True(t, meta.KnownLeader,
		"QueryMeta.KnownLeader must be true")

	// Cleanup
	client.KV().Delete(key, nil)
}

// CK-002: Test KV get non-existent key returns nil
func TestKVGetNonExistent(t *testing.T) {
	client := getClient(t)

	pair, meta, err := client.KV().Get("non-existent-key-"+randomID(), nil)
	require.NoError(t, err, "Get non-existent key should not error")
	assert.Nil(t, pair, "Non-existent key must return nil pair")
	// Even 404 responses should have valid QueryMeta
	assert.Greater(t, meta.LastIndex, uint64(0),
		"QueryMeta.LastIndex must be > 0 even for 404")
}

// CK-003: Test KV delete
func TestKVDelete(t *testing.T) {
	client := getClient(t)
	key := "ck003/delete/" + randomID()

	// Put first
	_, err := client.KV().Put(&api.KVPair{
		Key:   key,
		Value: []byte("to-delete"),
	}, nil)
	require.NoError(t, err)

	// Verify exists
	pair, _, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair, "Key should exist before delete")

	// Delete
	_, err = client.KV().Delete(key, nil)
	require.NoError(t, err, "KV delete should succeed")

	// Verify deleted — must return nil, not empty
	pair, _, err = client.KV().Get(key, nil)
	require.NoError(t, err)
	assert.Nil(t, pair, "Key must be nil after delete")
}

// ==================== P1: Important Tests ====================

// CK-004: Test KV list by prefix with content validation
func TestKVList(t *testing.T) {
	client := getClient(t)
	prefix := "ck004/list/" + randomID() + "/"

	// Put multiple keys with different values
	for i := 0; i < 5; i++ {
		key := fmt.Sprintf("%skey%d", prefix, i)
		_, err := client.KV().Put(&api.KVPair{
			Key:   key,
			Value: []byte(fmt.Sprintf("value%d", i)),
		}, nil)
		require.NoError(t, err)
	}

	// List by prefix
	pairs, meta, err := client.KV().List(prefix, nil)
	require.NoError(t, err, "KV list should succeed")
	require.Len(t, pairs, 5, "Should have exactly 5 keys")

	// Validate each pair has correct structure
	for i, pair := range pairs {
		expectedKey := fmt.Sprintf("%skey%d", prefix, i)
		assert.Equal(t, expectedKey, pair.Key,
			"Key at index %d should match", i)
		assert.Equal(t, []byte(fmt.Sprintf("value%d", i)), pair.Value,
			"Value at index %d should match", i)
		assert.Greater(t, pair.CreateIndex, uint64(0),
			"CreateIndex at index %d must be > 0", i)
		assert.Greater(t, pair.ModifyIndex, uint64(0),
			"ModifyIndex at index %d must be > 0", i)
	}

	// QueryMeta
	assert.Greater(t, meta.LastIndex, uint64(0))

	// Cleanup
	client.KV().DeleteTree(prefix, nil)
}

// CK-005: Test KV keys only
func TestKVKeys(t *testing.T) {
	client := getClient(t)
	prefix := "ck005/keys/" + randomID() + "/"

	// Put multiple keys
	for i := 0; i < 3; i++ {
		key := fmt.Sprintf("%skey%d", prefix, i)
		_, err := client.KV().Put(&api.KVPair{
			Key:   key,
			Value: []byte("value"),
		}, nil)
		require.NoError(t, err)
	}

	// Get keys only
	keys, meta, err := client.KV().Keys(prefix, "", nil)
	require.NoError(t, err, "KV keys should succeed")
	require.Len(t, keys, 3, "Should have exactly 3 keys")

	// Verify each key starts with prefix
	for i, key := range keys {
		expectedKey := fmt.Sprintf("%skey%d", prefix, i)
		assert.Equal(t, expectedKey, key,
			"Key at index %d should match exactly", i)
	}

	// QueryMeta
	assert.Greater(t, meta.LastIndex, uint64(0))

	// Cleanup
	client.KV().DeleteTree(prefix, nil)
}

// CK-006: Test KV CAS (Compare-And-Swap) — success AND failure
func TestKVCAS(t *testing.T) {
	client := getClient(t)
	key := "ck006/cas/" + randomID()

	// Initial put
	_, err := client.KV().Put(&api.KVPair{
		Key:   key,
		Value: []byte("initial"),
	}, nil)
	require.NoError(t, err)

	// Get to obtain ModifyIndex
	pair, _, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair)

	correctIndex := pair.ModifyIndex
	assert.Greater(t, correctIndex, uint64(0),
		"ModifyIndex must be > 0")

	// CAS with correct index — should succeed
	pair.Value = []byte("updated")
	pair.ModifyIndex = correctIndex
	success, _, err := client.KV().CAS(pair, nil)
	require.NoError(t, err)
	assert.True(t, success, "CAS with correct index must succeed")

	// Verify the value was actually updated
	pair, _, err = client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair)
	assert.Equal(t, []byte("updated"), pair.Value,
		"Value must be updated after successful CAS")
	assert.Greater(t, pair.ModifyIndex, correctIndex,
		"ModifyIndex must increase after CAS update")

	// CAS with STALE index — must FAIL (this is the critical test)
	stalePair := &api.KVPair{
		Key:         key,
		Value:       []byte("should-not-be-written"),
		ModifyIndex: correctIndex, // old/stale index
	}
	success, _, err = client.KV().CAS(stalePair, nil)
	require.NoError(t, err, "CAS with stale index should not error")
	assert.False(t, success,
		"CAS with stale index MUST fail (return false)")

	// Verify value was NOT changed
	pair, _, err = client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair)
	assert.Equal(t, []byte("updated"), pair.Value,
		"Value must NOT change after failed CAS")

	// Cleanup
	client.KV().Delete(key, nil)
}

// CK-007: Test KV acquire/release with Session and LockIndex validation
func TestKVAcquireRelease(t *testing.T) {
	client := getClient(t)
	key := "ck007/lock/" + randomID()

	// Create session
	sessionID, _, err := client.Session().Create(&api.SessionEntry{
		Name: "test-lock-session",
		TTL:  "30s",
	}, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID, nil)

	// Acquire lock
	acquired, _, err := client.KV().Acquire(&api.KVPair{
		Key:     key,
		Value:   []byte("locked"),
		Session: sessionID,
	}, nil)
	require.NoError(t, err)
	assert.True(t, acquired, "First acquire must succeed")

	// Verify lock state — this is what most tests miss
	pair, _, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair)
	assert.Equal(t, sessionID, pair.Session,
		"Session field must be set to lock holder's session ID")
	assert.Equal(t, uint64(1), pair.LockIndex,
		"LockIndex must be 1 after first acquire")
	assert.Equal(t, []byte("locked"), pair.Value,
		"Value should be set by acquire")

	// Try to acquire with DIFFERENT session — must fail
	sessionID2, _, err := client.Session().Create(&api.SessionEntry{
		Name: "competing-session",
		TTL:  "30s",
	}, nil)
	require.NoError(t, err)
	defer client.Session().Destroy(sessionID2, nil)

	acquired2, _, err := client.KV().Acquire(&api.KVPair{
		Key:     key,
		Value:   []byte("should-not-lock"),
		Session: sessionID2,
	}, nil)
	require.NoError(t, err)
	assert.False(t, acquired2,
		"Competing session must NOT acquire held lock")

	// Verify lock is still held by original session
	pair, _, err = client.KV().Get(key, nil)
	require.NoError(t, err)
	assert.Equal(t, sessionID, pair.Session,
		"Lock must still be held by original session")

	// Release lock
	released, _, err := client.KV().Release(&api.KVPair{
		Key:     key,
		Session: sessionID,
	}, nil)
	require.NoError(t, err)
	assert.True(t, released, "Release must succeed")

	// Verify lock is released
	pair, _, err = client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair)
	assert.Empty(t, pair.Session,
		"Session must be empty after release")
	// LockIndex should NOT decrease — it tracks how many times locked
	assert.Equal(t, uint64(1), pair.LockIndex,
		"LockIndex must remain 1 after release (tracks lock count)")

	// Cleanup
	client.KV().Delete(key, nil)
}

// CK-008: Test KV with flags
func TestKVWithFlags(t *testing.T) {
	client := getClient(t)
	key := "ck008/flags/" + randomID()
	flags := uint64(12345)

	// Put with flags
	_, err := client.KV().Put(&api.KVPair{
		Key:   key,
		Value: []byte("value-with-flags"),
		Flags: flags,
	}, nil)
	require.NoError(t, err)

	// Get and verify flags are preserved
	pair, _, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair)
	assert.Equal(t, flags, pair.Flags,
		"Flags must be preserved exactly")
	assert.Equal(t, []byte("value-with-flags"), pair.Value)

	// Cleanup
	client.KV().Delete(key, nil)
}

// CK-009: Test KV tree delete
func TestKVDeleteTree(t *testing.T) {
	client := getClient(t)
	prefix := "ck009/tree/" + randomID() + "/"

	// Put multiple keys
	for i := 0; i < 5; i++ {
		_, err := client.KV().Put(&api.KVPair{
			Key:   fmt.Sprintf("%skey%d", prefix, i),
			Value: []byte("value"),
		}, nil)
		require.NoError(t, err)
	}

	// Verify all exist
	pairs, _, err := client.KV().List(prefix, nil)
	require.NoError(t, err)
	require.Len(t, pairs, 5, "Should have 5 keys before delete")

	// Delete tree
	_, err = client.KV().DeleteTree(prefix, nil)
	require.NoError(t, err, "DeleteTree should succeed")

	time.Sleep(500 * time.Millisecond)

	// Verify ALL deleted
	pairs, _, err = client.KV().List(prefix, nil)
	require.NoError(t, err)
	assert.Empty(t, pairs, "All keys must be deleted after DeleteTree")
}

// CK-010: Test KV DeleteCAS — success and failure
func TestKVDeleteCAS(t *testing.T) {
	client := getClient(t)
	key := "ck010/deletecas/" + randomID()

	// Put initial value
	_, err := client.KV().Put(&api.KVPair{
		Key:   key,
		Value: []byte("cas-delete-value"),
	}, nil)
	require.NoError(t, err)

	// Get to obtain ModifyIndex
	pair, _, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair)
	correctIndex := pair.ModifyIndex

	// DeleteCAS with correct index — must succeed
	success, _, err := client.KV().DeleteCAS(&api.KVPair{
		Key:         key,
		ModifyIndex: correctIndex,
	}, nil)
	require.NoError(t, err)
	assert.True(t, success, "DeleteCAS with correct ModifyIndex must succeed")

	// Verify actually deleted
	pair, _, err = client.KV().Get(key, nil)
	require.NoError(t, err)
	assert.Nil(t, pair, "Key must be nil after successful DeleteCAS")

	// Put again for stale CAS test
	_, err = client.KV().Put(&api.KVPair{
		Key:   key,
		Value: []byte("cas-delete-value-2"),
	}, nil)
	require.NoError(t, err)
	defer client.KV().Delete(key, nil)

	// DeleteCAS with STALE index — must fail
	success, _, err = client.KV().DeleteCAS(&api.KVPair{
		Key:         key,
		ModifyIndex: correctIndex, // stale
	}, nil)
	require.NoError(t, err)
	assert.False(t, success, "DeleteCAS with stale index must fail")

	// Verify key still exists
	pair, _, err = client.KV().Get(key, nil)
	require.NoError(t, err)
	assert.NotNil(t, pair, "Key must still exist after failed DeleteCAS")
	assert.Equal(t, []byte("cas-delete-value-2"), pair.Value)
}

// ==================== P2: Behavioral Tests ====================

// CK-011: Test KV blocking query — index progression
func TestKVBlockingQuery(t *testing.T) {
	client := getClient(t)
	key := "ck011/blocking/" + randomID()

	// Put initial value
	_, err := client.KV().Put(&api.KVPair{
		Key:   key,
		Value: []byte("initial"),
	}, nil)
	require.NoError(t, err)

	// Get to establish baseline index
	_, meta, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	initialIndex := meta.LastIndex
	assert.Greater(t, initialIndex, uint64(0),
		"Initial LastIndex must be > 0")

	// Launch a goroutine that updates the key after a delay
	done := make(chan struct{})
	go func() {
		time.Sleep(500 * time.Millisecond)
		client.KV().Put(&api.KVPair{
			Key:   key,
			Value: []byte("updated"),
		}, nil)
		close(done)
	}()

	// Blocking query — should wait until the update happens
	pair, meta2, err := client.KV().Get(key, &api.QueryOptions{
		WaitIndex: initialIndex,
		WaitTime:  5 * time.Second,
	})
	require.NoError(t, err)
	require.NotNil(t, pair)

	// Index should have advanced
	assert.Greater(t, meta2.LastIndex, initialIndex,
		"LastIndex must advance after blocking query detects change")
	assert.Equal(t, []byte("updated"), pair.Value,
		"Blocking query must return updated value")

	<-done
	client.KV().Delete(key, nil)
}

// CK-012: Test KV CreateIndex immutability
func TestKVCreateIndexImmutable(t *testing.T) {
	client := getClient(t)
	key := "ck012/createindex/" + randomID()

	// Put
	_, err := client.KV().Put(&api.KVPair{
		Key:   key,
		Value: []byte("v1"),
	}, nil)
	require.NoError(t, err)

	pair1, _, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair1)
	createIndex := pair1.CreateIndex

	// Update
	_, err = client.KV().Put(&api.KVPair{
		Key:   key,
		Value: []byte("v2"),
	}, nil)
	require.NoError(t, err)

	pair2, _, err := client.KV().Get(key, nil)
	require.NoError(t, err)
	require.NotNil(t, pair2)

	// CreateIndex must NOT change after update
	assert.Equal(t, createIndex, pair2.CreateIndex,
		"CreateIndex must be immutable after creation")
	// ModifyIndex must change
	assert.Greater(t, pair2.ModifyIndex, pair1.ModifyIndex,
		"ModifyIndex must increase after update")

	client.KV().Delete(key, nil)
}

// CK-013: Test KV keys with separator
func TestKVKeysWithSeparator(t *testing.T) {
	client := getClient(t)
	prefix := "ck013/sep/" + randomID() + "/"

	// Create hierarchical keys: prefix/a, prefix/b, prefix/sub/x, prefix/sub/y
	for _, key := range []string{"a", "b", "sub/x", "sub/y"} {
		_, err := client.KV().Put(&api.KVPair{
			Key:   prefix + key,
			Value: []byte("v"),
		}, nil)
		require.NoError(t, err)
	}

	// Keys with separator "/" — should return top-level keys + prefixes
	keys, _, err := client.KV().Keys(prefix, "/", nil)
	require.NoError(t, err)
	// Should return: prefix/a, prefix/b, prefix/sub/
	assert.Contains(t, keys, prefix+"a", "Should contain key 'a'")
	assert.Contains(t, keys, prefix+"b", "Should contain key 'b'")
	assert.Contains(t, keys, prefix+"sub/", "Should contain prefix 'sub/'")
	// Should NOT contain sub-keys directly
	assert.NotContains(t, keys, prefix+"sub/x",
		"Sub-keys should be collapsed into prefix")

	client.KV().DeleteTree(prefix, nil)
}
