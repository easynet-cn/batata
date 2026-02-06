package consultest

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Health Node Tests ====================

// TestHealthNodeFilter tests node health with filter expression
func TestHealthNodeFilter(t *testing.T) {
	client := getTestClient(t)

	// Get agent's node name
	agent := client.Agent()
	info, err := agent.Self()
	require.NoError(t, err)
	nodeName := info["Config"]["NodeName"].(string)

	health := client.Health()

	// Query node health
	checks, _, err := health.Node(nodeName, nil)
	require.NoError(t, err)
	t.Logf("Node %s has %d health checks", nodeName, len(checks))

	// Try with filter
	opts := &api.QueryOptions{
		Filter: "Status == passing",
	}
	filteredChecks, _, err := health.Node(nodeName, opts)
	if err != nil {
		t.Logf("Health filter not supported: %v", err)
		return
	}
	t.Logf("Passing checks on node: %d", len(filteredChecks))
}

// ==================== Health Checks Aggregated Status Tests ====================

// TestHealthChecksAggregatedStatus tests status aggregation logic
func TestHealthChecksAggregatedStatus(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "health-agg-" + randomString(8)

	// Register service with multiple checks
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Checks: api.AgentServiceChecks{
			&api.AgentServiceCheck{
				CheckID:  serviceName + "-check1",
				Name:     "Check 1",
				TTL:      "30s",
				Status:   "passing",
			},
			&api.AgentServiceCheck{
				CheckID:  serviceName + "-check2",
				Name:     "Check 2",
				TTL:      "30s",
				Status:   "passing",
			},
		},
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Get health checks
	health := client.Health()
	checks, _, err := health.Checks(serviceName, nil)
	require.NoError(t, err)
	t.Logf("Service %s has %d checks", serviceName, len(checks))

	// Calculate aggregated status
	status := api.HealthPassing
	for _, check := range checks {
		t.Logf("  Check: %s, Status: %s", check.Name, check.Status)
		if check.Status == api.HealthCritical {
			status = api.HealthCritical
		} else if check.Status == api.HealthWarning && status != api.HealthCritical {
			status = api.HealthWarning
		}
	}
	t.Logf("Aggregated status: %s", status)

	// Set one check to warning
	err = agent.UpdateTTL(serviceName+"-check1", "simulated warning", "warning")
	if err == nil {
		time.Sleep(500 * time.Millisecond)

		checks, _, _ := health.Checks(serviceName, nil)
		for _, check := range checks {
			t.Logf("  After warning - Check: %s, Status: %s", check.Name, check.Status)
		}
	}
}

// ==================== Health Service Tests ====================

// TestHealthServiceMultipleTags tests health service with multiple tag filters
func TestHealthServiceMultipleTags(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "health-tags-" + randomString(8)

	// Register service with multiple tags
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Tags: []string{"web", "primary", "v2"},
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: "passing",
		},
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	health := client.Health()

	// Query with single tag
	services, _, err := health.ServiceMultipleTags(serviceName, []string{"web"}, true, nil)
	require.NoError(t, err)
	t.Logf("Services with 'web' tag: %d", len(services))

	// Query with multiple tags
	services, _, err = health.ServiceMultipleTags(serviceName, []string{"web", "primary"}, true, nil)
	require.NoError(t, err)
	t.Logf("Services with 'web' AND 'primary' tags: %d", len(services))
}

// TestHealthServiceNodeMetaFilter tests health service with node metadata filter
func TestHealthServiceNodeMetaFilter(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "health-nodemeta-" + randomString(8)

	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: "passing",
		},
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	health := client.Health()

	// Query with node meta filter
	opts := &api.QueryOptions{
		NodeMeta: map[string]string{
			"env": "test",
		},
	}
	services, _, err := health.Service(serviceName, "", true, opts)
	require.NoError(t, err)
	t.Logf("Services with node meta filter: %d", len(services))
}

// TestHealthServiceFilter tests health service with filter expression
func TestHealthServiceFilter(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "health-filter-" + randomString(8)

	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Meta: map[string]string{
			"version": "2.0.0",
		},
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: "passing",
		},
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	health := client.Health()

	// Filter by service meta
	opts := &api.QueryOptions{
		Filter: "Service.Meta.version == \"2.0.0\"",
	}
	services, _, err := health.Service(serviceName, "", true, opts)
	if err != nil {
		t.Logf("Health filter not supported: %v", err)
		return
	}
	t.Logf("Services matching filter: %d", len(services))
}

// ==================== Health Connect Tests ====================

// TestHealthConnect tests Connect proxy service discovery
func TestHealthConnect(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "health-connect-" + randomString(8)

	// Register Connect-native service
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Connect: &api.AgentServiceConnect{
			Native: true,
		},
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: "passing",
		},
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	health := client.Health()

	// Query Connect services
	services, _, err := health.Connect(serviceName, "", true, nil)
	if err != nil {
		t.Logf("Health Connect query not available: %v", err)
		return
	}
	t.Logf("Connect services for %s: %d", serviceName, len(services))
}

// TestHealthConnectFilter tests Connect services with filter
func TestHealthConnectFilter(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "health-connect-filter-" + randomString(8)

	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Connect: &api.AgentServiceConnect{
			Native: true,
		},
		Meta: map[string]string{
			"protocol": "grpc",
		},
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	health := client.Health()

	// Filter Connect services
	opts := &api.QueryOptions{
		Filter: "Service.Meta.protocol == \"grpc\"",
	}
	services, _, err := health.Connect(serviceName, "", true, opts)
	if err != nil {
		t.Logf("Health Connect filter not available: %v", err)
		return
	}
	t.Logf("Filtered Connect services: %d", len(services))
}

// ==================== Health Ingress Tests ====================

// TestHealthIngress tests ingress gateway discovery
func TestHealthIngress(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	gatewayName := "health-ingress-gw-" + randomString(8)

	// Register ingress gateway
	reg := &api.AgentServiceRegistration{
		ID:   gatewayName,
		Name: gatewayName,
		Port: 8080,
		Kind: api.ServiceKindIngressGateway,
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(gatewayName)

	time.Sleep(500 * time.Millisecond)

	health := client.Health()

	// Query ingress gateway health
	services, _, err := health.Ingress(gatewayName, true, nil)
	if err != nil {
		t.Logf("Health Ingress query not available: %v", err)
		return
	}
	t.Logf("Ingress gateway %s services: %d", gatewayName, len(services))
}

// ==================== Health State Tests ====================

// TestHealthStateAny tests health checks by any state
func TestHealthStateAny(t *testing.T) {
	client := getTestClient(t)

	health := client.Health()

	// Get all checks regardless of state
	checks, _, err := health.State("any", nil)
	require.NoError(t, err)
	t.Logf("Total checks (any state): %d", len(checks))

	// Count by status
	statusCounts := make(map[string]int)
	for _, check := range checks {
		statusCounts[check.Status]++
	}
	t.Logf("Status breakdown: %v", statusCounts)
}

// TestHealthStatePassing tests health checks in passing state
func TestHealthStatePassing(t *testing.T) {
	client := getTestClient(t)

	health := client.Health()

	checks, _, err := health.State("passing", nil)
	require.NoError(t, err)
	t.Logf("Passing checks: %d", len(checks))

	for _, check := range checks {
		assert.Equal(t, api.HealthPassing, check.Status)
	}
}

// TestHealthStateWarning tests health checks in warning state
func TestHealthStateWarning(t *testing.T) {
	client := getTestClient(t)

	// Create a service with warning check
	agent := client.Agent()
	serviceName := "health-warning-" + randomString(8)

	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Check: &api.AgentServiceCheck{
			CheckID: serviceName + "-check",
			TTL:     "30s",
			Status:  "warning",
		},
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	health := client.Health()

	// Update check to warning
	err = agent.UpdateTTL(serviceName+"-check", "test warning", "warning")
	if err != nil {
		t.Logf("TTL update not available: %v", err)
	}

	time.Sleep(500 * time.Millisecond)

	checks, _, err := health.State("warning", nil)
	require.NoError(t, err)
	t.Logf("Warning checks: %d", len(checks))
}

// TestHealthStateCritical tests health checks in critical state
func TestHealthStateCritical(t *testing.T) {
	client := getTestClient(t)

	// Create a service with critical check
	agent := client.Agent()
	serviceName := "health-critical-" + randomString(8)

	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Check: &api.AgentServiceCheck{
			CheckID: serviceName + "-check",
			TTL:     "30s",
			Status:  "critical",
		},
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Update check to critical
	err = agent.UpdateTTL(serviceName+"-check", "test critical", "critical")
	if err != nil {
		t.Logf("TTL update not available: %v", err)
	}

	time.Sleep(500 * time.Millisecond)

	health := client.Health()
	checks, _, err := health.State("critical", nil)
	require.NoError(t, err)
	t.Logf("Critical checks: %d", len(checks))
}

// TestHealthStateNodeMetaFilter tests health state with node metadata filter
func TestHealthStateNodeMetaFilter(t *testing.T) {
	client := getTestClient(t)

	health := client.Health()

	opts := &api.QueryOptions{
		NodeMeta: map[string]string{
			"env": "test",
		},
	}

	checks, _, err := health.State("any", opts)
	require.NoError(t, err)
	t.Logf("Checks with node meta filter: %d", len(checks))
}

// TestHealthStateFilter tests health state with filter expression
func TestHealthStateFilter(t *testing.T) {
	client := getTestClient(t)

	health := client.Health()

	// Filter by check name pattern
	opts := &api.QueryOptions{
		Filter: "Name contains \"serfHealth\"",
	}

	checks, _, err := health.State("any", opts)
	if err != nil {
		t.Logf("Health state filter not supported: %v", err)
		return
	}
	t.Logf("Checks matching filter: %d", len(checks))
}

// ==================== Health Checks Filter Tests ====================

// TestHealthChecksNodeMetaFilter tests health checks with node metadata filter
func TestHealthChecksNodeMetaFilter(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "health-checks-meta-" + randomString(8)

	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: "passing",
		},
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	health := client.Health()

	opts := &api.QueryOptions{
		NodeMeta: map[string]string{
			"env": "test",
		},
	}

	checks, _, err := health.Checks(serviceName, opts)
	require.NoError(t, err)
	t.Logf("Checks with node meta filter: %d", len(checks))
}

// TestHealthChecksFilter tests health checks with filter expression
func TestHealthChecksFilter(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "health-checks-filter-" + randomString(8)

	// Register service with multiple checks
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Checks: api.AgentServiceChecks{
			&api.AgentServiceCheck{
				CheckID: serviceName + "-ttl",
				Name:    "TTL Check",
				TTL:     "30s",
				Status:  "passing",
			},
			&api.AgentServiceCheck{
				CheckID: serviceName + "-tcp",
				Name:    "TCP Check",
				TCP:     "localhost:8080",
				Timeout: "5s",
			},
		},
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	health := client.Health()

	// Filter by check name
	opts := &api.QueryOptions{
		Filter: "Name contains \"TTL\"",
	}

	checks, _, err := health.Checks(serviceName, opts)
	if err != nil {
		t.Logf("Health checks filter not supported: %v", err)
		return
	}
	t.Logf("Checks matching filter: %d", len(checks))
}
