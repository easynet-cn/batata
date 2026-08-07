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

	config, ok := self["Config"]
	require.True(t, ok, "Self should have 'Config' section")
	require.NotNil(t, config, "Config should not be nil")
	assert.NotEmpty(t, config["NodeName"], "NodeName should not be empty")
	assert.NotEmpty(t, config["Datacenter"], "Datacenter should not be empty")
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

	config, ok := self["Config"]
	require.True(t, ok, "Self should have 'Config' section")
	require.NotNil(t, config)

	version := config["Version"]
	assert.NotEmpty(t, version, "Agent version should not be empty")
	t.Logf("Agent version: %v", version)
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

	// Check service is in maintenance (Maintenance field removed in SDK v1.33+)
	// Verify by checking health checks instead
	// Consul creates maintenance checks with ID "_service_maintenance:{serviceID}"
	health, err := client.Agent().Checks()
	require.NoError(t, err)
	checkID := "_service_maintenance:" + serviceID
	chk, ok := health[checkID]
	require.True(t, ok, "Health check for service %s should exist", serviceID)
	assert.Equal(t, api.HealthCritical, chk.Status, "Service should be in maintenance mode (critical)")

	// Disable maintenance
	err = client.Agent().DisableServiceMaintenance(serviceID)
	assert.NoError(t, err, "Disable maintenance should succeed")

	// Verify maintenance is disabled
	time.Sleep(200 * time.Millisecond)
	health, err = client.Agent().Checks()
	require.NoError(t, err)
	chk, ok = health[checkID]
	if ok {
		assert.NotEqual(t, api.HealthCritical, chk.Status, "Service should not be in maintenance mode after disable")
	}
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

	// Verify node is in maintenance
	self, err := client.Agent().Self()
	require.NoError(t, err)
	config, ok := self["Config"]
	require.True(t, ok)
	maintenanceMode, _ := config["MaintenanceMode"].(bool)
	assert.True(t, maintenanceMode, "Node should be in maintenance mode")

	// Disable node maintenance
	err = client.Agent().DisableNodeMaintenance()
	assert.NoError(t, err, "Disable node maintenance should succeed")

	// Verify maintenance is disabled
	self, err = client.Agent().Self()
	require.NoError(t, err)
	config, ok = self["Config"]
	require.True(t, ok)
	maintenanceMode, _ = config["MaintenanceMode"].(bool)
	assert.False(t, maintenanceMode, "Node should not be in maintenance mode after disable")
}

// CAA-008: Test warn TTL check status
func TestAgentWarnTTL(t *testing.T) {
	client := getClient(t)

	checkID := "warn-check-" + randomID()
	err := client.Agent().CheckRegister(&api.AgentCheckRegistration{
		ID:   checkID,
		Name: "warn-check",
		AgentServiceCheck: api.AgentServiceCheck{
			TTL: "30s",
		},
	})
	require.NoError(t, err)
	defer client.Agent().CheckDeregister(checkID)

	// Warn the check
	err = client.Agent().WarnTTL(checkID, "Warning: resource usage high")
	assert.NoError(t, err, "Warn TTL should succeed")

	// Verify status
	checks, err := client.Agent().Checks()
	require.NoError(t, err)

	check, ok := checks[checkID]
	require.True(t, ok, "Check %s should exist", checkID)
	assert.Equal(t, api.HealthWarning, check.Status)
	assert.Equal(t, "Warning: resource usage high", check.Output)
}

// CAA-009: Test update TTL check with full status
func TestAgentUpdateTTL(t *testing.T) {
	client := getClient(t)

	checkID := "update-check-" + randomID()
	err := client.Agent().CheckRegister(&api.AgentCheckRegistration{
		ID:   checkID,
		Name: "update-check",
		AgentServiceCheck: api.AgentServiceCheck{
			TTL: "30s",
		},
	})
	require.NoError(t, err)
	defer client.Agent().CheckDeregister(checkID)

	// Update with passing status
	err = client.Agent().UpdateTTL(checkID, "All systems operational", api.HealthPassing)
	assert.NoError(t, err, "Update TTL should succeed")

	// Verify
	checks, err := client.Agent().Checks()
	require.NoError(t, err)

	check, ok := checks[checkID]
	require.True(t, ok, "Check %s should exist", checkID)
	assert.Equal(t, api.HealthPassing, check.Status)
	assert.Equal(t, "All systems operational", check.Output)
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
	check, ok := checks[checkID]
	require.True(t, ok, "Service check %s should be registered", checkID)
	assert.Equal(t, api.HealthPassing, check.Status,
		"Service check should have initial passing status")
	t.Logf("Service check registered: %s", checkID)
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

	check1, ok1 := checks[serviceID+"-check1"]
	check2, ok2 := checks[serviceID+"-check2"]
	assert.True(t, ok1, "First check %s-check1 should exist", serviceID)
	assert.True(t, ok2, "Second check %s-check2 should exist", serviceID)
	if ok1 {
		assert.Equal(t, "TTL Check 1", check1.Name)
	}
	if ok2 {
		assert.Equal(t, "TTL Check 2", check2.Name)
	}
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

	svc, ok := services[serviceID]
	require.True(t, ok, "Service %s should exist", serviceID)
	require.NotNil(t, svc.Weights, "Service should have weights")
	assert.Equal(t, 10, svc.Weights.Passing, "Passing weight should be 10")
	assert.Equal(t, 1, svc.Weights.Warning, "Warning weight should be 1")
	t.Logf("Service weights - Passing: %d, Warning: %d",
		svc.Weights.Passing, svc.Weights.Warning)
}
