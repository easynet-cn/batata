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

// CK-001: Test KV put
func TestKVPut(t *testing.T) {
	client := getClient(t)
	key := "ck001/put/" + randomID()
	value := []byte("test-value")

	// Put
	_, err := client.KV().Put(&api.KVPair{
		Key:   key,
		Value: value,
	}, nil)
	assert.NoError(t, err, "KV put should succeed")

	// Verify
	pair, _, err := client.KV().Get(key, nil)
	assert.NoError(t, err)
	assert.NotNil(t, pair)
	assert.Equal(t, value, pair.Value)

	// Cleanup
	client.KV().Delete(key, nil)
}

// CK-002: Test KV get
func TestKVGet(t *testing.T) {
	client := getClient(t)
	key := "ck002/get/" + randomID()
	value := []byte("get-test-value")

	// Put first
	_, err := client.KV().Put(&api.KVPair{
		Key:   key,
		Value: value,
	}, nil)
	require.NoError(t, err)

	// Get
	pair, _, err := client.KV().Get(key, nil)
	assert.NoError(t, err, "KV get should succeed")
	assert.NotNil(t, pair)
	assert.Equal(t, key, pair.Key)
	assert.Equal(t, value, pair.Value)

	// Get non-existent key
	pair, _, err = client.KV().Get("non-existent-key-"+randomID(), nil)
	assert.NoError(t, err)
	assert.Nil(t, pair, "Non-existent key should return nil")

	// Cleanup
	client.KV().Delete(key, nil)
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
	require.NotNil(t, pair)

	// Delete
	_, err = client.KV().Delete(key, nil)
	assert.NoError(t, err, "KV delete should succeed")

	// Verify deleted
	pair, _, err = client.KV().Get(key, nil)
	assert.NoError(t, err)
	assert.Nil(t, pair, "Key should be deleted")
}

// ==================== P1: Important Tests ====================

// CK-004: Test KV list by prefix
func TestKVList(t *testing.T) {
	client := getClient(t)
	prefix := "ck004/list/" + randomID() + "/"

	// Put multiple keys
	for i := 0; i < 5; i++ {
		key := fmt.Sprintf("%skey%d", prefix, i)
		_, err := client.KV().Put(&api.KVPair{
			Key:   key,
			Value: []byte(fmt.Sprintf("value%d", i)),
		}, nil)
		require.NoError(t, err)
	}

	// List by prefix
	pairs, _, err := client.KV().List(prefix, nil)
	assert.NoError(t, err, "KV list should succeed")
	assert.Len(t, pairs, 5, "Should have 5 keys")

	// Verify all keys
	for i, pair := range pairs {
		assert.Contains(t, pair.Key, prefix)
		assert.NotEmpty(t, pair.Value)
		_ = i
	}

	// Cleanup
	for i := 0; i < 5; i++ {
		client.KV().Delete(fmt.Sprintf("%skey%d", prefix, i), nil)
	}
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
	keys, _, err := client.KV().Keys(prefix, "", nil)
	assert.NoError(t, err, "KV keys should succeed")
	assert.Len(t, keys, 3, "Should have 3 keys")

	for _, key := range keys {
		assert.Contains(t, key, prefix)
	}

	// Cleanup
	for i := 0; i < 3; i++ {
		client.KV().Delete(fmt.Sprintf("%skey%d", prefix, i), nil)
	}
}

// CK-006: Test KV CAS (Compare-And-Swap)
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

	modifyIndex := pair.ModifyIndex

	// CAS with correct index - should succeed
	pair.Value = []byte("updated")
	pair.ModifyIndex = modifyIndex
	success, _, err := client.KV().CAS(pair, nil)
	assert.NoError(t, err)
	assert.True(t, success, "CAS with correct index should succeed")

	// Verify update
	pair, _, err = client.KV().Get(key, nil)
	assert.NoError(t, err)
	assert.Equal(t, []byte("updated"), pair.Value)

	// CAS with wrong index - should fail
	oldPair := &api.KVPair{
		Key:         key,
		Value:       []byte("should-fail"),
		ModifyIndex: modifyIndex, // Old index
	}
	success, _, err = client.KV().CAS(oldPair, nil)
	assert.NoError(t, err)
	assert.False(t, success, "CAS with old index should fail")

	// Cleanup
	client.KV().Delete(key, nil)
}

// ==================== P2: Nice to Have Tests ====================

// CK-007: Test KV acquire (distributed lock)
func TestKVAcquire(t *testing.T) {
	client := getClient(t)
	key := "ck007/lock/" + randomID()

	// Create session for locking
	session := client.Session()
	sessionID, _, err := session.Create(&api.SessionEntry{
		Name:     "test-lock-session",
		TTL:      "30s",
		Behavior: api.SessionBehaviorDelete,
	}, nil)
	require.NoError(t, err)
	defer session.Destroy(sessionID, nil)

	// Acquire lock
	pair := &api.KVPair{
		Key:     key,
		Value:   []byte("locked"),
		Session: sessionID,
	}
	acquired, _, err := client.KV().Acquire(pair, nil)
	assert.NoError(t, err, "Acquire should succeed")
	assert.True(t, acquired, "Should acquire lock")

	// Verify lock holder
	pair, _, err = client.KV().Get(key, nil)
	assert.NoError(t, err)
	assert.Equal(t, sessionID, pair.Session, "Session should hold the lock")

	// Try to acquire with different session - should fail
	sessionID2, _, err := session.Create(&api.SessionEntry{
		Name: "test-lock-session-2",
		TTL:  "30s",
	}, nil)
	require.NoError(t, err)
	defer session.Destroy(sessionID2, nil)

	pair2 := &api.KVPair{
		Key:     key,
		Value:   []byte("locked-by-2"),
		Session: sessionID2,
	}
	acquired2, _, err := client.KV().Acquire(pair2, nil)
	assert.NoError(t, err)
	assert.False(t, acquired2, "Second session should NOT acquire lock")

	// Cleanup
	client.KV().Release(pair, nil)
	client.KV().Delete(key, nil)
}

// CK-008: Test KV release (unlock)
func TestKVRelease(t *testing.T) {
	client := getClient(t)
	key := "ck008/release/" + randomID()

	// Create session
	session := client.Session()
	sessionID, _, err := session.Create(&api.SessionEntry{
		Name: "test-release-session",
		TTL:  "30s",
	}, nil)
	require.NoError(t, err)
	defer session.Destroy(sessionID, nil)

	// Acquire
	pair := &api.KVPair{
		Key:     key,
		Value:   []byte("locked"),
		Session: sessionID,
	}
	acquired, _, err := client.KV().Acquire(pair, nil)
	require.NoError(t, err)
	require.True(t, acquired)

	// Release
	released, _, err := client.KV().Release(pair, nil)
	assert.NoError(t, err, "Release should succeed")
	assert.True(t, released, "Should release lock")

	// Verify released
	pair, _, err = client.KV().Get(key, nil)
	assert.NoError(t, err)
	if pair != nil {
		assert.Empty(t, pair.Session, "Lock should be released")
	}

	// Cleanup
	client.KV().Delete(key, nil)
}

// CK-009: Test KV with flags
func TestKVWithFlags(t *testing.T) {
	client := getClient(t)
	key := "ck009/flags/" + randomID()
	flags := uint64(12345)

	// Put with flags
	_, err := client.KV().Put(&api.KVPair{
		Key:   key,
		Value: []byte("value-with-flags"),
		Flags: flags,
	}, nil)
	assert.NoError(t, err)

	// Get and verify flags
	pair, _, err := client.KV().Get(key, nil)
	assert.NoError(t, err)
	assert.NotNil(t, pair)
	assert.Equal(t, flags, pair.Flags, "Flags should be preserved")

	// Cleanup
	client.KV().Delete(key, nil)
}

// Test KV tree delete
func TestKVDeleteTree(t *testing.T) {
	client := getClient(t)
	prefix := "ck-tree/" + randomID() + "/"

	// Put multiple keys
	for i := 0; i < 5; i++ {
		key := fmt.Sprintf("%skey%d", prefix, i)
		_, err := client.KV().Put(&api.KVPair{
			Key:   key,
			Value: []byte("value"),
		}, nil)
		require.NoError(t, err)
	}

	// Delete tree
	_, err := client.KV().DeleteTree(prefix, nil)
	assert.NoError(t, err, "DeleteTree should succeed")

	time.Sleep(500 * time.Millisecond)

	// Verify all deleted
	pairs, _, err := client.KV().List(prefix, nil)
	assert.NoError(t, err)
	assert.Empty(t, pairs, "All keys should be deleted")
}
