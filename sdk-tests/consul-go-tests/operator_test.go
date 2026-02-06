package consultest

import (
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
	if err != nil {
		t.Logf("Raft configuration not available: %v", err)
		return
	}

	assert.NotNil(t, config, "Raft configuration should not be nil")
	t.Logf("Raft servers: %d", len(config.Servers))

	for _, server := range config.Servers {
		t.Logf("Server: ID=%s, Node=%s, Address=%s, Leader=%v, Voter=%v",
			server.ID, server.Node, server.Address, server.Leader, server.Voter)
	}
}

// TestOperatorRaftLeader tests getting Raft leader
func TestOperatorRaftLeader(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	config, err := operator.RaftGetConfiguration(nil)
	if err != nil {
		t.Logf("Raft configuration not available: %v", err)
		return
	}

	leaderFound := false
	for _, server := range config.Servers {
		if server.Leader {
			leaderFound = true
			t.Logf("Raft leader: %s at %s", server.Node, server.Address)
			break
		}
	}

	if !leaderFound {
		t.Log("No Raft leader found (may be single-node or no cluster)")
	}
}

// TestOperatorRaftRemovePeerByAddress tests removing peer by address
func TestOperatorRaftRemovePeerByAddress(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Try to remove a non-existent peer (should fail gracefully)
	err := operator.RaftRemovePeerByAddress("192.168.99.99:8300", nil)
	if err != nil {
		t.Logf("Remove peer by address: %v (expected for non-existent peer)", err)
	} else {
		t.Log("Remove peer by address succeeded")
	}
}

// TestOperatorRaftRemovePeerByID tests removing peer by ID
func TestOperatorRaftRemovePeerByID(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Try to remove a non-existent peer ID
	err := operator.RaftRemovePeerByID("nonexistent-peer-id", nil)
	if err != nil {
		t.Logf("Remove peer by ID: %v (expected for non-existent peer)", err)
	} else {
		t.Log("Remove peer by ID succeeded")
	}
}

// ==================== Operator Autopilot Tests ====================

// TestOperatorAutopilotGetConfiguration tests getting Autopilot configuration
func TestOperatorAutopilotGetConfiguration(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	config, err := operator.AutopilotGetConfiguration(nil)
	if err != nil {
		t.Logf("Autopilot configuration not available: %v", err)
		return
	}

	assert.NotNil(t, config, "Autopilot configuration should not be nil")
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
	if err != nil {
		t.Logf("Autopilot not available: %v", err)
		return
	}

	// Try to update configuration
	newConfig := &api.AutopilotConfiguration{
		CleanupDeadServers:      currentConfig.CleanupDeadServers,
		LastContactThreshold:    api.NewReadableDuration(500 * time.Millisecond),
		MaxTrailingLogs:         currentConfig.MaxTrailingLogs,
		ServerStabilizationTime: currentConfig.ServerStabilizationTime,
		RedundancyZoneTag:       currentConfig.RedundancyZoneTag,
		DisableUpgradeMigration: currentConfig.DisableUpgradeMigration,
	}

	err = operator.AutopilotSetConfiguration(newConfig, nil)
	if err != nil {
		t.Logf("Autopilot set configuration: %v", err)
	} else {
		t.Log("Autopilot configuration updated successfully")
	}

	// Restore original configuration
	operator.AutopilotSetConfiguration(currentConfig, nil)
}

// TestOperatorAutopilotCASConfiguration tests CAS update of Autopilot configuration
func TestOperatorAutopilotCASConfiguration(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Get current configuration
	currentConfig, qm, err := operator.AutopilotCASConfiguration(nil, nil)
	if err != nil {
		t.Logf("Autopilot CAS not available: %v", err)
		return
	}

	t.Logf("Current config index: %d", qm.LastIndex)

	// Try CAS update with stale index (should fail)
	currentConfig.MaxTrailingLogs = currentConfig.MaxTrailingLogs + 100
	success, _, err := operator.AutopilotCASConfiguration(currentConfig, &api.WriteOptions{})
	if err != nil {
		t.Logf("Autopilot CAS update: %v", err)
	} else {
		t.Logf("Autopilot CAS update success: %v", success)
	}
}

// TestOperatorAutopilotServerHealth tests getting Autopilot server health
func TestOperatorAutopilotServerHealth(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	health, err := operator.AutopilotServerHealth(nil)
	if err != nil {
		t.Logf("Autopilot server health not available: %v", err)
		return
	}

	assert.NotNil(t, health, "Autopilot server health should not be nil")
	t.Logf("Cluster healthy: %v, FailureTolerance: %d", health.Healthy, health.FailureTolerance)

	for _, server := range health.Servers {
		t.Logf("Server: ID=%s, Name=%s, Healthy=%v, Voter=%v, Leader=%v, LastContact=%v",
			server.ID, server.Name, server.Healthy, server.Voter, server.Leader, server.LastContact)
	}
}

// TestOperatorAutopilotState tests getting Autopilot state
func TestOperatorAutopilotState(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	state, err := operator.AutopilotState(nil)
	if err != nil {
		t.Logf("Autopilot state not available: %v", err)
		return
	}

	assert.NotNil(t, state, "Autopilot state should not be nil")
	t.Logf("Autopilot state: Healthy=%v, FailureTolerance=%d, Leader=%s",
		state.Healthy, state.FailureTolerance, state.Leader)
}

// ==================== Operator Keyring Tests ====================

// TestOperatorKeyringList tests listing gossip encryption keys
func TestOperatorKeyringList(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	keys, err := operator.KeyringList(nil)
	if err != nil {
		t.Logf("Keyring list not available (encryption may be disabled): %v", err)
		return
	}

	assert.NotNil(t, keys, "Keyring list should not be nil")
	t.Logf("Keyring entries: %d", len(keys))

	for _, key := range keys {
		t.Logf("Keyring: Datacenter=%s, Segment=%s, Keys=%v",
			key.Datacenter, key.Segment, key.Keys)
	}
}

// TestOperatorKeyringInstall tests installing a new gossip key
func TestOperatorKeyringInstall(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Try to install a test key (32 bytes base64 encoded)
	testKey := "cg8BpnCK7Nm+bxqkJ7fNBQ==" // Valid 16-byte key base64 encoded

	err := operator.KeyringInstall(testKey, nil)
	if err != nil {
		t.Logf("Keyring install: %v (encryption may be disabled)", err)
	} else {
		t.Log("Keyring key installed successfully")
		// Clean up - remove the test key
		operator.KeyringRemove(testKey, nil)
	}
}

// TestOperatorKeyringUse tests setting a primary gossip key
func TestOperatorKeyringUse(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	testKey := "cg8BpnCK7Nm+bxqkJ7fNBQ=="

	err := operator.KeyringUse(testKey, nil)
	if err != nil {
		t.Logf("Keyring use: %v (encryption may be disabled or key not installed)", err)
	} else {
		t.Log("Keyring primary key set successfully")
	}
}

// TestOperatorKeyringRemove tests removing a gossip key
func TestOperatorKeyringRemove(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	testKey := "cg8BpnCK7Nm+bxqkJ7fNBQ=="

	err := operator.KeyringRemove(testKey, nil)
	if err != nil {
		t.Logf("Keyring remove: %v (encryption may be disabled or key not found)", err)
	} else {
		t.Log("Keyring key removed successfully")
	}
}

// ==================== Operator Area Tests (Enterprise) ====================

// TestOperatorAreaList tests listing network areas (Enterprise feature)
func TestOperatorAreaList(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	areas, _, err := operator.AreaList(nil)
	if err != nil {
		t.Logf("Area list not available (Enterprise feature): %v", err)
		return
	}

	t.Logf("Areas: %d", len(areas))
	for _, area := range areas {
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
		t.Logf("Area create not available (Enterprise feature): %v", err)
		return
	}

	t.Logf("Area created: %s", id)

	// Clean up
	_, err = operator.AreaDelete(id, nil)
	if err != nil {
		t.Logf("Area delete: %v", err)
	}
}

// ==================== Operator License Tests (Enterprise) ====================

// TestOperatorLicenseGet tests getting license (Enterprise feature)
func TestOperatorLicenseGet(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	license, _, err := operator.LicenseGet(nil)
	if err != nil {
		t.Logf("License get not available (Enterprise feature): %v", err)
		return
	}

	if license != nil {
		t.Logf("License: Valid=%v, ID=%s", license.Valid, license.License.LicenseID)
	}
}

// ==================== Leader Transfer Tests ====================

// TestOperatorLeaderTransfer tests leader transfer operation
func TestOperatorLeaderTransfer(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Leader transfer to a specific node (will fail without proper cluster)
	err := operator.RaftLeaderTransfer("", nil)
	if err != nil {
		t.Logf("Leader transfer: %v (expected in single-node)", err)
	} else {
		t.Log("Leader transfer initiated successfully")
	}
}

// TestOperatorLeaderTransferToNode tests leader transfer to specific node
func TestOperatorLeaderTransferToNode(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	// Get current configuration to find a node
	config, err := operator.RaftGetConfiguration(nil)
	if err != nil {
		t.Logf("Cannot get Raft configuration: %v", err)
		return
	}

	if len(config.Servers) < 2 {
		t.Log("Skipping leader transfer - need at least 2 nodes")
		return
	}

	// Find a non-leader node
	var targetID string
	for _, server := range config.Servers {
		if !server.Leader && server.Voter {
			targetID = string(server.ID)
			break
		}
	}

	if targetID == "" {
		t.Log("No eligible transfer target found")
		return
	}

	err = operator.RaftLeaderTransfer(targetID, nil)
	if err != nil {
		t.Logf("Leader transfer to %s: %v", targetID, err)
	} else {
		t.Logf("Leader transfer to %s initiated", targetID)
	}
}

// ==================== Segment Tests (Enterprise) ====================

// TestOperatorSegmentList tests listing network segments (Enterprise feature)
func TestOperatorSegmentList(t *testing.T) {
	client := getTestClient(t)

	operator := client.Operator()

	segments, _, err := operator.SegmentList(nil)
	if err != nil {
		t.Logf("Segment list not available (Enterprise feature): %v", err)
		return
	}

	t.Logf("Segments: %v", segments)
}
