package tests

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== P0: Critical Tests ====================

// CH-001: Test health service query with full response validation
func TestHealthService(t *testing.T) {
	client := getClient(t)
	serviceName := "ch001-health-" + randomID()

	// Register service with metadata
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:      serviceName,
		Name:    serviceName,
		Port:    8080,
		Address: "192.168.1.1",
		Tags:    []string{"web", "v2"},
		Meta:    map[string]string{"version": "2.0"},
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)
	defer client.Agent().ServiceDeregister(serviceName)

	// Query health
	entries, meta, err := client.Health().Service(serviceName, "", false, nil)
	require.NoError(t, err, "Health service query should succeed")
	require.NotEmpty(t, entries, "Must have at least one entry")

	// QueryMeta validation
	assert.Greater(t, meta.LastIndex, uint64(0),
		"QueryMeta.LastIndex must be > 0")
	assert.True(t, meta.KnownLeader,
		"QueryMeta.KnownLeader must be true")

	// Validate entry structure
	entry := entries[0]

	// Node validation
	assert.NotEmpty(t, entry.Node.Node, "Node.Node must be set")
	assert.NotEmpty(t, entry.Node.Address, "Node.Address must be set")
	assert.NotEmpty(t, entry.Node.Datacenter, "Node.Datacenter must be set")

	// Service validation
	assert.Equal(t, serviceName, entry.Service.ID,
		"Service.ID must match")
	assert.Equal(t, serviceName, entry.Service.Service,
		"Service.Service (name) must match")
	assert.Equal(t, 8080, entry.Service.Port,
		"Service.Port must match")
	assert.Equal(t, "192.168.1.1", entry.Service.Address,
		"Service.Address must match")
	assert.Contains(t, entry.Service.Tags, "web",
		"Service.Tags must contain 'web'")
	assert.Contains(t, entry.Service.Tags, "v2",
		"Service.Tags must contain 'v2'")
	assert.Equal(t, "2.0", entry.Service.Meta["version"],
		"Service.Meta must contain version")

	// Tags must be a non-nil slice (Consul never returns null)
	assert.NotNil(t, entry.Service.Tags,
		"Service.Tags must never be nil (always [] or populated)")

	// Checks validation
	assert.NotNil(t, entry.Checks,
		"Checks must not be nil")
}

// CH-002: Test ?passing filter excludes non-passing checks
func TestHealthServicePassingFilter(t *testing.T) {
	client := getClient(t)
	serviceName := "ch002-passing-" + randomID()

	// Register service with TTL check (starts critical by default)
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: api.HealthCritical,
		},
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)
	defer client.Agent().ServiceDeregister(serviceName)

	// Query with passing=false — should include critical
	allEntries, _, err := client.Health().Service(serviceName, "", false, nil)
	require.NoError(t, err)
	require.NotEmpty(t, allEntries,
		"Should find service when passing=false")

	// Query with passing=true — critical service must be excluded
	passingEntries, _, err := client.Health().Service(serviceName, "", true, nil)
	require.NoError(t, err)
	assert.Empty(t, passingEntries,
		"Critical service must be excluded when passing=true")

	// Now pass the check
	checkID := "service:" + serviceName
	err = client.Agent().PassTTL(checkID, "healthy")
	require.NoError(t, err)
	time.Sleep(1 * time.Second)

	// Query with passing=true — now should include
	passingEntries, _, err = client.Health().Service(serviceName, "", true, nil)
	require.NoError(t, err)
	assert.NotEmpty(t, passingEntries,
		"Passing service must be included when passing=true")
}

// CH-003: Test ?passing filter excludes WARNING (not just critical)
func TestHealthServicePassingExcludesWarning(t *testing.T) {
	client := getClient(t)
	serviceName := "ch003-warning-" + randomID()

	// Register service with TTL check
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
	defer client.Agent().ServiceDeregister(serviceName)

	// Set check to warning
	checkID := "service:" + serviceName
	err = client.Agent().WarnTTL(checkID, "degraded performance")
	require.NoError(t, err)
	time.Sleep(1 * time.Second)

	// Query with passing=true — warning service MUST be excluded
	// (Consul's HealthFilterIncludeOnlyPassing excludes both warning AND critical)
	passingEntries, _, err := client.Health().Service(serviceName, "", true, nil)
	require.NoError(t, err)
	assert.Empty(t, passingEntries,
		"WARNING service must be excluded when passing=true "+
			"(Consul excludes both warning AND critical, not just critical)")
}

// ==================== P1: Important Tests ====================

// CH-004: Test health service with tag filter
func TestHealthServiceWithTag(t *testing.T) {
	client := getClient(t)
	serviceName := "ch004-tag-" + randomID()

	// Register with tags
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Tags: []string{"primary", "v2"},
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)
	defer client.Agent().ServiceDeregister(serviceName)

	// Query with matching tag
	entries, _, err := client.Health().Service(serviceName, "primary", false, nil)
	require.NoError(t, err)
	assert.NotEmpty(t, entries, "Must find service with matching tag")

	// Verify the found service has the tag
	if len(entries) > 0 {
		assert.Contains(t, entries[0].Service.Tags, "primary")
	}

	// Query with non-existent tag — must return empty
	noEntries, _, err := client.Health().Service(serviceName, "nonexistent", false, nil)
	require.NoError(t, err)
	assert.Empty(t, noEntries,
		"Must NOT find service with non-existent tag")
}

// CH-005: Test health checks for service
func TestHealthChecks(t *testing.T) {
	client := getClient(t)
	serviceName := "ch005-checks-" + randomID()

	// Register service with named TTL check
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
	defer client.Agent().ServiceDeregister(serviceName)

	// Query checks
	checks, meta, err := client.Health().Checks(serviceName, nil)
	require.NoError(t, err, "Health checks query should succeed")
	assert.Greater(t, meta.LastIndex, uint64(0))

	// Should have at least one check
	if assert.NotEmpty(t, checks, "Should have at least one check") {
		check := checks[0]
		assert.Equal(t, serviceName, check.ServiceName,
			"Check.ServiceName must match")
		assert.NotEmpty(t, check.CheckID, "Check.CheckID must be set")
		assert.NotEmpty(t, check.Status, "Check.Status must be set")
	}
}

// CH-006: Test health node
func TestHealthNode(t *testing.T) {
	client := getClient(t)

	self, err := client.Agent().Self()
	require.NoError(t, err)
	nodeName := self["Config"]["NodeName"].(string)

	checks, meta, err := client.Health().Node(nodeName, nil)
	require.NoError(t, err, "Health node query should succeed")
	assert.Greater(t, meta.LastIndex, uint64(0))
	// Checks may be empty, but should not error
	_ = checks
}

// CH-007: Test health state
func TestHealthState(t *testing.T) {
	client := getClient(t)

	// Query passing
	passingChecks, meta, err := client.Health().State(api.HealthPassing, nil)
	require.NoError(t, err, "Health state 'passing' should succeed")
	assert.Greater(t, meta.LastIndex, uint64(0))
	// Validate each check has correct status
	for _, check := range passingChecks {
		assert.Equal(t, api.HealthPassing, check.Status,
			"All checks in 'passing' state must have status 'passing'")
	}

	// Query critical
	criticalChecks, _, err := client.Health().State(api.HealthCritical, nil)
	require.NoError(t, err, "Health state 'critical' should succeed")
	for _, check := range criticalChecks {
		assert.Equal(t, api.HealthCritical, check.Status,
			"All checks in 'critical' state must have status 'critical'")
	}

	// Query any
	anyChecks, _, err := client.Health().State(api.HealthAny, nil)
	require.NoError(t, err, "Health state 'any' should succeed")
	// 'any' should return >= passing + critical
	assert.GreaterOrEqual(t, len(anyChecks), len(passingChecks),
		"'any' state should return at least as many as 'passing'")
}

// ==================== P2: Behavioral Tests ====================

// CH-008: Test health service returns empty array for unknown service (not error)
func TestHealthServiceUnknown(t *testing.T) {
	client := getClient(t)

	entries, meta, err := client.Health().Service(
		"nonexistent-service-"+randomID(), "", false, nil)
	require.NoError(t, err, "Unknown service should not error")
	assert.Empty(t, entries,
		"Unknown service must return empty array, not error")
	assert.Greater(t, meta.LastIndex, uint64(0),
		"QueryMeta.LastIndex must be > 0 even for empty results")
}

// CH-009: Test health service blocking query
func TestHealthServiceBlockingQuery(t *testing.T) {
	client := getClient(t)
	serviceName := "ch009-blocking-" + randomID()

	// Register service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)
	defer client.Agent().ServiceDeregister(serviceName)

	// Get baseline index
	_, meta, err := client.Health().Service(serviceName, "", false, nil)
	require.NoError(t, err)
	initialIndex := meta.LastIndex
	assert.Greater(t, initialIndex, uint64(0))

	// Update service in background
	go func() {
		time.Sleep(500 * time.Millisecond)
		client.Agent().ServiceRegister(&api.AgentServiceRegistration{
			ID:   serviceName,
			Name: serviceName,
			Port: 9090, // change port
		})
	}()

	// Blocking query
	entries, meta2, err := client.Health().Service(serviceName, "", false, &api.QueryOptions{
		WaitIndex: initialIndex,
		WaitTime:  5 * time.Second,
	})
	require.NoError(t, err)
	assert.NotEmpty(t, entries)
	// Index should have advanced (or at least not go backward)
	assert.GreaterOrEqual(t, meta2.LastIndex, initialIndex,
		"LastIndex must not decrease after blocking query")
}
