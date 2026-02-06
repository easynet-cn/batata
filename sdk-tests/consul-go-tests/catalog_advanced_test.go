package consultest

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Catalog Nodes Advanced Tests ====================

// TestCatalogNodesMetaFilter tests node listing with metadata filter
func TestCatalogNodesMetaFilter(t *testing.T) {
	client := getTestClient(t)

	catalog := client.Catalog()

	// First get all nodes
	nodes, _, err := catalog.Nodes(nil)
	require.NoError(t, err)
	t.Logf("Total nodes: %d", len(nodes))

	// Try filtering by metadata (if nodes have metadata)
	opts := &api.QueryOptions{
		NodeMeta: map[string]string{
			"env": "test",
		},
	}

	filteredNodes, _, err := catalog.Nodes(opts)
	require.NoError(t, err)
	t.Logf("Filtered nodes (env=test): %d", len(filteredNodes))
}

// TestCatalogNodesFilterExpression tests node listing with filter expression
func TestCatalogNodesFilterExpression(t *testing.T) {
	client := getTestClient(t)

	catalog := client.Catalog()

	// Filter by node name pattern
	opts := &api.QueryOptions{
		Filter: "Node matches \".*\"",
	}

	nodes, _, err := catalog.Nodes(opts)
	if err != nil {
		t.Logf("Filter expression not supported: %v", err)
		return
	}

	t.Logf("Nodes matching filter: %d", len(nodes))
	for _, node := range nodes {
		t.Logf("  Node: %s, Address: %s", node.Node, node.Address)
	}
}

// ==================== Catalog Services Advanced Tests ====================

// TestCatalogServicesNodeMetaFilter tests service listing with node metadata filter
func TestCatalogServicesNodeMetaFilter(t *testing.T) {
	client := getTestClient(t)

	// Register a service with node metadata
	agent := client.Agent()
	serviceName := "catalog-meta-svc-" + randomString(8)

	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Meta: map[string]string{
			"version": "1.0.0",
			"env":     "test",
		},
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Query catalog with node meta filter
	catalog := client.Catalog()
	opts := &api.QueryOptions{
		NodeMeta: map[string]string{
			"env": "test",
		},
	}

	services, _, err := catalog.Services(opts)
	require.NoError(t, err)
	t.Logf("Services with node meta filter: %d", len(services))
}

// TestCatalogServicesFilterExpression tests service listing with filter expression
func TestCatalogServicesFilterExpression(t *testing.T) {
	client := getTestClient(t)

	// Register test services
	agent := client.Agent()
	prefix := "catalog-filter-" + randomString(8)

	for i := 0; i < 3; i++ {
		reg := &api.AgentServiceRegistration{
			ID:   prefix + "-" + string(rune('a'+i)),
			Name: prefix,
			Port: 8080 + i,
			Tags: []string{"test", "filter"},
		}
		err := agent.ServiceRegister(reg)
		require.NoError(t, err)
		defer agent.ServiceDeregister(reg.ID)
	}

	time.Sleep(500 * time.Millisecond)

	// Filter with expression
	catalog := client.Catalog()
	opts := &api.QueryOptions{
		Filter: "ServiceName contains \"" + prefix + "\"",
	}

	services, _, err := catalog.Services(opts)
	if err != nil {
		t.Logf("Filter expression not supported: %v", err)
		return
	}

	t.Logf("Services matching filter: %v", services)
}

// TestCatalogServiceSingleTag tests service query with single tag
func TestCatalogServiceSingleTag(t *testing.T) {
	client := getTestClient(t)

	// Register services with tags
	agent := client.Agent()
	serviceName := "catalog-tag-svc-" + randomString(8)

	// Service with "primary" tag
	reg1 := &api.AgentServiceRegistration{
		ID:   serviceName + "-1",
		Name: serviceName,
		Port: 8080,
		Tags: []string{"primary", "production"},
	}
	err := agent.ServiceRegister(reg1)
	require.NoError(t, err)
	defer agent.ServiceDeregister(reg1.ID)

	// Service with "secondary" tag
	reg2 := &api.AgentServiceRegistration{
		ID:   serviceName + "-2",
		Name: serviceName,
		Port: 8081,
		Tags: []string{"secondary", "production"},
	}
	err = agent.ServiceRegister(reg2)
	require.NoError(t, err)
	defer agent.ServiceDeregister(reg2.ID)

	time.Sleep(500 * time.Millisecond)

	// Query by single tag
	catalog := client.Catalog()
	services, _, err := catalog.ServiceMultipleTags(serviceName, []string{"primary"}, nil)
	require.NoError(t, err)

	t.Logf("Services with 'primary' tag: %d", len(services))
	assert.True(t, len(services) >= 1, "Should find at least one service with primary tag")
}

// TestCatalogServiceMultipleTags tests service query with multiple tags
func TestCatalogServiceMultipleTags(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "catalog-multi-tag-" + randomString(8)

	// Register service with multiple tags
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Tags: []string{"web", "api", "v2"},
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Query by multiple tags (AND logic)
	catalog := client.Catalog()
	services, _, err := catalog.ServiceMultipleTags(serviceName, []string{"web", "api"}, nil)
	require.NoError(t, err)

	t.Logf("Services with 'web' AND 'api' tags: %d", len(services))
}

// TestCatalogServiceCached tests cached service queries
func TestCatalogServiceCached(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "catalog-cached-" + randomString(8)

	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	catalog := client.Catalog()

	// First query (cache miss)
	opts := &api.QueryOptions{
		UseCache: true,
	}
	services1, meta1, err := catalog.Service(serviceName, "", opts)
	require.NoError(t, err)
	t.Logf("First query: %d services, CacheHit=%v", len(services1), meta1.CacheHit)

	// Second query (should be cache hit)
	services2, meta2, err := catalog.Service(serviceName, "", opts)
	require.NoError(t, err)
	t.Logf("Second query: %d services, CacheHit=%v", len(services2), meta2.CacheHit)
}

// ==================== Catalog Connect Tests ====================

// TestCatalogConnect tests Connect-aware service queries
func TestCatalogConnect(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "catalog-connect-" + randomString(8)

	// Register a Connect-native service
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Connect: &api.AgentServiceConnect{
			Native: true,
		},
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Query Connect services
	catalog := client.Catalog()
	services, _, err := catalog.Connect(serviceName, "", nil)
	if err != nil {
		t.Logf("Connect catalog query not available: %v", err)
		return
	}

	t.Logf("Connect services for %s: %d", serviceName, len(services))
}

// TestCatalogConnectNative tests native Connect service discovery
func TestCatalogConnectNative(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "catalog-native-" + randomString(8)

	// Register native Connect service
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

	// Query with filter for native services
	catalog := client.Catalog()
	opts := &api.QueryOptions{
		Filter: "ServiceConnect.Native == true",
	}

	services, _, err := catalog.Connect(serviceName, "", opts)
	if err != nil {
		t.Logf("Connect native query not available: %v", err)
		return
	}

	t.Logf("Native Connect services: %d", len(services))
}

// ==================== Catalog Node Tests ====================

// TestCatalogNode tests getting a specific node with its services
func TestCatalogNode(t *testing.T) {
	client := getTestClient(t)

	// Get agent's node name
	agent := client.Agent()
	info, err := agent.Self()
	require.NoError(t, err)

	nodeName := info["Config"]["NodeName"].(string)
	t.Logf("Node name: %s", nodeName)

	// Register a service
	serviceName := "catalog-node-svc-" + randomString(8)
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	}
	err = agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Get node details
	catalog := client.Catalog()
	node, _, err := catalog.Node(nodeName, nil)
	require.NoError(t, err)
	require.NotNil(t, node)

	t.Logf("Node: %s, Services: %d", node.Node.Node, len(node.Services))
	for id, svc := range node.Services {
		t.Logf("  Service: %s (ID: %s)", svc.Service, id)
	}
}

// TestCatalogNodeServiceList tests listing services on a specific node
func TestCatalogNodeServiceList(t *testing.T) {
	client := getTestClient(t)

	// Get agent's node name
	agent := client.Agent()
	info, err := agent.Self()
	require.NoError(t, err)

	nodeName := info["Config"]["NodeName"].(string)

	// Register multiple services
	prefix := "catalog-nodelist-" + randomString(8)
	for i := 0; i < 3; i++ {
		reg := &api.AgentServiceRegistration{
			ID:   prefix + "-" + string(rune('a'+i)),
			Name: prefix,
			Port: 8080 + i,
		}
		err := agent.ServiceRegister(reg)
		require.NoError(t, err)
		defer agent.ServiceDeregister(reg.ID)
	}

	time.Sleep(500 * time.Millisecond)

	// Get node service list
	catalog := client.Catalog()
	services, _, err := catalog.NodeServiceList(nodeName, nil)
	require.NoError(t, err)
	require.NotNil(t, services)

	t.Logf("Node %s has %d services", nodeName, len(services.Services))
}

// TestCatalogNodeServiceListFilter tests filtering services on a node
func TestCatalogNodeServiceListFilter(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	info, err := agent.Self()
	require.NoError(t, err)

	nodeName := info["Config"]["NodeName"].(string)

	// Register services with different tags
	serviceName := "catalog-nodefilter-" + randomString(8)

	reg1 := &api.AgentServiceRegistration{
		ID:   serviceName + "-web",
		Name: serviceName,
		Port: 8080,
		Tags: []string{"web"},
	}
	err = agent.ServiceRegister(reg1)
	require.NoError(t, err)
	defer agent.ServiceDeregister(reg1.ID)

	reg2 := &api.AgentServiceRegistration{
		ID:   serviceName + "-api",
		Name: serviceName,
		Port: 8081,
		Tags: []string{"api"},
	}
	err = agent.ServiceRegister(reg2)
	require.NoError(t, err)
	defer agent.ServiceDeregister(reg2.ID)

	time.Sleep(500 * time.Millisecond)

	// Filter services on node
	catalog := client.Catalog()
	opts := &api.QueryOptions{
		Filter: "\"web\" in Tags",
	}

	services, _, err := catalog.NodeServiceList(nodeName, opts)
	if err != nil {
		t.Logf("Node service list filter not available: %v", err)
		return
	}

	t.Logf("Filtered services on node: %d", len(services.Services))
}

// ==================== Gateway Services Tests ====================

// TestCatalogGatewayServicesTerminating tests terminating gateway service associations
func TestCatalogGatewayServicesTerminating(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	gatewayName := "terminating-gw-" + randomString(8)

	// Register a terminating gateway
	reg := &api.AgentServiceRegistration{
		ID:   gatewayName,
		Name: gatewayName,
		Port: 8443,
		Kind: api.ServiceKindTerminatingGateway,
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(gatewayName)

	time.Sleep(500 * time.Millisecond)

	// Query gateway services
	catalog := client.Catalog()
	services, _, err := catalog.GatewayServices(gatewayName, nil)
	if err != nil {
		t.Logf("Gateway services query not available: %v", err)
		return
	}

	t.Logf("Terminating gateway %s linked services: %d", gatewayName, len(services))
}

// TestCatalogGatewayServicesIngress tests ingress gateway listener configurations
func TestCatalogGatewayServicesIngress(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	gatewayName := "ingress-gw-" + randomString(8)

	// Register an ingress gateway
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

	// Query gateway services
	catalog := client.Catalog()
	services, _, err := catalog.GatewayServices(gatewayName, nil)
	if err != nil {
		t.Logf("Gateway services query not available: %v", err)
		return
	}

	t.Logf("Ingress gateway %s services: %d", gatewayName, len(services))
}

// ==================== Catalog Registration Tests ====================

// TestCatalogRegistration tests direct catalog registration
func TestCatalogRegistration(t *testing.T) {
	client := getTestClient(t)

	catalog := client.Catalog()
	serviceName := "catalog-reg-" + randomString(8)

	// Get node name
	agent := client.Agent()
	info, err := agent.Self()
	require.NoError(t, err)
	nodeName := info["Config"]["NodeName"].(string)

	// Register via catalog
	reg := &api.CatalogRegistration{
		Node:    nodeName,
		Address: "127.0.0.1",
		Service: &api.AgentService{
			ID:      serviceName,
			Service: serviceName,
			Port:    9090,
			Tags:    []string{"catalog-registered"},
		},
	}

	_, err = catalog.Register(reg, nil)
	require.NoError(t, err)

	time.Sleep(500 * time.Millisecond)

	// Verify registration
	services, _, err := catalog.Service(serviceName, "", nil)
	require.NoError(t, err)
	assert.True(t, len(services) >= 1, "Should find registered service")

	// Deregister
	dereg := &api.CatalogDeregistration{
		Node:      nodeName,
		ServiceID: serviceName,
	}
	_, err = catalog.Deregister(dereg, nil)
	require.NoError(t, err)
}

// TestCatalogEnableTagOverride tests tag override setting
func TestCatalogEnableTagOverride(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "catalog-tag-override-" + randomString(8)

	// Register with EnableTagOverride
	reg := &api.AgentServiceRegistration{
		ID:                serviceName,
		Name:              serviceName,
		Port:              8080,
		Tags:              []string{"original"},
		EnableTagOverride: true,
	}
	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Get service and verify
	catalog := client.Catalog()
	services, _, err := catalog.Service(serviceName, "", nil)
	require.NoError(t, err)
	require.True(t, len(services) >= 1)

	svc := services[0]
	assert.True(t, svc.ServiceEnableTagOverride, "EnableTagOverride should be true")
	t.Logf("Service %s EnableTagOverride: %v", serviceName, svc.ServiceEnableTagOverride)
}
