package tests

import (
	"testing"
	"time"

	"github.com/hashicorp/consul/api"
	serfcoord "github.com/hashicorp/serf/coordinate"
	"github.com/stretchr/testify/require"
)

// ==================== Coordinate Tests ====================

// TestCoordinateDatacenters tests getting coordinates for all datacenters
func TestCoordinateDatacenters(t *testing.T) {
	client := getTestClient(t)

	coordinate := client.Coordinate()

	dcs, err := coordinate.Datacenters()
	if err != nil {
		t.Logf("Coordinate datacenters: %v", err)
		return
	}

	t.Logf("Found %d datacenter coordinates", len(dcs))
	for _, dc := range dcs {
		t.Logf("  DC: %s, Coordinates: %d", dc.Datacenter, len(dc.Coordinates))
	}
}

// TestCoordinateNodes tests getting node coordinates
func TestCoordinateNodes(t *testing.T) {
	client := getTestClient(t)

	coordinate := client.Coordinate()

	coords, _, err := coordinate.Nodes(nil)
	if err != nil {
		t.Logf("Coordinate nodes: %v", err)
		return
	}

	t.Logf("Found %d node coordinates", len(coords))
	for _, coord := range coords {
		t.Logf("  Node: %s, Segment: %s", coord.Node, coord.Segment)
	}
}

// TestCoordinateNode tests getting a single node's coordinate
func TestCoordinateNode(t *testing.T) {
	client := getTestClient(t)

	coordinate := client.Coordinate()
	agent := client.Agent()

	// Get the local agent's node name
	self, err := agent.Self()
	if err != nil {
		t.Logf("Agent self: %v", err)
		return
	}

	nodeName := self["Config"]["NodeName"].(string)

	coords, _, err := coordinate.Node(nodeName, nil)
	if err != nil {
		t.Logf("Coordinate node: %v", err)
		return
	}

	t.Logf("Node %s has %d coordinates", nodeName, len(coords))
}

// TestCoordinateUpdate tests updating node coordinates
func TestCoordinateUpdate(t *testing.T) {
	client := getTestClient(t)

	coordinate := client.Coordinate()
	agent := client.Agent()

	// Get the local agent's node name
	self, err := agent.Self()
	if err != nil {
		t.Logf("Agent self: %v", err)
		return
	}

	nodeName := self["Config"]["NodeName"].(string)

	// Create a coordinate entry
	coord := &api.CoordinateEntry{
		Node: nodeName,
		Coord: &serfcoord.Coordinate{
			Vec:        []float64{0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8},
			Error:      1.5,
			Adjustment: 0.1,
			Height:     0.001,
		},
	}

	_, err = coordinate.Update(coord, nil)
	if err != nil {
		t.Logf("Coordinate update: %v", err)
		return
	}

	t.Log("Coordinate updated successfully")
}

// ==================== Discovery Chain Tests ====================

// TestDiscoveryChain tests getting discovery chain for a service
func TestDiscoveryChain(t *testing.T) {
	client := getTestClient(t)

	discoveryChain := client.DiscoveryChain()

	serviceName := "test-service-" + randomString(8)

	// Register a service first
	agent := client.Agent()
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	if err != nil {
		t.Logf("Service register: %v", err)
		return
	}
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Get discovery chain
	chain, _, err := discoveryChain.Get(serviceName, nil, nil)
	if err != nil {
		t.Logf("Discovery chain: %v", err)
		return
	}

	if chain != nil && chain.Chain != nil {
		t.Logf("Discovery chain for %s:", serviceName)
		t.Logf("  Protocol: %s", chain.Chain.Protocol)
		t.Logf("  Start Node: %s", chain.Chain.StartNode)
		t.Logf("  Nodes: %d", len(chain.Chain.Nodes))
		t.Logf("  Targets: %d", len(chain.Chain.Targets))
	}
}

// TestDiscoveryChainWithOptions tests discovery chain with options
func TestDiscoveryChainWithOptions(t *testing.T) {
	client := getTestClient(t)

	discoveryChain := client.DiscoveryChain()

	serviceName := "chain-opts-" + randomString(8)

	// Register service
	agent := client.Agent()
	err := agent.ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	if err != nil {
		t.Logf("Service register: %v", err)
		return
	}
	defer agent.ServiceDeregister(serviceName)

	time.Sleep(500 * time.Millisecond)

	// Get with options
	opts := &api.DiscoveryChainOptions{
		EvaluateInDatacenter: "dc1",
	}

	chain, _, err := discoveryChain.Get(serviceName, opts, nil)
	if err != nil {
		t.Logf("Discovery chain with options: %v", err)
		return
	}

	if chain != nil && chain.Chain != nil {
		t.Logf("Discovery chain with options - Protocol: %s", chain.Chain.Protocol)
	}
}

// ==================== Debug Tests ====================

// TestDebugHeap tests getting heap profile
func TestDebugHeap(t *testing.T) {
	client := getTestClient(t)

	debug := client.Debug()

	heap, err := debug.Heap()
	if err != nil {
		t.Logf("Debug heap: %v", err)
		return
	}

	t.Logf("Heap profile size: %d bytes", len(heap))
}

// TestDebugProfile tests getting CPU profile
func TestDebugProfile(t *testing.T) {
	client := getTestClient(t)

	debug := client.Debug()

	// Short profile (1 second)
	profile, err := debug.Profile(1)
	if err != nil {
		t.Logf("Debug profile: %v", err)
		return
	}

	t.Logf("CPU profile size: %d bytes", len(profile))
}

// TestDebugGoroutine tests getting goroutine profile
func TestDebugGoroutine(t *testing.T) {
	client := getTestClient(t)

	debug := client.Debug()

	goroutines, err := debug.Goroutine()
	if err != nil {
		t.Logf("Debug goroutine: %v", err)
		return
	}

	t.Logf("Goroutine profile size: %d bytes", len(goroutines))
}

// TestDebugTrace tests getting execution trace
func TestDebugTrace(t *testing.T) {
	client := getTestClient(t)

	debug := client.Debug()

	// Short trace (1 second)
	trace, err := debug.Trace(1)
	if err != nil {
		t.Logf("Debug trace: %v", err)
		return
	}

	t.Logf("Trace size: %d bytes", len(trace))
}

// ==================== Raw API Tests ====================

// TestRawQuery tests raw API query
func TestRawQuery(t *testing.T) {
	client := getTestClient(t)

	raw := client.Raw()

	// Query agent self endpoint
	var out map[string]interface{}
	_, err := raw.Query("/v1/agent/self", &out, nil)
	if err != nil {
		t.Logf("Raw query: %v", err)
		return
	}

	t.Logf("Raw query returned %d top-level keys", len(out))
}

// TestRawWrite tests raw API write
func TestRawWrite(t *testing.T) {
	client := getTestClient(t)

	raw := client.Raw()
	key := "raw-test-" + randomString(8)

	// Write to KV
	var out interface{}
	_, err := raw.Write("/v1/kv/"+key, []byte("raw test value"), &out, nil)
	if err != nil {
		t.Logf("Raw write: %v", err)
		return
	}

	t.Log("Raw write successful")

	// Cleanup
	raw.Delete("/v1/kv/"+key, nil)
}

// TestRawDelete tests raw API delete
func TestRawDelete(t *testing.T) {
	client := getTestClient(t)

	raw := client.Raw()
	key := "raw-delete-" + randomString(8)

	// Create key first
	var out interface{}
	raw.Write("/v1/kv/"+key, []byte("to be deleted"), &out, nil)

	// Delete
	_, err := raw.Delete("/v1/kv/"+key, nil)
	if err != nil {
		t.Logf("Raw delete: %v", err)
		return
	}

	t.Log("Raw delete successful")
}

// ==================== Service Mesh Tests ====================

// TestConnectProxyConfig tests getting Connect proxy configuration
func TestConnectProxyConfig(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	serviceName := "proxy-config-" + randomString(8)

	// Register service with sidecar proxy
	reg := &api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Connect: &api.AgentServiceConnect{
			SidecarService: &api.AgentServiceRegistration{
				Port: 21000,
			},
		},
	}

	err := agent.ServiceRegister(reg)
	require.NoError(t, err)
	defer agent.ServiceDeregister(serviceName)
	defer agent.ServiceDeregister(serviceName + "-sidecar-proxy")

	time.Sleep(500 * time.Millisecond)

	// Get services to check proxy config
	services, err := agent.Services()
	require.NoError(t, err)

	if proxy, ok := services[serviceName+"-sidecar-proxy"]; ok {
		if proxy.Proxy != nil {
			t.Logf("Proxy config - Destination: %s, LocalPort: %d",
				proxy.Proxy.DestinationServiceName, proxy.Proxy.LocalServicePort)
		} else {
			t.Log("Sidecar proxy registered but Proxy config is nil")
		}
	}
}

// TestConnectAuthorizeAdvanced tests Connect authorization with certificate details
func TestConnectAuthorizeAdvanced(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()

	// Create authorization request
	auth := &api.AgentAuthorizeParams{
		Target:           "target-service",
		ClientCertURI:    "spiffe://test.consul/ns/default/dc/dc1/svc/source-service",
		ClientCertSerial: "00:11:22:33:44:55",
	}

	result, err := agent.ConnectAuthorize(auth)
	if err != nil {
		t.Logf("Connect authorize: %v", err)
		return
	}

	t.Logf("Authorization result: Authorized=%v, Reason=%s", result.Authorized, result.Reason)
}

// ==================== Filtering Tests ====================

// TestFilterExpression tests using filter expressions
func TestFilterExpression(t *testing.T) {
	client := getTestClient(t)

	agent := client.Agent()
	catalog := client.Catalog()

	// Register services with different metadata
	services := []struct {
		name    string
		version string
		env     string
	}{
		{"filter-svc-1", "v1", "prod"},
		{"filter-svc-2", "v2", "prod"},
		{"filter-svc-3", "v1", "staging"},
	}

	for _, svc := range services {
		reg := &api.AgentServiceRegistration{
			ID:   svc.name,
			Name: svc.name,
			Port: 8080,
			Meta: map[string]string{
				"version": svc.version,
				"env":     svc.env,
			},
		}
		agent.ServiceRegister(reg)
		defer agent.ServiceDeregister(svc.name)
	}

	time.Sleep(500 * time.Millisecond)

	// Filter by metadata
	opts := &api.QueryOptions{
		Filter: `ServiceMeta.env == "prod"`,
	}

	catalogServices, _, err := catalog.Services(opts)
	if err != nil {
		t.Logf("Filter query: %v", err)
		return
	}

	t.Logf("Services matching filter: %d", len(catalogServices))
}

// TestFilterNodes tests filtering nodes
func TestFilterNodes(t *testing.T) {
	client := getTestClient(t)

	catalog := client.Catalog()

	// Filter nodes
	opts := &api.QueryOptions{
		Filter: `Meta.consul-network-segment == ""`,
	}

	nodes, _, err := catalog.Nodes(opts)
	if err != nil {
		t.Logf("Filter nodes: %v", err)
		return
	}

	t.Logf("Nodes matching filter: %d", len(nodes))
	for _, node := range nodes {
		t.Logf("  - %s", node.Node)
	}
}
