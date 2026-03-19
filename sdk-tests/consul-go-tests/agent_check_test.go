package tests

import (
	"testing"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Check Lifecycle Tests ====================

// CA-014: Test check status transitions (pass → warn → fail → pass)
func TestAgentCheckStatusTransitions(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "ca014-transition-" + randomID()
	checkID := "service:" + serviceID

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID: serviceID, Name: "transition-svc", Port: 8080,
		Check: &api.AgentServiceCheck{CheckID: checkID, TTL: "30s", Status: "passing"},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceID)

	// Pass
	err = agent.PassTTL(checkID, "healthy")
	require.NoError(t, err)
	checks, err := agent.Checks()
	require.NoError(t, err)
	if c, ok := checks[checkID]; ok {
		assert.Equal(t, "passing", c.Status)
		assert.Equal(t, "healthy", c.Output)
	}

	// Warn
	err = agent.WarnTTL(checkID, "degraded")
	require.NoError(t, err)
	checks, err = agent.Checks()
	require.NoError(t, err)
	if c, ok := checks[checkID]; ok {
		assert.Equal(t, "warning", c.Status)
		assert.Equal(t, "degraded", c.Output)
	}

	// Fail
	err = agent.FailTTL(checkID, "unhealthy")
	require.NoError(t, err)
	checks, err = agent.Checks()
	require.NoError(t, err)
	if c, ok := checks[checkID]; ok {
		assert.Equal(t, "critical", c.Status)
	}

	// Recover
	err = agent.PassTTL(checkID, "recovered")
	require.NoError(t, err)
	checks, err = agent.Checks()
	require.NoError(t, err)
	if c, ok := checks[checkID]; ok {
		assert.Equal(t, "passing", c.Status)
	}
}

// CA-015: Test UpdateTTL with explicit status values
func TestAgentCheckUpdateTTLExplicit(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "ca015-updatettl-" + randomID()
	checkID := "service:" + serviceID

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID: serviceID, Name: "updatettl-svc", Port: 8080,
		Check: &api.AgentServiceCheck{CheckID: checkID, TTL: "30s"},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceID)

	err = agent.UpdateTTL(checkID, "custom output", api.HealthPassing)
	require.NoError(t, err)
	checks, err := agent.Checks()
	require.NoError(t, err)
	if c, ok := checks[checkID]; ok {
		assert.Equal(t, "passing", c.Status)
		assert.Equal(t, "custom output", c.Output)
	}

	err = agent.UpdateTTL(checkID, "slow", api.HealthWarning)
	require.NoError(t, err)
	checks, err = agent.Checks()
	require.NoError(t, err)
	if c, ok := checks[checkID]; ok {
		assert.Equal(t, "warning", c.Status)
	}

	err = agent.UpdateTTL(checkID, "down", api.HealthCritical)
	require.NoError(t, err)
	checks, err = agent.Checks()
	require.NoError(t, err)
	if c, ok := checks[checkID]; ok {
		assert.Equal(t, "critical", c.Status)
	}
}

// CA-016: Test standalone check (not attached to service)
func TestAgentStandaloneCheck(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	checkID := "standalone-" + randomID()

	err := agent.CheckRegister(&api.AgentCheckRegistration{
		ID: checkID, Name: "standalone-ttl",
		AgentServiceCheck: api.AgentServiceCheck{TTL: "30s", Status: "passing"},
	})
	require.NoError(t, err)
	defer agent.CheckDeregister(checkID)

	checks, err := agent.Checks()
	require.NoError(t, err)
	found, ok := checks[checkID]
	assert.True(t, ok, "Standalone check should exist")
	if ok {
		assert.Equal(t, "standalone-ttl", found.Name)
		assert.Equal(t, "passing", found.Status)
		assert.Empty(t, found.ServiceID, "Standalone check has no ServiceID")
	}
}

// CA-017: Test multiple checks on one service verified individually
func TestAgentServiceMultipleChecksVerify(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "ca017-multi-" + randomID()

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID: serviceID, Name: "multichecks-svc", Port: 8080,
		Checks: api.AgentServiceChecks{
			{CheckID: serviceID + "-ttl", TTL: "30s", Status: "passing"},
			{CheckID: serviceID + "-http", HTTP: "http://localhost:8080/health", Interval: "10s", Timeout: "5s"},
		},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceID)

	checks, err := agent.Checks()
	require.NoError(t, err)

	if ttl, ok := checks[serviceID+"-ttl"]; ok {
		assert.Equal(t, serviceID, ttl.ServiceID)
		assert.Equal(t, "passing", ttl.Status)
	} else {
		t.Error("TTL check should be registered")
	}

	if http, ok := checks[serviceID+"-http"]; ok {
		assert.Equal(t, serviceID, http.ServiceID)
	} else {
		t.Error("HTTP check should be registered")
	}
}

// CA-021: Test health query with mixed check states
func TestHealthChecksMixedStates(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceName := "mixed-health-" + randomID()
	serviceID := serviceName + "-1"
	checkID := "service:" + serviceID

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID: serviceID, Name: serviceName, Port: 8080,
		Check: &api.AgentServiceCheck{CheckID: checkID, TTL: "30s", Status: "passing"},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceID)

	err = agent.FailTTL(checkID, "service down")
	require.NoError(t, err)

	health := client.Health()

	// passing=true should filter critical
	passingEntries, _, err := health.Service(serviceName, "", true, nil)
	require.NoError(t, err)
	for _, entry := range passingEntries {
		for _, check := range entry.Checks {
			assert.NotEqual(t, "critical", check.Status, "Passing filter should exclude critical")
		}
	}

	// state=critical should include our check
	critChecks, _, err := health.State("critical", nil)
	require.NoError(t, err)
	foundCritical := false
	for _, check := range critChecks {
		if check.ServiceID == serviceID {
			foundCritical = true
			assert.Equal(t, "critical", check.Status)
			assert.Equal(t, "service down", check.Output)
			break
		}
	}
	assert.True(t, foundCritical, "Critical check should appear in state query")
}

// CA-022: Test listing all agent checks
func TestAgentChecksList(t *testing.T) {
	client := getClient(t)
	agent := client.Agent()
	serviceID := "ca022-checklist-" + randomID()
	checkID := "service:" + serviceID

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID: serviceID, Name: "checklist-svc", Port: 8080,
		Check: &api.AgentServiceCheck{CheckID: checkID, TTL: "30s", Status: "passing"},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceID)

	checks, err := agent.Checks()
	require.NoError(t, err)
	assert.NotEmpty(t, checks)

	found, ok := checks[checkID]
	assert.True(t, ok, "Should find our check")
	if ok {
		assert.Equal(t, serviceID, found.ServiceID)
		assert.NotEmpty(t, found.Name)
		assert.Contains(t, []string{"passing", "warning", "critical"}, found.Status)
	}
}
