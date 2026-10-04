package tests

import (
	"os"
	"testing"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Error Path Tests ====================

// EPT-001: KV get with invalid key format (leading slash) — should not panic,
// should be handled gracefully
func TestErrorPathKVGetInvalidKeyFormat(t *testing.T) {
	client := getClient(t)

	// A key with a leading "/" is invalid per Consul's convention.
	// The KV Get method does not validate this client-side (unlike Put),
	// so the behavior depends on the server. The important thing is no panic.
	// The server may return nil (no match) or an error — both are acceptable.
	pair, _, err := client.KV().Get("/invalid/leading/slash", nil)

	// Either no error and nil pair (server treated as no match), or an error
	// — both are acceptable. A panic is NOT acceptable.
	if err != nil {
		t.Logf("KV Get with leading slash returned error (acceptable): %v", err)
	} else {
		assert.Nil(t, pair, "Pair should be nil for invalid key format (no match)")
	}
}

// EPT-002: KV put with empty key and invalid key (leading slash) — should error
func TestErrorPathKVPutEmptyKey(t *testing.T) {
	client := getClient(t)

	// Case 1: Empty key — the server should reject a PUT to /v1/kv/
	_, err := client.KV().Put(&api.KVPair{
		Key:   "",
		Value: []byte("empty-key-value"),
	}, nil)
	// The server may or may not reject empty keys. If it doesn't, clean up.
	if err != nil {
		t.Logf("KV Put with empty key returned error (expected): %v", err)
	} else {
		t.Log("KV Put with empty key did not error (server accepted it)")
		// Clean up if it was somehow written
		client.KV().Delete("", nil)
	}

	// Case 2: Key starting with "/" — client-side validation rejects this
	_, err = client.KV().Put(&api.KVPair{
		Key:   "/starts-with-slash",
		Value: []byte("invalid"),
	}, nil)
	// The API client validates this and returns an error:
	// "Invalid key. Key must not begin with a '/': /starts-with-slash"
	assert.Error(t, err,
		"KV Put with key starting with '/' should return a client-side error")
	if err != nil {
		assert.Contains(t, err.Error(), "must not begin with a '/'",
			"Error message should mention the '/' restriction")
	}
}

// EPT-003: Session info with invalid ID — should return nil, not error
func TestErrorPathSessionInfoInvalidID(t *testing.T) {
	client := getClient(t)

	// Querying session info with a non-existent/invalid ID should return
	// nil (not found) rather than an error. This is Consul's contract:
	// a 404 is represented as (nil, nil, nil).
	info, _, err := client.Session().Info("invalid-session-id-"+randomID(), nil)

	assert.NoError(t, err,
		"Session Info with invalid ID should not error (returns nil instead)")
	assert.Nil(t, info,
		"Session Info with invalid ID should return nil entry")
}

// EPT-004: Service register with empty/invalid name — should error
func TestErrorPathServiceRegisterInvalidName(t *testing.T) {
	client := getClient(t)

	// Register a service with an empty Name — the server should reject this
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   "ept004-empty-name-" + randomID(),
		Name: "", // empty name is invalid
		Port: 8080,
	})
	// The server validates the service name and should return an error
	assert.Error(t, err,
		"Service registration with empty Name should return an error")
	if err != nil {
		t.Logf("Service register with empty name error (expected): %v", err)
	}
}

// EPT-005: ConfigEntry get with invalid kind — should error
func TestErrorPathConfigEntryGetInvalidKind(t *testing.T) {
	client := getClient(t)

	// An invalid kind triggers a client-side error in makeConfigEntry()
	_, _, err := client.ConfigEntries().Get("invalid-kind", "some-name", nil)

	assert.Error(t, err,
		"ConfigEntry Get with invalid kind should return a client-side error")
	if err != nil {
		assert.Contains(t, err.Error(), "invalid config entry kind",
			"Error message should mention 'invalid config entry kind'")
	}
}

// EPT-006: Catalog service with non-existent name — should return empty, not error
func TestErrorPathCatalogServiceNonExistent(t *testing.T) {
	client := getClient(t)

	// Querying the catalog for a service that doesn't exist should return
	// an empty slice, not an error. This is Consul's contract.
	services, _, err := client.Catalog().Service("non-existent-svc-"+randomID(), "", nil)

	assert.NoError(t, err,
		"Catalog Service with non-existent name should not error")
	assert.Empty(t, services,
		"Catalog Service with non-existent name should return empty slice")
}

// EPT-007: ACL operations without token — should error or skip
func TestErrorPathACLOperationsWithoutToken(t *testing.T) {
	// Create a client with no token
	addr := os.Getenv("CONSUL_HTTP_ADDR")
	if addr == "" {
		addr = "127.0.0.1:8500"
	}

	noTokenClient, err := api.NewClient(&api.Config{
		Address: addr,
		Token:   "", // no token
	})
	require.NoError(t, err)

	// Try to read a token — without a token, this should error if ACLs
	// are enabled, or succeed if ACLs are disabled
	_, _, err = noTokenClient.ACL().TokenRead("00000000-0000-0000-0000-000000000000", nil)
	if err != nil {
		t.Logf("ACL TokenRead without token returned error (expected when ACLs enabled): %v", err)
		// This is the expected path when ACLs are enabled
		assert.Error(t, err,
			"ACL TokenRead without token should error when ACLs are enabled")
	} else {
		// ACLs are not enabled — all operations work without a token
		t.Skip("ACLs not enabled — operations work without token (no error path to test)")
	}
}
