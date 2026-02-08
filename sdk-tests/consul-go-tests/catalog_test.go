package tests

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== P0: Critical Tests ====================

// CC-001: Test catalog list services
func TestCatalogServices(t *testing.T) {
	client := getClient(t)
	serviceName := "cc001-catalog-" + randomID()

	// Register service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Tags: []string{"test", "catalog"},
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)

	// List all services from catalog
	services, _, err := client.Catalog().Services(nil)
	assert.NoError(t, err, "Catalog services should succeed")
	assert.NotEmpty(t, services, "Should have at least one service")

	// Our service should be in the list
	_, exists := services[serviceName]
	assert.True(t, exists, "Our service should exist in catalog")

	// Cleanup
	client.Agent().ServiceDeregister(serviceName)
}

// ==================== P1: Important Tests ====================

// CC-002: Test catalog service nodes
func TestCatalogService(t *testing.T) {
	client := getClient(t)
	serviceName := "cc002-nodes-" + randomID()

	// Register service
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:      serviceName,
		Name:    serviceName,
		Port:    8080,
		Address: "192.168.1.50",
		Tags:    []string{"primary"},
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)

	// Get service nodes
	services, _, err := client.Catalog().Service(serviceName, "", nil)
	assert.NoError(t, err, "Catalog service should succeed")
	assert.NotEmpty(t, services)

	if len(services) > 0 {
		service := services[0]
		assert.Equal(t, serviceName, service.ServiceID)
		assert.Equal(t, serviceName, service.ServiceName)
		assert.Equal(t, 8080, service.ServicePort)
	}

	// Cleanup
	client.Agent().ServiceDeregister(serviceName)
}

// Test catalog service with tag filter
func TestCatalogServiceWithTag(t *testing.T) {
	client := getClient(t)
	serviceName := "cc002b-tag-" + randomID()

	// Register with tags
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Tags: []string{"web", "v2"},
	})
	require.NoError(t, err)
	time.Sleep(1 * time.Second)

	// Query with tag
	services, _, err := client.Catalog().Service(serviceName, "web", nil)
	assert.NoError(t, err)
	assert.NotEmpty(t, services, "Should find service with tag")

	// Query with non-existent tag
	noServices, _, err := client.Catalog().Service(serviceName, "nonexistent", nil)
	assert.NoError(t, err)
	assert.Empty(t, noServices, "Should not find service with non-existent tag")

	// Cleanup
	client.Agent().ServiceDeregister(serviceName)
}

// ==================== P2: Nice to Have Tests ====================

// CC-003: Test catalog datacenters
func TestCatalogDatacenters(t *testing.T) {
	client := getClient(t)

	// List datacenters
	datacenters, err := client.Catalog().Datacenters()
	assert.NoError(t, err, "Catalog datacenters should succeed")
	assert.NotEmpty(t, datacenters, "Should have at least one datacenter")

	t.Logf("Found datacenters: %v", datacenters)
}

// CC-004: Test catalog nodes
func TestCatalogNodes(t *testing.T) {
	client := getClient(t)

	// List all nodes
	nodes, _, err := client.Catalog().Nodes(nil)
	assert.NoError(t, err, "Catalog nodes should succeed")
	// May or may not have nodes depending on cluster setup
	t.Logf("Found %d nodes", len(nodes))
}

// Test catalog register/deregister
func TestCatalogRegister(t *testing.T) {
	client := getClient(t)
	serviceID := "cc-register-" + randomID()
	nodeName := "test-node-" + randomID()

	// Register via catalog
	registration := &api.CatalogRegistration{
		Node:    nodeName,
		Address: "192.168.1.100",
		Service: &api.AgentService{
			ID:      serviceID,
			Service: "catalog-registered",
			Port:    9090,
		},
	}

	_, err := client.Catalog().Register(registration, nil)
	assert.NoError(t, err, "Catalog register should succeed")

	time.Sleep(500 * time.Millisecond)

	// Verify registration
	services, _, err := client.Catalog().Service("catalog-registered", "", nil)
	assert.NoError(t, err)
	if len(services) > 0 {
		assert.Equal(t, serviceID, services[0].ServiceID)
	}

	// Deregister
	_, err = client.Catalog().Deregister(&api.CatalogDeregistration{
		Node:      nodeName,
		ServiceID: serviceID,
	}, nil)
	assert.NoError(t, err, "Catalog deregister should succeed")
}

// Test catalog node services
func TestCatalogNodeServices(t *testing.T) {
	client := getClient(t)

	// Get self to find node name
	self, err := client.Agent().Self()
	if err != nil {
		t.Skip("Could not get agent self info")
	}

	config := self["Config"]
	nodeName := config["NodeName"].(string)

	// Get node services
	node, _, err := client.Catalog().Node(nodeName, nil)
	assert.NoError(t, err, "Catalog node should succeed")
	if node != nil {
		t.Logf("Node %s has %d services", nodeName, len(node.Services))
	}
}
