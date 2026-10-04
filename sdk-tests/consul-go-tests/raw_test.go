package tests

import (
	"testing"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Raw API Tests ====================

// CRAW-001: Test raw query against a known endpoint (/v1/status/leader)
func TestRawQueryKnownEndpoint(t *testing.T) {
	client := getClient(t)

	// The /v1/status/leader endpoint returns a JSON-encoded string.
	// Raw.Query decodes the body into the provided `out` interface.
	var leader string
	qm, err := client.Raw().Query("/v1/status/leader", &leader, nil)

	require.NoError(t, err, "Raw query to /v1/status/leader should succeed")
	assert.NotNil(t, qm, "QueryMeta should not be nil")
	assert.Greater(t, qm.LastIndex, uint64(0),
		"QueryMeta.LastIndex should be > 0")

	// The leader address should be in host:port format
	assert.NotEmpty(t, leader, "Leader address should not be empty")
	assert.Contains(t, leader, ":",
		"Leader address should contain ':' (host:port format)")
	t.Logf("Raw query leader: %s", leader)
}

// CRAW-002: Test raw query against a non-existent path (should error)
func TestRawQueryNonExistentPath(t *testing.T) {
	client := getClient(t)

	var out interface{}
	_, err := client.Raw().Query("/v1/this-path-does-not-exist-"+randomID(), &out, nil)

	// A non-existent endpoint should return an error (404 Not Found)
	assert.Error(t, err,
		"Raw query to non-existent path should return an error")
}

// CRAW-003: Test raw query with special characters in the endpoint path
func TestRawQueryWithSpecialChars(t *testing.T) {
	client := getClient(t)

	// Query /v1/kv/<key-with-special-chars> — the key contains dots and
	// slashes which are valid URL path characters. The Raw API should
	// handle them without panicking or URL-encoding issues.
	key := "craw003/special.key/with.dots"
	// First, put a value at this key so the raw query returns data
	_, err := client.KV().Put(&api.KVPair{
		Key:   key,
		Value: []byte("special-value"),
	}, nil)
	require.NoError(t, err, "KV put for raw query test should succeed")
	defer client.KV().Delete(key, nil)

	// Now query the raw endpoint — /v1/kv/<key> returns a JSON array
	var rawResult []interface{}
	qm, err := client.Raw().Query("/v1/kv/"+key, &rawResult, nil)

	require.NoError(t, err, "Raw query with special chars should succeed")
	assert.NotNil(t, qm, "QueryMeta should not be nil")
	// The KV endpoint returns an array with at least one element
	assert.NotEmpty(t, rawResult,
		"Raw query to existing KV key should return data")
	t.Logf("Raw query with special chars returned %d items", len(rawResult))
}
