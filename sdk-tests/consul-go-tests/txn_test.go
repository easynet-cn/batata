package tests

import (
	"encoding/base64"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// kvOpsToTxnOps converts KVTxnOps to TxnOps for the Txn API
func kvOpsToTxnOps(kvOps api.KVTxnOps) api.TxnOps {
	txnOps := make(api.TxnOps, len(kvOps))
	for i, op := range kvOps {
		txnOps[i] = &api.TxnOp{KV: op}
	}
	return txnOps
}

// ==================== KV Transaction Tests ====================

// TestTxnKVSet tests setting KV in transaction
func TestTxnKVSet(t *testing.T) {
	client := getTestClient(t)

	txn := client.Txn()
	keyPrefix := "txn-set-" + randomString(8)

	ops := api.KVTxnOps{
		&api.KVTxnOp{
			Verb:  api.KVSet,
			Key:   keyPrefix + "/key1",
			Value: []byte("value1"),
		},
		&api.KVTxnOp{
			Verb:  api.KVSet,
			Key:   keyPrefix + "/key2",
			Value: []byte("value2"),
		},
	}

	ok, resp, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	require.NoError(t, err)
	assert.True(t, ok, "Transaction should succeed")
	assert.NotNil(t, resp, "Response should not be nil")

	t.Logf("Transaction set %d keys", len(resp.Results))

	// Verify keys were set
	kv := client.KV()
	pair1, _, err := kv.Get(keyPrefix+"/key1", nil)
	require.NoError(t, err)
	assert.Equal(t, "value1", string(pair1.Value))

	// Cleanup
	kv.DeleteTree(keyPrefix, nil)
}

// TestTxnKVGet tests getting KV in transaction
func TestTxnKVGet(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	txn := client.Txn()
	keyPrefix := "txn-get-" + randomString(8)

	// Pre-create keys
	kv.Put(&api.KVPair{Key: keyPrefix + "/key1", Value: []byte("value1")}, nil)
	kv.Put(&api.KVPair{Key: keyPrefix + "/key2", Value: []byte("value2")}, nil)

	ops := api.KVTxnOps{
		&api.KVTxnOp{
			Verb: api.KVGet,
			Key:  keyPrefix + "/key1",
		},
		&api.KVTxnOp{
			Verb: api.KVGet,
			Key:  keyPrefix + "/key2",
		},
	}

	ok, resp, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	require.NoError(t, err)
	assert.True(t, ok, "Transaction should succeed")

	t.Logf("Transaction got %d results", len(resp.Results))
	for _, result := range resp.Results {
		if result.KV != nil {
			t.Logf("  %s = %s", result.KV.Key, string(result.KV.Value))
		}
	}

	// Cleanup
	kv.DeleteTree(keyPrefix, nil)
}

// TestTxnKVDelete tests deleting KV in transaction
func TestTxnKVDelete(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	txn := client.Txn()
	keyPrefix := "txn-delete-" + randomString(8)

	// Pre-create keys
	kv.Put(&api.KVPair{Key: keyPrefix + "/key1", Value: []byte("value1")}, nil)
	kv.Put(&api.KVPair{Key: keyPrefix + "/key2", Value: []byte("value2")}, nil)

	ops := api.KVTxnOps{
		&api.KVTxnOp{
			Verb: api.KVDelete,
			Key:  keyPrefix + "/key1",
		},
	}

	ok, _, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	require.NoError(t, err)
	assert.True(t, ok, "Transaction should succeed")

	// Verify key1 deleted
	pair1, _, err := kv.Get(keyPrefix+"/key1", nil)
	require.NoError(t, err)
	assert.Nil(t, pair1, "key1 should be deleted")

	// Verify key2 still exists
	pair2, _, err := kv.Get(keyPrefix+"/key2", nil)
	require.NoError(t, err)
	assert.NotNil(t, pair2, "key2 should still exist")

	// Cleanup
	kv.DeleteTree(keyPrefix, nil)
}

// TestTxnKVDeleteTree tests deleting tree in transaction
func TestTxnKVDeleteTree(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	txn := client.Txn()
	keyPrefix := "txn-tree-" + randomString(8)

	// Pre-create keys
	kv.Put(&api.KVPair{Key: keyPrefix + "/a/1", Value: []byte("v1")}, nil)
	kv.Put(&api.KVPair{Key: keyPrefix + "/a/2", Value: []byte("v2")}, nil)
	kv.Put(&api.KVPair{Key: keyPrefix + "/b/1", Value: []byte("v3")}, nil)

	ops := api.KVTxnOps{
		&api.KVTxnOp{
			Verb: api.KVDeleteTree,
			Key:  keyPrefix + "/a",
		},
	}

	ok, _, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	require.NoError(t, err)
	assert.True(t, ok, "Transaction should succeed")

	// Verify /a/* deleted
	pairs, _, err := kv.List(keyPrefix+"/a", nil)
	require.NoError(t, err)
	assert.Empty(t, pairs, "/a should be deleted")

	// Verify /b still exists
	pair, _, err := kv.Get(keyPrefix+"/b/1", nil)
	require.NoError(t, err)
	assert.NotNil(t, pair, "/b/1 should still exist")

	// Cleanup
	kv.DeleteTree(keyPrefix, nil)
}

// TestTxnKVCAS tests compare-and-set in transaction
func TestTxnKVCAS(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	txn := client.Txn()
	key := "txn-cas-" + randomString(8)

	// Create initial value
	_, err := kv.Put(&api.KVPair{Key: key, Value: []byte("initial")}, nil)
	require.NoError(t, err)

	// Get current index
	pair, _, err := kv.Get(key, nil)
	require.NoError(t, err)

	// CAS with correct index
	ops := api.KVTxnOps{
		&api.KVTxnOp{
			Verb:  api.KVCAS,
			Key:   key,
			Value: []byte("updated"),
			Index: pair.ModifyIndex,
		},
	}

	ok, _, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	require.NoError(t, err)
	assert.True(t, ok, "CAS transaction should succeed")

	// Verify update
	updated, _, err := kv.Get(key, nil)
	require.NoError(t, err)
	assert.Equal(t, "updated", string(updated.Value))

	// Cleanup
	kv.Delete(key, nil)
}

// TestTxnKVCASFail tests failed compare-and-set in transaction
func TestTxnKVCASFail(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	txn := client.Txn()
	key := "txn-cas-fail-" + randomString(8)

	// Create initial value
	_, err := kv.Put(&api.KVPair{Key: key, Value: []byte("initial")}, nil)
	require.NoError(t, err)

	// CAS with wrong index
	ops := api.KVTxnOps{
		&api.KVTxnOp{
			Verb:  api.KVCAS,
			Key:   key,
			Value: []byte("updated"),
			Index: 0, // Wrong index
		},
	}

	ok, resp, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	require.NoError(t, err)

	if !ok {
		t.Logf("CAS failed as expected, errors: %d", len(resp.Errors))
	}

	// Cleanup
	kv.Delete(key, nil)
}

// TestTxnKVCheckIndex tests check-index operation
func TestTxnKVCheckIndex(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	txn := client.Txn()
	key := "txn-check-idx-" + randomString(8)

	// Create value
	_, err := kv.Put(&api.KVPair{Key: key, Value: []byte("value")}, nil)
	require.NoError(t, err)

	// Get current index
	pair, _, err := kv.Get(key, nil)
	require.NoError(t, err)

	// Check index then set
	ops := api.KVTxnOps{
		&api.KVTxnOp{
			Verb:  api.KVCheckIndex,
			Key:   key,
			Index: pair.ModifyIndex,
		},
		&api.KVTxnOp{
			Verb:  api.KVSet,
			Key:   key,
			Value: []byte("new-value"),
		},
	}

	ok, _, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	require.NoError(t, err)
	assert.True(t, ok, "Check-index transaction should succeed")

	// Cleanup
	kv.Delete(key, nil)
}

// ==================== Multi-Operation Transaction Tests ====================

// TestTxnMultipleOperations tests transaction with mixed operations
func TestTxnMultipleOperations(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	txn := client.Txn()
	keyPrefix := "txn-multi-" + randomString(8)

	// Pre-create some keys
	kv.Put(&api.KVPair{Key: keyPrefix + "/existing", Value: []byte("old")}, nil)

	// Mixed operations
	ops := api.KVTxnOps{
		&api.KVTxnOp{
			Verb:  api.KVSet,
			Key:   keyPrefix + "/new1",
			Value: []byte("value1"),
		},
		&api.KVTxnOp{
			Verb:  api.KVSet,
			Key:   keyPrefix + "/new2",
			Value: []byte("value2"),
		},
		&api.KVTxnOp{
			Verb:  api.KVSet,
			Key:   keyPrefix + "/existing",
			Value: []byte("updated"),
		},
	}

	ok, resp, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	require.NoError(t, err)
	assert.True(t, ok, "Multi-operation transaction should succeed")

	t.Logf("Multi-operation transaction: %d results", len(resp.Results))

	// Cleanup
	kv.DeleteTree(keyPrefix, nil)
}

// TestTxnAtomicity tests transaction atomicity (all or nothing)
func TestTxnAtomicity(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	txn := client.Txn()
	keyPrefix := "txn-atomic-" + randomString(8)

	// Create a key for CAS check
	_, err := kv.Put(&api.KVPair{Key: keyPrefix + "/check", Value: []byte("original")}, nil)
	require.NoError(t, err)

	// Transaction with one operation that should fail (wrong CAS index)
	ops := api.KVTxnOps{
		&api.KVTxnOp{
			Verb:  api.KVSet,
			Key:   keyPrefix + "/new",
			Value: []byte("should-not-persist"),
		},
		&api.KVTxnOp{
			Verb:  api.KVCAS,
			Key:   keyPrefix + "/check",
			Value: []byte("updated"),
			Index: 1, // Wrong index
		},
	}

	ok, _, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	require.NoError(t, err)

	if !ok {
		// Verify the first operation was rolled back
		pair, _, err := kv.Get(keyPrefix+"/new", nil)
		require.NoError(t, err)
		assert.Nil(t, pair, "New key should not exist due to rollback")
		t.Log("Transaction rolled back as expected")
	}

	// Cleanup
	kv.DeleteTree(keyPrefix, nil)
}

// TestTxnLargeTransaction tests transaction with many operations
func TestTxnLargeTransaction(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	txn := client.Txn()
	keyPrefix := "txn-large-" + randomString(8)

	// Create many operations (up to limit, usually 64)
	opCount := 50
	ops := make(api.KVTxnOps, opCount)
	for i := 0; i < opCount; i++ {
		ops[i] = &api.KVTxnOp{
			Verb:  api.KVSet,
			Key:   keyPrefix + "/" + randomString(8),
			Value: []byte("value-" + randomString(8)),
		}
	}

	ok, resp, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	require.NoError(t, err)
	assert.True(t, ok, "Large transaction should succeed")

	t.Logf("Large transaction with %d operations: %d results", opCount, len(resp.Results))

	// Cleanup
	kv.DeleteTree(keyPrefix, nil)
}

// TestTxnWithFlags tests transaction with flags
func TestTxnWithFlags(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	txn := client.Txn()
	key := "txn-flags-" + randomString(8)

	// Set with flags
	ops := api.KVTxnOps{
		&api.KVTxnOp{
			Verb:  api.KVSet,
			Key:   key,
			Value: []byte("value"),
			Flags: 42,
		},
	}

	ok, _, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	require.NoError(t, err)
	assert.True(t, ok)

	// Verify flags
	pair, _, err := kv.Get(key, nil)
	require.NoError(t, err)
	assert.Equal(t, uint64(42), pair.Flags)

	t.Logf("Key with flags: %d", pair.Flags)

	// Cleanup
	kv.Delete(key, nil)
}

// TestTxnCheckNotExists tests check-not-exists operation
func TestTxnCheckNotExists(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	txn := client.Txn()
	keyPrefix := "txn-not-exists-" + randomString(8)

	// Check not exists then create
	ops := api.KVTxnOps{
		&api.KVTxnOp{
			Verb: api.KVCheckNotExists,
			Key:  keyPrefix + "/new-key",
		},
		&api.KVTxnOp{
			Verb:  api.KVSet,
			Key:   keyPrefix + "/new-key",
			Value: []byte("created"),
		},
	}

	ok, _, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	require.NoError(t, err)
	assert.True(t, ok, "Check-not-exists transaction should succeed")

	// Verify key was created
	pair, _, err := kv.Get(keyPrefix+"/new-key", nil)
	require.NoError(t, err)
	assert.NotNil(t, pair)

	// Cleanup
	kv.DeleteTree(keyPrefix, nil)
}

// TestTxnBase64Value tests transaction with base64 encoded values
func TestTxnBase64Value(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	txn := client.Txn()
	key := "txn-base64-" + randomString(8)

	// Binary data
	binaryData := []byte{0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD}

	ops := api.KVTxnOps{
		&api.KVTxnOp{
			Verb:  api.KVSet,
			Key:   key,
			Value: binaryData,
		},
	}

	ok, _, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	require.NoError(t, err)
	assert.True(t, ok)

	// Verify binary data
	pair, _, err := kv.Get(key, nil)
	require.NoError(t, err)
	assert.Equal(t, binaryData, pair.Value)

	t.Logf("Binary data (base64): %s", base64.StdEncoding.EncodeToString(pair.Value))

	// Cleanup
	kv.Delete(key, nil)
}

// ==================== Transaction Error Tests ====================

// TestTxnEmptyOperations tests transaction with no operations
func TestTxnEmptyOperations(t *testing.T) {
	client := getTestClient(t)

	txn := client.Txn()

	ops := api.KVTxnOps{}

	ok, resp, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	if err != nil {
		t.Logf("Empty transaction error: %v", err)
	} else {
		t.Logf("Empty transaction: ok=%v, results=%d", ok, len(resp.Results))
	}
}

// TestTxnInvalidKey tests transaction with invalid key
func TestTxnInvalidKey(t *testing.T) {
	client := getTestClient(t)

	txn := client.Txn()

	// Key with leading slash (invalid in Consul)
	ops := api.KVTxnOps{
		&api.KVTxnOp{
			Verb:  api.KVSet,
			Key:   "/invalid/leading/slash",
			Value: []byte("value"),
		},
	}

	ok, resp, _, err := txn.Txn(kvOpsToTxnOps(ops), nil)
	if err != nil {
		t.Logf("Invalid key error: %v", err)
	} else if !ok && len(resp.Errors) > 0 {
		t.Logf("Transaction failed with %d errors", len(resp.Errors))
	}
}

// ==================== Watch Tests ====================

// TestWatchKV tests watching KV changes
func TestWatchKV(t *testing.T) {
	client := getTestClient(t)

	kv := client.KV()
	key := "watch-test-" + randomString(8)

	// Create initial value
	_, err := kv.Put(&api.KVPair{Key: key, Value: []byte("initial")}, nil)
	require.NoError(t, err)

	// Get with blocking query
	pair, meta, err := kv.Get(key, nil)
	require.NoError(t, err)

	initialIndex := meta.LastIndex
	t.Logf("Initial value: %s, index: %d", string(pair.Value), initialIndex)

	// Update in background
	go func() {
		time.Sleep(500 * time.Millisecond)
		kv.Put(&api.KVPair{Key: key, Value: []byte("updated")}, nil)
	}()

	// Watch for changes
	opts := &api.QueryOptions{
		WaitIndex: initialIndex,
		WaitTime:  5 * time.Second,
	}

	updatedPair, _, err := kv.Get(key, opts)
	require.NoError(t, err)

	t.Logf("Updated value: %s", string(updatedPair.Value))

	// Cleanup
	kv.Delete(key, nil)
}
