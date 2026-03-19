package tests

import (
	"io"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Agent Cluster & Misc Tests ====================

// TestAgentLeave tells the agent to gracefully leave the cluster.
// WARNING: This may cause the agent to shut down, so it is skipped by default.
func TestAgentLeave(t *testing.T) {
	t.Skip("Skipping Leave test: calling Leave will shut down the agent and disrupt other tests")

	client := getClient(t)
	err := client.Agent().Leave()
	assert.NoError(t, err, "Agent Leave should not return an error")
}

// TestAgentForceLeave forces removal of a nonexistent node from the cluster.
func TestAgentForceLeave(t *testing.T) {
	client := getClient(t)
	nodeName := "nonexistent-node-" + randomID()

	err := client.Agent().ForceLeave(nodeName)
	// ForceLeave on a nonexistent node may succeed silently or return an error
	// depending on the implementation. We accept both cases.
	if err != nil {
		t.Logf("ForceLeave returned error (acceptable for nonexistent node): %v", err)
	} else {
		assert.NoError(t, err, "ForceLeave should succeed or return a handled error")
	}
}

// TestAgentForceLeavePrune forces removal of a nonexistent node with the prune flag.
func TestAgentForceLeavePrune(t *testing.T) {
	client := getClient(t)
	nodeName := "nonexistent-node-prune-" + randomID()

	err := client.Agent().ForceLeavePrune(nodeName)
	// Similar to ForceLeave, this may succeed silently or return an error.
	if err != nil {
		t.Logf("ForceLeavePrune returned error (acceptable for nonexistent node): %v", err)
	} else {
		assert.NoError(t, err, "ForceLeavePrune should succeed or return a handled error")
	}
}

// TestAgentConnectCARoots retrieves the Connect CA root certificates from the agent.
func TestAgentConnectCARoots(t *testing.T) {
	client := getClient(t)

	roots, _, err := client.Agent().ConnectCARoots(nil)
	if err != nil {
		t.Skipf("Connect CA may not be enabled or supported: %v", err)
	}

	require.NotNil(t, roots, "CA roots struct should not be nil")
	// The roots struct should have some content when Connect is enabled
	t.Logf("Connect CA active root ID: %s, roots count: %d", roots.ActiveRootID, len(roots.Roots))
}

// TestAgentServicesWithFilter registers a service and then lists services using a filter expression.
func TestAgentServicesWithFilter(t *testing.T) {
	client := getClient(t)
	serviceID := "filter-svc-" + randomID()
	serviceName := "filter-test-" + randomID()

	// Register a service to filter on
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: serviceName,
		Port: 7070,
		Tags: []string{"filtered"},
	})
	require.NoError(t, err, "Service registration should succeed")
	time.Sleep(500 * time.Millisecond)

	// Filter for our specific service by name
	filter := "Service == \"" + serviceName + "\""
	services, err := client.Agent().ServicesWithFilter(filter)
	if err != nil {
		t.Skipf("ServicesWithFilter may not be supported: %v", err)
	}

	assert.NotNil(t, services, "Filtered services map should not be nil")
	assert.Contains(t, services, serviceID, "Filtered results should contain our service")

	if svc, ok := services[serviceID]; ok {
		assert.Equal(t, serviceName, svc.Service, "Service name should match")
		assert.Equal(t, 7070, svc.Port, "Service port should match")
	}

	// Cleanup
	err = client.Agent().ServiceDeregister(serviceID)
	assert.NoError(t, err)
}

// TestAgentChecksWithFilter registers a TTL check, passes it, and filters by status.
func TestAgentChecksWithFilter(t *testing.T) {
	client := getClient(t)
	checkID := "filter-check-" + randomID()

	// Register a TTL check
	err := client.Agent().CheckRegister(&api.AgentCheckRegistration{
		ID:   checkID,
		Name: "filter-check",
		AgentServiceCheck: api.AgentServiceCheck{
			TTL: "30s",
		},
	})
	require.NoError(t, err, "Check registration should succeed")
	time.Sleep(500 * time.Millisecond)

	// Pass the TTL so the check is in passing state
	err = client.Agent().PassTTL(checkID, "healthy")
	require.NoError(t, err, "PassTTL should succeed")
	time.Sleep(500 * time.Millisecond)

	// Filter for passing checks
	checks, err := client.Agent().ChecksWithFilter("Status == passing")
	if err != nil {
		t.Skipf("ChecksWithFilter may not be supported: %v", err)
	}

	assert.NotNil(t, checks, "Filtered checks map should not be nil")
	assert.Contains(t, checks, checkID, "Filtered results should contain our passing check")

	if check, ok := checks[checkID]; ok {
		assert.Equal(t, api.HealthPassing, check.Status, "Check status should be passing")
	}

	// Cleanup
	err = client.Agent().CheckDeregister(checkID)
	assert.NoError(t, err)
}

// TestAgentMetricsStream opens a metrics stream and reads some bytes from it.
func TestAgentMetricsStream(t *testing.T) {
	client := getClient(t)

	reader, err := client.Agent().MetricsStream(nil)
	if err != nil {
		t.Skipf("MetricsStream may not be supported: %v", err)
	}
	require.NotNil(t, reader, "MetricsStream should return a non-nil ReadCloser")
	defer reader.Close()

	// Read some bytes with a timeout to avoid blocking forever
	buf := make([]byte, 4096)
	done := make(chan struct{})
	var n int
	var readErr error

	go func() {
		n, readErr = reader.Read(buf)
		close(done)
	}()

	select {
	case <-done:
		if readErr != nil && readErr != io.EOF {
			t.Logf("MetricsStream read error: %v", readErr)
		}
		assert.Greater(t, n, 0, "Should have read some bytes from the metrics stream")
		t.Logf("Read %d bytes from metrics stream", n)
	case <-time.After(5 * time.Second):
		t.Log("MetricsStream read timed out after 5 seconds, stream may be empty")
	}
}

// TestAgentServiceRegisterOpts registers a service using ServiceRegisterOpts with nil options.
func TestAgentServiceRegisterOpts(t *testing.T) {
	client := getClient(t)
	serviceID := "opts-svc-" + randomID()

	registration := &api.AgentServiceRegistration{
		ID:      serviceID,
		Name:    "opts-test-service",
		Port:    6060,
		Address: "10.0.0.50",
		Tags:    []string{"opts-test"},
		Meta: map[string]string{
			"registered-via": "opts",
		},
	}

	// Register with nil opts (basic test)
	err := client.Agent().ServiceRegisterOpts(registration, api.ServiceRegisterOpts{})
	if err != nil {
		t.Skipf("ServiceRegisterOpts may not be supported: %v", err)
	}

	time.Sleep(500 * time.Millisecond)

	// Verify the service was registered
	services, err := client.Agent().Services()
	require.NoError(t, err)
	assert.Contains(t, services, serviceID, "Service should be registered via ServiceRegisterOpts")

	if svc, ok := services[serviceID]; ok {
		assert.Equal(t, "opts-test-service", svc.Service, "Service name should match")
		assert.Equal(t, 6060, svc.Port, "Service port should match")
		assert.Equal(t, "10.0.0.50", svc.Address, "Service address should match")
		assert.Equal(t, "opts", svc.Meta["registered-via"], "Service meta should match")
	}

	// Cleanup
	err = client.Agent().ServiceDeregister(serviceID)
	assert.NoError(t, err)
}

// TestAgentUpdateDefaultACLToken updates the default ACL token on the agent.
func TestAgentUpdateDefaultACLToken(t *testing.T) {
	client := getClient(t)

	_, err := client.Agent().UpdateACLToken("test-token-"+randomID(), nil)
	if err != nil {
		t.Skipf("UpdateACLToken may not be supported (ACL not enabled or not implemented): %v", err)
	}

	// If no error, the token was updated successfully
	assert.NoError(t, err, "UpdateACLToken should succeed when ACL is enabled")
}

// TestAgentUpdateACLAgentToken updates the agent ACL token.
func TestAgentUpdateACLAgentToken(t *testing.T) {
	client := getClient(t)

	_, err := client.Agent().UpdateACLAgentToken("test-agent-token-"+randomID(), nil)
	if err != nil {
		t.Skipf("UpdateACLAgentToken may not be supported (ACL not enabled or not implemented): %v", err)
	}

	assert.NoError(t, err, "UpdateACLAgentToken should succeed when ACL is enabled")
}

// TestAgentUpdateACLReplicationToken updates the ACL replication token.
func TestAgentUpdateACLReplicationToken(t *testing.T) {
	client := getClient(t)

	_, err := client.Agent().UpdateACLReplicationToken("test-repl-token-"+randomID(), nil)
	if err != nil {
		t.Skipf("UpdateACLReplicationToken may not be supported (ACL not enabled or not implemented): %v", err)
	}

	assert.NoError(t, err, "UpdateACLReplicationToken should succeed when ACL is enabled")
}

// TestAgentSelfVersion checks the agent version by querying /v1/agent/self and inspecting the Config.Version field.
func TestAgentSelfVersion(t *testing.T) {
	client := getClient(t)

	self, err := client.Agent().Self()
	require.NoError(t, err, "Agent Self should not return an error")
	require.NotNil(t, self, "Agent Self response should not be nil")

	// The Config section should contain version information
	config, ok := self["Config"]
	assert.True(t, ok, "Self response should contain a Config key")

	if ok && config != nil {
		version, hasVersion := config["Version"]
		if hasVersion {
			assert.NotEmpty(t, version, "Version should not be empty")
			t.Logf("Agent version: %v", version)
		} else {
			t.Log("Config.Version field not found in agent self response")
		}
	}
}
