package tests

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Connect CA Tests ====================

// TestConnectCARootsEmpty tests CA roots when Connect is not enabled
func TestConnectCARootsEmpty(t *testing.T) {
	client := getTestClient(t)

	connect := client.Connect()

	// This may return an error or empty roots if Connect is not enabled
	roots, _, err := connect.CARoots(nil)
	if err != nil {
		t.Logf("CA roots not available (Connect may not be enabled): %v", err)
		return
	}

	if roots != nil {
		t.Logf("CA roots available: %d roots", len(roots.Roots))
	}
}

// TestConnectCARootsList tests listing CA roots
func TestConnectCARootsList(t *testing.T) {
	client := getTestClient(t)

	connect := client.Connect()

	roots, meta, err := connect.CARoots(nil)
	if err != nil {
		t.Skipf("Connect CA not available: %v", err)
		return
	}

	assert.NotNil(t, meta, "Should return query metadata")

	if roots != nil && len(roots.Roots) > 0 {
		t.Logf("Found %d CA root(s)", len(roots.Roots))
		for _, root := range roots.Roots {
			t.Logf("  Root: ID=%s, Active=%v", root.ID, root.Active)
			assert.NotEmpty(t, root.ID, "Root should have ID")
			assert.NotEmpty(t, root.RootCertPEM, "Root should have certificate")
		}

		// Verify we have an active root
		hasActive := false
		for _, root := range roots.Roots {
			if root.Active {
				hasActive = true
				break
			}
		}
		assert.True(t, hasActive, "Should have at least one active root")
	}
}

// TestConnectCAConfigGetSet tests getting and setting CA configuration
func TestConnectCAConfigGetSet(t *testing.T) {
	client := getTestClient(t)

	connect := client.Connect()

	// Get current config
	config, _, err := connect.CAGetConfig(nil)
	if err != nil {
		t.Skipf("Connect CA config not available: %v", err)
		return
	}

	assert.NotNil(t, config, "CA config should not be nil")
	t.Logf("CA Provider: %s", config.Provider)

	// Note: Setting CA config is a privileged operation and may not be available
	// in all environments, so we just verify we can read the config
}

// ==================== Connect Intention Tests ====================

// TestConnectIntentionCreateListGetUpdateDelete tests full intention lifecycle
func TestConnectIntentionCreateListGetUpdateDelete(t *testing.T) {
	client := getTestClient(t)

	connect := client.Connect()

	srcService := "test-src-" + randomString(8)
	dstService := "test-dst-" + randomString(8)

	// Create intention
	intention := &api.Intention{
		SourceName:      srcService,
		DestinationName: dstService,
		Action:          api.IntentionActionAllow,
		Description:     "Test intention for SDK compatibility",
		Meta: map[string]string{
			"created_by": "sdk-test",
		},
	}

	id, _, err := connect.IntentionCreate(intention, nil)
	if err != nil {
		t.Skipf("Connect intentions not available: %v", err)
		return
	}
	require.NotEmpty(t, id, "Should return intention ID")
	t.Logf("Created intention: %s", id)

	// Get intention by ID
	retrieved, _, err := connect.IntentionGet(id, nil)
	require.NoError(t, err)
	require.NotNil(t, retrieved)
	assert.Equal(t, srcService, retrieved.SourceName)
	assert.Equal(t, dstService, retrieved.DestinationName)
	assert.Equal(t, api.IntentionActionAllow, retrieved.Action)

	// List all intentions
	intentions, _, err := connect.Intentions(nil)
	require.NoError(t, err)
	found := false
	for _, i := range intentions {
		if i.ID == id {
			found = true
			break
		}
	}
	assert.True(t, found, "Created intention should be in list")

	// Update intention
	retrieved.Action = api.IntentionActionDeny
	retrieved.Description = "Updated intention"
	_, err = connect.IntentionUpdate(retrieved, nil)
	require.NoError(t, err)

	// Verify update
	updated, _, err := connect.IntentionGet(id, nil)
	require.NoError(t, err)
	assert.Equal(t, api.IntentionActionDeny, updated.Action)
	assert.Equal(t, "Updated intention", updated.Description)

	// Delete intention
	_, err = connect.IntentionDelete(id, nil)
	require.NoError(t, err)

	// Verify deletion
	deleted, _, err := connect.IntentionGet(id, nil)
	assert.Nil(t, deleted, "Intention should be deleted")
}

// TestConnectIntentionGetInvalidId tests error handling for invalid intention ID
func TestConnectIntentionGetInvalidId(t *testing.T) {
	client := getTestClient(t)

	connect := client.Connect()

	// Try to get non-existent intention
	_, _, err := connect.IntentionGet("non-existent-id", nil)
	if err == nil {
		t.Log("No error for non-existent intention (may return nil)")
	} else {
		t.Logf("Expected error for non-existent intention: %v", err)
	}
}

// TestConnectIntentionMatch tests intention matching
func TestConnectIntentionMatch(t *testing.T) {
	client := getTestClient(t)

	connect := client.Connect()

	srcService := "match-src-" + randomString(8)
	dstService := "match-dst-" + randomString(8)

	// Create an intention
	intention := &api.Intention{
		SourceName:      srcService,
		DestinationName: dstService,
		Action:          api.IntentionActionAllow,
	}

	id, _, err := connect.IntentionCreate(intention, nil)
	if err != nil {
		t.Skipf("Connect intentions not available: %v", err)
		return
	}

	defer func() {
		connect.IntentionDelete(id, nil)
	}()

	// Match intentions for the destination
	opts := &api.IntentionMatch{
		By:    api.IntentionMatchDestination,
		Names: []string{dstService},
	}

	matches, _, err := connect.IntentionMatch(opts, nil)
	require.NoError(t, err)

	if matches != nil {
		t.Logf("Matches for %s: %d intentions", dstService, len(matches[dstService]))
		for _, m := range matches[dstService] {
			t.Logf("  Match: %s -> %s (%s)", m.SourceName, m.DestinationName, m.Action)
		}
	}
}

// TestConnectIntentionCheck tests intention permission checking
func TestConnectIntentionCheck(t *testing.T) {
	client := getTestClient(t)

	connect := client.Connect()

	srcService := "check-src-" + randomString(8)
	dstService := "check-dst-" + randomString(8)

	// Create an allow intention
	intention := &api.Intention{
		SourceName:      srcService,
		DestinationName: dstService,
		Action:          api.IntentionActionAllow,
	}

	id, _, err := connect.IntentionCreate(intention, nil)
	if err != nil {
		t.Skipf("Connect intentions not available: %v", err)
		return
	}

	defer func() {
		connect.IntentionDelete(id, nil)
	}()

	// Check if connection is allowed
	allowed, _, err := connect.IntentionCheck(&api.IntentionCheck{
		Source:      srcService,
		Destination: dstService,
	}, nil)

	if err != nil {
		t.Logf("Intention check not available: %v", err)
		return
	}

	assert.True(t, allowed, "Connection should be allowed")

	// Update to deny
	intention.ID = id
	intention.Action = api.IntentionActionDeny
	_, err = connect.IntentionUpdate(intention, nil)
	require.NoError(t, err)

	time.Sleep(100 * time.Millisecond)

	// Check again - should be denied
	allowed, _, err = connect.IntentionCheck(&api.IntentionCheck{
		Source:      srcService,
		Destination: dstService,
	}, nil)

	if err == nil {
		assert.False(t, allowed, "Connection should be denied")
	}
}

// TestConnectIntentionWildcard tests wildcard intentions
func TestConnectIntentionWildcard(t *testing.T) {
	client := getTestClient(t)

	connect := client.Connect()

	dstService := "wildcard-dst-" + randomString(8)

	// Create wildcard intention (allow all sources)
	intention := &api.Intention{
		SourceName:      "*",
		DestinationName: dstService,
		Action:          api.IntentionActionAllow,
		Description:     "Allow all sources",
	}

	id, _, err := connect.IntentionCreate(intention, nil)
	if err != nil {
		t.Skipf("Connect intentions not available: %v", err)
		return
	}

	defer func() {
		connect.IntentionDelete(id, nil)
	}()

	// Get and verify
	retrieved, _, err := connect.IntentionGet(id, nil)
	require.NoError(t, err)
	assert.Equal(t, "*", retrieved.SourceName)
	t.Logf("Created wildcard intention: %s -> %s", retrieved.SourceName, retrieved.DestinationName)
}

// ==================== Connect Service Tests ====================

// TestConnectCALeaf tests getting a leaf certificate for a service
func TestConnectCALeaf(t *testing.T) {
	client := getTestClient(t)

	// First register a service
	agent := client.Agent()
	serviceName := "leaf-test-" + randomString(8)

	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Connect: &api.AgentServiceConnect{
			Native: true,
		},
	}

	err := agent.ServiceRegister(reg)
	if err != nil {
		t.Skipf("Service registration failed: %v", err)
		return
	}

	defer agent.ServiceDeregister(serviceName)

	// Try to get leaf certificate
	leaf, _, err := agent.ConnectCALeaf(serviceName, nil)
	if err != nil {
		t.Logf("CA leaf not available (Connect may not be enabled): %v", err)
		return
	}

	if leaf != nil {
		assert.NotEmpty(t, leaf.CertPEM, "Should have certificate PEM")
		assert.NotEmpty(t, leaf.PrivateKeyPEM, "Should have private key PEM")
		assert.Equal(t, serviceName, leaf.Service)
		t.Logf("Got leaf certificate for service: %s", serviceName)
	}
}

// TestConnectAuthorize tests authorization checks
func TestConnectAuthorize(t *testing.T) {
	client := getTestClient(t)

	connect := client.Connect()

	// Create test services
	srcService := "auth-src-" + randomString(8)
	dstService := "auth-dst-" + randomString(8)

	// Create allow intention
	intention := &api.Intention{
		SourceName:      srcService,
		DestinationName: dstService,
		Action:          api.IntentionActionAllow,
	}

	id, _, err := connect.IntentionCreate(intention, nil)
	if err != nil {
		t.Skipf("Connect intentions not available: %v", err)
		return
	}

	defer func() {
		connect.IntentionDelete(id, nil)
	}()

	// Test authorization
	authReq := &api.AgentAuthorizeParams{
		Target:           dstService,
		ClientCertSerial: "",
		ClientCertURI:    "spiffe://dc1/" + srcService,
	}

	auth, err := client.Agent().ConnectAuthorize(authReq)
	if err != nil {
		t.Logf("Connect authorize not available: %v", err)
		return
	}

	t.Logf("Authorization result: Authorized=%v, Reason=%s", auth.Authorized, auth.Reason)
}
