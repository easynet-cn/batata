package tests

import (
	"strings"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Operator Raft Tests ====================

// TestOperatorRaftConfiguration tests getting Raft configuration
func TestOperatorRaftConfiguration(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	config, err := operator.RaftGetConfiguration(nil)
	require.NoError(t, err, "RaftGetConfiguration should succeed")
	require.NotNil(t, config, "Raft configuration should not be nil")

	assert.NotEmpty(t, config.Servers, "Raft configuration should have at least one server")

	for _, server := range config.Servers {
		assert.NotEmpty(t, server.ID, "Server ID should not be empty")
		assert.NotEmpty(t, server.Node, "Server Node should not be empty")
		assert.NotEmpty(t, server.Address, "Server Address should not be empty")
		t.Logf("Server: ID=%s, Node=%s, Address=%s, Leader=%v, Voter=%v",
			server.ID, server.Node, server.Address, server.Leader, server.Voter)
	}
}

// TestOperatorRaftLeader tests getting Raft leader
func TestOperatorRaftLeader(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	config, err := operator.RaftGetConfiguration(nil)
	require.NoError(t, err, "RaftGetConfiguration should succeed")
	require.NotNil(t, config, "Raft configuration should not be nil")

	leaderFound := false
	for _, server := range config.Servers {
		if server.Leader {
			leaderFound = true
			assert.NotEmpty(t, server.Node, "Leader node name should not be empty")
			assert.NotEmpty(t, server.Address, "Leader address should not be empty")
			assert.True(t, server.Voter, "Leader should be a voter")
			t.Logf("Raft leader: %s at %s", server.Node, server.Address)
			break
		}
	}

	assert.True(t, leaderFound, "A Raft leader should be found in the cluster")
}

// TestOperatorRaftRemovePeerByAddress tests removing peer by address
func TestOperatorRaftRemovePeerByAddress(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Try to remove a non-existent peer (should fail)
	err := operator.RaftRemovePeerByAddress("192.168.99.99:8300", nil)
	assert.Error(t, err, "Removing a non-existent peer by address should return an error")
	t.Logf("Remove peer by address error (expected): %v", err)
}

// TestOperatorRaftRemovePeerByID tests removing peer by ID
func TestOperatorRaftRemovePeerByID(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Try to remove a non-existent peer ID (should fail)
	err := operator.RaftRemovePeerByID("nonexistent-peer-id", nil)
	assert.Error(t, err, "Removing a non-existent peer by ID should return an error")
	t.Logf("Remove peer by ID error (expected): %v", err)
}

// ==================== Operator Autopilot Tests ====================

// TestOperatorAutopilotGetConfiguration tests getting Autopilot configuration
func TestOperatorAutopilotGetConfiguration(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	config, err := operator.AutopilotGetConfiguration(nil)
	require.NoError(t, err, "AutopilotGetConfiguration should succeed")
	require.NotNil(t, config, "Autopilot configuration should not be nil")

	// Verify reasonable defaults
	assert.True(t, config.CleanupDeadServers, "CleanupDeadServers should default to true")
	assert.True(t, config.LastContactThreshold.Duration() > 0, "LastContactThreshold should be positive")
	assert.True(t, config.MaxTrailingLogs > 0, "MaxTrailingLogs should be greater than 0")
	assert.True(t, config.ServerStabilizationTime.Duration() > 0, "ServerStabilizationTime should be positive")

	t.Logf("Autopilot config: CleanupDeadServers=%v, LastContactThreshold=%v, MaxTrailingLogs=%d",
		config.CleanupDeadServers, config.LastContactThreshold, config.MaxTrailingLogs)
	t.Logf("ServerStabilizationTime=%v, RedundancyZoneTag=%s, DisableUpgradeMigration=%v",
		config.ServerStabilizationTime, config.RedundancyZoneTag, config.DisableUpgradeMigration)
}

// TestOperatorAutopilotSetConfiguration tests setting Autopilot configuration
func TestOperatorAutopilotSetConfiguration(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Get current configuration
	currentConfig, err := operator.AutopilotGetConfiguration(nil)
	require.NoError(t, err, "AutopilotGetConfiguration should succeed")
	require.NotNil(t, currentConfig, "Autopilot configuration should not be nil")

	// Save original values for restoration and verification
	originalLastContact := currentConfig.LastContactThreshold

	// Try to update configuration with a modified value
	newConfig := &api.AutopilotConfiguration{
		CleanupDeadServers:      currentConfig.CleanupDeadServers,
		LastContactThreshold:    api.NewReadableDuration(500 * time.Millisecond),
		MaxTrailingLogs:         currentConfig.MaxTrailingLogs,
		ServerStabilizationTime: currentConfig.ServerStabilizationTime,
		RedundancyZoneTag:       currentConfig.RedundancyZoneTag,
		DisableUpgradeMigration: currentConfig.DisableUpgradeMigration,
	}

	err = operator.AutopilotSetConfiguration(newConfig, nil)
	require.NoError(t, err, "AutopilotSetConfiguration should succeed")

	// Verify the update took effect
	updatedConfig, err := operator.AutopilotGetConfiguration(nil)
	require.NoError(t, err, "AutopilotGetConfiguration after update should succeed")
	assert.Equal(t, 500*time.Millisecond, updatedConfig.LastContactThreshold.Duration(),
		"LastContactThreshold should be updated to 500ms")

	// Restore original configuration
	restoreConfig := &api.AutopilotConfiguration{
		CleanupDeadServers:      currentConfig.CleanupDeadServers,
		LastContactThreshold:    originalLastContact,
		MaxTrailingLogs:         currentConfig.MaxTrailingLogs,
		ServerStabilizationTime: currentConfig.ServerStabilizationTime,
		RedundancyZoneTag:       currentConfig.RedundancyZoneTag,
		DisableUpgradeMigration: currentConfig.DisableUpgradeMigration,
	}
	err = operator.AutopilotSetConfiguration(restoreConfig, nil)
	assert.NoError(t, err, "Restoring original Autopilot configuration should succeed")
}

// TestOperatorAutopilotCASConfiguration tests CAS update of Autopilot configuration
func TestOperatorAutopilotCASConfiguration(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Get current configuration
	currentConfig, err := operator.AutopilotGetConfiguration(nil)
	require.NoError(t, err, "AutopilotGetConfiguration should succeed")
	require.NotNil(t, currentConfig, "Autopilot configuration should not be nil")

	// Try CAS update
	originalMaxTrailingLogs := currentConfig.MaxTrailingLogs
	currentConfig.MaxTrailingLogs = currentConfig.MaxTrailingLogs + 100

	success, err := operator.AutopilotCASConfiguration(currentConfig, &api.WriteOptions{})
	require.NoError(t, err, "AutopilotCASConfiguration should not return an error")
	assert.True(t, success, "AutopilotCASConfiguration should succeed with valid ModifyIndex")

	// Verify the change and restore
	updatedConfig, err := operator.AutopilotGetConfiguration(nil)
	require.NoError(t, err, "AutopilotGetConfiguration after CAS should succeed")
	assert.Equal(t, originalMaxTrailingLogs+100, updatedConfig.MaxTrailingLogs,
		"MaxTrailingLogs should be updated via CAS")

	// Restore
	updatedConfig.MaxTrailingLogs = originalMaxTrailingLogs
	_, err = operator.AutopilotCASConfiguration(updatedConfig, &api.WriteOptions{})
	assert.NoError(t, err, "Restoring MaxTrailingLogs should succeed")
}

// TestOperatorAutopilotServerHealth tests getting Autopilot server health
func TestOperatorAutopilotServerHealth(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	health, err := operator.AutopilotServerHealth(nil)
	require.NoError(t, err, "AutopilotServerHealth should succeed")
	require.NotNil(t, health, "Autopilot server health should not be nil")

	assert.True(t, health.Healthy, "Cluster should be healthy")
	assert.GreaterOrEqual(t, health.FailureTolerance, 0, "FailureTolerance should be >= 0")
	assert.NotEmpty(t, health.Servers, "Server health list should not be empty")

	for _, server := range health.Servers {
		assert.NotEmpty(t, server.ID, "Server ID should not be empty")
		assert.NotEmpty(t, server.Name, "Server Name should not be empty")
		assert.True(t, server.Healthy, "Server should be healthy")
		t.Logf("Server: ID=%s, Name=%s, Healthy=%v, Voter=%v, Leader=%v, LastContact=%v",
			server.ID, server.Name, server.Healthy, server.Voter, server.Leader, server.LastContact)
	}
}

// TestOperatorAutopilotState tests getting Autopilot state
func TestOperatorAutopilotState(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	state, err := operator.AutopilotState(nil)
	require.NoError(t, err, "AutopilotState should succeed")
	require.NotNil(t, state, "Autopilot state should not be nil")

	assert.True(t, state.Healthy, "Cluster should be healthy")
	assert.GreaterOrEqual(t, state.FailureTolerance, 0, "FailureTolerance should be >= 0")
	assert.NotEmpty(t, string(state.Leader), "Leader should not be empty")
	assert.NotEmpty(t, state.Voters, "Voters list should not be empty")

	t.Logf("Autopilot state: Healthy=%v, FailureTolerance=%d, Leader=%s, Voters=%v",
		state.Healthy, state.FailureTolerance, state.Leader, state.Voters)
}

// ==================== Operator Keyring Tests ====================

// TestOperatorKeyringList tests listing gossip encryption keys
func TestOperatorKeyringList(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	keys, err := operator.KeyringList(nil)
	if err != nil {
		t.Skipf("Keyring list not available (encryption may be disabled): %v", err)
	}

	require.NotNil(t, keys, "Keyring list should not be nil")
	assert.NotEmpty(t, keys, "Keyring list should have at least one entry")

	for _, key := range keys {
		assert.NotEmpty(t, key.Datacenter, "Datacenter should not be empty")
		assert.NotEmpty(t, key.Keys, "Keys map should not be empty")
		t.Logf("Keyring: Datacenter=%s, Segment=%s, Keys=%v",
			key.Datacenter, key.Segment, key.Keys)
	}
}

// TestOperatorKeyringInstall tests installing a new gossip key
func TestOperatorKeyringInstall(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Use a unique key (different from TestOperatorKeyringUse which may set its key as primary)
	testKey := "X4EVZrHKLcbQpCkwJoSHnA=="

	err := operator.KeyringInstall(testKey, nil)
	if err != nil {
		t.Skipf("Keyring install not available (encryption may be disabled): %v", err)
	}

	// Verify the key was installed by listing keys
	keys, err := operator.KeyringList(nil)
	require.NoError(t, err, "KeyringList after install should succeed")

	found := false
	for _, entry := range keys {
		if _, ok := entry.Keys[testKey]; ok {
			found = true
			break
		}
	}
	assert.True(t, found, "Installed key should appear in keyring list")

	// Clean up - remove the test key
	err = operator.KeyringRemove(testKey, nil)
	assert.NoError(t, err, "Removing installed test key should succeed")
}

// TestOperatorKeyringUse tests setting a primary gossip key
func TestOperatorKeyringUse(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	testKey := "cg8BpnCK7Nm+bxqkJ7fNBQ=="

	// First install the key
	err := operator.KeyringInstall(testKey, nil)
	if err != nil {
		t.Skipf("Keyring not available (encryption may be disabled): %v", err)
	}

	// Set it as primary
	err = operator.KeyringUse(testKey, nil)
	require.NoError(t, err, "KeyringUse should succeed for an installed key")

	// Verify the key is listed (it should have usage count reflecting primary status)
	keys, err := operator.KeyringList(nil)
	require.NoError(t, err, "KeyringList after use should succeed")

	found := false
	for _, entry := range keys {
		if count, ok := entry.Keys[testKey]; ok {
			found = true
			assert.True(t, count > 0, "Primary key should have non-zero usage count")
			break
		}
	}
	assert.True(t, found, "Primary key should appear in keyring list")

	t.Log("Keyring primary key set successfully")
}

// TestOperatorKeyringRemove tests removing a gossip key
func TestOperatorKeyringRemove(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Use a different key from TestOperatorKeyringUse (which may have set
	// "cg8BpnCK7Nm+bxqkJ7fNBQ==" as primary — can't remove primary)
	testKey := "pUqJrVyVRj5jYyMvsBkiHg=="

	// First install the key so we have something to remove
	err := operator.KeyringInstall(testKey, nil)
	if err != nil {
		t.Skipf("Keyring not available (encryption may be disabled): %v", err)
	}

	// Remove the key (not primary, should succeed)
	err = operator.KeyringRemove(testKey, nil)
	require.NoError(t, err, "KeyringRemove should succeed for a non-primary installed key")

	// Verify the key was removed
	keys, err := operator.KeyringList(nil)
	require.NoError(t, err, "KeyringList after remove should succeed")

	for _, entry := range keys {
		_, exists := entry.Keys[testKey]
		assert.False(t, exists, "Removed key should not appear in keyring list")
	}

	t.Log("Keyring key removed successfully")
}

// ==================== Operator Area Tests (Enterprise) ====================

// TestOperatorAreaList tests listing network areas (Enterprise feature)
func TestOperatorAreaList(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	areas, _, err := operator.AreaList(nil)
	if err != nil {
		t.Skipf("Area list not available (Enterprise feature): %v", err)
	}

	// Enterprise feature: just verify we got a valid response
	assert.NotNil(t, areas, "Areas list should not be nil")
	t.Logf("Areas: %d", len(areas))
	for _, area := range areas {
		assert.NotEmpty(t, area.ID, "Area ID should not be empty")
		t.Logf("Area: ID=%s, PeerDatacenter=%s", area.ID, area.PeerDatacenter)
	}
}

// TestOperatorAreaCreate tests creating a network area (Enterprise feature)
func TestOperatorAreaCreate(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	area := &api.Area{
		PeerDatacenter: "test-dc-" + randomString(8),
	}

	id, _, err := operator.AreaCreate(area, nil)
	if err != nil {
		t.Skipf("Area create not available (Enterprise feature): %v", err)
	}

	assert.NotEmpty(t, id, "Created area ID should not be empty")
	t.Logf("Area created: %s", id)

	// Clean up
	_, err = operator.AreaDelete(id, nil)
	assert.NoError(t, err, "Area delete should succeed for created area")
}

// ==================== Leader Transfer Tests ====================

// TestOperatorLeaderTransfer tests leader transfer operation
func TestOperatorLeaderTransfer(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Leader transfer with empty string transfers to any eligible peer
	// In a single-node cluster this should still not error
	resp, err := operator.RaftLeaderTransfer("", nil)
	require.NoError(t, err, "Leader transfer should not return an error")
	assert.NotNil(t, resp, "Leader transfer response should not be nil")
	t.Logf("Leader transfer initiated successfully")
}

// TestOperatorLeaderTransferToNode tests leader transfer to specific node
func TestOperatorLeaderTransferToNode(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Get current configuration to find a node
	config, err := operator.RaftGetConfiguration(nil)
	require.NoError(t, err, "RaftGetConfiguration should succeed")
	require.NotNil(t, config, "Raft configuration should not be nil")

	if len(config.Servers) < 2 {
		t.Skip("Skipping leader transfer - need at least 2 nodes")
	}

	// Find a non-leader node
	var targetID string
	for _, server := range config.Servers {
		if !server.Leader && server.Voter {
			targetID = string(server.ID)
			break
		}
	}

	require.NotEmpty(t, targetID, "Should find an eligible non-leader voter for transfer target")

	resp, err := operator.RaftLeaderTransfer(targetID, nil)
	require.NoError(t, err, "Leader transfer to specific node should not error")
	assert.NotNil(t, resp, "Leader transfer response should not be nil")
	t.Logf("Leader transfer to %s initiated", targetID)
}

// ==================== Utilization Test (Enterprise-only) ====================

// TestOperatorUtilizationEnterpriseOnly verifies the utilization endpoint
// returns the Consul OSS-style Enterprise-only error (HTTP 501 with a body
// mentioning "Enterprise"). The hashicorp/consul Go SDK does not expose a
// public Utilization() method on *api.Operator, so we call the endpoint via
// raw HTTP — the same shape consul-enterprise clients would hit.
func TestOperatorUtilizationEnterpriseOnly(t *testing.T) {
	resp, body := rawRequest(t, "PUT", "/v1/operator/utilization", nil)
	require.Equal(t, 501, resp.StatusCode,
		"utilization must return 501 on OSS/Batata")
	assert.True(t,
		strings.Contains(body, "Enterprise"),
		"body should mention Enterprise, got: %s", body,
	)
}

// ==================== Segment Tests (Enterprise) ====================

// TestOperatorSegmentList tests listing network segments (Enterprise feature)
func TestOperatorSegmentList(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	segments, _, err := operator.SegmentList(nil)
	if err != nil {
		t.Skipf("Segment list not available (Enterprise feature): %v", err)
	}

	assert.NotNil(t, segments, "Segments list should not be nil")
	t.Logf("Segments: %v", segments)
}
