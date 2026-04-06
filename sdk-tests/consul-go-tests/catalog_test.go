package tests

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== P0: Critical Tests ====================

// CC-001: Test catalog list services with response metadata validation
func TestCatalogServices(t *testing.T) {
	client := getClient(t)
	serviceName := "cc001-catalog-" + randomID()

	// Register service with tags
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Tags: []string{"web", "v2"},
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)
	defer client.Agent().ServiceDeregister(serviceName)

	// List all services from catalog
	services, meta, err := client.Catalog().Services(nil)
	require.NoError(t, err, "Catalog services should succeed")
	require.NotEmpty(t, services, "Should have at least one service")

	// QueryMeta validation
	assert.Greater(t, meta.LastIndex, uint64(0),
		"QueryMeta.LastIndex must be > 0")
	assert.True(t, meta.KnownLeader,
		"QueryMeta.KnownLeader must be true")

	// Our service must be in the list with correct tags
	tags, exists := services[serviceName]
	require.True(t, exists,
		"Our service '%s' must exist in catalog", serviceName)
	assert.Contains(t, tags, "web",
		"Service tags must contain 'web'")
	assert.Contains(t, tags, "v2",
		"Service tags must contain 'v2'")
}

// CC-002: Test catalog service nodes with field validation
func TestCatalogServiceNodes(t *testing.T) {
	client := getClient(t)
	serviceName := "cc002-nodes-" + randomID()

	// Register service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:      serviceName,
		Name:    serviceName,
		Port:    8080,
		Address: "192.168.1.50",
		Tags:    []string{"primary"},
		Meta:    map[string]string{"env": "test"},
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)
	defer client.Agent().ServiceDeregister(serviceName)

	// Get service nodes
	services, meta, err := client.Catalog().Service(serviceName, "", nil)
	require.NoError(t, err, "Catalog service should succeed")
	require.NotEmpty(t, services, "Must have at least one service node")

	assert.Greater(t, meta.LastIndex, uint64(0))

	svc := services[0]

	// Service field validation
	assert.Equal(t, serviceName, svc.ServiceID, "ServiceID must match")
	assert.Equal(t, serviceName, svc.ServiceName, "ServiceName must match")
	assert.Equal(t, 8080, svc.ServicePort, "ServicePort must match")
	assert.Equal(t, "192.168.1.50", svc.ServiceAddress,
		"ServiceAddress must match")

	// ServiceTags must be a non-nil slice (Consul guarantees [] not null)
	assert.NotNil(t, svc.ServiceTags,
		"ServiceTags must never be nil (Consul always returns [] or populated)")
	assert.Contains(t, svc.ServiceTags, "primary",
		"ServiceTags must contain 'primary'")

	// ServiceMeta
	assert.Equal(t, "test", svc.ServiceMeta["env"],
		"ServiceMeta must contain env=test")

	// Node fields
	assert.NotEmpty(t, svc.Node, "Node must be set")
	assert.NotEmpty(t, svc.Datacenter, "Datacenter must be set")
	assert.NotEmpty(t, svc.Address, "Address must be set")

	// Index fields
	assert.Greater(t, svc.CreateIndex, uint64(0),
		"CreateIndex must be > 0")
	assert.Greater(t, svc.ModifyIndex, uint64(0),
		"ModifyIndex must be > 0")
}

// CC-003: Test catalog service with tag filter
func TestCatalogServiceWithTag(t *testing.T) {
	client := getClient(t)
	serviceName := "cc003-tag-" + randomID()

	// Register with tags
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Tags: []string{"web", "v2"},
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)
	defer client.Agent().ServiceDeregister(serviceName)

	// Query with matching tag
	services, _, err := client.Catalog().Service(serviceName, "web", nil)
	require.NoError(t, err)
	assert.NotEmpty(t, services,
		"Must find service with matching tag 'web'")

	// Query with non-existent tag — must return empty
	noServices, _, err := client.Catalog().Service(serviceName, "nonexistent", nil)
	require.NoError(t, err)
	assert.Empty(t, noServices,
		"Must NOT find service with non-existent tag")
}

// CC-004: Test ServiceTags is never null for service without tags
func TestCatalogServiceTagsNeverNull(t *testing.T) {
	client := getClient(t)
	serviceName := "cc004-notags-" + randomID()

	// Register service WITHOUT tags
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)
	defer client.Agent().ServiceDeregister(serviceName)

	// Get from catalog
	services, _, err := client.Catalog().Service(serviceName, "", nil)
	require.NoError(t, err)
	require.NotEmpty(t, services)

	// ServiceTags MUST be non-nil even when empty
	// Consul guarantees [] (empty array) not null in JSON
	assert.NotNil(t, services[0].ServiceTags,
		"ServiceTags must be [] (empty array), never nil/null "+
			"(Consul JSON contract: ServiceTags is always an array)")
}

// ==================== P1: Important Tests ====================

// CC-005: Test catalog datacenters
func TestCatalogDatacenters(t *testing.T) {
	client := getClient(t)

	datacenters, err := client.Catalog().Datacenters()
	require.NoError(t, err, "Catalog datacenters should succeed")
	require.NotEmpty(t, datacenters, "Must have at least one datacenter")

	// Each datacenter must be a non-empty string
	for _, dc := range datacenters {
		assert.NotEmpty(t, dc, "Datacenter name must not be empty")
	}
}

// CC-006: Test catalog nodes
func TestCatalogNodes(t *testing.T) {
	client := getClient(t)

	nodes, meta, err := client.Catalog().Nodes(nil)
	require.NoError(t, err, "Catalog nodes should succeed")
	require.NotEmpty(t, nodes, "Must have at least one node")
	assert.Greater(t, meta.LastIndex, uint64(0))

	// Validate node fields
	node := nodes[0]
	assert.NotEmpty(t, node.Node, "Node.Node must be set")
	assert.NotEmpty(t, node.Address, "Node.Address must be set")
	assert.NotEmpty(t, node.Datacenter, "Node.Datacenter must be set")
	assert.Greater(t, node.CreateIndex, uint64(0),
		"Node.CreateIndex must be > 0")
	assert.Greater(t, node.ModifyIndex, uint64(0),
		"Node.ModifyIndex must be > 0")
}

// CC-007: Test catalog register/deregister
func TestCatalogRegisterDeregister(t *testing.T) {
	client := getClient(t)
	serviceID := "cc-register-" + randomID()
	nodeName := "test-node-" + randomID()

	// Register via catalog
	_, err := client.Catalog().Register(&api.CatalogRegistration{
		Node:    nodeName,
		Address: "192.168.1.100",
		Service: &api.AgentService{
			ID:      serviceID,
			Service: "catalog-registered",
			Port:    9090,
			Tags:    []string{"registered"},
		},
	}, nil)
	require.NoError(t, err, "Catalog register should succeed")

	time.Sleep(500 * time.Millisecond)

	// Verify registered with field validation
	services, _, err := client.Catalog().Service("catalog-registered", "", nil)
	require.NoError(t, err)
	require.NotEmpty(t, services, "Registered service must appear in catalog")
	assert.Equal(t, serviceID, services[0].ServiceID)
	assert.Equal(t, 9090, services[0].ServicePort)

	// Deregister
	_, err = client.Catalog().Deregister(&api.CatalogDeregistration{
		Node:      nodeName,
		ServiceID: serviceID,
	}, nil)
	require.NoError(t, err, "Catalog deregister should succeed")

	time.Sleep(500 * time.Millisecond)

	// Verify deregistered
	services, _, err = client.Catalog().Service("catalog-registered", "", nil)
	require.NoError(t, err)
	// Filter only our service ID
	found := false
	for _, svc := range services {
		if svc.ServiceID == serviceID {
			found = true
			break
		}
	}
	assert.False(t, found,
		"Deregistered service must not appear in catalog")
}

// CC-008: Test catalog node detail
func TestCatalogNodeDetail(t *testing.T) {
	client := getClient(t)

	self, err := client.Agent().Self()
	require.NoError(t, err)
	nodeName := self["Config"]["NodeName"].(string)

	node, meta, err := client.Catalog().Node(nodeName, nil)
	require.NoError(t, err, "Catalog node should succeed")
	require.NotNil(t, node, "Node info must not be nil")
	assert.Greater(t, meta.LastIndex, uint64(0))

	// Node fields
	assert.Equal(t, nodeName, node.Node.Node, "Node name must match")
	assert.NotEmpty(t, node.Node.Address, "Node address must be set")

	// Services on this node
	assert.NotNil(t, node.Services, "Node services must not be nil")
}

// ==================== P2: Behavioral Tests ====================

// CC-009: Test catalog blocking query
func TestCatalogBlockingQuery(t *testing.T) {
	client := getClient(t)
	serviceName := "cc009-blocking-" + randomID()

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
	_, meta, err := client.Catalog().Service(serviceName, "", nil)
	require.NoError(t, err)
	initialIndex := meta.LastIndex
	assert.Greater(t, initialIndex, uint64(0))

	// Update in background
	go func() {
		time.Sleep(500 * time.Millisecond)
		client.Agent().ServiceRegister(&api.AgentServiceRegistration{
			ID:   serviceName,
			Name: serviceName,
			Port: 9090, // change port
		})
	}()

	// Blocking query
	_, meta2, err := client.Catalog().Service(serviceName, "", &api.QueryOptions{
		WaitIndex: initialIndex,
		WaitTime:  5 * time.Second,
	})
	require.NoError(t, err)
	assert.GreaterOrEqual(t, meta2.LastIndex, initialIndex,
		"LastIndex must not decrease after blocking query")
}

// CC-010: Test catalog services returns empty tags as empty array
func TestCatalogServicesEmptyTags(t *testing.T) {
	client := getClient(t)
	serviceName := "cc010-emptytags-" + randomID()

	// Register without tags
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)
	defer client.Agent().ServiceDeregister(serviceName)

	// Get from catalog services list (map[service][]tags format)
	services, _, err := client.Catalog().Services(nil)
	require.NoError(t, err)

	tags, exists := services[serviceName]
	require.True(t, exists, "Service must exist")
	// Tags should be an empty slice, not nil
	assert.NotNil(t, tags,
		"Tags in Catalog().Services() must be [] not nil")
}
