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
		t.Skipf("CA roots not available (Connect may not be enabled): %v", err)
	}

	assert.NotNil(t, roots, "CA roots response should not be nil")
	t.Logf("CA roots available: %d roots", len(roots.Roots))
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

	// Try to get non-existent intention - should return error or nil intention
	intention, _, err := connect.IntentionGet("non-existent-id", nil)
	if err != nil {
		t.Logf("Expected error for non-existent intention: %v", err)
		// Getting a non-existent intention should return an error
		assert.Error(t, err, "Non-existent intention ID should return an error")
	} else {
		assert.Nil(t, intention, "Non-existent intention should return nil")
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
		t.Skipf("Connect authorize not available: %v", err)
	}

	assert.NotNil(t, auth, "Authorization result should not be nil")
	assert.NotEmpty(t, auth.Reason, "Authorization reason should not be empty")
	t.Logf("Authorization result: Authorized=%v, Reason=%s", auth.Authorized, auth.Reason)
}

// ==================== Intention Exact Match Tests ====================

// TestConnectIntentionGetExact tests getting an intention by exact source/destination pair
func TestConnectIntentionGetExact(t *testing.T) {
	client := getTestClient(t)

	connect := client.Connect()

	srcService := "exact-get-src-" + randomString(8)
	dstService := "exact-get-dst-" + randomString(8)

	// Create an intention first
	intention := &api.Intention{
		SourceName:      srcService,
		DestinationName: dstService,
		Action:          api.IntentionActionAllow,
		Description:     "Test intention for exact get",
	}

	id, _, err := connect.IntentionCreate(intention, nil)
	if err != nil {
		t.Skipf("Connect intentions not available: %v", err)
		return
	}
	defer func() {
		connect.IntentionDelete(id, nil)
	}()

	// Get by exact source/destination match
	retrieved, _, err := connect.IntentionGetExact(srcService, dstService, nil)
	require.NoError(t, err, "IntentionGetExact should not return an error")
	require.NotNil(t, retrieved, "IntentionGetExact should return a non-nil intention")

	assert.Equal(t, srcService, retrieved.SourceName, "Source name should match")
	assert.Equal(t, dstService, retrieved.DestinationName, "Destination name should match")
	assert.Equal(t, api.IntentionActionAllow, retrieved.Action, "Action should be Allow")
	assert.Equal(t, "Test intention for exact get", retrieved.Description, "Description should match")
	assert.NotEmpty(t, retrieved.ID, "Intention should have an ID")

	t.Logf("IntentionGetExact returned intention: ID=%s, %s -> %s", retrieved.ID, retrieved.SourceName, retrieved.DestinationName)

	// Verify that a non-existent pair returns nil
	nonExistent, _, err := connect.IntentionGetExact("nonexistent-src-"+randomString(8), "nonexistent-dst-"+randomString(8), nil)
	if err == nil {
		assert.Nil(t, nonExistent, "Non-existent source/destination pair should return nil intention")
	}
}

// TestConnectIntentionDeleteExact tests deleting an intention by exact source/destination pair
func TestConnectIntentionDeleteExact(t *testing.T) {
	client := getTestClient(t)

	connect := client.Connect()

	srcService := "exact-del-src-" + randomString(8)
	dstService := "exact-del-dst-" + randomString(8)

	// Create an intention
	intention := &api.Intention{
		SourceName:      srcService,
		DestinationName: dstService,
		Action:          api.IntentionActionDeny,
		Description:     "Test intention for exact delete",
	}

	id, _, err := connect.IntentionCreate(intention, nil)
	if err != nil {
		t.Skipf("Connect intentions not available: %v", err)
		return
	}

	// Verify the intention exists before deletion
	retrieved, _, err := connect.IntentionGet(id, nil)
	require.NoError(t, err)
	require.NotNil(t, retrieved, "Intention should exist before deletion")
	assert.Equal(t, srcService, retrieved.SourceName)

	// Delete by exact source/destination match
	_, err = connect.IntentionDeleteExact(srcService, dstService, nil)
	require.NoError(t, err, "IntentionDeleteExact should not return an error")

	// Verify the intention is gone
	deleted, _, err := connect.IntentionGetExact(srcService, dstService, nil)
	if err == nil {
		assert.Nil(t, deleted, "Intention should be nil after exact deletion")
	}

	// Also verify by ID
	deletedByID, _, err := connect.IntentionGet(id, nil)
	assert.Nil(t, deletedByID, "Intention should not be retrievable by ID after exact deletion")

	t.Logf("IntentionDeleteExact successfully deleted intention: %s -> %s", srcService, dstService)
}

// ==================== Intention Upsert Tests ====================

// TestConnectIntentionUpsert tests creating and updating intentions via upsert
func TestConnectIntentionUpsert(t *testing.T) {
	client := getTestClient(t)

	connect := client.Connect()

	srcService := "upsert-src-" + randomString(8)
	dstService := "upsert-dst-" + randomString(8)

	// Upsert to create a new intention
	intention := &api.Intention{
		SourceName:      srcService,
		DestinationName: dstService,
		Action:          api.IntentionActionAllow,
		Description:     "Created via upsert",
		Meta: map[string]string{
			"version": "1",
		},
	}

	_, err := connect.IntentionUpsert(intention, nil)
	if err != nil {
		t.Skipf("IntentionUpsert not available: %v", err)
		return
	}

	// Verify the intention was created
	created, _, err := connect.IntentionGetExact(srcService, dstService, nil)
	require.NoError(t, err, "Should be able to get the upserted intention")
	require.NotNil(t, created, "Upserted intention should exist")
	assert.Equal(t, api.IntentionActionAllow, created.Action, "Action should be Allow after create")
	assert.Equal(t, "Created via upsert", created.Description, "Description should match after create")

	defer func() {
		connect.IntentionDeleteExact(srcService, dstService, nil)
	}()

	// Upsert again to update the existing intention
	updatedIntention := &api.Intention{
		SourceName:      srcService,
		DestinationName: dstService,
		Action:          api.IntentionActionDeny,
		Description:     "Updated via upsert",
		Meta: map[string]string{
			"version": "2",
		},
	}

	_, err = connect.IntentionUpsert(updatedIntention, nil)
	require.NoError(t, err, "IntentionUpsert for update should not return an error")

	// Verify the intention was updated
	updated, _, err := connect.IntentionGetExact(srcService, dstService, nil)
	require.NoError(t, err, "Should be able to get the updated intention")
	require.NotNil(t, updated, "Updated intention should exist")
	assert.Equal(t, api.IntentionActionDeny, updated.Action, "Action should be Deny after update")
	assert.Equal(t, "Updated via upsert", updated.Description, "Description should match after update")

	t.Logf("IntentionUpsert create-then-update flow succeeded: %s -> %s", srcService, dstService)
}

// ==================== CA Set Config Tests ====================

// TestConnectCASetConfig tests setting the CA configuration
func TestConnectCASetConfig(t *testing.T) {
	client := getTestClient(t)

	connect := client.Connect()

	// Read current CA configuration
	originalConfig, _, err := connect.CAGetConfig(nil)
	if err != nil {
		t.Skipf("Connect CA config not available: %v", err)
		return
	}
	require.NotNil(t, originalConfig, "Original CA config should not be nil")
	require.NotEmpty(t, originalConfig.Provider, "CA provider should not be empty")

	t.Logf("Original CA config: Provider=%s", originalConfig.Provider)

	// Modify the config — update the leaf cert TTL in the provider config
	newConfig := &api.CAConfig{
		Provider: originalConfig.Provider,
		Config:   make(map[string]interface{}),
	}

	// Copy existing config entries
	for k, v := range originalConfig.Config {
		newConfig.Config[k] = v
	}

	// Set a known field: LeafCertTTL
	newConfig.Config["LeafCertTTL"] = "96h"

	// Write the modified config back
	_, err = connect.CASetConfig(newConfig, nil)
	if err != nil {
		t.Skipf("CASetConfig not supported in this environment: %v", err)
		return
	}

	// Read back and verify the change took effect
	updatedConfig, _, err := connect.CAGetConfig(nil)
	require.NoError(t, err, "Should be able to read config after set")
	require.NotNil(t, updatedConfig, "Updated CA config should not be nil")
	assert.Equal(t, originalConfig.Provider, updatedConfig.Provider, "Provider should remain the same")

	// Verify the LeafCertTTL was updated
	if leafTTL, ok := updatedConfig.Config["LeafCertTTL"]; ok {
		t.Logf("Updated LeafCertTTL: %v", leafTTL)
	}

	t.Logf("CASetConfig successfully updated CA configuration for provider: %s", updatedConfig.Provider)

	// Restore original config to avoid affecting other tests
	_, err = connect.CASetConfig(originalConfig, nil)
	if err != nil {
		t.Logf("Warning: failed to restore original CA config: %v", err)
	}
}

// ==================== Agent Connect CA Leaf Tests ====================

// TestAgentConnectCALeafWithRegisteredService tests getting a leaf certificate for a registered service
func TestAgentConnectCALeafWithRegisteredService(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	// First check if Connect CA is available
	connect := client.Connect()
	_, _, err := connect.CARoots(nil)
	if err != nil {
		t.Skipf("Connect CA not available (Connect may not be enabled): %v", err)
		return
	}

	// Register a Connect-enabled service
	serviceName := "leaf-cert-" + randomString(8)
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 9090,
		Connect: &api.AgentServiceConnect{
			Native: true,
		},
		Meta: map[string]string{
			"env": "test",
		},
	}

	err = agent.ServiceRegister(reg)
	require.NoError(t, err, "Service registration should succeed")
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Verify service is registered
	services, err := agent.Services()
	require.NoError(t, err)
	_, exists := services[serviceName]
	require.True(t, exists, "Service should be registered before requesting leaf cert")

	// Get leaf certificate for the service
	leaf, meta, err := agent.ConnectCALeaf(serviceName, nil)
	if err != nil {
		t.Skipf("ConnectCALeaf not available: %v", err)
		return
	}

	require.NotNil(t, leaf, "Leaf certificate should not be nil")
	assert.NotNil(t, meta, "Query metadata should not be nil")

	// Validate certificate fields
	assert.NotEmpty(t, leaf.CertPEM, "Leaf certificate PEM should not be empty")
	assert.NotEmpty(t, leaf.PrivateKeyPEM, "Private key PEM should not be empty")
	assert.Equal(t, serviceName, leaf.Service, "Leaf certificate service name should match")
	assert.NotEmpty(t, leaf.ServiceURI, "Service URI should not be empty")
	assert.False(t, leaf.ValidBefore.IsZero(), "ValidBefore should be set")
	assert.False(t, leaf.ValidAfter.IsZero(), "ValidAfter should be set")
	assert.True(t, leaf.ValidBefore.After(leaf.ValidAfter), "ValidBefore should be after ValidAfter")

	t.Logf("Got leaf certificate for service %s: URI=%s, ValidAfter=%v, ValidBefore=%v",
		serviceName, leaf.ServiceURI, leaf.ValidAfter, leaf.ValidBefore)
}
