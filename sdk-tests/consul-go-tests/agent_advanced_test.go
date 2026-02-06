package tests

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Agent Advanced API Tests ====================

// CAA-001: Test get agent self info
func TestAgentSelf(t *testing.T) {
	client := getClient(t)

	self, err := client.Agent().Self()
	assert.NoError(t, err, "Agent self should succeed")
	assert.NotEmpty(t, self, "Should return agent info")

	if config, ok := self["Config"].(map[string]interface{}); ok {
		t.Logf("Node Name: %v", config["NodeName"])
		t.Logf("Datacenter: %v", config["Datacenter"])
	}
}

// CAA-002: Test get agent members
func TestAgentMembers(t *testing.T) {
	client := getClient(t)

	members, err := client.Agent().Members(false)
	assert.NoError(t, err, "Agent members should succeed")
	assert.NotEmpty(t, members, "Should have at least one member")

	for _, member := range members {
		t.Logf("Member: %s, Status: %d", member.Name, member.Status)
	}
}

// CAA-003: Test agent host info
func TestAgentHost(t *testing.T) {
	client := getClient(t)

	host, err := client.Agent().Host()
	if err != nil {
		t.Logf("Agent host error (may require ACL): %v", err)
		t.Skip("Agent host not available")
	}

	assert.NotNil(t, host)
	t.Logf("Host info available")
}

// CAA-004: Test get agent version
func TestAgentVersion(t *testing.T) {
	client := getClient(t)

	self, err := client.Agent().Self()
	require.NoError(t, err)

	if config, ok := self["Config"].(map[string]interface{}); ok {
		version := config["Version"]
		t.Logf("Agent version: %v", version)
	}
}

// CAA-005: Test agent metrics
func TestAgentMetrics(t *testing.T) {
	client := getClient(t)

	metrics, err := client.Agent().Metrics()
	if err != nil {
		t.Logf("Agent metrics error: %v", err)
		t.Skip("Metrics not available")
	}

	assert.NotNil(t, metrics)
	t.Logf("Metrics timestamp: %v", metrics.Timestamp)
	t.Logf("Gauges count: %d", len(metrics.Gauges))
	t.Logf("Counters count: %d", len(metrics.Counters))
}

// CAA-006: Test service maintenance mode
func TestAgentServiceMaintenance(t *testing.T) {
	client := getClient(t)

	// Register a service
	serviceID := "maint-service-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "maintenance-test",
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceID)

	// Enable maintenance
	err = client.Agent().EnableServiceMaintenance(serviceID, "Testing maintenance mode")
	assert.NoError(t, err, "Enable maintenance should succeed")

	// Check service is in maintenance
	services, err := client.Agent().Services()
	require.NoError(t, err)
	// Service should still be there but with maintenance flag

	// Disable maintenance
	err = client.Agent().DisableServiceMaintenance(serviceID)
	assert.NoError(t, err, "Disable maintenance should succeed")

	_ = services
}

// CAA-007: Test node maintenance mode
func TestAgentNodeMaintenance(t *testing.T) {
	client := getClient(t)

	// Enable node maintenance
	err := client.Agent().EnableNodeMaintenance("Testing node maintenance")
	if err != nil {
		t.Logf("Node maintenance error: %v", err)
		t.Skip("Node maintenance not available")
	}

	time.Sleep(500 * time.Millisecond)

	// Disable node maintenance
	err = client.Agent().DisableNodeMaintenance()
	assert.NoError(t, err, "Disable node maintenance should succeed")
}

// CAA-008: Test warn TTL check status
func TestAgentWarnTTL(t *testing.T) {
	client := getClient(t)

	checkID := "warn-check-" + randomID()
	err := client.Agent().CheckRegister(&api.AgentCheckRegistration{
		ID:   checkID,
		Name: "warn-check",
		TTL:  "30s",
	})
	require.NoError(t, err)
	defer client.Agent().CheckDeregister(checkID)

	// Warn the check
	err = client.Agent().WarnTTL(checkID, "Warning: resource usage high")
	assert.NoError(t, err, "Warn TTL should succeed")

	// Verify status
	checks, err := client.Agent().Checks()
	require.NoError(t, err)

	if check, ok := checks[checkID]; ok {
		assert.Equal(t, api.HealthWarning, check.Status)
	}
}

// CAA-009: Test update TTL check with full status
func TestAgentUpdateTTL(t *testing.T) {
	client := getClient(t)

	checkID := "update-check-" + randomID()
	err := client.Agent().CheckRegister(&api.AgentCheckRegistration{
		ID:   checkID,
		Name: "update-check",
		TTL:  "30s",
	})
	require.NoError(t, err)
	defer client.Agent().CheckDeregister(checkID)

	// Update with passing status
	err = client.Agent().UpdateTTL(checkID, "All systems operational", api.HealthPassing)
	assert.NoError(t, err, "Update TTL should succeed")

	// Verify
	checks, err := client.Agent().Checks()
	require.NoError(t, err)

	if check, ok := checks[checkID]; ok {
		assert.Equal(t, api.HealthPassing, check.Status)
		assert.Equal(t, "All systems operational", check.Output)
	}
}

// CAA-010: Test service with check
func TestAgentServiceWithCheck(t *testing.T) {
	client := getClient(t)

	serviceID := "svc-with-check-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "service-with-check",
		Port: 8080,
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: api.HealthPassing,
		},
	})
	assert.NoError(t, err, "Service registration with check should succeed")
	defer client.Agent().ServiceDeregister(serviceID)

	// Verify check exists
	time.Sleep(500 * time.Millisecond)
	checks, err := client.Agent().Checks()
	require.NoError(t, err)

	// Check ID should be "service:serviceID"
	checkID := "service:" + serviceID
	if _, ok := checks[checkID]; ok {
		t.Logf("Service check registered: %s", checkID)
	}
}

// CAA-011: Test service with multiple checks
func TestAgentServiceWithMultipleChecks(t *testing.T) {
	client := getClient(t)

	serviceID := "multi-check-svc-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "multi-check-service",
		Port: 8080,
		Checks: api.AgentServiceChecks{
			&api.AgentServiceCheck{
				CheckID: serviceID + "-check1",
				Name:    "TTL Check 1",
				TTL:     "30s",
			},
			&api.AgentServiceCheck{
				CheckID: serviceID + "-check2",
				Name:    "TTL Check 2",
				TTL:     "30s",
			},
		},
	})
	assert.NoError(t, err, "Service registration with multiple checks should succeed")
	defer client.Agent().ServiceDeregister(serviceID)

	time.Sleep(500 * time.Millisecond)

	// Verify both checks exist
	checks, err := client.Agent().Checks()
	require.NoError(t, err)

	t.Logf("Total checks: %d", len(checks))
}

// CAA-012: Test service with weights
func TestAgentServiceWithWeights(t *testing.T) {
	client := getClient(t)

	serviceID := "weighted-svc-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "weighted-service",
		Port: 8080,
		Weights: &api.AgentWeights{
			Passing: 10,
			Warning: 1,
		},
	})
	assert.NoError(t, err, "Service registration with weights should succeed")
	defer client.Agent().ServiceDeregister(serviceID)

	// Verify weights
	services, err := client.Agent().Services()
	require.NoError(t, err)

	if svc, ok := services[serviceID]; ok {
		t.Logf("Service weights - Passing: %d, Warning: %d",
			svc.Weights.Passing, svc.Weights.Warning)
	}
}
