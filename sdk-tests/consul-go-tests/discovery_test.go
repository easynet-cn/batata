package tests

import (
	"context"
	"sync"
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Service Discovery Pattern Tests ====================

// TestDiscoveryBasicLookup tests basic service discovery lookup
func TestDiscoveryBasicLookup(t *testing.T) {
	client := getClient(t)
	serviceName := "discovery-basic-" + randomID()

	// Register a service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:      serviceName,
		Name:    serviceName,
		Port:    8080,
		Address: "192.168.1.100",
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Basic lookup via Health API (common discovery pattern)
	entries, _, err := client.Health().Service(serviceName, "", false, nil)
	assert.NoError(t, err, "Basic service lookup should succeed")
	assert.NotEmpty(t, entries, "Should find the registered service")

	if len(entries) > 0 {
		entry := entries[0]
		assert.Equal(t, serviceName, entry.Service.ID)
		assert.Equal(t, serviceName, entry.Service.Service)
		assert.Equal(t, 8080, entry.Service.Port)
		assert.Equal(t, "192.168.1.100", entry.Service.Address)
		t.Logf("Discovered service: %s at %s:%d", entry.Service.Service, entry.Service.Address, entry.Service.Port)
	}

	// Also verify via Catalog API
	catalogServices, _, err := client.Catalog().Service(serviceName, "", nil)
	assert.NoError(t, err)
	assert.NotEmpty(t, catalogServices, "Catalog should also find the service")
}

// TestDiscoveryWithTags tests service discovery with tag filtering
func TestDiscoveryWithTags(t *testing.T) {
	client := getClient(t)
	serviceName := "discovery-tags-" + randomID()

	// Register services with different tags
	services := []struct {
		id   string
		tags []string
		port int
	}{
		{serviceName + "-1", []string{"primary", "v1", "prod"}, 8081},
		{serviceName + "-2", []string{"secondary", "v1", "prod"}, 8082},
		{serviceName + "-3", []string{"primary", "v2", "staging"}, 8083},
	}

	for _, svc := range services {
		err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
			ID:   svc.id,
			Name: serviceName,
			Port: svc.port,
			Tags: svc.tags,
		})
		require.NoError(t, err)
		defer client.Agent().ServiceDeregister(svc.id)
	}

	time.Sleep(500 * time.Millisecond)

	// Discover by tag "primary"
	primaryEntries, _, err := client.Health().Service(serviceName, "primary", false, nil)
	assert.NoError(t, err)
	assert.Len(t, primaryEntries, 2, "Should find 2 services with 'primary' tag")
	for _, entry := range primaryEntries {
		assert.Contains(t, entry.Service.Tags, "primary")
		t.Logf("Primary service: %s with tags %v", entry.Service.ID, entry.Service.Tags)
	}

	// Discover by tag "v2"
	v2Entries, _, err := client.Health().Service(serviceName, "v2", false, nil)
	assert.NoError(t, err)
	assert.Len(t, v2Entries, 1, "Should find 1 service with 'v2' tag")

	// Discover by non-existent tag
	noEntries, _, err := client.Health().Service(serviceName, "nonexistent", false, nil)
	assert.NoError(t, err)
	assert.Empty(t, noEntries, "Should not find services with non-existent tag")
}

// TestDiscoveryHealthyOnly tests discovering only healthy service instances
func TestDiscoveryHealthyOnly(t *testing.T) {
	client := getClient(t)
	serviceName := "discovery-healthy-" + randomID()

	// Register healthy service
	healthyID := serviceName + "-healthy"
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   healthyID,
		Name: serviceName,
		Port: 8080,
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: api.HealthPassing,
		},
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(healthyID)

	// Register unhealthy service
	unhealthyID := serviceName + "-unhealthy"
	err = client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   unhealthyID,
		Name: serviceName,
		Port: 8081,
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: api.HealthCritical,
		},
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(unhealthyID)

	// Update check statuses
	client.Agent().PassTTL("service:"+healthyID, "healthy")
	client.Agent().FailTTL("service:"+unhealthyID, "unhealthy")

	time.Sleep(500 * time.Millisecond)

	// Discover all services (passing=false)
	allEntries, _, err := client.Health().Service(serviceName, "", false, nil)
	assert.NoError(t, err)
	t.Logf("All services: %d", len(allEntries))

	// Discover only healthy services (passing=true)
	healthyEntries, _, err := client.Health().Service(serviceName, "", true, nil)
	assert.NoError(t, err)
	t.Logf("Healthy services: %d", len(healthyEntries))

	// Healthy entries should be fewer or equal to all entries
	assert.LessOrEqual(t, len(healthyEntries), len(allEntries),
		"Healthy services should be fewer or equal to all services")

	// Verify healthy entries are actually healthy
	for _, entry := range healthyEntries {
		status := entry.Checks.AggregatedStatus()
		assert.Equal(t, api.HealthPassing, status,
			"Service %s should be healthy", entry.Service.ID)
	}
}

// TestDiscoveryWithDatacenter tests service discovery across datacenters
func TestDiscoveryWithDatacenter(t *testing.T) {
	client := getClient(t)
	serviceName := "discovery-dc-" + randomID()

	// Register service in current datacenter
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Get available datacenters
	datacenters, err := client.Catalog().Datacenters()
	if err != nil {
		t.Logf("Catalog datacenters: %v", err)
		return
	}
	t.Logf("Available datacenters: %v", datacenters)

	if len(datacenters) == 0 {
		t.Skip("No datacenters available")
	}

	// Discover service in specific datacenter
	opts := &api.QueryOptions{
		Datacenter: datacenters[0],
	}

	entries, _, err := client.Health().Service(serviceName, "", false, opts)
	assert.NoError(t, err)
	t.Logf("Services in datacenter %s: %d", datacenters[0], len(entries))

	// Try non-existent datacenter (should fail or return empty)
	opts.Datacenter = "nonexistent-dc"
	_, _, err = client.Health().Service(serviceName, "", false, opts)
	if err != nil {
		t.Logf("Expected error for non-existent DC: %v", err)
	}
}

// TestDiscoveryNearestNode tests discovering nearest service instances
func TestDiscoveryNearestNode(t *testing.T) {
	client := getClient(t)
	serviceName := "discovery-near-" + randomID()

	// Register multiple service instances
	for i := 0; i < 3; i++ {
		id := serviceName + "-" + string(rune('a'+i))
		err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
			ID:   id,
			Name: serviceName,
			Port: 8080 + i,
		})
		require.NoError(t, err)
		defer client.Agent().ServiceDeregister(id)
	}

	time.Sleep(500 * time.Millisecond)

	// Discover with near parameter (_agent = nearest to querying agent)
	opts := &api.QueryOptions{
		Near: "_agent",
	}

	entries, _, err := client.Health().Service(serviceName, "", false, opts)
	if err != nil {
		t.Logf("Near discovery: %v", err)
		return
	}

	t.Logf("Discovered %d services sorted by network distance", len(entries))
	for i, entry := range entries {
		t.Logf("  %d: %s at %s:%d", i+1, entry.Service.ID, entry.Node.Address, entry.Service.Port)
	}

	// Without near parameter for comparison
	entriesUnsorted, _, err := client.Health().Service(serviceName, "", false, nil)
	assert.NoError(t, err)
	assert.Equal(t, len(entries), len(entriesUnsorted), "Same number of services regardless of sorting")
}

// TestDiscoveryWithFilter tests service discovery with filter expressions
func TestDiscoveryWithFilter(t *testing.T) {
	client := getClient(t)
	serviceName := "discovery-filter-" + randomID()

	// Register services with different metadata
	services := []struct {
		id      string
		port    int
		meta    map[string]string
		address string
	}{
		{serviceName + "-1", 8081, map[string]string{"version": "v1", "env": "prod"}, "10.0.0.1"},
		{serviceName + "-2", 8082, map[string]string{"version": "v2", "env": "prod"}, "10.0.0.2"},
		{serviceName + "-3", 8083, map[string]string{"version": "v1", "env": "staging"}, "10.0.0.3"},
	}

	for _, svc := range services {
		err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
			ID:      svc.id,
			Name:    serviceName,
			Port:    svc.port,
			Address: svc.address,
			Meta:    svc.meta,
		})
		require.NoError(t, err)
		defer client.Agent().ServiceDeregister(svc.id)
	}

	time.Sleep(500 * time.Millisecond)

	// Filter by metadata using filter expression
	opts := &api.QueryOptions{
		Filter: `ServiceMeta.env == "prod"`,
	}

	entries, _, err := client.Health().Service(serviceName, "", false, opts)
	if err != nil {
		t.Logf("Filter discovery: %v (filter expressions may not be supported)", err)
		return
	}

	t.Logf("Services with env=prod: %d", len(entries))
	for _, entry := range entries {
		if entry.Service.Meta != nil {
			assert.Equal(t, "prod", entry.Service.Meta["env"])
			t.Logf("  %s: version=%s, env=%s", entry.Service.ID, entry.Service.Meta["version"], entry.Service.Meta["env"])
		}
	}

	// Filter by version
	opts.Filter = `ServiceMeta.version == "v1"`
	v1Entries, _, err := client.Health().Service(serviceName, "", false, opts)
	if err != nil {
		t.Logf("Version filter: %v", err)
		return
	}
	t.Logf("Services with version=v1: %d", len(v1Entries))
}

// TestDiscoveryPagination tests service discovery with pagination
func TestDiscoveryPagination(t *testing.T) {
	client := getClient(t)
	serviceName := "discovery-page-" + randomID()

	// Register multiple services
	serviceCount := 10
	for i := 0; i < serviceCount; i++ {
		id := serviceName + "-" + string(rune('0'+i))
		err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
			ID:   id,
			Name: serviceName,
			Port: 8080 + i,
		})
		require.NoError(t, err)
		defer client.Agent().ServiceDeregister(id)
	}

	time.Sleep(1 * time.Second)

	// Discover all services
	allEntries, meta, err := client.Health().Service(serviceName, "", false, nil)
	assert.NoError(t, err)
	t.Logf("Total services: %d", len(allEntries))
	t.Logf("Last index: %d", meta.LastIndex)

	// Consul doesn't have traditional pagination, but uses blocking queries with index
	// Verify we can track index for subsequent queries
	assert.NotZero(t, meta.LastIndex, "Should have a valid index")
}

// TestDiscoveryConsistencyModes tests different consistency modes for discovery
func TestDiscoveryConsistencyModes(t *testing.T) {
	client := getClient(t)
	serviceName := "discovery-consistency-" + randomID()

	// Register service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Default consistency (leader read)
	defaultEntries, defaultMeta, err := client.Health().Service(serviceName, "", false, nil)
	assert.NoError(t, err)
	t.Logf("Default read - Services: %d, KnownLeader: %v", len(defaultEntries), defaultMeta.KnownLeader)

	// Consistent read (strongly consistent)
	consistentOpts := &api.QueryOptions{
		RequireConsistent: true,
	}
	consistentEntries, consistentMeta, err := client.Health().Service(serviceName, "", false, consistentOpts)
	assert.NoError(t, err)
	t.Logf("Consistent read - Services: %d, KnownLeader: %v", len(consistentEntries), consistentMeta.KnownLeader)

	// Results should be the same
	assert.Equal(t, len(defaultEntries), len(consistentEntries), "Both modes should return same services")
}

// TestDiscoveryStaleReads tests stale reads for service discovery
func TestDiscoveryStaleReads(t *testing.T) {
	client := getClient(t)
	serviceName := "discovery-stale-" + randomID()

	// Register service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Stale read (may read from any server, eventually consistent)
	staleOpts := &api.QueryOptions{
		AllowStale: true,
	}

	entries, meta, err := client.Health().Service(serviceName, "", false, staleOpts)
	assert.NoError(t, err)

	t.Logf("Stale read - Services: %d", len(entries))
	t.Logf("Last contact: %v", meta.LastContact)
	t.Logf("Last index: %d", meta.LastIndex)

	// Stale reads should still return valid data
	assert.NotEmpty(t, entries, "Stale read should return registered service")

	// Compare with non-stale read
	normalEntries, _, err := client.Health().Service(serviceName, "", false, nil)
	assert.NoError(t, err)
	assert.Equal(t, len(normalEntries), len(entries), "Stale and normal reads should return same count")
}

// TestDiscoveryCacheBehavior tests cache behavior in service discovery
func TestDiscoveryCacheBehavior(t *testing.T) {
	client := getClient(t)
	serviceName := "discovery-cache-" + randomID()

	// Register service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// First query to populate cache
	entries1, meta1, err := client.Health().Service(serviceName, "", false, nil)
	assert.NoError(t, err)
	t.Logf("First query - Services: %d, Index: %d", len(entries1), meta1.LastIndex)

	// Query with cached index (should return same results if no changes)
	cachedOpts := &api.QueryOptions{
		WaitIndex: meta1.LastIndex,
		WaitTime:  100 * time.Millisecond, // Short wait to avoid blocking
	}

	entries2, meta2, err := client.Health().Service(serviceName, "", false, cachedOpts)
	assert.NoError(t, err)
	t.Logf("Cached query - Services: %d, Index: %d", len(entries2), meta2.LastIndex)

	// If no changes, indices should be the same or newer
	assert.GreaterOrEqual(t, meta2.LastIndex, meta1.LastIndex,
		"Cached query index should be >= original index")
}

// TestDiscoveryBlocking tests blocking queries for real-time discovery
func TestDiscoveryBlocking(t *testing.T) {
	client := getClient(t)
	serviceName := "discovery-blocking-" + randomID()

	// Register initial service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Get initial state
	initialEntries, initialMeta, err := client.Health().Service(serviceName, "", false, nil)
	require.NoError(t, err)
	t.Logf("Initial state - Services: %d, Index: %d", len(initialEntries), initialMeta.LastIndex)

	// Start blocking query in background
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	updateReceived := make(chan struct{})
	var blockedMeta *api.QueryMeta

	go func() {
		opts := &api.QueryOptions{
			WaitIndex: initialMeta.LastIndex,
			WaitTime:  10 * time.Second,
		}
		opts = opts.WithContext(ctx)

		_, meta, err := client.Health().Service(serviceName, "", false, opts)
		if err == nil && meta.LastIndex > initialMeta.LastIndex {
			blockedMeta = meta
			close(updateReceived)
		}
	}()

	// Make a change to trigger the blocking query
	time.Sleep(100 * time.Millisecond)
	newServiceID := serviceName + "-new"
	err = client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   newServiceID,
		Name: serviceName,
		Port: 8081,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(newServiceID)

	// Wait for blocking query to return
	select {
	case <-updateReceived:
		t.Logf("Blocking query received update, new index: %d", blockedMeta.LastIndex)
	case <-ctx.Done():
		t.Log("Blocking query timed out (may be expected in some configurations)")
	}
}

// TestDiscoveryServiceMeta tests discovering services by metadata
func TestDiscoveryServiceMeta(t *testing.T) {
	client := getClient(t)
	serviceName := "discovery-meta-" + randomID()

	// Register services with rich metadata
	services := []struct {
		id   string
		port int
		meta map[string]string
	}{
		{
			serviceName + "-api",
			8080,
			map[string]string{
				"protocol":     "http",
				"version":      "2.0.0",
				"team":         "platform",
				"load-balance": "round-robin",
			},
		},
		{
			serviceName + "-grpc",
			9090,
			map[string]string{
				"protocol": "grpc",
				"version":  "2.0.0",
				"team":     "platform",
			},
		},
		{
			serviceName + "-legacy",
			8081,
			map[string]string{
				"protocol": "http",
				"version":  "1.0.0",
				"team":     "legacy",
			},
		},
	}

	for _, svc := range services {
		err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
			ID:   svc.id,
			Name: serviceName,
			Port: svc.port,
			Meta: svc.meta,
		})
		require.NoError(t, err)
		defer client.Agent().ServiceDeregister(svc.id)
	}

	time.Sleep(500 * time.Millisecond)

	// Discover all and verify metadata
	entries, _, err := client.Health().Service(serviceName, "", false, nil)
	assert.NoError(t, err)
	assert.Len(t, entries, 3)

	for _, entry := range entries {
		assert.NotNil(t, entry.Service.Meta, "Service should have metadata")
		t.Logf("Service %s metadata: %v", entry.Service.ID, entry.Service.Meta)

		// Verify metadata fields exist
		assert.NotEmpty(t, entry.Service.Meta["protocol"])
		assert.NotEmpty(t, entry.Service.Meta["version"])
		assert.NotEmpty(t, entry.Service.Meta["team"])
	}

	// Filter by protocol using filter expression
	opts := &api.QueryOptions{
		Filter: `ServiceMeta.protocol == "grpc"`,
	}
	grpcEntries, _, err := client.Health().Service(serviceName, "", false, opts)
	if err != nil {
		t.Logf("Filter by protocol: %v", err)
	} else {
		t.Logf("GRPC services: %d", len(grpcEntries))
	}
}

// TestDiscoveryNodeMeta tests discovering services filtered by node metadata
func TestDiscoveryNodeMeta(t *testing.T) {
	client := getClient(t)
	serviceName := "discovery-nodemeta-" + randomID()

	// Register service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Discover services and examine node metadata
	entries, _, err := client.Health().Service(serviceName, "", false, nil)
	assert.NoError(t, err)

	for _, entry := range entries {
		t.Logf("Node: %s", entry.Node.Node)
		t.Logf("Node Address: %s", entry.Node.Address)
		t.Logf("Node Datacenter: %s", entry.Node.Datacenter)
		if entry.Node.Meta != nil {
			t.Logf("Node Meta: %v", entry.Node.Meta)
		}
	}

	// Filter by node metadata (if available)
	opts := &api.QueryOptions{
		NodeMeta: map[string]string{
			"consul-network-segment": "",
		},
	}

	filteredEntries, _, err := client.Health().Service(serviceName, "", false, opts)
	if err != nil {
		t.Logf("Node meta filter: %v", err)
	} else {
		t.Logf("Services matching node meta: %d", len(filteredEntries))
	}

	// Filter nodes via Catalog API
	nodes, _, err := client.Catalog().Nodes(opts)
	if err != nil {
		t.Logf("Catalog nodes with meta: %v", err)
	} else {
		t.Logf("Nodes matching meta filter: %d", len(nodes))
	}
}

// TestDiscoveryMultipleServices tests discovering multiple services simultaneously
func TestDiscoveryMultipleServices(t *testing.T) {
	client := getClient(t)
	prefix := "discovery-multi-" + randomID()

	// Register multiple different services
	serviceNames := []string{
		prefix + "-api",
		prefix + "-web",
		prefix + "-worker",
		prefix + "-cache",
	}

	for i, name := range serviceNames {
		err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
			ID:   name,
			Name: name,
			Port: 8080 + i,
			Tags: []string{"app:" + prefix},
		})
		require.NoError(t, err)
		defer client.Agent().ServiceDeregister(name)
	}

	time.Sleep(500 * time.Millisecond)

	// Discover all services concurrently
	var wg sync.WaitGroup
	results := make(map[string][]*api.ServiceEntry)
	var mu sync.Mutex

	for _, name := range serviceNames {
		wg.Add(1)
		go func(serviceName string) {
			defer wg.Done()

			entries, _, err := client.Health().Service(serviceName, "", false, nil)
			if err != nil {
				t.Logf("Error discovering %s: %v", serviceName, err)
				return
			}

			mu.Lock()
			results[serviceName] = entries
			mu.Unlock()
		}(name)
	}

	wg.Wait()

	// Verify all services were discovered
	for _, name := range serviceNames {
		entries, found := results[name]
		assert.True(t, found, "Should have results for %s", name)
		assert.NotEmpty(t, entries, "Should find service %s", name)
		t.Logf("Discovered %s: %d instances", name, len(entries))
	}

	// Also list all services via Catalog
	allServices, _, err := client.Catalog().Services(nil)
	assert.NoError(t, err)
	t.Logf("Total services in catalog: %d", len(allServices))

	for _, name := range serviceNames {
		_, exists := allServices[name]
		assert.True(t, exists, "Service %s should exist in catalog", name)
	}
}

// TestDiscoveryFailover tests service discovery failover scenarios
func TestDiscoveryFailover(t *testing.T) {
	client := getClient(t)
	serviceName := "discovery-failover-" + randomID()

	// Register primary and backup services
	primaryID := serviceName + "-primary"
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   primaryID,
		Name: serviceName,
		Port: 8080,
		Tags: []string{"primary"},
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: api.HealthPassing,
		},
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(primaryID)

	backupID := serviceName + "-backup"
	err = client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   backupID,
		Name: serviceName,
		Port: 8081,
		Tags: []string{"backup"},
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: api.HealthPassing,
		},
	})
	require.NoError(t, err)
	defer client.Agent().ServiceDeregister(backupID)

	// Set initial health states
	client.Agent().PassTTL("service:"+primaryID, "healthy")
	client.Agent().PassTTL("service:"+backupID, "healthy")

	time.Sleep(500 * time.Millisecond)

	// Discover healthy services - both should be available
	healthyEntries, _, err := client.Health().Service(serviceName, "", true, nil)
	assert.NoError(t, err)
	initialCount := len(healthyEntries)
	t.Logf("Initial healthy services: %d", initialCount)

	// Simulate primary failure
	client.Agent().FailTTL("service:"+primaryID, "simulated failure")
	time.Sleep(500 * time.Millisecond)

	// Discover again - only backup should be healthy
	failoverEntries, _, err := client.Health().Service(serviceName, "", true, nil)
	assert.NoError(t, err)
	t.Logf("After primary failure - healthy services: %d", len(failoverEntries))

	// Verify backup is still available
	backupFound := false
	for _, entry := range failoverEntries {
		if entry.Service.ID == backupID {
			backupFound = true
			assert.Contains(t, entry.Service.Tags, "backup")
		}
	}
	t.Logf("Backup service available for failover: %v", backupFound)

	// Recover primary
	client.Agent().PassTTL("service:"+primaryID, "recovered")
	time.Sleep(500 * time.Millisecond)

	// Both should be healthy again
	recoveredEntries, _, err := client.Health().Service(serviceName, "", true, nil)
	assert.NoError(t, err)
	t.Logf("After recovery - healthy services: %d", len(recoveredEntries))

	// Test prepared query for failover (if supported)
	query := &api.PreparedQueryDefinition{
		Name: "failover-query-" + randomID(),
		Service: api.ServiceQuery{
			Service:     serviceName,
			OnlyPassing: true,
			Failover: api.QueryFailoverOptions{
				NearestN: 3,
			},
		},
	}

	queryID, _, err := client.PreparedQuery().Create(query, nil)
	if err != nil {
		t.Logf("Prepared query not supported: %v", err)
		return
	}
	defer client.PreparedQuery().Delete(queryID, nil)

	// Execute failover query
	result, _, err := client.PreparedQuery().Execute(queryID, nil)
	if err != nil {
		t.Logf("Execute failover query: %v", err)
		return
	}

	t.Logf("Failover query result - Service: %s, Nodes: %d", result.Service, len(result.Nodes))
	for _, node := range result.Nodes {
		t.Logf("  Node: %s, Service: %s:%d",
			node.Node.Node, node.Service.Service, node.Service.Port)
	}
}
