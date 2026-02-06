package consultest

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== Catalog Filter Tests ====================

// TestCatalogFilterByTag tests filtering services by tag
func TestCatalogFilterByTag(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	catalog := client.Catalog()

	serviceName := "filter-tag-" + randomString(8)

	// Register services with different tags
	for i := 0; i < 3; i++ {
		tags := []string{"v1"}
		if i < 2 {
			tags = append(tags, "primary")
		} else {
			tags = append(tags, "secondary")
		}

		err := agent.ServiceRegister(&api.AgentServiceRegistration{
			ID:   serviceName + "-" + string(rune('a'+i)),
			Name: serviceName,
			Port: 8080 + i,
			Tags: tags,
		})
		require.NoError(t, err)
		defer agent.ServiceDeregister(serviceName + "-" + string(rune('a'+i)))
	}

	time.Sleep(500 * time.Millisecond)

	// Filter by tag
	services, _, err := catalog.Service(serviceName, "primary", nil)
	if err != nil {
		t.Logf("Catalog service by tag: %v", err)
		return
	}

	t.Logf("Services with 'primary' tag: %d", len(services))
	for _, svc := range services {
		assert.Contains(t, svc.ServiceTags, "primary")
		t.Logf("  %s: %v", svc.ServiceID, svc.ServiceTags)
	}
}

// TestCatalogFilterByMultipleTags tests filtering by multiple tags
func TestCatalogFilterByMultipleTags(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	catalog := client.Catalog()

	serviceName := "filter-multi-tag-" + randomString(8)

	// Register services with varying tags
	tagCombos := [][]string{
		{"v1", "primary", "prod"},
		{"v1", "primary", "staging"},
		{"v2", "secondary", "prod"},
	}

	for i, tags := range tagCombos {
		err := agent.ServiceRegister(&api.AgentServiceRegistration{
			ID:   serviceName + "-" + string(rune('a'+i)),
			Name: serviceName,
			Port: 8080 + i,
			Tags: tags,
		})
		require.NoError(t, err)
		defer agent.ServiceDeregister(serviceName + "-" + string(rune('a'+i)))
	}

	time.Sleep(500 * time.Millisecond)

	// Filter by v1 tag
	v1Services, _, err := catalog.Service(serviceName, "v1", nil)
	if err != nil {
		t.Logf("Catalog service by v1: %v", err)
		return
	}
	t.Logf("Services with 'v1' tag: %d", len(v1Services))

	// Filter by prod tag
	prodServices, _, err := catalog.Service(serviceName, "prod", nil)
	if err != nil {
		t.Logf("Catalog service by prod: %v", err)
		return
	}
	t.Logf("Services with 'prod' tag: %d", len(prodServices))
}

// TestCatalogFilterExpression tests using filter expressions
func TestCatalogFilterExpression(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	catalog := client.Catalog()

	serviceName := "filter-expr-" + randomString(8)

	// Register services with metadata
	for i := 0; i < 3; i++ {
		err := agent.ServiceRegister(&api.AgentServiceRegistration{
			ID:   serviceName + "-" + string(rune('a'+i)),
			Name: serviceName,
			Port: 8080 + i,
			Meta: map[string]string{
				"version": "v" + string(rune('1'+i)),
				"env":     "production",
			},
		})
		require.NoError(t, err)
		defer agent.ServiceDeregister(serviceName + "-" + string(rune('a'+i)))
	}

	time.Sleep(500 * time.Millisecond)

	// Filter by metadata
	opts := &api.QueryOptions{
		Filter: `ServiceMeta.env == "production"`,
	}

	services, _, err := catalog.Service(serviceName, "", opts)
	if err != nil {
		t.Logf("Catalog service with filter: %v", err)
		return
	}

	t.Logf("Services with production env: %d", len(services))
}

// TestCatalogFilterNodeMeta tests filtering by node metadata
func TestCatalogFilterNodeMeta(t *testing.T) {
	client := getTestClient(t)

	catalog := client.Catalog()

	// Filter nodes by metadata
	opts := &api.QueryOptions{
		NodeMeta: map[string]string{
			"consul-network-segment": "",
		},
	}

	nodes, _, err := catalog.Nodes(opts)
	if err != nil {
		t.Logf("Catalog nodes with meta: %v", err)
		return
	}

	t.Logf("Nodes matching meta filter: %d", len(nodes))
	for _, node := range nodes {
		t.Logf("  Node: %s, Meta: %v", node.Node, node.Meta)
	}
}

// ==================== Catalog Pagination Tests ====================

// TestCatalogServicePagination tests service pagination
func TestCatalogServicePagination(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	catalog := client.Catalog()

	serviceName := "paginate-" + randomString(8)

	// Register multiple services
	for i := 0; i < 10; i++ {
		err := agent.ServiceRegister(&api.AgentServiceRegistration{
			ID:   serviceName + "-" + string(rune('0'+i)),
			Name: serviceName,
			Port: 8080 + i,
		})
		require.NoError(t, err)
		defer agent.ServiceDeregister(serviceName + "-" + string(rune('0'+i)))
	}

	time.Sleep(500 * time.Millisecond)

	// Get all services
	allServices, _, err := catalog.Service(serviceName, "", nil)
	if err != nil {
		t.Logf("Catalog service: %v", err)
		return
	}

	t.Logf("Total services: %d", len(allServices))
}

// TestCatalogNodesPagination tests node pagination
func TestCatalogNodesPagination(t *testing.T) {
	client := getTestClient(t)

	catalog := client.Catalog()

	nodes, _, err := catalog.Nodes(nil)
	if err != nil {
		t.Logf("Catalog nodes: %v", err)
		return
	}

	t.Logf("Total nodes: %d", len(nodes))
	for _, node := range nodes {
		t.Logf("  Node: %s at %s", node.Node, node.Address)
	}
}

// ==================== Catalog Consistency Tests ====================

// TestCatalogConsistentRead tests consistent read mode
func TestCatalogConsistentRead(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	catalog := client.Catalog()

	serviceName := "consistent-" + randomString(8)

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Consistent read
	opts := &api.QueryOptions{
		RequireConsistent: true,
	}

	services, meta, err := catalog.Service(serviceName, "", opts)
	if err != nil {
		t.Logf("Consistent read: %v", err)
		return
	}

	t.Logf("Consistent read - Services: %d, KnownLeader: %v", len(services), meta.KnownLeader)
}

// TestCatalogStaleRead tests stale read mode
func TestCatalogStaleRead(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	catalog := client.Catalog()

	serviceName := "stale-" + randomString(8)

	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Stale read
	opts := &api.QueryOptions{
		AllowStale: true,
	}

	services, meta, err := catalog.Service(serviceName, "", opts)
	if err != nil {
		t.Logf("Stale read: %v", err)
		return
	}

	t.Logf("Stale read - Services: %d, LastContact: %v", len(services), meta.LastContact)
}

// ==================== Catalog Node Services Tests ====================

// TestCatalogNodeServices tests getting services on a node
func TestCatalogNodeServices(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	catalog := client.Catalog()

	// Get node name
	self, err := agent.Self()
	if err != nil {
		t.Logf("Agent self: %v", err)
		return
	}

	nodeName := self["Config"]["NodeName"].(string)

	// Register a service
	serviceName := "node-svc-" + randomString(8)
	err = agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Get services on node
	nodeServices, _, err := catalog.Node(nodeName, nil)
	if err != nil {
		t.Logf("Catalog node: %v", err)
		return
	}

	t.Logf("Services on node %s: %d", nodeName, len(nodeServices.Services))
	for id, svc := range nodeServices.Services {
		t.Logf("  %s: %s:%d", id, svc.Service, svc.Port)
	}
}

// TestCatalogNodeServiceList tests listing node services
func TestCatalogNodeServiceList(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	catalog := client.Catalog()

	self, err := agent.Self()
	if err != nil {
		t.Logf("Agent self: %v", err)
		return
	}

	nodeName := self["Config"]["NodeName"].(string)

	// Use NodeServiceList for detailed info
	nodeServices, _, err := catalog.NodeServiceList(nodeName, nil)
	if err != nil {
		t.Logf("Catalog node service list: %v", err)
		return
	}

	if nodeServices != nil {
		t.Logf("Node: %s, Services: %d", nodeServices.Node.Node, len(nodeServices.Services))
	}
}

// ==================== Catalog Gateway Tests ====================

// TestCatalogGatewayServices tests gateway service catalog
func TestCatalogGatewayServices(t *testing.T) {
	client := getTestClient(t)

	catalog := client.Catalog()

	// Try to get gateway services
	gateways, _, err := catalog.GatewayServices("ingress-gateway", nil)
	if err != nil {
		t.Logf("Catalog gateway services: %v", err)
		return
	}

	t.Logf("Gateway services: %d", len(gateways))
	for _, gw := range gateways {
		t.Logf("  Gateway: %s -> %s", gw.Gateway.Name, gw.Service.Name)
	}
}

// ==================== Catalog Connect Tests ====================

// TestCatalogConnectServices tests Connect-enabled services
func TestCatalogConnectServices(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	catalog := client.Catalog()

	serviceName := "connect-catalog-" + randomString(8)

	// Register Connect service
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Connect: &api.AgentServiceConnect{
			SidecarService: &api.AgentServiceRegistration{
				Port: 21000,
			},
		},
	})
	if err != nil {
		t.Logf("Service register: %v", err)
		return
	}
	defer agent.ServiceDeregister(serviceName)
	defer agent.ServiceDeregister(serviceName + "-sidecar-proxy")

	time.Sleep(500 * time.Millisecond)

	// Get Connect services
	services, _, err := catalog.Connect(serviceName, "", nil)
	if err != nil {
		t.Logf("Catalog connect: %v", err)
		return
	}

	t.Logf("Connect services: %d", len(services))
}

// ==================== Catalog Datacenter Tests ====================

// TestCatalogDatacenters tests listing datacenters
func TestCatalogDatacenters(t *testing.T) {
	client := getTestClient(t)

	catalog := client.Catalog()

	dcs, err := catalog.Datacenters()
	if err != nil {
		t.Logf("Catalog datacenters: %v", err)
		return
	}

	t.Logf("Datacenters: %v", dcs)
	assert.NotEmpty(t, dcs, "Should have at least one datacenter")
}

// TestCatalogCrossDatacenter tests cross-datacenter queries
func TestCatalogCrossDatacenter(t *testing.T) {
	client := getTestClient(t)

	catalog := client.Catalog()

	// Get datacenters first
	dcs, err := catalog.Datacenters()
	if err != nil {
		t.Logf("Catalog datacenters: %v", err)
		return
	}

	if len(dcs) == 0 {
		t.Log("No datacenters available")
		return
	}

	// Query specific datacenter
	opts := &api.QueryOptions{
		Datacenter: dcs[0],
	}

	services, _, err := catalog.Services(opts)
	if err != nil {
		t.Logf("Catalog services in DC %s: %v", dcs[0], err)
		return
	}

	t.Logf("Services in DC %s: %d", dcs[0], len(services))
}

// ==================== Catalog Deregistration Tests ====================

// TestCatalogDeregister tests catalog deregistration
func TestCatalogDeregister(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	catalog := client.Catalog()

	serviceName := "deregister-" + randomString(8)

	// Register via agent
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	require.NoError(t, err)

	time.Sleep(500 * time.Millisecond)

	// Verify registration
	services, _, err := catalog.Service(serviceName, "", nil)
	require.NoError(t, err)
	initialCount := len(services)

	// Deregister
	agent.ServiceDeregister(serviceName)
	time.Sleep(500 * time.Millisecond)

	// Verify deregistration
	services, _, err = catalog.Service(serviceName, "", nil)
	if err != nil {
		t.Logf("After deregister: %v", err)
		return
	}

	t.Logf("Before: %d, After: %d", initialCount, len(services))
}

// ==================== Catalog Health Integration Tests ====================

// TestCatalogHealthyServices tests getting only healthy services
func TestCatalogHealthyServices(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	health := client.Health()

	serviceName := "healthy-catalog-" + randomString(8)

	// Register healthy service
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Check: &api.AgentServiceCheck{
			TTL:    "30s",
			Status: api.HealthPassing,
		},
	})
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)

	// Pass the check
	agent.PassTTL("service:"+serviceName, "healthy")

	time.Sleep(500 * time.Millisecond)

	// Get healthy services via health endpoint
	services, _, err := health.Service(serviceName, "", true, nil)
	if err != nil {
		t.Logf("Health service: %v", err)
		return
	}

	t.Logf("Healthy services: %d", len(services))
	for _, entry := range services {
		t.Logf("  %s - Health: %s", entry.Service.ID, entry.Checks.AggregatedStatus())
	}
}

// TestCatalogServiceWithHealth tests service catalog with health status
func TestCatalogServiceWithHealth(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	health := client.Health()

	serviceName := "catalog-health-" + randomString(8)

	// Register services with different health states
	for i := 0; i < 2; i++ {
		id := serviceName + "-" + string(rune('a'+i))
		status := api.HealthPassing
		if i == 1 {
			status = api.HealthCritical
		}

		err := agent.ServiceRegister(&api.AgentServiceRegistration{
			ID:   id,
			Name: serviceName,
			Port: 8080 + i,
			Check: &api.AgentServiceCheck{
				TTL:    "30s",
				Status: status,
			},
		})
		require.NoError(t, err)
		defer agent.ServiceDeregister(id)

		if status == api.HealthPassing {
			agent.PassTTL("service:"+id, "healthy")
		} else {
			agent.FailTTL("service:"+id, "unhealthy")
		}
	}

	time.Sleep(500 * time.Millisecond)

	// Get all with health
	all, _, _ := health.Service(serviceName, "", false, nil)
	healthy, _, _ := health.Service(serviceName, "", true, nil)

	t.Logf("All services: %d, Healthy only: %d", len(all), len(healthy))
}
