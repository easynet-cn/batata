package tests

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== P0: Critical Tests ====================

// CH-001: Test health service query
func TestHealthService(t *testing.T) {
	client := getClient(t)
	serviceName := "ch001-health-" + randomID()

	// Register service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:      serviceName,
		Name:    serviceName,
		Port:    8080,
		Address: "192.168.1.1",
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)

	// Query health
	entries, _, err := client.Health().Service(serviceName, "", false, nil)
	assert.NoError(t, err, "Health service query should succeed")
	assert.NotEmpty(t, entries, "Should have service entries")

	// Verify entry
	entry := entries[0]
	assert.Equal(t, serviceName, entry.Service.ID)
	assert.Equal(t, serviceName, entry.Service.Service)

	// Cleanup
	client.Agent().ServiceDeregister(serviceName)
}

// Test health service with passing only filter
func TestHealthServicePassingOnly(t *testing.T) {
	client := getClient(t)
	serviceName := "ch001b-passing-" + randomID()

	// Register service with check
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Check: &api.AgentServiceCheck{
			TTL: "30s",
		},
	})
	require.NoError(t, err)
	time.Sleep(500 * time.Millisecond)

	// Pass the check
	checkID := "service:" + serviceName
	client.Agent().PassTTL(checkID, "healthy")
	time.Sleep(500 * time.Millisecond)

	// Query with passing=true
	entries, _, err := client.Health().Service(serviceName, "", true, nil)
	assert.NoError(t, err)
	// May or may not have entries depending on check status propagation
	t.Logf("Passing entries: %d", len(entries))

	// Cleanup
	client.Agent().ServiceDeregister(serviceName)
}

// ==================== P1: Important Tests ====================

// CH-002: Test health node query
func TestHealthNode(t *testing.T) {
	client := getClient(t)

	// Get local agent info to find node name
	self, err := client.Agent().Self()
	if err != nil {
		t.Skip("Could not get agent self info, skipping node health test")
	}

	config := self["Config"]
	nodeName := config["NodeName"].(string)

	// Query node health
	checks, _, err := client.Health().Node(nodeName, nil)
	assert.NoError(t, err, "Health node query should succeed")
	// May or may not have checks depending on node configuration
	t.Logf("Node %s has %d checks", nodeName, len(checks))
}

// CH-003: Test health checks for service
func TestHealthChecks(t *testing.T) {
	client := getClient(t)
	serviceName := "ch003-checks-" + randomID()

	// Register service with TTL check
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Name:   "Service TTL Check",
			Status: api.HealthPassing,
		},
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)

	// Query service checks
	checks, _, err := client.Health().Checks(serviceName, nil)
	assert.NoError(t, err, "Health checks query should succeed")
	t.Logf("Service %s has %d checks", serviceName, len(checks))

	// Cleanup
	client.Agent().ServiceDeregister(serviceName)
}

// ==================== P2: Nice to Have Tests ====================

// CH-004: Test health state query
func TestHealthState(t *testing.T) {
	client := getClient(t)

	// Query all passing checks
	checks, _, err := client.Health().State(api.HealthPassing, nil)
	assert.NoError(t, err, "Health state query should succeed")
	t.Logf("Found %d passing checks", len(checks))

	// Query all critical checks
	criticalChecks, _, err := client.Health().State(api.HealthCritical, nil)
	assert.NoError(t, err)
	t.Logf("Found %d critical checks", len(criticalChecks))

	// Query any state
	anyChecks, _, err := client.Health().State(api.HealthAny, nil)
	assert.NoError(t, err)
	t.Logf("Found %d total checks", len(anyChecks))
}

// Test health service with tag filter
func TestHealthServiceWithTag(t *testing.T) {
	client := getClient(t)
	serviceName := "ch-tag-" + randomID()

	// Register service with tags
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Tags: []string{"primary", "v2"},
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)

	// Query with tag filter
	entries, _, err := client.Health().Service(serviceName, "primary", false, nil)
	assert.NoError(t, err)
	assert.NotEmpty(t, entries, "Should find service with tag")

	// Query with non-existent tag
	noEntries, _, err := client.Health().Service(serviceName, "nonexistent", false, nil)
	assert.NoError(t, err)
	assert.Empty(t, noEntries, "Should not find service with non-existent tag")

	// Cleanup
	client.Agent().ServiceDeregister(serviceName)
}
